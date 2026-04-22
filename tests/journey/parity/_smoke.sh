# Minimal smoke test for tests/parity.sh + expect_parity helper.
# Not tied to a real command — just exercises the wiring.
# Delete or keep as documentation, user's choice.

title "parity smoke test"
(sandbox
  (with "a trivial git invocation"
    it "git --version matches itself (sanity)" && {
      # expect_parity is designed for git ↔ gix; for smoke we fake both
      # by invoking git on both sides via $exe_plumbing override.
      # Here we only verify the helper's exit-code comparison path works.
      git_out="$(git --version 2>&1)"
      test -n "$git_out" || fail "git not callable"
      echo "${GREEN} - OK (git is callable: $git_out)"
    }
  )

  (with "expect_parity exit-code match path"
    # Run a command that exits 0 on both sides. Use `git --version` vs
    # `git --version` by pointing $exe_plumbing at /usr/bin/env git for
    # this smoke only.
    saved_exe_plumbing="$exe_plumbing"
    exe_plumbing="$(command -v git)"
    it "same command in both slots returns OK" && {
      expect_parity effect -- --version
    }
    exe_plumbing="$saved_exe_plumbing"
  )
)
