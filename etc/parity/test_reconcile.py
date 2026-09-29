from copy import deepcopy
import json
from pathlib import Path
import tempfile
import unittest
import reconcile


class ReconciliationTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.suite = 'tests/journey/parity/add.sh'
        self.write(self.suite, 'title "add dry run"\nonly_for_hash dual && (sandbox\n  it "works" && {\n    expect_parity effect -- add --dry-run file\n    expect_parity bytes -- add --bogus\n  }\n)\ntitle "later"\nit "tail" && {\n  compat_effect "not implemented" -- add --all\n}\n')
        self.methods = {}
        for name in ('tests/parity.sh', 'tests/helpers.sh', 'tests/utilities.sh', 'etc/parity/runtime.sh', 'etc/parity/event.py', 'etc/parity/run.py'):
            self.write(name, '# fixture\n')
            self.methods[name] = reconcile.digest(self.root / name)
        self.reference = {'schemaVersion': 1, 'source': {'revision': 'a'*40}, 'commands': [
            {'id': 'git:command:add', 'name': 'add', 'terms': [{'id': 'dry', 'option_names': ['-n', '--dry-run']}]},
            {'id': 'git:command:missing', 'name': 'missing', 'terms': []}]}
        self.binary = {'path': '/measured/brit', 'sha256': 'a'*64, 'sha256After': 'a'*64, 'version': 'fixture'}

    def write(self, name, value):
        path = self.root / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(value)

    def event(self, **values):
        row = {'schemaVersion': 1, 'event': 'assertion-end', 'suite': self.suite,
               'suiteSha256': reconcile.digest(self.root / self.suite), 'hashKind': 'sha1',
               'line': 4, 'title': 'add dry run', 'it': 'works', 'helper': 'expect_parity',
               'mode': 'effect', 'comparison': 'exit-status', 'status': 'pass',
               'gitExit': 0, 'actualExit': 0, 'argv': ['add', '--dry-run'], 'binary': '/measured/brit', 'invocationId': '1'}
        return {**row, **values}

    def receipt(self, events=None):
        events = events if events is not None else [self.event()]
        witnessed = []
        for event in events:
            if event['event'] == 'assertion-end':
                witnessed.append({**event, 'event': 'assertion-start', 'status': 'started'})
            witnessed.append(event)
        return {'schemaVersion': 1, 'evidenceKind': 'runtime-parity-receipt', 'stableInputs': True, 'method': self.methods,
                'inputs': reconcile.runtime_runner.input_inventory(self.root),
                'binaries': {'brit': self.binary, 'git': {**self.binary, 'path': '/measured/git'},
                             'ein': {**self.binary, 'path': '/measured/ein'}, 'jtt': {**self.binary, 'path': '/measured/jtt'}},
                'runs': [{'suite': self.suite, 'suiteSha256': reconcile.digest(self.root / self.suite),
                          'hashKind': 'sha1', 'status': 'completed', 'exitCode': 0,
                          'events': witnessed, 'incompleteAssertions': []}]}

    def test_denominator_candidates_and_unexecuted_lanes(self):
        result = reconcile.reconcile(self.reference, [self.receipt()], self.root)
        self.assertEqual(result['counts']['commandDenominator'], 2)
        self.assertEqual(result['counts']['sourceLanes'], 4)
        self.assertEqual(result['counts']['unexecutedSourceLanes'], 3)
        row = result['commands'][0]['sourceRows'][0]
        self.assertEqual(next(a for a in row['associations'] if a['option'] == '--dry-run')['sharedGroupIds'], ['dry'])
        self.assertEqual(row['lanes']['sha1']['rowVerdict'], 'not-inferred-from-individual-assertions')
        self.assertEqual(result['ratiosByAssertionStrength'][0]['commandsWithFreshPassingAssertion'], 1)

    def test_failures_deferred_and_negative_input_remain_separate(self):
        events = [self.event(), self.event(line=5, mode='bytes', comparison='bash-normalized-combined-output',
                    status='fail', gitExit=129, actualExit=0, invocationId='2'),
                  self.event(line=10, title='later', it='tail', compatReason='unimplemented', helper='compat_effect', invocationId='3')]
        result = reconcile.reconcile(self.reference, [self.receipt(events)], self.root)
        outcomes = {(o['strength'], o['status']): o['count'] for o in result['assertionOutcomes']}
        self.assertEqual(outcomes[('deferred-exit-comparison', 'deferred')], 1)
        self.assertEqual(outcomes[('normalized-output-comparison', 'fail')], 1)
        self.assertTrue(result['commands'][0]['sourceRows'][0]['observations'][1]['negativeInput'])
        ratio = next(r for r in result['ratiosByAssertionStrength'] if r['strength'] == 'deferred-exit-comparison')
        self.assertEqual(ratio['commandsWithFreshPassingAssertion'], 0)

    def test_source_method_binary_drift_never_qualifies(self):
        for alter in ('source', 'method', 'binary', 'missing-provenance', 'wrong-executable'):
            with self.subTest(alter=alter):
                receipt = deepcopy(self.receipt())
                if alter == 'source': receipt['runs'][0]['suiteSha256'] = 'old'
                if alter == 'method': receipt['method']['tests/helpers.sh'] = 'old'
                if alter == 'binary': receipt['binaries']['brit']['sha256After'] = 'old'
                if alter == 'missing-provenance': receipt['method'] = {}
                if alter == 'wrong-executable': receipt['runs'][0]['events'][-1]['binary'] = '/usr/bin/git'
                result = reconcile.reconcile(self.reference, [receipt], self.root)
                self.assertEqual(result['ratiosByAssertionStrength'][0]['commandsWithFreshPassingAssertion'], 0)
                self.assertEqual(result['counts']['staleObservations'], 1)

    def test_unmatched_dynamic_and_incomplete_preserved(self):
        self.write('tests/journey/parity/dynamic.sh', 'title "$title"\nit "$case" && {\nexpect_parity effect -- dynamic "$args"\n}\n')
        receipt = self.receipt([self.event(line=999)])
        receipt['runs'][0].update(status='timeout', incompleteAssertions=[{'invocationId': 'stopped'}])
        result = reconcile.reconcile(self.reference, [receipt], self.root)
        self.assertEqual(result['counts']['unmatchedObservations'], 1)
        self.assertEqual(result['issues'][0]['kind'], 'incomplete-assertion')
        self.assertEqual(result['unmappedSourceRows'][0]['sourceMapping'], 'unresolved')
        self.assertEqual(result['unmappedSourceRows'][0]['optionCandidates'], [])

    def test_unknown_metadata_and_schema_do_not_become_evidence(self):
        receipt = self.receipt([self.event(mode='mystery', comparison='mystery')])
        result = reconcile.reconcile(self.reference, [receipt], self.root)
        self.assertEqual(result['ratiosByAssertionStrength'][0]['commandsWithFreshPassingAssertion'], 0)
        with self.assertRaises(ValueError):
            reconcile.reconcile(self.reference, [{'schemaVersion': 999}], self.root)
        self.assertIn('not parity percentages', reconcile.markdown(result))

    def test_fixture_addition_invalid_events_and_duplicate_receipts(self):
        receipt = self.receipt()
        self.write('tests/fixtures/new-file', 'changed')
        result = reconcile.reconcile(self.reference, [receipt], self.root)
        self.assertEqual(result['counts']['unexecutedSourceLanes'], 4)
        self.assertIn('fixture-input-drift-or-unverified', result['commands'][0]['sourceRows'][0]['observations'][0]['freshnessIssues'])
        receipt = self.receipt()
        receipt['runs'][0]['status'] = 'invalid-events'
        result = reconcile.reconcile(self.reference, [receipt], self.root)
        self.assertEqual(result['ratiosByAssertionStrength'][0]['commandsWithFreshPassingAssertion'], 0)
        with self.assertRaises(ValueError):
            reconcile.reconcile(self.reference, [receipt, receipt], self.root)

    def test_skip_and_unbound_end_cannot_pass(self):
        event = self.event(event='skip', helper='only_for_hash', line=2, it='',
                           status='skipped', mode='', comparison=None, binary=None)
        result = reconcile.reconcile(self.reference, [self.receipt([event])], self.root)
        self.assertEqual(result['assertionOutcomes'][0]['status'], 'skipped')
        self.assertEqual(result['assertionOutcomes'][0]['freshness'], 'fresh')
        receipt = self.receipt()
        receipt['runs'][0]['events'] = receipt['runs'][0]['events'][1:]
        result = reconcile.reconcile(self.reference, [receipt], self.root)
        self.assertEqual(result['ratiosByAssertionStrength'][0]['commandsWithFreshPassingAssertion'], 0)

    def test_suite_filename_never_establishes_runtime_command(self):
        result = reconcile.reconcile(self.reference, [self.receipt([self.event(argv=['status'])])], self.root)
        self.assertEqual(result['ratiosByAssertionStrength'][0]['commandsWithFreshPassingAssertion'], 0)
        self.assertEqual(result['commands'][0]['sourceRows'][0]['observations'][0]['commandAssociation'], 'unresolved-or-mismatched')

    def test_title_only_shortcoming_and_multi_it_skip_remain_distinct(self):
        self.write(self.suite, 'title "held"\nonly_for_hash sha1-only && (sandbox\n shortcoming "later"\n)\ntitle "two cases"\nonly_for_hash sha1-only && (sandbox\nit "first" && true\nit "second" && true\n)\n')
        events = [self.event(event='skip', helper='only_for_hash', line=2, title='held', it='', status='skipped', mode='', comparison=None),
                  self.event(event='skip', helper='only_for_hash', line=6, title='two cases', it='', status='skipped', mode='', comparison=None)]
        result = reconcile.reconcile(self.reference, [self.receipt(events)], self.root)
        self.assertEqual(result['counts']['sourceRows'], 3)
        self.assertEqual(result['counts']['matchedObservations'], 1)
        self.assertEqual(result['unmatchedObservations'][0]['mappingIssue'], 'ambiguous')
        self.assertEqual(len(result['unmatchedObservations'][0]['candidateRowIds']), 2)
        self.assertEqual(result['commands'][0]['sourceRows'][0]['rowKind'], 'title-only')

    def test_real_source_denominator_without_receipts(self):
        path = reconcile.ROOT / 'docs/parity/git-census.json'
        result = reconcile.reconcile(json.loads(path.read_text()), [], reconcile.ROOT)
        self.assertEqual(result['counts']['commandDenominator'], 159)
        self.assertGreater(result['counts']['sourceRows'], 500)
        self.assertEqual(result['counts']['sourceLanes'], result['counts']['unexecutedSourceLanes'])
        self.assertFalse(result['ratiosByAssertionStrength'])


if __name__ == '__main__':
    unittest.main()
