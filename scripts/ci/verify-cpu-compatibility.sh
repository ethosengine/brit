#!/usr/bin/env bash
# @concern:brit-native-daily-driver
# Given a released executable and a supported CPU (native or emulated), fetching
# real delta packs and reading their objects must preserve Git's exact bytes.
# No network, shared repository, user config, or installed binary is changed.
set -euo pipefail
[[ $# -ge 1 && $1 = /* && -x $1 ]] || {
  echo 'usage: verify-cpu-compatibility.sh <absolute-brit> [runner args...]' >&2
  exit 2
}
binary=$1
shift
runner=("$@")
if [[ -n ${BRIT_CPU_FIXTURE_DIR:-} ]]; then
  fixture=$BRIT_CPU_FIXTURE_DIR
  [[ $fixture = /* ]] || { echo 'fixture path must be absolute' >&2; exit 2; }
  mkdir -- "$fixture"
else
  fixture=$(mktemp -d)
fi
server_pid=
cleanup() {
  if [[ -n $server_pid ]]; then
    kill "$server_pid" 2>/dev/null || true
    wait "$server_pid" 2>/dev/null || true
  fi
  if [[ -z ${BRIT_CPU_FIXTURE_DIR:-} ]]; then rm -rf -- "$fixture"; fi
}
trap cleanup EXIT
for name in ${!GIT_@}; do unset "$name"; done
unset EPR_FEEDBACK_ROOT EPR_FEEDBACK_INVOCATION
export GIT_CONFIG_NOSYSTEM=1 GIT_CONFIG_GLOBAL=/dev/null GIT_TERMINAL_PROMPT=0
export GIT_AUTHOR_NAME='Brit CPU Fixture' GIT_AUTHOR_EMAIL=fixture@example.invalid
export GIT_COMMITTER_NAME="$GIT_AUTHOR_NAME" GIT_COMMITTER_EMAIL="$GIT_AUTHOR_EMAIL"
export GIT_AUTHOR_DATE='2026-01-01T00:00:00Z'
export GIT_COMMITTER_DATE="$GIT_AUTHOR_DATE"
ulimit -c 0
brit_run() { "${runner[@]}" "$binary" "$@"; }
git init -q --initial-branch=main "$fixture/upstream"
cd "$fixture/upstream"
# Larger than the SIMD/chunk boundaries, compressible, and similar enough across
# revisions to force delta reconstruction rather than a help-only startup smoke.
awk 'BEGIN { for (i=0; i<8192; i++) printf "%08d: the same object bytes on every supported processor\n", i }' > payload.txt
for revision in 1 2 3 4 5 6; do
  printf 'revision %s\n' "$revision" >> payload.txt
  git add payload.txt
  git -c commit.gpgsign=false commit -qm "revision $revision"
done
git repack -adq
git verify-pack -v .git/objects/pack/*.idx > "$fixture/pack.txt"
awk 'NF == 7 { delta++ } END { exit !(delta > 0) }' "$fixture/pack.txt"
expected=$(git rev-parse HEAD)
git init -q --initial-branch=main "$fixture/client"
cd "$fixture/client"
git remote add origin "$fixture/upstream"
brit_run --version
brit_run fetch --remote origin
test "$(git rev-parse refs/remotes/origin/main)" = "$expected"
brit_run cat refs/remotes/origin/main:payload.txt > "$fixture/actual"
cmp "$fixture/upstream/payload.txt" "$fixture/actual"
# The second transfer exercises negotiation with already retained objects.
cd "$fixture/upstream"
printf 'incremental transfer\n' >> payload.txt
git add payload.txt
git -c commit.gpgsign=false commit -qm incremental
git repack -adq
expected=$(git rev-parse HEAD)
cd "$fixture/client"
brit_run fetch --remote origin
test "$(git rev-parse refs/remotes/origin/main)" = "$expected"
brit_run cat refs/remotes/origin/main:payload.txt > "$fixture/actual"
cmp "$fixture/upstream/payload.txt" "$fixture/actual"
git fsck --strict --no-dangling
# The observed Shem SIGILL was in TLS initialization, before object transfer.
# A deliberately untrusted local certificate must reach normal certificate
# rejection, never a signal or a generic transport/configuration failure.
openssl req -x509 -newkey rsa:2048 -nodes -days 1 -subj /CN=localhost \
  -addext subjectAltName=DNS:localhost \
  -keyout "$fixture/key.pem" -out "$fixture/cert.pem" > "$fixture/certificate.log" 2>&1
openssl s_server -accept 127.0.0.1:0 -cert "$fixture/cert.pem" -key "$fixture/key.pem" \
  -www -naccept 1 < /dev/null > "$fixture/tls-server.log" 2>&1 &
server_pid=$!
port=
for _ in {1..50}; do
  port=$(sed -n 's/^ACCEPT .*:\([0-9][0-9]*\).*$/\1/p' "$fixture/tls-server.log")
  [[ -z $port ]] || break
  kill -0 "$server_pid" 2>/dev/null || break
  sleep 0.1
done
[[ $port =~ ^[0-9]+$ ]] || { cat "$fixture/tls-server.log" >&2; exit 1; }
tls_status=0
# Do not let inherited proxies turn this loopback fixture into a network test.
unset http_proxy https_proxy all_proxy HTTP_PROXY HTTPS_PROXY ALL_PROXY
export NO_PROXY=localhost,127.0.0.1 no_proxy=localhost,127.0.0.1
brit_run fetch --dry-run --remote "https://localhost:$port/repo" \
  '+refs/heads/main:refs/remotes/tls/main' > "$fixture/tls-client.log" 2>&1 || tls_status=$?
if [[ $tls_status != 1 ]] || ! grep -Ei 'invalid peer certificate|UnknownIssuer|self[- ]signed certificate|certificate verify failed' "$fixture/tls-client.log" > /dev/null; then
  cat "$fixture/tls-client.log" >&2
  echo "TLS compatibility failed: expected certificate refusal (1), got $tls_status" >&2
  exit 1
fi
printf 'cpu-compatibility=passed (packed fetch, incremental fetch, exact bytes, TLS certificate refusal)\n'
