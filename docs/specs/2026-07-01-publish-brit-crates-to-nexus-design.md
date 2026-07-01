# Publish brit crates to the Nexus cargo registry — design

**Date:** 2026-07-01
**Repo:** `ethosengine/brit` (a fork of GitoxideLabs/gitoxide + brit-authored crates)
**Companion:** `elohim/rakia` Nexus publish pipeline (`scripts/ci/cargo-publish-rakia.sh`), `project_brit_rakia_nexus_ci` memory
**Status:** design approved 2026-07-01; implementation pending

## Problem

brit's crates are not consumable from a registry. The `brit-*` crates sit at
version `0.0.0` and their cross-crate dependencies are `path`-only, so nothing
outside a full brit checkout can depend on them. The goal is to publish the
brit-authored crate surface to the internal Nexus cargo registry
(`cargo-internal`, aliased `elohim`) so they can be consumed as registry
dependencies — the same way brit already *consumes* `rakia-core`/`rakia-brit`
and `elohim-epr` from Nexus.

### Why this is not a one-liner

brit is a **fork of gitoxide**. The brit-authored crates depend on the *forked*
gix crates by path:

- `brit-epr` → `gix-object` (path fork; carries the SHA-256 `feat!` divergence
  `add hash_kind to TreeRefIter and Data`)
- `brit-verify` → `gix` (path fork, full)

Because the fork's SHA-256 change is a **breaking API change** at the *same
version* as crates.io, fork crates and upstream crates cannot be mixed at a type
boundary — the fork must be treated as a coherent unit, not cherry-picked.

## Scope (Approach B — chosen)

`brit-verify` is the only crate that path-depends on the **full** forked `gix`
(≈67 crates). It is a **binary**, consumed via the ci-builder image build, not
as a library — nothing depends on it as a registry crate. So it stays
`publish = false`, and the registry closure collapses to **15 crates**:

**4 brit-authored** (currently `0.0.0`):
`brit-epr`, `brit-graph`, `brit-build-ref`, `brit-cli`

**11 forked gix-\*** (keep upstream versions):
`gix-object 0.58.0`, `gix-hash 0.23.0`, `gix-actor 0.40.0`, `gix-date 0.15.1`,
`gix-error 0.2.1`, `gix-features 0.46.2`, `gix-hashtable 0.13.0`,
`gix-path 0.11.2`, `gix-trace 0.1.18`, `gix-utils 0.3.1`, `gix-validate 0.11.0`

`rakia-core 0.1.0` / `rakia-brit 0.1.0` are already published (dependencies of
`brit-cli`) and are out of scope.

The closure is derived mechanically from `cargo metadata`: the set of
path/workspace members (`source == null`) reachable via normal+build deps from
the four brit publishables. Re-run the closure check after the manifest rewrites
to confirm no drift.

Rejected alternatives:
- **Approach A** — publish the entire forked workspace (~72 crates). Maximum
  completeness but republishes 67 gix forks that must be re-synced with upstream
  forever, and maximizes crates.io collision surface. Not worth it when nothing
  consumes `brit-verify` as a library.
- **Approach C** — status quo (build brit from source into the ci-builder image,
  migration-roadmap Stage 1b). Zero registry work; kept as the baseline.

## Design

### 1. Versioning

- **brit-\* crates**: `0.0.0` → **`0.1.0`**, matching rakia's published epoch.
- **11 forked gix-\* crates**: **keep upstream versions** as-is. They publish to
  the `cargo-internal` **hosted** registry, a namespace distinct from the
  crates.io **mirror** (`sparse+.../repository/cargo/`), so forked
  `gix-object 0.58.0` @ `registry=elohim` never collides with upstream
  `gix-object 0.58.0` @ crates.io in cargo's resolution graph.
- **Immutability discipline**: registry versions are write-once. If the fork
  later re-diverges an *already-published* gix crate at the same upstream
  version, that publish is a no-op (409) and the change will NOT propagate — such
  a change requires a version bump (e.g. `0.58.0` → `0.58.1`). Documented in the
  publish script docstring and the `project_brit_rakia_nexus_ci` memory.

### 2. Dependency rewrites (dual path + registry)

For all 15 crates, every dependency **within the publish set** carries both:
- `path = "..."` — for local/workspace dev (cargo prefers path in-workspace), and
- `version = "..."` + `registry = "elohim"` — what cargo emits into the published
  metadata, so consumers resolve from Nexus.

Concrete edits:
- `brit-epr`: `gix-object = { version = "0.58", registry = "elohim", path = "../gix-object", features = ["sha1"] }`
- `brit-graph`, `brit-build-ref`: `brit-epr` dep gains `version = "0.1", registry = "elohim"` (currently `^0.0.0` path)
- `brit-cli`: `brit-graph` dep gains `version = "0.1", registry = "elohim"` (rakia deps already correct)
- each of the 11 `gix-*` forks: every intra-fork dep (e.g. `gix-object`'s deps on
  `gix-hash`, `gix-actor`, `gix-features`, `gix-date`, `gix-hashtable`,
  `gix-utils`, `gix-validate`, `gix-trace`, `gix-path`, `gix-error`) gains
  `registry = "elohim"` alongside its existing path + version.

**Correctness invariant**: without `registry = "elohim"` on the inter-fork deps,
cargo's published metadata defaults them to **crates.io** — silently pulling
*upstream* gix instead of the fork. This rewrite is what makes the fork cohere on
the registry.

**Unchanged**: `brit-cli`'s *direct* `gix = "0.81"` dep stays pointed at the
mirror (upstream). This preserves today's exact source topology — upstream `gix`
for the CLI's own use, forked `gix-object` under `brit-epr` — so the published
graph behaves identically to the local build (which already compiles with both a
mirror `gix-object` and a path fork `gix-object` as distinct-source packages).

**Local dev unaffected**: `cargo build`/`check`/`test` in a brit checkout still
use the path crates; `registry` coords only take effect when a crate is consumed
*from* Nexus.

### 3. Publish pipeline

Mirror rakia's proven pattern:

- **`scripts/ci/cargo-publish-brit.sh`** publishes the 15 crates to
  `cargo-internal` in topological order:
  1. leaf gix forks: `gix-trace`, `gix-hash`, `gix-error`, `gix-path`,
     `gix-date`, `gix-utils`, `gix-validate`, `gix-hashtable`, `gix-features`,
     `gix-actor`
  2. `gix-object`
  3. `brit-epr` → `brit-graph` → `brit-build-ref`, `brit-cli`

  (Verify the topo order against `cargo metadata` at implementation time; encode
  it explicitly with a comment, or topo-sort from metadata.) **409 / "already
  exists" is treated as success** for idempotent re-runs, exactly like
  `cargo-publish-rakia.sh`. Auth reuses the `NEXUS_NPM_TOKEN` → derived `Bearer`
  cargo header pattern (`CARGO_REGISTRIES_ELOHIM_TOKEN="Bearer ${NEXUS_NPM_TOKEN}"`,
  `CARGO_REGISTRY_GLOBAL_CREDENTIAL_PROVIDERS=cargo:token`).

- **CI wiring**: add a `publish` job to `.github/workflows/ci.yml`, **gated on
  `main`**, running after build/test pass (mirrors rakia's job at ci.yml:72).
  brit already holds `CARGO_REGISTRIES_ELOHIM_TOKEN` (read); publishing needs a
  **write-capable** token, so brit gains a `NEXUS_NPM_TOKEN` secret (same
  ethosenginebot cargo-deployer value as rakia's).

- **First publish de-risked locally**: the script runs identically locally (with
  the token env) or in CI. Do a dry-run of the full topo list, then the first
  real publish, from a trusted environment; CI keeps it current on subsequent
  `main` merges.

- **`cargo publish` verification**: publish in strict topo order so each dep is
  live on the registry before its dependent is verified. Use `--no-verify` only
  if a fork crate's verify step is blocked by the source-replacement config
  (`.cargo/config.toml` replaces `crates-io` with the `elohim-mirror` proxy);
  confirm which crates, if any, need it during implementation.

### 4. Verification & rollback

Verification gates:
1. `cargo metadata` closure re-check — publish set is exactly these 15 after the
   rewrites, no drift.
2. `cargo package --list` per crate — each packages cleanly (no stray path
   escapes; ignore/`.epr-meta` patterns OK).
3. Local-dev regression: `cargo check -p brit-cli -p brit-verify` in a full brit
   checkout still resolves via paths.
4. Standalone-consumer proof: in a scratch dir with no sibling fork,
   `cargo add brit-cli --registry elohim` (or a tiny crate depending on
   `brit-epr`) resolves and builds purely from Nexus. This is the real
   acceptance test that the fork coheres on the registry.
5. `curl` the sparse index for `gix-object`, `brit-epr`, `brit-cli` → 200 with
   the expected versions.

Rollback: registry publishes are immutable (no unpublish, only `cargo yank`).
Safeguard: dry-run the whole topo list first; publish gix leaves → `gix-object`
→ brit-\* only after the dry-run is clean. A mid-sequence failure resumes via the
idempotent script (already-published = success). A bad version is handled by
`cargo yank` + a version bump, not deletion.

### 5. Docs / memory

- Update `project_brit_rakia_nexus_ci` memory: add the brit-publish closure (15
  crates), the keep-upstream-versions decision, and the immutability rule.
- `genesis/docs/integrations/brit-migration-roadmap.md` Stage 1b: note the new
  alternative — the ci-builder image can `cargo install brit-cli
  brit-build-ref --registry elohim` instead of building the fork from a submodule
  (once the binaries' closure is on Nexus).

## Out of scope

- Publishing `brit-verify` (binary; stays `publish = false`) or the other ~56 gix
  forks not in the closure.
- Changing the ci-builder image (separate `ee-jenkins-ci-builder` repo; a
  follow-on once crates are live).
- Any change to the SHA-256 fork behavior itself.
