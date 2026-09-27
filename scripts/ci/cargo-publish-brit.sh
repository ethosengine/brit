#!/usr/bin/env bash
# Fail-closed guardrail for the generated closure; bootstrap is still HOLD
# until the fork dependency tags and immutable versions are reconciled.
# Use --check for a no-upload rehearsal or --audit-index for read-only counts.
set -euo pipefail

if [ -z "${CARGO_REGISTRIES_ELOHIM_TOKEN:-}" ] && [ -n "${NEXUS_NPM_TOKEN:-}" ]; then
  export CARGO_REGISTRIES_ELOHIM_TOKEN="Bearer ${NEXUS_NPM_TOKEN}"
elif [ -z "${CARGO_REGISTRIES_ELOHIM_TOKEN:-}" ] && [ -n "${NPM_TOKEN:-}" ]; then
  export CARGO_REGISTRIES_ELOHIM_TOKEN="Bearer ${NPM_TOKEN}"
fi

mode="${1:---publish}"
case "$mode" in
  --check|--publish|--audit-index) ;;
  *) echo "usage: $0 [--check|--publish|--audit-index]" >&2; exit 2 ;;
esac

script_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
exec python3 "${script_dir}/publish_brit.py" "$mode"
