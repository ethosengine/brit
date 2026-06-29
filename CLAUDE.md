# CLAUDE.md — brit

Guidance for Claude Code when working in the `brit` submodule. (Upstream gitoxide conventions: `.github/copilot-instructions.md`. Quick cargo/just reference: `.claude/instructions.md`.)

## What brit is

brit is a **fork of gitoxide** that makes version control **covenantal**: every commit carries three-pillar EPR metadata (lamad/shefa/qahal) as RFC-822 trailers, and git artifacts project to **content-addressed ContentNodes**. The `gix-*` crates (~60) are upstream parity — **do not do gitoxide parity-maintenance**; the first-party work is `brit-epr` (engine + elohim app-schema), `brit-graph`, `brit-cli` (binary: `rakia`), `brit-build-ref`, `brit-verify`. Master design: `docs/specs/2026-04-12-brit-design.md`.

**Engine boundary (load-bearing):** `brit-epr::engine` is the generic covenant engine and MUST stay codec-/protocol-agnostic — disabling feature `elohim-protocol` must leave a working real-CID git tool. The Elohim vocabulary lives behind that feature flag.

## The brit-dev mission (current focus)

`brit-dev` is where the **next-generation `epr-meta`** is being designed and prototyped: content-addressed `EprMeta` / `NodeSeed` artifacts, an internal **git bridge**, two-layer notarization (static CID identity + runtime-supplied blessing), and a compiler-grade import/export contract — **canonical-first, git-as-bridge, from bits to apps.** Design: `docs/specs/2026-06-29-canonical-epr-meta-git-bridge-design.md`.

This work is intended to **supersede the parent monorepo's memory/discipline machinery** (its `epr_meta.py` / `cite_graph.py` / `placement-audit` toolchain), not replicate it inward. **Master the discipline here; bring it back to the whole.** Do NOT port the parent's Python enforcement tooling.

## Canonical content addressing

Identity is **CIDv1 · multicodec `0x71` dag-cbor · multihash `0x12` sha2-256** (`bafyrei…`); raw blobs use codec `0x55` (`bafkrei…`). The codec is **not reproduced** — consume the published **`elohim-epr = "0.1"`** crate from the `elohim` Nexus registry (`elohim_epr::cid::compute_cid`, `cbor::encode`, plus the `Epr`/`Coupling`/`Reach`/`EprKind`/`proof` vocabulary). Byte-parity is proven against the vendored golden vectors (`tests/cid_vectors.rs`, `tests/canonical_bytes.rs`) — the cross-implementation conformance spec (so the engine could be reimplemented in machine code). BLAKE3 is for non-address fingerprints (dedup/index keys) ONLY — never as a content address.

**Registry:** consuming `elohim-epr` needs `[registries.elohim]` in `.cargo/config.toml` + the cross-format Nexus token (already provisioned for `ethosenginebot`).

## Discipline

- **Docs cadence:** dated specs in `docs/specs/YYYY-MM-DD-*.md`, dated plans in `docs/plans/` (TDD checklists, bite-sized tasks), phase summaries in `docs/plans/phases/`. Decompose specs → plans before implementing.
- **Memory:** `.claude/memory/MEMORY.md` is the index (one pointer line per memory).
- **TDD:** make the scenario/test pass; the generic engine must build with `elohim-protocol` off.

## Dev-loop

- `just test` = clippy + check + doc + unit-tests + doc-tests + journey-tests. `just check` = feature-matrix `cargo check`. `cargo nextest run -p <crate>` for a single crate.
- Format: `cargo +nightly fmt -- --config-path rustfmt-nightly.toml` then `cargo +stable fmt --check`.
- Lints: clippy `pedantic` via root `[workspace.lints.clippy]`; every brit crate opts in with `lints.workspace = true`.

## Integration

brit is a submodule of the elohim monorepo at `elohim/brit`. Work lands on `brit-dev` (commit-only here; the operator integrates). The monorepo is the integration surface.
