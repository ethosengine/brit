#!/usr/bin/env bash
# Exercise the actual build guard through Cargo/Clippy in a tiny isolated crate.
set -euo pipefail
: "${CARGO_TARGET_DIR:?set an external Cargo pool target}"
root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)
fixture=$(mktemp -d)
trap 'rm -rf -- "$fixture"' EXIT
mkdir -p "$fixture/src" "$fixture/build"
cp "$root/brit-cli/build/cpu_compatibility.rs" "$root/brit-cli/build/cpu_baseline.c" "$fixture/build/"
cat > "$fixture/Cargo.toml" <<'EOF'
[package]
name = "brit-cpu-baseline-probe"
version = "0.0.0"
edition = "2021"
[build-dependencies]
cc = "=1.11.1"
[workspace]
EOF
cat > "$fixture/build.rs" <<'EOF'
#[path = "build/cpu_compatibility.rs"]
mod cpu_compatibility;
fn main() -> Result<(), Box<dyn std::error::Error>> {
    cpu_compatibility::check()
}
EOF
cat > "$fixture/src/main.rs" <<'EOF'
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
unsafe fn accelerated() -> u32 { std::hint::black_box(42) }
fn main() {
    #[cfg(target_arch = "x86_64")]
    if std::is_x86_feature_detected!("avx2") {
        // SAFETY: runtime detection establishes this function's CPU requirement.
        assert_eq!(unsafe { accelerated() }, 42);
    }
}
EOF
cd "$fixture"
export CARGO_TARGET_DIR="$CARGO_TARGET_DIR/cpu-baseline-probe"
unset CARGO_ENCODED_RUSTFLAGS CFLAGS_x86_64_unknown_linux_gnu TARGET_CFLAGS
export RUSTFLAGS= CFLAGS= CXXFLAGS=
# No CFLAGS: cc (>= 1.11) pins -march=x86-64 for x86_64 Linux, so this passes
# even where the host compiler defaults above the floor (Ubuntu 26: x86-64-v3).
# On a runner whose compiler already defaults to plain x86-64 this case proves
# nothing; the cc-v3 wrapper cases below are the real proof on every runner.
# The fixture has no lock file, so cc is pinned exactly in its manifest.
cargo clippy --quiet -- -D warnings
# Positive controls below also declare the published v2 target explicitly.
if [[ $(rustc --print cfg) == *'target_arch="x86_64"'* && $(rustc --print cfg) == *'target_os="linux"'* ]]; then
  export CFLAGS=-march=x86-64-v2
fi
cargo clippy --quiet -- -D warnings
if [[ $(rustc --print cfg) != *'target_arch="x86_64"'* ]]; then
  echo 'cpu-baseline: non-x86 target compiles; x86 refusal controls require an x86 host' >&2
  exit 2
fi
RUSTFLAGS='-C target-cpu=x86-64-v2' cargo clippy --quiet -- -D warnings
reject() {
  if "$@" > "$fixture/refusal" 2>&1; then
    cat "$fixture/refusal" >&2
    echo "CPU baseline unexpectedly accepted: $*" >&2; exit 1
  fi
  grep -F 'Brit CPU baseline' "$fixture/refusal" >/dev/null || { cat "$fixture/refusal" >&2; exit 1; }
}
reject env RUSTFLAGS='-C target-feature=+avx2' cargo clippy --quiet
reject env RUSTFLAGS='-C target-feature=+bmi2' cargo check --quiet
reject env RUSTFLAGS='-C target-cpu=x86-64-v3' cargo build --quiet
if [[ $(rustc --print cfg) == *'target_os="linux"'* ]]; then
  reject env CFLAGS=-mavx2 cargo clippy --quiet
  reject env CFLAGS=-mbmi2 cargo check --quiet
  # A compiler whose default exceeds the floor: cc's own -march=x86-64 follows
  # the compiler's and wins, so the floor holds without CFLAGS ...
  compiler=$(command -v cc)
  printf '#!/usr/bin/env bash\nexec "%s" -march=x86-64-v3 "$@"\n' "$compiler" > "$fixture/cc-v3"
  chmod +x "$fixture/cc-v3"
  CC="$fixture/cc-v3" CFLAGS= cargo check --quiet
  CC="$fixture/cc-v3" CFLAGS=-march=x86-64-v2 cargo clippy --quiet -- -D warnings
  # ... but not when cc's defaults are switched off, nor when rustc names a
  # target-cpu (cc then passes no -march, so CFLAGS must carry it).
  reject env CC="$fixture/cc-v3" CFLAGS= CRATE_CC_NO_DEFAULTS=1 cargo check --quiet
  reject env CC="$fixture/cc-v3" CFLAGS= RUSTFLAGS='-C target-cpu=x86-64-v2' cargo check --quiet
fi
echo 'cpu-baseline=passed (Clippy/check/build refusal; guarded acceleration permitted)'
