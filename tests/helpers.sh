# Must be sourced into the main journey test

function set-static-git-environment() {
  set -a
  export GIT_AUTHOR_DATE="2020-09-09 09:06:03 +0800"
  export GIT_COMMITTER_DATE="${GIT_AUTHOR_DATE}"
  export GIT_AUTHOR_NAME="Sebastian Thiel"
  export GIT_COMMITTER_NAME="${GIT_AUTHOR_NAME}"
  export GIT_AUTHOR_EMAIL="git@example.com"
  export GIT_COMMITTER_EMAIL="${GIT_AUTHOR_EMAIL}"
  set +a
}

# git-init-hash-aware — drop-in replacement for `git init` in fixture
# helpers. Respects GIX_TEST_FIXTURE_HASH (sha1 default).
function git-init-hash-aware() {
  git init --object-format="${GIX_TEST_FIXTURE_HASH:-sha1}" "$@"
}

function remove-paths() {
  sed -E 's#/.*#"#g'
}

function repo-with-remotes() {
  if [[ $((($# - 1) % 2)) != 0 ]] || [[ $# = 0 ]]; then
    echo "need <path> (<remote> <url>)[,...] tuples"
    exit 42
  fi

  mkdir -p "$1"
  (
    cd "$1"
    shift
    git-init-hash-aware
    while [[ $# != 0 ]]; do
        git remote add "$1" "$2"
        shift 2
    done
    git config commit.gpgsign false
    git config tag.gpgsign false
    touch a
    git add a
    git commit -m "non-bare"
  ) &>/dev/null
}

function bare-repo-with-remotes() {
  if [[ $((($# - 1) % 2)) != 0 ]] || [[ $# = 0 ]]; then
    echo "need <path> (<remote> <url>)[,...] tuples"
    exit 42
  fi

  mkdir -p "$1"
  (
    cd "$1"
    shift
    git-init-hash-aware --bare
    while [[ $# != 0 ]]; do
        git remote add "$1" "$2"
        shift 2
    done
  ) &>/dev/null
}

function small-repo-in-sandbox() {
  sandbox
  {
    git-init-hash-aware
    git checkout -b main
    git config commit.gpgsign false
    git config tag.gpgsign false
    touch a
    git add a
    git commit -m "first"
    git tag unannotated
    touch b
    git add b
    git commit -m "second"
    git tag annotated -m "tag message"
    git branch dev
    echo hi >> b
    git commit -am "third"
  } &>/dev/null
}

function launch-git-daemon() {
    git -c uploadpack.allowrefinwant daemon --verbose --base-path=. --export-all --user-path &>/dev/null &
    daemon_pid=$!
    while ! nc -z localhost 9418; do
      sleep 0.1
    done
    trap 'kill $daemon_pid' EXIT
}

# only_for_hash <coverage> — returns 0 if the active GIX_TEST_FIXTURE_HASH
# is in the coverage set, else 1 (caller's subshell short-circuits).
#
# Coverage values:
#   dual       — always run (SHA-1 and SHA-256)
#   sha1-only  — run under sha1, skip under sha256 (legitimate when
#                the feature genuinely can't exercise hashing, e.g.,
#                operating on a remote that gix doesn't yet support
#                in sha256 mode)
function only_for_hash() {
  local want="${1:?only_for_hash: need coverage (dual|sha1-only)}"
  local have="${GIX_TEST_FIXTURE_HASH:-sha1}"
  case "$want" in
    dual) return 0 ;;
    sha1-only)
      if [[ "$have" == "sha1" ]]; then
        return 0
      else
        echo 1>&2 "${YELLOW}  [hash=$have] skipped (row coverage: sha1-only)"
        return 1
      fi
      ;;
    *)
      echo 1>&2 "${RED}only_for_hash: unknown coverage '$want' (want dual|sha1-only)"
      return 2
      ;;
  esac
}

# expect_parity — run the same args through git and gix, compare per-mode.
# Usage: expect_parity <effect|bytes> [--] <shared-args...>
# Modes:
#   effect  — exit-code match required; output diff reported but not fatal.
#             Callers can use $PARITY_GIT_OUT / $PARITY_GIX_OUT for token checks.
#   bytes   — exit-code AND byte-exact stdout+stderr match required.
# Requires $exe_plumbing (the gix binary) in scope — sourced by tests/parity.sh
# or tests/journey.sh.
function expect_parity() {
  local mode="${1:?expect_parity: need mode (effect|bytes)}"
  shift
  [[ "${1:-}" == "--" ]] && shift

  local git_out git_exit gix_out gix_exit
  git_out="$(git "$@" 2>&1)"; git_exit=$?
  gix_out="$("$exe_plumbing" "$@" 2>&1)"; gix_exit=$?

  export PARITY_GIT_OUT="$git_out" PARITY_GIT_EXIT="$git_exit"
  export PARITY_GIX_OUT="$gix_out" PARITY_GIX_EXIT="$gix_exit"

  if [[ "$git_exit" != "$gix_exit" ]]; then
    echo 1>&2 "${RED} - FAIL (exit-code divergence: git=$git_exit gix=$gix_exit)"
    echo 1>&2 "${WHITE}\$ git $*"
    echo 1>&2 "--- git ---"; echo 1>&2 "$git_out"
    echo 1>&2 "--- gix ---"; echo 1>&2 "$gix_out"
    return 1
  fi

  if [[ "$mode" == "bytes" && "$git_out" != "$gix_out" ]]; then
    echo 1>&2 "${RED} - FAIL (byte-level output divergence, exit=$git_exit)"
    echo 1>&2 "${WHITE}\$ $*"
    diff <(echo "$git_out") <(echo "$gix_out") 1>&2 || true
    return 1
  fi

  if [[ "$mode" != "effect" && "$mode" != "bytes" ]]; then
    echo 1>&2 "${RED}expect_parity: unknown mode '$mode' (want effect|bytes)"
    return 2
  fi

  local active_hash="${GIX_TEST_FIXTURE_HASH:-sha1}"
  echo 1>&2 "${GREEN} - OK ($mode parity, hash=$active_hash, exit=$git_exit)"
  return 0
}
