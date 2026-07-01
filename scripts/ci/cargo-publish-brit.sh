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
  awk '/^\[package\]/{p=1} p&&/^version[[:space:]]*=/{gsub(/.*"([^"]+)".*/,"\\1"); print; exit}' \
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
