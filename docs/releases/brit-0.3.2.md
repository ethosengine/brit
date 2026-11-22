# Brit 0.3.2

Brit 0.3.2 is brit 0.3.1 made publishable, plus a CI change that stops deferred legs from
gating. The `brit` executable behaves as 0.3.1 does.

## What 0.3.2 contains

- **`brit-graph` 0.2.4.** 0.3.1 changed `brit-graph` (`from_repo_globs_excluding`, and the
  fingerprint sources and tests around it) without moving it off 0.2.3, which the `elohim`
  registry already held. Main's publish job refused it as an immutable version collision
  (run 44961618503, `brit-graph 0.2.3`: changed `src/fingerprint.rs`, `src/repo_fingerprint.rs`,
  `tests/repo_fingerprint.rs`), so `brit-cli` 0.3.1 was never published. `brit-cli` 0.3.2
  requires `brit-graph` 0.2.4.
- **Deferred legs outside the gate.** `scripts/ci/inherited_ci_policy.py` writes the required
  planned jobs to `matrix` and the deferred ones to a new `deferred` output. `run` executes only
  `matrix` and stays in `Tests pass`; a new `run-deferred` job executes `deferred` with the same
  permissions, environment and steps (shared through YAML anchors) and is not in `Tests pass`.
  An empty `deferred` skips the job. `.epr-meta/readiness.json` declares `run-deferred` deferred,
  and `scripts/ci/test_qualify_readiness.py` checks the split.

## Recorded verification

- Publish rehearsal (`scripts/ci/cargo-publish-brit.sh --check`, from a standalone clone): the
  0.3.1 tree skips 65 crates as equivalent and stops at the `brit-graph 0.2.3` collision; this
  release's result is recorded in its commit message.
- `run-ci/deferred-legs-20261121` (run 44971088719) at the CI change alone: `Tests pass`
  success; `advisories@linux-x86_64` failed as a deferred leg without failing the gate.
- Local: `test_qualify_readiness.py` 12 tests and `test_inherited_ci_policy.py` 14 tests OK.
- The release head's own `Tests pass` run is the release evidence.

## Not qualified

- The wall-clock saving was not shown by the run above: its slowest leg was a required one
  (`tests@windows-aarch64`), so `Tests pass` had no deferred leg left to stop waiting for.
- `advisories@linux-x86_64` and `size@linux-x86_64` stay red, as on 0.3.0 and 0.3.1.
- The hosted `tests@windows-aarch64` runner lost communication after 93 minutes on main's 0.3.1
  run; a re-run of the failed jobs passed.
