#!/usr/bin/env bash
# Select this private Unix tool pair with PATH=<prefix>/current/bin:$PATH.
# Pins are independently supplied evidence. EPR does not embed its source SHA:
# its source pin is a caller declaration. Interface smoke is not acceptance.
set -euo pipefail
if [ "$#" -ne 7 ]; then
  echo 'usage: install-brit-epr.sh <brit-package> <brit-source> <brit-sha256> <epr-binary> <epr-source> <epr-sha256> <private-prefix>' >&2
  exit 2
fi
package=$1 brit_source=$2 brit_hash=$3 epr_binary=$4 epr_source=$5 epr_hash=$6 prefix=$7
script_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd -P)
fail() { echo "Brit/EPR installation refused: $*" >&2; exit 2; }
[[ "$epr_source" =~ ^[0-9a-f]{40}$ ]] || fail 'expected a full EPR source revision pin'
[[ "$epr_hash" =~ ^[0-9a-f]{64}$ ]] || fail 'expected an independently supplied EPR checksum'
[ -f "$epr_binary" ] && [ ! -L "$epr_binary" ] && [ -x "$epr_binary" ] || fail 'EPR must be a regular executable'
sha256_file() {
  if command -v sha256sum >/dev/null 2>&1; then sha256sum "$1" | awk '{print $1}';
  else shasum -a 256 "$1" | awk '{print $1}'; fi
}
[ "$(sha256_file "$epr_binary")" = "$epr_hash" ] || fail 'EPR checksum mismatch'
case "$(uname -s)" in
  Linux) rename_option=-T ;;
  Darwin) rename_option=-h ;;
  *) fail 'pair activation requires Linux or macOS' ;;
esac
[ ! -L "$prefix" ] || fail 'prefix must not be a symbolic link'
mkdir -p -- "$prefix"
prefix=$(cd -- "$prefix" && pwd -P)
[ ! -L "$prefix/releases" ] || fail 'releases must not be a symbolic link'
mkdir -p -- "$prefix/releases"
check_active() {
  [ ! -e "$prefix/current" ] || [ -L "$prefix/current" ] || fail 'refusing to replace an unmanaged current path'
  if [ -L "$prefix/current" ]; then
    case "$(readlink "$prefix/current")" in "$prefix/releases/"*) ;; *) fail 'current link is not a managed generation' ;; esac
  fi
}
check_active
stage=$(mktemp -d "$prefix/releases/pair.XXXXXXXX")
cleanup() {
  if [ "$(readlink "$prefix/current" 2>/dev/null || true)" != "$stage" ]; then rm -rf -- "$stage"; fi
}
trap cleanup EXIT
trap 'exit 130' INT
trap 'exit 143' TERM
bash "$script_dir/install-brit-release.sh" "$package" "$stage" "$brit_source" "$brit_hash" >/dev/null
cp -- "$epr_binary" "$stage/bin/epr"
[ "$(sha256_file "$stage/bin/epr")" = "$epr_hash" ] || fail 'copied EPR checksum mismatch'
help=$(cd -- "$stage" && ./bin/epr --help)
[[ "$help" == *'ready [--target REF]'* ]] || fail 'EPR lacks the required readiness interface'
help=$(cd -- "$stage" && ./bin/epr integrate --help)
[[ "$help" == *'integrate --acceptance <CID>'* ]] || fail 'EPR lacks the required integration interface'
printf 'format=brit-epr-install-v1\nbrit_source_commit=%s\nbrit_binary_sha256=%s\nepr_source_commit_declared=%s\nepr_binary_sha256=%s\nqualification=interface-smoke-only\n' \
  "$brit_source" "$brit_hash" "$epr_source" "$epr_hash" > "$stage/BUILD-INFO.txt"
check_active
ln -s "$stage" "$stage/activate"
# Plain mv follows directory symlinks; explicit no-follow replacement is vital.
# The unique source and destination share a filesystem, making rename atomic.
mv -f "$rename_option" -- "$stage/activate" "$prefix/current"
printf 'installed_pair=%s/current/bin\nbrit_source=%s\nepr_source_declared=%s\ninterface_smoke=passed\nworkflow_acceptance=not-established\n' "$prefix" "$brit_source" "$epr_source"
