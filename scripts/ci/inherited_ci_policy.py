#!/usr/bin/env python3
"""Apply Brit's inherited-CI contract to the job matrix planned by the gitoxide QA scheduler.

The scheduler (`tests/it/src/qa`) decides which inherited checks exist and where they
run. `.epr-meta/readiness.json` decides what each planned job means for Brit: required
for `Tests pass`, deferred as visible debt, replaced by a first-party job, or disabled.
A planned job the contract does not name is refused, so a leg that appears upstream
cannot start or stop gating Brit without a recorded decision.

This reads the planner's `key=value` output lines and writes the same three keys
(`matrix`, `plan`, `downstream`). It runs no checks and fetches nothing.
"""
import argparse
import json
import os
from pathlib import Path
import sys


DEFAULT_CONTRACT = Path(__file__).resolve().parents[2] / '.epr-meta/readiness.json'
MILESTONE = 'inherited-ci'
PLANNED_JOB = 'run'
CONSUMER_JOB = 'downstream'
# Windows limits one environment variable to 32,767 characters; the plan travels in one.
MAX_PLAN_CHARACTERS = 32_000


class PolicyError(ValueError):
    """The planned jobs and the contract disagree; every finding is listed."""


def planned_names(marker):
    """Planned-job names by class. Planner job names always contain '@'."""
    classes = {
        'required': {leg['name'] for leg in marker.get('required', []) if leg['job'] == PLANNED_JOB},
        'deferred': {name for name in marker.get('deferred', {}) if '@' in name},
        'replaced': set(marker.get('replaced', {})),
        'disabled': {name for name in marker.get('disabled', {}) if '@' in name},
    }
    seen = {}
    for kind, names in classes.items():
        for name in sorted(names):
            if '@' not in name:
                raise PolicyError(f'{name}: {kind} planned jobs are named <group>@<platform>')
            if name in seen:
                raise PolicyError(f'{name}: declared both {seen[name]} and {kind}')
            seen[name] = kind
    return classes


def retarget(row, runners):
    """Move a planned job to the platform Brit can run it on."""
    target = runners.get(row['platform'])
    if target is None:
        return row
    if row.get('container') or row.get('host') or row.get('freebsd_release'):
        raise PolicyError(f"{row['id']}: only native runner jobs can change platform")
    group = row['id'].rsplit('@', 1)[0]
    moved = dict(row, id=f"{group}@{target['platform']}", platform=target['platform'], runner=target['runner'])
    if 'cache' in moved:
        moved['cache'] = f"ci-{group}-{target['platform']}"
    return moved


def apply(marker, rows, mode):
    """Return the jobs to run and a report of what happened to every planned job."""
    if mode not in ('fast', 'exhaustive'):
        raise PolicyError(f'unknown planner mode {mode!r}')
    classes = planned_names(marker)
    runners = marker.get('runners', {})
    issues, kept, seen = [], [], set()
    report = {kind: [] for kind in classes}
    for row in rows:
        row = retarget(row, runners)
        name = row['id']
        if name in seen:
            issues.append(f'{name}: planned more than once after platform substitution')
            continue
        seen.add(name)
        kind = next((kind for kind, names in classes.items() if name in names), None)
        if kind is None:
            issues.append(f'{name}: not declared required, deferred, replaced or disabled')
            continue
        report[kind].append(name)
        if kind in ('required', 'deferred'):
            kept.append(dict(row, blocking=kind == 'required'))
    if mode == 'exhaustive':
        # Every package and stage is selected, so a declared job that is absent no longer exists.
        for kind, names in classes.items():
            for name in sorted(names - seen):
                issues.append(f'{name}: declared {kind}, but the exhaustive plan has no such job')
    if not kept and not issues:
        issues.append('no planned job remains to run')
    if issues:
        raise PolicyError('\n'.join(issues))
    return kept, report


def negative_numbers(value):
    if isinstance(value, dict):
        return any(negative_numbers(item) for item in value.values())
    if isinstance(value, list):
        return any(negative_numbers(item) for item in value)
    return isinstance(value, (int, float)) and not isinstance(value, bool) and value < 0


def encode(value):
    """Serialize JSON on one line without a literal hyphen.

    GitHub withholds a job output that contains the value of any secret. This repository
    has a secret whose mask is a lone hyphen (its job logs show no hyphen at all), so an
    output with one never reaches the next job: CI run 44517078199 planned 32 jobs and
    started none. Inside a JSON string the escape \\u002d is the same character to every
    parser, so `fromJSON` and the scheduler read the original names.
    """
    if negative_numbers(value):
        raise PolicyError('a negative number cannot be written without a hyphen')
    return json.dumps(value, separators=(',', ':')).replace('-', '\\u002d')


def parse_outputs(text):
    """Read `key=value` lines as the planner writes them for GitHub Actions."""
    outputs = {}
    for line in text.splitlines():
        key, separator, value = line.partition('=')
        if not separator or key in outputs:
            raise PolicyError(f'malformed or repeated planner output line: {line[:60]!r}')
        outputs[key] = value
    missing = {'matrix', 'plan', 'downstream'} - set(outputs)
    if missing:
        raise PolicyError('planner output is missing ' + ', '.join(sorted(missing)))
    return outputs


def transform(contract, outputs):
    marker = contract['milestones'][MILESTONE]
    plan = json.loads(outputs['plan'])
    matrix = json.loads(outputs['matrix'])
    if matrix.get('include') != plan.get('jobs'):
        raise PolicyError('the planner matrix and its execution plan name different jobs')
    rows, report = apply(marker, plan['jobs'], plan['selection']['mode'])
    plan['jobs'] = rows
    consumers = outputs['downstream']
    if CONSUMER_JOB in marker.get('disabled', {}):
        consumers = ''
    elif consumers:
        consumers = encode(json.loads(consumers))
    governed = {'matrix': encode({'include': rows}), 'plan': encode(plan), 'downstream': consumers}
    if len(governed['plan']) > MAX_PLAN_CHARACTERS:
        raise PolicyError(f"the execution plan needs {len(governed['plan'])} characters; "
                          f'jobs receive it in one environment variable of at most {MAX_PLAN_CHARACTERS}')
    return governed, report


def summary(report, marker):
    lines = ['### Inherited QA jobs under Brit\'s contract', '', '| Planned job | Decision |', '| --- | --- |']
    notes = {'required': lambda name: 'required for `Tests pass`',
             'deferred': lambda name: 'deferred: ' + marker['deferred'][name],
             'replaced': lambda name: 'replaced by `' + marker['replaced'][name] + '`',
             'disabled': lambda name: 'disabled: ' + marker['disabled'][name]}
    for kind in ('required', 'deferred', 'replaced', 'disabled'):
        for name in report[kind]:
            lines.append(f'| `{name}` | {notes[kind](name)} |')
    return '\n'.join(lines) + '\n'


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--contract', type=Path, default=DEFAULT_CONTRACT)
    parser.add_argument('--planned', type=Path, required=True, help='file the planner wrote its outputs to')
    parser.add_argument('--github-output', type=Path, required=True, help='file to append the governed outputs to')
    args = parser.parse_args()
    try:
        contract = json.loads(args.contract.read_text())
        outputs, report = transform(contract, parse_outputs(args.planned.read_text()))
    except PolicyError as error:
        print('Brit\'s inherited-CI contract (.epr-meta/readiness.json) does not cover this plan:', file=sys.stderr)
        for issue in str(error).splitlines():
            print('  ' + issue, file=sys.stderr)
        return 1
    except (OSError, ValueError, KeyError, TypeError) as error:
        print(f'cannot apply the inherited-CI contract: {error!r}', file=sys.stderr)
        return 2
    with args.github_output.open('a') as output:
        for key in ('matrix', 'plan', 'downstream'):
            output.write(f'{key}={outputs[key]}\n')
    text = summary(report, contract['milestones'][MILESTONE])
    print(text)
    step_summary = os.environ.get('GITHUB_STEP_SUMMARY')
    if step_summary:
        with open(step_summary, 'a') as output:
            output.write(text)
    return 0


if __name__ == '__main__':
    sys.exit(main())
