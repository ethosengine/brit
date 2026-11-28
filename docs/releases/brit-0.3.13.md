# Brit 0.3.13

Brit 0.3.13 adds the `grantsCarry` covenant clause, which a steward signs to let one device
carry named gate projects for named branches for at most a day. Brit records the clause and
checks its shape; whether the grant covers a run, and whether the party may give it, are
decided by epr. The format is described in
[the session covenant spec](../specs/2026-10-19-session-covenant-design.md).

## What 0.3.13 contains

- **The `grantsCarry` kind.** Its payload has exactly three fields. `runner` is the carrying
  device as an Ed25519 `did:key`. `projects` names gate projects, or is exactly `["*"]`.
  `refs` holds branch-name globs without the `refs/heads/` prefix, with `*` as the only
  metacharacter. Entries are non-empty and contain no whitespace. Unknown fields are refused.
- **A bounded window.** A `grantsCarry` clause requires `validUntil`, no earlier than
  `validFrom` and at most 24 hours after it.
- **`covenant put` checks before signing.** A refused clause writes nothing.
- **Older readers refuse the new kind.** A reader that does not know a clause kind reports
  the record as unverifiable rather than reading it as another kind.
- **List by kind.** `brit-build-ref covenant list --kind grantsCarry` lists the verified
  clauses of that kind across every session. With `--session`, it filters that session.

## Allocated versions

| Crate | 0.3.12 release | 0.3.13 release |
| --- | --- | --- |
| `brit-cli` | 0.3.12 | 0.3.13 |
| `brit-epr` | 0.2.6 | 0.2.7 |
| `brit-build-ref` | 0.1.13 | 0.1.14 |
| `brit-graph` | 0.2.9 | 0.2.10 |

brit-graph has no source change. Its published package carries a Cargo.lock, and that lock now resolves brit-epr 0.2.7, so 0.2.9 could not be republished.

Each crate in the table declares `rust-version = "1.100"`.
