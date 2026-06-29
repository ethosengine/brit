# brit memory index

One line per memory. This is the brit-local discipline index — the place the **next-generation, content-addressed memory/discipline is mastered before being brought back to the whole** monorepo (see `../../CLAUDE.md`). Keep entries to a single pointer line; content lives in the linked file.

## project

- [Canonical EPR-meta & the git bridge design](../../docs/specs/2026-06-29-canonical-epr-meta-git-bridge-design.md) — next-gen `epr-meta` = content-addressed `EprMeta`/`NodeSeed` artifacts (CIDv1 via published `elohim-epr` 0.1) + git-as-bridge + two-layer (static CID / runtime notarization) + import/export world-contract; canonical-first, bits-to-apps; supersedes the parent's Python epr-meta discipline. Branch: `brit-dev`.
- [EPR-meta composition snapshot — canonical cites & parity slice](../../docs/specs/2026-06-29-epr-meta-composition-snapshot-canonical-cites-design.md) — the Snapshot discipline: generic cite verdict engine + EprMeta/NodeSeed import/export envelope + `brit epr-meta status`, proven at parity with the parent cite oracle; governance (floor/ceiling, Dunbar-graduated stewardship, Mishpat::Commitment) is Layer-2/DHT, deferred.
- elohim-epr 0.1.0 is PUBLISHED (2026-06-29) to the internal `elohim` Nexus registry — consume as `elohim-epr = "0.1"` (needs `[registries.elohim]` in `.cargo/config.toml` + the cross-format Nexus token). Do NOT reproduce the codec; it is the source of truth for CIDv1 · dag-cbor · sha2-256.

## reference

- brit is a gitoxide fork; `gix-*` crates are upstream parity (do NOT do parity-maintenance work). The EPR layer is `brit-epr` / `brit-graph` / `brit-cli` (`rakia` bin) / `brit-build-ref` / `brit-verify`. Generic `brit-epr::engine` stays codec-agnostic; the `elohim` vocabulary is behind feature `elohim-protocol`.
- Dev-loop: `just test` (clippy+check+doc+unit+journey), `just check`, `cargo +nightly fmt -- --config-path rustfmt-nightly.toml`. Clippy pedantic via workspace lints. `cargo nextest run` for units.
