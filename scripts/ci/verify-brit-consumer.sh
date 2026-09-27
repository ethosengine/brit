#!/usr/bin/env bash
# Exercise a published Brit release from a fresh, source-free Cargo consumer.
set -euo pipefail
if [ "$#" -ne 1 ]; then
  echo "usage: $0 <exact-brit-cli-version>" >&2
  exit 2
fi
script_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
exec python3 "$script_dir/verify_brit_consumer.py" "$1"
