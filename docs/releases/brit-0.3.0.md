# Brit 0.3.0

Brit 0.3.0 brings gitoxide's upstream work into Brit, lands the twelve lanes of
sprint 3 together with it, and adds four daily-driver fixes chosen from our own
Brit usage. The executable version comes from `brit-cli`; library crates keep
their own versions.

## What 0.3.0 contains

- **Upstream gitoxide.** The 2026-11-19 upstream integration (468 commits over
  0.2.6) is merged with sprint 3 in one merge commit. Upstream-owned `gix-*`
  and gitoxide-core code stays as upstream ships it, apart from `registry`
  tags and the push, trust and history changes Brit carries inside those
  crates. The encoding groups `show` and `log` need are enabled on the `brit`
  executable's own `gix` dependency, not in gitoxide-core's manifest.
- **Sprint 3.** The twelve lanes integrated on `run-ci/sprint3-integrate`
  (`6507caaaa1`): the rebase recovery lane with `brit rebase --retire`, the
  declared-trust and push-transport lanes, the read-only closed-reader
  extension, `log` revisions and paths, `diff` forms, and the rest recorded in
  the sprint-3 handoff.
- **Delivered push reports success.** A push that delivered its refs exits 0
  when its report stream is closed; `build baseline write` and `migrate`
  follow the same rule. The push opens one connection when no hook ran and a
  second only after a hook ran. A push whose configured hooks directory or
  `pre-push` hook cannot run is refused before anything is sent, naming the
  path and `--no-verify`. `--atomic` asks the remote for all-or-nothing; notes
  refs fast-forward like branches and an existing tag moves only with
  `--force`.
- **Closed stdout.** `brit-build-ref`, `brit-verify` and
  `brit-rebase-retained` no longer panic when their reader goes away:
  read-only commands end quietly with status 141 (Git's SIGPIPE status), and
  commands that already changed something exit 0. Each Brit binary installs a
  panic hook that names the binary, version and location and says it is a
  Brit bug; the exit status stays 101.
- **Encodings and wording.** `brit log` decodes a commit's `encoding` header
  the way `show` does. A refusal of `-C` names `-C <DIR>`, not the internal
  argument id.
- **CI.** `test-journey` runs 21 journey files it did not list before, and
  `Tests pass` names each dependency that did not succeed, saying when it was
  cancelled.

## Recorded verification

Each lane passed `Tests pass` on its own `run-ci/lane-*` branch (runs
44811229071, 44818923773, 44820023096, 44828926787), on top of the merge head
`91942dfd7` (run 44798244116). The release head's own run is the release
evidence; the delivered-push fix is pinned by
`delivered_push_succeeds_when_its_reader_closed_the_report_stream`, which fails
with the fix reverted.

## Not qualified

- `advisories@linux-x86_64` (cargo-deny) is red, as on 0.2.6's main; advisories
  were not suppressed.
- `size@linux-x86_64` is red: 409.5 KB against a 225 KB budget (320.5 KB before
  the upstream merge).
- `tests@windows-x86_64`, the 32-bit Linux legs and FreeBSD remain observed,
  not maintained (`.epr-meta/readiness.json`). macOS ARM release assets stay
  suspended.
- The monorepo's "use Git for landing pushes" exception is not retired by this
  release alone: it needs a pinned 0.3.0 and supervised real landings first.
