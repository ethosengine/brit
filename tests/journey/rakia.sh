# Must be sourced into the main journey test
# Smoke tests for the rakia binary — proves it starts + emits help.
# Detailed per-subcommand coverage lives in cli-journey/tests/rakia.rs (Rust).

title rakia
exe_rakia="${exe%/*}/rakia"
[ -x "$exe_rakia" ] || { echo "  (skip: rakia binary not built at $exe_rakia)"; return 0; }

(when "running 'rakia --help'"
  it "prints the top-level help" && {
    expect_run $SUCCESSFULLY "$exe_rakia" --help
  }
)

(when "running 'rakia' with no subcommand"
  it "exits 2 (clap usage error)" && {
    expect_run $WITH_CLAP_FAILURE "$exe_rakia"
  }
)
