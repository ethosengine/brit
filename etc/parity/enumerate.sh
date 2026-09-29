#!/usr/bin/env bash
# Reproduce the pinned Git census, or inspect a compiled Brit against it.
set -euo pipefail
repo_root="$(cd "$(dirname "$0")/../.." && pwd)"
if [[ "${1:-}" == "--compare" ]]; then
  shift
  exec python3 "$repo_root/etc/parity/compare.py" "$@"
fi
if [[ "${1:-}" == "--walk" ]]; then
  shift
  exec python3 "$repo_root/etc/parity/atoms.py" "$@"
fi
exec python3 "$repo_root/etc/parity/census.py" "$@"
