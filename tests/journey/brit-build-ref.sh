# Must be sourced into the main journey test
# Smoke tests for the brit-build-ref binary.

title brit-build-ref
exe_brit_build_ref="${exe%/*}/brit-build-ref"

(when "running 'brit-build-ref --help'"
  it "prints the top-level help with subcommand list" && {
    expect_run $SUCCESSFULLY "$exe_brit_build_ref" --help
  }
)

(when "running 'brit-build-ref' with no subcommand"
  it "exits 2 (clap usage error)" && {
    expect_run $WITH_CLAP_FAILURE "$exe_brit_build_ref"
  }
)
