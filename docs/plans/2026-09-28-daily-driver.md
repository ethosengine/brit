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
- [ ] Two-developer qualification: disposable fixture then two developers complete the loop, preserve Git interoperability and recover interrupted work; operator admits selected Elohim work only after all preceding readiness stations have independent acceptance. Serves brit-native-daily-driver.

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
