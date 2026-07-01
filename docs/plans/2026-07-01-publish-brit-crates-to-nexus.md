# Publish brit crates to Nexus — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Publish the 4 brit-authored crates + the 11 forked `gix-*` crates in `brit-epr`'s `gix-object` path-closure (15 total) to the internal Nexus cargo registry (`elohim`), so they're consumable as ordinary registry dependencies.

**Architecture:** Add dual `path` + `version`+`registry="elohim"` coordinates to every intra-set dependency (path wins for local dev; registry coords drive published metadata + external resolution). Bump the 4 brit-authored crates `0.0.0` → `0.1.0`; keep the 11 forked gix versions at upstream. Publish via a rakia-style idempotent, `--no-verify`, topologically-ordered script, wired as a `main`-gated CI job.

**Tech Stack:** Rust / Cargo (sparse registry), Nexus `cargo-internal` hosted repo, GitHub Actions, bash.

## Global Constraints

- **Registry alias**: `elohim` → `sparse+https://nexus.ethosengine.com/repository/cargo-internal/` (already defined in `.cargo/config.toml`).
- **Auth for any publish/resolve command**: `export CARGO_REGISTRIES_ELOHIM_TOKEN="Bearer ${NPM_TOKEN}"` and `export CARGO_REGISTRY_GLOBAL_CREDENTIAL_PROVIDERS=cargo:token`. (`NPM_TOKEN` is the ethosenginebot cargo-deployer token, present in this environment.)
- **Native-build env** (this is a native, not WASM, workspace): `export RUSTFLAGS=""` and `export RUSTC_WRAPPER=""` for every cargo invocation. Set `export CARGO_TARGET_DIR=/tmp/claude-0/-projects-elohim/def5ef25-b752-4f4b-b13f-3ff3b1e738b1/scratchpad/brit-target` to avoid the fingerprint-ENOENT trap on the projects volume.
- **Publish set (15 crates)**: `gix-trace 0.1.18`, `gix-utils 0.3.1`, `gix-validate 0.11.0`, `gix-error 0.2.1`, `gix-path 0.11.2`, `gix-features 0.46.2`, `gix-hash 0.23.0`, `gix-hashtable 0.13.0`, `gix-date 0.15.1`, `gix-actor 0.40.0`, `gix-object 0.58.0`, `brit-epr 0.1.0`, `brit-graph 0.1.0`, `brit-build-ref 0.1.0`, `brit-cli 0.1.0`. `rakia-core`/`rakia-brit` already published.
- **Topological publish order** (deps before dependents): the 15-crate list above is already in a valid topological order — use it verbatim.
- **Do NOT edit `[dev-dependencies]`**: cargo strips path-only dev-deps at publish; touching them is unnecessary and risks breaking the workspace. Only `[dependencies]` / `[build-dependencies]` are rewritten.
- **Do NOT touch** `brit-cli`'s direct `gix = "0.81"` dep (stays crates.io/mirror = upstream), `brit-verify` as a publish target (stays `publish = false`), or the `.cargo/config.toml` `paths` override.
- **Working branch**: `feat/publish-brit-crates` (already created; the design spec is committed there). Commit only — the operator pushes/merges.

---

### Task 1: Bump brit-authored crate versions to 0.1.0

**Files:**
- Modify: `brit-epr/Cargo.toml` (`[package]` version)
- Modify: `brit-graph/Cargo.toml` (`[package]` version)
- Modify: `brit-build-ref/Cargo.toml` (`[package]` version)
- Modify: `brit-cli/Cargo.toml` (`[package]` version)
- Modify: `brit-verify/Cargo.toml:18` (its `brit-epr` version req, so the workspace still resolves after the bump)

**Interfaces:**
- Produces: `brit-epr`, `brit-graph`, `brit-build-ref`, `brit-cli` at version `0.1.0`.

- [ ] **Step 1: Bump each brit-authored crate's `[package]` version**

In each of `brit-epr/Cargo.toml`, `brit-graph/Cargo.toml`, `brit-build-ref/Cargo.toml`, `brit-cli/Cargo.toml`, change the `[package]` section's:
```toml
version = "0.0.0"
```
to:
```toml
version = "0.1.0"
```
(Match the `version` line inside `[package]` only — each of these files has exactly one `version = "0.0.0"` in `[package]`.)

- [ ] **Step 2: Fix brit-verify's version requirement on brit-epr**

In `brit-verify/Cargo.toml:18`, change:
```toml
brit-epr = { version = "^0.0.0", path = "../brit-epr" }
```
to:
```toml
brit-epr = { version = "0.1", path = "../brit-epr" }
```

- [ ] **Step 3: Verify the workspace still resolves and builds via paths**

Run (with Global Constraints env set):
```bash
cargo check -p brit-cli -p brit-verify -p brit-epr -p brit-graph -p brit-build-ref
```
Expected: PASS (compiles clean; path deps resolve to the in-workspace 0.1.0 crates). A failure here means a `^0.0.0` version requirement somewhere still points at the old version — grep `grep -rn 'brit-\(epr\|graph\|build-ref\|cli\).*0\.0\.0' --include=Cargo.toml .` and fix.

- [ ] **Step 4: Commit**

```bash
git add brit-epr/Cargo.toml brit-graph/Cargo.toml brit-build-ref/Cargo.toml brit-cli/Cargo.toml brit-verify/Cargo.toml
git commit -m "chore(brit): version brit-authored crates 0.0.0 -> 0.1.0 for publish

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

### Task 2: Add `registry = "elohim"` to intra-fork gix-* dependencies

**Files:**
- Modify: `gix-object/Cargo.toml` (`[dependencies]`: gix-features, gix-hash, gix-hashtable, gix-validate, gix-actor, gix-date, gix-utils)
- Modify: `gix-hash/Cargo.toml` (gix-features)
- Modify: `gix-actor/Cargo.toml` (`[dependencies]`: gix-date, gix-error)
- Modify: `gix-date/Cargo.toml` (`[dependencies]`: gix-error)
- Modify: `gix-features/Cargo.toml` (gix-path, gix-trace, gix-utils)
- Modify: `gix-hashtable/Cargo.toml` (`[dependencies]`: gix-hash — line 24 only, NOT the dev-dep at line 27)
- Modify: `gix-path/Cargo.toml` (gix-trace, gix-validate)

**Interfaces:**
- Produces: every forked gix crate's *normal* dependency on another forked gix crate carries `registry = "elohim"` + a version, so `cargo publish` records fork-pointing metadata (default would silently resolve upstream on crates.io).

- [ ] **Step 1: Rewrite `gix-object/Cargo.toml` `[dependencies]`**

Add `registry = "elohim"` to each intra-fork dep. Change these lines (inside `[dependencies]`):
```toml
gix-features = { version = "^0.46.2", path = "../gix-features", features = [
    "progress",
] }
gix-hash = { version = "^0.23.0", path = "../gix-hash" }
gix-hashtable = { version = "^0.13.0", path = "../gix-hashtable" }
gix-validate = { version = "^0.11.0", path = "../gix-validate" }
gix-actor = { version = "^0.40.0", path = "../gix-actor" }
gix-date = { version = "^0.15.1", path = "../gix-date" }
gix-utils = { version = "^0.3.1", path = "../gix-utils" }
```
to:
```toml
gix-features = { version = "^0.46.2", path = "../gix-features", registry = "elohim", features = [
    "progress",
] }
gix-hash = { version = "^0.23.0", path = "../gix-hash", registry = "elohim" }
gix-hashtable = { version = "^0.13.0", path = "../gix-hashtable", registry = "elohim" }
gix-validate = { version = "^0.11.0", path = "../gix-validate", registry = "elohim" }
gix-actor = { version = "^0.40.0", path = "../gix-actor", registry = "elohim" }
gix-date = { version = "^0.15.1", path = "../gix-date", registry = "elohim" }
gix-utils = { version = "^0.3.1", path = "../gix-utils", registry = "elohim" }
```

- [ ] **Step 2: Rewrite the remaining gix crates' normal intra-fork deps**

In `gix-hash/Cargo.toml`, change:
```toml
gix-features = { version = "^0.46.2", path = "../gix-features", features = ["progress"] }
```
to:
```toml
gix-features = { version = "^0.46.2", path = "../gix-features", registry = "elohim", features = ["progress"] }
```

In `gix-actor/Cargo.toml` `[dependencies]`, change:
```toml
gix-date = { version = "^0.15.0", path = "../gix-date" }
gix-error = { version = "^0.2.0", path = "../gix-error" }
```
to:
```toml
gix-date = { version = "^0.15.0", path = "../gix-date", registry = "elohim" }
gix-error = { version = "^0.2.0", path = "../gix-error", registry = "elohim" }
```

In `gix-date/Cargo.toml` `[dependencies]`, change:
```toml
gix-error = { version = "^0.2.1", path = "../gix-error" }
```
to:
```toml
gix-error = { version = "^0.2.1", path = "../gix-error", registry = "elohim" }
```

In `gix-features/Cargo.toml`, change:
```toml
gix-trace = { version = "^0.1.18", path = "../gix-trace" }
gix-path = { version = "^0.11.2", path = "../gix-path", optional = true }
gix-utils = { version = "^0.3.1", path = "../gix-utils", optional = true }
```
to:
```toml
gix-trace = { version = "^0.1.18", path = "../gix-trace", registry = "elohim" }
gix-path = { version = "^0.11.2", path = "../gix-path", registry = "elohim", optional = true }
gix-utils = { version = "^0.3.1", path = "../gix-utils", registry = "elohim", optional = true }
```

In `gix-hashtable/Cargo.toml`, change ONLY the `[dependencies]` line (line 24), leaving the `[dev-dependencies]` gix-hash (line 27) untouched:
```toml
gix-hash = { version = "^0.23.0", path = "../gix-hash" }
```
to:
```toml
gix-hash = { version = "^0.23.0", path = "../gix-hash", registry = "elohim" }
```

In `gix-path/Cargo.toml`, change:
```toml
gix-trace = { version = "^0.1.18", path = "../gix-trace" }
gix-validate = { version = "^0.11.0", path = "../gix-validate" }
```
to:
```toml
gix-trace = { version = "^0.1.18", path = "../gix-trace", registry = "elohim" }
gix-validate = { version = "^0.11.0", path = "../gix-validate", registry = "elohim" }
```

- [ ] **Step 3: Verify the workspace still builds (path still wins over registry)**

Run:
```bash
cargo check -p gix-object -p brit-epr
```
Expected: PASS. Adding `registry` + `version` alongside `path` does not change local resolution — cargo uses the path. A failure indicates a malformed inline table (e.g. a missing comma); re-read the edited line.

- [ ] **Step 4: Commit**

```bash
git add gix-object/Cargo.toml gix-hash/Cargo.toml gix-actor/Cargo.toml gix-date/Cargo.toml gix-features/Cargo.toml gix-hashtable/Cargo.toml gix-path/Cargo.toml
git commit -m "chore(gix-fork): tag intra-fork deps with registry=elohim for Nexus publish

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

### Task 3: Add version + `registry = "elohim"` to brit-* intra-set dependencies

**Files:**
- Modify: `brit-epr/Cargo.toml:26` (gix-object)
- Modify: `brit-graph/Cargo.toml:17` (brit-epr)
- Modify: `brit-build-ref/Cargo.toml:18` (brit-epr)
- Modify: `brit-cli/Cargo.toml:18` (brit-graph)

**Interfaces:**
- Consumes: `gix-object` @ `registry="elohim"` (Task 2), `brit-epr`/`brit-graph` @ `0.1.0` (Task 1).
- Produces: the 4 brit publishables reference each other and the forked gix-object via registry coords in published metadata.

- [ ] **Step 1: Rewrite `brit-epr/Cargo.toml:26`**

Change:
```toml
gix-object = { version = "^0.58.0", path = "../gix-object", features = ["sha1"] }
```
to:
```toml
gix-object = { version = "^0.58.0", path = "../gix-object", registry = "elohim", features = ["sha1"] }
```

- [ ] **Step 2: Rewrite `brit-graph/Cargo.toml:17`**

Change:
```toml
brit-epr = { path = "../brit-epr", default-features = false }
```
to:
```toml
brit-epr = { version = "0.1", registry = "elohim", path = "../brit-epr", default-features = false }
```

- [ ] **Step 3: Rewrite `brit-build-ref/Cargo.toml:18`**

Change:
```toml
brit-epr = { version = "^0.0.0", path = "../brit-epr" }
```
to:
```toml
brit-epr = { version = "0.1", registry = "elohim", path = "../brit-epr" }
```

- [ ] **Step 4: Rewrite `brit-cli/Cargo.toml:18`**

Change:
```toml
brit-graph = { path = "../brit-graph", features = ["repo"] }
```
to:
```toml
brit-graph = { version = "0.1", registry = "elohim", path = "../brit-graph", features = ["repo"] }
```

- [ ] **Step 5: Verify closure unchanged and workspace builds**

Run:
```bash
cargo check -p brit-cli -p brit-build-ref -p brit-graph -p brit-epr -p brit-verify
```
Expected: PASS.

Then re-confirm the publish closure is exactly the 15 crates (drift guard):
```bash
python3 - <<'PY'
import json, subprocess
md = json.loads(subprocess.check_output(["cargo","metadata","--format-version","1"]))
pkgs = {p["id"]: p for p in md["packages"]}
local = {i for i,p in pkgs.items() if p.get("source") is None}
resolve = {n["id"]: n for n in md["resolve"]["nodes"]}
def deps(pid):
    r=[]
    for d in resolve[pid]["deps"]:
        if any(k.get("kind") in (None,"build") for k in d["dep_kinds"]): r.append(d["pkg"])
    return r
n2i={p["name"]:i for i,p in pkgs.items() if p.get("source") is None}
seen=set(); st=[n2i[r] for r in ("brit-cli","brit-epr","brit-graph","brit-build-ref")]
while st:
    x=st.pop()
    if x in seen: continue
    seen.add(x)
    st += [d for d in deps(x) if d in local]
print(sorted(pkgs[i]["name"] for i in seen))
PY
```
Expected: a 17-name list = the 15 publish crates + `rakia-brit` + `rakia-core`. If any other name appears, STOP — the closure drifted and the publish set/topo order must be revisited.

- [ ] **Step 6: Commit**

```bash
git add brit-epr/Cargo.toml brit-graph/Cargo.toml brit-build-ref/Cargo.toml brit-cli/Cargo.toml
git commit -m "chore(brit): tag brit-* intra-deps with registry=elohim + 0.1 versions

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

### Task 4: Write the topological publish script

**Files:**
- Create: `scripts/ci/cargo-publish-brit.sh`

**Interfaces:**
- Consumes: the rewritten manifests (Tasks 1-3), `NEXUS_NPM_TOKEN` or `CARGO_REGISTRIES_ELOHIM_TOKEN` in env.
- Produces: an idempotent, 409-safe publisher that pushes the 15 crates in topo order.

- [ ] **Step 1: Write the script**

Create `scripts/ci/cargo-publish-brit.sh` with exactly this content:
```bash
#!/usr/bin/env bash
# Publish the brit crate set to the internal Nexus registry (`elohim`).
# Mirrors rakia's cargo-publish-rakia.sh, adapted for a multi-version,
# topologically-ordered crate set (the forked gix-* crates keep their upstream
# versions; the brit-* crates are 0.1.x).
#
# Idempotent + 409-safe: a crate+version already on the index is a no-op
# success, so this runs on every main build and only publishes on a version bump.
#
# --no-verify: crates publish in dependency order, but the sparse index needs a
# moment to serve a just-published crate before a dependent's verify build could
# resolve it. The CI `test` job already builds+tests the whole workspace, so the
# verify build here is redundant; skipping it avoids the index-propagation race.
#
# Immutability: registry versions are write-once. If the fork later re-diverges
# an ALREADY-PUBLISHED gix crate at the same upstream version, this is a silent
# no-op — such a change requires a version bump (e.g. 0.58.0 -> 0.58.1).
#
# Auth: bind the CI npm token to NEXUS_NPM_TOKEN (cargo-deployer role on
# cargo-internal) and this derives the cargo Bearer header. Override: set
# CARGO_REGISTRIES_ELOHIM_TOKEN directly.
set -euo pipefail

if [ -z "${CARGO_REGISTRIES_ELOHIM_TOKEN:-}" ] && [ -n "${NEXUS_NPM_TOKEN:-}" ]; then
  export CARGO_REGISTRIES_ELOHIM_TOKEN="Bearer ${NEXUS_NPM_TOKEN}"
fi

REPO_ROOT="${GITHUB_WORKSPACE:-$(git rev-parse --show-toplevel)}"
INDEX_BASE="https://nexus.ethosengine.com/repository/cargo-internal"

# Topological order: dependencies before dependents.
CRATES=(
  gix-trace gix-utils gix-validate gix-error gix-path
  gix-features gix-hash gix-hashtable gix-date gix-actor
  gix-object
  brit-epr brit-graph brit-build-ref brit-cli
)

export RUSTFLAGS=""
export RUSTC_WRAPPER=""
export CARGO_REGISTRY_GLOBAL_CREDENTIAL_PROVIDERS="cargo:token"

# Cargo sparse-index path rules, by crate-name length.
sparse_path() {
  local n="$1"
  case "${#n}" in
    1) echo "1/${n}" ;;
    2) echo "2/${n}" ;;
    3) echo "3/${n:0:1}/${n}" ;;
    *) echo "${n:0:2}/${n:2:2}/${n}" ;;
  esac
}

# Read a crate's [package] version from its own Cargo.toml.
crate_version() {
  local crate="$1"
  awk '/^\[package\]/{p=1} p&&/^version[[:space:]]*=/{match($0,/"[^"]+"/); print substr($0,RSTART+1,RLENGTH-2); exit}' \
    "${REPO_ROOT}/${crate}/Cargo.toml"
}

cd "${REPO_ROOT}"

for crate in "${CRATES[@]}"; do
  version="$(crate_version "${crate}")"
  [ -n "${version}" ] || { echo "ERROR: could not parse version for ${crate}" >&2; exit 1; }
  index_url="${INDEX_BASE}/$(sparse_path "${crate}")"

  if curl -fsSL "${index_url}" 2>/dev/null | grep -q "\"vers\":\"${version}\""; then
    echo "${crate} ${version} already on the index — nothing to publish."
    continue
  fi

  if [ -z "${CARGO_REGISTRIES_ELOHIM_TOKEN:-}" ]; then
    echo "ERROR: no cargo registry credential — set NEXUS_NPM_TOKEN" >&2
    echo "       (or CARGO_REGISTRIES_ELOHIM_TOKEN directly)." >&2
    exit 1
  fi

  echo "Publishing ${crate} ${version} to the elohim registry…"
  set +e
  OUT="$(cargo publish -p "${crate}" --registry elohim --no-verify 2>&1)"
  RC=$?
  set -e
  echo "${OUT}"

  if [ ${RC} -ne 0 ]; then
    if echo "${OUT}" | grep -qiE 'already (exists|uploaded)|409|conflict'; then
      echo "${crate} ${version} already present (409) — treating as success."
      continue
    fi
    exit ${RC}
  fi
  echo "Published ${crate} ${version}."

  # Give the sparse index a beat to serve the new crate before its dependents.
  sleep 3
done

echo "brit publish stage complete."
```

- [ ] **Step 2: Make it executable and syntax-check it**

Run:
```bash
chmod +x scripts/ci/cargo-publish-brit.sh
bash -n scripts/ci/cargo-publish-brit.sh && echo "syntax OK"
```
Expected: `syntax OK`.

- [ ] **Step 3: Verify version parsing works for a fork and a brit crate**

Run:
```bash
bash -c 'source <(sed -n "/^crate_version()/,/^}/p" scripts/ci/cargo-publish-brit.sh); REPO_ROOT=$(git rev-parse --show-toplevel); echo "gix-object=$(crate_version gix-object) brit-cli=$(crate_version brit-cli)"'
```
Expected: `gix-object=0.58.0 brit-cli=0.1.0`.

- [ ] **Step 4: Commit**

```bash
git add scripts/ci/cargo-publish-brit.sh
git commit -m "ci(brit): add topological Nexus publish script for the brit crate set

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

### Task 5: Wire the publish job into brit CI (main-gated)

**Files:**
- Modify: `.github/workflows/ci.yml` (add a `publish` job)

**Interfaces:**
- Consumes: `scripts/ci/cargo-publish-brit.sh`, repo secrets `NEXUS_NPM_TOKEN` + `CARGO_REGISTRIES_ELOHIM_TOKEN` (both already set on the repo).

- [ ] **Step 1: Read the current workflow to find the last job and the toolchain-install step**

Run:
```bash
sed -n '1,200p' .github/workflows/ci.yml
```
Note the existing job that builds/tests (its `id`/`name`) and the checkout + Rust-install step shape, so the new job matches house style.

- [ ] **Step 2: Append the publish job**

Add this job to the `jobs:` map (adjust `needs:` to the actual id of the existing build/test job; use the same pinned `actions/checkout` SHA already used elsewhere in the file):
```yaml
  publish:
    name: Publish brit crates to Nexus (elohim registry)
    needs: [test]
    if: github.event_name == 'push' && github.ref == 'refs/heads/main'
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@de0fac2e4500dabe0009e67214ff5f5447ce83dd # v6.0.2
        with:
          submodules: false
          persist-credentials: false

      - name: Install Rust (stable)
        run: |
          rustup toolchain install stable --profile minimal
          rustup default stable

      - name: Publish brit crates (idempotent, 409-safe)
        env:
          NEXUS_NPM_TOKEN: ${{ secrets.NEXUS_NPM_TOKEN }}
          CARGO_REGISTRIES_ELOHIM_TOKEN: ${{ secrets.CARGO_REGISTRIES_ELOHIM_TOKEN }}
        run: bash scripts/ci/cargo-publish-brit.sh
```
If the existing build/test job is NOT named `test`, change `needs: [test]` to its real id.

- [ ] **Step 3: Validate the workflow YAML**

Run:
```bash
python3 -c "import yaml,sys; yaml.safe_load(open('.github/workflows/ci.yml')); print('yaml OK')"
```
Expected: `yaml OK`. Also confirm the `needs:` target matches a real job id:
```bash
python3 -c "import yaml; d=yaml.safe_load(open('.github/workflows/ci.yml')); j=d['jobs']; assert 'publish' in j; [assert_ok:=n in j for n in j['publish']['needs']]; print('jobs:', list(j)); print('publish.needs:', j['publish']['needs'])"
```
Expected: `publish.needs` values all appear in the printed `jobs` list.

- [ ] **Step 4: Commit**

```bash
git add .github/workflows/ci.yml
git commit -m "ci(brit): publish brit crate set to Nexus on main

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

### Task 6: First publish (de-risked, local) + standalone-consumer proof

> **GATE:** This task performs the real, irreversible publish to Nexus. Do NOT run Step 2+ without explicit operator go-ahead. Steps 1 (dry-run) is safe to run anytime.

**Files:** none (execution + verification only).

- [ ] **Step 1: Dry-run every crate (safe, no upload)**

With the Global Constraints env set (token + RUSTFLAGS + target dir), run per crate in topo order:
```bash
for c in gix-trace gix-utils gix-validate gix-error gix-path gix-features gix-hash gix-hashtable gix-date gix-actor gix-object brit-epr brit-graph brit-build-ref brit-cli; do
  echo "=== $c ==="; cargo publish -p "$c" --registry elohim --dry-run --no-verify 2>&1 | tail -3
done
```
Expected: each reports `Packaging`/`Uploading (dry run)` with no manifest errors. Investigate any "missing version" or "dependency ... does not specify a version" error before proceeding — it means a Task 2/3 rewrite was missed.

- [ ] **Step 2: (GATED) Run the real publish**

Only after operator go-ahead:
```bash
NEXUS_NPM_TOKEN="$NPM_TOKEN" bash scripts/ci/cargo-publish-brit.sh
```
Expected: each crate prints `Published <crate> <version>` (or `already on the index` on a re-run). If it exits non-zero on a real error (not a 409), stop and diagnose — earlier crates are already live (immutable), so fix-forward with the idempotent re-run.

- [ ] **Step 3: Verify the sparse index serves the new crates**

```bash
for p in "gi/x-/gix-object" "br/it/brit-epr" "br/it/brit-cli"; do
  echo "=== $p ==="; curl -fsSL "https://nexus.ethosengine.com/repository/cargo-internal/$p" | tail -1
done
```
Expected: JSON lines including `"vers":"0.58.0"` for gix-object and `"vers":"0.1.0"` for brit-epr / brit-cli.

- [ ] **Step 4: Standalone-consumer proof (the real acceptance test)**

In a scratch dir OUTSIDE the brit checkout (no sibling fork present):
```bash
cd "$(mktemp -d)"
cargo new brit-consume-probe && cd brit-consume-probe
mkdir -p .cargo
cat > .cargo/config.toml <<'EOF'
[registries.elohim]
index = "sparse+https://nexus.ethosengine.com/repository/cargo-internal/"
[source.crates-io]
replace-with = "elohim-mirror"
[source.elohim-mirror]
registry = "sparse+https://nexus.ethosengine.com/repository/cargo/"
EOF
export CARGO_REGISTRIES_ELOHIM_TOKEN="Bearer $NPM_TOKEN"
export CARGO_REGISTRY_GLOBAL_CREDENTIAL_PROVIDERS=cargo:token
export RUSTFLAGS="" RUSTC_WRAPPER=""
cargo add brit-epr --registry elohim
cargo build 2>&1 | tail -5
```
Expected: `brit-epr` (and its forked gix-object closure) resolves purely from Nexus and the probe builds. This proves the fork coheres on the registry with no local paths. A resolution failure pointing at crates.io means an intra-fork dep is missing its `registry = "elohim"` — return to Task 2.

- [ ] **Step 5: No commit** (execution/verification only; nothing to commit).

---

### Task 7: Update docs and memory

**Files:**
- Modify: `/projects/.claude-config/projects/-projects-elohim/memory/project_brit_rakia_nexus_ci.md`
- Modify: `genesis/docs/integrations/brit-migration-roadmap.md` (note the new cargo-install option in Stage 1b) — path is in the elohim monorepo, outside the brit submodule.

- [ ] **Step 1: Update the memory file**

Append to `project_brit_rakia_nexus_ci.md` a paragraph recording: the brit-publish closure (15 crates = 4 brit-* @ 0.1.0 + 11 forked gix-* @ upstream versions), the keep-upstream-versions + hosted-namespace decision, the immutability/bump-on-re-divergence rule, and that `brit-verify` stays `publish=false`. Keep it to a few lines; link `[[project_brit_next_gen_epr_meta_foundation]]`.

- [ ] **Step 2: Note the ci-builder alternative in the migration roadmap**

In `genesis/docs/integrations/brit-migration-roadmap.md` Stage 1b, add a short note that the ci-builder image may now `cargo install brit-cli brit-build-ref --registry elohim` (with an `.cargo/config.toml` declaring `[registries.elohim]`) as an alternative to building the fork from a submodule.

- [ ] **Step 3: Commit (brit repo docs only; monorepo/memory edits are committed in their own trees per their conventions)**

The memory file and the monorepo roadmap live outside the brit submodule — do not `git add` them from the brit checkout. Commit the memory update via the memory tooling / monorepo as appropriate. No brit-repo commit in this task unless a brit-local doc changed.

---

## Self-Review

**Spec coverage:**
- §Scope (15-crate set, brit-verify excluded) → Global Constraints + Task 3 Step 5 closure check ✓
- §1 Versioning (brit→0.1.0, gix→upstream, immutability) → Task 1 + Task 4 script docstring ✓
- §2 Dependency rewrites (dual path+registry, brit-cli gix untouched, dev-deps untouched) → Tasks 2 & 3 + Global Constraints ✓
- §3 Publish pipeline (topo script, main-gated CI job, first-publish de-risk) → Tasks 4, 5, 6 ✓
- §4 Verification (closure re-check, package/dry-run, local regression, standalone proof, index curl) → Task 3 Step 5, Task 6 Steps 1/3/4 ✓
- §4 Rollback (immutable, yank+bump) → Task 4 docstring + Task 6 Step 2 note ✓
- §5 Docs/memory → Task 7 ✓

**Placeholder scan:** No TBD/TODO; every code/edit step shows exact before/after content and exact commands with expected output.

**Type/name consistency:** Crate names, versions, topo order, and the `registry = "elohim"` alias are identical across Global Constraints, the script's `CRATES` array, and the per-file edits. The closure check expects 17 names (15 + 2 rakia), matching the metadata computation used during design.
