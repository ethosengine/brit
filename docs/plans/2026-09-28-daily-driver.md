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

This bounded implementation refuses recursive directories, wildcard/pathspec
magic, interactive staging, selected submodules, external clean filters, special
index flags and conflicted indexes. Mutations also refuse split/sparse indexes
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
