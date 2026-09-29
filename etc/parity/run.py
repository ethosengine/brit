#!/usr/bin/env python3
"""Run witnessed parity helpers in disposable copies, never in the source checkout."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import shutil
import signal
import subprocess
import sys
import tempfile
import time

ROOT = Path(__file__).resolve().parents[2]
METHODS = ['tests/parity.sh', 'tests/helpers.sh', 'tests/utilities.sh',
           'etc/parity/runtime.sh', 'etc/parity/event.py', 'etc/parity/run.py']


def digest(path):
    h = hashlib.sha256()
    with Path(path).open('rb') as source:
        for block in iter(lambda: source.read(1024 * 1024), b''):
            h.update(block)
    return h.hexdigest()


def binary(path, version=True):
    resolved = Path(path).resolve(strict=True)
    if not resolved.is_file() or not os.access(resolved, os.X_OK):
        raise ValueError(f'not an executable: {resolved}')
    data = {'path': str(resolved), 'sha256': digest(resolved), 'version': None}
    if version:
        result = subprocess.run([str(resolved), '--version'], capture_output=True, text=True, timeout=10)
        data['version'] = result.stdout.strip() if result.returncode == 0 else None
        data['versionExit'] = result.returncode
    return data


def copy_inputs(root, destination):
    # Copy only suite inputs; repository metadata and development state never enter cwd.
    for relative in ['tests/parity.sh', 'tests/helpers.sh', 'tests/utilities.sh',
                     'etc/parity/runtime.sh', 'etc/parity/event.py']:
        target = destination / relative
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(root / relative, target)
    for relative in ['tests/journey/parity', 'tests/fixtures', 'tests/snapshots/parity']:
        source = root / relative
        if source.exists():
            # Refuse external symlinks rather than copying links back to live source.
            for item in source.rglob('*'):
                if item.is_symlink():
                    item.resolve(strict=True).relative_to(root)
            shutil.copytree(source, destination / relative, symlinks=False)


def captured(path, limit=65536):
    with path.open('rb') as stream:
        data = stream.read(limit)
    size = path.stat().st_size
    return {'text': data.decode('utf-8', errors='replace'), 'bytes': size,
            'truncated': size > limit, 'sha256': digest(path)}


def execute(root, suite, lane, binaries, timeout):
    source = (root / suite).resolve(strict=True)
    source.relative_to((root / 'tests/journey/parity').resolve())
    suite_sha = digest(source)
    started = time.monotonic()
    with tempfile.TemporaryDirectory(prefix='brit-parity-receipt-') as directory:
        work = Path(directory)
        isolated = work / 'source'
        isolated.mkdir()
        copy_inputs(root, isolated)
        home = work / 'home'
        home.mkdir()
        tools = work / 'bin'
        tools.mkdir()
        (tools / 'git').symlink_to(binaries['git']['path'])
        temporary = work / 'tmp'
        temporary.mkdir()
        events_path = work / 'events.jsonl'
        env = {'PATH': str(tools) + os.pathsep + os.defpath, 'HOME': str(home),
               'XDG_CONFIG_HOME': str(home / 'config'), 'LC_ALL': 'C', 'LANG': 'C',
               'TERM': 'dumb', 'TMPDIR': str(temporary), 'GIT_CONFIG_NOSYSTEM': '1',
               'GIT_CONFIG_GLOBAL': os.devnull, 'GIT_TERMINAL_PROMPT': '0',
               'GIT_PAGER': 'cat', 'GIX': binaries['brit']['path'],
               'EIN': binaries['ein']['path'], 'JTT': binaries['jtt']['path'],
               'KIND': 'max', 'PARITY_HASH_KINDS': lane,
               'PARITY_SUITE': suite, 'PARITY_SUITE_SHA256': suite_sha,
               'PARITY_SUITE_PATH': str(isolated / suite), 'PARITY_EVENTS': str(events_path),
               'PARITY_RUNTIME_HELPER': str(isolated / 'etc/parity/runtime.sh'),
               'PARITY_EVENT_HELPER': str(isolated / 'etc/parity/event.py'),
               'PARITY_PYTHON': sys.executable}
        stdout, stderr = work / 'stdout', work / 'stderr'
        with stdout.open('wb') as out, stderr.open('wb') as err:
            process = subprocess.Popen(['/bin/bash', str(isolated / 'tests/parity.sh'), str(isolated / suite)],
                                       cwd=isolated, env=env, stdout=out, stderr=err, start_new_session=True)
            try:
                code = process.wait(timeout=timeout)
                status = 'completed' if code == 0 else 'failed'
            except subprocess.TimeoutExpired:
                os.killpg(process.pid, signal.SIGKILL)
                code = process.wait()
                status = 'timeout'
            finally:
                # Kill background children even when a helper replaced an EXIT trap.
                try:
                    os.killpg(process.pid, signal.SIGKILL)
                except ProcessLookupError:
                    pass
        events, problems = [], []
        if events_path.exists():
            for number, line in enumerate(events_path.read_text().splitlines(), 1):
                try:
                    event = json.loads(line)
                    if not isinstance(event, dict):
                        raise ValueError('event is not an object')
                    events.append(event)
                except ValueError as error:
                    problems.append(f'event line {number}: {error}')
        active = {}
        for event in events:
            key = event.get('invocationId')
            if event.get('event') == 'assertion-start':
                active[key] = event
            elif event.get('event') == 'assertion-end':
                if key not in active:
                    problems.append(f'unmatched assertion end: {key}')
                active.pop(key, None)
        if problems:
            status = 'invalid-events'
        if (isolated / (suite + '.stop')).exists():
            status = 'stopped'
        return {'suite': suite, 'suiteSha256': suite_sha, 'hashKind': lane, 'status': status,
                'exitCode': code, 'elapsedSeconds': round(time.monotonic() - started, 3),
                'timeoutSeconds': timeout, 'events': events, 'incompleteAssertions': list(active.values()),
                'eventProblems': problems, 'stdout': captured(stdout), 'stderr': captured(stderr)}


def input_inventory(root):
    inputs = {}
    for relative in ['tests/journey/parity', 'tests/fixtures', 'tests/snapshots/parity']:
        for path in sorted((root / relative).rglob('*')):
            if path.is_file():
                inputs[path.relative_to(root).as_posix()] = digest(path)
    return inputs


def run(root, suites, binaries, timeout, output=None):
    inputs = input_inventory(root)
    result = {'schemaVersion': 1, 'evidenceKind': 'runtime-parity-receipt',
              'binaries': binaries, 'method': {name: digest(root / name) for name in METHODS},
              'inputs': inputs, 'environment': {'platform': platform.platform(),
              'python': sys.version.split()[0], 'bash': subprocess.check_output(['/bin/bash', '--version'], text=True).splitlines()[0],
              'cleanEnvironment': True, 'cwd': 'disposable copied suite inputs', 'kind': 'max',
              'oracleScope': 'installed Git binary version; not pinned source census equivalence'},
              'expectedRuns': [{'suite': suite, 'hashKind': lane} for suite in suites for lane in ['sha1', 'sha256']],
              'measurementComplete': False, 'runs': []}
    for suite in suites:
        for lane in ['sha1', 'sha256']:
            result['runs'].append(execute(root, suite, lane, binaries, timeout))
            if output is not None:
                output.write_text(json.dumps(result, indent=2) + '\n')
    for data in binaries.values():
        data['sha256After'] = digest(data['path'])
    result['stableInputs'] = input_inventory(root) == inputs and all(digest(root / name) == sha for name, sha in result['method'].items())
    result['measurementComplete'] = result['stableInputs'] and all(data['sha256'] == data['sha256After'] for data in binaries.values()) and all(row['status'] != 'invalid-events' for row in result['runs'])
    result['complete'] = result['stableInputs'] and all(data['sha256'] == data['sha256After'] for data in binaries.values()) and all(row['status'] == 'completed' and not row['incompleteAssertions'] for row in result['runs'])
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ['brit', 'ein', 'jtt', 'git']:
        parser.add_argument('--' + name, required=True, type=Path)
    parser.add_argument('--suite', action='append', help='repo-relative test suite; default every parity suite')
    parser.add_argument('--timeout', type=float, default=120)
    parser.add_argument('--output', required=True, type=Path)
    args = parser.parse_args()
    if args.timeout <= 0:
        parser.error('--timeout must be positive')
    binaries = {name: binary(getattr(args, name), version=name != 'jtt') for name in ['brit', 'ein', 'jtt', 'git']}
    suites = args.suite or [path.relative_to(ROOT).as_posix() for path in sorted((ROOT / 'tests/journey/parity').glob('*.sh'))]
    args.output.parent.mkdir(parents=True, exist_ok=True)
    result = run(ROOT, suites, binaries, args.timeout, output=args.output)
    args.output.write_text(json.dumps(result, indent=2) + '\n')
    print(json.dumps({'receipt': str(args.output), 'runs': len(result['runs']), 'complete': result['complete'], 'measurementComplete': result['measurementComplete']}))
    return 0 if result['measurementComplete'] else 2


if __name__ == '__main__':
    sys.exit(main())
