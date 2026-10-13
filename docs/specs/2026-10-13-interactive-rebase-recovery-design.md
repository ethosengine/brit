---
id: interactive-rebase-recovery-design
title: Interactive rebase and conflict recovery
status: proposed
created: 2026-10-13
updated: 2026-10-13
serves: brit-native-daily-driver
habit: .epr-meta/brit-native-daily-driver.habit.md
station: 5c
---

# Interactive rebase and conflict recovery

This is the implementation contract for station 5c, based on clean evidence
commit `7ffdf6b0ad5f63e723df50c569bd78010a38c0cb`. It specifies future behavior;
it does not implement, expose or qualify Brit rebase. The owning
`brit-native-daily-driver` habit remains **UNWIRED**. Only the design station can
close after characterization and independent recovery review. Existing 5a/5b
executable qualifications retain their original scope and bindings.

Evidence is the separate stock-Git suite
`tests/cli-journey/tests/git_rebase_characterization.rs`, with deterministic shell
fixtures in `tests/cli-journey/fixtures/git-rebase-characterization.sh`, and the owning
workspace's `genesis/local-dev/brit-readiness/rebase-recovery-design/`. The suite
is an observation of Git, never evidence that Brit implements the behavior.
The evidence receipt pins Git version, commands, test names and outcomes. In this
contract **G** means observation covered by that suite; **D** means an explicit
intentional Brit design choice/difference; **F** means deferred. Any proposed
behavior not explicitly marked G is D. This default is deliberate: unobserved
Git compatibility must never be inferred from the design's normative language.
The future fault matrix below is an implementation acceptance plan, not executed
Brit recovery or physical power-cut proof.

## Local-first and entity assessment

Alone with a key on the device, a key held by a doorway, or no protocol key at
all, the same local Git replay and recovery work from already-present objects.
There is no network step, protocol-key lookup, server, conductor or peer wait.
Deleting all doorways changes nothing; no first step waits on another person. Closing the device leaves synchronized
private state and an explicit recovery obligation; reopening offline permits
inspection, continue or abort under the rules below.

The working version belongs to the author: private todo, public working files
and index, and detached replay HEAD. The original attached branch remains the
published local baseline until the final decision. Diagnostics are shown state;
merge candidates and editor buffers are working state; committed replay objects
are local Git published objects, not published protocol content. Another person
or device learns only deliberately transferred Git commits; its arrival or loss
adds no authority or dependency. The CLI shows current step, original/upstream
OIDs, stop reason, dirty preservation paths, decision state, retention location,
and whether an invocation outcome is unknown.

P2P gate: this is external-Git bridge state analogous to **Private (B)**, not a
Holochain private source-chain entity. The authority is the owning Git/worktree
location, native Git OIDs and verified local journal ownership. Checksums detect
private corruption and confer no protocol authority. No DHT entry type,
integrity/coordinator zome, signal, HTTP route, SQLite/Automerge projection,
transport affinity or reach tier is added. The head-plane footprint at creation
and at one year is zero recurring protocol heads; no quiesce cost or DNA change.
It would be a misrouting to notarize drafts or ship private backups to a store.
The sound home is native Git plumbing plus a private worktree journal. Private
backups never enter the Git object database, protocol storage or shared refs.

## Boundary and command contract

Future public forms are:

```sh
brit rebase <explicit-local-upstream>
brit rebase -i <explicit-local-upstream>
brit rebase --status
brit rebase --continue
brit rebase --skip
brit rebase --abort
brit rebase --edit-todo
brit rebase --show-current-patch
```

An upstream is an explicitly supplied local revision resolving unambiguously to
one commit in local storage (local branch, tag peeled to commit, or full OID).
Remote-tracking names may be read as local refs; no fetching or configuration
inference occurs. Persist both literal argument bytes and resolved OID before
hooks/editors. Reject unborn HEAD, missing objects, ambiguous names and options
outside this contract before mutation. Resolve HEAD/attachment independently of
OID: a foreign switch to another branch at the same OID is still a change.

Selected todo commands: `pick`, `reword`, `edit`, `squash`, plain `fixup`, `drop`,
`break`, plus unambiguous Git one-letter aliases. Reordering and explicit edit
stops for amendment or splitting are supported. F: arbitrary `exec`, autosquash,
`fixup -c/-C`, labels/resets/merge commands, merge-preserving replay, `--onto`,
`--root`, `--quit`, autostash, signing and automatic updates of other branches.
No public exposure until all applicable implementation and recovery proofs pass.

Also F: sparse/split/alternate indexes, unmerged starts, external content filters,
untrusted hooks, affected gitlinks/nested repositories and unsupported filesystem
semantics. Reject these before creating an operation. Reuse the 5a/5b refusal
checks rather than accidentally broadening them. An existing Git rebase/merge,
cherry-pick, revert or bisect state, Brit switch/pull/rebase journal, or competing
reservation refuses a new operation. Foreign Git rebase control cannot interpret
Brit state; `git rebase --continue/--abort` is not an interchangeable recovery API.

## Replay selection and authorship

Pin original HEAD H, upstream U, hash algorithm and the complete local object
closure before changing HEAD. The candidate set is commits reachable from H and
not U. Require a single linear chain: if any selected commit has more than one
parent, refuse before hooks/checkout. G: ordinary Git can flatten such history;
D: Brit refuses instead of silently losing merges. Multiple/no merge bases or
shallow boundaries that prevent proving the complete chain also refuse.
Traverse parent-first, oldest to newest; store full OIDs and original parents,
never rely on timestamps or later ref resolution. The original selected chain
is immutable even when the todo changes.

Noninteractive equal or U-ancestor-of-H is a no-op: no journal, hooks, ORIG_HEAD
or reflog changes (D, narrower than some Git modes). H-ancestor-of-U is a guarded
fast-forward with preservation checks and the same final-decision rules, zero
rewrites. Interactive mode still offers the H-minus-U list when U is an ancestor
so reword/reorder/drop are meaningful; an empty interactive list takes the
no-op/fast-forward path. An explicitly edited todo dropping everything finishes
at U. No change to another branch is authorized.

For each candidate, apply the old-parent → old-commit tree delta to the current
replay tip with native three-way merge. G: conflict stage 1 is the old parent,
stage 2 (ours) is the rebased/upstream side, stage 3 (theirs) is the original
commit being replayed. This is the reverse of many users' intuitive branch names;
print it in conflict status. Renames, modes, symlinks and directory/file conflicts
are tree/index facts; never reduce them to text-marker detection. Binary conflicts
require choosing/staging complete bytes. Persist conflict participants, rename
sources/destinations and directory collision paths before publishing a stop.

D: retain initially empty commits. Do not pre-drop patch-equivalent commits by
patch-id: replay them, then stop if a formerly nonempty commit becomes empty.
At an empty stop, `--continue` explicitly keeps an empty commit with the original
message/author; `--skip` drops it. Status explains this choice. G: the suite
records Git's initially empty, duplicate and newly-empty behavior; Git's automatic
patch-equivalence skipping is an intentional difference. `drop` creates no commit.

Pick/reword/edit preserve original author name/email and author timestamp (raw
identity/timezone), use the current committer identity and a timestamp fixed in
the durable step proposal, and one parent: current replay tip. Recovery reuses
exact proposed bytes/OID and never resamples time. Reword changes message only.
Squash/fixup replace the immediately preceding replay group, keep its first
commit's author/date and parent, and create a new committer timestamp. Squash
opens an editor seeded with constituent messages; fixup retains the group's
message, discards the fixup message and opens no message editor. Signing-required
configuration refuses because signatures cannot be preserved on new objects;
old signatures are not copied. Encoding and arbitrary message bytes are preserved
unless edited; invalid editor output is retained and refuses publication.

The mapping ledger records original OID → zero, one or multiple resulting OIDs:
drop/skip → zero; pick/reword → one; squash/fixup constituents → same final group
OID; split → ordered chain. Record intermediate superseded OIDs separately.
For the `post-rewrite` Git wire format, emit one old/new line for each retained
mapping edge in original candidate order, split edges in new-chain order; omit
zero mappings. Split's multiple lines are a D extension with explicit tests
required; never pretend a single OID represents the entire split.

## Todo and editors

A todo is a byte-preserving file with command tokens and full pinned OIDs; subject
text after the OID is display-only. Parse ASCII command tokens, blank lines and
comments. Reject unknown commands/flags, OIDs outside the pinned selection,
duplicate consumption, malformed lines and squash/fixup without a preceding
retained replay group. Every unconsumed original must occur exactly once,
including explicit `drop`; missing lines refuse rather than silently deleting
work. A todo cannot change completed entries or insert a new commit. `break`
has no OID, cannot establish a squash target, and stops before the next action.
Reordering across a conflict dependency is permitted, then conflicts are resolved
normally. Validate syntax and preservation footprint before installing a new todo.
Keep rejected bytes for inspection; retain the last valid todo as authoritative.

Sequence editor precedence: `GIT_SEQUENCE_EDITOR`, `sequence.editor`, then normal
editor. Normal editor: `GIT_EDITOR`, `core.editor`, `VISUAL`, `EDITOR`, then `vi`
only with an interactive terminal; otherwise refuse with configuration guidance.
An explicitly empty editor setting refuses. Invoke with a quoted path argument,
using existing trusted command parsing, never concatenate repository path bytes
into shell source. Persist editor program/config, input digest, output path and
invocation ID before releasing locks and starting it. No secrets are printed.

Initial interactive todo editing happens after preparation, before replay.
`--edit-todo` is allowed only at a stable stop, editing the unconsumed suffix.
Normal message editing for reword/squash occurs before that step's commit
proposal; an edit stop amendment uses the normal authoring interface.
Failed editors leave the last accepted content and stop. Interrupted editor start
without a durable exit receipt is UNKNOWN, not success or failure. `--continue`
can explicitly accept the saved output after validation, showing that it accepts
bytes, not assuming the editor succeeded. `--edit-todo` explicitly requests a
fresh sequence-editor invocation; it is never automatically replayed. For a
message stop, the user may edit the displayed private message file directly then
continue. Empty messages refuse without publishing (no allow-empty-message form
in this scope). Hook/editor mutation of other state is validated separately.

## Preservation and authoring isolation

Let T0 be original HEAD's tree, I0 exact original index bytes, and W0 original
working files. P is the set of paths dirty in T0→I0 or I0→W0, including intent-to-add
and modes/deletions; preserve the original staged/unstaged distinction. Unsupported
index extensions refuse; recognized cache-only extensions may be rebuilt on
success, but abort restores exact I0 bytes. Capture checksummed raw bytes privately
for P, without writing their content to Git's object database. Existing staged
blobs already in Git remain existing objects; preservation creates none.

R is the conservative replay footprint: union of original candidate parent/tree
differences, H↔U checkout differences, every rename source/destination, all
ancestor/descendant collision paths and dynamically discovered merge outputs.
Use Git path bytes, not Unicode normalization, case-insensitive text comparison
or shell splitting. Respect filesystem alias/case collisions with 5a checks.
Refuse if P intersects R by equality or ancestor/descendant relation. Untracked
and ignored files remain in place; any overwrite/removal collision refuses even
if ignored. Do not relocate dirty neighbors, invoke stash or use a global reset.
Revalidate R after todo edits, merges and resolutions. A rename or new resolution
path expanding R into P refuses without accepting or overwriting either version.

There are three distinct indexes:

| Index | Purpose | May enter a rewritten commit? |
|---|---|---|
| Preserved I0 plus dirty-path ledger | Exact user's original staging baseline and abort restore | Never |
| Public index V | Replay/conflict entries overlaid with preserved P entries, used by status and external Git staging | Never wholesale |
| Private replay index J | Replay tip plus only validated active-step changes, no preserved overlay | Yes, exclusively |

At each stable stop publish V=J with P overlaid from I0. W contains replay outputs
and untouched W0 at P. On success final V overlays P onto final tree, preserving
both staged blob/mode and unstaged working bytes. Final validation proves these
values unchanged; no rewritten tree may include P's dirty version. A preserved
path can have staged and unstaged changes simultaneously. The assertion is about
path bytes, blob/mode and working content, not cosmetic status timestamps.

Conflict stops permit scoped Brit add/remove at the active resolution set A
(conflicted paths, rename endpoints and paths changed by this step). No broad
`add .`, `commit -a`, normal commit or amend is allowed. External `git add` is
accepted only when V's P entries remain identical, all index mutations outside
A are absent, no unmerged stages remain on continue, and resulting J is valid.
Outside-A staging is refused (G: Git may consume it; D: Brit never does). A normal
working-file edit outside A is left untouched but causes continue/skip to refuse
until restored; abort leaves unrelated edits in place if restoration does not touch them. Expand A only through an explicit validated
todo/merge result, never by treating whatever the user staged as authorized.
Continue imports only A from V into J and makes one replay commit. Unstaged
changes in A refuse continue, preventing ambiguous inclusion or deletion.

At `edit`, the replayed commit is already a durable step. Permit scoped Brit
staging and amend of the tip, or split via scoped mixed reset to the recorded
step parent followed by one or more native commits. These operations must be
new rebase-aware authoring entry points; existing general commit/reset is not
implicitly authorized. Keep P frozen and A equal to the edit group's footprint.
To include a new path within an edit, an explicit scoped staging request may
expand A only after proving no P/untracked collision and journaling its before
image. External Git staged resolutions are supported; external Git reset/commit/
amend are **F**: unexpected HEAD changes refuse, even if ancestry looks plausible.
Splitting authoring changes HEAD through Brit receipts recording each commit and
parent. Continue requires a clean A, zero unmerged entries and a linear authored
chain rooted at the recorded parent, then records one-to-many mapping. Amendment
consumes J, never V. A split may deliberately end empty via `--skip` after retaining
its authored work. Normal editor/hooks retain authoring policies below.

At `break`, inspect, edit todo or continue; no authoring is authorized. Unstaged
new work anywhere is not silently discarded. `--skip` at conflict/empty/edit
retains new step work privately before restoring the recorded step-entry state
and advancing without that original entry; at edit it removes the whole authored
split chain from replay, not earlier steps. Skip at break advances past break;
at preparation/editor-before-step or completed-final state it refuses. For a
squash/fixup conflict, skip retains the earlier group and skips only the current
input. The working step snapshot distinguishes prior group from new input.

## Hooks and observable outcomes

G: Git's `pre-rebase` receives the upstream argument alone when no separate
branch argument is supplied; this interface supplies no second branch argument.
Run once after durable preparation and before detaching HEAD or changing files,
with original attachment and branch intact. Known failure leaves integration
untouched, reports rejection and permits abort; it does not count as replayed.
Unknown outcome permits status and abort only: restarting a fresh operation after
abort is a new explicit user invocation, not replay of an uncertain side effect.

The suite pins these Git hook observations; they are evidence, not a requirement
to copy incidental internal authoring paths:

| Stock-Git path | Observed invocation sequence after pre-rebase |
|---|---|
| Plain replay / conflict continue | prepare-commit-msg (`.git/COMMIT_EDITMSG`, `message`), post-commit; no pre-commit/commit-msg |
| Reword's internal amendment | pre-commit, prepare-commit-msg (`.git/COMMIT_EDITMSG`, `commit`, `HEAD`), commit-msg (`.git/COMMIT_EDITMSG`), post-commit, post-rewrite `amend`, eventual post-rewrite `rebase` |
| Squash | prepare-commit-msg `message` and post-commit for each internal commit, no pre-commit/commit-msg, post-rewrite `amend` then `rebase` |

G: Git `post-rewrite` receives argument `rebase` and old/new pairs on stdin;
post-hook failure does not undo the published branch. Run once after durable
final publication using the stored mapping. Failure yields completed-with-warning;
unknown yields published-with-unknown-hook until explicit continue acknowledges
without rerun. Never call post-checkout or post-merge for rebase (D).

Automatic pick/reword/squash/fixup replay invokes only operation pre-rebase and
final post-rewrite hooks (D); do not inherit Git-version-dependent internal commit
hook paths. Explicit edit-stop Brit authoring uses pre-commit (no arguments),
prepare-commit-msg (message path, `commit`, amended OID for amend; message path,
`message` for a supplied split message), commit-msg (message path), post-commit
(no arguments), and post-rewrite `amend` for amendment with old/new stdin.
Normal editor message preparation uses source `commit` for amendment and no
source argument for an unsupplied new commit message. These are the existing
native authoring contracts to retain and characterize before that follow-on lands.
Pre-authoring hook failures publish nothing; explicit retry after known failure
is a new invocation with a new ID. Unknown pre-authoring hook outcome requires
abort of the operation; unknown post-authoring hook may be explicitly acknowledged
by continue without replay, retaining the already committed step result.

Every hook receives J via `GIT_INDEX_FILE` except pre-rebase (original public
index). The worktree still contains preserved W0, so hooks can read it; secrecy
from arbitrary local executable hooks is not promised. Validate hook mutations:
P must be unchanged; J changes outside A refuse; final tree is frozen and checked
again after pre-hooks. Existing `authoring::run_hook_with_input` hardcodes the
public index and `GIT_EDITOR=:`; it must gain an explicit invocation context.
Existing `index_tree` consumes the whole index and cannot be used on V.
Unknown post-hooks never justify rolling back an already-published commit.

## Durable resources and publication protocol

Use a new versioned journal namespace `brit-rebase-recovery/` inside the owning
worktree's private Git directory (the result of native git-dir discovery, not
`<worktree>/.git` string concatenation). Do not reinterpret 5a version-1 switch or
5b pull schemas. Reuse their ownership, checksum, resource-state, lock identity,
file/directory synchronization and tombstone primitives behind typed interfaces.
A repository-wide mutation guard checks all three journal types and linked
worktree reservations. Extend that guard only in the implementation follow-on;
this design changes no existing recovery behavior.

The journal manifest includes version/hash algorithm, random operation nonce,
repository/common-dir/worktree identity, original attachment/H/U/ORIG_HEAD,
original and expected reflog bytes, candidate list and current accepted todo,
P/A/R and original index bytes, exact before/desired resource descriptions,
sequence number and checksums, expected HEAD/index/ref/config fingerprints,
step-entry snapshot, invocation input/start/result records, proposal commit bytes,
mappings, retained-copy receipt and separate step/skip/final/abort decisions. Paths
are length-delimited bytes (or base64 in diagnostic JSON); absolute paths are
identity evidence, not trusted instructions to overwrite arbitrary files.

Private directories are mode 0700, files 0600, same filesystem as required by
existing safe replacement rules. Reject symlink traversal, unexpected journal
members, unsafe permissions, unsupported version, wrong owner/identity,
checksum/hash mismatches and impossible state transitions. Corruption is an
inspection-only refusal: retain evidence, do not guess phase from HEAD alone or
permit destructive abort without valid before images. Storage-full/sync errors
are incomplete outcomes; do not report durability or delete the last good copy.

Persist files with fsync, containing directories after creation/rename/unlink,
and all durable decision records by write-temp + fsync + atomic rename + parent
fsync. Before publishing any live write, durably record its allowed old/new
states and ownership. Recheck under ordinary native index/ref locks after waiting.
Create lock-ownership anchors before external lock visibility, as 5a does; remove
only a lock whose inode/ownership evidence still matches. In-flight temporary
files without proven ownership are retained, never reclaimed heuristically.

A persisted invocation-start alone does not prove its child has stopped. Use a
Linux supervised launch barrier: create a gated supervisor/child process group,
record boot identity, PID/start-time identities and operation/invocation nonce,
fsync that launch receipt, then release the child through a pipe to exec. Before
the durable receipt, EOF on the gate (parent death) makes the child exit without
executing the editor/hook. This closes the spawn-to-PID-receipt gap. The supervisor
holds an invocation lease until it has reaped the child and verified its child process
group is empty (the supervisor itself lives outside that group); persist exit/quiescence before releasing that lease. No filesystem
recovery, abort capture or second invocation is allowed while any recorded child
or supervisor is live. Recovery reports waiting/unknown and does not kill it
implicitly. After owner death, inspect boot/PID start identities and group members;
if quiescence cannot be proved, refuse mutation. A reboot proves old processes
cannot still write but not the invocation outcome. A forged/reused PID must never
be accepted as the old child. A user may stop the reported process group explicitly
and retry recovery; ambiguous liveness requires manual repair, not forced abort.
Hooks/editors which daemonize outside the supervised group or delegate background
writes are unsupported arbitrary external writers; no process-group scheme can
isolate such code. A detected escape refuses recovery; do not claim isolation
against an undetectable hostile writer.

Do not hold ordinary index/ref locks while a user edits, resolves or a child
hook/editor runs. Keep the persistent reservation across these waits and release
process locks. On return, reacquire the guard and required resource locks and
validate attachment, branch, HEAD, index, files, todo, hook configuration and
reservation. Other worktrees must not check out the original branch while it is
reserved; Brit checks both native worktree occupancy and Brit reservations.
External Git does not respect Brit reservations. A changed original branch,
foreign HEAD/index/resource state, linked-worktree checkout, replaced lock,
foreign journal or changed pinned hook/config refuses without overwriting it.

Preserve original attached branch H until final publication; detached replay
HEAD moves per accepted step. Original ORIG_HEAD is captured; set it to H only
as part of final publication (D). At final attached completion, compare-and-swap
branch H→final and restore symbolic HEAD to that same branch; detached completion
leaves HEAD detached at final. Reflog messages carry operation nonce and step
ordinal in addition to human action text, and exact intended old/new log bytes
are journaled. Reuse compare-old-or-already-desired logic: a recovery retry must
append a logical event once, even for identical OIDs. A third state never counts
as an already-applied event. Branch and HEAD publication can be partially visible
between filesystem operations; the final decision makes explicit continue finish
that publication and status names each completed resource.

Retain original and every intermediate committed tree's complete reachable
object closure before a step decision, including created loose objects and
preexisting objects reused by hash. Validate native object hashes, sync objects
and directory entries, then establish durable GC protection. Create owned retention refs for **both H and U** before detaching or replaying,
even if existing branches currently reach them: detached H and full-OID U may
otherwise be unreachable. Ref names are
`refs/brit-rebase-retain/<nonce>/original` and `/upstream`; require absent-before
creation and persist their ownership. Use operation-specific retention refs for rewritten Git
commits with namespace `refs/brit-rebase-retain/<nonce>/<ordinal>` (D). These refs
contain only normal authored commit OIDs, never dirty backup blobs. They live in
the shared Git ref store because that is where GC looks, but the journal/backup
payload stays private; never automatically push these refs. Explicit `--mirror`
can export such native refs and is outside this privacy boundary. Remove owned
retention refs only after final commits are durably reachable through their final
ref/HEAD, or abort/skip has durably captured their content in private retained
copies. The original object closure remains pinned until restoration completes.
Do not assume reflog expiry or loose-object grace periods provide retention.

A **step decision** says its proposal commit and detached HEAD transition must
finish exactly once; it does not decide the operation. Abort after any number of
step decisions first completes any decided step metadata, captures current work,
then restores H/I0/W0. A **final decision** authorizes branch/HEAD/index/ORIG_HEAD
publication and is the only point after which operation abort refuses. A **skip decision** durably authorizes retaining current-step work, restoring its
entry state and advancing its cursor once; it never authorizes final publication.
Sticky skip intent is recorded before copy creation. Interrupted skip continuation
finishes skip instead of accidentally committing the resolution; abort first
finishes a decided skip then starts operation abort. A separate
**abort decision** commits to retained-copy creation and restoring original state;
continue or abort thereafter both complete that restore. This four-decision separation
is mandatory; simply nesting 5a's roll-forward-only decision would be incorrect.

## State × action × interruption matrix

Every row inherits the global third-state/corruption refusal rules and the child-
quiescence gate above: no continue, abort, capture, cleanup or second invocation
may mutate state while a recorded child is live or its quiescence is unproven.
Rows for unknown invocation outcomes apply only after quiescence is established.
`status` and `show-current-patch` never mutate or implicitly recover. Show patch
uses pinned old-parent→old-commit bytes (or the current squash input), not mutable
refs; before a step/after completion it reports no current patch. All invocations
other than the actions listed refuse. At stable stops, only the explicitly listed
mutations are allowed. Recovering a decided resource is guarded and idempotent.

| State and durable resources | Allowed user mutation | Continue | Skip | Abort | Crash / restart result |
|---|---|---|---|---|---|
| NONE; no active journal | Normal existing commands | No operation refusal | Refuse | Refuse | No inferred recovery from orphan files |
| PREPARED; original snapshots, P/R, pinned objects, reservation | None, except recorded initial todo editor | Validate; run pre-hook if never started, then initial todo and replay | Refuse | Restore/no-op integration; clean owned preparation | Inspect only until explicit action; original branch intact |
| INVOKING_PRE; invocation-start durable, no exit receipt | Only child; no concurrent authoring | UNKNOWN: refuse | Refuse | Capture observed child changes if safe, restore | Never rerun hook; abort-only |
| PRE_REJECTED; known hook failure | None | Refuse; abort then new operation | Refuse | Restore preparation | Failure remains known |
| EDITING_TODO/MESSAGE; input and start durable | The named editor file only | Validate and explicitly accept saved bytes; known/unknown editor result shown | Refuse before step; current step rules otherwise | Retain edited bytes then restore | No editor automatically resumed; saved output is untrusted until accepted |
| READY; accepted todo/current step entry snapshot | None | Compute merge and stage proposal | Refuse until stable step stop | Retain any new work and restore | Unsynchronized proposal can be recomputed; no invocation repeated |
| APPLYING; before/desired file/index resources, J, no step decision | None | Finish validated merge outputs to stop/proposal | Refuse | Roll back this tentative write then retain/restore | Each resource must equal recorded old or desired; otherwise refuse |
| CONFLICT or EMPTY; V/J/A and stop snapshots | Scoped resolution staging/files; empty choice only for EMPTY | Validate and commit J, or explicitly keep empty | Retain current step work; restore step-entry, advance | Retain allowed new work; restore original | Stops are durable; never automatically continue |
| STEP_PROPOSED; commit bytes/OID, pinned closure and desired HEAD/log, no decision | None | Revalidate and write step decision | Refuse | Retain proposal/current bytes; restore | Uncommitted proposal is not published merely because object exists |
| STEP_DECIDED; durable decision and exact resource postimages | None | Finish HEAD/index/log and mapping publication once | Refuse until publication finishes | Finish decided step, retain, then restore original | Recovery cannot skip decided metadata or promote it to final decision |
| EDIT_STOP; durable replay tip/group, mutable scoped J/V | Scoped Brit amend/reset/split with child receipts | Validate authored chain/clean A; accept mapping, replay next | Retain entire new group; restore entry parent, drop input | Retain authored commits/files then restore | Sub-authoring receipt has same decision/invocation rules; unknown pre-hook abort-only |
| BREAK_STOP; completed-step ledger and next cursor | Todo edit only | Consume break, resume | Same as continue | Retain then restore | Remain stopped |
| READY_FINAL; complete mappings, final index/tree proposal, all objects retained | None | Validate all guards; write final decision | Refuse | Retain and restore, even though every replay step succeeded | Still abortable until final decision is durable |
| FINAL_DECIDED; full publication postimages and decision | None | Complete partial branch/HEAD/index/ORIG_HEAD/log publication | Refuse | Refuse: operation decided; name continue | Roll forward only, no hook before publication complete |
| FINAL_PUBLISHED; metadata durable, post-hook not started | None | Start post-rewrite once | Refuse | Refuse | Start hook only on explicit recovery |
| INVOKING_POST; durable start, no exit receipt | Child only | Acknowledge UNKNOWN, do not rerun, cleanup | Refuse | Refuse | Report published-with-unknown-hook |
| COMPLETED; known post outcome/acknowledgement | None until tombstone | Retire reservation and cleanup | Refuse | Refuse | Published success/failure-warning remains truthful |
| RETAINING_SKIP; sticky skip intent, step-entry snapshot and incomplete copy | None | Finish checked retained copy, then skip decision | Same as continue | Finish copy, cancel undecided skip, begin original-state abort | Never reinterpret interrupted skip as replay; live work retained until copy complete |
| SKIP_DECIDED; complete copy, cursor target and restoration postimages | None | Restore step-entry and advance once | Same as continue | Finish decided skip then retain/restore original operation | Exact restoration/log/cursor resource writes idempotent; no replay of skipped input |
| SKIPPED; restored step-entry, durable mapping omission and cursor | None | Retire owned skip resources, resume next step | Refuse double skip | Begin operation abort | Completed skip receipt prevents duplicate cursor advancement |
| RETAINING_ABORT; copy building, no abort decision | No writes; inspect only | Finish checked copy, then abort decision | Refuse | Same as continue | Originals still exist; partial copy never licenses overwrite |
| ABORT_DECIDED; complete retained copy/receipt, restore postimages | None | Restore original resources guardedly, finish abort | Refuse | Same as continue | Continue restore, never resume replay |
| ABORTED; originals restored, retained copy durable | None until tombstone | Retire active reservation/journal | Refuse | Finish cleanup idempotently | Report original state restored plus retention path |
| TOMBSTONED; no active reservation, cleanup receipt | Normal commands | No active operation | Refuse | No active operation | Cleanup only known owned members; retained copies remain |
| DAMAGED/FOREIGN; any invalid identity/checksum/resource | Read/export only | Refuse with exact mismatch | Refuse | Refuse; no destructive best guess | Preserve everything for explicit manual repair outside this command contract |

At an active stop, changes within A matching its mutation permission are expected
user work and can be captured on abort. A changed P or unrelated file is not a
license to overwrite it: abort leaves unrelated resources in place, and refuses
if restoration would touch them or require claiming unknown ownership. A valid
external resolution index is captured/imported only at explicit continue/abort.
The same validation runs after every child invocation. Unknown child effects
outside the authorized resources yield DAMAGED/FOREIGN inspection-only refusal.

## Retained abort and skip copies

Before discarding any new resolution/edit/todo/message work, create a complete
private retained copy. Location:
`<owning-private-git-dir>/brit-rebase-retained/<operation-nonce>/<copy-sequence>/`.
Status/abort/skip print the exact resolved absolute location and receipt digest.
Its schema is separate from active journals and never reserves a branch or blocks
new operations. No timer or cleanup heuristic expires it.

Contents: manifest with byte-encoded paths and original/replay/observed OIDs,
raw original and observed index files, raw todo/message buffers and invocation
receipts, per-path raw staged/unstaged/before/after file bytes, type/mode/symlink
payload/deletion records, conflict-stage blobs, and raw reachable authored Git
object bytes with native hash/type metadata for every new rewritten/edit commit
being released from retention. Objects already public remain public; copying
those bytes privately does not turn private dirty bytes into Git objects. Capture
only operation-touched resources plus any identified new stop work, not an
unbounded copy of unrelated ignored secrets. Deletion is explicit in the manifest.
The original P snapshots are included for exact forensic reconstruction.

Files use opaque numeric names; manifest path bytes identify them, so a filename
cannot escape the copy directory. Validate every copied digest and fsync every
member and directory before recording COPY_COMPLETE and deciding abort/skip.
Only then restore integration resources. A crash while copying preserves original
work and leaves incomplete copy unaccepted; explicit recovery finishes or refuses
if source bytes changed. A crash after copy completion may finish restoring with
no live work lost even when no rewritten commits remain reachable in refs.

Inspection/extraction contract (D, implementation follow-on): provide a standalone
read-only retained-copy decoder in the recovery library and a documented shell
invocation through a shipped recovery-inspection helper, taking explicit copy
path and an **empty** destination directory. It validates schema, ownership and
checksums, then extracts `original/`, `staged/`, `worktree/`, `messages/`,
`objects/` and a lossless path manifest; symlinks extract as payload files by
default, never follow links or apply into a repository. It needs no active journal
or protocol key. The future helper contract is:

```sh
brit-rebase-retained inspect --copy '/absolute/printed/copy'
brit-rebase-retained extract --copy '/absolute/printed/copy' --to '/empty/destination'
```

This separately shipped native helper is part of handoff 4; these commands do not
exist today and are not a claim of implemented support. Raw member files
can already be copied manually after reading the manifest. To reconstruct a file,
select its manifest member, copy the raw payload into a separate directory and
apply recorded mode deliberately. To recover a committed edit, feed verified raw
objects through native hash-object import into a disposable repository, verify
reported OIDs, then inspect there; never import draft backup payload as objects.

Deletion is an explicit filesystem operation on the printed copy directory after
inspection/extraction, never part of abort cleanup. The future manual procedure
uses the exact printed leaf path, never a glob or the retention root:

```sh
brit-rebase-retained inspect --copy '/absolute/printed/copy'
brit-rebase-retained extract --copy '/absolute/printed/copy' --to '/empty/destination'
# After verifying the extracted manifest and payloads and choosing to delete:
rm -r -- '/absolute/printed/copy'
```

The literal sample path must be replaced with the copy leaf shown by status;
check it before removal. Deletion needs no Git command and changes no active
journal. Extraction rejects a nonempty destination, bad checksum, unsafe path or
symlink destination; it never restores directly over live repository files. Documentation must provide
quoted-path removal instructions and warn that this deletes
the retained evidence; no automatic GC path deletes it. Active operation cleanup
uses a durable rename to a tombstone before removing members, as 5a/5b; retained
copies are outside that namespace. A failed cleanup reports completed-with-cleanup
pending, not failed replay or lost backup.

## Existing primitive map and missing capabilities

| Requirement | Existing foundation inspected at the base | Missing work / constraint |
|---|---|---|
| Public replay control | `gitoxide-core/src/repository/rebase.rs` stub; existing parser/parity placeholders | Real replay driver and guarded public dispatch; placeholder success is not support |
| Sequencing | `gix-sequencer/src/lib.rs` empty | Todo grammar, cursor/group/mapping machine, interrupted invocation contract |
| Tree replay/conflict stages | `gix-merge`, `gix-index`, `gix-diff` native primitives | Compose commit-parent deltas, rename/directory conflict footprints and durable stops; merge support is not sequencer support |
| Object identity and commit creation | `gix-object`, ODB and native staged authoring | Per-step immutable proposal/retention closure and preservation-safe J authoring |
| Hooks/editor | `repository/authoring/mod.rs` trusted hook discovery/invocation | Explicit replay-index hook context; sequence/message editors; durable invocation outcome receipts |
| Commit tree | `authoring::index_tree` and index authoring operations | Never use whole public V; conflict-aware scoped staging and split/amend child receipts |
| Files/index restoration | `repository/switch/linux/mod.rs`, `switch/durability.rs` | Multiple step snapshots, dynamic A, retention-copy protocol; no copying monolithic final decision |
| Object durability | `repository/switch/linux/pull.rs` durable object validation/synchronization | Reuse validation for locally rewritten closure; per-operation GC protection and release |
| Refs/reflogs/occupancy | `gix-ref`, switch/pull guards and native linked-worktree checks | Original-branch reservation across detached replay; distinct final publication and idempotent per-step logs |
| Existing journal compatibility | Switch version 1 and pull transition journal | New namespace/version plus cross-operation guard; retain old readers/tests unchanged |
| Private retained work | Checksummed private switch before images | Separate complete-copy schema, decoder/extraction docs and interrupted retention/abort tests |

## Future fault and preservation qualification matrix

Run real SIGINT/SIGKILL checkpoints and a **separate** loss-of-unsynchronized-writes
model at every boundary below, before/after each actual file and directory sync.
Record model omission controls so a test proves the synchronization matters.
Neither stock-Git interruption at a hook/editor nor ordinary process signals
qualify Brit's power-loss behavior. Linux successful fsync/rename/parent-sync is
the claim boundary; broken filesystem/device guarantees, remote filesystems and
hardware power-cut survival remain outside this design's qualification.

| Boundary / injected fault | Required assertion on explicit recovery |
|---|---|
| Reservation/nonce creation before journal | No live writes; never steal foreign lock/reservation; owned orphan diagnosable |
| Each original snapshot/index/P payload write and sync | Incomplete preparation cannot detach or publish; exact before bytes retained |
| Pinned old/new objects, retention ref publication | Missing/corrupt/unsynchronized closure prevents step/final decision; GC cannot collect decided replay |
| Hook/editor start marker, gated spawn/PID receipt/barrier release, after spawn, before exit/quiescence receipt | No execution before durable identity; live/ambiguous child prevents recovery mutation; UNKNOWN preserved, no automatic replay; pre-hook abort-only; editor saved bytes explicitly validated |
| Editor invalid/failed output, todo reorder touching P | Last valid todo remains authoritative; no changed replay cursor |
| Each replay file/type/directory/index transition | Old/desired only; third-state refusal; P, untracked and ignored neighbors byte-identical |
| Conflict-stop publication and external Git add | Complete stage set or recoverable write-ahead state; no external staged neighbor enters J |
| Reword/squash/fixup message proposal | Reuse saved message/author/committer timestamp; no duplicate editor |
| Step object sync → step decision → detached HEAD/log/map write | Exactly one logical step, original branch unchanged; operation still abortable |
| Edit-stop amend/reset/split child decisions and post-hook | Child outcome reported accurately; whole group recoverable; no P swept into commit |
| Skip retention creation → skip decision → step restoration | Resolution/authored bytes inspectable before loss; previous group preserved |
| Final validation before final decision | Abort restores H/I0/W0 regardless of number of completed steps |
| Final decision and each branch/HEAD/ORIG_HEAD/index/reflog publication | Continue finishes exact publication once; abort refuses with decided state |
| Post-rewrite start/exit receipt failure | Branch remains published; warning/UNKNOWN truthful; no hook rerun |
| Copy members/digest/manifest/COPY_COMPLETE synchronization | No restore before complete copy; partial copy plus live originals retained |
| Abort decision and each restoration boundary | Continue/abort finishes restore, new work survives independently of active journal |
| Retention-ref release, reservation release, tombstone rename and unlink | No GC window before durable reachability/copy; completed copies never block next operation |
| Foreign index/ref/HEAD/file changes, same-OID attachment change | Detect and refuse before overwrite; retain evidence; no unconditional rollback |
| Replaced lock inode, foreign worktree checkout, legacy switch/pull journal | Never remove foreign lock; reserve across worktrees; existing recovery still works |
| Journal/path/schema/checksum/object corruption, ENOSPC, fsync error | Read-only refusal/partial outcome, no success claim or destructive cleanup |

Preservation acceptance combines every stop kind with: staged-only, unstaged-only,
same-path staged+unstaged neighbors, deletions/modes/symlinks, byte-invalid paths,
untracked/ignored files, ancestor/descendant collisions and external mutation.
For each, compare exact original P index entries/worktree bytes, all rewritten
trees (absence of dirty neighbor content), final staged/unstaged distinction and
abort exact index bytes. Repeat with detached HEAD and linked worktrees. Dirty
starts refused by Git use its equivalent clean fixture as replay evidence and
these separate Brit assertions; never label a Git refusal a preservation pass.

Arbitrary external writers cannot be isolated by advisory locks or snapshots.
Compare digests and identities under locks and immediately before each live write,
but a hostile writer can race after validation. Such writers are outside the
atomicity claim; detectable mismatches always refuse. This limit must be public,
not hidden by claiming checksums enforce exclusive filesystem ownership.

## Characterization-to-contract map

The names below are executable cases in the dedicated stock-Git suite. Full
commands/version and pass counts belong to the retained run receipt rather than
being inferred from this table. G applies to observed assertions only; all Brit
recovery/preservation rules above remain D until implemented and qualified.

| Git evidence cases | Contract reading / intentional boundary |
|---|---|
| `attached`, `detached`, `noop_fastforward` | Selection, branch/HEAD/order/authorship; Brit no-op metadata policy is D |
| `empty_and_equivalent`, `newly_empty`, `merge_history` | Git empty/drop/flatten observations; Brit retain-empty, explicit new-empty stop and merge refusal are D |
| `reword`, `squash`, `fixup`, `reorder_drop`, `break_stop` | Selected interactive commands; strict missing/duplicate todo rejection remains D |
| `edit_amend`, `edit_split` | Native Git edit-stop behavior; Brit scoped authoring with separate J remains unimplemented D |
| `invalid_todo`, `failed_editors`, `editor_precedence` | Rejected/failed editors and tested precedence chain; unknown-outcome bytes acceptance is D |
| `text`, `add_add`, `modify_delete`, `rename`, `binary`, `symlink`, `mode`, `mode_conflict`, `directory` | Tree/conflict stages and resolution shapes; semantic modes are checked even when Git resolves without a stop |
| `dirty_neighbors`, `skip_abort` | Git dirty-start refusal and clean equivalent; Brit preservation/retained abort copies are D |
| `hooks`, `hook_failures`, `hooks_reword`, `hooks_squash`, `hooks_conflict`, `failed_prepare_hook` | Exact hook argument/order/publication observations; restricted automatic hook policy above is D |
| `linked_worktree`, `external_mutations`, `external_index_and_worktree` | Git external-writer/index/HEAD observations; Brit reservations and foreign-state refusal are D |
| `untracked_collision`, `ignored_collision` | Git untracked refusal and ignored overwrite; Brit refuses both kinds of overwrite |
| `interruption_editor`, `interruption_hook` | Observable Git process interruption only; no Brit sync/retention/power-cut claim |

## Implementation handoffs and completion gate

1. **Conflict-aware authoring:** separate V/J/I0, scoped resolution staging,
   preservation validator, edit amend/split receipts and explicit hook index
   context. Keep rebase public forms unavailable. Prove no P content reaches
   any commit and external staging imports only the authorized set.
2. **Durable linear replay:** pinned selection, merge-history refusal, empty
   policy, object protection, step decisions, detached replay HEAD, final
   decision, abort retention and compatibility with switch/pull journals.
3. **Interactive sequencing:** validated todo/editor state, reword/edit/squash/
   fixup/drop/break, mappings and unknown invocation outcomes, with scope
   revalidation after every change and stop.
4. **Combined recovery/artifact qualification:** complete fault matrix,
   retained-copy decoder and extraction/deletion instructions, independent
   recovery review, rebuilt executable/source bindings and integration journeys.
   Only then consider public parser exposure and readiness claims.

Design completion requires the independent reviewer to approve preservation,
stop mutation permissions, step/final separation, retention, privacy, ownership
and partial outcomes, with every blocking issue resolved in this text. The
characterization suite must pass with scoped formatting/Clippy and source-CI
selection, and observations must be pinned in the evidence receipt. Root work
updates station 5c, the owning habit DELTA and existing recipe, creates scoped
local design/test and evidence commits, and runs
`epr ready --target 7ffdf6b0ad5f63e723df50c569bd78010a38c0cb`.
No push, publication, parent submodule-pin move or daily-driver completion is
part of this sprint.
