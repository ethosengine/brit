"""Readiness must survive incomplete, stale and misleadingly green CI evidence."""
import copy
import json
from pathlib import Path
import re
import unittest

import qualify_readiness as readiness


class ReadinessTests(unittest.TestCase):
    def setUp(self):
        self.contract = json.loads(readiness.DEFAULT_CONTRACT.read_text())
        self.sha = 'a' * 40
        self.run = {'id': 42, 'run_attempt': 2, 'head_sha': self.sha,
                    'repository': {'full_name': 'ethosengine/brit'},
                    'path': '.github/workflows/ci.yml', 'status': 'completed'}
        self.jobs = {'jobs': [
            {'id': n, 'name': leg['name'], 'run_id': 42, 'run_attempt': 2,
             'head_sha': self.sha, 'status': 'completed', 'conclusion': 'success',
             'steps': [{'name': s, 'status': 'completed', 'conclusion': 'success'} for s in leg['steps']]}
            for n, leg in enumerate(self.contract['milestones']['inherited-ci']['required'], 1)]}
        self.jobs['total_count'] = len(self.jobs['jobs'])

    def result(self, milestone='inherited-ci'):
        return readiness.qualify(self.contract, milestone, self.sha, self.run, self.jobs)

    def test_complete_exact_evidence_and_paginated_input(self):
        self.assertTrue(self.result()['qualified'])
        rows = self.jobs['jobs']
        self.jobs = [{'total_count': len(rows), 'jobs': rows[:4]},
                     {'total_count': len(rows), 'jobs': rows[4:]}]
        self.assertTrue(self.result()['qualified'])

    def test_required_jobs_reject_every_non_success_state(self):
        for state in ('failure', 'cancelled', 'skipped', 'neutral', None, 'timed_out'):
            with self.subTest(state=state):
                self.jobs['jobs'][0]['conclusion'] = state
                self.assertFalse(self.result()['qualified'])

    def test_required_leg_missing_even_with_green_aggregate(self):
        self.jobs['jobs'][0]['name'] = 'Tests pass'
        self.assertFalse(self.result()['qualified'])

    def test_wrong_run_attempt_revision_repository_and_workflow(self):
        for field, value in [('id', 43), ('run_attempt', 1), ('head_sha', 'b'*40),
                             ('repository', {'full_name': 'other/repo'}), ('path', 'other.yml'),
                             ('status', 'in_progress')]:
            with self.subTest(field=field):
                previous = self.run[field]
                self.run[field] = value
                self.assertFalse(self.result()['qualified'])
                self.run[field] = previous

    def test_job_from_other_attempt_or_revision_cannot_be_mixed(self):
        for field, value in [('run_id', 41), ('run_attempt', 1), ('head_sha', 'b'*40)]:
            previous = self.jobs['jobs'][0][field]
            self.jobs['jobs'][0][field] = value
            self.assertFalse(self.result()['qualified'])
            self.jobs['jobs'][0][field] = previous

    def test_skipped_journey_step_is_not_proven_by_job_success(self):
        journey = next(j for j in self.jobs['jobs'] if j['name'] == 'test-journey')
        journey['steps'][0]['conclusion'] = 'skipped'
        self.assertFalse(self.result()['qualified'])

    def test_truncated_and_duplicate_pages_refused(self):
        with self.assertRaises(ValueError):
            readiness.job_rows({'total_count': 999, 'jobs': self.jobs['jobs']})
        duplicate = copy.deepcopy(self.jobs['jobs'][0])
        self.jobs['jobs'].append(duplicate)
        self.jobs['total_count'] += 1
        with self.assertRaises(ValueError):
            self.result()

    def test_unwired_markers_cannot_qualify_from_ci(self):
        for marker in ('native-daily-driver', 'git-compatibility'):
            self.assertFalse(self.result(marker)['qualified'])

    def test_contract_tracks_blocking_jobs_and_real_step_selectors(self):
        workflow = (Path(__file__).resolve().parents[2] / '.github/workflows/ci.yml').read_text()
        blocks = dict(re.findall(r'^  ([a-z][\w-]*):\n(.*?)(?=^  [a-z][\w-]*:|\Z)', workflow, re.M | re.S))
        needs_block = blocks['tests-pass'].split('    needs:\n', 1)[1].split('\n    if:', 1)[0]
        blocking = set(re.findall(r'^      - ([\w-]+)$', needs_block, re.M))
        required = self.contract['milestones']['inherited-ci']['required']
        self.assertEqual(blocking, {leg['job'] for leg in required})
        for leg in required:
            name_line = re.search(r'^    name: (.+)$', blocks[leg['job']], re.M)
            display = name_line.group(1) if name_line else leg['job']
            if '${{ matrix.os }}' in display:
                axis = leg['name'].rsplit('(', 1)[1].rstrip(')')
                display = display.replace('${{ matrix.os }}', axis)
            self.assertEqual(display, leg['name'])
            for step in leg['steps']:
                self.assertIn('- name: ' + step, blocks[leg['job']])
        self.assertIn("contains(needs.*.result, 'skipped')", blocks['tests-pass'])
        for job in ('msrv', 'test-fast'):
            axes = re.findall(r'^          - ([\w-]+)$', blocks[job], re.M)
            axes += re.findall(r'^          - os: ([\w-]+)$', blocks[job], re.M)
            for axis in axes:
                self.assertTrue(any(leg['job'] == job and leg['name'].endswith(f'({axis})') for leg in required))


if __name__ == '__main__':
    unittest.main()
