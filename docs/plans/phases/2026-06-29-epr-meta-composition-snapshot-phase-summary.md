---
id: epr-meta-composition-snapshot-phase-summary
cites:
  - epr-meta-composition-snapshot | the slice spec this summarizes | sha256:fill-on-propagate
---

# Phase summary — EPR-meta composition snapshot (canonical cites & parity slice)

**Date:** 2026-06-29
**Status:** Landed on `brit-dev` (commit-only; operator integrates). Whole-branch review: **ready to merge, 0 blockers.**

Spec: `docs/specs/2026-06-29-epr-meta-composition-snapshot-canonical-cites-design.md`
Plan: `docs/plans/2026-06-29-epr-meta-composition-snapshot-canonical-cites-plan.md`

## What landed (12 tasks, 13 commits `fb3495ce81..a4ac3f43ad`)

- **(A) Drift fingerprint + first parity gate** — `engine/frontmatter.rs` (`split_frontmatter`/`canonical_body`/`drift_fingerprint` = `sha256:hex16` over the frontmatter-excluded trimmed body) + `tests/canonical_body_conformance.rs` (5 oracle-generated vectors). `BritCid: Ord` (A1).
- **(B) Generic verdict engine** — `engine/interface_ref.rs` (`InterfaceRef`/`EdgeKind`/`EdgeRole`/`parse_cite_line`), `engine/cite.rs` (`extract_id`/`extract_cites`/`SlugIndex`), `engine/verdict.rs` (`Verdict`, `dead>held>stale>ok`). All generic — no `elohim-protocol` vocabulary.
- **(C) Envelope + verbs + second parity gate + dogfood + hook** — `EprMeta`/`NodeSeed` lifted into `engine/` with `imports`/`exports`/`sub_seeds` (Decision-A `#[serde(default)]`, no `skip_serializing_if`); `seal` projects cites; `brit epr-meta status` verb; `tests/cite_parity.rs` verdict-label golden gate vs the live parent oracle; doc dogfood; advisory PostToolUse hook.

## The thesis, proven (not asserted)

Parity-supersession: brit reproduces the parent monorepo's `_lib.cite_graph` verdicts so the parent Python cite tooling can be retired for brit. **Both gates run against the LIVE oracle and are non-vacuous** — A3 (fingerprint conformance, byte-exact) and C4 (verdict-label parity across ok/stale/held/dead). The engine stays generic (`cargo build --no-default-features` green); the CID codec golden-vectors/byte-parity are untouched.

## Parity-hardening backlog (next slice — surfaced by the whole-branch review; latent, none block merge)

1. **Held matcher on relative paths** — `verdict.rs` `path.components().any(|c| == "held")` vs the oracle's substring `"/held/" in path`. Agree for absolute paths (always used → gates green); diverge only if `held` is the LEADING component of a relative path. Normalize or document.
2. **SlugIndex duplicate-id resolution** — `cite.rs` `or_insert_with` is **first-wins**; the oracle's `index[sid] = str(md)` is **last-wins**. On a duplicate-`id:` corpus the two could resolve different paths → different held/stale verdicts. No dup ids in the fixture. Decide the canonical rule + add a dup-id parity case.
3. **Frontmatter tolerance** — `extract_cites` keeps `- ` items after a blank line inside the `cites:` block (the oracle's `parse` resets `pending_list_key` on a blank); `extract_id`'s `strip_prefix("id:")` rejects `id : value` (space before colon) which the oracle's regex accepts. Latent on hand-malformed frontmatter; the tool-managed corpus has neither (spec §7.1 = corpus-scoped parity honesty).
4. **Advisory hook cold-build** — `.claude/hooks/epr-meta-status-signal.sh` sets `RUSTFLAGS=""` but not `CARGO_TARGET_DIR`; if the harness auto-loads the submodule `.claude/settings.json`, a `.md` edit could trigger a cold debug build (multi-second stall). Never blocks (`2>/dev/null || true`). Guard/pre-build before relying on auto-fire.
5. **Cosmetic** — redundant in-function `use ... frontmatter` in `seal`; `engine/mod.rs` `pub mod verdict;` placed among `pub use` lines; `status` prints a dangling space when a doc has frontmatter but no `id:`; `walk_md` follows symlinks (no cycle guard); `frontmatter.rs` `strip_suffix("\n---")` (frontmatter-only doc) untested.

## Deferred to later slices (genuinely deferred — no half-built stubs)

Layer-2 runtime governance (deterministic floor ↔ elohim ceiling, Dunbar-graduated stewardship, `Mishpat::Commitment` self-executing contracts with circuit-breaker tag-off, head election / successor candidates), the git bridge (oid↔CID), the version-DAG / `parent` commit-node + signed head records, functional edge kinds (`Content`/`SchemaVersion`/`Capability`/`Contract`/`External`), and `NodeSeed` consume-check (`A.imports ⊆ B.exports`). The envelope is converged-but-unpopulated (Decision A) so these compose inward without a re-encode. See spec §6.
