#!/usr/bin/env bash
# Build the held addressed checker source outside this checkout; never fetch a head.
set -euo pipefail
root=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)
fail() { printf 'rust-style install: %s\n' "$*" >&2; exit 2; }
verify=0
if [[ ${1:-} == --verify ]]; then verify=1; shift; fi
[[ $# == 1 && $1 == /* && $1 != / ]] || fail 'usage: install-rust-style.sh [--verify] <absolute-private-prefix>'
prefix=$1
[[ $prefix != */../* && $prefix != */.. && $prefix != */./* && $prefix != */. && $prefix != *//* && $prefix != */ && $prefix != *$'\n'* ]] || fail 'prefix requires a normal absolute path'
prefix_parent=$prefix
while [[ $prefix_parent != / && -n $prefix_parent ]]; do
    [[ ! -L $prefix_parent ]] || fail 'installation prefix traverses a symlink'
    prefix_parent=${prefix_parent%/*}
done
for tool in jq sha256sum base32 xxd tar mktemp; do command -v "$tool" >/dev/null || fail "required tool unavailable: $tool"; done
for path in "$root/.epr-meta" "$root/.epr-meta/rust-style" "$root/.epr-meta/rust-style/objects"; do
    [[ ! -L $path ]] || fail 'source holding traverses a symlink'
done
config="$root/.epr-meta/rust-style/source.json"
[[ -f $config && ! -L $config ]] || fail 'held source declaration unavailable or symlinked'
jq -e 'keys == ["archive","cid","format","sha256","toolchain"] and .format == "rust-style-source-holding@1" and .toolchain == "1.100.0"' "$config" >/dev/null || fail 'unsupported source declaration'
source_sha=$(jq -er '.sha256 | select(type == "string")' "$config")
source_cid=$(jq -er '.cid | select(type == "string")' "$config")
archive_relative=$(jq -er '.archive | select(type == "string")' "$config")
[[ $source_sha =~ ^[0-9a-f]{64}$ && $source_cid =~ ^bafkrei[a-z2-7]+$ ]] || fail 'malformed source identity'
computed_cid="b$(printf '01551220%s' "$source_sha" | xxd -r -p | base32 | tr -d '=\n' | tr 'A-Z' 'a-z')"
[[ $computed_cid == "$source_cid" ]] || fail 'source CID and digest disagree'
[[ $archive_relative == ".epr-meta/rust-style/objects/$source_cid.tar.gz" ]] || fail 'source archive is not its addressed local holding'
archive="$root/$archive_relative"
[[ ! -L $archive ]] || fail 'source holding traverses a symlink'
[[ -f $archive ]] || fail 'addressed source archive is unavailable'
[[ $(wc -c < "$archive") -le 16777216 ]] || fail 'source archive exceeds size bound'
actual=$(sha256sum "$archive"); actual=${actual%% *}
[[ $actual == "$source_sha" ]] || fail 'source archive digest mismatch'
binary="$prefix/bin/epr-rust-style"
receipt="$prefix/install.json"
verify_installed() {
    [[ ! -L $prefix && ! -L $prefix/bin && -f $receipt && ! -L $receipt && -x $binary && ! -L $binary ]] || return 1
    local expected actual_binary
    expected=$(jq -er --arg source "$source_sha" --arg cid "$source_cid" 'select(.format == "rust-style-installation@1" and .sourceSha256 == $source and .sourceCid == $cid and .toolchain == "1.100.0") | .binarySha256 | select(type == "string")' "$receipt") || return 1
    [[ $expected =~ ^[0-9a-f]{64}$ ]] || return 1
    actual_binary=$(sha256sum "$binary"); actual_binary=${actual_binary%% *}
    [[ $expected == "$actual_binary" ]]
}
if (( verify )); then
    verify_installed || fail 'missing, stale or modified checker installation; run install-rust-style.sh with this prefix'
    printf '%s\n' "$binary"
    exit 0
fi
if verify_installed; then printf '%s\n' "$binary"; exit 0; fi
[[ ${CARGO_TARGET_DIR:-} == /* ]] || fail 'CARGO_TARGET_DIR must name an absolute native build slot'
[[ ! -L $prefix && ! -L $prefix/bin ]] || fail 'installation prefix must not be symlinked'
scratch_parent=$(cd /tmp && pwd -P)
ancestor=$scratch_parent
while :; do
    [[ ! -e $ancestor/.git && ! -e $ancestor/.cargo/config && ! -e $ancestor/.cargo/config.toml ]] || fail 'scratch ancestry contains repository or Cargo configuration'
    [[ $ancestor != / ]] || break
    ancestor=${ancestor%/*}; [[ -n $ancestor ]] || ancestor=/
done
mkdir -p "$prefix/bin"
mkdir "$prefix/.install-lock" 2>/dev/null || fail 'another installation holds this prefix'
scratch=$(mktemp -d "$scratch_parent/brit-rust-style.XXXXXXXX")
cleanup() { rm -rf "$scratch"; rmdir "$prefix/.install-lock"; }
trap cleanup EXIT
# Only the producer's regular-file, normalized namespace is admissible. Do this
# before extraction; GNU tar then receives no links or traversal components.
tar -tzf "$archive" > "$scratch/names"
while IFS= read -r entry; do
    [[ $entry =~ ^rust-style-source/[A-Za-z0-9._/-]+$ && $entry != */../* && $entry != */./* && $entry != */.. && $entry != */. && $entry != *//* ]] || fail 'unsafe archive path'
done < "$scratch/names"
[[ -s $scratch/names ]] || fail 'empty source archive'
[[ $(sort -u "$scratch/names" | wc -l) == $(wc -l < "$scratch/names") ]] || fail 'duplicate archive path'
tar -tvzf "$archive" > "$scratch/types"
while IFS= read -r entry; do [[ $entry == -* ]] || fail 'source archive contains a link or nonregular entry'; done < "$scratch/types"
awk '{ total += $3 } END { exit(total > 67108864) }' "$scratch/types" || fail 'expanded source exceeds size bound'
tar -xzf "$archive" --no-same-owner --no-same-permissions -C "$scratch"
manifest="$scratch/rust-style-source/genesis/orchestrator/rust-style/Cargo.toml"
[[ -f $manifest ]] || fail 'checker manifest absent from held source'
build_target="$CARGO_TARGET_DIR/rust-style-source-$source_sha"
printf 'rust-style install: building verified source %s\n' "$source_cid" >&2
# Run from the isolated extraction so neither parent nor consumer Cargo configs
# can silently add source paths. First-party relative paths stay within the holding.
(cd "$scratch/rust-style-source" && env CARGO_TARGET_DIR="$build_target" RUSTUP_TOOLCHAIN=1.100.0 RUSTFLAGS= RUSTC_WRAPPER= CARGO_BUILD_JOBS=1 \
    cargo build --locked --manifest-path "$manifest" --bin epr-rust-style) >&2
built="$build_target/debug/epr-rust-style"
[[ -f $built && ! -L $built ]] || fail 'Cargo produced no regular checker binary'
stage=$(mktemp "$prefix/bin/.epr-rust-style.XXXXXX")
cp "$built" "$stage"
chmod 755 "$stage"
binary_sha=$(sha256sum "$stage"); binary_sha=${binary_sha%% *}
receipt_stage=$(mktemp "$prefix/.install.XXXXXX")
jq -n --arg cid "$source_cid" --arg source "$source_sha" --arg binary "$binary_sha" \
    '{format:"rust-style-installation@1",sourceCid:$cid,sourceSha256:$source,binarySha256:$binary,toolchain:"1.100.0"}' > "$receipt_stage"
mv -f "$stage" "$binary"
mv -f "$receipt_stage" "$receipt"
verify_installed || fail 'installed binary did not reproduce its receipt'
printf '%s\n' "$binary"
