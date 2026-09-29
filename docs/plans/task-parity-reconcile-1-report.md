---
id: task-parity-reconcile-1-report
gap: plans__2026-09-28-parity-evidence-reconciliation#1
actor: agent:implementer@gpt-6
status: DONE_WITH_CONCERNS
commits: []
serves: brit-git-compatibility
---
# Witnessed parity helper execution

Added an opt-in Bash runtime witness, append-only JSONL event emitter and disposable
suite runner. Events identify actual source callstack line, suite digest, hash lane,
title/it, helper/mode, argv, actual executable, exit statuses and invocation pairing.
Explicit compat deferrals have a separate compatReason; failures and fixture errors
cannot inherit a semantic approval from equal exits. Bash `bytes` means normalized
combined output (command substitution trims trailing newlines), never byte-exact
stdout/stderr parity. Effect mode proves exit comparison only. Generic helpers do
not pretend their executable was Brit. Snapshot bootstrap and filtered-pipeline
modes remain distinct non-parity observations.

Every suite/hash lane runs separately in a disposable input copy with a clean HOME,
Git configuration and exact selected binary paths. Timeouts kill the process group;
partial assertions and unexecuted tails remain visible. Logs retain bounded previews
and streamed hashes. Method, suite and fixture inputs are fingerprinted before and
after, with exact inventory equality detecting added/deleted inputs. Executable
fingerprints are checked before and after. Installed Git version is recorded as a
different oracle from the pinned source census. jtt is fingerprinted without invoking
its unsupported --version command.

Scope: tests/parity.sh, tests/helpers.sh, tests/utilities.sh, and new
etc/parity/{runtime.sh,event.py,run.py,test_runtime.py}. Existing helper pass/fail
semantics remain; reset setup failures are observed as invalid-fixture rather than
promoted parity. No Rust changes, builds, commits, pushes or shared-checkout fixture
execution. Parent owns the full measured run and portable reconciliation consumes
the schema; reviewer remains independent.

Gate evidence: native `epr flow context` on all three existing helper files reports
no standalone gate project. Owning parent `just gate brit` → `EXIT=0`, existing
committed pin ethosengine/brit@bd915393df11 Tests pass success only; this does not
verify dirty runtime changes. Focused executable evidence:

- `python3 -m unittest discover -s elohim/brit/etc/parity -p test_runtime.py` → `EXIT=0`, 4 tests, no skips.
- `bash -n` on tests/parity.sh, tests/helpers.sh, tests/utilities.sh, etc/parity/runtime.sh → `EXIT=0`.
- Scoped `git diff --check` → `EXIT=0`.

Regression tests exercise actual Bash execution/location/argv, hash skip and semantic
deferral, isolated file creation, failed setup despite matching later exit codes,
actual Git/false/helper executable identity, comparison failure stopping later work,
timeout retaining an incomplete assertion, and normalized rather than byte-exact
comparison strength.

CLI exit 0 means the requested measurement was captured with stable inputs and
valid event syntax; red assertions, suite failures and timed-out lanes remain red
observations. It does not mean parity. `measurementComplete`, per-run status,
`incompleteAssertions`, and the stricter suite-completion field remain separate.
Invalid receipt provenance/event syntax exits 2. Incremental output preserves
completed lanes if an outer process interrupts the campaign. Sources are frozen
for the parent full run.

Concerns: this Linux/Bash observation runner does not establish cross-platform
behavior, independent acceptance or pinned-Git-version parity. Full campaign results
and CI observation wiring are separate parent integration evidence. Parent consumer
must reject unknown modes, wrong actual executable, stale method/input fingerprints
and unbound helper calls before deriving any qualified ratio.
