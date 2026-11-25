---
title: Brit sprint 6 — reads that never stop, the last daily fallbacks, and a small object store
status: proposed
serves: brit-native-daily-driver
---
# Brit sprint 6: reads that never stop, the last daily fallbacks, and a small object store

This is the whole sprint. It has eight items, and it ends when they are done or
skipped. Progress is "x of 8". The
[backlog](2026-11-15-bounded-memory-backlog.md) is the pool. Do not work from
the pool, and do not add to this list. Anything new you find goes into the pool
as one line.

## Why these eight

Item 1 is the steward's priority. The rest follow the elohim integrator's (shem's) ranking of 2026-11-25. Items are
ranked first by what each one costs the three devices today, then by what the
steward opened that day.

1. **Pack maintenance comes first because it is an owed cost.** Past 32 pack
   indexes, brit's reads fail at reduced trust. On 2026-11-25 the laptop held
   51 pack indexes and shem held 59, and `just status device` named
   `git repack -a -d` as the cure. A daily driver that stops reading until
   someone runs Git is the sharpest remaining break, and a thin client reaches
   it by design, through many small fetches.
2. **The daily-driver remainder after 0.3.5.** These are the fallbacks agents
   still typed on 2026-11-25, in order of frequency:
   - restoring one tracked path in a worktree;
   - `add -A` refusing on a repository with submodules;
   - `switch -c` / creating a branch at a ref;
   - `show --stat` and `diff --stat` of a range;
   - `config get`;
   - `ls-tree`.
   One failure is rare but sharp: a fetch that prints nothing and leaves a
   just-pushed ref unfetched. It was seen three times. The three most costly of
   these are items 2 to 4; the rest are reserves.
3. **A small object store for thin clients.** On 2026-11-25 the steward
   opened thin-client participation: "being able to have a fleet of repurposed
   chromebooks participate effectively where they can IS a win for the
   wholistic wellbeing of the network". The laptop's probe of `just dev prep`
   found the largest thin-client costs are the submodule object stores (brit's
   is several GB) and a full checkout. This sprint asks for the measurement and
   one working path, not the whole design.
Narrow CI comes first of all, by the steward's ruling of 2026-11-25: "you
should harvest those shorter integration cycles regardless.. that's a
priority". A one-test narrow run on macOS took 19 minutes, 18 of them building
every test binary in the workspace, and every other item pays that cost on
each check.

The fixes were steward-confirmed in sprint 5. The scratch-ref reflog race is
fixed in 0.3.5.

## How to work

As in [sprint 5](2026-11-25-brit-sprint-5.md#how-to-work), with three changes
learned there:

1. **The confidence ladder.** Check each change, in order:
   1. the failing test on a local build;
   2. a narrow CI dispatch (`scripts/ci/narrow.sh <branch> --platforms … --tests …`)
      on the platform that matters;
   3. one exhaustive `run-ci/sprint6-<item>` push before integration.
   Do not rerun an exhaustive run to retry one test.
2. **Code that compiles only on some platforms** gets compiled in a narrow run for that platform before its exhaustive run. Item 6 of sprint 5
   reached Windows CI with a missing import that the Linux build never saw.
3. **Versions are allocated once, at integration.** Each changed crate gets its
   bump and `rust-version = "1.100"` together. Never raise a dependent's
   requirement unless the dependent is bumped too: a caret requirement
   already accepts a patch release, and an edited manifest at a published
   version is refused by main's publish job.

The stop rule and the DELTA line are as in sprint 5.

**Out of scope:**
- the rust-style checker consumed as an artifact, which is monorepo work and
  the integrator's station;
- the elohim hook and instruction text;
- `rebase` on a shallow repository;
- the bounded-memory matrix rows.

## The eight items

Sizes:
- S is one function with a test.
- M is a new mode built on code that exists.
- L is a new command over an existing engine.

- [ ] **1. Narrow CI: per-package builds and a repeat count.** (M)
  - A `packages` input, or a package set derived from `package(...)` in the
    filterset, builds only the owning crates.
  - A `repeat` input runs the selected tests N times (nextest's stress or
    retries support, or N jobs), so a narrow run can confirm a flake fix.
  *Done when:* a one-test macOS dispatch for a brit-epr test takes under 6
  minutes from dispatch to finish, and a `repeat=20` run reports 20 passes.
- [ ] **2. Pack maintenance: reads never stop at the reduced-trust cap.** (L)
  - `brit maintenance` consolidates packs under the cap.
  - `fetch` consolidates on its own once the count passes a threshold below
    the cap. Each fetch does bounded work, so a fetch never takes unbounded
    time.
  - It never drops an object that something else may need: another worktree's
    HEAD or index, a submodule, or a repository that borrows objects through an
    `objects/info/alternates` file. `just dev prep` creates such borrowing
    clones with `--reference`.
  - It is safe to interrupt: a kill at any point leaves every object readable.

  *Done when:* journey tests show each of these:
  - a repository with 40 packs reads again at reduced trust after `brit
    maintenance`, and after one `fetch` that crosses the threshold;
  - a `--reference` clone and a linked worktree still read every object they
    need after maintenance runs in the repository they share objects with;
  - a kill test at each checkpoint leaves the store readable, and the next run
    completes.

  The laptop's own elohim checkout (51 packs) is the live proof.
- [ ] **3. Restore one tracked path in a worktree.** (S–M) This replaces
  `git checkout -- <file>`. First find out what `brit restore` does today for a
  path in a linked worktree, then close the gap:
  - restore from the index (the default);
  - restore from `--source <rev>`;
  - `--staged`.
  Each works in the main checkout and in a linked worktree.

  *Done when:* journey tests show each form leaving the same worktree and index
  bytes as `git restore` does, in both kinds of checkout, and a path that
  doesn't exist refuses and changes nothing.
- [ ] **4. `add -A` with submodules and nested repositories.** (M) Today it
  refuses and names the submodule, so every agent stages by path. Do what Git
  does:
  - stage every other change;
  - record a registered submodule's checked-out commit as its gitlink;
  - leave an unregistered nested repository unstaged, printing one warning
    line.
  *Done when:* a journey test on a fixture with a registered submodule (moved
  and unmoved), an unregistered nested repository and ordinary changes gives the
  same index as `git add -A`, with the warning on stderr.
- [ ] **5. The silent fetch miss.** (S–M) `brit fetch origin <branch>` once
  printed an index-file line and no ref, and left a branch pushed seconds
  earlier unfetched; a second fetch got it. Seen three times on shem on
  2026-11-25. Reproduce first, in a journey test that pushes from a second
  clone and fetches at once, repeated under item 1's repeat count.
  Then:
  - if fetch skipped the ref, fix it;
  - if the remote was really stale, fetch must say that no ref changed.
  Fetch must never print success while leaving a named ref unfetched.
  *Done when:* the reproduction is a test, and it passes N of N.
- [ ] **6. A small object store: measure, then one working path.** (L)
  - (a) For the elohim repository and for brit itself, measure the
    object-store bytes of a full clone, a blobless clone (`--filter=blob:none`)
    and a depth-1 clone. Record which brit verbs work on each today: status,
    log, show, diff, fetch, `pull --ff-only`, commit, and push of a hand-over
    ref.
  - (b) Make the thin client's path work on a blobless clone: fetch, commit,
    push a hand-over ref, `pull --ff-only`. Missing blobs are fetched on demand
    from origin.

  Write a short design note on fetching a missing blob from a peer holder by
  content address instead. That is where brit meets the epr artifact store.
  Say what it would take; do not build it in this sprint.

  *Done when:* the measurement table is in the design note, and a journey test
  on a blobless clone of a fixture runs the path above end to end, with the
  blob fetches counted.
- [ ] **7. Name the epr in the attribution warning.** (S) A commit made by a
  session whose epr predates `commit-trailers` prints `Brit attribution
  UNMEASURED` and carries no trailers. Agents read that as a brit fault. The
  warning must name:
  - which epr it asked, by path and version;
  - that a newer epr is installed, when one is.
  *Done when:* a test with a stub old epr and a stub newer epr shows both
  facts in the warning.
- [ ] **8. The costliest flake: the macOS fsmonitor interop tests.** (M) This
  file cost three exhaustive reruns on 2026-11-25:
  `native_queries_preserve_coverage_through_unicode_and_case_root_aliases`
  failed twice (`interoperability.rs:648`), and
  `linked_worktree_uses_external_cookie_coverage` once (`:542`). All three were
  on branches that didn't touch fsmonitor. Find the cause with item 1's repeat
  count on macOS, and fix it or quarantine it with a reason. The Windows
  `gix-notify` helper flake (`windows.rs:43`) stays in the pool.

  Also in scope, because it reproduces locally on Linux (5 of 200 runs of the
  test binary on the laptop, 2026-11-25; first seen in integration run
  45672180791): `server::asynchronous::cancelling_startup_does_not_leave_an_endpoint_owned`
  asserts at `gix-fsmonitor/tests/fsmonitor/server/asynchronous.rs:215` that
  no endpoint is running right after the replacement shuts down. Find out
  whether the timed-out startup can still take the endpoint after its lock is
  released (a product bug) or only releases it late (a test race).

  *Done when:* a `repeat=50` macOS narrow run of the file passes 50 of 50, or
  the quarantine lands with its reason and a pool line.

## If an item is skipped

Take one replacement from this reserve, in order, and keep the count at eight:
1. `switch -c <branch> [<start>]` and creating a branch at a ref.
2. `show --stat` and `diff --stat` of a range, matching Git byte for byte.
3. `config get` (and fix the session-pinned `config set` no-op if it still
   reproduces on 0.3.5).
4. `ls-tree <rev> [-- <path>]`.
5. `ls-remote` inside a submodule.
6. A guard refusal names its lock. Today `brit commit` beside a running
   pre-push in another worktree fails with only `Resource temporarily
   unavailable (os error 11)` from `try_lock` (`switch/linux/guard.rs:250`).
   Name the lock path, the holder's pid and command (from the hook marker),
   and how long it has been held. (Reported by the steward, 2026-11-25.)

**Item 2 release note owes two proofs** before 0.3.6 states them:
- A fetch in worktree B while a push in worktree A holds the repository guard
  waits or refuses, and runs no consolidation. The guard is a `flock` in the
  common dir, so this should hold; a journey test proves it.
- A reader that already has an old pack mapped keeps reading it through the
  swap, and a reader that misses an object finds it in the replacement. On
  Windows, a mapped source pack makes retirement stop with an error, and the
  next run completes it.

## Meeting points with the epr runtime and the monorepo

- **Item 6 and the artifact store.** A missing blob fetched from a peer by
  content address is the same move as `epr artifact fetch`. The design note
  names the interface. The elohim eprfs lane decides whether to build it.
- **Item 7 and device epr.** The integrator is making device acts use the
  device's head epr. Item 7 only makes the cause visible.
- **The pin.** The integrator moves the monorepo pin and the brit-first verb
  table for 0.3.5, and re-counts the friction register a few days later. That
  count re-ranks the reserve.

## Ending the sprint

1. Cut one release, `brit-v0.3.6`, matching the CLI version. Create the tag at
   a commit main already holds and push it with `brit push`.
2. Tell the integrator the release commit; the monorepo pin is theirs to move.
3. Write three lines here: what shipped, what was skipped and why, and the
   three biggest pains.
