# Brit 0.3.10

Brit 0.3.10 gives every feedback condition brit reports a stable claim id. It also stops the
cost a missing or slow recorder put on every command: brit 0.3.9 printed the recorder gap at
most once per session, but still spawned the recorder and waited up to two seconds each time.
The format is described in [docs/feedback.md](../feedback.md).

## What 0.3.10 contains

- **A compiled claims table.** Every condition brit reports has a literal claim id such as
  `brit/capability-absent:reset` or `epr/recorder-unavailable:timeout`, declared with its
  polarity, severity, weight, ceiling and what clears it. No id is built from input. A test
  pins the full id set, so ids are only ever added.
- **`brit feedback claims [--json]`** prints the table. Release archives carry the same JSON
  as `claims.json`, and `BUILD-INFO.txt` records its `claims_sha256`.
- **One capture per session and claim.** A marker checked before the recorder is spawned skips
  repeats, so a repeated claim costs no process. When the recorder is unavailable, nothing is
  spawned again in that session until the `epr` on `PATH` changes.
- **Recorder gaps are kept for epr.** The recorder cannot record its own absence, so brit
  writes each gap to `$XDG_STATE_HOME/brit/feedback-spool/` as a small JSON file (spool v1,
  at most 256 files). epr reads and drains them.
- **Claim ids on stderr.** Bridge refusals, typed refusals and the recorder gap line end with
  ` [fb <claim id>]`. stdout and Git-parity stderr are unchanged.
- **Claim and emitter reach the recorder as environment.** `EPR_FEEDBACK_CLAIM` and
  `EPR_FEEDBACK_EMITTER` are set for `epr feedback capture`; its arguments are unchanged, so
  an older epr records as before.

## Allocated versions

| Crate | 0.3.9 release | 0.3.10 release |
| --- | --- | --- |
| `brit-build-ref` | 0.1.12 | 0.1.13 |
| `brit-cli` | 0.3.9 | 0.3.10 |
| `brit-epr` | 0.2.5 | 0.2.6 |
| `brit-graph` | 0.2.8 | 0.2.9 |
| `gitoxide` | 0.61.9 | 0.61.10 |
| `gitoxide-core` | 0.64.9 | 0.64.10 |

Each crate in the table declares `rust-version = "1.100"`.
