# Brit 0.3.14

Brit 0.3.14 makes Brit and epr agree on where the feedback spool lives, and stops leftover
temporary files from filling it. The format is described in [feedback](../feedback.md).

## What 0.3.14 contains

- **One directory rule.** An XDG variable counts only when it is absolute. Otherwise Brit uses
  `.cache` or `.local/state` under `HOME`, only when `HOME` is absolute. On Windows, Brit then
  falls back to the profile directory. Otherwise there is no capture directory, and Brit writes
  no spool records, markers or notices. Before this release, a relative or empty `HOME` put
  records under the current directory, and an unset `HOME` fell back to the passwd home. epr
  reads neither, so those records were never drained.
- **Temporaries do not fill the spool.** The 256-entry limit counts `.json` records only. Under
  the spool lock, Brit removes `.tmp` files last modified more than an hour ago.

## Allocated versions

| Crate | 0.3.13 release | 0.3.14 release |
| --- | --- | --- |
| `brit-cli` | 0.3.13 | 0.3.14 |

No library crate changed, so no other crate needs a new version.

Each crate in the table declares `rust-version = "1.100"`.
