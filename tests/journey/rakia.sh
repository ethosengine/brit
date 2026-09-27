# Must be sourced into the main journey test
# Smoke tests for the build commands in the one brit executable.

title 'brit build'
[ -x "$exe_plumbing" ] || { echo "missing brit binary at $exe_plumbing" >&2; return 1; }

(when "running 'brit build --help'"
  it "prints the top-level help" && {
    expect_run $SUCCESSFULLY "$exe_plumbing" build --help
  }
)

(when "running 'brit build' with no subcommand"
  it "exits 2 (clap usage error)" && {
    expect_run $WITH_CLAP_FAILURE "$exe_plumbing" build
  }
)
