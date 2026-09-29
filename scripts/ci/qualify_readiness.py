#!/usr/bin/env python3
"""Read-only qualification of captured GitHub Actions evidence, never an attestation.

Capture the run and ALL job pages from the same run attempt. This checker does not
fetch, authenticate, publish, or promote evidence. Governance acceptance remains
with epr; input provenance must be established by the caller.
"""
import argparse
import json
from pathlib import Path
import re
import sys


DEFAULT_CONTRACT = Path(__file__).resolve().parents[2] / '.epr-meta/readiness.json'


def job_rows(payload):
    pages = payload if isinstance(payload, list) else [payload]
    if not pages or any(not isinstance(p, dict) or not isinstance(p.get('jobs'), list) for p in pages):
        raise ValueError('expected Actions jobs response or --paginate --slurp response pages')
    rows = [row for page in pages for row in page['jobs']]
    counts = {p.get('total_count') for p in pages}
    if counts != {len(rows)}:
        raise ValueError('job pages are incomplete or disagree on total_count')
    ids = [row.get('id') for row in rows]
    if any(type(i) is not int or i <= 0 for i in ids) or len(set(ids)) != len(ids):
        raise ValueError('job evidence has missing or duplicate job identities')
    return rows


def qualify(contract, milestone, sha, run, payload):
    if contract.get('version') != 1:
        raise ValueError('unsupported readiness contract version')
    marker = contract['milestones'][milestone]
    result = {'milestone': milestone, 'head_sha': sha, 'qualified': False,
              'evidence_kind': 'captured-actions-evidence-not-attestation',
              'issues': [], 'jobs': [], 'deferred': marker.get('deferred', [])}
    issues = result['issues']
    if marker.get('status') != 'wired':
        issues.append('unwired: ' + marker.get('reason', 'no required evidence contract'))
        return result
    required = marker.get('required', [])
    if not required or len({leg['name'] for leg in required}) != len(required):
        raise ValueError('wired milestone needs nonempty, uniquely named required legs')
    if not re.fullmatch(r'[0-9a-f]{40}|[0-9a-f]{64}', sha):
        raise ValueError('expected full lowercase source object ID')
    if run.get('repository', {}).get('full_name') != contract['repository']:
        issues.append('repository does not match governed boundary')
    if run.get('path') != contract['workflow']:
        issues.append('workflow does not match governed CI source')
    if run.get('head_sha') != sha or run.get('status') != 'completed':
        issues.append('run is incomplete or names a different source revision')
    for field in ('id', 'run_attempt'):
        if type(run.get(field)) is not int or run[field] <= 0:
            issues.append('run is missing valid ' + field)
    rows = job_rows(payload)
    for leg in required:
        matches = [row for row in rows if row.get('name') == leg['name']]
        if len(matches) != 1:
            issues.append(f"{leg['name']}: expected exactly one job, found {len(matches)}")
            continue
        row = matches[0]
        result['jobs'].append({'name': row['name'], 'id': row['id'],
                               'url': row.get('html_url'), 'conclusion': row.get('conclusion')})
        if (row.get('run_id'), row.get('run_attempt'), row.get('head_sha')) != (run.get('id'), run.get('run_attempt'), sha):
            issues.append(f"{leg['name']}: different run, attempt or source revision")
        if row.get('status') != 'completed' or row.get('conclusion') != 'success':
            issues.append(f"{leg['name']}: required job did not complete successfully")
        for selector in leg.get('steps', []):
            steps = [s for s in row.get('steps', []) if s.get('name') == selector]
            if len(steps) != 1 or steps[0].get('status') != 'completed' or steps[0].get('conclusion') != 'success':
                issues.append(f"{leg['name']}: required step absent or unsuccessful: {selector}")
    result['qualified'] = not issues
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--contract', type=Path, default=DEFAULT_CONTRACT)
    parser.add_argument('--milestone', required=True)
    parser.add_argument('--sha', required=True)
    parser.add_argument('--run', type=Path, required=True)
    parser.add_argument('--jobs', type=Path, required=True)
    args = parser.parse_args()
    try:
        result = qualify(json.loads(args.contract.read_text()), args.milestone, args.sha,
                         json.loads(args.run.read_text()), json.loads(args.jobs.read_text()))
    except (OSError, ValueError, KeyError, TypeError, AttributeError) as error:
        print(json.dumps({'qualified': False, 'issues': [str(error)]}))
        return 2
    print(json.dumps(result, indent=2))
    return 0 if result['qualified'] else 1


if __name__ == '__main__':
    sys.exit(main())
