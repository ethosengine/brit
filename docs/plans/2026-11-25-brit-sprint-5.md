---
title: Brit sprint 5 — the daily driver's largest fallbacks
status: active
serves: brit-native-daily-driver
---
# Brit sprint 5: the daily driver's largest fallbacks

This is the whole sprint. It has seven items and it ends when they are done or
skipped. Progress is always "x of 7". The
[backlog](2026-11-15-bounded-memory-backlog.md) is the pool; do not work from
the pool, and do not add to this list. Anything new you find goes into the pool
as one line.

## Why these seven

The elohim integrator (shem) counted every stated reason for running Git where
the repository asks for brit. The register is at
`genesis/docs/superpowers/specs/evidence/2026-11-25-brit-friction/register.md`
in the elohim repository; until it reaches `dev`, it is on the ref
`self/shem-brit-friction` (4dc3684f6). It found 3,111 stated fallbacks: 708
were template text, and 2,403 were kept. Its own limits apply here:
- the classes come from a first-match keyword pass;
- the counts are of comments, not of distinct incidents;
- most reasons predate brit 0.3.4;
- nothing that mutates was re-run on 0.3.4.

Ranked by kept occurrences, the classes brit can still close are:
- landing push 624 (26% of all kept);
- cherry-pick 150;
- local fast-forward merge 149;
- notes refs 62;
- the open part of read forms and log (`grep <rev>`, `log -S`, `ls-tree`);
- staging a gitlink at a named commit, the open part of the submodule class.

At least 562 kept occurrences are for verbs that already exist in 0.3.4. The
cure for those is the elohim repository's hook text and instructions, which the
integrator owns, not this sprint.

The order puts first what retires the elohim repository's standing landing
exception, because every device pays for that one on every landing.

## How to work

1. Take the items in order where they share files. Items 1, 5 and 7 touch
   disjoint code and may run in parallel, each in its own worktree and branch.
   Items 2 and 3 both build on the replay and checkout paths and run in
   sequence. Items 4 and 6 are independent read and transport verbs.
2. Bounded implementation work is delegated to Codex (`codex exec`), one brief
   per item, in that item's worktree. The coordinating session reviews each
   diff and runs the verification itself.
3. For each item:
   1. Write the failing test first, then make it pass.
   2. Run that crate's whole test set (`cargo test -p <crate>` with no name
      filter).
   3. Run `cargo clippy -p <crate> --all-targets -- -D warnings`.
   4. Run the format commands under "Dev-loop" in this repository's root
      `CLAUDE.md`.

   Tests that run the built binary live in `brit-cli/tests/`; workflow
   ("journey") tests live in `tests/cli-journey/tests/`. Keep each worktree's
   Cargo target separate, and keep targets and `TMPDIR` on disk on hosts whose
   `/tmp` is RAM-backed.
4. Push the branch to `run-ci/sprint5-<item>`. When `ci.yml`'s `Tests pass` is
   green there, merge into the sprint's integration branch. Sprint 4 showed that
   one integration branch with one version allocation avoids re-allocating
   versions on every main move.
5. **Any commit that changes a published crate's source must bump that crate's
   version in the same commit**, along with every dependent in the publish set
   whose version the registry already holds. Rehearse with
   `scripts/ci/cargo-publish-brit.sh --check` before landing. Docs-only commits
   need no bump.
6. In the same commit that finishes an item:
   1. Tick the item below.
   2. Append one line to `.epr-meta/brit-native-daily-driver.habit.md` in the
      shape of its last line: `DELTA <date> (RED preserved): <what landed, with
      the test or run that shows it>`.

**Stop rule.** If an item is not green after four hours of active work on it
(waiting on CI does not count), stop. Write what blocked it under the item,
with file and line, mark it `skipped`, and take the next reserve item. A
skipped item is a finished item for counting. Do not widen an item to make it
pass.

**Out of scope:**
- the epr-runtime side of carrying brit to a device: `epr bin follow`, release
  acceptance and the session pin, which belong to the elohim eprfs lane (see
  "Meeting points");
- the elohim hook and instruction text;
- `pull` and `rebase` on a shallow repository;
- rebase cost scaling on a full history;
- the bounded-memory matrix rows.

## The seven items

Sizes:
- S is one function with a test.
- M is a new mode built on code that exists.
- L is a new command over an existing engine.

- [ ] **1. `brit push` completes a gated `dev` landing.** (M) The elohim
  repository still lands with `git push` under a standing exception written for
  brit 0.1.12, which closed send-pack after a green hook. On 0.3.4, shem pushed
  a hand-over ref with a `src:dst` refspec through a green hook. Two cases
  remain unproven:
  - the long case the exception was written for, a pre-push hook that runs for
    about an hour before send-pack;
  - the landing's own form, `push origin HEAD:dev`, which the pool records as
    refused: "only a named local branch is supported".

  Work: accept a revision (`HEAD`, a sha) as a push source, or name the cure in
  the refusal. Keep the transport open, or reconnect, across a long hook.
  The real landing command is `GATE_EVIDENCE=reuse git -c
  core.hooksPath=<worktree>/.husky/_ push origin HEAD:dev`, so the item also
  covers what that command does.
  *Done when* journey tests show each of these:
  - a pre-push hook sleeps past the transport's idle timeout and exits 0, and
    the remote ref then moves;
  - `push origin HEAD:<ref>` and `push origin <sha>:<ref>` move the remote ref;
  - (a) a hooks path can be given per invocation (the equivalent of `-c
    core.hooksPath=…`), so a worktree landing runs that worktree's hook and not
    the main checkout's;
  - (b) the hook's environment reaches it unchanged (`GATE_EVIDENCE`,
    `GATE_LANDING_INTENT`, `GATE_CARRY_WAIT`, `HUSKY`), and its stdout and
    stderr reach the caller line by line while it runs, so a watcher sees
    `[pre-push]` and `[gate-live]` lines live;
  - (c) a hooks path that is missing or not executable refuses, and nothing is
    sent. Git treats that as no hook and pushes ungated without a word, as in
    elohim landing 21 on 2026-11-13;
  - (d) the sha sent is the sha the hook judged. The revision is resolved once,
    before the hook, given to the hook on stdin, and sent exactly. Test: HEAD
    moves while the hook sleeps, and the remote receives the old sha;
  - if the remote branch moved during the long hook (not a fast-forward any
    more), the push refuses, writes nothing and exits non-zero.

  Then shem completes one real `dev` landing with `brit push` on a build of
  this item, with `[pre-push]` lines in its log. The build reaches shem as a
  named brit branch and full commit sha, which shem builds with `epr bin` from
  a clean brit checkout. A release closure is not used, because the device
  store refused one for 0.3.4. The integrator then decides whether the
  exception is retired. This item alone retires the root `CLAUDE.md` landing
  exception.
- [ ] **2. `brit cherry-pick <commit>`.** (L) This is 150 kept occurrences. The
  pool records it as a new replay mode, and sprint 4 put it out of scope. Scope
  is one commit with no conflict, including a commit that only moves a gitlink.
  A conflict refuses, names the paths, and leaves the tree untouched.
  `--continue` is not part of this item. The author, author date, message and
  every trailer (`Change-Id`, `Co-Authored-By`, the session trailers) are kept
  byte for byte, because the elohim hand-over hook keys on `Change-Id`. One
  commit per call is enough, since the elohim `landing-pick` wrapper loops.
  *Done when:* journey tests show three things:
  - the result tree matches `git cherry-pick` for one file change and for one
    pin move;
  - the picked commit's author, date, message and trailers equal the source
    commit's;
  - a conflict refuses with the tree unchanged.

  This retires the Git inside `landing-pick`.
- [ ] **3. Local `brit merge --ff-only <ref>`, and `pull --ff-only` across a
  pin move.** (M) This is 149 kept occurrences. `brit merge` has only object
  merges. `brit pull --ff-only` refuses when a submodule pin moved, so the
  integrator uses `git merge --ff-only`. *Done when:* journey tests show both a
  fast-forward of the checked-out branch and a pull across a gitlink move
  leaving the same HEAD, index and worktree as Git does. They also show:
  - a non-fast-forward refused with nothing written;
  - a fast-forward that succeeds while unrelated tracked files are dirty.
    Hooks keep `.claude/data` ledgers dirty in every elohim main checkout;
  - a refusal, naming the path, only when a dirty path is one the fast-forward
    would change.

  This retires the integrator's `git merge --ff-only` after each landing.
- [ ] **4. Notes refs: `brit notes show` and fast-forward fetch and push of
  `refs/notes/*`.** (M) This is 62 kept occurrences. Each elohim device
  publishes its build receipts to its own
  `refs/notes/brit/devices/<did>/build/*`, fast-forward only, and today does it
  through Git. Fetch and push take ONE named notes ref and touch no other;
  never a wildcard push, because the shared `refs/notes/brit/build/*` on origin
  is frozen. *Done when:* a journey test round-trips one device notes ref
  through a bare remote while a second notes ref there stays unchanged. The
  test also refuses a non-fast-forward and shows one note's body. This retires
  the Git in `just dev notes publish`.
- [ ] **5. Stage a gitlink at a named commit without a checked-out
  submodule.** (S–M) This was sprint 4's second reserve. A pin move still needs
  `git update-index --cacheinfo`. *Done when:* the index entry holds the named
  commit with mode `160000`, for a submodule that was never initialized.
- [ ] **6. `brit grep <rev> -- <pattern>` and `brit log -S <string>`.** (M)
  This is the open part of the read-form and log classes. Tracing a hook change
  needed `git grep <rev>` and `git log -S`. *Done when:* both match Git's output
  byte for byte on a fixture repository at one revision.
- [ ] **7. Diagnostics stay off stdout, and the feedback-capture line is
  quiet.** (S) This is every scripted use, on all three devices:
  - `brit rev-parse` and `ls-remote` print `brit: …` lines on stdout beside the
    answer;
  - `status --no-write` writes `\x1b[2K` into piped output;
  - most commands print `feedback capture gap: …`, which agents read as a
    failure. The wording varies: `recorder timeout`, `migrate
    --legacy-writers-quiesced` and `unknown structured operation` were all seen
    on 2026-11-25.

  Work: diagnostics go to stderr; no control codes when stdout is not a
  terminal; the capture gap prints at most once per session and names its
  cause; a `--quiet` flag and an environment switch that scripts can set now
  silence it.

  *Done when:* journey tests show piped `rev-parse` and `status` output byte-
  equal to the bare answer, and a second command in the same session prints no
  capture line.

## If an item is skipped

Take one replacement from this reserve, in order, and keep the count at seven:

- `brit config get <key>`: prints the value alone and exits 1 when unset.
  Test: `core.hooksPath` both ways.
- `ls-remote` inside a worktree's submodule reads that repository's remote,
  not the main checkout's local copy; the wrong answer said a release was absent
  from GitHub when it was not. Test: a submodule whose local origin differs from
  its remote.
- A pack maintenance verb, so a store past the reduced-trust cap of 32 pack
  indexes becomes readable without `git repack -a -d`. The laptop has 46, and
  ethosengine and shem sit near the cap. Test: a 46-pack store reads after the
  verb.
- `brit ls-tree <rev> [-- path]`, or a documented `brit tree entries` form with
  the same output. Test: byte-equal to Git on one tree.
- brit's own pre-push needs the pinned rust-style checker built locally even
  for a docs-only push: 1m42s and 333 MB of target on the laptop on
  2026-11-25. A first contributor should not pay that build. Ship the checker,
  or skip it when no Rust file changed. Test: a docs-only push on a host with
  no checker installed.

## Meeting points with the epr runtime

These are not sprint items. They are where brit meets the device runtime, so
an item does not undo one:

- brit reaches a device through `epr bin follow --tool brit`, which reads the
  main checkout's gitlink and refuses `non-descendant` from a feature-branch
  checkout. The laptop never follows a newer brit for that reason. Until then,
  a release here must say which installed brit version a workflow needs.
- A session stays on the brit it started with while the device head moves, so
  a fixed verb is absent for hours. The sharp case, seen 2026-11-25: on 0.3.0,
  `brit config set <key> <value>` exits 0 and writes nothing, because `set`
  only arrived in 0.3.3. An absent verb must refuse, never no-op. Every new verb
  in this sprint therefore says, in its refusal on older builds, which version
  it needs.
- Items 1 to 4 together make a whole elohim landing Git-free. Items 5 to 7 are
  quality.
- Item 7 touches the `epr feedback` call. Keep the recorder's contract;
  change only what is printed.
- Item 4 must keep the device notes publish fast-forward only, because the
  shared `refs/notes/brit/build/*` on origin is frozen.

## Ending the sprint

When the count reaches 7 of 7 green, land the integration branch on `main`.
Then:
1. Cut one release (`brit-v0.3.5`, matching the CLI version), create the tag
   with `brit tag -a`, and push it with `brit push`.
2. Tell the integrator the release commit; the monorepo pin is theirs to move.
3. The integrator re-counts the register's classes from transcripts after the
   pin moves, with the extraction script kept beside the register so the two
   counts compare.
4. Write three lines here: what shipped, what was skipped and why, and the
   three things that hurt most while doing it. Those three start the next
   list.
