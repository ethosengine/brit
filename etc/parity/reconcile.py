#!/usr/bin/env python3
"""Reconcile bounded Bash source candidates with observed parity assertions.

No shell is evaluated. Option spelling associations are candidates, never coverage.
"""
from __future__ import annotations
import argparse
from collections import Counter
import hashlib
import json
from pathlib import Path
import re
import shlex
import sys
import run as runtime_runner

ROOT = Path(__file__).resolve().parents[2]
HELPERS = ('expect_parity_reset', 'expect_parity', 'compat_effect', 'shortcoming')


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def literal_arguments(text):
    if any(char in text for char in ('$','`')) or text.rstrip().endswith('\\'):
        return None
    try:
        return shlex.split(text, comments=True)
    except ValueError:
        return None


def source_rows(root):
    """Recognize explicit title/it lines; arbitrary Bash stays unresolved."""
    rows = []
    for path in sorted((root / 'tests/journey/parity').glob('*.sh')):
        relative = path.relative_to(root).as_posix()
        title, title_line, current = None, None, None
        lines = path.read_text().splitlines()
        for number, line in enumerate(lines, 1):
            match = re.match(r'^\s*(title|it)\s+(.+)', line)
            if match:
                if current:
                    current['endLine'] = number - 1
                args = literal_arguments(match[2].split(' && ', 1)[0])
                label = args[0] if args and len(args) == 1 else None
                if match[1] == 'title':
                    title, title_line = label, number
                elif current and current['rowKind'] == 'title-only':
                    rows.remove(current)
                current = {'id': f'{relative}:{number}', 'source': relative,
                    'sourceSha256': digest(path), 'line': number, 'title': title,
                    'titleLine': title_line, 'it': label if match[1] == 'it' else None,
                    'rowKind': 'assertion-row' if match[1] == 'it' else 'title-only',
                    'commandCandidate': path.stem,
                    'sourceMapping': 'literal' if title is not None and label is not None else 'unresolved',
                    'helpers': [], 'optionCandidates': [], 'infrastructure': path.name.startswith('_')}
                rows.append(current)
            helper = re.match(r'^\s*(' + '|'.join(HELPERS) + r')\s+(.+)', line)
            if helper and current:
                args = literal_arguments(helper[2])
                argv = args[args.index('--') + 1:] if args and '--' in args else None
                options = []
                if argv and argv[0] == path.stem:
                    for arg in argv[1:]:
                        if arg == '--':
                            break
                        if re.fullmatch(r'--[A-Za-z0-9][\w-]*(?:=.*)?|-[A-Za-z0-9]', arg):
                            options.append(arg.split('=', 1)[0])
                current['helpers'].append({'helper': helper[1], 'line': number,
                    'argv': argv, 'mapping': 'literal' if argv else 'unresolved'})
                current['optionCandidates'] = sorted(set(current['optionCandidates'] + options))
        if current:
            current['endLine'] = len(lines)
    return rows


def freshness(receipt, root):
    issues = []
    if receipt.get('stableInputs') is not True:
        issues.append('run-inputs-unverified')
    if receipt.get('inputs') != runtime_runner.input_inventory(root):
        issues.append('fixture-input-drift-or-unverified')
    methods = receipt.get('method', {})
    if not isinstance(methods, dict) or not methods:
        return ['method-unverified']
    for name in ('tests/parity.sh', 'tests/helpers.sh', 'tests/utilities.sh', 'etc/parity/runtime.sh', 'etc/parity/event.py', 'etc/parity/run.py'):
        if name not in methods:
            issues.append('method-unbound:' + name)
    for name, expected in methods.items():
        path = (root / name).resolve()
        if not path.is_relative_to(root.resolve()) or not path.is_file():
            issues.append('method-missing:' + name)
        elif digest(path) != expected:
            issues.append('method-drift:' + name)
    binaries = receipt.get('binaries', {})
    for name in ('brit', 'git', 'ein', 'jtt'):
        binary = binaries.get(name, {})
        if not binary.get('path') or not re.fullmatch('[0-9a-f]{64}', binary.get('sha256', '')) or (name != 'jtt' and not binary.get('version')):
            issues.append('binary-unverified:' + name)
        elif binary.get('sha256After') != binary['sha256']:
            issues.append('binary-drift:' + name)
    return issues


def strength(event):
    if event.get('event') in ('skip', 'deferred'):
        return 'skip' if event['event'] == 'skip' else 'deferred-unexecuted'
    if event.get('helper') == 'compat_effect':
        return 'deferred-exit-comparison'
    return {'effect': 'exit-comparison', 'bytes': 'normalized-output-comparison',
            'exit': 'expected-exit', 'snapshot': 'normalized-snapshot'}.get(event.get('mode'), 'unclassified')


def reconcile(census, receipts, root=ROOT):
    root = Path(root)
    if census.get('schemaVersion') != 1:
        raise ValueError('unsupported census schema')
    names = [c['name'] for c in census['commands']]
    if len(set(names)) != len(names):
        raise ValueError('duplicate census command')
    rows = source_rows(root)
    observations, unmatched, runs, issues = [], [], [], []
    receipt_ids = [hashlib.sha256(json.dumps(r, sort_keys=True, separators=(',', ':')).encode()).hexdigest() for r in receipts]
    if len(set(receipt_ids)) != len(receipt_ids):
        raise ValueError('duplicate receipt input would duplicate observations')
    for receipt_index, receipt in enumerate(receipts):
        if receipt.get('schemaVersion') != 1 or receipt.get('evidenceKind') != 'runtime-parity-receipt':
            raise ValueError('unsupported runtime receipt schema')
        method_issues = freshness(receipt, root)
        for run_index, run in enumerate(receipt.get('runs', [])):
            suite = run.get('suite', '')
            source = (root / suite).resolve()
            stale = list(method_issues)
            if run.get('status') not in ('completed', 'failed', 'timeout', 'stopped') or run.get('eventProblems'):
                stale.append('run-events-invalid')
            if not source.is_relative_to(root.resolve()) or not source.is_file():
                stale.append('suite-missing')
            elif digest(source) != run.get('suiteSha256'):
                stale.append('suite-drift')
            run_id = f'{receipt_index}:{run_index}'
            runs.append({'id': run_id, 'suite': suite, 'hashKind': run.get('hashKind'),
                         'status': run.get('status'), 'exitCode': run.get('exitCode'),
                         'freshnessIssues': stale, 'binaries': receipt.get('binaries', {})})
            starts = {}
            for start in run.get('events', []):
                if start.get('event') == 'assertion-start':
                    starts.setdefault(start.get('invocationId'), []).append(start)
            end_ids = Counter(e.get('invocationId') for e in run.get('events', []) if e.get('event') == 'assertion-end')
            for event in run.get('events', []):
                if event.get('event') == 'assertion-start':
                    continue
                line = event.get('line')
                candidates = [row for row in rows if row['source'] == suite and isinstance(line, int)
                              and (row['titleLine'] if event.get('event') == 'skip' and row['titleLine'] else row['line']) <= line <= row.get('endLine', row['line'])]
                candidates = [row for row in candidates if (event.get('title') in (None, '', row['title']))
                              and (event.get('event') == 'skip' or event.get('it') in (None, '', row['it']))]
                event_issues = list(stale)
                if event.get('suite') != suite or event.get('suiteSha256') != run.get('suiteSha256') or event.get('hashKind') != run.get('hashKind'):
                    event_issues.append('event-source-mismatch')
                assertion = event.get('event') == 'assertion-end'
                if assertion:
                    paired = starts.get(event.get('invocationId'), [])
                    if len(paired) != 1 or any(paired[0].get(key) != event.get(key) for key in ('suite', 'suiteSha256', 'hashKind', 'line', 'helper', 'mode', 'binary', 'argv')):
                        event_issues.append('assertion-start-unbound')
                if assertion and (not event.get('invocationId') or end_ids[event['invocationId']] != 1):
                    event_issues.append('assertion-identity-ambiguous')
                if assertion and (event.get('binary') != receipt.get('binaries', {}).get('brit', {}).get('path') or not event.get('binary')):
                    event_issues.append('implementation-binary-unbound')
                modes = {'effect': 'exit-status', 'bytes': 'bash-normalized-combined-output', 'exit': 'expected-exit', 'snapshot': 'snapshot-normalized-output'}
                if assertion and (event.get('mode') not in modes or event.get('comparison') != modes.get(event.get('mode'))):
                    event_issues.append('assertion-method-unclassified')
                status = event.get('status', 'unknown')
                if event.get('helper') not in ('expect_parity', 'expect_parity_reset', 'compat_effect', 'expect_run', 'only_for_hash', 'shortcoming'):
                    event_issues.append('assertion-helper-unclassified')
                if event.get('schemaVersion') != 1 or event.get('event') not in ('assertion-end', 'skip', 'deferred'):
                    event_issues.append('event-schema-unclassified')
                if status == 'pass' and not assertion:
                    event_issues.append('non-assertion-pass-invalid')
                if status == 'pass':
                    expected = event.get('gitExit') if event.get('mode') in ('effect', 'bytes') else event.get('expectedExit')
                    if not isinstance(expected, int) or not isinstance(event.get('actualExit'), int) or expected != event['actualExit']:
                        event_issues.append('pass-exit-evidence-inconsistent')
                if status not in ('pass', 'fail', 'skipped', 'deferred', 'invalid-fixture'):
                    event_issues.append('assertion-status-unclassified')
                if event.get('helper') == 'compat_effect' and status == 'pass':
                    status = 'deferred'
                record = {'run': run_id, 'hashKind': run.get('hashKind'), 'event': event,
                          'strength': strength(event), 'status': status, 'underlyingStatus': event.get('status'),
                          'freshnessIssues': event_issues,
                          'negativeInput': (event.get('gitExit') != 0 if isinstance(event.get('gitExit'), int) else None),
                          'rowId': candidates[0]['id'] if len(candidates) == 1 else None,
                          'candidateRowIds': [row['id'] for row in candidates]}
                argv = event.get('argv', [])
                command_token = argv[0] if isinstance(argv, list) and argv else None
                if event.get('helper') == 'expect_run' and command_token == event.get('binary'):
                    command_token = argv[1] if len(argv) > 1 else None
                record['commandAssociation'] = ('exact-command-token' if len(candidates) == 1 and command_token == candidates[0]['commandCandidate'] and not candidates[0]['infrastructure'] else 'unresolved-or-mismatched')
                if len(candidates) != 1:
                    record['mappingIssue'] = 'ambiguous' if candidates else 'unmatched'
                    unmatched.append(record)
                else:
                    observations.append(record)
            for unfinished in run.get('incompleteAssertions', []):
                issues.append({'run': run_id, 'kind': 'incomplete-assertion', 'assertion': unfinished})
    commands = []
    for reference in census['commands']:
        command_rows = []
        for row in rows:
            if row['infrastructure'] or row['commandCandidate'] != reference['name']:
                continue
            evidence = [o for o in observations if o['rowId'] == row['id']]
            associations = [{'option': option, 'sharedGroupIds': sorted(t['id'] for t in reference.get('terms', []) if option in t.get('option_names', [])),
                             'relationship': 'candidate-only'} for option in row['optionCandidates']]
            lanes = {}
            for lane in ('sha1', 'sha256'):
                lane_events = [e for e in evidence if e['hashKind'] == lane]
                statuses = Counter(e['status'] for e in lane_events)
                lanes[lane] = {'observations': dict(statuses), 'execution': 'observed' if lane_events else 'unexecuted',
                               'rowVerdict': 'not-inferred-from-individual-assertions'}
            command_rows.append({**row, 'associations': associations, 'lanes': lanes, 'observations': evidence})
        command_evidence = [o for row in command_rows for o in row['observations']]
        counts = Counter((o['strength'], o['status']) for o in command_evidence if not o['freshnessIssues'] and o['commandAssociation'] == 'exact-command-token')
        commands.append({'id': reference['id'], 'name': reference['name'], 'sourceRows': command_rows,
                         'assertions': [{'strength': s, 'status': v, 'count': n} for (s, v), n in sorted(counts.items())],
                         'behavioralCompleteness': 'unmeasured'})
    total = len(commands)
    strength_names = sorted({o['strength'] for o in observations})
    ratios = []
    for label in strength_names:
        supported = sum(any(o['strength'] == label and o['status'] == 'pass' and not o['freshnessIssues'] and o['commandAssociation'] == 'exact-command-token'
                            for row in command['sourceRows'] for o in row['observations']) for command in commands)
        ratios.append({'strength': label, 'commandsWithFreshPassingAssertion': supported,
                       'commandDenominator': total, 'ratio': supported / total if total else None})
    outcome_counts = Counter((o['strength'], o['status'], 'stale-or-unverified' if o['freshnessIssues'] else 'fresh') for o in observations)
    unexecuted = sum(not any(o['rowId'] == row['id'] and o['hashKind'] == lane and not o['freshnessIssues'] for o in observations) for row in rows for lane in ('sha1', 'sha256'))
    return {'schemaVersion': 1, 'evidenceKind': 'parity-assertion-reconciliation', 'reference': census.get('source', {}),
            'reconciliationMethod': {'path': 'etc/parity/reconcile.py', 'sha256': digest(Path(__file__))},
            'censusSha256': hashlib.sha256(json.dumps(census, sort_keys=True, separators=(',', ':')).encode()).hexdigest(),
            'digestEncoding': 'sha256 of canonical JSON (sorted keys, compact separators) for census and receipts; raw bytes for method files',
            'counts': {'commandDenominator': total, 'commandsWithSourceRows': sum(bool(c['sourceRows']) for c in commands),
                       'sourceRows': len(rows), 'sourceLanes': 2 * len(rows), 'unexecutedSourceLanes': unexecuted,
                       'staleObservations': sum(bool(o['freshnessIssues']) for o in observations), 'matchedObservations': len(observations), 'unmatchedObservations': len(unmatched)},
            'assertionOutcomes': [{'strength': s, 'status': v, 'freshness': f, 'count': n} for (s, v, f), n in sorted(outcome_counts.items())],
            'receiptProvenance': [{'sha256': hashlib.sha256(json.dumps(r, sort_keys=True, separators=(',', ':')).encode()).hexdigest(), 'method': r.get('method'), 'binaries': r.get('binaries')} for r in receipts],
            'ratiosByAssertionStrength': ratios, 'commands': commands, 'runs': runs, 'unmatchedObservations': unmatched,
            'unmappedSourceRows': [row for row in rows if row['commandCandidate'] not in names], 'issues': issues,
            'limits': ['Source parsing recognizes explicit literal title/it/helper lines only; no Bash evaluation.',
                       'Option spellings are candidate associations, never complete option or state-equivalence proof.',
                       'Passing individual assertions do not establish complete rows, lanes, suites or commands.',
                       'Ratios count commands with at least one fresh passing assertion of the named strength; they are not parity percentages.',
                       'Deferred, failed, skipped, stale and unexecuted observations remain separate.',
                       'unexecutedSourceLanes counts source rows/hash lanes without a fresh bound observation; stale or unverified events do not reduce it. Observed skips remain separately classified, not passing assertions.']}


def markdown(result):
    lines = ['# Parity assertion reconciliation', '', f"Command denominator: {result['counts']['commandDenominator']}. No command is certified complete.", '',
             '| Strength | Commands with fresh passing assertion | Denominator |', '|---|---:|---:|']
    for ratio in result['ratiosByAssertionStrength']:
        lines.append(f"| {ratio['strength']} | {ratio['commandsWithFreshPassingAssertion']} | {ratio['commandDenominator']} |")
    lines.extend(['', '| Command | Source rows | Fresh assertion outcomes |', '|---|---:|---|'])
    for command in result['commands']:
        summary = '; '.join(f"{r['strength']} {r['status']}={r['count']}" for r in command['assertions']) or 'unmeasured'
        lines.append(f"| {command['name']} | {len(command['sourceRows'])} | {summary} |")
    lines.extend(['', *result['limits'], ''])
    return '\n'.join(lines)


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--root', type=Path, default=ROOT)
    parser.add_argument('--census', type=Path)
    parser.add_argument('--receipt', type=Path, action='append', default=[])
    parser.add_argument('--markdown', action='store_true')
    args = parser.parse_args(argv)
    try:
        data = json.loads((args.census or args.root / 'docs/parity/git-census.json').read_text())
        result = reconcile(data, [json.loads(path.read_text()) for path in args.receipt], args.root)
        print(markdown(result) if args.markdown else json.dumps(result, indent=2))
        return 0
    except (OSError, ValueError, KeyError, TypeError) as error:
        print('reconciliation refused: ' + str(error), file=sys.stderr)
        return 2


if __name__ == '__main__':
    raise SystemExit(main())
