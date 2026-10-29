#!/usr/bin/env bash
# Daily-driver probe ladder for the brit-native-daily-driver habit.
#
#   scripts/daily-driver-probe.sh [--json] [--repo PATH]
#
# BRIT_BIN / BRIT_BUILD_REF_BIN select the binaries (default: brit / brit-build-ref on PATH).
# To probe the installed launcher, leave them UNSET: the launcher refuses ("launcher-alias") when
# BRIT_BIN names its own alias.
# The probed repo is never written to: R1-R3, R6 only read it; R4/R5 work in a
# `git clone --shared --no-checkout` under a mktemp dir; R7 uses a fresh scratch repo.
#
# Every rung prints exactly one line:  ok <rung> <detail> | REFUSED <rung> <reason>
# | unmeasured <rung> <reason>. A rung that prints none is a script bug and exits 2:
# a skip is legitimate, a silent skip is not.
# Exit 0 only if every rung is ok or unmeasured-with-reason; exit 1 on any REFUSED.
set -u

json=0
repo=/projects/elohim
while [ $# -gt 0 ]; do
  case "$1" in
    --json) json=1 ;;
    --repo) shift; repo="${1:-}" ;;
    -h | --help) sed -n '2,12p' "$0"; exit 0 ;;
    *) echo "usage: $0 [--json] [--repo PATH]" >&2; exit 64 ;;
  esac
  shift
done

BRIT="${BRIT_BIN:-brit}"
BBR="${BRIT_BUILD_REF_BIN:-brit-build-ref}"
RUNGS="R1 R2 R3 R4 R5 R6 R7"

scratch_root=$(mktemp -d "${TMPDIR:-/var/tmp}/daily-driver-probe.XXXXXX") || { echo "cannot create scratch dir" >&2; exit 2; }
trap 'rm -rf "$scratch_root"' EXIT

results=""    # tab-separated: state, rung, detail, one per line
refused=0
nl='
'

emit() { # state rung detail
  local detail
  detail=$(printf '%s' "$3" | tr '\n\t' '  ' | cut -c1-300)
  results="${results}${1}	${2}	${detail}${nl}"
  [ "$1" = REFUSED ] && refused=1
  [ "$json" = 1 ] || printf '%s %s %s\n' "$1" "$2" "$detail"
}
emitted() { printf '%s' "$results" | cut -f2 | grep -qx "$1"; }

have() { command -v "$1" > /dev/null 2>&1; }
# One-line reason from a failed command: its first Error line plus the deepest cause, else its last line.
tail1() {
  local first cause clean
  clean=$(printf '%s\n' "$1" | sed -e 's/\x1b\[[0-9;]*[A-Za-z]//g' -e 's/\r/\n/g')
  set -- "$clean"
  first=$(printf '%s\n' "$1" | grep -m1 -E '^(Error|error|REFUSED)')
  cause=$(printf '%s\n' "$1" | grep -E '^ +[0-9]+: [A-Za-z]' | tail -n 1 | sed 's/^ *[0-9]*: //')
  if [ -n "$first" ]; then printf '%s%s' "$first" "${cause:+ / $cause}"; else printf '%s\n' "$1" | tail -n 1; fi
}

# A binary that cannot launch makes every rung that needs it a REFUSED, never a silence.
brit_ok=1; have "$BRIT" || brit_ok=0
bbr_ok=1; have "$BBR" || bbr_ok=0
git_ok=1; have git || git_ok=0
is_repo=1
[ "$git_ok" = 1 ] && git -C "$repo" rev-parse --git-dir > /dev/null 2>&1 || is_repo=0

r1() {
  [ "$brit_ok" = 1 ] || { emit REFUSED R1 "brit binary '$BRIT' not found"; return; }
  [ "$is_repo" = 1 ] || { emit unmeasured R1 "'$repo' is not a git repository (or git missing)"; return; }
  local out rc
  out=$("$BRIT" -r "$repo" status --no-write 2>&1 > /dev/null); rc=$?
  if [ "$rc" = 0 ]; then emit ok R1 "status --no-write exit 0"; else emit REFUSED R1 "status --no-write exit $rc: $(tail1 "$out")"; fi
}

r2() {
  [ "$brit_ok" = 1 ] || { emit REFUSED R2 "brit binary '$BRIT' not found"; return; }
  [ "$is_repo" = 1 ] || { emit unmeasured R2 "'$repo' is not a git repository"; return; }
  local out rc t0 t1
  t0=$(date +%s)
  out=$("$BRIT" -r "$repo" verify 2>&1); rc=$?
  t1=$(date +%s)
  if [ "$rc" = 0 ]; then emit ok R2 "verify exit 0 in $((t1 - t0))s"; else emit REFUSED R2 "verify exit $rc after $((t1 - t0))s: $(tail1 "$out")"; fi
}

r3() {
  [ "$brit_ok" = 1 ] || { emit REFUSED R3 "brit binary '$BRIT' not found"; return; }
  [ "$is_repo" = 1 ] || { emit unmeasured R3 "'$repo' is not a git repository"; return; }
  local out rc first head
  out=$("$BRIT" -r "$repo" log 2>&1); rc=$?
  if [ "$rc" != 0 ]; then emit REFUSED R3 "log exit $rc: $(tail1 "$out")"; return; fi
  head=$(git -C "$repo" rev-parse HEAD 2> /dev/null) || { emit unmeasured R3 "git rev-parse HEAD failed (no commits?)"; return; }
  first=$(printf '%s\n' "$out" | head -n 1 | awk '{print $1}')
  if [ -n "$first" ] && [ "${head#"$first"}" != "$head" ]; then
    emit ok R3 "first log id $first is HEAD ${head:0:12}"
  else
    emit REFUSED R3 "first log id '${first}' != HEAD ${head:0:12}"
  fi
}

# Shared body of R4 and R5. $1 rung, remaining args: extra brit global args (e.g. -c ...).
author_rung() {
  local rung="$1"; shift
  [ "$brit_ok" = 1 ] || { emit REFUSED "$rung" "brit binary '$BRIT' not found"; return; }
  [ "$is_repo" = 1 ] || { emit unmeasured "$rung" "'$repo' is not a git repository"; return; }
  local clone="$scratch_root/$rung" out rc oracle got idx
  if ! out=$(git clone -q --shared --no-checkout "$repo" "$clone" 2>&1); then
    emit unmeasured "$rung" "scratch clone failed: $(tail1 "$out")"; return
  fi
  git -C "$clone" config user.name probe
  git -C "$clone" config user.email probe@example.invalid
  git -C "$clone" config commit.gpgSign false
  if ! git -C "$clone" read-tree HEAD 2> /dev/null; then
    emit unmeasured "$rung" "scratch clone has no HEAD to seed the index"; return
  fi
  mkdir -p "$clone/.git/hooks"
  printf '#!/bin/sh\ntouch "$GIT_DIR/hook-ran" 2>/dev/null || touch .git/hook-ran\n' > "$clone/.git/hooks/pre-commit"
  chmod +x "$clone/.git/hooks/pre-commit"
  printf 'probe %s\n' "$rung" > "$clone/probe-$rung.txt"
  # Oracle: stock git adds the same file to a private copy of the index.
  idx="$scratch_root/$rung.index"
  cp "$clone/.git/index" "$idx"
  GIT_INDEX_FILE="$idx" git -C "$clone" add -- "probe-$rung.txt" || { emit unmeasured "$rung" "git oracle add failed"; return; }
  oracle=$(GIT_INDEX_FILE="$idx" git -C "$clone" write-tree) || { emit unmeasured "$rung" "git oracle write-tree failed"; return; }
  if ! out=$("$BRIT" "$@" -r "$clone" add -- "probe-$rung.txt" 2>&1); then
    emit REFUSED "$rung" "add refused: $(tail1 "$out")"; return
  fi
  if ! out=$("$BRIT" "$@" -r "$clone" commit -m "probe $rung" 2>&1); then
    emit REFUSED "$rung" "commit refused: $(tail1 "$out")"; return
  fi
  got=$(git -C "$clone" rev-parse 'HEAD^{tree}' 2> /dev/null)
  rc=0; [ -e "$clone/.git/hook-ran" ] || rc=1
  if [ "$got" != "$oracle" ]; then
    emit REFUSED "$rung" "brit tree $got != git tree $oracle"
  elif [ "$rc" != 0 ]; then
    emit REFUSED "$rung" "tree matches git but the pre-commit hook did not run"
  else
    emit ok "$rung" "tree ${got:0:12} equals git's; pre-commit hook ran"
  fi
}

r4() { author_rung R4; }

r5() {
  # Explicit LFS filter driver and no attributes: configured drivers alone must never refuse.
  author_rung R5 \
    -c 'filter.lfs.clean=git-lfs clean -- %f' \
    -c 'filter.lfs.smudge=git-lfs smudge -- %f' \
    -c 'filter.lfs.process=git-lfs filter-process' \
    -c 'filter.lfs.required=true'
}

r6() {
  [ "$bbr_ok" = 1 ] || { emit REFUSED R6 "brit-build-ref binary '$BBR' not found"; return; }
  [ "$is_repo" = 1 ] || { emit unmeasured R6 "'$repo' is not a git repository"; return; }
  have jq || { emit unmeasured R6 "jq not installed; cannot read inputsHash from notes"; return; }
  local ref step enc blob obj hash out notes=0 steps=0 bad=0 first=""
  while read -r ref; do
    [ -n "$ref" ] || continue
    steps=$((steps + 1))
    enc="${ref#refs/notes/brit/build/}"
    step=$(printf '%b' "$(printf '%s' "$enc" | sed 's/%\([0-9A-Fa-f][0-9A-Fa-f]\)/\\x\1/g')")
    while read -r blob obj; do
      [ -n "$blob" ] || continue
      notes=$((notes + 1))
      hash=$(git -C "$repo" cat-file blob "$blob" 2> /dev/null | jq -r '.inputsHash // empty' 2> /dev/null)
      [ -n "$hash" ] || continue
      out=$("$BBR" --repo "$repo" build find --step "$step" --inputs-hash "$hash" 2>&1)
      if printf '%s\n' "$out" | grep -q 'unverifiable:'; then
        bad=$((bad + 1))
        [ -n "$first" ] || first=$(printf '%s\n' "$out" | grep -m1 'unverifiable:')
      fi
    done < <(git -C "$repo" notes --ref="$ref" list 2> /dev/null)
  done < <(git -C "$repo" for-each-ref --format='%(refname)' 'refs/notes/brit/build/*')
  if [ "$steps" = 0 ]; then
    emit unmeasured R6 "no refs/notes/brit/build/* in '$repo'"
  elif [ "$bad" = 0 ]; then
    emit ok R6 "build find over $notes notes in $steps steps: 0 unverifiable"
  else
    emit REFUSED R6 "$bad of $notes notes unverifiable across $steps steps; first: $first"
  fi
}

r7() {
  [ "$bbr_ok" = 1 ] || { emit REFUSED R7 "brit-build-ref binary '$BBR' not found"; return; }
  have git || { emit unmeasured R7 "git not installed; cannot make a scratch repo"; return; }
  have jq || { emit unmeasured R7 "jq not installed; cannot read agentId"; return; }
  local s="$scratch_root/R7" before after out rc id
  git init -q "$s" || { emit unmeasured R7 "scratch git init failed"; return; }
  before=$(cd "$s" && find . -type f | sort | sha1sum)
  out=$("$BBR" --repo "$s" signer --json 2>&1); rc=$?
  after=$(cd "$s" && find . -type f | sort | sha1sum)
  if [ "$rc" != 0 ]; then emit REFUSED R7 "signer --json exit $rc: $(tail1 "$out")"; return; fi
  id=$(printf '%s' "$out" | jq -r 'if .agentId == null then "null" else .agentId end' 2> /dev/null)
  if [ "$id" != null ]; then
    emit REFUSED R7 "fresh repo reports agentId '${id}' (expected null)"
  elif [ "$before" != "$after" ]; then
    emit REFUSED R7 "agentId null but signer created files in the scratch repo"
  else
    emit ok R7 "agentId null, no key file created"
  fi
}

for r in $RUNGS; do
  "$(printf '%s' "$r" | tr 'R' 'r')"
done

missing=""
for r in $RUNGS; do emitted "$r" || missing="$missing $r"; done
if [ -n "$missing" ]; then
  echo "probe bug: rung(s) printed no ok/REFUSED/unmeasured line:$missing" >&2
  exit 2
fi

if [ "$json" = 1 ]; then
  printf '%s' "$results" | jq -R -s '{rungs: (split("\n") | map(select(length > 0) | split("\t") | {rung: .[1], state: .[0], detail: (.[2] // "")}))}'
fi
exit "$refused"
