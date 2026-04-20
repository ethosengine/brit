# Must be sourced into the main journey test
# Smoke tests for the brit-verify binary.
# brit-verify is a single-purpose binary (no clap, no subcommands).
# Usage: brit-verify <commit-rev> [--repo <path>]
# Exits 2 on usage error, 1 on validation failure, 3 on repo error.

title brit-verify
exe_brit_verify="${exe%/*}/brit-verify"

(when "running 'brit-verify' with no args"
  it "prints usage and exits 2 (usage error)" && {
    expect_run $WITH_CLAP_FAILURE "$exe_brit_verify"
  }
)
