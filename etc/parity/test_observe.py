import copy
import unittest
from unittest.mock import patch
import observe


class ObservationTests(unittest.TestCase):
    def setUp(self):
        self.report = {'schemaVersion': 1, 'evidenceKind': 'parity-assertion-reconciliation',
                       'counts': {'sourceRows': 2, 'sourceLanes': 4, 'unexecutedSourceLanes': 1,
                                  'matchedObservations': 3, 'unmatchedObservations': 1, 'staleObservations': 1},
                       'assertionOutcomes': [{'strength': 'exit-status', 'status': 'pass', 'freshness': 'fresh', 'count': 2},
                                             {'strength': 'captured-output', 'status': 'fail', 'freshness': 'stale-or-unverified', 'count': 1}]}

    def test_strengths_and_stale_failures_remain_distinct_dimensions(self):
        notes = observe.measurements(self.report)
        vector = [n for n in notes if n['measure'] == 'brit-parity-assertion-outcomes@1']
        self.assertEqual(len(vector), 2)
        self.assertEqual(vector[1]['env']['status'], 'fail')
        self.assertEqual(vector[1]['env']['freshness'], 'stale-or-unverified')
        self.assertEqual(sum(n['value'] for n in vector), 3)

    def test_inconsistent_or_duplicate_outcomes_refuse_before_recording(self):
        self.report['assertionOutcomes'][0]['count'] = 100
        with self.assertRaisesRegex(ValueError, 'partition'):
            observe.measurements(self.report)
        self.report['assertionOutcomes'].append(copy.deepcopy(self.report['assertionOutcomes'][0]))
        with self.assertRaisesRegex(ValueError, 'duplicate'):
            observe.measurements(self.report)

    def test_report_is_reconciled_again_before_recording(self):
        current = copy.deepcopy(self.report)
        current['counts']['staleObservations'] = 2
        with patch.object(observe, 'reconcile', return_value=current):
            with self.assertRaisesRegex(ValueError, 'current receipt/source'):
                observe.validate_current(self.report, {}, [], '.')
        with patch.object(observe, 'reconcile', return_value=self.report) as derive:
            observe.validate_current(self.report, {}, [], '.')
            derive.assert_called_once_with({}, [], '.')

    def test_invalid_denominators_and_booleans_are_not_measurements(self):
        self.report['counts']['unexecutedSourceLanes'] = 5
        with self.assertRaisesRegex(ValueError, 'denominator'):
            observe.measurements(self.report)
        self.report['counts']['unexecutedSourceLanes'] = True
        with self.assertRaisesRegex(ValueError, 'nonnegative integers'):
            observe.measurements(self.report)


if __name__ == '__main__':
    unittest.main()
