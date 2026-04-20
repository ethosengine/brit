# Must be sourced into the main journey test

WHITE="$(tput setaf 9 2>/dev/null || echo -n '')"
YELLOW="$(tput setaf 3 2>/dev/null || echo -n '')"
GREEN="$(tput setaf 2 2>/dev/null || echo -n '')"
RED="$(tput setaf 1 2>/dev/null || echo -n '')"
OFFSET=( )
STEP="  "

function on_ci () {
  [ -n "${CI-}" ] || {
    function expect_run () {
      echo 1>&2 "${WHITE} - skipped (runs only on CI)"
    }
    function expect_run_sh () {
      echo 1>&2 "${WHITE} - skipped (runs only on CI)"
    }
    function expect_run_sh_no_pipefail () {
      echo 1>&2 "${WHITE} - skipped (runs only on CI)"
    }
  }
}

function not_on_ci () {
  [ -z "${CI-}" ] || {
    function expect_run () {
      echo 1>&2 "${WHITE} - skipped (runs only locally)"
    }
    function expect_run_sh () {
      echo 1>&2 "${WHITE} - skipped (runs only locally)"
    }
    function expect_run_sh_no_pipefail () {
      echo 1>&2 "${WHITE} - skipped (runs only locally)"
    }
  }
}

function title () {
  echo "$WHITE-----------------------------------------------------"
  echo "${GREEN}$*"
  echo "$WHITE-----------------------------------------------------"
}

function _context () {
  local name="${1:?}"
  shift
  echo 1>&2 "${YELLOW}${OFFSET[*]:-}[$name] $*"
  OFFSET+=("$STEP")
}

function step () {
  _note step "${WHITE}" "$*"
}

function stepn () {
  step "$*" $'\n'
}

function with () {
  _context with "$*"
}

function when () {
  _context when "$*"
}

function _note () {
  local name="${1:?}"
  local color="${2:-}"
  shift 2
  echo 1>&2 -n "${OFFSET[*]:-}${color}[$name] ${*//  /}"
}

function it () {
  _note it "${GREEN}" "$*"
}

function precondition () {
  _note precondition "${WHITE}" "$*"
}

function shortcoming () {
  _note shortcoming "${RED}" "$*"
}

function step () {
  _note step "${WHITE}" "$*"
}

function stepn () {
  step "$*" $'\n'
}

function fail () {
  echo 1>&2 "${RED} $*"
  exit 1
}

function sandbox () {
  sandbox_tempdir="$(mktemp -t sandbox.XXXXXX -d)"
  # shellcheck disable=2064
  trap "popd >/dev/null" EXIT
  pushd "$sandbox_tempdir" >/dev/null \
   || fail "Could not change directory into temporary directory."

  local custom_init="${1:-}"
  if [ -n "$custom_init" ]; then
    eval "$custom_init"
  fi
}

function expect_equals () {
  expect_run 0 test "${1:?}" = "${2:?}"
}

function expect_exists () {
  expect_run 0 test -e "${1:?}"
}

function expect_run_sh () {
  expect_run "${1:?}" bash -c -eu -o pipefail "${2:?}"
}

function expect_run_sh_no_pipefail () {
  expect_run "${1:?}" bash -c -eu "${2:?}"
}

function expect_snapshot () {
  local expected=${1:?}
  local actual=${2:?}
  if ! [ -e "$expected" ]; then
    mkdir -p "${expected%/*}"
    cp -R "$actual" "$expected"
  fi
  expect_run 0 diff -r -N "$expected" "$actual"
}

function expect_run () {
  local expected_exit_code=$1
  shift
  local output=
  set +e
  if [[ -n "${SNAPSHOT_FILTER-}" ]]; then
    output="$("$@" 2>&1 | "$SNAPSHOT_FILTER")"
  else
    output="$("$@" 2>&1)"
  fi

  local actual_exit_code=$?
  if [[ "$actual_exit_code" == "$expected_exit_code" ]]; then
    if [[ -n "${WITH_SNAPSHOT-}" ]]; then
      local expected="$WITH_SNAPSHOT"
      if ! [ -f "$expected" ]; then
        mkdir -p "${expected%/*}"
        echo -n "$output" > "$expected" || exit 1
      fi
      if ! diff "$expected" <(echo -n "$output"); then
        echo 1>&2 "${RED} - FAIL"
        echo 1>&2 "${WHITE}\$ $*"
        echo 1>&2 "Output snapshot did not match snapshot at '$expected'"
        echo 1>&2 "$output"
        if [ -n "${ON_ERROR:-}" ]; then
          eval "$ON_ERROR"
        fi
        exit 1
      fi
    fi
    echo 1>&2
  else
    echo 1>&2 "${RED} - FAIL"
    echo 1>&2 "${WHITE}\$ $*"
    echo 1>&2 "${RED}Expected actual status $actual_exit_code to be $expected_exit_code"
    echo 1>&2 "$output"
    if [ -n "${ON_ERROR:-}" ]; then
      eval "$ON_ERROR"
    fi
    exit 1
  fi
  set -e
}

# Dump captured output to the cli-test-page staging directory.
# No-op when BRIT_TEST_PAGE_STAGING is unset (so journey.sh can run standalone
# without affecting the unrelated test-page artifact). Used by per-binary
# journey shells to feed shell-side captures into the unified baseline.md.
#
# Args:
#   $1 — binary name (e.g. "brit", "rakia")
#   $2 — subcommand path joined by slashes (e.g. "log" or "branch/list")
#   $3 — captured output (typically: "$(cmd 2>&1 || true)")
function dump-to-staging() {
  if [[ -z "${BRIT_TEST_PAGE_STAGING:-}" ]]; then
    return 0
  fi
  local binary="$1"
  local subpath="$2"
  local content="$3"
  local dir="${BRIT_TEST_PAGE_STAGING}/shell/${binary}/${subpath%/*}"
  # When subpath has no slash, dirname yields the full subpath; correct that.
  if [[ "$subpath" != */* ]]; then
    dir="${BRIT_TEST_PAGE_STAGING}/shell/${binary}"
  fi
  mkdir -p "$dir"
  local filename="${subpath##*/}"
  printf "%s" "$content" > "${dir}/${filename}.txt"
}
