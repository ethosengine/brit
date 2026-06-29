#!/usr/bin/env bash
# Advisory: on edit of a brit *.md, report any non-ok cite verdicts. Never blocks.
set -euo pipefail
# brit is an all-native workspace; clear the container's WASM RUSTFLAGS leak so the build works.
export RUSTFLAGS=""
file="${CLAUDE_FILE_PATH:-}"
case "$file" in *.md) ;; *) exit 0 ;; esac
dir="$(dirname "$file")"
out="$(cd "${CLAUDE_PROJECT_DIR:-.}" && cargo run -q -p brit-build-ref -- --repo . meta status --dir "$dir" 2>/dev/null | grep -Ev '^ok ' || true)"
[ -n "$out" ] && printf 'epr-meta drift/debt:\n%s\n' "$out"
exit 0
