# Optional witness only. Arguments and location come from the running Bash helper.
function _parity_emit() {
  [[ -n "${PARITY_EVENTS:-}" ]] || return 0
  local phase="$1" helper="$2" mode="$3" status="$4" actual="$5" git_status="$6" expected="$7" reason="$8"
  shift 8
  local line=0 i
  for ((i=1; i<${#BASH_SOURCE[@]}; i++)); do
    if [[ "${BASH_SOURCE[$i]}" == "${PARITY_SUITE_PATH:-}" ]]; then
      line="${BASH_LINENO[$((i-1))]}"
      break
    fi
  done
  if [[ "$phase" != "assertion-end" ]]; then
    _PARITY_SEQUENCE=$((${_PARITY_SEQUENCE:-0} + 1))
    _PARITY_INVOCATION="$BASHPID:$_PARITY_SEQUENCE"
  fi
  local actual_binary="${exe_plumbing:-}"
  if [[ "$helper" == expect_run ]]; then
    actual_binary="$(command -v -- "${1:-}" || true)"
  fi
  PARITY_ACTUAL_BINARY="$actual_binary" PARITY_COMPAT_REASON="${_PARITY_COMPAT_REASON:-}" "${PARITY_PYTHON:?}" "${PARITY_EVENT_HELPER:?}" \
    "$phase" "${_PARITY_HELPER:-$helper}" "$mode" "$status" "$actual" "$git_status" "$expected" \
    "$reason" "$line" "${_PARITY_INVOCATION:-unknown}" "$@"
}
