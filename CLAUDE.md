# CLAUDE.md — brit

Guidance for Claude Code when working in the `brit` submodule. (Upstream gitoxide conventions: `.github/copilot-instructions.md`. Quick cargo/just reference: `.claude/instructions.md`.)

## What brit is

brit is a **fork of gitoxide** that makes version control **covenantal**: every commit carries three-pillar EPR metadata (lamad/shefa/qahal) as RFC-822 trailers, and git artifacts project to **content-addressed ContentNodes**. The `gix-*` crates (~60) are upstream parity — **do not do gitoxide parity-maintenance**; the first-party work is `brit-epr` (engine + elohim app-schema), `brit-graph`, `brit-cli` (binary: `brit`; `rakia` remains a deprecated alias), `brit-build-ref`, `brit-verify`. Master design: `docs/specs/2026-04-12-brit-design.md`.

**Engine boundary (load-bearing):** `brit-epr::engine` is the generic covenant engine and MUST stay codec-/protocol-agnostic — disabling feature `elohim-protocol` must leave a working real-CID git tool. The Elohim vocabulary lives behind that feature flag.

## The brit-dev mission (current focus)

`brit-dev` is where the **next-generation `epr-meta`** is being designed and prototyped: content-addressed `EprMeta` / `NodeSeed` artifacts, an internal **git bridge**, two-layer notarization (static CID identity + runtime-supplied blessing), and a compiler-grade import/export contract — **canonical-first, git-as-bridge, from bits to apps.** Design: `docs/specs/2026-06-29-canonical-epr-meta-git-bridge-design.md`.

This work is intended to **supersede the parent monorepo's memory/discipline machinery** (its `epr_meta.py` / `cite_graph.py` / `placement-audit` toolchain), not replicate it inward. **Master the discipline here; bring it back to the whole.** Do NOT port the parent's Python enforcement tooling.

## Canonical content addressing

Three self-describing CIDv1 profiles, minted only by the one Rust addressing kernel **`elohim-address`** (published to the `elohim` Nexus registry; `BritCid` delegates to it, never re-implements it): **records** — dag-cbor `0x71` + sha2-256 (`bafyrei…`, `BritCid::compute`); **payload bytes** (files, manifests, artifacts) — raw `0x55` + BLAKE3-256 (`bafkr4i…`, `BritCid::compute_bytes`, admitted from text only by the full-decode `BritCid::parse_bytes`); **citations** — raw `0x55` + sha2-256 (`bafkrei…`, `BritCid::compute_raw`, cite fingerprints only). A CID names its multihash, so historical `bafkrei…` addresses stay verifiable under their own profile. The signature envelope comes from **`elohim-epr = "0.2"`**. Parity is proven against the kernel's vendored conformance vectors (`brit-epr/tests/cid_vectors.rs`).

**Notes:** historical notes keep the **frozen legacy format** (epoch 1: the node with `"signature": ""` in its DAG-CBOR preimage, hex ed25519, raw-key `agentId`) — never re-encoded; `v1_note_bytes_are_frozen` pins one. New notes are the **envelope format** `brit.<kind>@2` (`{epoch: 2, scheme, key, sig, body}`, transcript domain `brit:<kind>@2:`, key must equal `ed01 ‖ agentId`). An unknown epoch or scheme is **HELD** (`refs::Admission::Held`) — typed, never folded into the invalid, never a reuse candidate.

**Registry:** consuming `elohim-epr` needs `[registries.elohim]` in `.cargo/config.toml` + the cross-format Nexus token (already provisioned for `ethosenginebot`).

## Discipline

- **Docs cadence:** dated specs in `docs/specs/YYYY-MM-DD-*.md`, dated plans in `docs/plans/` (TDD checklists, bite-sized tasks), phase summaries in `docs/plans/phases/`. Decompose specs → plans before implementing.
- **Memory:** `.claude/memory/MEMORY.md` is the index (one pointer line per memory).
- **TDD:** make the scenario/test pass; the generic engine must build with `elohim-protocol` off.

## Dev-loop

- `just test` = clippy + check + doc + unit-tests + doc-tests + journey-tests. `just check` = feature-matrix `cargo check`. `cargo test -p <crate>` for a single crate (nextest is not installed in this workspace).
- Format: `cargo +nightly fmt -p <brit-crate> -- --config-path rustfmt-nightly.toml` then `cargo +stable fmt --check`. Scope the nightly pass to the `brit-*` crates — never run it over the workspace: its import grouping rewrites upstream `gix-*` files, and every such diff becomes merge noise against gitoxide (the 2026-09 upstream merge had to reset 63 of them).
- Lints: clippy `pedantic` via root `[workspace.lints.clippy]`; every brit crate opts in with `lints.workspace = true`.

## Integration

brit is a submodule of the elohim monorepo at `elohim/brit`. Work lands on brit `main` once its GitHub CI (`ci.yml`: "Tests pass") is green; verify risky changes on a `run-ci/**` branch first, since `ci.yml` also runs there. The monorepo pins brit by its submodule pointer and is the integration surface. `gix-main` is the upstream mirror kept current by `sync-upstream.yml`; bring upstream in with a merge commit. Upstream-owned `gix-*` code stays byte-identical to upstream apart from `registry = "elohim"` tags on the publish-set crates listed in `scripts/ci/cargo-publish-brit.sh`.
