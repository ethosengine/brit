# Brit 0.2.0

Brit 0.2.0 releases the measured improvements toward more reliable daily use.
The next release is prioritised from feedback logs of our own Brit usage.

## Changes for daily use

- Bounded object handling streams or maps large inputs across reading, staging,
  restore, comparisons, history, transfer and Tix paths. Existing allocation
  settings and reduced-trust policy still apply.
- Switch/pull recovery and replay journals use raw payloads; retained-copy
  capture streams inputs. The recorded cases cover preserving staged and dirty
  bytes through recovery and abort.
- Receipt compatibility restores legacy flat epoch-1 reads alongside epoch-2
  envelopes, preserving legacy preimages and record CIDs. The CLI returns
  verified flat rows understood by the existing gate adapter.
- Dependency updates address Hickory, rustix and bundled SQLite advisories and
  update process control through nix. The frontend minimum is Rust 1.89;
  the build toolchain remains 1.100 and the gix library minimum remains 1.88.
  The remaining pin-utils maintenance exception follows async-std's retirement.

## Recorded measurements

The daily-driver plan records Linux runs under a 1 GiB whole-cgroup cap with
swap disabled, counting Brit and owned children. These are historical results
for the source and executable identities recorded there, not a new-build claim.

- [“Fixed-ceiling memory work — 2026-11-13, incomplete” and “Measured local proof, not final qualification”](../plans/2026-09-28-daily-driver.md#fixed-ceiling-memory-work--2026-11-13-incomplete)
  describe streamed/mapped inputs and loose/packed object fixtures.
- [“Public-command audit in progress”](../plans/2026-09-28-daily-driver.md#public-command-audit-in-progress)
  records the measured comparison, checkout/recovery, local transfer, history,
  metadata, helper and Tix paths, alongside their remaining proof.
  It also records receipt compatibility and the capped gate-adapter check.
- [“2026-11-15 — bounded note reads and rewrite inputs”](../plans/2026-09-28-daily-driver.md#2026-11-15--bounded-note-reads-and-rewrite-inputs)
  records both-hash note reading, rewriting and rollback under the cap.
- [“2026-11-15 — remaining dependency advisories”](../plans/2026-09-28-daily-driver.md#2026-11-15--remaining-dependency-advisories)
  records recovery and corpus measurements after the dependency changes.

## Not qualified

The complete public-command matrix, separate 1 GiB aggregate scratch proof and
full both-device qualification are not established. Scratch peaks remain
unmeasured in the cited results. Open command paths, fixture limitations and
feedback capture follow-up are in the [bounded-memory backlog](../plans/2026-11-15-bounded-memory-backlog.md).
The [scope ruling](../plans/2026-09-28-daily-driver.md#2026-11-15--020-release-scope-ruling)
sets the six release stations; the daily-driver habit remains RED.
