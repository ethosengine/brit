#!/usr/bin/env bash
# @concern:brit-native-daily-driver
# Exercise TLS initialization before admitting an executable on this device.
# This deliberately refuses an untrusted loopback certificate; it proves neither
# successful HTTPS transfer nor release authority. No repository/network service
# outside this disposable fixture is touched. Optional runner is for CI emulation.
set -euo pipefail
[[ $# -ge 1 && $1 = /* && -x $1 ]] || {
  echo 'usage: verify-native-https.sh <absolute-brit> [runner args...]' >&2; exit 2;
}
binary=$1
shift
for dependency in openssl timeout; do
  command -v "$dependency" >/dev/null || { echo "HTTPS compatibility unmeasured: missing $dependency" >&2; exit 2; }
done
fixture=$(mktemp -d)
server_pid=
cleanup() {
  if [[ -n $server_pid ]]; then
    kill "$server_pid" 2>/dev/null || true
    wait "$server_pid" 2>/dev/null || true
  fi
  rm -rf -- "$fixture"
}
trap cleanup EXIT
trap 'exit 130' INT
trap 'exit 143' TERM
for name in ${!GIT_@}; do unset "$name"; done
unset EPR_FEEDBACK_ROOT EPR_FEEDBACK_INVOCATION
unset http_proxy https_proxy all_proxy HTTP_PROXY HTTPS_PROXY ALL_PROXY
export NO_PROXY=localhost,127.0.0.1 no_proxy=localhost,127.0.0.1
export GIT_CONFIG_NOSYSTEM=1 GIT_CONFIG_GLOBAL=/dev/null GIT_TERMINAL_PROMPT=0
ulimit -c 0
cd "$fixture"
openssl req -x509 -newkey rsa:2048 -nodes -days 1 -subj /CN=localhost \
  -addext subjectAltName=DNS:localhost -keyout key.pem -out cert.pem > certificate.log 2>&1
openssl s_server -accept 127.0.0.1:0 -cert cert.pem -key key.pem \
  -www -naccept 1 < /dev/null > server.log 2>&1 &
server_pid=$!
port=
for _ in {1..50}; do
  port=$(sed -n 's/^ACCEPT .*:\([0-9][0-9]*\).*$/\1/p' server.log)
  [[ -z $port ]] || break
  kill -0 "$server_pid" 2>/dev/null || break
  sleep 0.1
done
[[ $port =~ ^[0-9]+$ ]] || { cat server.log >&2; exit 1; }
status=0
timeout 30 "$@" "$binary" clone "https://localhost:$port/repo" "$fixture/client" > client.log 2>&1 || status=$?
if [[ $status != 1 ]] || ! grep -Ei 'invalid peer certificate|UnknownIssuer|self[- ]signed certificate|certificate verify failed' client.log >/dev/null; then
  cat client.log >&2
  echo "HTTPS compatibility refused: expected certificate refusal (1), got $status" >&2
  exit 1
fi
echo 'native-https=passed (TLS initialization and certificate refusal)'
