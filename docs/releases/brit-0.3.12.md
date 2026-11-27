# Brit 0.3.12

Brit 0.3.12 completes the review findings on the feedback lifecycle from 0.3.10, and makes
the `cures` list cumulative so a device that skips a release still learns of its cures. The
format is described in [docs/feedback.md](../feedback.md).

## What 0.3.12 contains

- **One skip marker per outcome.** The marker key always includes the outcome, so each
  (producer, claim, outcome, emitter, session) gets its own record. Earlier releases left the
  outcome out for claims other than commands unless it was `passed`.
- **A full spool says so.** When the feedback spool reaches its 256-entry limit, brit prints
  one line on stderr per session, subject to the same quiet settings as the gap notice.
  Earlier releases stopped recording gaps silently.
- **Bounded marker and notice directories.** `feedback-markers/` and `feedback-notices/` hold
  at most 4096 entries each. At the limit brit removes files older than 7 days and never
  removes lock files. If a directory is still full, brit skips the marker; the recorder may
  run again, or a notice may repeat, but no record is lost.
- **Cumulative cures.** Each `cures` entry has `claim`, `release`, `state` (`cured` or
  `withdrawn`), `cite` and `summary`. The list is append-only from the floor release 0.3.10,
  with at most one entry per claim and release. A claim's state is its latest entry by
  semantic-version precedence. Brit's own `passed` capture follows that rule. The list is
  empty in this release.

## Allocated versions

| Crate | 0.3.11 release | 0.3.12 release |
| --- | --- | --- |
| `brit-cli` | 0.3.11 | 0.3.12 |

Each crate in the table declares `rust-version = "1.100"`.
