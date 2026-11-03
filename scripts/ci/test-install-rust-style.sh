#!/usr/bin/env bash
# Installer boundary tests use a fake Cargo producer, never the shared checkout.
set -euo pipefail
source_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)
scratch=$(mktemp -d)
trap 'rm -rf "$scratch"' EXIT
root="$scratch/repository"
mkdir -p "$root/scripts/ci" "$root/.epr-meta/rust-style/objects" "$scratch/fake-bin" "$scratch/payload/rust-style-source/genesis/orchestrator/rust-style"
cp "$source_root/scripts/ci/install-rust-style.sh" "$root/scripts/ci/install-rust-style.sh"
printf '[package]\nname="fixture"\n' > "$scratch/payload/rust-style-source/genesis/orchestrator/rust-style/Cargo.toml"
cat > "$scratch/fake-bin/cargo" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
[[ $1 == build && $2 == --locked && $PWD == */rust-style-source ]]
printf '%s\n' "$PWD" >> "$CARGO_TEST_LOG"
mkdir -p "$CARGO_TARGET_DIR/debug"
printf '#!/usr/bin/env bash\nprintf "fixture checker\\n"\n' > "$CARGO_TARGET_DIR/debug/epr-rust-style"
chmod +x "$CARGO_TARGET_DIR/debug/epr-rust-style"
EOF
chmod +x "$scratch/fake-bin/cargo"
export PATH="$scratch/fake-bin:$PATH" CARGO_TARGET_DIR="$scratch/target" CARGO_TEST_LOG="$scratch/cargo.log"
hold() {
    local file=$1 digest address
    digest=$(sha256sum "$file"); digest=${digest%% *}
    address="b$(printf '01551220%s' "$digest" | xxd -r -p | base32 | tr -d '=\n' | tr 'A-Z' 'a-z')"
    cp "$file" "$root/.epr-meta/rust-style/objects/$address.tar.gz"
    jq -n --arg sha "$digest" --arg cid "$address" '{format:"rust-style-source-holding@1",sha256:$sha,cid:$cid,archive:(".epr-meta/rust-style/objects/"+$cid+".tar.gz"),toolchain:"1.96.1"}' > "$root/.epr-meta/rust-style/source.json"
}
refuses() {
    if "$@" > "$scratch/out" 2> "$scratch/err"; then echo 'expected refusal' >&2; exit 1; fi
}
tar -czf "$scratch/valid.tar.gz" -C "$scratch/payload" rust-style-source/genesis/orchestrator/rust-style/Cargo.toml
hold "$scratch/valid.tar.gz"
installer="$root/scripts/ci/install-rust-style.sh"
refuses "$installer" "$scratch/.."
refuses "$installer" "$scratch/../"
ln -s "$scratch" "$scratch/prefix-link"
refuses "$installer" "$scratch/prefix-link/install"
[[ ! -e $scratch/cargo.log ]]
refuses "$installer" --verify "$scratch/install"
# A consumer-controlled TMPDIR must not put the extraction beneath its Cargo config.
installed=$(TMPDIR="$root" "$installer" "$scratch/install")
[[ $installed == "$scratch/install/bin/epr-rust-style" && $($installed) == 'fixture checker' ]]
[[ $(cat "$scratch/cargo.log") == /tmp/brit-rust-style.*/rust-style-source ]]
[[ $("$installer" --verify "$scratch/install") == "$installed" ]]
[[ $(wc -l < "$scratch/cargo.log") == 1 ]]
"$installer" "$scratch/install" >/dev/null
[[ $(wc -l < "$scratch/cargo.log") == 1 ]]
printf 'damage' >> "$installed"
refuses "$installer" --verify "$scratch/install"
archive=$(jq -r .archive "$root/.epr-meta/rust-style/source.json")
printf 'damage' >> "$root/$archive"
refuses "$installer" "$scratch/install"
[[ $(wc -l < "$scratch/cargo.log") == 1 ]]
ln -s /etc/passwd "$scratch/payload/rust-style-source/link"
tar -czf "$scratch/link.tar.gz" -C "$scratch/payload" rust-style-source/link
hold "$scratch/link.tar.gz"
refuses "$installer" "$scratch/link-install"
[[ $(wc -l < "$scratch/cargo.log") == 1 ]]
tar -czf "$scratch/traversal.tar.gz" --transform='s|rust-style-source/|rust-style-source/../|' -C "$scratch/payload" rust-style-source/genesis/orchestrator/rust-style/Cargo.toml
hold "$scratch/traversal.tar.gz"
refuses "$installer" "$scratch/traversal-install"
[[ $(wc -l < "$scratch/cargo.log") == 1 ]]
mv "$root/.epr-meta" "$root/.metadata"
ln -s .metadata "$root/.epr-meta"
refuses "$installer" --verify "$scratch/install"
printf 'rust-style installer boundaries: PASS\n'
