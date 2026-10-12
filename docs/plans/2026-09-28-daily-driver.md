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
