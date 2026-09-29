---
id: task-git-census-2-report
gap: plans__2026-09-28-git-feature-census#2
actor: agent:implementer@gpt-6
status: DONE_WITH_CONCERNS
commits: []
serves: brit-git-compatibility
---
# Compiled public parser census

Added public `brit --cli-surface-json`. It introspects the same final composed Clap
command used for ordinary dispatch, after Clap expands generated help, global
arguments and inferred argument metadata. No Git/native operation is dispatched.
A combined introspection and operation request is refused before side effects.

JSON schemaVersion 1 declares evidenceKind `parser-surface`, build metadata
sourceHead/sourceState/frontendPresets, and a recursive command tree containing
names, accepted aliases, options, positionals and subcommands. Options carry id,
long/short names and aliases, value names, required/global flags and possible values.
Frontend preset metadata stays the existing comma-separated build string. Source
state explicitly remains unverified: HEAD does not attest dirty worktree bytes.

Changes are limited to brit-cli/src/main.rs, new brit-cli/src/surface.rs and new
tests/cli-journey/tests/cli_surface.rs. No upstream gix changes, commits or pushes.
Existing dirty changes are preserved. Parent owns CI journey wiring and Python
comparison, and the pinned Git extractor is a separate implementation seat.

Gate evidence: `just gate brit` → `EXIT=0`, attesting committed parent pin
ethosengine/brit@bd915393df11 Tests pass success. This is not verification of the
dirty tree. Fresh local verification under the shared claimed/released cargo berth,
RUSTFLAGS empty, CARGO_BUILD_JOBS=1 and target
/tmp/brit-cutover-cargo-pool/family/main/home__matthew__git__elohim__elohim__brit/dev:

- `cargo +1.98 build -p brit-cli --bin brit` → `EXIT=0`.
- `BRIT_BIN=<target>/debug/brit cargo +1.98 test -p brit-cli -p cli-journey --bin brit --test cli_surface` → `EXIT=0`; 4 unit tests and 2 public journeys passed, 0 ignored.
- `cargo +1.98 clippy -p brit-cli -p gitoxide-core -p cli-journey --all-targets -- -D warnings` → `EXIT=0`; inherited removed-lint toolchain warnings remain.
- Scoped rustfmt --check and git diff --check → `EXIT=0` each.

Tests cover hidden/public command and option aliases, short aliases, inherited
global arguments, choices and required positionals. Public fresh-binary journeys
prove deterministic JSON in an empty non-repository, actual composed namespaces,
matching version provenance, zero filesystem writes, and refusal of malformed
introspection-plus-init invocations before initialization.

Concerns: this measures compiled parser exposure only, never command behavior,
option semantics, acceptance or Git compatibility. The default/max build was
exercised; the schema reports the actual preset for separate builds without
asserting unmeasured preset conformance. Logs are /tmp/brit-census-build.log,
/tmp/brit-census-tests.log and /tmp/brit-census-clippy.log; commands/results above
preserve their transient evidence.
