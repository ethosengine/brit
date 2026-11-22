# Rebase: retention scoped to the change (2026-11-22)

Source of the design: the monorepo backlog entry
`genesis/data/timeline/backlog/brit-rebase-cost-gitlinks-and-stranding.md`, sections "Cost at
real-repository scale", "Redesign (report §6)" items 1-8 and the steward ruling of 2026-11-22
(retained-manifest v2 with a floor for every new copy; a newer brit still reads, restores and
cleans v1 copies and the journals 0.2.6-0.3.1 wrote).

## Problem

Every pass a rebase makes (selection, preparation, each decision, each step's retention recipe,
the retained copy, the release proof) covers the complete object closure of both tips, so its cost
is O(history), not O(change). A two-commit docs rebase in the 3.9 GB monorepo is refused before it
starts; on a full brit clone a one-commit rebase took about 26 minutes to publish.

## The model

- **Base p**: the single merge base. **Chain**: the commits from the original tip down to p, each
  with exactly one parent. **Upstream U**.
- **Floor** = {p, U}. Both stay reachable from refs the user owns: p from the original branch (after
  an abort) and from U; U from the user's upstream ref and, once published, from the new branch. A
  floor commit's history is never walked, verified or copied.
- **Custody(r)** of a retained root r: the commits from r down to the floor (each must have one
  parent; leaving the range refuses), each commit's tree delta against its parent's tree (a tree
  entry equal, by id and mode, to the parent's same-path entry is inherited and skipped), the
  parent trees the delta descends into, and the floor commits that are parents of walked commits.
  A tree root is held whole. Gitlinks are never objects.
- The journal's `closure` field keeps its name (format compatibility) but now lists the union of the
  custody sets, plus `floor`. A journal without `floor` is a legacy (0.2.6-0.3.1) journal: every
  closure, witness and copy rule for it stays exactly as before.

## Invariants carried unchanged

R1 admission refuses before anything changes; R2 a forward step is refused only before it writes,
naming `--abort`; R3 `--abort` and cleanup are never refused by a quota; R4 `--retire` only for this
repository's own terminal or retaining journal, no lock deletion, no checkout advice; R5 a journal
brit cannot verify as its own is reported and never touched. Exact restore of HEAD, index, worktree
and the user's uncommitted work on abort. Gitlink ids are never looked up as local objects.

## Tasks (test first; each red before its change)

1. Plan (this file).
2. **Tests first** (red on the base):
   a. scale journey: history whose closure exceeds the (test) scratch limit, a small docs rebase is
      admitted, publishes, cleans up; journal and retained copy are asserted O(change); a conflict
      on the same shape aborts with exact restore (items 1, 3, 5, 8);
   b. unit: a copy written for a new journal is manifest v2 with `floor`; `validate_release` stops at
      the floor and treats an inherited tree entry as present (item 5);
   c. unit: a v1 copy (as 0.3.1 wrote it) is still inspected, restored from and its release proven;
      a legacy journal (no `floor`) still aborts and cleans (ruling);
   d. journey: untracked and ignored files appearing outside the footprint during a stop do not
      refuse `--continue` (item 7).
3. **Item 1 + 2, selection**: merge base by the painting walk; chain by following single parents
   from the tip to p (cost proportional to the range); kinds from p; `Selection::prepare` verifies
   (public API, read-only); `prepare_for_session` defers verification until preparation's
   index- and tree-only refusals (refStorage, branch occupancy, scope, collisions) have run, and
   verification runs before the journal exists. Shallow: the existing refusal stays (no shallow
   support is added without its own tests; see "Not in scope").
4. **Item 3, verification**: verify custody(original) plus U's and p's trees and blobs along the
   footprint; nothing else. One `Validation` (verified-id cache and one spool Budget under the Git
   directory) per operation pass.
5. **Item 4, retention refs**: unchanged refs (`original`, `upstream`, steps, stops); the release
   proof does not walk below the floor, so `upstream` is protected by the user's ref.
6. **Item 5, retained copy**: members as before, objects = custody union, plus staged blobs of the
   original and observed indexes only where they differ from the original/tip tree; manifest v2
   records `floor` (empty for a legacy journal, which then proves the full closure exactly as v1);
   `validate_release` floor- and inheritance-aware; v1 copies still read and proven the old way.
7. **Item 6, journal**: no history closure; `floor`; retention recipes record custody sets.
8. **Item 7, witness**: for new journals, the files and links on the footprint paths' ancestors
   and beneath footprint directories only.
9. **Item 8, quotas**: the retained copy is durable data under the Git directory and is never
   metered (capture, skip reservation, admission copy test and the preflight's object-bytes test
   go); the scratch limit meters the journal only. The preflight measures the range (custody
   headers, no blob reads) and keeps the journal model.
10. Docs: `docs/rebase.md` (formats, what is retained, what still strands) and the changelog under
    `## 0.3.2`.
11. Verification: memory_replay untouched and passing; every rebase/switch/pull journey and the
    replay fault and power-loss suites; clippy `-D warnings`, fmt; real-world probe on a fresh
    monorepo clone.

## Outcome and what is not done (stated so nothing is silently weakened)

Added while probing the monorepo, each its own commit:

- Unchanged gitlinks are index-only (the monorepo has five; every rebase there was refused by
  preservation, whatever the range). Changed or staged gitlinks are still refused by name.
- The preservation record is stored once as a journal payload: with history gone it was the
  remaining O(index) term, 56 MB per state record on the monorepo, which admission refused.
  This stands in for item 6's compact encoding: envelopes and byte fields stay JSON arrays
  (older journals' digests and child bindings depend on their bytes; tests parse them).

Not done:

- Shallow rebase support (item 2's bounded-walk acceptance): the refusal stays as it is.
- A stop's tree (conflict, message, edit) is retained whole, O(tree) rather than O(change):
  its objects are verified, listed in the journal and copied by an abort. The fix is a
  delta against the tip's tree with the base recorded in the recipe and the manifest.
- The authoring child's records and receipts still cover its stop's tracked tree.
- Preservation still hashes every tracked worktree file (O(tracked bytes)), spooling each.
- The upstream keeps its retention ref during the rebase; the release proof does not walk it.
