#!/usr/bin/env python3
"""Record reconciled parity magnitudes through native EPR observations (Middot).

No acceptance thresholds: these are source-bound claims, not policy decisions.
Without --record, validate and show the measurements without writing native state.
"""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess
import sys

from reconcile import reconcile

ROOT = Path(__file__).resolve().parents[2]
SCALARS = {
    'sourceRows': 'source-rows', 'sourceLanes': 'source-lanes',
    'unexecutedSourceLanes': 'unexecuted-source-lanes',
    'matchedObservations': 'matched-observations',
    'unmatchedObservations': 'unmatched-observations',
    'staleObservations': 'stale-observations',
}


def sha(data):
    return hashlib.sha256(data).hexdigest()


def count(value):
    if type(value) is not int or value < 0:
        raise ValueError('measurement counts must be nonnegative integers')
    return value


def measurements(report):
    if report.get('schemaVersion') != 1 or report.get('evidenceKind') != 'parity-assertion-reconciliation':
        raise ValueError('expected version 1 parity assertion reconciliation')
    counts = report['counts']
    result = [{'measure': 'brit-parity-' + suffix + '@1', 'value': count(counts[key]), 'env': {}}
              for key, suffix in SCALARS.items()]
    outcomes = report['assertionOutcomes']
    seen = set()
    total = 0
    for row in outcomes:
        keys = ('strength', 'status', 'freshness')
        if any(not isinstance(row.get(k), str) or not row[k].strip() for k in keys):
            raise ValueError('outcome requires strength, status and freshness')
        identity = tuple(row[k] for k in keys)
        if identity in seen:
            raise ValueError('duplicate outcome dimension')
        seen.add(identity)
        value = count(row['count'])
        total += value
        result.append({'measure': 'brit-parity-assertion-outcomes@1', 'value': value,
                       'env': {k: row[k] for k in keys}})
    if total != counts['matchedObservations']:
        raise ValueError('outcome dimensions do not partition matched observations')
    if counts['sourceLanes'] != 2 * counts['sourceRows'] or counts['unexecutedSourceLanes'] > counts['sourceLanes']:
        raise ValueError('invalid source-lane denominator')
    if counts['staleObservations'] > counts['matchedObservations']:
        raise ValueError('stale observations exceed matched observations')
    return result


def validate_current(report, census, receipts, root):
    current = reconcile(census, receipts, root)
    if current != report:
        raise ValueError('report differs from current receipt/source reconciliation; regenerate with these receipts')


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--root', type=Path, default=ROOT)
    parser.add_argument('--report', type=Path, required=True)
    parser.add_argument('--epr', default='epr')
    parser.add_argument('--receipt', type=Path, action='append', default=[], help='Original runtime receipts; required to reproduce report provenance')
    parser.add_argument('--census', type=Path)
    parser.add_argument('--record', action='store_true')
    parser.add_argument('--actor', help='Native actor attribution; required with --record')
    args = parser.parse_args(argv)
    try:
        if args.record and not args.actor:
            raise ValueError('--record requires explicit --actor attribution')
        root = args.root.resolve(strict=True)
        path = args.report.resolve(strict=True)
        if not path.is_relative_to(root):
            raise ValueError('report must be inside the Brit root for native subject resolution')
        raw = path.read_bytes()
        report = json.loads(raw)
        census = json.loads((args.census or root / 'docs/parity/git-census.json').read_text())
        receipts = [json.loads(receipt.read_text()) for receipt in args.receipt]
        validate_current(report, census, receipts, root)
        notes = measurements(report)
        method = report['reconciliationMethod']
        if method['path'] != 'etc/parity/reconcile.py' or method['sha256'] != sha((root / method['path']).read_bytes()):
            raise ValueError('reconciliation method drift; regenerate the report')
        registry = root / '.epr-meta/measures.yaml'
        environment = {
            'reconciliation-method-sha256': method['sha256'],
            'observation-adapter-sha256': sha(Path(__file__).read_bytes()),
            'registry-sha256': sha(registry.read_bytes()),
            'receipt-set-sha256': sha(json.dumps(report['receiptProvenance'], sort_keys=True, separators=(',', ':')).encode()),
            'reference-revision': report['reference']['revision'],
            'report-sha256': sha(raw), 'evidence-standing': 'claimed',
        }
        if args.record:
            for note in notes:
                command = [args.epr, 'flow', 'note', '--root', str(root), '--kind', 'observation',
                           '--measures', '.epr-meta/measures.yaml', '--measure', note['measure'],
                           '--subject', str(path.relative_to(root)), '--value', str(note['value']),
                           '--as', args.actor]
                for key, value in sorted({**environment, **note['env']}.items()):
                    command += ['--env', key + '=' + value]
                subprocess.run(command, check=True)
        else:
            print(json.dumps({'evidenceKind': 'observation-preview', 'subject': str(path.relative_to(root)),
                              'env': environment, 'observations': notes}, indent=2))
        return 0
    except (OSError, ValueError, KeyError, TypeError, subprocess.SubprocessError) as error:
        print('native parity observation refused: ' + str(error), file=sys.stderr)
        return 2


if __name__ == '__main__':
    sys.exit(main())
