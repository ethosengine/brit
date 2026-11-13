#!/usr/bin/env bash
# Opt-in Linux qualification, never a runtime sandbox or an automatic Git fallback.
# Usage: BRIT_MEMORY_CGROUP_PARENT=/delegated/cgroup qualify-memory.sh EVIDENCE EXECUTABLE [ARG...]
set -euo pipefail
[[ $# -ge 2 ]] || { echo 'usage: qualify-memory.sh EVIDENCE EXECUTABLE [ARG...]' >&2; exit 2; }
evidence=$1; shift
mkdir -- "$evidence"
evidence=$(cd -- "$evidence" && pwd -P)
parent=${BRIT_MEMORY_CGROUP_PARENT:?a delegated cgroup parent is required}
[[ -f "$parent/cgroup.controllers" ]] || { echo 'cgroup v2 delegation is required' >&2; exit 2; }
group="$parent/brit-qualification-$$"
mkdir -- "$group"
cleanup() {
  # This group was created exclusively for this invocation. Never touch a foreign group.
  if [[ -f "$group/cgroup.kill" ]]; then echo 1 > "$group/cgroup.kill"; fi
  rmdir -- "$group" || true
}
trap cleanup EXIT
printf '1073741824\n' > "$group/memory.max"
printf '0\n' > "$group/memory.swap.max"
printf '1\n' > "$group/memory.oom.group"
[[ $(cat "$group/memory.max") == 1073741824 && $(cat "$group/memory.swap.max") == 0 ]]
executable=$(realpath -- "$1")
sha=$(sha256sum -- "$executable"); sha=${sha%% *}
brit_executable=$(realpath -- "${BRIT_BIN:-$executable}")
brit_sha=$(sha256sum -- "$brit_executable"); brit_sha=${brit_sha%% *}
printf '%s\0' "$@" > "$evidence/argv.nul"
rustc -vV > "$evidence/toolchain.txt"
cp -- "$group/memory.events" "$evidence/events-before.txt"
start=$(date +%s%N)
set +e
# Enter before exec: filters, transports, hooks and other descendants inherit the same ceiling.
bash -c 'set -e; echo "$$" > "$1/cgroup.procs"; shift; exec "$@"' bash "$group" "$@" \
  > "$evidence/stdout" 2> "$evidence/stderr"
status=$?
set -e
end=$(date +%s%N)
cp -- "$group/memory.events" "$evidence/events-after.txt"
peak=$(cat "$group/memory.peak")
oom=$(awk '$1 == "oom_kill" { print $2 }' "$group/memory.events")
survivors=$(wc -l < "$group/cgroup.procs")
runner_after=$(sha256sum -- "$executable"); runner_after=${runner_after%% *}
brit_after=$(sha256sum -- "$brit_executable"); brit_after=${brit_after%% *}
identity_changed=false
if [[ "$runner_after" != "$sha" || "$brit_after" != "$brit_sha" ]]; then identity_changed=true; fi
outcome=$status
if [[ "$identity_changed" == true ]]; then outcome=1; fi
if (( oom != 0 || survivors != 0 )); then outcome=1; fi
jq -n --arg executable "$executable" --arg sha256 "$sha" \
  --arg britExecutable "$brit_executable" --arg britSha256 "$brit_sha" \
  --arg source "${BRIT_MEMORY_SOURCE:-uncommitted}" --argjson exit "$status" \
  --arg fixtureHash "${GIX_TEST_FIXTURE_HASH:-default}" --argjson identityChanged "$identity_changed" \
  --argjson peak "$peak" --argjson oom "$oom" --argjson survivors "$survivors" \
  --argjson elapsed "$(( (end-start)/1000000 ))" --argjson outcome "$outcome" \
  '{executable:$executable,sha256:$sha256,britExecutable:$britExecutable,britSha256:$britSha256,
    source:$source,fixtureHash:$fixtureHash,identityChanged:$identityChanged,memoryMax:1073741824,swapMax:0,
    memoryPeak:$peak,oomKills:$oom,remainingProcesses:$survivors,elapsedMs:$elapsed,exit:$exit,
    scratchLimit:1073741824,scratchPeak:null,qualificationComplete:false,outcome:$outcome}' > "$evidence/result.json"
cat "$evidence/result.json"
exit "$outcome"
