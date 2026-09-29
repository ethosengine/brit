import json
from pathlib import Path
import shutil
import tempfile
import unittest

import run as runtime


class RuntimeTests(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory()
        self.root = Path(self.directory.name)
        for relative in runtime.METHODS:
            target = self.root / relative
            target.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(runtime.ROOT / relative, target)
        (self.root / 'tests/journey/parity').mkdir(parents=True)
        self.git = runtime.binary(shutil.which('git'))
        self.binaries = {name: dict(self.git) for name in ['brit', 'ein', 'jtt', 'git']}

    def tearDown(self):
        self.directory.cleanup()

    def execute(self, body, lane='sha1', timeout=10):
        suite = 'tests/journey/parity/fixture.sh'
        (self.root / suite).write_text(body)
        return runtime.execute(self.root, suite, lane, self.binaries, timeout)

    def test_runtime_location_argv_skip_deferral_and_nonrepo_isolation(self):
        result = self.execute('''title "real title"
(sandbox
  it "observed actual argv"
  expect_parity effect -- --version
  compat_effect "explicit semantic deferral" -- --version
  shortcoming "not implemented"
  only_for_hash sha1-only || true
  expect_run 0 true
)
touch should-never-land-in-source
''', lane='sha256')
        self.assertEqual(result['status'], 'completed', result['stderr'])
        events = result['events']
        ends = [e for e in events if e['event'] == 'assertion-end']
        self.assertEqual(ends[0]['line'], 4)
        self.assertEqual(ends[0]['title'], 'real title')
        self.assertEqual(ends[0]['it'], 'observed actual argv')
        self.assertEqual(ends[0]['argv'], ['--version'])
        self.assertEqual(ends[0]['comparison'], 'exit-status')
        self.assertEqual(ends[1]['helper'], 'compat_effect')
        self.assertEqual(ends[1]['compatReason'], 'explicit semantic deferral')
        self.assertIsNone(ends[0]['compatReason'])
        self.assertNotEqual(ends[-1]['binary'], self.git['path'])
        self.assertTrue(any(e['status'] == 'skipped' for e in events))
        self.assertTrue(any(e['status'] == 'deferred' for e in events))
        self.assertFalse((self.root / 'should-never-land-in-source').exists())

    def test_failed_fixture_does_not_become_equal_exit_parity(self):
        result = self.execute('''title "bad setup"
function broken_setup() { return 7; }
expect_parity_reset broken_setup effect -- --version
''')
        end = [e for e in result['events'] if e['event'] == 'assertion-end'][0]
        self.assertEqual(end['status'], 'invalid-fixture')
        self.assertIn('setup failed', end['reason'])
        self.assertIsNone(end['compatReason'])

    def test_failure_and_timeout_preserve_missing_tail_and_actual_binary(self):
        result = self.execute('''title "failed operation"
exe_plumbing=/bin/false
expect_parity effect -- --version
expect_parity effect -- --version
''')
        self.assertEqual(result['status'], 'failed')
        ends = [e for e in result['events'] if e['event'] == 'assertion-end']
        self.assertEqual(len(ends), 1)
        self.assertEqual(ends[0]['binary'], '/bin/false')
        self.assertEqual(ends[0]['actualExit'], 1)
        self.assertEqual(ends[0]['gitExit'], 0)
        timed = self.execute('expect_run 0 sleep 10\n', timeout=.15)
        self.assertEqual(timed['status'], 'timeout')
        self.assertEqual(len(timed['incompleteAssertions']), 1)

    def test_command_substitution_output_is_never_claimed_byte_exact(self):
        result = self.execute('expect_parity bytes -- --version\n')
        end = [e for e in result['events'] if e['event'] == 'assertion-end'][0]
        self.assertEqual(end['comparison'], 'bash-normalized-combined-output')
        self.assertEqual(end['status'], 'pass')


if __name__ == '__main__':
    unittest.main()
