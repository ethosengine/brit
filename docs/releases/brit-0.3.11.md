# Brit 0.3.11

Brit 0.3.11 lets `brit push` publish one notes ref under another notes name. Brit 0.3.10
and earlier refuse `refs/notes/A:refs/notes/B` and accept only a same-name transfer.

## What 0.3.11 contains

- **Mapped notes push.** `brit push <remote> refs/notes/<src>:refs/notes/<dst>` sends the local
  notes ref `<src>` to the remote notes ref `<dst>`. The existing limits still apply:
  - exactly one refspec, and both sides must be full names under `refs/notes/`;
  - no `+`, `--force` or `--force-with-lease`, no deletion and no `--prune`;
  - the new value must be a commit that fast-forwards the destination's current remote value,
    checked by ancestry without the commit graph.

  A refused push leaves the remote ref unchanged.
- **Fetch keeps the same-name rule.** `brit fetch` still accepts only
  `refs/notes/X:refs/notes/X`, and refuses a mapped notes refspec.

## Allocated versions

| Crate | 0.3.10 release | 0.3.11 release |
| --- | --- | --- |
| `brit-cli` | 0.3.10 | 0.3.11 |
| `gitoxide` | 0.61.10 | 0.61.11 |
| `gitoxide-core` | 0.64.10 | 0.64.11 |

Each crate in the table declares `rust-version = "1.100"`.
