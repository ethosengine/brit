"""A planned job gates Brit only by declaration; an undeclared or vanished job is refused."""
import copy
import json
from pathlib import Path
import tempfile
import unittest

import inherited_ci_policy as policy


def row(name, platform=None, **extra):
    group, _, named_platform = name.partition('@')
    platform = platform or named_platform
    base = {'id': f'{group}@{platform}', 'platform': platform, 'runner': 'runner-for-' + platform,
            'container': '', 'host': '', 'runner_architecture': '', 'freebsd_release': '',
            'cache': f'ci-{group}-{platform}', 'blocking': True, 'tasks': [group], 'tools': []}
    base.update(extra)
    return base


class PolicyTests(unittest.TestCase):
    def setUp(self):
        self.contract = json.loads(policy.DEFAULT_CONTRACT.read_text())
        self.marker = self.contract['milestones'][policy.MILESTONE]
        self.classes = policy.planned_names(self.marker)
        moved = {target['platform']: source for source, target in self.marker['runners'].items()}
        # What the scheduler plans before Brit's runner substitution.
        self.rows = []
        for names in self.classes.values():
            for name in sorted(names):
                platform = name.rsplit('@', 1)[1]
                self.rows.append(row(name, moved.get(platform, platform)))

    def apply(self, mode='exhaustive'):
        return policy.apply(self.marker, copy.deepcopy(self.rows), mode)

    def test_declared_jobs_gate_defer_or_leave_the_matrix(self):
        rows, report = self.apply()
        by_name = {entry['id']: entry for entry in rows}
        self.assertEqual({name for name, entry in by_name.items() if entry['blocking']}, self.classes['required'])
        self.assertEqual({name for name, entry in by_name.items() if not entry['blocking']}, self.classes['deferred'])
        self.assertFalse(set(by_name) & (self.classes['replaced'] | self.classes['disabled']))
        self.assertEqual(set(report['replaced']), self.classes['replaced'])

    def test_the_scheduler_cannot_decide_what_gates(self):
        deferred = next(iter(sorted(self.classes['deferred'])))
        required = next(iter(sorted(self.classes['required'])))
        for entry in self.rows:
            # Reverse upstream's own flags for these two jobs.
            if entry['id'] == deferred:
                entry['blocking'] = True
            if entry['id'] == required:
                entry['blocking'] = False
        by_name = {entry['id']: entry for entry in self.apply()[0]}
        self.assertFalse(by_name[deferred]['blocking'])
        self.assertTrue(by_name[required]['blocking'])

    def test_undeclared_job_is_refused_in_both_modes(self):
        self.rows.append(row('new-check@linux-x86_64'))
        for mode in ('fast', 'exhaustive'):
            with self.subTest(mode=mode), self.assertRaisesRegex(policy.PolicyError, 'new-check@linux-x86_64: not declared'):
                self.apply(mode)

    def test_exhaustive_plan_must_contain_every_declared_job(self):
        for kind in ('required', 'deferred', 'replaced'):
            with self.subTest(kind=kind):
                name = sorted(self.classes[kind])[0]
                saved = self.rows
                self.rows = [entry for entry in self.rows if policy.retarget(entry, self.marker['runners'])['id'] != name]
                with self.assertRaisesRegex(policy.PolicyError, f'{name}: declared {kind}'):
                    self.apply('exhaustive')
                # A fast plan legitimately selects fewer jobs.
                self.assertNotIn(name, {entry['id'] for entry in self.apply('fast')[0]})
                self.rows = saved

    def test_platform_substitution_moves_the_job_and_its_identity(self):
        self.assertTrue(self.marker['runners'], 'the contract declares at least one substitution')
        rows, _ = self.apply()
        for source, target in self.marker['runners'].items():
            self.assertFalse([entry for entry in rows if entry['platform'] == source])
            moved = [entry for entry in rows if entry['platform'] == target['platform']]
            self.assertTrue(moved)
            for entry in moved:
                group = entry['id'].split('@')[0]
                self.assertEqual(entry['id'], f"{group}@{target['platform']}")
                self.assertEqual(entry['runner'], target['runner'])
                self.assertEqual(entry['cache'], f"ci-{group}-{target['platform']}")

    def test_substitution_refuses_containers_and_collisions(self):
        source, target = next(iter(self.marker['runners'].items()))
        with self.assertRaisesRegex(policy.PolicyError, 'only native runner jobs'):
            policy.retarget(row('tests@' + source, container='image'), self.marker['runners'])
        self.rows.append(row('tests@' + target['platform']))
        with self.assertRaisesRegex(policy.PolicyError, 'planned more than once'):
            self.apply()

    def test_contradictory_or_malformed_declarations_are_refused(self):
        name = sorted(self.classes['required'])[0]
        self.marker['deferred'][name] = 'also deferred'
        with self.assertRaisesRegex(policy.PolicyError, 'declared both'):
            self.apply()
        del self.marker['deferred'][name]
        self.marker['replaced']['journey'] = 'test-journey'
        with self.assertRaisesRegex(policy.PolicyError, '<group>@<platform>'):
            self.apply()

    def test_an_empty_matrix_and_unknown_modes_are_refused(self):
        self.rows = [entry for entry in self.rows if entry['id'] in self.classes['replaced']]
        with self.assertRaisesRegex(policy.PolicyError, 'no planned job remains'):
            self.apply('fast')
        with self.assertRaisesRegex(policy.PolicyError, 'unknown planner mode'):
            self.apply('other')

    def outputs(self, mode='exhaustive', downstream='{"include":[{"name":"consumer"}]}'):
        plan = {'selection': {'mode': mode, 'packages': ['gix']}, 'jobs': self.rows}
        return {'matrix': json.dumps({'include': self.rows}), 'plan': json.dumps(plan), 'downstream': downstream}

    def test_outputs_split_required_and_deferred_jobs_of_one_execution_plan(self):
        outputs, _ = policy.transform(self.contract, self.outputs())
        matrix, plan = json.loads(outputs['matrix']), json.loads(outputs['plan'])
        deferred = json.loads(outputs['deferred'])
        # Each job must find itself in the plan it executes, and run in exactly one matrix.
        self.assertEqual(matrix['include'] + deferred['include'],
                         [row for row in plan['jobs'] if row['blocking']] + [row for row in plan['jobs'] if not row['blocking']])
        self.assertEqual({row['id'] for row in matrix['include']}, self.classes['required'])
        self.assertEqual({row['id'] for row in deferred['include']}, self.classes['deferred'])
        self.assertTrue(all(row['blocking'] for row in matrix['include']), 'Tests pass waits only for required jobs')
        self.assertFalse(any(row['blocking'] for row in deferred['include']))
        self.assertEqual(plan['selection'], {'mode': 'exhaustive', 'packages': ['gix']})
        self.assertNotIn('\n', outputs['matrix'] + outputs['plan'] + outputs['deferred'])
        self.assertIn('-', json.dumps(matrix), 'the decoded jobs keep their real names')
        self.assertEqual(outputs['downstream'], '', 'disabled consumer checks start no job')
        del self.marker['disabled'][policy.CONSUMER_JOB]
        self.assertTrue(policy.transform(self.contract, self.outputs())[0]['downstream'])
        mismatched = self.outputs()
        mismatched['matrix'] = json.dumps({'include': self.rows[:1]})
        with self.assertRaisesRegex(policy.PolicyError, 'name different jobs'):
            policy.transform(self.contract, mismatched)

    def test_no_deferred_job_leaves_the_deferred_matrix_empty(self):
        # GitHub refuses an empty matrix, so the deferred job is skipped on an empty output.
        self.rows = [entry for entry in self.rows if policy.retarget(entry, self.marker['runners'])['id'] not in self.classes['deferred']]
        outputs, _ = policy.transform(self.contract, self.outputs('fast'))
        self.assertEqual(outputs['deferred'], '')
        self.assertTrue(json.loads(outputs['matrix'])['include'])

    def test_a_plan_with_only_deferred_jobs_is_refused(self):
        self.rows = [entry for entry in self.rows if policy.retarget(entry, self.marker['runners'])['id'] in self.classes['deferred']]
        with self.assertRaisesRegex(policy.PolicyError, 'no required planned job remains'):
            self.apply('fast')

    def test_outputs_survive_a_secret_that_masks_the_hyphen(self):
        outputs, _ = policy.transform(self.contract, self.outputs())
        for key in ('matrix', 'deferred', 'plan'):
            self.assertNotIn('-', outputs[key], 'GitHub withholds an output containing a masked value')
        self.assertEqual(policy.encode({'path': 'a\\-b', 'flag': '--locked'}), '{"path":"a\\\\\\u002db","flag":"\\u002d\\u002dlocked"}')
        self.assertEqual(json.loads(policy.encode({'path': 'a\\-b'})), {'path': 'a\\-b'})
        del self.marker['disabled'][policy.CONSUMER_JOB]
        consumers = policy.transform(self.contract, self.outputs(downstream='{"include":[{"name":"a-b"}]}'))[0]['downstream']
        self.assertNotIn('-', consumers)
        self.assertEqual(json.loads(consumers), {'include': [{'name': 'a-b'}]})
        with self.assertRaisesRegex(policy.PolicyError, 'negative number'):
            policy.encode({'jobs': [{'offset': -1}]})
        self.assertEqual(policy.encode({'ok': True, 'count': 0}), '{"ok":true,"count":0}')

    def test_a_plan_too_large_for_one_environment_variable_is_refused(self):
        self.rows[0]['tools'] = ['x' * policy.MAX_PLAN_CHARACTERS]
        with self.assertRaisesRegex(policy.PolicyError, 'one environment variable'):
            policy.transform(self.contract, self.outputs())

    def test_command_reads_planner_lines_and_refuses_without_writing(self):
        with tempfile.TemporaryDirectory() as directory:
            planned, governed = Path(directory, 'planned'), Path(directory, 'governed')
            planned.write_text(''.join(f'{key}={value}\n' for key, value in self.outputs().items()))
            argv = ['--planned', str(planned), '--github-output', str(governed)]
            self.assertEqual(self.run_main(argv), 0)
            written = policy.parse_outputs(governed.read_text())
            self.assertEqual(set(written), set(policy.OUTPUTS))
            governed.unlink()
            self.rows.append(row('new-check@linux-x86_64'))
            planned.write_text(''.join(f'{key}={value}\n' for key, value in self.outputs().items()))
            self.assertEqual(self.run_main(argv), 1)
            self.assertFalse(governed.exists(), 'a refused plan publishes no matrix')
            planned.write_text('matrix={}\n')
            self.assertEqual(self.run_main(argv), 1)

    def run_main(self, argv):
        import contextlib
        import io
        import sys
        saved = sys.argv
        sys.argv = ['inherited_ci_policy.py'] + argv
        try:
            with contextlib.redirect_stdout(io.StringIO()), contextlib.redirect_stderr(io.StringIO()):
                return policy.main()
        finally:
            sys.argv = saved


if __name__ == '__main__':
    unittest.main()
