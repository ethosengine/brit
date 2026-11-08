#!/usr/bin/env bash
# Qualify the exact Linux delivery bytes, including runtime-dispatched paths.
set -euo pipefail
[[ $# -ge 1 ]] || { echo 'usage: verify-cpu-matrix.sh --preflight | <absolute-brit> [absolute-helpers...]' >&2; exit 2; }
root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)
qemu=${QEMU_X86_64:-qemu-x86_64}
command -v "$qemu" >/dev/null || { echo 'CPU compatibility unmeasured: install qemu-user (qemu-x86_64)' >&2; exit 2; }
fixture=$(mktemp -d)
trap 'rm -rf -- "$fixture"' EXIT
cd "$fixture"
ulimit -c 0
# A restricted CPU must also run the loader/libc used by this measurement.
# Otherwise a SIGILL says nothing yet about the candidate's own instructions.
cat > "$fixture/baseline.c" <<'EOF'
int main(void) { return 0; }
EOF
cc -march=x86-64-v2 "$fixture/baseline.c" -o "$fixture/baseline"
timeout 10 "$qemu" -cpu Nehalem "$fixture/baseline" || {
  echo 'CPU qualification unmeasured: the measurement runtime cannot start a v2 control' >&2
  exit 2
}
cat > "$fixture/avx2.c" <<'EOF'
int main(void) {
    __asm__ volatile("vpxor %%ymm0, %%ymm0, %%ymm0" ::: "ymm0");
    return 0;
}
EOF
cc -march=x86-64-v2 -mavx2 "$fixture/avx2.c" -o "$fixture/avx2"
status=0
"$qemu" -cpu Nehalem "$fixture/avx2" > "$fixture/control.log" 2>&1 || status=$?
[[ $status == 132 ]] || { echo "CPU restriction control failed: AVX2 exited $status, expected SIGILL (132)" >&2; exit 1; }
"$qemu" -cpu Haswell "$fixture/avx2"
echo 'CPU restriction control passed: baseline rejects AVX2; newer CPU executes it'
if [[ $1 == --preflight ]]; then
  echo 'cpu-runtime=qualified (v2 positive and AVX2 negative controls)'
  exit 0
fi
primary=$1
for cpu in native Nehalem Haswell; do
  runner=()
  if [[ $cpu != native ]]; then runner=("$qemu" -cpu "$cpu"); fi
  printf 'cpu-model=%s\n' "$cpu"
  timeout 120 bash "$root/scripts/ci/verify-cpu-compatibility.sh" "$primary" "${runner[@]}"
  timeout 45 bash "$root/scripts/ci/verify-native-https.sh" "$primary" "${runner[@]}"
  for binary in "$@"; do
    [[ $binary = /* && -x $binary ]] || { echo "invalid delivery executable: $binary" >&2; exit 2; }
    printf 'startup-probe=%s\n' "${binary##*/}"
    if [[ ${binary##*/} == brit-verify ]]; then
      # This helper documents a no-argument usage error, not a --help flag.
      # Demand that exact response; a signal, timeout or repo error is not a pass.
      status=0
      timeout 30 "${runner[@]}" "$binary" > "$fixture/helper.log" 2>&1 || status=$?
      [[ $status == 2 ]] && grep -F 'Usage: brit-verify <commit-rev>' "$fixture/helper.log" >/dev/null || {
        cat "$fixture/helper.log" >&2
        echo "brit-verify startup probe failed: exit $status" >&2
        exit 1
      }
    else
      timeout 30 "${runner[@]}" "$binary" --help > /dev/null
    fi
  done
done
sha256sum -- "$@"
echo 'cpu-matrix=passed (exact bytes; native, x86-64-v2/Nehalem, AVX2/Haswell)'
