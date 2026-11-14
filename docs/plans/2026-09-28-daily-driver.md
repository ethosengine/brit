---
title: Brit daily driver stations
status: active
serves: brit-native-daily-driver
---
# Daily driver

This is the per-habit work projection of `2026-09-28-governed-readiness.md`.
The owning habit declares status; a checked station never establishes acceptance.
Dependencies below are explicit review requirements, not an implemented checkbox
scheduler. Use native context for observed environment and citation blockers.

- [ ] Safe scoped authoring and recovery: public executable proves staged-byte scope, hook execution/refusal, dirty-neighbor preservation, linked-worktree safety and interruption recovery against stock Git. Local safe-authoring sprint is evidence, not complete station acceptance. Serves brit-native-daily-driver.
- [ ] Native gates and independent integration: exact target/source revision binds gate results and independent review; concurrent target movement or contrary evidence refuses integration; pre-push cannot be silently skipped. Requires scoped recovery and inherited discipline; serves brit-native-daily-driver.
- [ ] Installed delivery: fresh standalone install, feature/platform matrix, upgrade and rollback prove the executable and installed epr interface; package publication and clean consumer tests identify exact source revisions. Requires native integration; serves brit-native-daily-driver.
- [ ] Operational qualification through our own ongoing development across workspaces: complete the loop, preserve Git interoperability and recover interrupted work, recording actual outcomes as we daily-drive Brit. Cross-workspace verification follows this integration pass; scripted identities do not establish it. Serves brit-native-daily-driver.

## Daily-driver release integration — 2026-11-12

The first candidate (`abc4c1023`, CI 43320400970) passed strict lint, MSRV,
readiness/package contracts and optimized CPU qualification, but exposed three
portability gaps before main moved. Git 2.56 clips diff-stat names by display
width; Brit now preserves whole UTF-8 characters, with the hosted oracle captured
as a fixture and separate wide/combining/malformed-byte unit cases. Ubuntu's
coreutils requires the invoked name to match the executable name, so the
attribution test's synthetic `gtimeout` delegates to the real `timeout` path.
Both 32-bit minimal containers now install GNU time for the existing RSS probe;
neither the 64 MiB measurement assertion nor the 1 GiB scratch cap is relaxed.
The Unicode fixture failed locally before the renderer correction, then all
26 diff-stat, attribution and bounded-checkout journeys passed, along with the
Unicode unit cases, strict scoped Clippy, formatting and 12 readiness contracts. The candidate's
20 dependency-advisory findings remain separately recorded; no ignore was added.
The final Windows fixture log also exposed invalid host filenames (tab, quote
and literal backslash). Those entries remain exercised on Unix; Windows retains
the shared names, scaling, selection and rename assertions. All 14 diff-stat
journeys and formatting pass locally after this fixture-only correction.
Candidate `01ca0de97` then passed both 32-bit suites, but macOS exposed a missing
timer prerequisite and host filename assumptions. CI and installed delivery now
install Homebrew coreutils on macOS, and timer fixtures resolve either executable
name. The combining-name fixture uses x plus acute, which has no precomposed form;
macOS retains newline quoting while malformed-byte names remain on filesystems
that admit them. All 18 affected local journeys pass. Platform jobs now finish
their siblings after a failure so independent portability results stay visible.
Candidate `a893fcf97` passes macOS and both 32-bit suites. Windows ARM's only
failure (5,414 cases pass) and the x64 diff-stat failure are byte-for-byte the
same output except for eight CR bytes added to the golden file by checkout.
That fixture now declares `-text`; the exact-byte assertion is unchanged.
The separate x64 fixture lane also reports a concurrent note-lock refusal in
the existing CAS stress test. Its source and test match main exactly, and the
first Windows candidate passed it; this observation remains separate from the
fixture correction, with no timeout, retry or lock-semantics change.

Brit 0.1.14 packages the nine sprint commits after 0.1.13: branch creation and
shared mutation guarding, resolved merge commits, diff statistics, passive commit
attribution, linked-worktree rebase publication, canonical unchanged-file detection,
replay loop cancellation and bounded worktree checkout. The release preserves the
hard 1 GiB scratch cap and the measured cancellation boundaries below.

The root toolchain and fork-owned CI/delivery workflows select Rust 1.100.0.
Library MSRVs, explicit MSRV/nightly checks and the addressed governance checker
remain separately declared. The journey runner resolves both the old `deps` layout
and Cargo 1.100's `build/<package>/<unit>/out` layout, including Windows suffixes,
so workspace tests can find the built executable without `BRIT_BIN`. The previously recorded `commit.rs:307` Clippy finding
is resolved by combining the nested guard without changing its short-circuit order;
all four workspace Clippy configurations now pass, with no new suppression.

The 70-package registry closure now carries 56 patch-version increments,
including reverse dependencies whose minimum requirements bind the new payloads.
The first 55 candidate versions were absent at the read-only registry audit;
all 15 then-reused payloads matched before publication. Main CI `43335646602`
passed every test job, then correctly refused a changed packaged lockfile for
`gix-imara-diff 0.3.1` after uploading 16 dependencies. Its explicitly versioned
dev dependencies now resolved the newly available gix-hash/gix-object versions;
the pre-publication payload comparison had not observed that later registry state.

The correction allocates unused `gix-imara-diff 0.3.2` and binds its two consumers
to that version. Their allocated versions and Brit 0.1.14 remain unpublished and
can be completed without overwriting a release. All 16 newly uploaded payloads
still match exactly, and the other 14 reused packages remain equivalent. The new
imara package passes the publisher's preflight; 163 tests across its diff/merge
consumers pass with no failures or ignored tests. All 657 external workspace-lock
records are unchanged; only the local imara package version changes. No runtime
source or publisher immutability rule is relaxed. The superseded source-build job
`43340236592` was cancelled before publication while this correction is qualified.

Maintainer station: package qualification / between pre-upload payload audit →
immutable closure publication / missing node: packaged dev-dependency lockfiles
remain admissible after earlier dependency versions become available. The real
before/after lock diff and package receipts are in isolated Elohim
`genesis/local-dev/brit-noise-20261112/integration-publication-repair-*` and
`integration-lock-drift-gix-imara-diff.diff`; future preflight should model the
final dependency availability, not assume the initial registry snapshot is final.
Publication still verifies immutable package payloads and
registry metadata in dependency order; no version may overwrite published bytes.

Local integration verification passes all four workspace strict-Clippy presets,
workspace formatting, 198 ordinary CLI tests, 19 branch/feedback tests, 163 native
replay/recovery tests and 34 fault-enabled commit tests. The ordinary binaries are
restored after fault testing. A fresh Cargo 1.100 fixture, the runner unit test and
two CLI tests without executable overrides prove discovery in the new layout;
strict journey-target Clippy also passes. The 25 publisher tests, 12 readiness
contracts, delivery/artifact contracts, isolated reach-hook checks and declared
Rust-style checker pass. Hosted qualification remains a separate exact-commit gate.

Delivery qualification includes the new attribution, merge, diff-stat and bounded
worktree journeys. Candidate branch CI must qualify the exact integration commit
before main moves; main's existing workflows then publish the crate closure and
CPU-qualified source-pinned binaries. Neither source checks nor a release change
the RED daily-driver readiness declaration. Independent review, full operational
qualification, installed upgrade, the broader cancellation backlog and the separate
cleanup race remain open. The Elohim gitlink must wait for its own landing lane
and the pinned Brit commit's green hosted attestation.

## Bounded worktree checkout — 2026-11-12

`brit worktree add` now streams loose and packed blobs into operation-owned scratch
on the destination filesystem. OFS/REF deltas use a disk-backed descriptor stack,
streamed instructions and one live base/result pair; obsolete bases are reclaimed
before reconstructing the next version. Hash and size validation, alternates,
replacement objects and SHA-1/SHA-256 are retained. The buffered object reader's
allocation limit remains unchanged. A size-only observation of the reported shared
file found 20,574,865 bytes on this run, above the 16 MiB reduced-trust default;
its contents and original failed checkout were not replayed. This is the operator-authorized, narrowly scoped
upstream-ready plumbing exception, not a new cache or worker pool.

The **1 GiB aggregate scratch cap is fixed** and charged before growth, including
holes, delta metadata and simultaneous input/output spools. Publication releases
scratch only for completed destination files; serial entry processing avoids
competing blob stages. Cleanup failure does not refund still-live scratch. There
is no automatic cap increase or Git fallback. A base/result pair or conversion
that cannot fit still refuses with the quota diagnostic. Completed worktree files
and the existing index/tree/attribute metadata machinery are outside the scratch
budget. The payload transfer buffers are fixed at 64 KiB; process RSS measurements
below are observations, not attribution of every allocation or a whole-process cap.
External filter allocations, OS cache and blocking I/O remain outside the bound.

Built-in conversion order remains ident, EOL, encoding, then smudge. Incremental
statistics and encoding match the existing buffered implementation across chunk
boundaries. Single-file filters pump a borrowed spool concurrently into stdin
while draining stdout into charged scratch; optional filter failure retains its
input, while quota and I/O failures refuse. Immediate and delayed process-filter
protocols reuse the existing transport. Existing gix ident output has no space
before the closing `$`, unlike Git; this slice preserves that behavior and tests
buffered/streamed equality rather than claiming new ident parity.

The existing prepared-worktree boundary owns registration and rollback. Failure
removes owned destination/registration state, preserves source HEAD/index and
foreign locks, and reports that a newly created branch remains. After inspection,
retry uses the existing branch without `-b`. Tests also cover empty trees,
executables, symlinks, gitlinks and pre-existing destination content. The separate
hostile-writer cleanup race remains open.

- [x] Cross the buffered allocation boundary with loose/packed entries at 16 MiB minus one byte, exactly 16 MiB, plus one byte and 20 MiB while keeping the 16 MiB object allocation limit. Probe: `large_loose_and_packed_worktrees_preserve_allocation_safeguards`. Serves brit-native-daily-driver.
- [x] Reclaim delta bases under the actual cap: a 384 MiB base plus three delta versions succeeds with 805,306,393 bytes peak scratch (about 768 MiB); retaining all four versions would exceed 1.5 GiB. OFS depth 1,500, REF depth 40, cross-pack bases, alternates, replacements, corruption and cycles are separately exercised. Probe: `gix-odb` stream tests. Serves brit-native-daily-driver.

Measurements use Rust 1.100.0 (`0ef1bc386`, LLVM 23.1.5), Linux x86_64 debug builds,
debug information disabled and two compiler workers. This loop selects the new
toolchain explicitly. The subsequent integration pass pins Brit only to 1.100.0;
the other session owns the shared Elohim pins. `BRIT_BIN` and
`BRIT_REBASE_RETAINED_BIN` explicitly select the tested artifacts because this
Cargo version changed test executable layout. No package version or shared Elohim
Rust pin changes are included. Private source/executable identities, commands,
exit codes and measurements live in the isolated Elohim evidence directory,
`genesis/local-dev/brit-noise-20261112/bounded-evidence.json`.

The final executable is SHA-256
`d0149e45ecfb6dc8c0b94f40325e813c9bfbf9344b019066d3ebc8f1a0f3a8db`.
All six controlled cases succeeded with exact-byte comparisons, below the 64 MiB
RSS test ceiling. GNU Time 1.9 measured process maximum RSS; a monotonic clock
measured elapsed time. These are debug-build observations on a shared host,
not an isolated throughput SLA or a heap-allocation census.

| Input | Conversion | Wall time (ms) | Peak RSS (KiB) |
| --- | --- | ---: | ---: |
| 64 MiB | plain | 425 | 43968 |
| 64 MiB | crlf | 2435 | 44736 |
| 64 MiB | filter | 553 | 43776 |
| 256 MiB | plain | 1614 | 44160 |
| 256 MiB | crlf | 8800 | 43968 |
| 256 MiB | filter | 1754 | 44160 |

The 384 MiB delta-chain worker returned in 3.768551418 s with peak scratch
805,306,393 bytes. A real SIGINT after observing at least 1 MiB of blob scratch
returned in 35 ms, preserving the source index and foreign lock and retaining
the created branch. The hard-quota test makes an optional filter produce 1 GiB
while its input remains live: checkout must refuse rather than silently fall
back, and owned destination/registration state must disappear.

Validation passes all eight public bounded-checkout journeys; three quota/copy
checks; five focused ODB stream checks and the 92-case ODB suite; 69 filter,
12 zlib, 30 worktree-state and 52 repository-worktree checks. Regression coverage
passes 13 replay unit checks, 130 native recovery cases, 72 fault-enabled and
42 ordinary public rebase/pull/switch cases, with zero ignored tests. The minimal
object-format build also passes without requiring the new stream feature.
Workspace formatting, all 12 readiness-contract checks and the declared Rust-style
checker pass; the latter retains its `brit-build-ref` scope. Strict Clippy passes
the affected plumbing crates, `gix` library and new CLI test. Full core strict
Clippy still refuses the untouched `commit.rs:307` `collapsible_if` finding,
recorded separately in `bounded-clippy-core.log`. No suppression is added.
Rust 1.100 additionally reports existing manifest warnings for `gix-imara-diff`
lint inheritance and the test-only `gix-hashtable` dependency in a minimal
`gix-object` build; these are recorded separately from source lint verdicts.

Story-harvest node: `brit-native-developer`, between branch creation and usable
linked worktree. Given a packed 384 MiB blob with three delta versions and the
1 GiB scratch cap, when checkout reconstructs it, then exact bytes are produced
with peak scratch below the cap; if the cap is exhausted, owned setup rolls back
and the retained branch supports an inspected retry. The runnable stations are
the ODB resource test and public `worktree_bounded` journeys. This preserves the
resource constraint without weakening whole-workflow acceptance.

Readiness remains **RED**. Independent review, upstream CI, publication,
installation, version allocation, parent-pin integration and the broader
cancellation backlog remain subsequent work. Controlled SIGINT during a blob
write is tested; no universal Ctrl-C latency guarantee is claimed.

## Cancellation within selection and index loops — 2026-11-12

Brit-owned selected-commit collection, candidate-chain traversal, tree collection,
footprint traversal and materialization, both index-map constructions and changed-path
comparison now poll the existing interrupt flag. External-filter tree traversal also
polls between trees. Library calls remain unchanged, with checks at their boundaries.
Ordering, deduplication, mode comparisons, affected-gitlink refusal and selected-history
semantics are retained. No public command, flag, type, cache or journal format changes.

- [x] Refuse after 16 completed items, before another item is processed, in process-isolated deterministic probes at 10,000/100,000 index entries and 1,000/10,000 synthetic candidate/history records. Probe: `cargo test -p gitoxide-core --lib repository::replay::tests`. Serves brit-native-daily-driver.
- [x] Preserve recovery when the shared comparison helper cancels inside a prepared replay step: journal, index, refs and worktree snapshot stay equal; only unreachable merge objects may be added. Reopen and resume or abort, retaining the independent recovery copy. Probe: `cancelled_merge_keeps_prepared_evidence_and_can_resume_or_abort`. Serves brit-native-daily-driver.

All 24 final controlled measurements returned in less than one second; the slowest
was 68.611856 ms. Eighteen cover the two requested sizes across nine loop points;
six exercise both resume and abort after interrupting shared index comparison.
Each worker starts a monotonic clock immediately before triggering the real flag.
Test injection returns to the production poll, and a worker that finishes before
interruption fails. The public SIGINT rendezvous likewise cannot detect cancellation
itself: the controller signals, releases it, and the real poll must refuse.
All eight new public SIGINT checkpoints passed; the slowest signal-to-process-exit
measurement was 78.126607 ms. All 72 fault-enabled and 42 ordinary public
rebase/pull/switch journeys passed against the separately hashed rebuilt binaries.

Measurements use Rust 1.96.1, unoptimized Linux x86_64 test binaries with debug info
disabled and two Cargo build jobs. Private evidence records source and executable
hashes, the full toolchain identity, workload, processed count, elapsed nanoseconds
and outcome under the isolated Elohim worktree's
`genesis/local-dev/brit-noise-20261112/cancellation-measurements.json`.
These are controlled local results. Decompression, individual-object hashing,
merge-base internals, serialization, allocation/deallocation and blocking filesystem
calls remain outside the latency claim; there is no universal Ctrl-C guarantee.

Story-graph station (`@regression`, executable Rust probes above): given a selected
history of 1,000/10,000 records or an index of 10,000/100,000 entries, when the real
interrupt flag is triggered after item 16, then the loop refuses before item 17 and
returns within one second. Chain: daily-driver safe recovery / between validated
history → durable preparation (and prepared replay → next step) / missing node:
Brit-owned loop cancellation preserves the appropriate read-only or recoverable
boundary / current state: locally green; independent review and delivery pending.
Native abort and terminal journal cleanup are separate stations; this slice does
not change the journal-cleanup race.
The final native replay run passed 65 cases but two existing 20-second lifecycle
checkpoint waits expired while the CLI recovery suite ran concurrently. Both
failed fixtures passed individually after that competing suite finished, with
no source or timeout changes. The failed run remains in
`cancellation-core-complete.log`; the focused recovery verdicts are separate.

Local validation covers 13 replay unit checks (including five new tests), 130
native replay/recovery cases across the final run and focused timeout reruns,
72 fault-enabled and 42 ordinary public journeys, and 12 readiness-contract
checks. Workspace formatting and the declared Rust-style checker pass. The
checker retains its declared `brit-build-ref` scope; this is not new core style
certification. Strict core Clippy still refuses the untouched `commit.rs:307`
`collapsible_if` finding; advisory all-target Clippy has no new replay findings,
and strict CLI rebase Clippy passes. No suppression was added.

Readiness remains RED. Independent review, upstream CI, publication, installation,
version allocation and parent-pin integration remain subsequent work.

## Unchanged attributed working files — 2026-11-12

Ordinary `brit diff` and `brit diff --stat` now skip an attributed working file
only after its effective mode and canonical content bytes equal the index.
Previously, even a clean checkout refused on an unchanged `diff=lfs` file.
The comparison reuses the authoring reader's path validation, symlink handling,
line-ending conversion, external-filter refusal and `core.filemode` behavior.
It reads content rather than trusting timestamps, sizes or raw filesystem bytes,
and reuses those loaded bytes when a patch or summary is required.

Changed attributed content, deletions, modes and types still refuse with no
partial output. Unconfigured filter names can pass through; configured external
filters still refuse. Global external diff commands, unsupported index states
and working gitlinks retain their refusals. Custom drivers and text conversion
remain unsupported. Staged and explicit-tree comparison semantics are unchanged.

The `native_authoring` and `diff_stat` suites preserve the boundary with isolated
Git oracles, unchanged LFS/binary/symlink fixtures, canonical CRLF conversion,
effective executable modes, literal selection, external-command sentinels,
repository snapshots and foreign locks. The attribute-refusal fixture now has
an actual unstaged change; its old working bytes incorrectly equaled the index.
The fault-enabled cancellation probe also crosses an unchanged attributed file
before cancelling both patch and summary output.

Local ordinary verification passes 36 tests and fault-enabled verification passes
37; readiness-contract checks pass 12 tests. Workspace formatting and the
declared Rust-style checker pass. Actual-checkout patch and stat smoke checks
also pass with the rebuilt executable. The
previously recorded strict Clippy finding in untouched `commit.rs:307` remains
separate. Readiness stays RED: independent review, upstream CI, publication,
installation, version allocation and parent-pin integration remain separate.
Private evidence belongs to the parent workspace's
`.worktrees/epr-noise-20261112/genesis/local-dev/brit-noise-20261112/unchanged-evidence.json`.

## Linked-worktree rebase publication — 2026-11-12

Git normally writes `../..` in a linked worktree's `commondir`. Replaying an
attached branch reached `ready-final` and then refused with `unsafe metadata
ancestor`: replay compared a canonical directory with its lexical spelling,
and shared publication validation selected the shorter worktree prefix before
the common-directory prefix.

Replay now checks every original ancestor as a directory without symlinks,
including components later removed by `..`, before comparing the canonical
location with the existing path utility's lexical normalization. Shared metadata
validation selects the longest matching declared root. Descendant validation and
cross-filesystem refusals remain in force, as do exact resource allowlists,
repository/inode identity, branch and old/desired state comparisons, and owned
locks. There is no journal migration, path rewrite, public interface or version
change. Version-1 encoding and repository identity remain readable; ordinary
continuation still advances the recorded state.

The public rebase suite covers ordinary attached completion with a nested branch
and spaces in the worktree path, conflict resolution followed by `--continue`,
abort, and detached HEAD. It checks history, trees, attachment, reflogs, staged,
unstaged and untracked neighbors, and the main worktree's private state. The
existing main-worktree journeys remain controls. For a compatibility rehearsal,
`BRIT_REBASE_BASELINE_BIN` can point to the preserved predecessor executable when
running `linked_worktree_rebase_publishes_nested_branch_and_preserves_both_worktrees`:
it creates a disposable old-format `ready-final` journal, checks unchanged bytes
on inspection, then completes with the candidate. Both invocations use the same
editor environment because journal recovery also pins configuration.

Focused replay tests reopen version-1 state, interrupt a durable final decision
and partial metadata publication, and refuse symlinked ancestors, a symlink hidden
by `..`, replaced repository identity, foreign paths and locks, and changed branch
or HEAD state. Existing recoverable evidence and external targets survive refusal.
CI's existing replay unit lane now includes the ancestor regression.

Story boundary: `brit-native-daily-driver` / between completed replay and branch
publication / the original path traversal must be safe before lexical cleanup;
`metadata_checks_original_traversal_even_when_parent_components_cancel` measures
the distinction. Given `directory/./../topic`, publication validation accepts a
real directory; replacing it with a symlink refuses even when canonical and
normalized destinations would agree. The runnable story lives in this standalone
repository; parent a2o placement remains separate work.

Measured locally: 130 core replay/recovery/retained-copy regressions, 8 replay
unit checks, 4 power-loss checks, 72 fault-enabled public rebase/pull/switch tests,
8 ordinary-build rebase tests and 12 readiness-contract tests pass. The preserved
predecessor reproduces the failure and its journal completes with the rebuilt
candidate. Workspace formatting and declared Rust style pass. Additional strict
Clippy refuses a pre-existing `collapsible_if` in unchanged `commit.rs:307`;
the advisory run reports no new findings in this change. No suppression was added.

Readiness remains RED. Independent review, upstream CI, publication, installation,
version allocation and parent-pin integration remain separate delivery work.
Private receipts: parent
`.worktrees/epr-noise-20261112/genesis/local-dev/brit-noise-20261112/rebase-evidence.json`.

## Commit attribution adapter — 2026-11-12

The standalone `.githooks/prepare-commit-msg` connects Brit's existing hook
invocation to installed EPR's `session commit-trailers --json` interface. EPR
owns model and session attribution; the adapter adds no identity lookup or
session mutation. The existing hooks path and pre-push hook are unchanged.
An explicitly set `EPR_BIN` is authoritative, including an empty, missing or
obsolete override; otherwise the adapter uses `epr` on PATH. Inspect an obsolete
override before deliberately removing it from the command environment.

The hook requires Bash, jq, Git, standard file utilities, and `timeout` or
`gtimeout`. EPR lookup, JSON validation and Git trailer processing each have a
five-second deadline and one-second forced-termination grace. A measured response
must contain valid trailer pairs without embedded control characters. Git's
`interpret-trailers` preserves existing contributors with `addIfDifferent` and
`add` semantics. Decoration uses an owned temporary copy beside the message,
preserves its permissions, and replaces the original only after success.
`not-applicable` is quiet. Missing prerequisites, incompatible EPR, invalid
responses, timeouts and processing failures leave the original message intact
and warn `UNMEASURED`; attribution remains passive, including under `--no-verify`.

`tests/cli-journey/tests/commit_attribution.rs` runs the checked-in hook through
Brit and stock Git in disposable main and linked worktrees. It checks exact
committed trailers for ordinary, empty, amended and resolved merge commits,
spaces in paths, existing contributors, repeated invocation, explicit overrides,
dependency failures, malformed pairs, partial parser output and forced timeout
termination. Staged scope and dirty neighbors remain intact. The existing public
CLI CI lane now selects it. A stale authoring fixture also now distinguishes
orphaned merge metadata from an independently unmerged index.

Local verification passes 50 Rust tests (19 authoring, 18 amendment, 9 merge,
4 attribution), 12 readiness-contract tests, workspace formatting and the
declared Rust-style checker. A disposable-message rehearsal against real installed
EPR produces measured trailers; the final committed object must be checked too.
Private receipts: parent
`.worktrees/epr-noise-20261112/genesis/local-dev/brit-noise-20261112/attribution-evidence.json`.
Readiness remains RED. Independent review, upstream CI execution, publication,
installation, version allocation and parent-pin integration remain separate.

Historical omission: `5520dcdc7f919ddd35bcfcb42d34c577871ecfbf` lacks attribution
trailers despite a successful native lookup because this standalone hook was
absent. Preserve that commit and earlier history; coordinate append-only follow-up
with the integrator before publication, without retrospective trailer claims.

## Read-only diff summaries — 2026-11-12

`brit diff --stat [-- <literal files>]` compares tracked working files with the
index. `brit diff --cached --stat` (also `--staged`) compares the index with HEAD,
including unborn HEAD. `brit diff tree --stat <old-treeish> <new-treeish>` compares
only the requested trees, including in bare repositories or with an unusable
working index. Existing patch output and `diff file` retain their interfaces.

Summaries use fixed, uncolored 80-column Git layout with rename detection off;
renames appear as deletion plus addition. They cover text, binary sizes,
symlinks, executable modes and file-type replacements. File selection retains
the existing literal-path restrictions. Revision positionals, tree path filters,
width controls, `--numstat`, and `--shortstat` remain unsupported.

`tests/cli-journey/tests/diff_stat.rs` binds this station to exact Git output and
repository-state snapshots. It proves staged/unstaged separation, unchanged
refs/reflogs/index/objects/worktree/foreign locks, and inspection during an active
mutation. Unsupported index states, working or changed staged gitlinks, external
commands and changed paths with diff attributes refuse before output. Tree
comparisons check attributes in both trees;
working attributes cannot stand in for those declarations. The fault-enabled
lane proves cancellation before output for all three summary modes. Ordinary
and fault journeys run in the existing public CLI CI lanes. These are local
source proofs; review, upstream CI and installed delivery remain separate.

## Branch creation — 2026-11-12

Daily-driving follow-up, 2026-11-12: a developer must be able to create a named
branch at HEAD or an explicit commit without changing HEAD, staged bytes or
unstaged work. Existing names and invalid/non-commit start points must refuse
without replacing a ref. `brit-cli/tests/branch_create.rs` is the executable
story; installed delivery remains separate from source validation.

The native command is `brit branch create <name> [start-point]`. It reuses the
worktree branch-name validator and a `MustNotExist` ref transaction. It does
not offer branch replacement or deletion. Four executable regressions cover
dirty/staged preservation, tag peeling and invalid sources, and linked-worktree
HEAD selection, and the shared mutation guard. Creation refuses during an active
guarded operation in either checkout; branch listing remains available.
The remote-tracking regression also verifies the existing
`brit remote -n origin refs`, `brit fetch -r origin`, and explicit destination
refspecs. Supplying refspecs replaces the configured mappings; when naming a
single branch, use `brit fetch -r origin +refs/heads/dev:refs/remotes/origin/dev`
to explicitly refresh `origin/dev`.

The inherited `EPR_BIN` candidate can select a recorder older than the installed
device head. On this workspace, removing that obsolete override restored both
feedback capture and native Codex commit trailers. A new Elohim worktree also
needs its private feedback capacity store initialized: inspect `epr feedback
capacity` before an explicit `migrate --legacy-writers-quiesced`; the migration
is only appropriate after establishing that no legacy writer is active. Neither
finding calls for suppressing capture-gap warnings or changing another session's
pin. Arbitrary-shell seam coverage and missing live Claude model
context remain separate gaps.

## Resolved merge commits — 2026-11-12

`brit commit -m <message>` or `brit commit -F <file>` completes an ordinary
two-parent merge after its conflicts have been resolved and staged. The commit
uses the original HEAD and then MERGE_HEAD as parents, including when the staged
tree equals HEAD's tree. Hooks retain the normal message and attribution path;
the committed tree reads staging after hooks, preserving unstaged work.

This path supplies no merge engine or editor. It refuses unresolved indexes,
unborn HEAD, malformed or multiple merge heads, mixed operations, required
signing, amendment, implicit messages and pending autostash/rerere work.
Metadata snapshots and ordinary locks protect completion; the existing guarded
ref transaction pins HEAD's identity and attachment before publication.

After publication, only captured and unchanged merge metadata is removed from
the worktree's Git directory, with MERGE_HEAD last. ORIG_HEAD, index and worktree
remain intact. Post-commit runs after cleanup. Cleanup failure reports the
published commit and remaining metadata, with no rollback or automatic retry.
An incorporated MERGE_HEAD refuses another commit, including after interrupted
cleanup. An uncatchable process kill cannot print a diagnostic and may leave
ordinary lock files; inspect the published HEAD and retained metadata before
any manual recovery.

`tests/cli-journey/tests/commit_merge.rs` is the executable story, with stock Git
as the exact-object oracle and the existing `switch-test-faults` build feature
for publication and cleanup interruption. Source verification does not establish
independent review, installed delivery, upstream CI or parent-pin integration.

## Collision-fixture rebase — 2026-11-05

Installed Brit 0.1.9 cannot replay divergent commits in its own repository:
the worktree witness hashes raw file bytes, and the committed SHA-1-is-a-Shambles
fixtures trigger collision detection. An isolated native worktree rehearsal
(`brit rebase c387e79bbeb9fb8b3ad86304aaded1b3b9cfd4ef`) reproduced the refusal
after 41 seconds. HEAD, index bytes, every worktree file and the absence of an
operation journal remained unchanged. An earlier ancestor-only rehearsal was
a no-op and is not the reproduction.

The file fingerprint keeps ordinary raw SHA-1 values unchanged. Only a detected
collision selects `git-blob-sha1:<digest>`, calculated with Git blob framing and
collision detection still enabled. Capture and original-image validation use
the same calculation. Full public journeys then exposed the same raw-digest
refusal in retained-copy payloads after branch publication. Their capture,
inspection and extraction now share the file fingerprint too. Journal and
manifest checksums, receipt formats and Git object verification retain their
existing algorithms and refusal behavior. Ordinary retained payloads and their
receipts remain byte-identical; collision payloads previously could not produce
a complete retained copy.

Full-history cleanup then exposed repeated manifest validation: every released
object called `payload()`, which rebuilt the logical-path set for every entry.
Cleanup now borrows a manifest-validated reader for its traversal. Rust's
immutable borrow prevents metadata changes during that traversal; each payload
still rereads live paths and bytes and verifies ownership, its fingerprint and
Git object identity. Public single-payload reads retain per-call validation.
A controlled test checks 64 and 128 manifest-entry visits for batches of 64 and
128 payloads, plus live tamper and edited-metadata refusals. Extraction retains
its existing per-call validation; no cancellation-latency bound is claimed.
The first collision candidate passed complete CI (run 41967763430 at
bb4b1f85436235ae0830499fa3b53ff127def205); this cleanup follow-up requires its own
exact-revision qualification. Both full-history clones completed through explicit recovery with the reviewed
reader change. Their staged, unstaged and untracked neighbors remained identical,
both collision fixtures survived, and active journals were removed. The original
0.1.9 abort journal also recovered after hard interruption: HEAD, the retained
original index bytes, private file and deliberately deleted fixtures were
preserved, with an independently validated retained copy left behind.

- [x] Reproduce the installed refusal without modifying HEAD, index or private work. Serves brit-native-daily-driver.
- [x] Preserve legacy fingerprints and checksum envelopes while distinguishing both committed collision fixtures. Serves brit-native-daily-driver.
- [x] Verify actual Brit-history replay, conflict/continue, abort, interrupted recovery and an old-binary journal across the upgrade. Serves brit-native-daily-driver.
- [x] Pass affected core/public suites, formatting, strict Clippy, Rust-style gates and independent review. Serves brit-native-daily-driver.
- [ ] Pass exact-revision CI, publish coordinated unused versions, advance the parent pin and prove the installed binary. Brit 0.1.10 remains the peer's repository-opening/fingerprint release. Serves brit-native-daily-driver.

Maintainer node: between verified Git objects and durable replay preparation,
checkout witnesses and retained recovery payloads must distinguish collision-test
payloads that are valid, distinct Git blobs. The fingerprint and public recovery regressions name
this boundary. Between retained-copy completion and reservation release, the
cleanup traversal must reuse immutable metadata validation while rechecking
every live payload; the borrowed-reader regression names that station.
Cancellation latency, helper packaging, submodule support and
named-remote handling remain separate work. Evidence is retained in the parent
`genesis/local-dev/brit-collision-20261105/`; this station does not qualify the
whole daily-driver habit.

Verification so far: 186 initial core checks passed (zero ignored), then the
retained-payload extension passed all 13 retained-copy checks and all 13 public
fault/recovery journeys with both collision fixtures present. A genuine 0.1.9
conflict journal was resolved and completed by the candidate, preserving staged,
unstaged and untracked neighboring files and keeping them out of the commit.
The cleanup follow-up passes all 127 affected core replay checks, the isolated
reader regression, all 13 public fault/recovery journeys, formatting, prescribed
strict Clippy and Rust style. Independent review approved the production changes
after three test-fixture corrections (private journal permissions, extraction output path and isolation
of the reader regression).

Rehearsal limits remain explicit. Changing shared repository configuration during
an active linked-worktree rehearsal correctly refused recovery until restored.
A Python subprocess with `cwd` changed but inherited `PWD` also refused the policy;
the matching shell working directory continued the same journal. Final publication
in a linked checkout separately refuses `unsafe metadata ancestor` when its
common-directory path contains `../..`; that path-handling issue is outside this
collision slice. The real-history finish proof uses an independent native clone.
The first native HTTPS candidate push passed its gates but failed in send-pack
with a closed channel. The branch was confirmed absent remotely before a Git
fallback pushed it with the same gates enabled. The first follow-up public run
also selected a CLI without fault checkpoints; rebuilding through the CI CLI
feature chain made all 13 tests pass without a source change.
Selected unused candidate versions are CLI 0.1.11, core 0.63.5 and gitoxide
0.60.5; the peer's CLI 0.1.10 and possible .4 versions remain available. The
allocation proposal was sent through the steward channel; peer acknowledgment
is still outstanding. Scoped Clippy uses the repository's CI compatibility
flags (`-D warnings -A unknown-lints --no-deps`).

## Rebase reliability — 2026-11-04

A one-commit integration spent more than three minutes validating history before
creating a journal and did not stop on the first SIGINT. This slice retains the
complete pinned history required by the replay design. It removes repeated
verification of identical objects within one selection and makes that read-only
phase cancellable. Validation results never survive into another operation.

- [x] Prove cancellation leaves HEAD, index, worktree and operation metadata unchanged, including a real CLI SIGINT during object validation. Serves brit-native-daily-driver.
- [x] Verify the same complete object closure as Git for shared histories; retain missing-object, checksum and wrong-type refusals. Measure repeated-content fixtures at two history sizes. Serves brit-native-daily-driver.
- [x] Run local replay/recovery and public artifact journeys. Serves brit-native-daily-driver.
- [ ] Qualify the committed revision in CI and independent review, then deliver the release and parent pin. Serves brit-native-daily-driver.

The prior release `a8187cb3` is already published and installed as Brit 0.1.8 on
shem; parent `bbb8db95` reached dev. Its seven installed probe rungs and complete
CI/publication are recorded in the parent's
`genesis/local-dev/brit-delivery-20261104/`. These observations supersede the
earlier pending delivery entries, without qualifying the whole daily-driver habit.
The peer agent owns the separate fingerprint/open-helper repair in `brit-cli`;
this slice owns `gitoxide-core` replay validation.

Local evidence: the cancellation regression first failed on the unmodified
validator. With the repair, 49 replay, 21 interactive and two validation tests
pass (zero ignored). The final fault-enabled run passes 96 checks across replay,
I/O, qualification, retained recovery and validation. All 12 public rebase journeys also pass, including a single
SIGINT during validation followed by a successful retry. A normal max-pure
0.1.9 build, with fault hooks disabled, also stopped its actual Brit-history
validation 0.032 seconds after the first SIGINT; HEAD, index and replay source
bytes stayed identical and no journal appeared. All six ordinary-build public
rebase journeys pass too. Scoped strict Clippy for core/all targets and the
CLI rebase suite passes on Rust 1.96.1, as do formatting and both Rust-style
gates. Registry audit finds the three release versions unused (core 0.63.3,
gitoxide 0.60.3, CLI 0.1.9). The two controlled fixtures each reuse one 256 KiB blob
under 128 names per revision. Eight revisions took 8.856349 seconds before and
0.010849 seconds after; 32 took 36.043735 seconds before and 0.039315 seconds
after. The verified closure remains exactly 17 and 65 objects respectively.
These are debug-build synthetic selection timings, not a monorepo latency or
whole-workflow budget. The type cache is private to one validation operation;
the next selection rechecks missing/corrupt content. Raw logs and binary hashes
are in the parent's `genesis/local-dev/brit-reliability-20261104/`.

Story-graph seam: between explicit upstream selection and durable journal
creation, a developer must be able to cancel the read-only object walk without
changing HEAD, index or private work. The public SIGINT journey names this
station; full closure, wrong-type and per-operation revalidation tests protect
its integrity boundary. No new shared authority or persistent cache is added.

### Independent review follow-up

Review of `eb63955d` accepted complete closure/type/checksum preservation and
per-operation cache scope, but required cancellation checks around preservation,
the initial journal write and retention publication. Those neighboring phases
are part of this repair; the initial local green did not establish them.

Preparation now polls preservation entries, worktree and object-directory walks,
and the durability scan. All original metadata reads precede journal allocation.
Cancellation before the initial state write checks allocation identity before
removing the empty directory; this coordinates cooperating writers and does not
isolate a hostile process that can mutate `.git` paths. After state writing
starts, the journal remains recoverable. Retention
verification checks cancellation before and after hashing, and each retention-ref
publication has its own check. A cancellation after one ref is written retains
that partial preparation for explicit recovery. Cancellation during recovery
stops subsequent publication while preserving progress already made. The tested
pre-publication checkpoint leaves journal bytes and ref count unchanged; a later
checkpoint can legitimately leave a new ref, and recovery can promote
`state.pending` before cancellation.

The public journeys now name six pre-journal interruption points and three
post-journal points, including another interrupt during `--abort`. They preserve
the original integration and private work. All 158 affected core checks, 13
fault-enabled rebase journeys and 40 ordinary pull/rebase/switch journeys pass,
with zero ignored. Scoped strict Clippy, formatting and both Rust-style gates
pass. Independent re-review of `a524bf36` accepted the three required code fixes
and conditioned LAND on these documentation corrections. Exact final CI remains
required before landing; this is technical review, not human acceptance.

- [ ] Bound the remaining cancellation frontier: individual large-object decompression/hash operations, merge-base traversal, whole-worktree witness serialization/hashing, journal serialization, allocation/deallocation and blocking filesystem calls. The 2026-11-12 selection/index loop slice proves only its controlled workloads within one second; it does not preempt library internals or establish a universal latency budget. Retain representative large-object and real-repository probes as subsequent work. Serves brit-native-daily-driver.
- [x] Implement bounded `brit worktree add` payload processing and retained-branch recovery under the hard 1 GiB scratch cap. Disposable loose/packed fixtures reproduce the allocation-limit failure in the buffered reader and succeed through bounded checkout. The original shared `governance-findings.jsonl` incident is not replayed. Probes: `worktree_bounded` CLI suite and `gix-odb` stream tests; see the 2026-11-12 bounded-checkout evidence above. Serves brit-native-daily-driver.
- [ ] Harden unpublished-journal cleanup against a hostile local process with write access inside `.git`: path substitution can race create/metadata or inode-check/remove, including ancestor symlinks, and cause removal of an empty directory not created by this operation. `remove_dir` refuses non-empty directories and does not follow a final-component symlink; that bounds the observed threat. Investigate private staging and handle-anchored ownership, with adversarial path-swap probes, as a separate slice. Serves brit-native-daily-driver.
- [ ] Reconcile retained-copy helper delivery with the followed-tool install: `brit-build-artifacts.yml` ships only brit/build-ref/verify, while `brit-release.yml` includes and qualifies `brit-rebase-retained`. On shem, `command -v brit-rebase-retained` currently fails. Abort/recovery itself runs in-process; independent retained-copy inspection/extraction still needs the separate helper. Probe the installed helper and export a retained copy before declaring that delivery station qualified. Serves brit-native-daily-driver.

## Sprint — 2026-10-12

Bounded local work under safe authoring and native gates. The implementation
tracks have separate owners; an independent reviewer checks the resulting diff
and disposable-repository evidence. Builds share the existing Cargo pool slot.

- [x] Execute pre-push with the transport's exact resolved create/update/delete refs and OIDs; hook rejection prevents all remote changes, and explicit bypass, dry-run and no-op behavior match Git for the tested refspecs. Probe: public executable pre_push journey against disposable bare remotes. Serves brit-native-daily-driver.
- [x] Preserve Git commit-message cleanup semantics for the supported noninteractive authoring flow, including configured cleanup and comment markers; compare committed message bytes with stock Git. Probe: public executable commit_cleanup journey. Serves brit-native-daily-driver.

These probes establish bounded local behavior. Installed delivery, interrupted
scoped authoring, exact-revision remote CI and two-developer qualification remain
separate acceptance stations; the habit stays unqualified until those are proven.

### Local evidence

Base: `7e4573528` plus this working diff. Two implementation agents worked on
separate files; a third reviewed both and independently rebuilt and tested the
combined public executable. Native builds used Rust 1.99.0, empty `RUSTFLAGS`,
two compiler workers and the existing external Cargo pool. The final executable
SHA-256 was `fff406ed3ddd38a6e2ad1937b93fe60db541323cea55363e47ae71a5a0941c3e`.

`BRIT_BIN=<pool>/debug/brit cargo test --locked -p cli-journey --test authoring
--test brit --test cli_surface --test commit_cleanup --test context --test pre_push
--test shared_tree` passed all 56 tests without skips: 19 authoring, 16 Brit,
2 CLI surface, 7 cleanup, 2 context, 8 pre-push and 2 shared-tree tests.
Both new suites are wired into the existing GitHub journey job. Local source
review and test proof do not stand in for exact-revision remote CI acceptance.
Scoped `cargo clippy --locked -p brit-cli -p gitoxide-core -p cli-journey
--all-targets -- -D warnings` passed after three mechanical lint corrections.
`cargo check --locked -p gitoxide --no-default-features --features small` and
the corresponding `lean-async` check both passed.
After lint fixes, the final binary passed the 34 affected authoring, pre-push and
cleanup tests again; the other 22 tests were unchanged. Edition-correct scoped
Rust formatting checks and `git diff --check` passed. Independent local technical
review approved the bounded changes in native flow record
`bafyreiayiftmay4lcbe2ufv7pwqwejtt5zdkm5a2n47nguklcovjz3vtqe`.
Source inspection used direct shell reads outside the private recall executor;
this sprint reconciled no memory edges.

The transport resolves once and exposes an additive before-send callback;
the existing `transmit` entry point delegates without changing its call signature.
This narrowly necessary gix extension avoids a second CLI refspec resolver:
the former API exposed resolved refs only after it had already sent them.

### Discovered constraints and remaining boundary

Chain: safe local authoring → guarded transport → native integration. The
intermediate assertion is that every update is resolved before hook input is
formed, and a rejecting hook prevents all updates. The `pre_push` probe now
passes locally, including 2,000 refs with a hook that closes stdin immediately.
The regression story is: given input larger than a pipe buffer, when the hook
closes its input, then Brit reaps it and follows its exit status; a nonzero status
leaves every remote ref unchanged. EPIPE alone is not hook rejection.

The cleanup oracle exposed a distinct byte-level constraint: Git's `sane_ctype`
whitespace excludes vertical tab and form feed. Given those bytes in a message,
when cleanup runs, the committed bytes must match stock Git. The retained test
also covers config alias ordering and hook-edited messages. Public options added
are `--cleanup`, `-F/--file` and `--allow-empty-message`; editor, fixup and squash
workflows were not exposed by this sprint. Automatic or empty comment prefixes
with `strip` cleanup explicitly refuse rather than risk removing message text.

Enabling hooks exposed inherited unsafe push behavior, so unsupported
`--force-with-lease` and `--force-if-includes` now refuse before connecting.
Without global `--force`, existing branch updates require proven local ancestry;
unknown ancestry and tag replacements refuse. `--no-verify` bypasses hooks only.
This conservatively rejects some Git-supported operations, including per-ref
force cases, and aborts the whole push before hooks on a safety refusal.
Full Git rejection-status filtering, HEAD/revision-expression resolution and
annotated-tag handling remain unqualified engine work. Stock Git remains the
bridge for those operations. No release, installation or parent pin promotion
is established by this local sprint.

## Qualification sprint — 2026-10-12

Objective: qualify one installed, two-developer scoped workflow with enforced
gates, interruption recovery and independently accepted integration. Explicit Git
bridges remain part of the supported workflow; exhaustive porcelain parity is a
separate milestone. The earlier hook/cleanup sprint is complete local work.

- [x] Prove scoped authoring in a separate linked worktree, preserving neighboring staged, unstaged and untracked bytes through hook rejection and interruption before and after ref publication; prove resume and explicit Git abort. Serves brit-native-daily-driver.
- [x] Make the installed native gate honor standalone repository authority without weakening the parent package/projection floor; removing governing files cannot opt out. Serves brit-native-daily-driver.
- [x] Integrate only the exact independently accepted source into its pinned target, after bounded approved gates; changed source/target/evidence or contrary evidence refuses without ref mutation. Serves brit-native-daily-driver.
- [ ] Qualify an installed Unix executable and atomic upgrade/rollback, including corrupt and interrupted installs; bind automated evidence to exact source and keep synthetic fixtures distinct from real delivery. Serves brit-native-daily-driver.
- [ ] Run the complete installed workflow with two actual developers and record their experiential acceptance through the existing native flow; agent fixtures alone cannot complete this station. Serves brit-native-daily-driver.

### Local-first and data-design decisions

Alone, the device can inspect, stage through an explicit Git bridge, commit,
recover, run local gates and retain its review candidate. Independent acceptance
waits only for the reviewer; once local records are present, integration needs no
doorway or network. Cryptographic key custody is not changed by this local Git
adapter: a hosted key does not prevent local work, and an unattributed visitor
cannot claim independent acceptance. The working version stays in its author's
Git worktree; accepted integration moves only an explicitly named unchecked-out
branch ref. Another device adds a repository copy; another developer adds review
and acceptance. They see the shared candidate and its evidence, not unrelated
dirty bytes. Their absence leaves local work and recovery available. No pool,
doorway or DHT service participates. CLI output distinguishes refusal, gate
failure and completed local integration; it makes no remote-publication claim.

P2P design gate: the new decision result is an Ephemeral (C) projection of
existing local CID-addressed acceptance records, pinned report bytes and Git
refs. It is reconstructed on each invocation; no new ledger, notarized entity,
entry type, coordinator, signal, HTTP route or recurring head is introduced.
Git OIDs and full ref names are external bridge identities, not EPR CIDs. Extra
source/target/gate bindings are attributes of the existing CID-pinned acceptance
report; existing native actor/appointment/review/fulfillment validation remains
authoritative. Network stages do not relax any check. At one year this creates
zero additional protocol heads and no quiesce cost. No transport affinity or
sync projection applies. The worst local holder can already read/modify their
Git repository and local sidecar; the mechanism makes no cryptographic peer
qualification claim and exports no cross-person activity store.

Integration uses the existing flow transaction lock and reconciliation, followed
by one Git ref transaction verifying the source ref and comparing/updating the
target. The existing native readiness evaluator and the exact gate argv approved
in the pinned report both must pass. Gate execution is time-bounded and never
holds the flow append lock; evidence is revalidated under that lock afterward.
Unknown integration fields, absent evidence, superseded acceptance, dirty source,
unproven fast-forward or a checked-out target refuse. No source checkout or
neighboring index is rewritten. Cooperative native appenders are serialized;
arbitrary external filesystem writers are outside the local lock's guarantee.
Concern-canon answers and contract tests belong in the existing epr-cli seam
registry, beside the Brit-facing adapter's source.

### Qualification evidence so far

Independent rebuild: all 60 public CLI tests passed (19 authoring, 16 Brit,
2 surface, 7 cleanup, 3 context, 3 recovery, 8 pre-push, 2 shared-tree), followed
by strict scoped Clippy. Tested development binary SHA-256:
`64d94e36554ebbffba32406f86066fd4f54c53a2af0a81e493be602c41da82bf`.
Installer fault fixtures passed; consumer tests passed 14/14 and readiness
qualification tests passed 12/12. These are local source and mechanism proofs,
not released-version upgrade or human acceptance.

The native evaluator's standalone governance tests passed 8/8 and integration
tests passed 6/6 on Rust 1.96.1. Independent review found and corrected stale
hook configuration after an approved gate and a flow-lock/reference-hook
deadlock. Integration refuses executable reference-transaction hooks; its
bounded Git transaction reports uncertain outcomes on timeout rather than
claiming rollback. A final race regression verifies that replacing the target
with a symbolic ref cannot redirect the write into another branch; global
`update-ref --no-deref` covers both transaction operations. Arbitrary background
descendants of successful commands remain outside complete process-tree
supervision. The full EPR workspace passed 1,194 tests with no failures or
ignored tests, strict workspace Clippy and formatting. The EPR changes are
committed separately as `7fd1c3a45` on `feat/brit-daily-driver-epr-2026-10-12`.

Clean source `a114c15d3e0578a78fc0af019d380e523f00a78c` produced an actual Linux
`max-pure` / `release-github` archive. Its checksum was verified, it was extracted
and installed into a private prefix, and the installed binary passed the
source-free operational probe and all 60 journeys. With installed epr it passed
all six integration tests, including a separately declared repository identity.
Candidate binary SHA-256:
`76e72d279781ce55551c216f50944c9bce2f8fe4115fa1cafe20b825f79f79ef`;
epr SHA-256:
`ac874f5ed678bb6c2b37e8b41bd53ded53db56d2667de816aa9872aebefa60c4`.
This candidate is not release-qualified: its exact-source remote CI exposed
helper placement, an inherited diagnostic snapshot and a macOS path-alias test
failure. Those findings require a corrected candidate and green CI. The first
optimized build took 17 minutes, including about eight minutes of final LTO;
future package runs should follow the complete inherited gate to avoid repeated
optimization while source corrections are still arriving.

No published Brit release was available on 2026-10-12 for a genuine
released-version A-to-B-to-A upgrade exercise. Synthetic installation fixtures
do not establish that qualification.

### Activating the bounded native reach gate

The Unix `.githooks/pre-push` adapter invokes installed `epr`; it contains no
copied governance evaluator. After committing the adapter, run `epr setup` and
`epr doctor` in each clone. Setup changes only local `core.hooksPath` and preserves
an existing explicit hook configuration. An incompatible or missing evaluator
must be upgraded before this gate can pass. The shared development checkout is
not activated automatically by the tests.

The adapter permits one named branch update whose source is the checked-out
HEAD. The advertised existing remote commit must exist locally and be an
ancestor of HEAD. Deletions, tags, detached or other source refs, multiple updates,
and unknown or non-ancestor baselines fail before remote mutation. A no-op with
no update tuples succeeds. First publication requires a reviewed, explicit
baseline, configured as a full commit object ID:

```sh
git config --local branch.<branch-name>.britReachBase <full-baseline-commit-oid>
```

That commit must exist locally, precede HEAD, and be its ancestor. No upstream
or remote-tracking baseline is inferred. This local declaration selects the
comparison; it does not manufacture an acceptance record. Once the remote
branch exists, its advertised commit replaces the configured first-push base.

The index, tracked files, and untracked files must be clean before and after
evaluation because native policy resolution also reads worktree authority.
Ignored private state is allowed. Refusal preserves the worktree: the hook never
stashes, resets, or cleans files. Source ref, HEAD, and any first-push baseline
are checked again after evaluation. Index `assume-unchanged` and `skip-worktree`
flags are refused because they can conceal tracked authority edits. These checks
detect observed changes; they
are not an atomic lock against every concurrent writer. Native governance
readiness does not replace the required GitHub `Tests pass` result before main.

`bash scripts/ci/test-brit-reach-hook.sh` proves adapter mechanics with an isolated
fake evaluator. Set `EPR_TEST_BIN=/absolute/path/to/epr` to additionally prove real
setup, doctor, readiness, and an invalid-governance push refusing without moving
the remote. CI runs the explicitly labeled fixture lane; the real installed
evaluator and human daily-driving qualification remain separate evidence.

### Final installed candidate and handoff

The clean Linux candidate is source `d6cb92865e707015108d197f8eed418119600921`,
with binary SHA-256
`48ebeceb12d223c1299d87267f3a0d7470a2c7df3ba898cf2b082c68ba80cddc`
and archive SHA-256
`a6c6da000413d33890485b8f22163927fb1dcae023868c142d15be11e8a0fa3a`.
The packaged, extracted and installed executable passed all 60 public journeys
and all six real installed-evaluator integration tests. Actual source candidates
A-to-B-to-A-to-B passed checksum, embedded-source and operational checks at every
activation. Both candidates are version 0.1.5; this proves candidate replacement
and rollback, not an upgrade between published versions.

The separate hook-only revision `eff40d6ab353687bd7e40178066d2baf150290e8`
passed 27 mechanics checks and an independent real-EPR probe. Its own push from
a clean linked worktree ran the activated hook and native readiness successfully,
evaluating all five changed paths. A final combined disposable-repository probe
used the installed candidate, this hook and the pinned EPR executable: valid
governance published the exact source, then invalid governance refused without
changing the remote. No bypass was used. The clone's `core.hooksPath` is now
`.githooks`; the original shared worktree's unrelated edits were preserved.

In this workspace, the private installation and durable proof live under
`genesis/local-dev/brit-readiness/` in the parent Elohim checkout. The default
workspace EPR executable has not been replaced. Select the tested binaries in
the current shell before operating the activated hook:

```sh
export PATH=/projects/elohim/genesis/local-dev/brit-readiness/release-install/bin:/projects/elohim/genesis/local-dev/brit-readiness/private-install/bin:$PATH
brit --version
epr doctor
```

Machine-readable artifact evidence is `release-candidate-final/candidate-evidence.json`;
the combined push proof is `installed-native-reach.log` within that evidence
directory's parent. Remote CI is tracked separately for the binary source and
hook revision. The binary source's [CI run 38259066507](https://github.com/ethosengine/brit/actions/runs/38259066507)
completed successfully with 29 successful jobs and publication skipped by its
branch condition; CodeQL and workflow-security checks also passed. The hook
revision's [CI run 38260354830](https://github.com/ethosengine/brit/actions/runs/38260354830)
also completed successfully with 29 successful jobs, publication skipped, and
both security workflows green. Exact-revision CI receipts are saved alongside
the artifact evidence as `ci-d6cb.json` and `ci-eff40.json`. The full
local inherited shell journey at that handoff failed on unchanged
pack hashes, while that original journey job passed remotely for the binary
source. The later acceptance sprint below resolves the compression cause. Expected hashes and assertions were not weakened.
Actual two-developer acceptance and a genuine published-version upgrade remain
open. The overall daily-driver habit remains unqualified.

## Installed acceptance sprint — 2026-10-12

This sprint closes delivery and handoff prerequisites. The overall habit remains
unqualified until the platform matrix, genuine released-version upgrade and two
actual developers supply their separate evidence.

- [x] Install independently pinned Brit and EPR binaries as one private generation; failed checks and interrupted activation preserve the previous pair, and overlapping installers retain rollback generations. Probe: `bash scripts/ci/test-install-brit-epr.sh`. Serves brit-native-daily-driver.
- [x] Prepare the existing installed-platform matrix for manual branch qualification without creating a tag, draft or release. Probe: `bash scripts/ci/test-brit-delivery-source.sh`; actual remote matrix remains pending. Serves brit-native-daily-driver.
- [x] Exercise the public native claim, fulfillment, independent appointment/review, pinned acceptance and integration commands with installed binaries in a disposable agent fixture; replay refuses. This proves handoff syntax, not human acceptance. Serves brit-native-daily-driver.
- [ ] Capture a green exact-source manual `brit release` workflow run and qualify `installed-unix`; Linux and both Mac architectures are required. Serves brit-native-daily-driver.
- [ ] Deliver the native EPR integration revision and final Brit evidence to their remotes, then bind the available artifacts to those revisions. Serves brit-native-daily-driver.
- [ ] Exercise two distinct published versions through `verify-brit-upgrade.sh`, preserving repository bytes through upgrade and rollback. Serves brit-native-daily-driver.
- [ ] Two actual developers each author and independently accept one scoped loop with the installed pair, recording failures, recoveries and experiential judgments. Serves brit-native-daily-driver.

### Installing the pair

Run the helper from a checkout containing this sprint, supplying a verified,
extracted Brit package and independently obtained source/hash pins:

```sh
bash scripts/ci/install-brit-epr.sh \
  "$BRIT_PACKAGE" "$BRIT_SOURCE" "$BRIT_SHA256" \
  "$EPR_BINARY" "$EPR_SOURCE" "$EPR_SHA256" "$PRIVATE_PREFIX"
PAIR_BIN=$(cd "$PRIVATE_PREFIX/current/bin" && pwd -P)
export PATH="$PAIR_BIN:$PATH"
brit --version
epr doctor
```

The helper copies both tools into a unique generation, verifies their bytes,
checks the required EPR command interfaces and atomically activates the pair.
Repeat with the previous pins to roll back. Resolving the generation once per
shell keeps Brit and its EPR child on that generation during an upgrade; reselect
the pair when ready to use the new version. It never modifies the user's default
PATH or installed commands. Existing generations are retained. An interrupted or
failed activation may leave an inactive generation, deliberately preserving any
possibly activated rollback candidate. Linux is exercised locally; macOS's rename
behavior still needs the real platform job. The pair receipt explicitly identifies
EPR's source as caller-declared: this EPR binary does not embed its commit ID, so
its checksum binds the supplied bytes without manufacturing build provenance.
Interface smoke alone does not establish workflow compatibility or acceptance.

The tested local pair is available at:

```sh
PAIR_BIN=$(cd /projects/elohim/genesis/local-dev/brit-readiness/acceptance-sprint/tool-pair/current/bin && pwd -P)
export PATH="$PAIR_BIN:$PATH"
```

It contains the prior final Brit artifact (`d6cb92865`, binary SHA-256
`48ebeceb12d223c1299d87267f3a0d7470a2c7df3ba898cf2b082c68ba80cddc`)
and EPR (`7fd1c3a4516e4b47888c70d0b70999052e73e204`, binary SHA-256
`ac874f5ed678bb6c2b37e8b41bd53ded53db56d2667de816aa9872aebefa60c4`).
The new installer does not imply either artifact was rebuilt from this sprint.
Local pair evidence lives in `acceptance-sprint/pair-{install,operational,reach,fault-fixtures}.log`
under the existing parent campaign directory. Source-free Brit operations and the
27 reach-hook mechanics plus real EPR governance/refusal probes pass.

### Qualifying a candidate without publication

After pushing this sprint to a branch, manually dispatch `.github/workflows/brit-release.yml`
against that exact branch. Manual dispatch is candidate-only: source validation
requires the branch commit to match HEAD; prepare and build have read-only contents
permissions; the release-writing job is skipped. The same four-target build and
Unix installed tests produce checksummed Actions artifacts named with source,
run attempt and target. Capture the completed exact run for the existing
`installed-unix` qualifier. Local workflow fixtures are preparation, not that
remote result. A `brit-v*` tag push retains the release path and publishes only
after all targets pass; do not push a release tag merely to obtain a measurement.

### Two-developer procedure

Start with a disposable clean clone containing the root governance files, recipes
and checked-in hook, and use the tested pair. Run `epr setup` then `epr doctor`
before the ceremony. An arbitrary empty `git init` repository lacks those inputs.
Repeat in the selected real repository only after its preceding readiness stations
are accepted.
Both people use the same candidate checkout and native sidecar with distinct
sessions for this first exercise. The procedure does not promise record transfer
between independent clones. Each person issues their own identity and acceptance
commands; an agent must not claim a human identity or fill in an experiential
verdict on their behalf. Run the loop twice with the roles reversed.

The author selects one eligible intent from installed context. Choose a fresh
session name and actual human handle, then claim it (do not supersede another
owner). Commands below use `jq` to read returned IDs, never invented CIDs:

```sh
mkdir -p .eprfs/status/daily-driver
epr flow project --root . --recipes .epr-meta/recipes.yaml
brit context docs/plans/2026-09-28-daily-driver.md --root . --json
epr actor claim --as "human:$AUTHOR" --session "$AUTHOR_SESSION" --json \
  > .eprfs/status/daily-driver/author.json
epr flow claim --on "$INTENT_CID" --session "$AUTHOR_SESSION" \
  --brief "$BRIEF_PATH" --json > .eprfs/status/daily-driver/claim.json
COMMITMENT=$(jq -er .commitment_cid .eprfs/status/daily-driver/claim.json)
```

In the disposable exercise each person uses scoped staging through stock Git,
commits with Brit, experiences a pre-push rejection without remote movement,
recovers an interrupted operation, and successfully pushes through the installed
native hook. Compare staged, unstaged and untracked neighboring bytes with stock
Git. Write the observations, exact commands, binary pins and limitations in a
UTF-8 author report under `.eprfs/status/daily-driver/`; failures remain evidence.
For the selected candidate, pin the checked-out source branch and an explicitly
named, unchecked-out target branch after all source changes are committed. Ensure
both the tracked tree and untracked tree are clean (private `.eprfs/` is ignored).
Then fulfill only the work actually completed:

```sh
SOURCE_REF=$(git symbolic-ref HEAD)
SOURCE_COMMIT=$(git rev-parse "$SOURCE_REF")
TARGET_COMMIT=$(git rev-parse "$TARGET_REF")
epr flow fulfill --on "$COMMITMENT" --report "$AUTHOR_REPORT" \
  --status DONE_WITH_CONCERNS --commit "$SOURCE_COMMIT" \
  --session "$AUTHOR_SESSION" --json > .eprfs/status/daily-driver/fulfillment.json
```

The reviewer registers their own distinct identity. The appointing person records
that reviewer for this commitment; the reviewer independently examines the exact
candidate and records a technical verdict. Use `changes-requested` if warranted:

```sh
epr actor claim --as "human:$REVIEWER" --session "$REVIEWER_SESSION" --json \
  > .eprfs/status/daily-driver/reviewer.json
REVIEWER_CLAIM=$(jq -er .recordCid .eprfs/status/daily-driver/reviewer.json)
epr flow note --on "$COMMITMENT" --kind ruling --appoint "$REVIEWER_CLAIM" \
  --reason "$APPOINTMENT_REASON" --session "$AUTHOR_SESSION" --json \
  > .eprfs/status/daily-driver/appointment.json
epr flow note --on "$COMMITMENT" --kind verdict --verdict approved \
  --reason "$TECHNICAL_REVIEW" --session "$REVIEWER_SESSION" --json \
  > .eprfs/status/daily-driver/review.json
```

The reviewer next actually uses the installed workflow and writes an acceptance
JSON report. Run mutating rejection/recovery exercises in disposable sibling
repositories, preserving the selected candidate HEAD, tree and pinned evidence.
If the candidate changes, repin it and repeat review before acceptance. Required fields are `intent`, `revision` (exact source commit),
`environment`, nonempty `actions` and `observations` arrays, an explicit
`limitations` array, and `evidence` entries with repository-relative `path` and
`cid`. One evidence entry must name the unchanged author report and the exact
`.evidence` CID returned by fulfillment. For integration, add:

```json
{
  "integration": {
    "sourceRef": "<full refs/heads/ source>",
    "sourceCommit": "<pinned source commit>",
    "targetRef": "<full refs/heads/ target>",
    "targetCommit": "<pinned target commit>",
    "gate": ["<approved executable>", "<argument>"],
    "gateTimeoutSeconds": 1800
  }
}
```

This is the integration portion, not a complete report. Choose the real owning
quality gate and its argv with the reviewer; do not use a no-op to pass the
ceremony. Keep all reports and pinned evidence inside the repository. Record
experiential approval only after the reviewer concludes the scoped assertion is
met, with limitations explicit:

```sh
APPOINTMENT=$(jq -er .record_cid .eprfs/status/daily-driver/appointment.json)
FULFILLMENT=$(jq -er .record_cid .eprfs/status/daily-driver/fulfillment.json)
REVIEW=$(jq -er .record_cid .eprfs/status/daily-driver/review.json)
epr flow note --on "$COMMITMENT" --kind verdict --verdict approved \
  --purpose acceptance --appointment "$APPOINTMENT" --fulfillment "$FULFILLMENT" \
  --review "$REVIEW" --report "$ACCEPTANCE_REPORT" --reason "$ACTUAL_JUDGMENT" \
  --session "$REVIEWER_SESSION" --json > .eprfs/status/daily-driver/accepted.json
ACCEPTANCE=$(jq -er .record_cid .eprfs/status/daily-driver/accepted.json)
brit integrate --acceptance "$ACCEPTANCE"
```

A moved source/target, changed evidence, failed gate or contrary finding must
refuse. Inspect the target afterwards; integration is local and does not push.
Preserve each loop's reports and records before starting the reverse-role loop:
use a different evidence directory and session names so old pins do not change.
The actor response uses `recordCid`; flow notes and fulfillments use `record_cid`.
These commands were exercised with two explicitly agent-only identities and the
installed binaries: integration succeeded, the tree stayed clean, and replay
refused. `acceptance-sprint/native-ceremony.log` records that syntax proof;
actual two-person qualification remains open.

### Pack snapshot finding resolved

The former unexplained local pack hash is a producer-compression difference.
Holding Git 2.52, the nine fixture objects and the installed Brit binary fixed,
system zlib `1.3.1.zlib-ng` produces pack `9380796d…` / index `c02d9…`;
standard zlib `1.3.2` produces the exact expected pack `24d2f055…` / index
`8e484…`. Git 2.54 reproduces both outcomes. Object order, headers and all nine
inflated objects agree; four compressed streams differ. Brit receives the valid
pack produced by Git in either case. All ten original `file://` receive assertions
pass with standard zlib and unchanged snapshots. No production or snapshot fix
was needed. Use a reference-compatible Git compression implementation for these
byte-sensitive fixtures; diagnostic library paths are not installation dependencies.

Story constraint: between installing the pinned executable and reproducing
byte-sensitive interoperability snapshots, declare the producer's compression
implementation. The probe is `acceptance-sprint/probe-pack-producer.sh` plus the
unchanged original assertions in `original-file-pack.log`; the detailed receipt is
`pack-producer-evidence.json` (SHA-256
`b2bcbc0ad27795dd4d813ae2658f52b1c88cd4c913ca9f387be07a1cec75c0b0`).
The complete inherited journey was attempted again but the current sandbox
refuses the Git daemon's local TCP listener with `EPERM`. This is a distinct
execution restriction, not a full-suite pass or a remaining pack defect.

The reviewed sprint is committed on local branch
`run-ci/daily-driver-acceptance-2026-10-12` in the separate checkout
`/projects/elohim/.worktrees/brit-acceptance-sprint`. Native setup/readiness passes
against `eff40d6ab`; the shared checkout and its unrelated changes were preserved.
GitHub API access and Git remote discovery currently fail DNS/network resolution,
so no new push, remote matrix or publication is claimed. The isolated EPR branch
`feat/brit-daily-driver-epr-2026-10-12` remains local as well.

## Native authoring sprint — 2026-10-12

Objective: perform the local stage → inspect → unstage → commit loop through the
public executable. This sprint requires no push or remote acceptance. Stock Git
is the byte/state oracle in disposable repositories.

- [x] Stage explicitly named literal files, including additions, modifications and deletions, without changing unrelated staged entries or worktree bytes. Validate the entire batch before publishing an index update. Serves brit-native-daily-driver.
- [x] Inspect HEAD-to-index (`diff --cached`, alias `--staged`) and index-to-worktree changes as readable patches without writing the index, worktree or object database; preserve existing tree/file comparison commands. Serves brit-native-daily-driver.
- [x] Restore explicitly selected index entries from HEAD with `restore --staged`, leaving every worktree byte unchanged; prove refusal and lock contention preserve the prior index. Serves brit-native-daily-driver.
- [x] Verify the combined public loop against stock Git with dirty neighbors, linked worktrees, deletions, symlinks and executable modes; independently review index publication and failure behavior. Serves brit-native-daily-driver.

Scope starts with explicit literal file paths, not recursive directories, wildcard
pathspecs or interactive hunk selection. Unsupported forms must refuse clearly.
Existing compatibility placeholder drivers are not implementations and will not
be exposed by changing parser wiring alone. The core operations reuse the native
Git object/index/filter/lock libraries; no subprocess Git fallback performs work.
The integration tests use stock Git only as their independent oracle.

Local-first reading: one device can stage, inspect, unstage and retain its source
bytes with no other person or service present. These unsigned local Git operations
do not depend on whether a protocol key is local, hosted or absent. The worktree
and index retain the author's working version; this sprint adds no shared draft
or publication. Other devices and collaborators receive only what the person
subsequently chooses to transfer through existing Git operations; their absence
does not change local behavior. Closing the process before index publication
leaves the prior index; successful publication is a Git lock-file transaction.
The UI shows a patch or explicit refusal, not remote acceptance or sync standing.

P2P design gate: no new protocol entity, verdict schema, route, coordinator,
notarized type, sync message or recurring head is introduced. Mutable Git index
entries and content-derived Git object IDs remain the existing external bridge's
source of truth; they are not new EPR CIDs, private-chain entries or DHT facts.
Diff output is an ephemeral local view of those bytes. There is no SQLite or
Automerge projection, transport affinity, DNA-hash change or additional head-plane
cost at one year. Existing local filesystem access bounds who can read the draft;
no aggregator, cross-person activity store or new identity claim is created.

The public local loop is:

```sh
brit add -- src/example.rs README.md
brit diff --cached
brit diff -- src/example.rs
brit restore --staged -- README.md
brit commit -m "Describe the staged change"
```

Paths are relative to the current repository directory and must follow `--`.
`diff --staged` aliases `diff --cached`; the existing `diff tree` and `diff file`
commands remain available. New untracked files appear in cached diff after staging.
Restore requires an existing HEAD; an unborn repository can stage and inspect its
first commit but cannot restore from a commit that does not exist.

This bounded implementation refuses wildcard/pathspec
magic, interactive staging, selected submodules, external clean filters, special
index flags. Staging accepts conflicted indexes, resolves only selected paths,
and preserves unselected stages and Git resolve-undo records. Diff and restore
retain their conflicted-index refusal. Mutations also refuse split/sparse indexes
and unsupported case aliases or Unicode precomposition. Diff refuses custom diff
attributes/external drivers and submodule patches; explicitly selecting ordinary
files works alongside unrelated submodules. Text patches use Myers with three
context lines, without rename detection or function-context headers. These are
explicit compatibility limits, not complete Git CLI parity.

Story constraint: between staging changed bytes and committing the resulting
tree, stale index TREE caches must be invalidated. The runnable station is
`staged_changes_invalidate_cached_tree_before_commit` in the native authoring
journey: prime a real TREE extension, stage new content through Brit, commit, and
compare the committed tree with stock Git. Between inspecting a draft and
continuing to edit, diff must preserve the raw index and object inventory; the
`patches_match_git_and_leave_index_and_objects_unchanged` station checks both
immediately around Brit, separately from Git's optional stat-cache refresh.

Local verification: all 17 native-authoring journeys and the existing 60 public
CLI journeys passed against the rebuilt executable. Strict Clippy for `brit-cli`,
`gitoxide-core` and `cli-journey` (all targets), workspace formatting, and the
census unit suite passed. Independent agents reviewed index publication,
read-only diff and parser/fixture integration. The new journey is selected by the
existing source CI and installed-release qualification jobs. Logs are retained
under `/projects/elohim/genesis/local-dev/brit-readiness/native-authoring/`.
The `small`, `lean-async` and `max-pure` frontend feature checks also pass, as do
all 16 CLI package tests with `RUST_BACKTRACE=0 RUST_LIB_BACKTRACE=0`; an existing
exact-error-output test otherwise includes the workspace's enabled backtrace.
This is Linux local evidence; it does not stand in for a remote platform matrix,
published upgrade or actual two-person acceptance. The owning habit remains
UNWIRED for its complete invariant.

## Ordered local daily-driver backlog — 2026-10-12

Stations 1–5b have completed bounded local proof; 5c has independently reviewed
design and stock-Git characterization only. Rebase implementation remains queued.
Refresh each handoff against its predecessor's actual result before work. All stations serve `brit-native-daily-driver`; its full
invariant remains unqualified. Station numbers live in checkbox labels because
the existing native recipe extracts bullet checkboxes.

- [x] 1. [Commit amendment and recovery proof](#handoff-1-commit-amendment). Serves brit-native-daily-driver.
- [x] 2. [Scoped worktree restore](#handoff-2-scoped-worktree-restore). Serves brit-native-daily-driver.
- [x] 3. [Broader staging](#handoff-3-broader-staging). Serves brit-native-daily-driver.
- [x] 4. [Explicit lease protection](#handoff-4-explicit-lease-protection). Serves brit-native-daily-driver.
- [x] 5a. [Safe branch switching](#handoff-5a-safe-branch-switching). Serves brit-native-daily-driver.
- [x] 5b. [Fast-forward pull](#handoff-5b-fast-forward-pull). Serves brit-native-daily-driver.
- [x] 5c. [Rebase/conflict recovery design](#handoff-5c-rebaseconflict-recovery-design). Design only; no rebase implementation or executable qualification. Serves brit-native-daily-driver.

### Handoff 1: Commit amendment

Prerequisite: native-authoring source `c6aa6b1d28a157d1ef6b3c099e8bc6d9ed6ff0a2`
and its 77 passing CLI journeys. Work in an isolated branch; no push or upgrade.
Public behavior: `commit --amend --no-edit`, `--amend -m MESSAGE`, and
`--amend -F FILE` (including stdin). Bare amend refuses without launching an
editor. `--no-edit` requires amend and conflicts with message inputs. Explicit
empty input never implicitly reuses HEAD. Existing cleanup, allow-empty-message
and bypass behavior applies; `--cleanup=verbatim` preserves reused message bytes.

Implementation boundary: compiled commit engine in
`gitoxide-core/src/repository/commit.rs`, native helpers in
`gitoxide-core/src/repository/authoring/mod.rs`, parser and dispatch in
`src/plumbing/options/mod.rs` and `src/plumbing/main.rs`. No Git subprocess
implementation. Preserve staged tree, all parents and original author/time;
use current committer. Support root, ordinary, merge, detached HEAD and linked
worktrees and match Git empty-commit rules, including removing the original change.
Refuse before hooks: non-UTF-8 messages, encoding headers or explicit
`i18n.commitEncoding`, extra headers/signatures, required signing, enabled notes
rewriting and active unsupported operations.

Retain pre-commit, prepare-commit-msg and commit-msg semantics. Run post-commit,
then post-rewrite with argument `amend` and exact `old-oid new-oid\n` input via
the existing hook-input helper, even if post-commit fails. Post-hook failures
warn and cannot imply rollback. One native transaction guards original symbolic
HEAD attachment and old commit while updating its pinned direct branch; detached
HEAD uses a guarded direct update. Symbolic branch chains refuse. Check interruption
after hooks and before publication; after publication report honestly. Preserve
user index/worktree bytes and intentional hook side effects.

Runnable acceptance: dedicated `cli-journey` amendment suite with stock Git as
oracle in disposable repositories; fixed-time object/reflog comparisons for root,
ordinary, merge and detached commits; message and staged-content amendments;
message sources, cleanup, explicit empty and verbatim reuse; hook order, args,
mapping, rejection, edited messages, staged changes, bypass and warnings;
unborn/metadata/policy/operation/option refusals; dirty neighbors, alternate
indexes, linked worktrees; locks, moved HEAD, same-OID branch switch and interrupts
both during a waiting hook and after publication. Wire CI/release selectors.
Run existing 77 journeys, CLI package tests, strict scoped Clippy, formatting and
small/lean-async/max-pure checks with one Cargo runner. Independent reviewer:
a separate agent after integration, focused on preservation, HEAD races,
cancellation and hook semantics. Evidence: durable logs and revision/binary pins
under `genesis/local-dev/brit-readiness/commit-amend/`, scoped local commits,
clean-source rebuilt executable, reviewer verdict, habit DELTA and `epr ready`
against sprint base. Published upgrade and human acceptance are excluded.

### Handoff 2: Scoped worktree restore

Prerequisite refreshed from station 1's completed receipt: clean source
`806084bf33b4c7670f90242ba4c0a5ea134b2346`, 95 passing journeys, independent
technical review and passing native readiness. Its executable SHA-256 is
`7d38079d13bdadde11cb04ccfe749d448cc3df3c1578557045b36d080956a35b`;
receipt: `/projects/elohim/genesis/local-dev/brit-readiness/commit-amend/verification.json`.
Station 2 starts from that commit on isolated branch
`sprint/worktree-restore-2026-10-12`; stations 3–5c remain queued.

Public behavior: literal selected files restored from index by
`restore --worktree --`, or HEAD by `--worktree --source=HEAD --`.
Preserve index bytes and unselected files; retain index-only `--staged`.
Resolve and validate the entire selection before writing. Support regular files,
executable modes, symlinks and tracked deletions; refuse unsupported index forms,
directories, pathspecs, submodules and external filters. Reuse native path,
object/filter and filesystem primitives; the porcelain stub is not an engine.
Use safe per-file replacement. Cancellation before publication changes nothing;
interruption after completed files names completed and remaining paths without
claiming rollback. Combined staged/worktree restoration remains deferred.

Acceptance: disposable stock-Git-oracle CLI journeys cover both sources, dirty
neighbors, deletions, modes, symlinks, linked worktrees, alternate indexes,
preflight refusals, write failures and interruption. Existing CI/release selectors
must include the journey. One Cargo runner reruns all 95 previous journeys,
package/parser tests, scoped strict Clippy, formatting and feature checks.
Separate implementation/test agents and independent preservation/recovery review
supply local technical evidence. Finish with scoped local commits, clean-source
rebuilt executable, durable receipts under
`/projects/elohim/genesis/local-dev/brit-readiness/worktree-restore/`, station
proof, habit DELTA, recipe projection and `epr ready` against the sprint base.
No push or publication; full habit remains UNWIRED.

### Handoff 3: Broader staging

Prerequisite refreshed from station 2: clean source
`f597a9365261bc80ab2793fd00c3da097bd4cdfe`, 109 passing public journeys,
independent review and passing native readiness. Clean rebuilt executable SHA-256:
`6e1b1c2a8c417a83c473f32e55052fd0ef24c2357e2ea34283123a1d2828c533`.
Receipt: `/projects/elohim/genesis/local-dev/brit-readiness/worktree-restore/verification.json`.
Station 3 starts from that source on isolated branch
`sprint/broader-staging-2026-10-12`; stations 4–5c remain queued.

Public behavior: `add -- <literal-files-or-directories>`, `add -u [-- <paths>]`
and `add -A [-- <paths>]`. Directories select additions, modifications and
deletions; `-u` selects tracked modifications/deletions. Either flag without
paths selects the entire repository, including when invoked from a subdirectory.
Reuse native traversal, ignores, filters and index primitives. Skip discovered
ignored untracked files; refuse explicitly selected ignored files, selected nested
repositories and submodules. Never traverse directory symlinks or nested repos.

Resolve and validate the complete selection under the existing index lock before
one atomic index publication. Preserve working files and unselected index entries;
invalidate affected index caches. Cancellation or failure before publication keeps
the original index; report publication honestly afterward. Interactive staging,
force-add, pathspec patterns and unsupported index/filter forms stay deferred.
Restore retains its literal-file boundary.

Acceptance: stock-Git-oracle journeys for selection scope, ignores, deletions,
file/directory transitions, symlinks, modes, nested repositories, dirty neighbors,
linked worktrees, alternate indexes, locks, failures and interruption. Separate
implementation/test agents, one Cargo runner, independent index/traversal review.
Wire source-CI/release selectors; rerun all 109 previous journeys, package/parser
tests, scoped strict Clippy, formatting and small/lean-async/max-pure checks.
Finish with scoped local commits, clean-source rebuilt executable, durable receipts
under `/projects/elohim/genesis/local-dev/brit-readiness/broader-staging/`,
station evidence, habit DELTA, recipe projection and native readiness against the
sprint base. No push or publication; full habit remains UNWIRED.

### Handoff 4: Explicit lease protection

Prerequisite refreshed from station 3: clean source
`6a945992b4016bdbb391efcf0139376d879ffc9f`, 124 passing public journeys,
independent review and passing native readiness. Clean rebuilt executable SHA-256:
`2f931512704f7f762ef33d71234a99623932b39c9ceb0890532bbcd3bfaf8997`.
Receipt: `/projects/elohim/genesis/local-dev/brit-readiness/broader-staging/verification.json`.
Station 4 starts from that source on isolated branch
`sprint/explicit-leases-2026-10-12`; stations 5a–5c remain queued.

Public behavior: `brit push --force-with-lease=refs/heads/<branch>:<full-expected-oid>
<remote> <source>:refs/heads/<branch>` for one explicitly selected existing remote
branch. Require a full branch ref and full hexadecimal expected OID; the expected
object need not exist locally. Reuse the existing resolved-update callback to
validate destination and advertised old OID before hooks or transmission, including
otherwise unchanged updates. A matching lease permits non-fast-forward replacement.
Retain the advertised old OID in the transport command so a later remote change
causes server rejection; no separate preflight lookup. `--no-verify` bypasses hooks
only, and `--force` cannot override a stale lease. Ordinary push remains unchanged.
Implicit leases, empty expectations/creation, deletion, tags, multiple destinations
and `--force-if-includes` remain deferred.

Acceptance: disposable stock-Git-oracle journeys for matching/stale leases,
missing local expected objects, destination mismatch, unsupported forms,
dry-run/no-op, hook rejection/bypass and remote movement during pre-push.
Refusal preserves remote state except the deliberate competing writer's change.
Separate implementation/test agents, one Cargo runner, independent transport review.
Wire source-CI/release selectors; rerun all 124 prior journeys, package/parser tests,
scoped strict Clippy, formatting and small/lean-async/max-pure feature checks.
Finish with scoped local commits, clean-source rebuilt executable, durable receipts
under `/projects/elohim/genesis/local-dev/brit-readiness/explicit-leases/`, station
evidence, habit DELTA, recipe projection and native readiness against the sprint base.
No push or publication; full habit remains UNWIRED.

### Handoff 5a: Safe branch switching

Refreshed prerequisite: station 4 source `e4d631d78eb8c563df61c501d05705e0dbe59594`,
134 passing journeys, independent approval, clean rebuilt executable and readiness.
Station 5a starts from that clean source on `sprint/safe-switch-2026-10-12`.
The retained station-4 executable also passed all 134 journeys in this fresh context.

Public behavior: switch an existing local branch (short or `refs/heads/` name),
or explicitly inspect/continue/abort a private interrupted operation. Preserve
unrelated staged, unstaged, untracked and ignored bytes; refuse overlaps and
overwrites, including ancestor/descendant conflicts. Reuse native worktree
occupancy and rebase/bisect reservations. Creation, reset, remote guessing,
previous-branch shorthand, detach, force, merge, tracking and submodule recursion
remain deferred. Unborn, unresolved, sparse, split and alternate indexes, external
filters and affected nested repositories/gitlinks refuse before mutation.

The user expanded this station to crash-durable recovery. A versioned private
journal holds exact index bytes, before/after file states and checksummed backups
in the owning worktree's private Git directory. Successful file and directory
synchronization is the durability boundary; storage that violates these primitives
is outside the claim. Before the durable commit decision, continue resumes and
abort restores; afterward recovery rolls forward only. Recovery never starts
automatically, never overwrites unexpected third states, and never removes an
unproven lock. Hook-start is durable; interrupted hook outcomes remain unknown
and are not replayed. Pending recovery blocks new Brit mutations while inspection
remains available; arbitrary external Git writers are detected, not isolated.

Design gate, before implementation: this journal is private external-Git bridge
state analogous to Private (B), not a protocol entity or source-chain record.
Identity is the owning Git/worktree location plus pinned Git OIDs; checksums only
detect damaged private bytes. No DHT type, coordinator, HTTP route, Automerge
projection, transport affinity or new recurring head applies (zero at one year).
It works offline independently of protocol-key custody. It confers no peer
authority and never exports backups into the object database or shared store.

Acceptance: independent checkout/recovery review, Git-oracle journeys, real
SIGINT/SIGKILL checkpoints and a separate loss-of-unsynchronized-writes model.
The engine and recovery probes passed before enabling the four public forms.
Finish with scoped local commits, clean rebuilt artifact, retained journeys,
parser/package/ref/recovery tests, strict lint, feature checks and readiness against
the exact base. Durable receipts:
`/projects/elohim/genesis/local-dev/brit-readiness/safe-switch/`.
Station 5a is locally complete: switching and durable recovery both pass, including
clean committed artifact qualification. See the verification evidence below.
No push, publication or parent pin change; full habit remains UNWIRED.

### Handoff 5b: Fast-forward pull

Prerequisite: station 5a source `1ffde6e22`, clean copied artifact, 149 ordinary
journeys, 25 switch/recovery journeys, independent approval and native readiness.
Public behavior: fetch and integrate explicitly
selected remote branch only with proven fast-forward; divergence preserves branch
and files. Boundary: native fetch and proven checkout/ref publication primitives.
Exclude merge/rebase pull. Acceptance: disposable local remote journey for fast
forward, divergence, dirty neighbors and recovery. Independent reviewer: separate
integration reviewer. Evidence: pinned binary/source, tests, review and DELTA.

Sprint scope (2026-10-12): start from clean station-5a evidence commit
`bff8b4f532146fde1ea40a70496548db8404a93c` on isolated branch
`sprint/fast-forward-pull-2026-10-12`. Expose only
`pull --ff-only <remote-name-or-url-or-path> <branch>` and explicit
`pull --status|--continue|--abort`. Include attached and detached HEAD; defer
unborn repositories, implicit upstreams, merge/rebase, autostash and arbitrary
refspecs. Preserve unrelated staged/unstaged edits and untracked/ignored files.
Completed fetch results survive integration refusal or abort. Fetch publication
and integration have separate durable decisions; recovery after durable fetch
completion works offline. Reuse private switch recovery and retain version-1
journal compatibility. Linux file/directory synchronization is the durability
boundary, not hardware power-cut qualification. The full habit stays UNWIRED;
no push, publication or parent submodule pin change belongs to this sprint.
Durable proof lives under the owning workspace's
`genesis/local-dev/brit-readiness/fast-forward-pull/`; completion requires the
retained 149 ordinary and 25 switch/recovery journeys, new pull journeys,
independent review and clean rebuilt artifact bindings.

The bounded command contract is:

```sh
brit pull --ff-only origin main
brit pull --ff-only /path/to/repository refs/heads/main
brit pull --status
brit pull --continue
brit pull --abort
```

A configured remote may update the selected branch's single mapping under
`refs/remotes/`; a URL/path creates no tracking ref. Automatic tags are disabled.
Equal or older remote tips leave HEAD, index and worktree unchanged and write
`ORIG_HEAD`, matching the characterized Git no-op. A successful fast-forward
runs `post-merge 0`; hook failure reports a warning without undoing the pull.
Recovery never replays a hook whose start was recorded but whose result is unknown.

Before the fetch decision, explicit continue may reconnect only to the pinned
remote target with unchanged URL/mapping configuration. After that decision,
continue or abort finishes the decided fetch metadata. Abort before the
integration decision restores integration resources while keeping completed
fetch results. After the integration decision, abort refuses and continue finishes
publication. A pending journal blocks other Brit mutations; read-only inspection
remains available. External Git is not journal-aware, so unexpected resources
refuse recovery and retain evidence. Legacy version-1 switch journals remain
recoverable. Private recovery data adds no protocol entity or recurring head;
durable-fetch recovery is offline and independent of protocol key custody.

Discovered stations (story harvest; final run state belongs to the sprint receipt):

- Chain 5b / between object lookup → durable object receipt / missing node:
  decompression alone does not prove that existing bytes match their requested
  Git object ID. Verify every reachable object's native content hash before the
  fetch decision. Probe:
  `corrupt_existing_loose_object_refuses_no_pack_pull_without_integration`.
- Chain 5b / between received objects → fetch publication / missing node:
  required objects are durable even when the fetch receives no new pack.
  Probe: `pull_power_loss_preserves_objects_before_both_durable_decisions`
  exercises actual sync events for pack and no-pack fetches with omission controls.
- Chain 5b / between interrupted receive → cleanup / missing node:
  a pre-existing pack keep is preserved and never claimed; a new keep carries
  creation identity before deletion is authorized. Probe:
  `a_foreign_keep_is_never_truncated_or_claimed` and the raw-receive signal cases.
  An unrecorded orphan keep is intentionally retained after an abrupt receive.
- Chain 5b / between completed fetch → integration publication / missing node:
  external Git changes to completed fetch metadata must refuse recovery without
  overwriting that third state. Probe:
  `foreign_completed_fetch_metadata_refuses_continue_and_abort`.

### Handoff 5c: Rebase/conflict recovery design

Prerequisite: station 5b implementation `5882e60b71f8d98d324fbf97d8aec78f4d6ef445`,
clean copied artifacts, 169 ordinary journeys, 32 pull and 25 switch/recovery
journeys, both durability models, independent approval and native readiness.
The evidence-only follow-up revision is bound in
`genesis/local-dev/brit-readiness/fast-forward-pull/verification.json` in the
owning workspace; start from its clean evidence revision when opening 5c.
Deliver a decision-complete design for sequencing,
conflict state, continue/abort and crash recovery. Boundary: existing native
sequencer capabilities assessed against Git oracle; no public dormant command.
Acceptance: runnable disposable Git characterization scripts and a complete
state/recovery matrix. Independent reviewer: separate recovery design reviewer.
Evidence: design, characterization results and explicit review before any exposure.

Completed design contract:
`docs/specs/2026-10-13-interactive-rebase-recovery-design.md` at
`31a2cf294f76af070e876aa32e10f7a036236792`. Independent review approves
preservation, stop mutation permissions, step/final separation, private retained
copies, ownership and truthful partial outcomes. Stock Git 2.52.0 passes 40
isolated characterization cases and the separate source-CI selector, with scoped
Clippy and formatting green. These observations do not qualify Brit rebase.

Ordered implementation follow-ons (the original daily-driver finish line stays
unchanged; each needs its own implementation and proof):

- [x] 5d. Conflict-aware authoring: separate original/public/replay indexes, scoped resolution staging, validated external Git staging and native amend/split child receipts. Probe: no preserved dirty byte enters any rewritten tree; original staged/unstaged distinction survives every stop. Depends on 5c. Serves brit-native-daily-driver.
- [x] 5e. Durable linear replay: pinned local selection, step/skip/final/abort decisions, retained objects, private abort copies and switch/pull journal compatibility. Probe: explicit recovery at every durable boundary, including abort after completed replay steps. Depends on 5d. Serves brit-native-daily-driver.
- [x] 5f. Interactive sequencing: validated todo, every selected command, editor/hook invocation outcomes and rewrite mappings. Probe: invalid/failed/interrupted editors and hooks never replay unknown effects or consume preserved edits. Depends on 5e. Serves brit-native-daily-driver.
- [x] 5g. Combined recovery and artifact qualification: complete the spec's fault matrix, retained-copy decoder/extraction/deletion, independent recovery review and exact-source rebuilt artifact journeys before public rebase exposure. Probe: interruption and synchronization-loss models, private-copy recovery and retained 5a/5b regressions. Depends on 5f. Serves brit-native-daily-driver.

Maintainer discoveries captured for those stations:

- Chain 5e / between a decided replay step → final branch publication / missing node: operation abort remains available until the separate final decision; probe `abort_after_each_completed_step_restores_exact_index_and_retains_rewritten_objects`. Current state: green in station 5e internal native proof; public exposure remains pending.
- Chain 5f / between invocation start → recovery mutation / missing node: durably recorded gated child identity and proved quiescence precede capture/restore; probes `unknown_sequence_editor_accepts_validated_saved_bytes_without_relaunch` and `unknown_edit_author_message_is_explicitly_accepted_without_second_editor`. Current state: green in internal native proof (`native-tests-2.log`); public exposure remains pending.
- Chain 5e / between detached replay → garbage collection / missing node: explicit H/U and intermediate commit retention precedes mutation; probe original/upstream/intermediate and stop-tree GC tests. Current state: green in station 5e internal native proof; full combined qualification remains 5g.
- Chain 5g / between retained-copy completion → active-journal retirement / missing node: standalone byte-preserving inspection/extraction works after operation cleanup; probe recovery into an empty destination plus explicit leaf deletion. Current state: green in station 5g retained-copy tests and exact committed public artifact journeys; full daily-driver habit remains UNWIRED.

### Amendment local-first and data boundary

Alone, a developer can amend staged bytes and retain the original author and
parents entirely on-device. Protocol key location (device, doorway or absent)
does not participate in unsigned local Git authoring. The working version is the
author's index/worktree; Git objects and refs retain committed state. No passing
state or shared draft is introduced. Another device or person adds a Git copy
and sees only deliberately transferred commits; removing it leaves local work
unchanged. No always-on host, doorway or pool is needed, and no step waits on one.
Closing during hooks preserves the prior ref unless the hook itself changes it;
after publication the new commit remains local and is recoverable from reflogs.
The CLI distinguishes refusal from a completed amendment with post-hook warnings.
Signing-required repositories refuse because this bounded path cannot sign.

P2P design gate: there is no new protocol entity or authority predicate. Git
objects retain their external content-derived OIDs, refs their native names,
and the Git object/ref stores remain the source of truth for this bridge.
Diagnostic output is Ephemeral (C); DHT classification and entry-type lookup,
integrity/coordinator zomes, signals, HTTP routes, SQLite/Automerge projections,
transport affinity and reach tiers do not apply. Protocol head count at seed and
one year is unchanged (zero added), with no quiesce cost or DNA hash change.
A local filesystem holder can already read/change this repository; this adds no
aggregator, cross-person ledger or identity correlation. Existing native ref
locking bounds publication; no parallel governance authority is introduced.

### Amendment recovery constraints captured by the journeys

Chain: prepared amendment → guarded local publication → recovery evidence.
Between preparation and publication, acquiring a ref lock is a waiting step:
an interrupt while waiting must be checked again after all locks are held.
The `interruption_while_waiting_for_ref_lock_prevents_publication` journey is
that station's probe. A branch switch to the same OID while waiting must also
refuse; `attachment_cas_is_checked_after_waiting_for_the_branch_lock` checks
the attachment guard separately from the earlier hook-time HEAD reread.

Between a successful amendment and reflog recovery, unchanged object bytes
still carry an action. The Git oracle shows an identical-OID amendment appends
HEAD's reflog only when HEAD is attached; it appends neither the direct branch
log nor a detached HEAD log. `identical_oid_success_records_the_same_reflog_as_git`
compares entire logs. Native plumbing now has an opt-in named-ref log policy,
with existing callers unchanged and all amendment locks retained through commit.
An identical OID is not proof of publication after an error; the lock-refusal
journey verifies that no post-hook or success claim follows a failed prepare.

The native ref transaction can fail after the direct branch update but before
HEAD's reflog completes. Brit reports a witnessed published OID as published
with a warning and continues post hooks; an unobservable outcome remains
uncertain and calls for inspecting refs/reflogs. It does not promise atomic
multi-file reflogs or roll back intentional hook effects. The interrupt flag
is checked immediately before entering commit; it cannot undo a publication
already in progress, and post-publication cancellation is explicitly reported.


### Amendment local verification — 2026-10-12

Station 1 has bounded local technical proof: the original 77 CLI journeys and
18 amendment journeys pass against the rebuilt public executable. The full
native ref suite passes 207 tests with two pre-existing ignored cases; both
new transaction regressions pass. All 16 Brit package tests and all eight
Gitoxide CLI library tests pass. Strict all-target Clippy covers `brit-cli`,
`gitoxide`, `gitoxide-core`, `cli-journey` and local `gix-ref@0.69.0`.
Workspace formatting and `small`, `lean-async`, `max-pure` checks pass.
The amendment journey is in the existing source CI and installed-release selectors.

Independent agent review examined preservation, attachment/OID guards, lock-wait
cancellation, hook delivery and partial publication. Its findings became retained
regressions. This is agent technical review, not two-human acceptance. The source
base is `c6aa6b1d28a157d1ef6b3c099e8bc6d9ed6ff0a2`; the isolated branch is
`sprint/commit-amend-2026-10-12`. Durable commands, logs, reviewer verdict,
clean-source executable and exact revision/hash receipt live under
`/projects/elohim/genesis/local-dev/brit-readiness/commit-amend/`.
The external `verification.json` pins the final committed source without a
self-referential commit hash in this document. Native readiness is rehearsed
against the named sprint base after commit; its receipt is `epr-ready.log`.

The reflog-failure journey uses a nonempty directory obstruction: the native
writer deliberately recovers from an empty directory, so an empty obstruction
would not test failure. Both the obstructing user's bytes and unrelated draft
bytes survive. Failure before publication refuses; failure after the pinned
branch moves reports the published commit and still delivers post hooks.

Evidence is local Linux x86_64 with Rust 1.99.0 and Git 2.52.0, empty `RUSTFLAGS`,
two compiler workers and the existing disposable external target. No push,
remote platform matrix, published upgrade or human acceptance is claimed.
The full `brit-native-daily-driver` habit remains UNWIRED. At the amendment
handoff stations 2–5c were queued; station 2's subsequent evidence follows below.


### Scoped restore boundary and recovery stations

Restoration is an on-device Git bridge operation. The selected source is a
snapshot of the current index or HEAD; no protocol identity, DHT entity, shared
pool, network route or additional protocol head is introduced. Another person
or device sees only deliberately transferred Git commits. The author's index
and unselected working files remain outside this operation's write set.
The existing Git object/filter and filesystem primitives own conversion and
publication; combined staged/worktree restoration remains deferred.

Chain: selected source → complete preflight → per-file publication → recovery.
Between source selection and publication, attribute selection is a distinct
assertion: restoring `.gitattributes` with a file must convert that file as Git
would after the selected attributes are restored, without writing either file
before full preflight succeeds. Unselected dirty attributes and index fallback
remain separate inputs. The Git-oracle attribute journey measures this boundary.

Between preparation and safe publication, permissions must respect the process
umask and cancellation must discard temporary files. Between partial publication
and recovery, the command must name completed and remaining paths; it cannot
promise rollback across multiple files. The real-signal journey stops the process
while preparing a 128 MiB temporary file, then tests cancellation before any
publication and after a smaller selected file has completed. The I/O-failure
journey installs a nonempty destination obstruction during that same boundary
and checks that the other writer's bytes survive. These are executable recovery
stations, not a claim of power-loss durability or isolation from hostile
concurrent directory renames. Cleanup may remove only empty directories actually
created by this invocation.


### Scoped restore local verification — 2026-10-12

Station 2 has bounded local technical proof. All 95 prior public journeys and
14 new worktree-restore journeys pass (109 total, none ignored). The attribute
journey includes 25 stock-Git cases distinguishing selected source attributes,
unselected dirty attributes, absent attributes and index fallback under both
sources. The suite also proves unchanged index bytes, dirty neighbors, linked
worktrees, alternate indexes, tracked deletions, executable modes, native and
configured emulated symlinks, restrictive umask, special-index/filter refusals,
preflight rejection and real-signal cancellation before and after publication.
I/O failures report completed and remaining paths and preserve the obstruction.
The existing source-CI and installed-release selectors now include this suite;
two earlier refusal tests now exercise the still-deferred combined mode.

All 16 Brit package tests, eight Gitoxide CLI library tests, strict scoped
all-target Clippy, workspace formatting, and `small`, `lean-async`, `max-pure`
feature checks pass. Implementation and tests had separate agents; one Cargo
runner performed verification. Independent preservation/recovery review checked
whole-selection preflight, source/attribute resolution, private object-memory
writes, index preservation, symlink traversal, umask, temporary-file ownership,
and truthful partial completion. The final clean-source artifact and rerun bind
the late PersistError cleanup correction as well: release the temporary file
before removing newly created empty ancestors.

Source base: `806084bf33b4c7670f90242ba4c0a5ea134b2346`; implementation commit:
`042a9b6ce`. Isolated branch: `sprint/worktree-restore-2026-10-12`.
Durable commands, complete logs, independent review, clean-source executable,
revision/hash receipt and native readiness rehearsal live under
`/projects/elohim/genesis/local-dev/brit-readiness/worktree-restore/`.
`verification.json` binds the final committed source and executable without a
self-referential revision in this document. `finish-verify.sh` rebuilds from a
clean source, reruns all 109 journeys against the copied executable, and runs
`epr ready --target 806084bf33b4c7670f90242ba4c0a5ea134b2346`.
The existing `.epr-meta/recipes.yaml` remains the recipe projection authority.

Evidence is local Linux x86_64, Rust 1.99.0, Git 2.52.0, empty `RUSTFLAGS` and
two compiler workers sharing the existing external Cargo target. Berth access
returned its documented exit-4 unleased fallback because the store was read-only;
no second Cargo runner was used. Direct shell reads supplied source and prior
receipt context; no memory curation or remote acceptance occurred. No push,
publication, remote platform qualification or human acceptance is claimed.
The full `brit-native-daily-driver` habit remains UNWIRED. Stations 3–5c remain
queued at that handoff; station 3's subsequent evidence follows below.


### Broader staging boundary and recovery stations

Staging remains an on-device Git bridge operation over the existing index and
Git content-addressed object store. No protocol entity, HTTP route, DHT head,
shared pool or additional authority is introduced. A separate person or device
sees only deliberately transferred commits. Working files remain untouched.

Chain: literal scope → native traversal → complete preparation → index publication.
Between scope and traversal, a tracked file or directory may have become a nested
repository. The walker's tracked-directory fast path and disk/index kind mismatch
must not authorize reading through that boundary. The nested-conversion journey
checks both transitions and original-index preservation. Discovered untracked
nested repositories are skipped; explicitly selected repositories or descendants,
selected gitlinks, and tracked paths converted to repositories refuse.

Between preparation and publication, all candidates share the index lock and
unsupported inputs or object-write failure must leave the original index bytes
unchanged. Real signal interruption after lock acquisition exercises that same
boundary. The resulting index is published by one native lock-file rename;
there is no multi-file worktree publication or rollback claim. Object preparation
can leave unreachable blobs on refusal, as Git's object store permits. Success
means index publication completed; cancellation cannot undo that publication.
Restore keeps its prior explicit literal-file boundary.

The Git oracle also distinguishes a repository-wide tracked-only selection from
an explicit unmatched scope: `add -u` can select nothing, but `add -u --` naming
an untracked file or empty directory refuses. The retained mixed-batch oracle
requires the same unchanged index even when another selected path is valid.
Directory syntax remains meaningful before normalization: a trailing slash or
`/.` on a file or symlink refuses rather than silently staging its normalized name.


### Broader staging local verification — 2026-10-12

Station 3 has bounded local technical proof. All 109 prior public journeys and
15 broader-staging journeys pass (124 total, none ignored). The new suite covers
literal directories, repository-wide flags from subdirectories, tracked-only
scope, ignored discovery and explicit refusal, deletions and file/directory
transitions, symlinks, modes, nested boundaries, dirty neighbors, linked worktrees,
alternate indexes, lock contention, object-write failure and real interruption.
The interruption fixture uses a 256 MiB file and observes the owned index lock,
stops the process, checks the unchanged original index, then delivers SIGINT.
Old refusal assertions now test still-deferred options; restore remains bounded.

All 17 Brit package tests, eight Gitoxide CLI library tests, strict scoped
all-target Clippy, workspace formatting, and small/lean-async/max-pure checks
pass. The source-CI and installed-release selectors include the new journey.
Separate agents owned implementation and tests; one Cargo runner performed checks.
Independent index/traversal review found no remaining blocking source issues.
Its findings became regressions for tracked paths becoming nested repositories,
ignored nested replacements, directory syntax before normalization, and unmatched
tracked-only scopes. This is agent technical review, not human acceptance.

Source base: `f597a9365261bc80ab2793fd00c3da097bd4cdfe`; implementation commit:
`15a2b34ca`. Isolated branch: `sprint/broader-staging-2026-10-12`.
Durable commands, full logs, independent review, clean-source executable and
revision/hash receipt live under
`/projects/elohim/genesis/local-dev/brit-readiness/broader-staging/`.
`verification.json` pins the final source and artifact outside this document;
`finish-verify.sh` rebuilds clean committed source, runs all 124 journeys against
the copied executable and rehearses `epr ready` against the sprint base.
The existing `.epr-meta/recipes.yaml` remains the recipe projection authority.

Evidence is local Linux x86_64, Rust 1.99.0 and Git 2.52.0, empty RUSTFLAGS,
two compiler workers and the existing external Cargo target. Berth claims used
the documented exit-4 unleased fallback. Source inspection used direct shell
reads outside the private recall executor; no memory edges were reconciled.
No push, publication, remote platform matrix, power-loss durability or hostile
concurrent-directory-rename isolation is claimed. The full habit remains UNWIRED.
Next: station 4 explicit leases, then safe branch switching, fast-forward pull
and rebase/conflict recovery design, one locally verified station per fresh context.


### Explicit lease boundary and transport stations

Lease expectations are on-device Git bridge inputs over existing ref names and
Git OIDs. They create no protocol entity, shared ledger, HTTP route or recurring
head. The receiver's advertised ref is the comparison source; a remote-tracking
ref or a separate lookup cannot authorize the push. A matching expectation permits
replacement without requiring the old object in the local object store.

Chain: explicit scope → advertisement authorization → hook → receiver transaction.
Between authorization and receiver transaction, the advertised old OID must remain
in the transport command even if the hook moves the remote. The competing-writer
journey compares stock Git and requires a receiver CAS refusal with only the
competing writer's new state remaining. Hook bypass removes only the hook step.
Between advertisement and authorization, unchanged updates still carry a lease
assertion. Brit checks it before invoking any hook, even when no ref needs moving.
The Git 2.52.0 oracle permits stale no-ops and lets global `--force` override a
stale lease. Brit deliberately refuses both under this station's explicit safety
contract; the journeys retain these differences rather than claim parity.

Between authorization and pack creation, an advertised old object absent locally
cannot seed the local exclusion walk. Pack traversal omits only that unavailable
exclusion; the transport retains its original old OID. The absent-object journey
uses several unsent local commits and checks the remote object graph after receipt.
This distinction preserves both complete object delivery and receiver race safety.


### Explicit leases local verification — 2026-10-12

Station 4 has bounded local technical proof. All 124 retained public journeys
and 10 explicit-lease journeys pass (134 total, none ignored). The new suite
covers matching non-fast-forward replacement, stale expectations under force,
hook bypass and dry-run, expected objects absent locally, complete multi-commit
pack delivery, destination mismatch, unsupported forms, matching/stale no-ops,
hook rejection and competing remote movement during the pre-push hook. Refusal
preserves every remote ref except the deliberate competing writer's change.
The receiver's CAS diagnostic is asserted separately from the final ref state.

All 17 Brit package tests, eight Gitoxide CLI library tests, two feature-enabled
lease unit tests, strict scoped all-target Clippy, workspace formatting and
small/lean-async/max-pure checks pass. The two unit tests require
`cargo test --locked -p gitoxide-core --features blocking-client --lib explicit_lease`;
the default package feature set excludes the push module and proves none of them.
The first actual unit run caught two malformed 42-character fixture IDs; corrected
40-character fixtures pass without production changes. An inherited pre-push
assertion now expects the explicit-lease syntax diagnostic for implicit leases,
retaining its refusal and unchanged-remote checks. Source CI and both release
qualification selectors include the new journey.

Separate agents owned implementation and journeys; one Cargo runner performed
verification. Independent transport review checked advertisement binding, no-op
validation, hook ordering, missing-object pack traversal and unchanged receiver
compare-and-swap input. Review and oracle corrections preserved the stricter Brit
contract rather than weakening it to stock Git's force and no-op behavior.
This is agent technical review, not human acceptance.

Source base: `6a945992b4016bdbb391efcf0139376d879ffc9f`; implementation commit:
`4701b8f4a`. Isolated branch: `sprint/explicit-leases-2026-10-12`.
Durable commands, full logs, independent review, clean-source executable and
revision/hash receipt live under
`/projects/elohim/genesis/local-dev/brit-readiness/explicit-leases/`.
`verification.json` pins the final committed source and artifact outside this
document. `finish-verify.sh` rebuilds clean committed source, reruns all 134
journeys against the copied executable, and runs `epr ready --target
6a945992b4016bdbb391efcf0139376d879ffc9f`. The existing
`.epr-meta/recipes.yaml` remains the recipe projection authority.

Evidence is local Linux x86_64, Rust 1.99.0 and Git 2.52.0, empty RUSTFLAGS,
two compiler workers and the existing external Cargo target. Berth claims used
the documented exit-4 unleased fallback. Direct shell reads supplied source and
prior receipt context outside the private recall executor; no memory curation
occurred. No push, publication, remote platform qualification, parent pin change
or human acceptance is claimed. The full habit remains UNWIRED.
Next: safe branch switching → fast-forward pull → rebase/conflict recovery design,
one locally verified station per fresh context.

### Safe switch recovery constraints — 2026-10-12

Station 5a exposes only `switch <existing-local-branch>`, `switch --status`,
`switch --continue` and `switch --abort`. The Linux engine supports ordinary
files, symlinks, executable modes and built-in conversions. Other platforms
refuse switching before mutation because this implementation has not established
their synchronization and crash-released locking contract. They retain existing
authoring behavior and refuse a pending journal brought from Linux.
Cross-filesystem publication refuses during preflight; journal-backed atomic
replacement requires the private Git directory and affected paths on one filesystem.

The private Git-directory journal records exact before/after bytes, index and
HEAD/reflog states, branch pins, path bytes, filesystem identities and owned
lock inodes. Its checksums detect corruption; they are not protocol addresses.
Preparation and resource intent are synchronized before live mutation. Files
are synchronized before the commit decision; the decision precedes index, HEAD
and reflog publication. Recovery can abort only before that decision and never
overwrites an unexpected third state. Pending state fences cooperating Brit
mutations throughout the shared repository, including explicit output paths.
External Git remains outside this serialization boundary.

Review discovered three missing stations between preparation and completion:

- Chain 5a / between preparation → ordinary lock acquisition / missing node:
  durable ownership intent must precede each initial or recovered lock link.
  Probe: real SIGKILL at `lock-intent` and `lock-acquired`, then explicit recovery. Current state: locally green.
- Chain 5a / between desired file bytes → commit decision / missing node:
  matching bytes after interruption must still be synchronized before deciding.
  Probe: the actual-engine synchronization trace loses every unsynchronized byte
  and namespace change at each boundary; negative controls must fail.
  Current state: locally green.
- Chain 5a / between metadata publication → hook / missing node:
  release ordinary Git guards durably while retaining the private journal fence.
  Probe: `post-checkout` successfully stages its changes through Git; interrupted
  hooks preserve those changes and are never automatically replayed.
  Current state: locally green.

The Git oracle also established that a hook exiting 7 makes `git switch` exit 1.
Brit preserves that failure status and the committed switch. A durable hook-start
without a recorded outcome reports uncertainty. Directory transitions preserve
original directory identity/modes through private renames; affected nested
repositories and gitlinks remain outside this station's supported boundary.


### Safe switch local verification — 2026-10-12

Station 5a has bounded local technical proof including crash-durable explicit
recovery. All 134 retained journeys, 14 normal switch journeys and one inherited
Git-environment isolation test pass (149 total). A separately built fault-test
executable passes 25 switch journeys: the same 14 ordinary cases plus 11 recovery
cases, including real SIGINT/SIGKILL at deterministic checkpoints, interrupted
abort, lock ownership windows, corrupt journals, foreign resource changes and
running hooks. These are test counts, not a count of individual fault cases.

The separate durability model executes one test across four actual filesystem
shapes (regular file, symlink, file-to-directory and directory-to-file). It models
loss of unsynchronized bytes and namespace changes at every engine sync boundary
and checks negative controls. This is not hardware power-cut qualification.

All 18 Brit package tests, eight Gitoxide CLI parser tests, 207 native ref tests
(two inherited ignored), two feature-enabled lease tests, strict scoped all-target
Clippy with and without fault features, workspace formatting and
small/lean-async/max-pure checks pass. Source CI, release-source qualification and
installed-release selectors include the applicable switch tests; Linux fault tests
use an explicit test-only feature. Unsupported-platform refusal has its own test;
no remote or non-Linux execution is claimed here.

Separate implementation and journey agents, one Cargo runner and an independent
checkout/recovery reviewer completed this station. Review approved preservation,
journal privacy, synchronization ordering, retry idempotence, lock ownership,
resource conflicts and truthful partial outcomes. The clean-source copied normal
artifact reran all 149 journeys, and its separate fault artifact reran all 25
switch/recovery journeys. The full habit remains UNWIRED.

Source base: `e4d631d78eb8c563df61c501d05705e0dbe59594`; implementation source:
`1ffde6e22a73554d1d98b50648df5eb9520a7086`; isolated branch:
`sprint/safe-switch-2026-10-12`. Normal executable SHA-256:
`601c776c41f2f1c72adc6e0501b6c928fa361c6ac8bf784049569590c7128109`.
Fault executable SHA-256:
`8186f25620210b3464f41e0e954ea773087413bb60b1fe934107f62111136fca`.
The evidence-only follow-up commit does not change executable source.

Durable commands, logs, fault matrix, independent review, copied executables and
revision/hash bindings live under
`/projects/elohim/genesis/local-dev/brit-readiness/safe-switch/`.
`verify.sh` describes source verification; `finish-verify.sh` requires clean
committed source before rebuilding and copying both artifacts. `verification.json`
binds the source, artifacts and final evidence revision. Native readiness passed
against the exact base; the final evidence revision is rehearsed with the same
`epr ready --target e4d631d78eb8c563df61c501d05705e0dbe59594` command. The existing
`.epr-meta/recipes.yaml` remains the recipe projection authority.

Evidence is local Linux x86_64, Rust 1.99.0 and Git 2.52.0, empty RUSTFLAGS,
two Cargo workers and the existing external target. Berth used its documented
exit-4 unleased fallback. Direct source and prior-receipt reads occurred outside
the private recall executor; no memory curation occurred. No push, publication,
parent-submodule pin change, remote qualification or human acceptance is claimed.
Next queued: 5b fast-forward pull, then 5c rebase/conflict recovery design.

### Fast-forward pull local verification — 2026-10-13

Station 5b has bounded local proof for explicitly selected fast-forward pull,
including durable fetch publication and explicit integration recovery. The clean
copied normal executable passes all 149 retained journeys and 20 new pull
journeys (169 total, none ignored). The separate fault executable passes 32 pull
journeys and all 25 retained switch/recovery journeys (57 total, none ignored).
The pull count includes the same 20 ordinary cases plus 12 recovery cases;
its main signal matrix exercises 17 checkpoints with both real SIGINT and SIGKILL
(34 boundary/signal cases). Other cases cover offline continuation, interrupted
abort, external metadata changes, stale/foreign locks, corrupt evidence, hook
interruption and legacy version-1 switch recovery.

Two durability model tests execute and pass. The retained switch model covers
four filesystem shapes. The pull model covers pack and no-pack receives, object
durability before the fetch decision, completed fetch metadata before integration,
and metadata before hooks/cleanup, with omitted data/directory-sync negative
controls. These are synchronization-loss models, not hardware power-cut tests.

All 19 Brit package tests, eight Gitoxide parser tests, 158 native pack tests,
207 native ref tests (two inherited ignored), two feature-enabled lease tests,
strict scoped all-target Clippy with and without fault features, formatting and
small/lean-async/max-pure checks pass. Source CI and installed-release selectors
include pull; the model command explicitly enables the features needed to run
both tests. No remote CI or non-Linux execution is claimed.

Independent review approved preservation, private journal identity/permissions,
object verification, both durable decisions, retry idempotence, ref/reflog
publication, lock/keep ownership, recovery conflicts and truthful hook outcomes.
Native receive now preserves existing pack keeps; newly created keeps carry
creation identity. A crash before that identity can enter the private journal may
leave an unclaimed keep, which recovery deliberately retains. Object durability
currently synchronizes and fingerprints the object store conservatively;
unrelated external repacking/removal may therefore refuse recovery. External
Git writers are detected through expected-state checks, not isolated.

Source base: `bff8b4f532146fde1ea40a70496548db8404a93c`; implementation source:
`5882e60b71f8d98d324fbf97d8aec78f4d6ef445`; isolated branch:
`sprint/fast-forward-pull-2026-10-12`. Normal executable SHA-256:
`b35866d083446ba308e0a3bf5185eacce2af4f2098064de9e0c0e89c9e0bb89d`.
Fault executable SHA-256:
`718c54fcc853877996467cc665e421603a45674f9f024d3c73e0607441c576e8`.
Both report the implementation revision; the rebuild script requires a clean
worktree before and after copying and testing them. This evidence-only follow-up
does not change executable source.

Commands, logs, fault matrix, independent review, copied executables and exact
revision/hash bindings live under
`/projects/elohim/genesis/local-dev/brit-readiness/fast-forward-pull/`.
`verification.json` binds the final evidence revision and distinguishes the
superseding successful checks from retained iteration logs. The existing
`.epr-meta/recipes.yaml` owns projection. Native readiness passed against the
exact base and is rerun on the evidence revision with
`epr ready --target bff8b4f532146fde1ea40a70496548db8404a93c`.

Evidence is local Linux x86_64, Rust 1.99.0 and Git 2.52.0, empty RUSTFLAGS,
two Cargo workers and the existing external target. Berth reported its existing
unleased fallback because its store was read-only. Direct source and receipt
reads occurred outside private recall; no memory curation occurred. No push,
publication, parent-submodule pin change or human acceptance is claimed.
The full habit remains UNWIRED. Next queued: 5c rebase/conflict recovery design.

## Sprint 5c evidence — 2026-10-13

Design station only is complete on branch
`sprint/rebase-recovery-design-2026-10-13`, starting from clean 5b evidence
`7ffdf6b0ad5f63e723df50c569bd78010a38c0cb`. Design/test commit:
`31a2cf294f76af070e876aa32e10f7a036236792`. No production rebase, authoring or
recovery code changed. Rebase stays unimplemented and unexposed.

The implementation-ready contract is
`docs/specs/2026-10-13-interactive-rebase-recovery-design.md`. It defines
preservation of staged/unstaged neighbors, isolated replay authoring, every stop
and recovery action, separate step/skip/final/abort decisions, retained private
abort copies, hook/editor quiescence, ownership and object retention. Every claim
is a Git observation, an explicit intentional design difference or a deferred
boundary. Its future fault matrix is not an executed recovery qualification.

Local checks passed on Git 2.52.0 / Linux / Rust 1.99.0:

- Dedicated stock-Git `cli-journey` characterization: 40 passed, none ignored.
- Separate source-CI selector: the same 40 passed; remote CI was not run.
- Strict scoped Clippy, Rust formatting, shell syntax and whitespace checks.
- Independent recovery review: approved after closing skip-state completeness,
  child-quiescence, H/U retention-root and retained-copy extraction findings.
- Existing native recipe projected: 80 resources, zero unresolvable locators.

Durable commands, Git observations, matrix extracts, review and hash bindings
are under `genesis/local-dev/brit-readiness/rebase-recovery-design/` in the owning
workspace. `source-binding.json` pins the design/test files to the source commit;
`review.md` is pinned by SHA-256
`dac405babdbbcf3d6ae526bd2de5e1449e8493921000ab86f5ee51a5109caa64`.
`verification.json` records final local commit/readiness receipts.

The 5b executable bindings remain unchanged: source
`5882e60b71f8d98d324fbf97d8aec78f4d6ef445`, ordinary binary SHA-256
`b35866d083446ba308e0a3bf5185eacce2af4f2098064de9e0c0e89c9e0bb89d`, fault binary
`718c54fcc853877996467cc665e421603a45674f9f024d3c73e0607441c576e8`.
Both retained files were rehashed; no executable was requalified in 5c.

Next: 5d conflict-aware authoring → 5e durable linear replay → 5f interactive
sequencing → 5g combined recovery/artifact qualification. Public rebase exposure
waits for those proofs. No push, publication, parent gitlink move, physical
power-cut proof or full daily-driver acceptance is claimed. The habit remains
**UNWIRED**.


## Station 5d — conflict-aware authoring, 2026-10-13

Implementation source: `60cd907e830aedfd46cb3a6dbb9ef281055a7a83`, from
`2a3e51d7c15cc7b83c733f32ed4aea2de5d05d59`, on isolated local branch
`sprint/conflict-aware-authoring-2026-10-13`. This closes the internal authoring
foundation only. Full daily-driver readiness remains **UNWIRED**; stations
5e–5g and public rebase exposure remain unchanged.

The internal Linux session keeps exact original staging, a public resolution
index and a separate commit index. Scoped resolution staging and external Git
imports preserve dirty neighbors, including simultaneous staged/unstaged edits,
intent-to-add, deletion, modes, symlinks and non-UTF-8 names. Native amendment and
split commits consume only the private index. Explicit scope expansion records
before-images and refuses overlap or a baseline it cannot reconstruct exactly;
clean CRLF conversion is one tested refusal. Split completion requires the
private index to match the authored HEAD, not merely a clean worktree.

Child decisions record exact HEAD/index/reflog old and desired bytes. Recovery
refuses third states and foreign worktree ownership. Hooks receive the private
index, and their mutations pass the same scope checks. Durable invocation
preparation precedes the pending marker; a pipe prevents execution before the
boot/PID/start/group receipt is durable. Unknown pre-hooks require retained abort;
unknown post-hooks can be acknowledged without rerunning. Failures after commit
publication report the committed result and remaining recovery work.

Abort retains raw active work, original index bytes and checksummed authored Git
object closures before restoring the stop. The retained copy survives Git GC.
Unrelated new work remains untouched. Corrupt ownership, replaced leases,
symlinked metadata directories and out-of-scope mutations refuse before mutation.

Verification (one Cargo runner; separate implementation/test owners and an
independent preservation/recovery reviewer):

| Check | Result |
| --- | --- |
| Internal native/fault suite, wired into source CI | 32 entries pass: 31 substantive tests and one subprocess helper |
| Retained stock-Git rebase characterization | 40 pass; Git observations only |
| Existing authoring, switch, pull and hook-input journeys | 147 pass |
| Existing switch/pull signal-recovery journeys | 57 pass (25 switch, 32 pull) |
| Existing switch/pull synchronization-loss models | 2 pass; no new authoring power-loss claim |
| Scoped formatting, strict Clippy, whitespace | Pass |
| Independent preservation/recovery review | Approved for bounded station 5d |

Commands, logs, review, source hashes and the final readiness result live under
`genesis/local-dev/brit-readiness/conflict-aware-authoring/` in the owning
workspace, particularly `verification.json`, `source-binding.json`, and
`review.md`. The existing recipe projects this station and the remaining work.
Reach rehearsal: `epr ready --target 2a3e51d7c15cc7b83c733f32ed4aea2de5d05d59`.
No push, publication, parent submodule-pin change, replay sequencer or combined
rebase artifact qualification is included. The local debug binary is a test
artifact, not a release candidate. Full authoring loss-of-unsynchronized-writes
and power-cut qualification, retained-copy decoder/extraction, and complete
replay final/skip/abort orchestration remain later stations. Arbitrary writers
escaping the supervised process group remain unsupported.

Story-harvest node: chain 5d / between recording a pending hook → executing its
child / missing prerequisite: durable owned invocation preparation must exist
before the pending marker, otherwise a crash leaves no inspectable abort path /
probe `unknown_pre_hook_is_abort_only_and_unknown_post_hook_acknowledges_without_retry`
with the pre-hook `after-start` checkpoint / current state: green in the native
suite. This child-level proof supplies a prerequisite for the still-open 5f
invocation node; it does not close interactive sequencing.


## Station 5e — durable internal linear replay, 2026-10-13

Implementation source: `3743b1c490b29c8cc1ee54c0479c3409a9469a0b`, from
`70b08fb4d2a4bc1903c3c91f84da621ca555776b`, on isolated local branch
`sprint/durable-linear-replay-2026-10-13`. This closes internal linear replay.
Public rebase remains unavailable; interactive sequencing and combined public
qualification remain stations 5f and 5g. Full readiness remains **UNWIRED**.

The driver pins explicit local selection and complete required object closures,
keeps the original branch reserved, and replays native three-way merges with
original authorship and durably fixed proposed commit bytes. Original, public
and private indexes remain separate throughout. No-op and fast-forward cases,
conflicts, initial/newly empty commits, external resolution staging, sticky skip,
final publication and abort share owned recovery decisions. Parent-child receipts
bind the operation nonce, worktree, ordinal and exact parent state; preservation
is never recaptured from an already-replayed checkout.

Step, skip, final and abort decisions are distinct. Abort remains available after
completed steps until the final decision. Independent copies contain raw work,
index distinctions, messages, invocation evidence and complete authored objects
before restoration or retention-ref release. Unknown private allocation orphans
remain untouched and make cleanup pending; they do not prevent restoring the
parent's pinned live state. Retired cleanup receipts permit new operations and
cannot claim their journals. Known and unknown post-hook outcomes validate exact
published resources; unknown hooks never rerun. Foreign changes in inspect-only
phases remain inspection-only refusals.

Verification used separate implementation/test owners, one Cargo runner and an
independent preservation/recovery reviewer:

| Check | Result |
| --- | --- |
| Internal native suite | 24 core/model, 32 retained authoring, 63 replay and 8 retained-copy entries pass; includes two subprocess helpers |
| Stock-Git characterization | 40 pass; observations, not public Brit rebase support |
| Existing authoring/staging/hook/switch/pull CLI journeys | 147 pass |
| Existing switch/pull signal-recovery journeys | 57 pass |
| Real signal checkpoints and synchronization-loss models | Pass, including omission controls and replay final/abort/cleanup ordering |
| Strict scoped Clippy, formatting and whitespace | Pass |
| Small, lean-async and max-pure configurations | Pass |
| Independent preservation/recovery review | Approved for bounded internal Linux 5e |

Commands, failures/fixes, review, logs and exact source hashes are retained under
`genesis/local-dev/brit-readiness/durable-linear-replay/` in the owning workspace:
`verification.json`, `review.json` and `source-binding.json`. The review receipt
is pinned by SHA-256
`3c30b528c6fcf60df56c429dea1006479822849d2e296b8118a09010d69b4560`. The existing recipe
projects this station. Reach rehearsal uses
`epr ready --target 70b08fb4d2a4bc1903c3c91f84da621ca555776b`.
The local fault-enabled binary is an internal regression artifact. These proofs
cover Linux fsync/rename behavior and loss-of-unsynchronized-writes models, not
physical power cuts or hostile-writer isolation. No push, publication, parent
submodule-pin change, public activation or whole daily-driver acceptance occurred.

Story-harvest node: chain 5e / between abort copy completion → restoration /
missing prerequisite: interrupted private allocation must not make the immutable
copy or staging path reusable; persist a fresh copy sequence and preserve the
unproved orphan before retrying / probe
`abort_retry_after_its_own_unowned_staging_retains_orphan_without_replacing_prior_copy`,
including a second interruption while saving the new sequence / current state:
green in `native-tests-20.log`. This closes the internal abort-availability node;
the full combined state/action/preservation matrix remains station 5g.


## Station 5f — internal interactive sequencing, 2026-10-13

Source base: `dc9d72a2fba24bdaefacf28f2684c3a355eba6ae`; isolated local branch:
`sprint/interactive-sequencing-2026-10-13`. Implementation source: `09ba4016cc1683e4c536c495ccf78691e2b69179`. Final internal native, CLI,
formatting, strict lint and feature checks pass; independent bounded Linux
preservation/recovery approval is recorded with exact source hashes. Public rebase remains unavailable and full readiness remains
**UNWIRED**; combined recovery, retained-copy tooling and public artifact
qualification belong to station 5g.

The existing `gix-sequencer` crate now owns pure todo parsing and original-to-result
mapping transitions. The replay driver owns durable acceptance and mutation.
Pick, reword, edit, squash, plain fixup, drop and break support their unambiguous
aliases; rejected output cannot replace the last accepted todo. Completed entries
remain fixed, every remaining candidate must occur exactly once, and reordered
work revalidates the preservation footprint. Execution order stays separate from
original mapping order, including zero-result drops, squash/fixup groups and
ordered one-to-many split results.

Sequence and message editors use the existing supervised invocation machinery,
byte-safe arguments, explicit precedence and trust checks. Failed outputs remain
available; unknown outcomes require explicit saved-byte acceptance without
relaunch. Launch authorization rechecks the exact parent and live resources under
the shared guard. Terminal tests exercise Ctrl-Z, foreground resumption and
Ctrl-C. Explicit edit-stop staging, amendment and splitting consume the private
index and preserve the operation's original staged/unstaged distinction. Message
hooks retain their pre-commit → prepare-commit-msg → editor → commit-msg order;
automatic replay itself runs only operation hooks.

Child authoring seals its exact proposed decision and raw object closure before
creating owned retention refs or publishing a decision. Recovery reuses those
bytes after GC without resampling the commit or rerunning pre-hooks. All authored
receipt heads, including superseded amendments and skipped split chains, remain
recoverable from independent retained copies. Partial raw-object `.new` files
are opaque retained evidence, never adopted objects; they can leave cleanup
pending without blocking abort. Durable post-notification queues distinguish
not-yet-started hooks from unknown outcomes: recovery runs the former once and
requires explicit acknowledgement of the latter without rerun.

Separate implementation/test owners, one Cargo runner and independent
preservation/recovery review remain in use. Current evidence is scoped as follows:

| Check | Recorded result |
| --- | --- |
| Full internal native suite | `native-tests-final.log`: 24 core/model, 32 retained authoring, 63 replay, 27 interactive and 8 retained-copy entries pass, including subprocess helpers |
| Pure sequencer and mapping tests | 8 pass in `sequencer-tests-2.log` |
| Retained stock-Git characterization | 40 pass in `characterization.log`; compatibility observations remain distinct from intentional Brit recovery behavior |
| Interactive synchronization-loss model | Six execution modes across final/abort, with omission controls, pass in `interactive-model-1.log` |
| Strict scoped Clippy | Core and sequencer pass in `clippy-final.log` and `sequencer-clippy.log` |
| Final queued-hook, terminal and signal tests | All 27 interactive entries pass in `native-tests-final.log`, including repeated background suspension and exact unknown-hook acknowledgement |
| Retained CLI and feature configurations | 147 CLI and 57 switch/pull fault entries pass; small, lean-async and max-pure checks pass against the corrected source |
| Independent preservation/recovery review | `review.json`: bounded internal Linux approval confirmed against all 15 source hashes and final native proof; no 5g/public approval |

Commands, failures/fixes, review and final source/readiness bindings belong under
`genesis/local-dev/brit-readiness/interactive-sequencing/` in the owning workspace.
`verification.json`, `source-binding.json` and `review.json` bind the implementation,
commands, artifact hash and independent review. The evidence commit projects this
station through the existing recipe. Reach rehearsal targets
`epr ready --target dc9d72a2fba24bdaefacf28f2684c3a355eba6ae`.
These proofs cover Linux fsync/rename and loss-of-unsynchronized-writes models;
they do not claim physical power-cut qualification or hostile-writer isolation.
No push, publication, parent submodule-pin change, public activation or full
daily-driver acceptance is included.

Story-harvest prerequisites retained as measured nodes:

- Chain 5f / between proposed child authorship → owned retention and decision publication / missing node: seal exact decision bytes and complete raw object closure before the first retention ref, so recovery after GC reuses the same commit and never reruns pre-hooks / probe `durable_child_proposal_recovers_fixed_commit_after_gc_without_repeating_hooks` / current state: green in `native-tests-final.log`, including queued post-notification recovery.
- Chain 5f / between interrupted raw-object persistence → abort restoration / missing node: an unclaimed `.new` file remains opaque retained evidence and cannot block restoration or become an adopted object / probe `interrupted_child_raw_object_temporary_is_retained_opaquely_on_abort` / current state: green in `native-tests-2.log`; orphan cleanup remains explicitly pending.
- Chain 5f / between committed child decision → post-authoring notification completion / missing node: a durable queue launches known-not-started notifications once, while unknown outcomes require acknowledgement without rerun before the next notification / probe `unknown_first_queued_post_hook_is_acknowledged_once_before_second_hook_runs` / current state: green in `native-tests-final.log`.
- Chain 5f / between editor suspension → foreground terminal handoff / missing node: synchronously stop the supervising calling thread before checking foreground ownership or resuming the editor, so repeated background continuation cannot reclaim the terminal; process-directed SIGSTOP alone can leave a multithreaded caller running past the stop request / probe `terminal_editor_ctrl_z_fg_and_ctrl_c_return_terminal_to_calling_shell` exercises background continuation twice, foreground resumption and Ctrl-C / current state: green in `native-tests-final.log` after both stops use `nix::sys::signal::raise(SIGSTOP)`; independent review confirms the final source binding.


## Station 5g — qualified local Linux public rebase, 2026-10-13

Source base: `5125ceb339b7a23b24426ab008378bf7c6892d9c`; isolated branch:
`sprint/rebase-qualification-2026-10-13`. Implementation source:
`b09a6090749d6517bbe2ef049f1ab13447b37343`. Bounded public rebase and the
`brit-rebase-retained` helper are enabled after internal qualification and
independent preservation/recovery source approval. Exact committed ordinary and
fault artifacts pass their public journeys. Full daily-driver readiness remains
**UNWIRED**; remote platform qualification, published-version upgrade/rollback
and two-developer experiential acceptance remain separate open stations.

The public interface supports explicit local upstreams, interactive sequencing,
status/continue/skip/abort, todo editing, current-patch inspection, literal scoped
staging and explicit edit-stop amendment/splitting. Deferred options refuse;
unsupported platforms refuse execution. Private journals, three-index preservation,
owned invocation receipts and retention refs remain local Git bridge state.
No protocol entity, network service or recurring protocol head is introduced.
The [usage and recovery guide](../rebase.md) documents stop outcomes, editor
configuration, helper installation, lossless extraction and explicit copy-leaf
removal. Retained copies do not expire automatically; symlinks extract as inert
payload files into an empty checked destination.

Separate implementation/test owners, one Cargo runner and independent recovery
review were maintained. Evidence under
`genesis/local-dev/brit-readiness/rebase-qualification/` in the owning workspace
binds commands, results, source hashes and binary hashes:

| Check | Recorded result |
| --- | --- |
| Final native suite | 178 entries pass: 24 core/model, 32 authoring, 63 replay, 28 interactive, 2 invocation, 6 I/O, 11 combined qualification and 12 retained-copy entries, including helpers |
| Preservation matrix | Six stops × eight dirty/preserved shapes × finish/abort = 96 cases; 12 ancestor/descendant collision cases pass |
| Synchronization-loss qualification | 16 modes × final/abort = 32 paths follow actual synchronization boundaries, including child decisions and invocation ordering; omission controls demonstrate missing synchronization breaks invariants |
| Real interruption and I/O | SIGINT/SIGKILL, live/ambiguous owners, replaced locks, corruption, ENOSPC and file/directory sync errors distinguish unpublished, committed, UNKNOWN and cleanup-pending outcomes |
| Pure sequencing / retained Git observations | 8 sequencer cases and all 40 Git characterization cases pass; intentional Brit recovery differences remain explicit |
| Strict checks | Core and CLI all-target Clippy, scoped formatting, small/lean-async/max-pure configurations, readiness contracts, workflow selectors, shell syntax, packaging and installer fixtures pass |
| Exact ordinary artifacts | 153 installed CLI entries pass, including all public rebase controls/authoring and retained-copy recovery after cleanup without runtime Git |
| Exact fault artifacts | 68 entries pass: 11 rebase, 32 pull and 25 switch/recovery journeys |
| Independent review | `review.json` binds production hashes, native proof and committed artifacts; reviewer contributed public tests but no production changes |

Artifacts were built from the clean implementation commit with Rust/Cargo 1.99.0,
`x86_64-unknown-linux-gnu`, **dev profile**. Ordinary features are default `max`;
the fault build additionally enables `switch-test-faults`. These are local copied
installed artifacts, not a claim that optimized release CI ran. Source CI and
Linux installed-release selectors now include the native/public/fault/helper
suites and checked helper archive delivery.

| Artifact | SHA-256 |
| --- | --- |
| Ordinary `brit` | `1dfd338aecc55d7f7731b8b80bfde2fc6956bed8442dc61303f4ddd0e6618fbc` |
| Ordinary `brit-rebase-retained` | `49e773767cfb1ec7e622386e7b55ed48328e5ff96b58bf4c617328fcf48b2c9c` |
| Fault `brit` | `54873045c356b1d514615e22f19393af6ebdc340c61853a3fc3502f53dbc0348` |
| Fault `brit-rebase-retained` | `f880892022a74e8fbdef1400c0d406789230c59012ddac99632f818975d6f081` |

`verification.json`, `source-binding.json`, `artifacts.json`,
`artifact-check-receipts.json` and `review.json` retain exact commands, toolchain,
paths and hashes. Failed runs are retained alongside the fixes: the extended
model exposed unchanged directory postimages omitted from parent state; public
journeys exposed relative repository paths, an interactive/control parser
combination and CLI-handled SIGINT that the earlier native fixtures did not
exercise. Editor fixture configuration skew and test compile errors were fixed
without relaxing production configuration guards.

Story-harvest prerequisites retained as measured nodes:

- Chain 5g / between checkout transition → later replay guard / missing node: repair every existing tracked directory postimage before filtering no-op directory writes, but never add unchanged ancestors to mutation scope / probes the directory-to-file loss model and `todo_edit_at_break_and_edit_stop_keeps_unchanged_ancestor_out_of_preserved_sibling_scope` / current state: green.
- Chain 5g / between CLI discovery → private absolute-path metadata / missing node: normalize repository paths in place while retaining effective configuration and object/ref context / probe exact public artifact journeys from relative working directories / current state: green.
- Chain 5g / between interrupt handler → supervised child quiescence / missing node: poll cancellation during child wait and saturated stdin, signal only the validated owned group, and bound uncooperative waiting without escalation or a fabricated exit receipt / probe `large_unread_hook_input_interrupt_is_bounded_and_never_retried` with 1 MiB unread input plus public live-hook SIGINT / current state: green.
- Chain 5g / between failed editor → explicit saved-byte acceptance / missing node: preserve pinned effective editor configuration across commands and prove the intended editor actually ran before asserting acceptance behavior / probe failed-todo public journey checks exact saved invalid bytes / current state: green.

The existing recipe projects this completed station. Reach rehearsal targets
`epr ready --target 5125ceb339b7a23b24426ab008378bf7c6892d9c`.
Qualification covers Linux fsync/rename behavior and a separate loss-of-unsynchronized-
writes model; it does not establish physical power-cut safety or hostile-writer
isolation. No push, publication, parent submodule-pin change, remote CI execution
or full daily-driver acceptance is claimed.

## Integration and dogfooding — 2026-10-13

The integration candidate includes the complete implementation sequence through
5g, evidence commit `947bf313905383c0dcf8f62a72ce52b008f3ebb9`, on
`run-ci/daily-driver-integration-2026-10-13`. The parent submodule checkout was
behind this separate worktree sequence; it was not the integration source.

Operator clarification: operational verification will happen through our own
ongoing development with Brit, including handoffs between workspaces. A separate
two-human ceremony or two scripted local identities is not this pass's finish
line. Land the tested implementation and make the candidate usable now; record
real dogfooding outcomes as they occur. Cross-workspace operation has not yet
been exercised, and fixtures or green CI must not claim that evidence. The
habit remains UNWIRED until its workflow assertion is actually established.

The installed 5g development artifact (`b09a6090`, checksum recorded above) was
used for real status inspection and fetching this integration branch from GitHub.
Its fetch matched the advertised `947bf3139` source without stock-Git transport.
This is a bounded real-use observation, not cross-workspace qualification.

GitHub CI run [38376775271](https://github.com/ethosengine/brit/actions/runs/38376775271)
exposed integration gaps that the scoped Linux proofs did not cover:

- The pack command serialized its new device/inode ownership witness, changing
  an inherited JSON snapshot. `keep_identity` is now excluded from serialization
  and deserialization: local ownership cannot be transported as output data.
  The original pack JSON snapshot passes without changing expected bytes.
- Git rebase characterization editors used GNU-only `sed` forms. The fixtures
  now use BSD/GNU-compatible backup suffixes and a portable break insertion.
  All 40 characterization cases still pass locally.
- The broader-staging alias fixture wrote over the existing lowercase file on
  case-insensitive filesystems. It now renames through a distinct temporary
  name so the directory entry actually differs from the index spelling.
  All 15 broader-staging tests pass locally; macOS CI verifies that platform.

Maintainer nodes: between scoped authoring proof and portable CI, the fixture
must create the same intended state on each filesystem; between pack custody
and JSON output, a local ownership witness must remain local. Existing staging,
characterization and pack snapshot tests are the probes. No assertion was
removed or weakened. Private logs live under the parent workspace's
`genesis/local-dev/brit-readiness/integration-2026-10-13/`.

The 32-bit Debian lane additionally lacked Python's `pty` standard-library
module (`python3-minimal`); its prerequisite is now `python3`, keeping the terminal
recovery test enabled. Its Git server reports a failed compare-and-swap as
`failed to update ref`; the lease probe accepts that diagnostic while still
requiring rejection, the hook's advertised old OID, and the competitor's exact
remote ref to survive. Those behavioral assertions remain unchanged.

Real-use boundary: `brit add` once refused the integration worktree because system
and user Git configuration declare Git LFS clean/process filters, though no path
there uses LFS. No index change occurred. The refusal is now scoped per path:
configured drivers alone never refuse. `add`, `restore --worktree`, `switch`,
`pull --ff-only` and replay resolve each path they convert (for switch, pull and
replay, every path in the change set) through gix's attribute stack, and refuse
only when a path resolves `filter=<name>` for a configured driver. Attribute
state brit cannot know in full fails closed whatever the paths: an unreadable
attribute source, a source with a line gix's parser drops, or an `[attr]` macro
that assigns `filter`. A refusal leaves the index, HEAD and worktree unchanged
and never runs the driver; use Git for those paths. We did not remove or
override the workspace's filter configuration to claim native staging.

Continued integration at `e0773f57e` exposed four further boundaries:

- The small CLI's inherited typo-suggestion snapshot needed the now-supported
  `restore` command. macOS hook observations needed numeric comparisons for
  BSD `wc`'s padded counts, preserving the exact expected mapping counts.
- The i386 synchronization-loss probe encountered Linux `ESRCH` while reading
  an unrelated exiting process during the hook-group scan. Opening a process's
  stat file, terminating/reaping that process and reading the open file reproduced
  errno 3 locally. Treat this vanished-process result like `ENOENT`; live group
  members and reused identities still prevent recovery.
- Real native HTTPS push waited on Che's askpass child. The configured shell
  credential helper contains an inner `"$@"`, which caused the generic command
  launcher to omit the operation on its final function call. Append the helper
  operation to the script as Git does. The isolated synthetic regression failed
  before the fix; all 71 credential integration tests then passed. No credentials
  were printed or copied into test fixtures. Git transport completed the earlier
  push after the stalled native process was stopped and remote state checked.
- Candidate run 38378712945 compiled Linux successfully but packaging received
  an empty version: GitHub explicitly suppressed the `version_tag` job output
  because of secret masking. Each build now derives this public version from
  its exact validated source. Manual dispatch still cannot publish a release.

Maintainer nodes: between helper configuration and authenticated transport,
the helper's final call must receive its operation (synthetic function-forwarding
probe); between process enumeration and recovery permission, disappearance during
an open proc-file read must remain distinguishable from a live owned group
(Linux proc read and existing synchronization-loss probes); between validated
source and package identity, the public version must be available independently
of cross-job output masking (candidate packaging). These discoveries come from
integration and real use; cross-workspace operational verification remains future
work in our own development.

Local follow-up: the rebuilt `max-pure` executable completed an authenticated
GitHub push of the already-advertised integration head with the existing system
credential helper and prompting disabled (exit 0). The complete real replay
synchronization-loss test passed in 340.55 seconds, all 40 Git rebase observations
passed, and the small-mode typo snapshot matched exactly. These are bounded
integration observations, not cross-workspace acceptance.

The full inherited lane also found missing executable bits on the delivery-source
scripts and Git rebase fixture; `just check-mode` passes after correcting them.
Publication preflight found the new `gitoxide-core` → `gix-sequencer` edge lacked
its version/registry declaration. The corrected graph validates all 70 local
crates; the read-only index audit found all 70 exact versions absent. No-upload
packaging reaches the expected bootstrap boundary when a later package needs an
earlier, not-yet-published dependency. Actual publication remains the main-only
CI lane's responsibility.

GitHub masking also replaces hyphens in runner-reported step names. Three public
readiness labels now use spaces in both workflow and contract, retaining exact
matching and the same commands instead of weakening the evidence check.

At `49c226115fa5c8eda635c39141c5d7928f31f4e2`, GitHub's `Tests pass`
gate is green (run 38383733374). Candidate 38383774189 passed Linux and
Windows delivery. The Linux archive was checksummed and installed privately at
`genesis/local-dev/brit-readiness/daily-driver/bin` in the owning workspace;
its source-free local fixture and real checkout status inspection pass.
This install and real native commits/fetches/pushes are development observations,
not cross-workspace operational acceptance.

The macOS candidate passed installed commands, then found a fault-fixture path
alias: the pair installer canonicalizes its prefix, while the concurrent-activation
shim compared it to the `/var` spelling rather than `/private/var`. A symlinked
`TMPDIR` reproduced the failure on Linux. Canonicalizing the fixture root makes
both alias and physical-directory runs pass, with rollback/interruption/concurrent
activation assertions unchanged. The next candidate must confirm this on macOS.

Revision `1d055172ac43861a413614a7a501bc582c1377a7` passed the entire
integration CI run 38386962091 and all four delivery platforms in run
38386981865. Both strict readiness qualifiers passed without issues. Its Linux
artifact is installed in the private daily-driver prefix and passed the local
installed smoke fixture; it also performed the real main fast-forward push
through the EPR reach hook (1,584 paths, ready).

The post-landing run 38391282996 exposed a concurrent fixture race on i386:
`command_in_path_with_args` listed a shared working directory while the relative
shell-path test removed its temporary `$HOME…` entry. Of 5,565 tests, only that
listing failed. The two listing probes now use their own stable directories,
preserving real command lookup and argument execution.
The corrected command suite also passed 100 runs at 16 test threads (6,100
integration test executions), plus strict scoped Clippy and formatting.
The main run was cancelled before publication; no registry delivery is claimed.
Maintainer node: between
parallel fixture creation and command-success assertions, the listing's directory
must remain owned by that test (isolated listing probes). Cross-workspace
operational acceptance still belongs to our ongoing development.

Follow-up CI 38393588588 exposed a separate ARM32 replay failure: the invocation
scanner parsed Linux's signed process-group field as `u32`, producing an invalid
digit error during `rebase --continue`. Linux `do_task_stat()` can retain a `-1`
group during task teardown. A local real-process exit probe observed state `X`
and group `-1` after 101 attempts. The parser now retains a signed group and uses
checked conversion when comparing it to an owned PID, so the sentinel cannot
authorize a signal or impersonate the owned group. Malformed values still refuse.
Maintainer node: between process exit and quiescence inspection, kernel teardown
sentinels must be readable without becoming process ownership (signed-record
regressions and the live proc exit probe). Kernel reference:
https://github.com/torvalds/linux/blob/v6.12/fs/proc/array.c#L450-L510.

Local validation of the signed-field correction: two parser regressions, 28
interactive replay tests, two invocation tests, 11 preservation/recovery matrix
tests, all six public rebase journeys using the rebuilt CLI, strict scoped
Clippy and formatting passed. Remote confirmation remains pending.


### 2026-11-05 — retained helper through the followed installation

The delivery closure is four executables: `brit`, `brit-build-ref`,
`brit-rebase-retained`, `brit-verify` (with `rakia` remaining an alias).
The workflow builds and publishes the helper with the same source SHA and
checksum set as Brit. No crate source/version or registry-only `cargo install
--bin brit` contract changes in this slice; source-SHA releases avoid the
reserved 0.1.10 lane.

The parent runtime declaration registers the helper and its existing
`BRIT_REBASE_RETAINED_BIN` override. Historical three-executable releases remain
usable, including plain-binary adoption and rollback. A session or explicit
Brit pin selects the whole release; asking an old release for its absent helper
names that release and refuses. It never borrows a newer helper. Declared-but-
missing and corrupt helpers fail checksum verification.

Measured candidate evidence: packaging executes the workflow shell bodies with
local build/forge fixtures and checks all five uploaded assets; six ordinary
public rebase journeys pass against preserved 0.1.11 binaries; exported payloads
match captured bytes, both collision fixtures are present independently, and
all repository file bytes remain unchanged by inspect/extract. Strict scoped
Clippy, formatting and Rust-style checks pass. The first exploratory journey
selected a stale 0.1.9 pool binary and reproduced the previously fixed collision
refusal; the corrected run names preserved 0.1.11 artifacts. Review round 1
found two moderate runtime regressions; both are corrected and round 2 reports
zero critical/high/moderate findings. Runtime gate, installed candidate launcher
and exact-revision CI/publication evidence will be appended after completion.

Maintainer stations: chain daily-driver retained recovery / between packaged
helper → installed recovery / node: historical inventory remains installable
without changing the selected release; probes `historical_install_upgrade_session_and_rollback_keep_one_release`
and `historical_launcher_install_preserves_plain_brit_without_borrowing_helper`.
Between installed recovery → exported recovery / node: every exported payload
matches retained bytes and helpers leave repository bytes unchanged; probe
`public_linear_replay_finishes_and_helper_exports_after_cleanup_without_runtime_git`.
The existing public journey remains the finish assertion. No new a2o scaffold
is needed for these executable regression boundaries.

Receipts live under parent `genesis/a2o/reports/recovery/brit-retained-20261105/`.
Direct shell source reads supplemented the bounded recall session
`brit-retained-delivery`; unsupported Codex shell adapter coverage is unmeasured.
Repository-opening, cancellation bounds, submodules and named remotes remain
outside this delivery slice.


Candidate follow-up: `84972abb3` CI run `42028702733` completed the Linux test
steps but failed the final repository executable-mode check: the new shell
fixture had a shebang and mode 0644. Corrected it to 0755 and ran it directly;
all four executables and their checksums/uploads pass. Production code is
unchanged. Parent runtime: 224 tests and strict Clippy pass; parent Rust-style
and orchestrator gates pass after supplying generated Husky shims in the fresh
worktree. The isolated installed-launcher run now passes all six public rebase
journeys. Registry audit finds all 70 existing versions, with zero new versions.
The full EPRFS gate and exact replacement-revision CI remain the publication
frontier; the mode failure is retained in `ci-test-failure.txt`.

Publication preflight found a missing station: release workflow / between exact
Tests pass → source-SHA artifact / required aggregate admission: read exactly
one successful Tests pass from the triggering run AND attempt, before checkout
or publication. Whole-workflow success wrongly included the known non-gating
advisory job. The producer now preserves push/main/same-repository guards and
checks the attempt-specific API with Actions read permission. Real jq fixtures
prove paginated admission and fail-closed missing, duplicate, unsuccessful,
malformed and unavailable verdicts. Independent review corrected run-attempt
binding and the unsupported gh --slurp/--jq combination. No dependency/security
policy is changed. Fresh exact-revision CI remains required before publication.


## Fixed-ceiling memory work — 2026-11-13, incomplete

The acceptance target is **1,073,741,824 bytes for Brit and all owned children
in one Linux cgroup**, swap disabled, and an independent **1 GiB scratch cap**.
This is external qualification, not an always-on runtime sandbox. Existing
allocation configuration and reduced-trust defaults remain authoritative. No
algorithm selection, binary decision, automatic Git fallback or release version
has changed. This work builds on the existing 0.1.14 integration candidate
`a4db877811c153a1f97f8a916877d1d18248005f`; it does not duplicate that release.

The first implementation reuses `FindStream`, bounded filter conversion and the
existing charged `Spool`. Raw cat, worktree-form cat, archive blob extraction,
staging and scoped restore avoid retaining complete blob copies. Working,
cached and tree-stat comparisons consume immutable operation-owned mappings
above the effective object allocation limit (or the existing large-file
threshold when no explicit allocation limit exists). The comparison algorithm
and tie-breaking are unchanged. Atomic patch output is itself charged scratch.
Restore retains whole-selection preflight and per-file atomic publication;
destination temporary files share the operation quota and completed files are
reclaimed promptly. Symlink, executable mode and conversion tests remain active.
Clean conversions now stream EOL, ident and encoding operations. Status hashes
ordinary files directly and reads auto-EOL index objects through `FindStream`.
Routine configuration reloads reapply the existing reduced-trust allocation
fallback; a privileged ownership fixture caught and now guards the previous loss
of that policy during the CLI's precious-file configuration update. Explicit limits
and the explicit reduced-trust fallback opt-out remain effective.
Pack decode diagnostics distinguish configured allocation refusal from allocator
failure and include requested sizes; packed object lookup also names the pack
and offset. Scratch exhaustion has a separate typed diagnostic.

### Measured local proof, not final qualification

`tests/cli-journey/tests/memory.rs` drives disposable repositories and the isolated
stock-Git helper. It compares bytes at 16 MiB minus one, exactly 16 MiB, plus one,
20 MiB, 64 MiB and 256 MiB, for loose and packed objects. Explicit 16 MiB and
1 GiB allocation settings exercise mapped and buffered representations of the
same two-sided inputs. Million-line fixtures cover both repetition and unique
lines with append-only changes; numeric-leading lines avoid the existing
function-context-header compatibility exception documented above. A two-file
64 MiB restore fixture checks index preservation and rejection before publication.

The opt-in `scripts/ci/qualify-memory.sh` enters its exclusively owned delegated
cgroup before exec; test harness, Brit, Git oracle and all descendants inherit it.
It records executable hashes before and after execution, fixture hash kind, toolchain, arguments, elapsed time, memory peak,
memory events, exits and surviving processes. An OOM, surviving child or changed executable fails the
run. A self-changing executable control exits successfully but is correctly rejected by the wrapper. The wrapper deliberately reports `qualificationComplete: false` while
scratch peak evidence and complete source/workload qualification are absent.
It does not turn missing evidence into a successful qualification.

| Local run | Outcome | Whole-group peak | Elapsed |
| --- | --- | ---: | ---: |
| Boundary through 20 MiB, diff/stat/cat/add | 2 passed, no OOM | 89,878,528 B | 21.23 s |
| Boundary through 256 MiB, buffered/mapped and loose/packed | 2 passed, no OOM | 1,073,741,824 B | 120.04 s |
| Two 64 MiB files, scoped restore | passed, no OOM | 214,777,856 B | 6.06 s |
| Million-line repetitive/unique comparisons | passed, no OOM | 160,239,616 B | 14.19 s |
| Three-sided 64 MiB file merge | passed, no OOM | 618,782,720 B | 1.59 s |
| Existing bounded checkout/recovery fixtures through 256 MiB | 8 passed, no OOM | 1,073,741,824 B | 64.35 s |
| 64 MiB clean conversions and status | passed, no OOM | 216,305,664 B | 24.09 s |
| Combined seven-case daily matrix, including 200,000-entry metadata | 7 passed, no OOM | 1,073,741,824 B | 173.60 s |
| Local clone/fetch/push at 64/256 MiB, including owned transports | passed, no OOM | 1,073,741,824 B | 49.12 s |
| Ownership-derived reduced trust, streamed blob and metadata refusal | passed, no OOM | 165,539,840 B | 1.91 s |
| 64/256 MiB converted archive and commit | passed, no OOM | 1,018,191,872 B | 21.54 s |
| 64-revision, 20 MiB append-only blame, before cache reclamation | failed, exit 137, 3 OOM kills | 1,073,741,824 B | 31.20 s |
| Same blame fixture after per-comparison reclamation | passed, all 64 attributions preserved | 643,690,496 B | 59.04 s |
| SIGINT during 256 MiB archive spooling | passed, controlled return under one second | 323,751,936 B | 3.75 s |
| Expanded daily matrix, SHA-1 | 12 passed, no OOM | 1,073,741,824 B | 302.60 s |
| Expanded daily matrix, SHA-256 | 12 passed, no OOM | 1,073,741,824 B | 937.66 s |
| 20 MiB switch journal, before repair | passed, ceiling reached | 1,073,741,824 B | 333.06 s |
| 64 MiB switch journal, before repair | failed, exit 137, 3 OOM kills | 1,073,741,824 B | 53.41 s |
| 20/64/256 MiB switch journal, raw payloads, loose/packed, 1 GiB/16 MiB allocation settings | 3 passed, no OOM | 1,073,741,824 B | 90.72 s |
| Snapshot 20 MiB, explicit allocation limit and actual reduced trust, before repair | 2 failed allocation refusal, no OOM | 76,120,064 B | 1.33 s |
| Snapshot 20/64 MiB round-trip, aggregate files, actual reduced trust and existing 256 MiB refusal | 5 passed, no OOM | 553,558,016 B | 115.29 s |
| 20 MiB `diff file`, explicit 16 MiB policy, before repair | failed allocation refusal, no OOM | 93,544,448 B | 1.20 s |
| `diff file`/diff-format reads at 20/64/256 MiB, textconv expansion refusal and real SIGINT | 5 passed, no OOM or surviving owned processes | 1,073,741,824 B | 65.64 s |
| Loose object verification before streaming, explicit 16 MiB policy at 20/64/256 MiB | 3 allocation refusals, no OOM | 561,020,928 B | 29.25 s |
| Object verification/inspection and pack creation/index/extraction, 20/64/256 MiB and 64 MiB OFS/REF histories; explicit-limit refusal contract | 5 passed, no OOM or surviving owned processes | 828,985,344 B | 95.52 s |
| Bounded blame: 64 revisions, actual reduced trust, one million unique lines; synthetic nested-bare discovery failure retained | 3 passed, 1 failed discovery; no OOM | 187,695,104 B | 81.86 s |
| Bounded blame policy equality: loose/packed, rename with edits, dirty worktree and actual bare clone | passed, no OOM | 178,540,544 B | 18.66 s |
| Object-backed merge before repair, 20/64 MiB at an explicit 16 MiB policy | 2 allocation refusals, no OOM | 619,696,128 B | 6.64 s |
| Object-backed file/tree/commit merges at 20/64 MiB, loose/packed, buffered/mapped and actual reduced trust | 3 passed, no OOM or surviving processes | 625,033,216 B | 34.88 s |
| Million-line repetitive/unique file/tree/commit merges, buffered/mapped and loose/packed | passed, no OOM | 363,417,600 B | 63.84 s |
| SIGINT during 64 MiB merge input spooling | passed, controlled return under one second | 254,152,704 B | 2.65 s |
| Deliberate 1.2 GiB child allocation | correctly rejected, exit 137, 2 OOM kills | 1,073,741,824 B | negative control |

The 256 MiB run reached `memory.max` with reclamation (495 max events), without
OOM or OOM kills. Reaching that peak is not headroom. Scratch peak is currently
unmeasured; these receipts cannot close the fixed-ceiling acceptance station.
Private logs and receipts are in Elohim's existing isolated
`genesis/local-dev/brit-readiness/bounded-memory/` directory. Source reads outside
the private recall executor, the explicit fast-forward Git fallback (installed
Brit has no branch-merge operation), and unsupported Codex shell-adapter
coverage are observation gaps, not clean governance verdicts.

Ordinary authoring verification passes 150 public journeys; the fault-enabled
native recovery matrix passes 163 tests. Shared reader/pack/archive tests, clean
conversion and status suites, strict affected-crate Clippy, formatting and the
declared Rust-style scope pass. These are local source checks, not CI or complete
resource qualification. The public memory suite is selected explicitly in CI;
the privileged ownership case is ignored there and was explicitly executed on
the delegated worker. All 121 fault-enabled public CLI authoring/recovery checks also pass.

Story-harvest station: **daily-driver allocation safety / between ownership
classification → command resource use / policy survives configuration reload**.
The probe is `reduced_trust_sets_a_default_limit_unless_disabled` plus
`actual_reduced_trust_keeps_ownership_policy_and_streams_large_blobs`; the
previous CLI succeeded on metadata that should have exceeded its implicit
16 MiB policy, and the repaired CLI correctly refuses while streaming the
20 MiB blob. This node is measured locally; whole-command qualification stays
open. Existing native fixtures own the regression, with no parallel registry.

Blame now reclaims the completed comparison's resource cache at each revision;
its existing 101 tests pass and the formerly OOM-killed append-only fixture
preserves all 64 expected attributions. Archive production now receives the
public interruption flag during blob spooling and conversion. The controlled
SIGINT fixture preserves HEAD/index and removes its scratch; blocking OS calls
remain outside the one-second claim. Both expanded hash matrices include long
single lines, unchanged files and binary content with buffered/mapped equality.
The separate privileged ownership fixture is explicitly run outside those 12-case
matrices. Scratch remains unmeasured in these receipts.

Story-harvest stations: **history attribution / between completed comparison →
next revision / completed blob buffers are reclaimed**, and **archive cancellation /
between signal received → command returns / blob producer observes interruption**.
The public memory fixtures and existing plumbing suites own these probes.
The 64 MiB switch fixture exposed another station: **durable checkout /
between preflight → publication / recovery payloads fit the same resource bounds**.
Its double JSON encoding and retained byte images caused the measured OOM.
Switch and pull now journal immutable raw payloads once, with checksummed metadata,
streamed dirty checks and conversions, and quota reservations for retained recovery
content. Journal versions 1 and 2 remain recoverable. Intent precedes payload
publication; recovery verifies payload bytes and ownership before changing state.
A bound scratch directory is reclaimed only after the previous owner is proven dead
and ordinary lock ownership passes. Foreign members and corruption retain evidence.
SIGINT during preparation preserves the explicit recovery path. Generic checkout
spools retain umask semantics; only private journal payloads are forced to mode 0600.
The packed 20/64/256 MiB matrix passes both allocation policies under the fixed
cgroup. This measures the station locally; large metadata and whole-operation
scratch telemetry remain open.

Snapshot sealing now transfers buffered blob ownership without a second whole-file
copy and uses the existing mapped spool above the effective allocation policy.
The canonical EPRFS codec, CIDs, Git OID verification and shared closure loader
are unchanged. Eleven bridge tests include buffered/mapped byte equality for
SHA-1 and SHA-256; public resource fixtures explicitly execute actual reduced
trust and verify complete exported/restored Git trees. The published native
DirectoryStorage already limits each content object to 64 MiB; its 256 MiB
refusal is an existing unsupported capability, not evidence that an otherwise
supported command fits in memory. This ceiling is preserved. Snapshot capture,
larger closure metadata and cancellation still need qualification.

Story-harvest station: **snapshot sealing / between Git object header → canonical
storage / blob custody honors the effective allocation policy without copying**.
The two formerly failing 20 MiB fixtures now pass; these observations do not
complete the command-family or global resource audit.

The shared comparison platform now also accepts resources through its existing
object-stream and filter interfaces, buffering converted results within the
effective allocation policy and mapping larger ones. Public `diff file` and
`revision resolve --cat-file --blob-format diff|diff-or-git` use that path;
`diff file` stages patch output in charged scratch without accumulating the
complete patch or each rendered hunk in RAM. Its original tokenization, selected
algorithm, slider heuristics and header/newline behavior are preserved.
The existing conversion fixtures compare both representations for all three
conversion modes, binary attributes, thresholds, symlinks, EOL and textconv;
a dangling-path converter regression also preserves the old stat behavior.
Cache clones share mappings and release quota after the final reader.

Textconv stdout and stderr share the operation quota. On Linux the converter
runs in its own process group; interruption or output refusal retires that group
before reaping the leader, avoiding PID reuse when cleaning descendants. Failed
converter diagnostics retain bounded 64 KiB prefixes with original output sizes
and exit status. Named converter inputs cannot subsequently be mapped. The
100-test diff suite, eleven conversion checks, eleven stream/process checks,
48 existing public authoring/filter/stat checks and strict affected Clippy pass.
The five-case public cgroup matrix covers 20/64/256 MiB sparse comparisons with
both storage representations forced by existing allocation settings, diff-format
reads, finite expanded NUL-bearing textconv output, a converter that exceeds
scratch, and real SIGINT after the
converter starts. Controlled return is under one second; HEAD/index survive,
patch stdout stays empty on refusal/interruption, and owned scratch is reclaimed.

Story-harvest stations: **comparison resources / between conversion → algorithm /
byte storage changes preserve conversion decisions**, and **owned converters /
between output growth or interruption → command return / both output pipes share
the quota and descendants cannot retain capture pipes**. Existing algorithms
remain unchanged. Remaining cache consumers and merge-driver output remain in the open audit.

Loose blob verification now reuses the bounded loose-header reader and hashes
fixed chunks, validating both the advertised length and Git object identity.
Non-blob objects retain structural decoding and configured allocation limits.
Corrupt length/hash fixtures and the existing cancellation contract pass. Pack
delta-tree allocation refusals now name the operation, pack offset, requested
bytes and configured limit, retaining distinct allocation-failure classification.
Two former delta-size tests stopped at the instruction buffer; they now reach
the declared base/result checks they claim to exercise. All 95 ODB and 158 pack
tests and strict Clippy for both crates and the CLI fixture pass.

`memory_object_store` exercises both verification algorithms and all three
verification modes, `fsck`, ODB entries/info/header statistics, pack creation,
index creation, Git pack verification and verified loose extraction. Its 64 MiB
append fixtures inspect the pack header to prove OFS and REF representations.
The successful large-pack matrix uses an explicit 1 GiB allocation setting.
The separate 16 MiB case proves successful loose streaming, accurate packed
refusal and unchanged HEAD/index; it does **not** claim successful bounded
packed verification. No algorithm, worker count or cache setting was changed.
Scratch peak and complete metadata/reduced-trust qualification remain open.

Story-harvest station: **object integrity / between object enumeration → verified
identity / blob validation streams without weakening metadata validation or
conflating configured limits with corruption**. The probe is
`streamed_blob_verification_preserves_limits_and_rejects_corruption` and the
public `memory_object_store` fixture.

Blame now streams initial and historical blobs into immutable operation-owned
mappings and uses the same bounded comparison conversion path. Rename similarity
receives that loader too, preserving its matching order, scores and tie-breaking.
The existing buffered API remains available. All 39 Git-oracle journeys compare
buffered and mapped outcomes, statistics and paths; both hash suites pass (61
unit, 39 integration and one documentation test each). The 101-test diff suite
and strict affected-crate Clippy pass. Rendering walks borrowed line slices,
avoiding a second whole-file line collection; disjoint ranges, CRLF, invalid
UTF-8 and the final unterminated line preserve the existing byte-display behavior.

The four public blame cases include 64 append-only revisions, a million unique
short lines, actual ownership-derived reduced trust, loose/packed objects,
rename similarity, dirty worktrees and an actual bare clone. The initial run
passed three cases but the synthetic nested `.git`-as-bare subsection failed
repository discovery before blame; the real bare-clone policy case passes in a
separate fixed-ceiling receipt. That nested-directory discovery issue is retained
as an unrelated compatibility observation, not counted as successful coverage.
An accidentally empty exact-name filter and an intentionally stopped old-fixture
run are excluded from acceptance. No changed executable was accepted.

Story-harvest station: **history attribution / between blob selection → rename
similarity and rendered lines / all blob readers honor the same storage policy**.
The explicit 16 MiB fixture first failed at the initial blob, then at rename
similarity; both now pass without changing attribution algorithms. Large history
metadata, whole-operation scratch proof and final source qualification stay open.

Object-backed merge resources now use streamed conversion with buffered or
immutable mapped storage selected by the same allocation policy. File, tree,
commit and recursive-base entry points retain the original algorithms, matching
order, conflict decisions and large-file thresholds. Tree merges reclaim completed
rename comparisons and blob inputs/output after publication to the object store;
the existing buffered low-level APIs retain their reuse behavior. Repository
stream forwarding preserves the synthetic empty-tree behavior of ordinary lookup.
The pipeline also retains its historical acceptance of raw object bytes under a
caller-supplied blob mode, rather than introducing an unrelated type restriction.

Existing merge and conversion suites compare buffered and mapped results,
including tree IDs, conflicts, merge bases and existing Git compatibility
exceptions. SHA-1-only Cartesian checks retain that scope; the other fixtures
run with both supported hashes. The public cgroup fixtures compare file bytes
and complete tree identities with Git at 20/64 MiB, on loose/packed inputs,
with forced buffer/mapping policies and actual reduced trust. A million-short-line
fixture checks both repetitive and unique content through those same public merge
forms and storage policies. Real SIGINT during
64 MiB input spooling returns within one second, preserves HEAD/index and
reclaims scratch. Blocking OS calls remain outside that latency claim. The
35 existing public commit/merge, pull and rebase journeys pass; strict affected
Clippy, readiness and declared Rust discipline pass. External merge-driver
output, replay/conflict payload retention, scratch telemetry and full metadata
qualification remain open.

Story-harvest station: **content merge / between object selection → merge input /
storage policy applies equally to worktree and object-backed resources**.
The three-sided filesystem fixture had passed while the same object-backed
inputs refused at 16 MiB; the new resource fixture guards both entry paths.

### Replay recovery payloads and retained copies

A 20 MiB public rebase exceeded the 1 GiB cgroup limit before this repair:
three OOM kills, exit 137, 87.02 s. Its `state.json` contained 503,360,857 logical
bytes. The journal repeated working-file images as nested JSON byte arrays.
Replay version 2 licenses immutable raw payload identities before publication;
worktree checks stream hashes and transition records refer to those payloads.
Version 1 inline journals still open without a rewrite and can advance or abort.
Conversion uses the existing streamed checkout pipeline. Closure verification,
preservation comparisons, retained-copy capture and inspection stream blobs;
retained manifests, checksums, object identities and completion receipts keep
their existing format. Cleanup hashes payloads without loading their contents.

The public Git-oracle fixture checks full resulting trees and parentage, staged
and unstaged neighboring content, reclaimed active journals and `fsck`, using
loose objects at a 1 GiB allocation setting and packed objects at 16 MiB.
The repaired 20 MiB case passes in 77.40 s with a 553,140,224 B whole-group peak;
64 MiB passes in 223.02 s with a 1,073,741,824 B peak. Both have zero OOM kills,
zero surviving processes and stable executable identities. The observed 20 MiB
state file is about 57 KB. These receipts precede the final cleanup-reader
refinement; committed-source qualification is still required. Scratch peak is
unmeasured, so neither receipt closes full acceptance.

The ordinary native authoring/replay run passes 117 tests; the subsequent
cleanup run passes 36 core/power-loss, 54 replay and 11 retained-copy tests.
These include legacy opening, scratch exhaustion and payload corruption,
missing-file, symlink and foreign-member refusals. A packed 20 MiB case with actual reduced trust also passes (333,959,168 B,
39.12 s, no OOM or surviving processes). Its foreign-owned worktree triggers
reduced trust while private Git metadata retains the acting user's ownership;
no trust override or allocation setting is supplied. The fault-enabled run
passes 33 authoring, 69 replay, 28 interactive, six I/O and twelve preservation
matrix tests; the cleanup refinement additionally passes 70 replay and six I/O
tests. The shared public switch/pull/rebase run passes 42 tests. Growing-source
and collision-fixture tests preserve the existing recovery semantics: raw SHA-1
collision detection must use the existing tagged Git-blob fingerprint fallback,
without weakening actual Git object validation. Strict affected Clippy, readiness
and declared Rust style pass for this slice. Further recovery work removes two measured copies. Owned conflict children
reuse their parent's verified raw before-images instead of encoding the file
again in both the child recipe and state. A 64 MiB conflict/abort fixture that
OOM-killed three processes now passes at 277,876,736 B in 132.80 s. Preserved
staged/unstaged neighboring files also use licensed raw custody; their public
64 MiB fixture changes from three OOM kills to a 520,847,360 B pass in 239.57 s.
The 64 MiB conflict continuation initially refused its clean check at the
explicit 16 MiB allocation limit. Reusing streamed canonical hashing and staging
now completes it at 277,880,832 B in 135.54 s. These passing receipts have zero
surviving children and unchanged executable identities. The full authoring/replay/interactive/I/O fault run passes 140 tests;
expanded preparation SIGINT/SIGKILL checks preserve both neighboring versions.

The original 503 MB version-1 journal now restores its original HEAD, index,
branches and working files at 721,387,520 B in 230.69 s, without OOM. Abort
licenses raw payloads before publishing them, streams its outer JSON envelope,
then replaces inline images with verified descriptors. Existing completed copies
remain intact; interrupted normalization accepts only its exact licensed or
completed successor. The resulting state is 46,921 B with a 20 MiB raw payload.
Cleanup correctly refuses the three unproven owner-only transition directories
left by prior OOM failures. This is bounded restoration with an explicit cleanup
refusal, not an exit-zero required-success qualification. The failed receipts
and original journal remain isolated evidence. Four legacy tests, including
process death at normalization boundaries, and the full lifecycle power-loss
model pass; that model is one test covering its existing 32 failure shapes.

Process-death qualification then exposed a child handoff bug: publishing a new
payload inventory changed the exact parent context while the live child still
required its old binding. Per-payload immutable licenses now bind the exact
active child, leaving that parent state unchanged until acceptance incorporates
the inventory. Accepted licenses must match a durable accepted-child binding.
The eight death cases cover publication before/after raw bytes, continuation
and abort. Restaging or deletion cannot change an already pinned proposal;
continuation refuses, and abort retains the changed work before restoration. Six foreign/corrupt/oversized license cases refuse
without mutating the operation. All 110 fault-enabled replay, interactive and I/O
tests pass, along with strict affected Clippy, formatting, readiness and the
declared Rust-style check. The same public 64 MiB conflict continuation
passes with these licenses at 277,864,448 B in 142.69 s, without OOM or survivors.

The 64 MiB edit/amend fixture next refused authored-object retention at the
explicit 16 MiB allocation limit (177,242,112 B peak, 51.15 s). New authored-object
receipts store a canonical Git loose header and raw body, using the bounded
object reader, streamed identity verification and charged private publication.
Traversal decodes only commits and trees; blobs are released after each receipt.
Restoration streams blobs back into the object database. Legacy checksummed JSON
receipts remain readable, including leading whitespace. Interrupted `.new` files
remain opaque retained evidence and cannot authorize cleanup. Accepted-child
validation streams object verification instead of loading every blob again.
Legacy decoding reads the already checked file handle with interruptible buffered
readers, avoiding another whole-file copy and a path reopen.

The public 64 MiB edit/amend fixture now passes in 143.31 s with a 209,580,032 B
whole-group peak, no OOM, no surviving children and stable executable identities.
It verifies authored content and parentage, staged/unstaged neighboring edits,
active journal cleanup and Git fsck. All 145 native authoring/replay/interactive/
I/O fault tests pass. Raw and legacy proposal receipts restore the exact commit
after real Git pruning without repeating hooks; corrupt content, lengths, headers,
identities, permissions, symlinks and oversized scratch refuse without changing
integration. The resource receipt predates only the legacy-reader refinement;
final committed-source qualification remains required. The final reader refinement
passes both authored-object tests and the three-shape GC recovery test, strict
affected Clippy, formatting, readiness and the declared Rust-style check.

Edit-scope expansion then reproduced the same 16 MiB refusal when reading a
20 MiB original blob (72,527,872 B peak, 5.80 s). Owned expansion now retains
both the current file and its verified original bytes through the existing exact
child payload licenses. The child scope decision records raw descriptors before
staging; the parent context remains unchanged. Legacy child state advances to
version 2 when raw images are added. Existing conversion refusals and original
worktree fingerprints retain their semantics.

The 20 MiB public continuation/abort fixture passes at 89,526,272 B in 79.23 s
with no OOM or survivors, including original restoration, staged/unstaged
neighbors and independently retained authored bytes after Git pruning. Eight
process-death cases distinguish a published payload from an accepted scope:
before acceptance, generic recovery refuses the changed outside-scope file and
explicit restaging or preserving abort succeeds; after acceptance, abort restores
the original and preserves the authored bytes. Fourteen existing scope cases
pass. The final fault-enabled authoring/interactive run passes all 64 tests;
strict affected Clippy, formatting, readiness and declared Rust style also pass.
The resource receipt precedes the added interruption checkpoints.

Large metadata, aggregate scratch accounting and final committed-source resource
proofs remain open. None of these memory receipts measures aggregate scratch
peak, and the overall habit remains red.

Story-harvest station: **replay / between retained history → recovery journal /
file custody must remain bounded independently of comparison storage**. A
bounded merge alone did not prevent the journal from exhausting memory.
A separate custody station checks **raw payload → cleanup witness**: Git-blob
identity and an unframed SHA-1 file digest are different checks. The existing
collision fixture failed cleanup before reusing the tagged fingerprint fallback,
then passed the full replay/cleanup path.

### Large ref, config, mailmap and tag metadata

The metadata fixture independently crosses 16 MiB with 200,000 config entries,
80,000 packed refs, 200,000 mailmap mappings and a 20 MiB annotated tag. Output
goes to files and is compared in fixed chunks. Config values and branch/tag
ordering use stock Git with fixed options; the branch oracle strips exactly
two ref components so Git's ambiguous-name shortening does not change the
comparison. Brit's annotated-tag detail format remains its existing format.

This found two additional affected readers. `mailmap.blob` used a complete
object buffer and could print an empty mapping after a 16 MiB allocation
refusal. It now uses the existing verified object stream and immutable mapping;
worktree and configured-file buffers are released after each source. Mailmap
JSON entries serialize one at a time, preserving source precedence, partial
source errors and worktree-symlink protections. Error reports retain their
causes. Annotated tags previously lost their name/signature details silently
when their object buffer exceeded policy. Tag listing now reads headers for
lightweight tags and streams annotated objects through the existing parser,
releasing each mapped body after extracting its fields. Version sorting is
unchanged; read/decode errors now propagate instead of erasing metadata.

Seven metadata cases pass for each of SHA-1 and SHA-256, including actual
ownership-derived reduced trust and explicit allocation settings. The exact
1 GiB, swap-disabled workers recorded 234,872,832 B / 306.998 s and
234,881,024 B / 339.768 s, respectively, with no OOM kills, surviving children
or executable changes. Main executable SHA-256:
`6297f6a8037e6e695c17e860c489be19b96f2958870274920fa572083f713cd4`.
These receipts precede the final buffered/cancellable mailmap JSON writer;
its separate follow-up is recorded below. Four native mailmap checks preserve
source precedence, one-byte allocation policy and symlink behavior; the two
existing tag-version checks also pass. The public fixture is selected by CI.
Aggregate scratch remains unmeasured, and these are not full acceptance
receipts. Private evidence: `bounded-memory/metadata-*` and `mailmap-*` in the
existing isolated readiness directory.

The final buffered/cancellable JSON writer passes both mailmap cases again in
each hash format: 160,911,360 B / 90.921 s (SHA-1) and 160,546,816 B /
108.641 s (SHA-256), with zero OOM kills, surviving children or executable
changes. Main SHA-256:
`cea8f8f592f46a1422484a99459c07594d032e391e3358127c0bb8e38bb7f555`.
Strict affected-crate Clippy, formatting, readiness contracts and the declared
Rust-style checker pass. Complete committed-source qualification and scratch
telemetry remain open.

### Build fingerprints release completed inputs

The pre-repair `build fingerprint` fixture with three 256 MiB inputs was killed
by the 1 GiB cgroup (three OOM kills, 17.321 s). The graph reader had cloned
each selected blob body and retained the collection until hashing. It now uses the
shared verified object stream and hashes each input in bounded reads, retaining
only paths and CIDs. The existing sorted path/NUL/CID/NUL contract, glob and
executable selection, and symlink/gitlink exclusions remain unchanged.
`brit-graph` and the CLI now consume the local reader instead of gix 0.81;
the graph crate's declared minimum follows that dependency at Rust 1.88, while
the build and checker remain Rust 1.100.0.

The repaired SHA-1 matrix passes four cases in 507.231 s: 20 MiB, three 256 MiB
inputs, 512 files totaling 32 MiB, and actual ownership-derived reduced trust.
Loose/packed objects and default/explicit 16 MiB allocation settings produce
independent golden CIDs; revision plans agree and HEAD/index remain unchanged.
The SHA-256 matrix passes five cases in 467.787 s, including the compatibility
control below. Both workers peak at 1,073,741,824 B with page reclamation,
zero OOM kills, zero surviving children and unchanged executable identities.
Main SHA-256: `9836861fd8ac04b4857cff5fefb0bce9b22eda5f3e922fc1700ca78aa83eebce`.
Scratch peak remains unmeasured; these are not full acceptance receipts.

Compatibility exception: revision-based `build plan --since` and
`build affected --since` still use published Rakia's gix 0.81 reader, which
cannot open SHA-256 repositories. An empty-repository control independently
reproduces both refusals with the pre-repair executable; the SHA-256 fixture
asserts that existing limitation rather than counting it as memory success.
Fingerprinting and explicit-file affected selection pass in SHA-256. The
initial assumed reduced-trust ownership refusal was disproved by the old
binary: reduced-trust planning succeeds and is tested as such in SHA-1.
The 77 native CID/graph checks compare buffered/streamed boundaries, propagate
read failure and preserve selection/ordering. All 36 CLI tests pass with
backtraces disabled, matching the existing diagnostic fixture contract; strict
affected-crate Clippy, formatting, 12 readiness contracts and the declared
Rust-style checker pass. Public fixtures are selected by CI. Private receipts are `bounded-memory/build-*` and
`fingerprint-*` in the existing isolated readiness directory. The stopped
intermediate mapped-path run is interruption evidence only. Final committed
source, broader build metadata, rename-change detection and whole-operation
scratch proof remain open.

### Packed verification preserves both traversal algorithms

Packed verification formerly refused a 20 MiB blob at the explicit 16 MiB
allocation setting. The existing lookup and delta-tree traversals now use
charged immutable spools when their buffered decoder reaches that configured
limit. Actual allocator failures retain their own diagnostic. Delta application
shares the existing streamed object-reader implementation; traversal selection,
worker/cache controls, object checksums, CRCs, decode/re-encode modes and
statistics remain unchanged. Completed mapped bases release their reservations.
Object hashing now checks cancellation between bounded reads.

The explicit-limit matrix passes 20/64/256 MiB loose and packed objects, including
64 MiB OFS and REF histories, under both hashes: SHA-1 peaks at 571,211,776 B
in 54.875 s; SHA-256 at 547,274,752 B in 401.415 s. Actual ownership-derived
reduced trust passes default and explicit-limit 20/64 MiB cases under both
hashes (343,433,216 B / 55.820 s and 339,886,080 B / 386.735 s). The ordinary
SHA-1 matrix, including pack create/index/explode, passes at 822,583,296 B
in 96.401 s. Every successful worker reports zero OOM kills, surviving children
and executable identity changes. Main SHA-256:
`359b1590b8c544d97a6e8a16b16c6454ca7c085c29889a2176580f77cdb6d759`.

The complete ordinary SHA-256 pack round trip exposed a pre-existing writer
bug: `free pack create` used a SHA-1 trailer regardless of repository format.
It now takes the repository's hash kind. All four 20/64/256 MiB cases pass,
including pack creation, indexing and verified explosion (845,455,360 B,
521.660 s; zero OOM or surviving children). Main SHA-256:
`85f135b15680de38fc7da77711227e4261478c7c9c63c3f9a572987c7f88373b`.
The standalone fixture now supplies the existing `--object-hash` option and
places temporary output destinations inside its own repository. Plain output
directories under the shared checkout had discovered its parent writer guard;
concurrent refusals are retained as fixture-isolation failures, not memory proof.
The isolated rerun passes all four SHA-1 cases at 820,572,160 B in 97.495 s
and a concurrent SHA-256 20 MiB control at 73,469,952 B in 23.073 s.

Native fixtures compare both algorithms, all decode modes and zero/16-byte
buffer limits against buffered checksums and statistics. They distinguish
corruption, cancellation and scratch exhaustion and verify reservation release.
Both serial and parallel builds preserve their existing traversal ordering.
Private receipts remain in `bounded-memory/pack-*`. Aggregate scratch telemetry,
final committed-source qualification and the remaining command audit stay open;
these memory receipts do not establish full acceptance.

### Large commit metadata and external merge qualification

A 20 MiB commit message made `log` refuse at the explicit 16 MiB allocation
setting and at actual reduced trust. The topological walker now permits
operation-owned immutable byte storage while retaining the same commit parser,
queues, graph-cache behavior, ordering and parent selection. Ordinary object
stores keep their existing buffered implementation. The CLI selects the shared
verified stream/mapping above its effective allocation policy and releases each
rendered commit; it no longer clones the title or loads an unrelated index.
Symbolic references use the existing reference-following routine before tag
peeling, so they do not force a buffered terminal commit read.

The initial repaired two-case matrix passes both hashes, loose/packed objects,
default/explicit allocation settings and actual reduced trust: 69,386,240 B
in 8.050 s (SHA-1) and 49,311,744 B in 52.452 s (SHA-256). The final SHA-1
matrix adds a symbolic-reference chain and passes at 69,406,720 B in 10.947 s.
The final diagnostic build also rejects a large blob at HEAD using its header,
naming the object without an allocation-limit error and preserving HEAD/index.
Both complete cases pass again at 54,587,392 B in 11.414 s and 65,085,440 B
in 75.954 s. The executable SHA-256 is
`a10b284b0764d72106b0f359edb7c19f96e34625bc7ed48167627c5d13341314`;
receipts are `commit-metadata-diagnostics-sha1` and `-sha256`. All 42 native
traversal checks pass per hash, the four public log feedback checks pass, and
strict affected-crate Clippy passes.
Every successful worker has zero OOM kills, surviving children and executable
identity changes. Native traversal fixtures compare mapped and buffered input
with and without commit graphs, including date/topological ordering and
first/all-parent selection. Private receipts are `bounded-memory/log-*` and
`commit-metadata-*`; aggregate scratch telemetry and complete history metadata
qualification remain open.

External merge drivers required no algorithm change for the declared three-sided
20/64 MiB fixtures. A real Git merge-file child runs for every loose/packed,
1 GiB/16 MiB allocation case; an invocation marker prevents accidentally
qualifying the builtin driver. Output hashes match Git and HEAD, index and input
files remain unchanged. Both hashes pass under the whole-group limit:
749,256,704 B in 25.553 s and 748,777,472 B in 83.258 s, zero OOM or survivors.
The fixture uses repository info attributes because object-only merges do not
read untracked worktree attributes. The failed initial setup remains private
evidence, not acceptance. External temporary-file accounting and interruption
still belong to the open scratch/recovery qualification.

### Large tree metadata qualification

A 240,000-entry tree (26,400,000 bytes in SHA-1, independently of its one-byte
blob) refused while peeling at the 16 MiB reduced-trust allocation limit.
Tree and commit readers now share the same immutable object buffer and verified
stream/mapping implementation. Revision peeling uses headers for terminal kinds;
supported revision path lookup retains the existing sequential tree parser.
Breadth/depth traversal retains its ordering and reusable ordinary buffers, while
completed mapped trees release their reservations, including borrowed traversal
state. Listing uses bounded output buffering and propagates output exhaustion;
recursive listing previously discarded those errors.

The initial repaired two-case matrices pass direct/recursive entries, extended
sizes, statistics and revision path lookup in loose/packed storage, full/actual
reduced trust and default/explicit 16 MiB policies. Whole-group peaks are
82,411,520 B in 301.678 s (SHA-1) and 66,682,880 B in 360.333 s (SHA-256), with
zero OOM, surviving children or executable changes. Native traversal passes 43
checks per hash, including mapped/buffered order and reclamation; all 49 revision
parsing checks pass. The broader gix suite passes 518 tests, with its existing
macOS Unicode-precomposition test ignored on Linux. A focused output-fault test
requires the original I/O error during rendering and final flush. Strict affected
Clippy passes. The final reader's combined five-case matrix includes tree,
commit-message and annotated-tag regressions; SHA-1 passes at 83,173,376 B
in 251.098 s and SHA-256 at 66,760,704 B in 380.948 s. Both hash runs pass
without OOM or surviving children; full receipts
are `tree-final-metadata-sha1` and `-sha256`, executable SHA-256
`9bcab966a6bd53dd509ebe790ff130e33c669d1653c74e06d44318b93f068e66`.

The first fixture also reproduced an existing syntax exception:
`tree entries HEAD:branch` appends `^{tree}` to the path and fails without
resource pressure. It is not a memory failure or a successful fixture. The
success matrix instead uses the existing direct-tree and recursive-HEAD modes,
and tests `HEAD:branch` and a leaf path through supported `revision resolve`.
Private receipts remain in `bounded-memory/tree-*`. Aggregate scratch and the
remaining command audit stay open.

### Reference peeling under allocation policy

Reference resolution now reads terminal object headers and uses the shared
immutable reader for annotated tags. The reference store retains symbolic-chain,
packed-ref and cached-target behavior; missing-object diagnostics retain their
classification, object ID and reference name. Existing buffered plumbing callers
retain their API. Both single-reference and iterator peeling use this path.

Two 20 MiB commit cases per hash pass direct/symbolic `revision resolve`, log
parity, loose/packed objects, default/explicit 16 MiB policy and actual reduced
trust. Peaks are 70,705,152 B / 52,891,648 B in 11.522 / 73.440 s. Two large
annotated-tag cases per hash also pass symbolic peeling and listing parity at
71,557,120 B / 70,754,304 B in 5.711 / 22.056 s. All four whole-group runs have
zero OOM kills, survivors or executable changes. The executable SHA-256 is
`d1b9d73aea26a844bc841e51da35e219cddf88b7b94ae56e77e78c974a5f5f3e`;
private receipts are `bounded-memory/ref-peel-*`. Native coverage passes 174
reference-store tests (two existing ignores), 49 revision tests and the broader
518-test gix suite (one existing platform ignore). Strict affected Clippy,
formatting, readiness contracts and the declared Rust-style checker pass.

This closes the shared reference reader, not Tix's remaining commit readers.
Aggregate scratch and complete-command qualification remain open.

### HEAD metadata in status and discovery

Tix's expanded 20 MiB fixture exposed two additional shared status prerequisites:
HEAD-to-tree lookup and submodule discovery both buffered the commit body. The
second is exercised even when `.gitmodules` is absent. These paths now reuse
the shared immutable reader; submodule lookup also uses bounded tree traversal
and reads the selected config object through that reader. Missing-index
reconstruction uses the same HEAD-tree helper. Detached tag-backed HEADs reuse
the existing reference peeler, retaining terminal IDs without reading their bodies.
Wrong-kind HEADs fail from the header with their object ID and actual kind.

The shared gix suite passes 519 tests with one existing platform ignore. Eight
focused HEAD checks include a one-byte object policy, all four HEAD attachment
shapes, wrong-kind diagnostics, missing objects, and missing-index/submodule
fallbacks. The latter opens a fresh handle so an existing reusable buffer cannot
mask an allocation-policy failure. Strict affected Clippy passes. The expanded
Tix cgroup runs and Tix-specific readers are recorded separately when complete;
these native checks do not establish whole-command resource acceptance.

### Tix history browsing

Tix history traversal, incremental graph refresh, row metadata, change/patch IDs
and internal-commit checks now use the shared immutable object reader. Completed
walks release their byte buffer before edit-metadata inspection. Commit-graph
selection, topology, hidden boundaries, tie-breaking and metadata parsing remain
unchanged. Internal rebase detection no longer turns read failures into an
ordinary selectable commit; corruption has a native regression check.

The public fixture uses a 20 MiB commit with TODO and checks-passed enrichments.
It compares `tix show`, `ref-tree`, `worktrunk show` and `op log` byte for byte
against buffered results in both hashes, loose/packed storage, full/actual
reduced trust and default/explicit 16 MiB policy. HEAD, index and all refs remain
unchanged. This exercises a large history commit with small enrichment/undo
metadata, not a large recovery journal. Two cases per hash pass under the fixed
whole-group cap: SHA-1 peaks at 80,793,600 B in 42.405 s; SHA-256 at 53,211,136 B
in 192.801 s. Both have zero OOM kills, survivors or executable changes.
The executable SHA-256 is
`a3424ad7a9bcddd615d4ba9b578211e570bde97b3a0b2894861b9e86f466a893`;
receipts are `bounded-memory/tix-status-repaired-sha1` and `-sha256`.

All 804 Tix native tests pass. Shared-reader native checks, strict affected
Clippy, formatting, readiness and declared style pass as recorded above.
Worktree workers reopen repositories with default options, so propagation of
CLI-only overrides remains an explicit audit item; the ownership-derived
reduced-trust case forces their bounded read path independently. Editing, large
notes/journals, interactive loads, aggregate scratch and final-source acceptance
remain open.

Story-harvest station: **history browsing / between graph metadata → populated
worktree table / status prerequisites must also read bounded metadata**. The
history view passed while status still buffered HEAD through submodule discovery,
even in a repository without submodules. The public `memory_tix` fixture and
native HEAD fallback tests now observe both stations.

### Scratch classification and composed budgets

The quota regression reproduced a missing classification: `ScratchLimit` retained
its diagnostic through an I/O wrapper, but `is_resource_exhausted()` returned
false. It now supplies a transparent `ScratchLimit` resource marker, distinct
from configured allocation limits, allocation failure and filesystem
`StorageFull`. Requested bytes, live bytes, fixed limit and scratch path remain
in the original diagnostic, and typed recovery still finds the concrete error.
The error suite and the original sparse-file quota/reclamation check pass.

Brit and its retained-copy helper initialize a shared process account before
starting workers. Every subsequently created `Budget` shares its reservations
and peak, including budgets created for different directories on worker threads.
Initialization is idempotent and cannot reset existing charges. Library users
retain independent operation budgets unless they opt into process sharing.
Thirteen stream/process checks pass, including a fresh-process regression in
which two workers consume the whole quota, a third refuses before growth, and
releasing the workers permits new writes without erasing the peak.

The immutable executable `b68a08e9875d182ad98f8f82b1cebca7698518deb5b392f0140dc4e39c921b63`
passes five comparison cases per hash (20/64/256 MiB, converter expansion refusal
and interruption), seven SHA-1 merge cases including external drivers and a
million lines, and the 64 MiB replay case preserving staged and dirty neighbors.
SHA-1 comparison, merge and replay peaks are respectively 1,073,741,824 B,
750,718,976 B and 528,121,856 B in 47.140, 128.234 and 229.406 seconds. SHA-256
comparison peaks at 1,073,741,824 B in 208.260 seconds. Every run
has zero OOM kills, surviving children and executable changes. Receipts remain
in the existing private `bounded-memory/scratch-process-*` directories.
Strict affected Clippy, 24 CLI checks, readiness and the declared Rust-style
checker pass. The CLI diagnostic assertion requires backtraces disabled; the
initial environment-sensitive failure is retained with the corrected run.

This accounts for budget-managed storage within one process. It does **not**
establish aggregate child-process storage, complete durable-journal inventory,
external-driver temporary files or qualification scratch telemetry. Those remain
open, and existing resource receipts still report `scratchPeak: null` and
`qualificationComplete: false`.

Story-harvest station: **bounded reader → composed command / every reader's
budget must participate in the same quota**. A per-reader limit alone does not
bound their simultaneous total. The isolated worker regression measures this
station; whole-operation scratch proof remains the next station.

### Revision listing and parent lookup failures

`revision list --long-hashes` independently reproduced the 20 MiB commit refusal
at explicit 16 MiB and actual reduced trust. Its starting point now uses bounded
peeling, and the simple revision walker reads through the shared immutable
storage contract. The same queues, date ordering, cutoff, first-parent and
hidden-frontier algorithms remain in place. Hidden-frontier graph lookups accept
the same storage, with existing buffered callers retaining their ordinary path.
Shared missing-object and wrong-kind diagnostic helpers avoid divergent messages.

The audit also reproduced a silent failure: a parent lookup refused by an
allocation limit became timestamp zero, so a date cutoff could omit that branch
and return success. Parent lookup/decode failures now propagate instead. A
regression checks allocation limits, allocation failure, scratch exhaustion,
cancellation, corruption, missing objects and permission errors; malformed parent
diagnostics identify the commit. Valid-input ordering and tie-breaking are unchanged.

All 44 traversal checks pass in both hash formats, including buffered/mapped
comparisons with and without commit graphs and reclamation after hidden-frontier
walks. The repository suite passes 520 cases with one existing platform-specific
ignore; 174 object tests and two object doc tests pass. Strict affected Clippy,
formatting, readiness and the declared Rust-style checker pass. Private evidence
is under the existing `bounded-memory/revision-*` names.

Two public commit-metadata cases per hash pass with the final executable
`c9727cd935d00d36290f6a063f7d746aa53d4ff5de0a795ba3c04fd500a20df4`:
SHA-1 peaks at 71,704,576 B in 13.901 s and SHA-256 at 49,831,936 B in
91.882 s. Loose/packed, default/explicit and actual reduced-trust paths agree
with the fixed Git oracle. Both cgroups have zero OOM kills, surviving children
and executable changes. Aggregate scratch remains unmeasured in these receipts.

This qualifies the text listing metadata path, not all revision operators.
Merge-base's separate starting-object and graph readers, large history metadata
in other consumers, SVG/owned-viewer behavior, aggregate scratch and full final
qualification remain open.

Story-harvest station: **bounded history → date-filtered history / failed parent
reads must refuse rather than fabricate a sorting timestamp**. The forced-error
regression observes the omission that otherwise looked like successful filtering.

### Shared merge-base metadata and lazy scratch

The separate `merge-base` starting-object reader reproduced the 20 MiB commit
refusal at explicit 16 MiB and actual reduced trust. Starting-point peeling now
uses headers where possible, and `Repository::revision_graph` uses the shared
bounded object source. Pairwise, many-head and octopus algorithms, commit-graph
caching and replacement handling remain unchanged. The source defers policy
lookup until bytes are needed and scratch setup until mapping is required;
clones share that deferred budget. Missing scratch reports the affected object.

The repository suite passes 521 cases with one existing platform ignore. A new
native check compares buffered and mapped pairwise/many/octopus results and graph
reuse in both hashes. Unix public tests prove small queries succeed with an
unavailable `TMPDIR`, while forced mapping reports the filesystem failure.
Strict affected Clippy, formatting, 12 readiness checks and the declared style
checker pass.

The two public 20 MiB metadata cases per hash pass loose/packed, default/explicit
and actual reduced-trust paths with executable
`a7aeeab8dca5384c3f045420e3a9430ad22e85e43acf92e9416a02602c1f8506`.
Whole-group peaks are 50,843,648 B in 15.403 s (SHA-1) and 49,573,888 B in
101.925 s (SHA-256), with zero OOM kills, surviving children or identity changes.
Private receipts are `bounded-memory/merge-base-*`. Aggregate scratch is still
unmeasured; these receipts do not establish complete command qualification.

Story-harvest station: **revision graph construction → first mapped read / defer
scratch requirements until the operation actually needs temporary storage**.
Parent/ancestor and message-search revision operators, shallow-walk error
handling, other metadata consumers and final-source qualification remain open.

### Bounded revision expressions

Revision-expression follow-up: `HEAD^` independently failed on the 20 MiB commit
at explicit 16 MiB and actual reduced trust. Parent lookup, object validation,
recursive tag peeling and message search now reuse bounded bytes. Kind-only
disambiguation reads headers, and tree-path peeling defers unused scratch setup.
The existing parent/date ordering and exact message-boundary/matching semantics
remain in place. Ancestor lookup no longer discards read errors while counting;
message search stops on a failed read rather than returning a later match.
Missing-ancestor tests retain both the not-found classification and object ID.

All 523 active repository cases pass (one existing platform ignore), including
the buffered/mapped expression checks; both new checks also pass with SHA-256.
Existing malformed-object snapshots now name the header read that discovered the
same corruption. Strict affected Clippy, formatting, readiness and declared style
pass. Four public commit/tag cases per hash compare with Git for loose/packed,
default/explicit and actual reduced-trust paths. With executable
`7ba2a2f22008573acf0c4709268c67b7b7646b0e8e37796408eadcc8e96c9c27`,
SHA-1 peaks at 115,494,912 B in 38.874 s and SHA-256 at 91,435,008 B in
221.610 s, with zero OOM kills, surviving children or identity changes.
Private evidence is `bounded-memory/revision-expressions-*`; aggregate scratch
is still unmeasured. Ambiguity diagnostics, reflog/shallow-walk error handling,
SVG/viewer behavior and other metadata consumers remain under audit.

Story-harvest station: **revision expression → usable ancestry / an unreadable
ancestor must remain a read failure, not become an absent match or range limit**.

### Bounded connectivity and faithful failures

The public `fsck` starting point and connectivity scanner now use bounded object
storage and the existing sequential commit/tree parsers. A failed tree read no
longer becomes a missing-tree report; fallible blob presence checks retain their
I/O failure, and a failed scan clears its visited cache before retry. Missing
objects retain the existing listing behavior. Output exhaustion returns the
original I/O error instead of panicking. Connectivity requires physically stored
empty trees; comparison readers retain their synthetic empty-tree behavior.

The original 20 MiB commit / 240,000-entry tree fixture failed all four cases:
commits refused the allocation limit and large trees were falsely reported as
missing. Six native connectivity cases pass in both hashes, including injected
allocation-limit, allocator, scratch, corruption, cancellation and filesystem
errors. Two output/empty-tree regressions pass in both hashes. All 523 active
repository tests pass (one existing platform ignore), along with the object
suite, strict affected Clippy, formatting, readiness and declared Rust style.

The normal default `max` binary
`e924a4703c516685f23839091cebb7fe80d1451d3a04eb6f1305d1b9e7384e0b`
passes five public metadata cases per hash, including loose/packed objects,
explicit 16 MiB policy and actual reduced trust. SHA-1 peaks at 114,843,648 B
in 275.178 s; SHA-256 at 95,023,104 B in 520.218 s. Both have zero OOM kills,
surviving children or identity changes. Private receipts are
`bounded-memory/fsck-stored-*`. Aggregate scratch remains unmeasured, and these
working-tree receipts do not establish final committed-source acceptance.

Story-harvest station: **connectivity lookup → missing-object report / absence
must be distinguished from failed access, and retries must not inherit success
from an incomplete visited set**. Remaining verification metadata, corpus tasks,
aggregate scratch and the broader command matrix stay open.

### Default corpus and loose metadata verification

The normal default `max` build exposed two corpus failures at the allocation
boundary: SWTR loaded the entire HEAD commit, while VERI streamed loose blobs
but buffered commit/tree/tag metadata. SWTR now uses the existing bounded
HEAD-tree lookup. Loose verification maps metadata above policy, shares the
pack verifier's scratch budget, releases each object's storage and retains the
existing structural parser. Structural parse failures now explicitly report
corruption with the original parser error and object ID. Ordinary buffered
lookup still enforces the configured allocation limit. Task names, algorithms
and existing versioned database records are preserved.

The corpus fixture uses a 64 MiB blob and 20 MiB commit. It checks all four
persisted task outcomes, since command exit alone does not report task failures,
and checks HEAD, index and working-blob identity. Loose/packed, default/explicit
and actual reduced-trust runs pass with both hashes. With default-max binary
`d0963faf35e99aa4dde23f06413eb0a6359baad763ba6e58a13ff5a9b50fd185`,
SHA-1 peaks at 188,452,864 B in 13.118 s and SHA-256 at 186,462,208 B in
65.910 s; neither run has OOM kills, surviving children or identity changes.
An earlier parallel run correctly refused a shared parent-repository lock;
fixture database outputs now live inside each private repository, with no lock
bypass. Private receipts are `bounded-memory/corpus-owned-*`.

All 94 object-database native tests and two doctests pass. The new
buffered/mapped commit, tag and tree checks also pass with SHA-256, including
correctly hashed malformed metadata. Strict affected Clippy, formatting,
readiness and declared style pass. Public commit/tree metadata verification
passes both algorithms and all hash/decode/re-encode modes with one configured
worker, loose/packed objects, explicit policy and actual reduced trust. The five
metadata cases per hash (including the annotated-tag regression) peak at
95,944,704 B / 92,053,504 B in 303.309 / 686.043 s, with zero OOM, surviving
children or identity changes. Receipts are `bounded-memory/verify-metadata-*`.
Aggregate scratch and final-source qualification remain open.

Story-harvest station: **corpus command completion → task success / inspect
persisted task errors and durations before accepting a successful process exit**.

### Bounded ambiguity diagnostics and repository-relative prefixes

Revision ambiguity diagnostics previously retained all complete candidate objects
before sorting, and an explicit allocation limit replaced useful commit/blob
descriptions with lookup failures. They now use headers for blob/tree kinds and
bounded storage for one commit/tag at a time, retaining only descriptions. The
existing kind/ID ordering, title trimming, date fallback and typed lookup errors
remain. Malformed loose-header diagnostics now name the header read.

The SHA-256 regression also exposed a separate compatibility defect: the syntax
parser inferred SHA-1 for short hexadecimal prefixes. The repository delegate
now interprets these bits using the repository's actual hash format, including
odd-length prefixes and 40-digit SHA-256 abbreviations. This does not change
the default enabled hashes or existing repository formats.

All 524 active repository tests pass, with one existing platform ignore and one
explicitly run resource fixture. The new native comparison passes both hashes.
Strict affected Clippy, formatting, readiness and declared style pass. The
20 MiB public CLI fixture preserves the expected ambiguity diagnostic across
loose/packed, default/explicit and actual reduced-trust paths. With binary
`09d0ccb2ea9f129e3b7ebe3acff2448cb7b967d586bdfbf40aaa37fdb465d788`,
SHA-1 peaks at 118,206,464 B in 3.006 s and SHA-256 at 98,127,872 B in
12.114 s, with zero OOM, survivors or identity changes. Private receipts are
`bounded-memory/ambiguity-*`; scratch and final committed-source qualification
remain open.

Story-harvest station: **hexadecimal revision syntax → object candidates / a
short prefix inherits the repository's hash format, and resource policy must
not erase otherwise available candidate descriptions**. Reflog/shallow-walk
handling, SVG/viewer behavior and remaining history consumers stay under audit.

### Streamed reflogs and faithful revision failures

Revision reflog queries now retain read and parse errors instead of skipping
failed entries or reporting an out-of-range index. Reflog presence checks retain
filesystem errors, so a failed HEAD log cannot silently fall back to a branch
log. Previous-checkout resolution follows the current reference without loading
its object; a regression with an unavailable target now matches stock Git
instead of returning the old reflog ID. Controlled cancellation checkpoints
return within one second; blocking OS calls are outside that latency claim.

`revision previous-branches` now reads and emits one entry at a time, preserving
oldest-to-newest order and the existing human/pretty-JSON formats. The collecting
library API remains available and uses the same error-preserving stream. Native
checks compare buffered/streamed parsing, long lines and line endings, exact
output bytes, corruption and output I/O failures. Reverse revision queries keep
the existing 4 KiB reader window and its pre-existing oversized-line refusal;
the new forward reader does not impose that window on branch listing.

All 527 active repository tests pass, with one existing platform ignore and one
separately qualified resource test. The 208 active reference tests, both-hash
reflog/output checks, strict affected Clippy, formatting, readiness and declared
style pass. The public fixture writes 200,000 entries exceeding 20 MiB and
checks Git revision answers, streamed branch ordering, JSON equality across
allocation policies and unchanged HEAD/index/content/reflog. Default, explicit
16 MiB/one-byte settings and actual reduced trust pass in both hashes with
default-max binary
`336264b72c859219e2c3fa32ffb4fb0fd0af90d54ed92294ed691b2245c9978d`.
Whole-group peaks are 16,584,704 B / 13,365,248 B in 74.740 / 77.859 s, with
zero OOM, survivors or identity changes. Private receipts are
`bounded-memory/reflog-stream-*`. Scratch telemetry and final-source acceptance
remain open.

Story-harvest station: **reflog presence → revision resolution / a failed read
is not absence, and current reference identity does not require loading its
target object**. Commit-graph starting-point reads, shallow walks and SVG/viewer
behavior remain under audit.

### Commit-graph consumers

`commit-graph list` previously refused a 20 MiB starting commit at explicit
16 MiB policy. It now reuses bounded peeling and polls cancellation while
emitting results. Repository and standalone graph verification preserve human
output I/O errors and poll cancellation instead of using a no-op processor.
The existing read-only mapped graph, structural verifier and traversal ordering
remain unchanged. A feature-specific Clippy finding in the earlier revision
message helper is corrected by borrowing its predicate in the helper signature;
regex and literal-search behavior are unchanged.

Both-hash native checks cover interruption under one second and output failures
for all three entry points; all 61 active revision tests per hash pass. Strict
affected Clippy, formatting, readiness and declared style pass. The public
listing fixture compares include/range results with Git on loose/packed 20 MiB
commits, at default/16 MiB/one-byte allocation policies and actual reduced trust.
With default-max binary
`8700f09a3d8670e9340fe4e8a90ab4f9a63a0c2ef9d967736614e9ab306b9be1`,
the two cases per hash peak at 151,339,008 B / 149,557,248 B in 9.039 / 43.039 s.

A separate 400,000-entry linear graph has 22,401,100 B of SHA-1 metadata or
32,001,112 B of SHA-256 metadata. Its synthetic IDs exercise structural cache
verification, not object existence. Both public verifiers preserve its checksum
and report the exact commit/parent/generation statistics under explicit settings
and actual reduced trust. Two cases per hash peak at 55,394,304 B / 43,429,888 B
in 9.793 / 36.143 s. All four workers have zero OOM, survivors or identity changes.
Private receipts are `bounded-memory/commitgraph-final-*`; aggregate scratch,
shallow-boundary failure handling and final-source acceptance remain open.

Story-harvest station: **graph cache → history / metadata-only work must not
load large commit bodies unnecessarily, and successful verification includes
successful output delivery**.

### Shallow-boundary failures

A reproduced shallow-boundary read failure was discarded by the walk's boolean
pruning callback. With a commit-graph cache, no later object read exposed that
failure and the walk returned cached history. The existing walker now retains
the first boundary error and ends the iterator after reporting it. Parent
tokens are decoded fallibly instead of silently dropping parse failures; valid
parent ordering and pruning are unchanged. Native checks cover missing objects,
invalid compressed data and malformed parent metadata, with the boundary both
at the starting point and below a valid tip, cache on/off and both hashes.

All 528 active repository tests pass; the expanded shallow checks pass both
hashes, with strict affected Clippy, formatting, readiness and declared style.
Three public cases per hash cover failed-read diagnostics/state preservation
and loose/packed 20 MiB shallow commits at default/16 MiB/one-byte allocation
policies and actual reduced trust. Visible commit IDs match Git. Brit's existing
raw parent-count column is preserved at shallow boundaries; full Brit output
matches across buffering policies and cache settings. With binary
`4299a510098e08856d325041438d34220713886190ca7181efa04f649cec03ab`,
whole-group peaks are 170,958,848 B / 150,106,112 B in 18.272 / 110.739 s, with
zero OOM, survivors or identity changes. Private receipts are
`bounded-memory/shallow-*`; aggregate scratch and final-source acceptance remain
open. These results also rerun the committed full-history/range listing cases.

Story-harvest station: **shallow boundary → cached walk / failure to read a
boundary cannot become either successful cache output or silent pruning**.
SVG/viewer behavior and remaining history consumers are still under audit.

The parent-expression follow-up reproduces Git rejecting a commit with a valid
first parent followed by malformed parent metadata. Brit previously returned
the first parent or described the malformed second parent as out of range.
Parent lookup now validates the parent list while retaining only the selected
ID and count; stored parent decode failures, including shallow boundaries,
are classified as corruption with the affected commit ID. Both hashes pass
buffered and mapped checks, and all 529 active repository tests pass (two
explicit ignores). Strict affected Clippy, formatting, readiness and declared
style pass. Evidence: `bounded-memory/parent-errors-*`; this diagnostic slice
does not constitute a new resource receipt or final-source qualification.

### Public-command audit in progress

The command boundary is `src/plumbing/options/mod.rs` plus
`brit-cli/src/main.rs` and its composed bridge. This table is interim audit
coverage in the existing readiness document, **not final classification**.
Every open row must finish qualified, traced/tested unaffected, or already
unsupported independently of this effort. No open row is accepted as deferred.
The candidate's existing `--cli-surface-json` projection enumerates 131
non-help parser leaves for the normal default `max` build and 129 for `max-pure`,
plus default command modes and aliases. Both enable SHA-1 and SHA-256 repository
support. The extra default leaves are `corpus refresh` and `corpus run`.
Private captures are `bounded-memory/public-surface-default-max.json` and
`bounded-memory/public-surface.json`; the projection explicitly does not attest
behavior or working-tree bytes. The separate legacy `ein` porcelain exposes
`query`; Brit composes the plumbing parser and does not expose that command.

| Public command family | Observed path / remaining proof |
| --- | --- |
| `diff`, `cat`, `add`, `restore`, `archive` | Shared readers repaired as above; large converted archive and clean/status fixtures pass; pinned diff, larger aggregate selections and remaining fault cases stay open. `diff file` and diff-format reads now use the streamed/mapped shared conversion path and pass the added 20/64/256 MiB matrix. Other comparison-cache consumers remain open. |
| `status`, `is-clean`, `is-changed`, `index`, `dirwalk`, `clean` | Hash-only status paths stream ordinary files; filtered status and a 200,000-entry index crossing 16 MiB are measured. Other metadata, dirwalk and clean paths still need complete tracing. |
| `worktree`, `clone` checkout | Existing bounded checkout, OFS/REF delta and filter machinery retained. Whole-cgroup 64/256 MiB worktree checkout/recovery passes; 64/256 MiB local clone/fetch/push also passes; other protocols and reduced-trust transfer paths remain open. |
| `switch`, `pull`, `rebase`, `merge`, `commit` | Switch/pull raw recovery payloads and streamed dirty checks are repaired and measured at 20/64/256 MiB. Replay working-file journals now use raw payloads and retained-copy capture streams, with 20/64 MiB public cases passing; 64 MiB conflict/abort and preserved large edits also pass after raw-custody repair; interrupted conflict handoff preserves exact child authority; 64 MiB authored-object retention and 20 MiB edit-scope continuation/abort pass after streamed custody fixes. Journal metadata remains open. Three-sided 64 MiB filesystem and object-backed file/tree/commit merge fixtures pass, with bounded rename inputs and recursive-base conformance. External Git merge-file drivers pass 20/64 MiB in both hashes with unchanged inputs; external scratch/cancellation, large journals and resource-bound recovery/death fixtures remain open. |
| `blame`, `log`, `revision`, `merge-base`, `tree` | Blame initial/history blobs, rename similarity and rendering are repaired and measured, including actual reduced trust and a million unique lines; large history metadata remains open. Ordinary log traverses commits without reading blobs or the index; 20 MiB commit metadata now uses mapped storage above policy and passes both hashes and actual reduced trust. Path-filtered log was already unsupported. Tree listing/statistics and revision path lookup now pass the 240,000-entry metadata matrices described above. Text revision listing now reads large commits through the shared bounded storage and refuses parent-read failures instead of inventing timestamp zero. Merge-base starting points and shared graphs now pass both-hash large-metadata qualification; parent/tag/object/message expressions and ambiguity descriptions also pass both-hash metadata checks; short prefixes now use the repository hash. Reflog queries and streamed previous-branch output now pass the 200,000-entry both-hash fixture described above. Shallow-boundary failure handling and large-metadata pruning pass the both-hash matrix above. SVG/viewer behavior and remaining history consumers still need qualification. |
| `fetch`, `push`, `odb`, `fsck`, `verify`, `commit-graph` | Pack decode diagnostics improved; 64/256 MiB local transfer passes without worker/cache changes. Loose blob verification streams; both packed verification algorithms now use bounded storage above configured allocation limits. All decode modes pass 20/64/256 MiB explicit-limit cases in both hashes, including 64 MiB OFS/REF histories, and actual reduced-trust 20/64 MiB cases. ODB inspection, connectivity and pack create/index/explode pass in both hashes; the SHA-256 writer now uses the repository hash format. Connectivity passes both-hash large commit/tree metadata and fault/retry checks. Loose metadata verification now maps above policy, and both verification algorithms/all decode modes pass the large commit/tree matrices described above. Additional protocols, other metadata families and final-source qualification stay open. |
| `branch`, `tag`, `remote`, `submodule`, `config`, `config-tree`, `attributes`, `exclude`, `mailmap` | Large config/packed-ref fixtures pass in both hashes and actual reduced trust. Mailmap object reads/output and annotated-tag reads are repaired and measured as above. Remote/submodule/path metadata and helper dispatch remain open. |
| `credential`, `env`, `editor`, `completions`, `free`, `corpus`, `tix` | Applicable metadata/output/owned-child audit remains open. Existing unsupported behavior cannot be relabeled a memory success. |
| `snapshot`, `build`, `run`, `graph`, `affected`, `plan`, `fingerprint`, `baseline`, `integrate`, `context` | Snapshot seal/verify/export/restore are measured on 20/64 MiB, aggregate and reduced-trust fixtures; the existing per-object 64 MiB native limit is preserved. `build fingerprint` and SHA-1 `build plan` now release streamed inputs and pass the aggregate 3×256 MiB workload; SHA-256 fingerprinting passes, while revision plan/affected retain the independently reproduced pre-existing Rakia limitation described above. Capture and other first-party metadata/output paths remain open; arbitrary external tasks are outside a universal success guarantee, but owned children still count in measured workloads. |
| `tix` | The compiled surface has 29 leaves covering browsing, operation history, enrichment, editing, replay and worktrunk. The 20 MiB commit refusal is repaired for show/ref-tree/worktrunk-show/op-log, with both-hash and actual reduced-trust receipts above. Native Tix804 passes. Editing, interactive loads, large notes/journals, CLI-only worker override propagation, retained edit state and owned processes stay open. Successful ordinary log or authoring checks do not qualify this separate surface. |
| `credential`, `editor` | Credential input/helper output and editor child lifetime need explicit qualification. `gix-credentials/src/program/main.rs` and `helper/invoke.rs` retain complete protocol input/output; `gitoxide-core/src/repository/editor.rs` passes paths to a child and waits. No blob-memory success is inferred for these owned-child paths. |
| `env`, `config-tree`, `completions`, help/version/surface output | Unaffected by repository blob or metadata growth, by implementation trace: `gitoxide-core/src/lib.rs::env` writes installation paths; `src/plumbing/progress.rs::show_progress` enumerates the compiled config schema; completion dispatch in `src/plumbing/main.rs` uses the compiled Clap tree; `brit-cli/src/main.rs` and `surface.rs` generate help/version/parser output before object access. `brit-cli/tests/unified.rs` checks the public help/version surface. Their allocations depend on the fixed executable/schema and process arguments, not repository contents. Output I/O and owned installation-path helper checks remain part of the ordinary fault/child suite. |
| `free` metadata and transport helpers | Eighteen leaves include pack operations, mailmap verification, commit graphs, indexes, remote advertisement, discovery and ownership-derived trust. Pack round trips and actual trust probes are measured above; remaining metadata and transport helper coverage stays open. |
| `corpus refresh/run` (normal default `max`) | The 64 MiB blob / 20 MiB commit fixture now passes persisted outcomes for all four tasks in both hashes, loose/packed, explicit policy and actual reduced trust, as described above. SWTR uses bounded HEAD metadata and VERI bounds loose metadata. Existing task identities and algorithms remain. Aggregate scratch and final-source qualification stay open. |

Outstanding acceptance also includes broader reduced-trust command coverage,
broader hash/alternate/replacement combinations beyond the expanded daily matrices, additional
metadata families over 16 MiB, aggregate scratch accounting and telemetry,
allocation and I/O faults, corruption, cancellation/death, cross-platform checks,
ordinary and fault-enabled recovery suites, and committed-source receipts.
The habit remains RED. Release and parent-pin changes wait for the complete
required matrix and coordination with the existing 0.1.14 integration owner.
