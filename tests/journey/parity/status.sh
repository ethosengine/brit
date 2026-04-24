# Must be sourced into tests/parity.sh or tests/journey.sh — see tests/parity.sh.
#
# Parity scaffold for `git status` ↔ `gix status`.
#
# One `title` + `it` block per flag derived from
# vendor/git/Documentation/git-status.adoc and vendor/git/wt-status.c
# (wt_status_print and porcelain variants). Every `it` body starts as a
# TODO: placeholder — iteration N of the parity loop picks the next
# TODO, converts it to a real `expect_parity` assertion, and removes the
# TODO marker.
#
# Verdict modes (comment above each block):
#   bytes  — scriptable output consumed by tooling (--porcelain, -z,
#            --short, --porcelain=v2); byte-exact match required
#   effect — exit-code + UX; output diff reported but not fatal
#
# Hash coverage: most rows are `sha1-only` — gix's config layer rejects
# `extensions.objectFormat=sha256` (gix/src/config/tree/sections/extensions.rs
# try_into_object_format, sha1-only validator), so `gix status` cannot
# open any sha256 repo. Rows that never touch a repo (--help, unknown
# option, outside-of-repo) are `dual`. Rows flip to `dual` once the
# gix-config validator accepts sha256.
#
# Coverage on gix's current Clap surface (src/plumbing/options/mod.rs):
# gix status has --format {simplified,porcelain-v2}, --ignored, --submodules,
# -s/--statistics (conflicts with git's -s/--short), --no-write,
# --index-worktree-renames, and [pathspec]. It does NOT yet accept
# -s/--short, -b/--branch, --show-stash, --porcelain[=v1], --long,
# -v/--verbose, -u/--untracked-files, --ignore-submodules (git's spelling;
# gix uses --submodules=none), -z, --column/--no-column, --ahead-behind,
# --renames/--no-renames, --find-renames. Most rows will fail in their
# first iteration and guide Clap-surface expansion.

title "gix status"

# --- meta / help ---------------------------------------------------------

# mode=effect — clap --help short-circuits before repo load, exits 0.
# git's --help delegates to `man git-status` (exit 0 when man is
# available); gix returns clap's auto-generated help. Message text
# diverges wildly; only the exit-code match is asserted.
# hash=dual
title "gix status --help"
only_for_hash dual && (sandbox
  it "matches git behavior" && {
    expect_parity effect -- status --help
  }
)

# --- argument-parsing error paths ---------------------------------------

# mode=effect — unknown flag: git exits 129 (usage_msg_opt in
# vendor/git/parse-options.c::PARSE_OPT_HELP). gix originally exited 2
# (clap default for ErrorKind::UnknownArgument); src/plumbing/main.rs
# intercepts try_parse_from and remaps UnknownArgument → exit 129 to
# restore parity. Other clap errors (ValueValidation, DisplayHelp,
# DisplayVersion) keep their default codes. Tested inside a repo: git
# outside a repo dies 128 before reaching arg-parse, while clap in gix
# always runs first — so the exercise must live in a small-repo fixture
# to isolate the arg-parse error path. That pushes this row to
# sha1-only (gix-config rejects sha256 on any repo open).
# hash=sha1-only "gix cannot load sha256 repos: extensions.objectFormat=sha256 rejected (gix/src/config/tree/sections/extensions.rs)"
title "gix status --bogus-flag"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    expect_parity effect -- status --bogus-flag
  }
)

# mode=effect — `git status` outside any repo dies 128 with
# "fatal: not a git repository (or any of the parent directories): .git".
# gix originally exited 1 with an anyhow trace through NoGitRepository;
# src/plumbing/main.rs's repository() closure now intercepts the
# gix_discover::upwards::Error::NoGitRepository* variants, writes git's
# exact wording, and exits 128. Scoped to plumbing commands that require
# a repo — env/clone are unaffected.
# hash=dual
title "gix status (outside a repository)"
only_for_hash dual && (sandbox
  it "matches git behavior" && {
    expect_parity effect -- status
  }
)

# --- happy paths ---------------------------------------------------------

# mode=effect — baseline vanilla status in a clean repo, no flags.
# git prints "On branch <br>\nnothing to commit, working tree clean";
# gix prints nothing. Both exit 0. Effect mode (exit-code parity only).
# hash=sha1-only "gix cannot load sha256 repos: extensions.objectFormat=sha256 rejected (gix/src/config/tree/sections/extensions.rs)"
title "gix status (clean working tree)"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    expect_parity effect -- status
  }
)

# mode=effect — a tracked file modified in the worktree but not staged.
# git prints a long-form "Changes not staged for commit" section;
# gix prints a simplified short-format line (`  M a`). Both exit 0.
# hash=sha1-only "gix cannot load sha256 repos: extensions.objectFormat=sha256 rejected (gix/src/config/tree/sections/extensions.rs)"
title "gix status (modified tracked file)"
only_for_hash sha1-only && (small-repo-in-sandbox
  echo modified >> a
  it "matches git behavior" && {
    expect_parity effect -- status
  }
)

# mode=effect — a new untracked file present in the worktree.
# Both exit 0; output wording diverges (git "Untracked files:" section
# vs gix's `? new-untracked` short line).
# hash=sha1-only "gix cannot load sha256 repos: extensions.objectFormat=sha256 rejected (gix/src/config/tree/sections/extensions.rs)"
title "gix status (untracked file)"
only_for_hash sha1-only && (small-repo-in-sandbox
  touch new-untracked
  it "matches git behavior" && {
    expect_parity effect -- status
  }
)

# mode=effect — a new file staged to the index, not yet committed.
# git prints a "Changes to be committed" section; gix prints `A  <path>`.
# Both exit 0.
# hash=sha1-only "gix cannot load sha256 repos: extensions.objectFormat=sha256 rejected (gix/src/config/tree/sections/extensions.rs)"
title "gix status (staged new file)"
only_for_hash sha1-only && (small-repo-in-sandbox
  touch staged-file && git add staged-file
  it "matches git behavior" && {
    expect_parity effect -- status
  }
)

# mode=effect — a tracked file modified in the worktree AND staged (two
# distinct XY states for the same path).
# hash=sha1-only "gix cannot load sha256 repos: extensions.objectFormat=sha256 rejected (gix/src/config/tree/sections/extensions.rs)"
title "gix status (modified + staged same path)"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    # TODO: echo x >> a && git add a && echo y >> a; expect_parity effect -- status
    true
  }
)

# --- short / long format -------------------------------------------------

# mode=bytes — `-s` short format: one `XY <path>` line per entry. Scripts
# parse this; byte-exact match required. gix has no `-s` flag yet (its
# `-s` means `--statistics`). Row will fail on Clap parse first.
# hash=sha1-only "gix cannot load sha256 repos: extensions.objectFormat=sha256 rejected (gix/src/config/tree/sections/extensions.rs)"
title "gix status -s"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    # TODO: stage + modify mix; expect_parity bytes -- status -s
    true
  }
)

# mode=bytes — `--short` long-form equivalent of `-s`.
# hash=sha1-only "gix cannot load sha256 repos: extensions.objectFormat=sha256 rejected (gix/src/config/tree/sections/extensions.rs)"
title "gix status --short"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    # TODO: stage + modify mix; expect_parity bytes -- status --short
    true
  }
)

# mode=effect — `--long` is the default; explicit flag should match
# bare `git status`. Output text diverges so effect only.
# hash=sha1-only "gix cannot load sha256 repos: extensions.objectFormat=sha256 rejected (gix/src/config/tree/sections/extensions.rs)"
title "gix status --long"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    # TODO: stage + modify mix; expect_parity effect -- status --long
    true
  }
)

# --- branch / stash metadata --------------------------------------------

# mode=bytes — `-b`/`--branch` prepends a branch/tracking-info header
# (format defined in git-status.adoc "Branch Headers" for porcelain=v2,
# or `## <branch>...<upstream>` for the short format).
# hash=sha1-only "gix cannot load sha256 repos: extensions.objectFormat=sha256 rejected (gix/src/config/tree/sections/extensions.rs)"
title "gix status -b / --branch"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior with -b" && {
    # TODO: expect_parity bytes -- status -b
    true
  }
  it "matches git behavior with --branch" && {
    # TODO: expect_parity bytes -- status --branch
    true
  }
)

# mode=effect — `--show-stash` prints `# stash <N>` if N > 0 (porcelain=v2)
# or a "Your stash currently has N entries" line (long format). Requires
# a populated stash to exercise.
# hash=sha1-only "gix cannot load sha256 repos: extensions.objectFormat=sha256 rejected (gix/src/config/tree/sections/extensions.rs)"
title "gix status --show-stash"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    # TODO: populate a stash entry; expect_parity effect -- status --show-stash
    true
  }
)

# --- porcelain format ---------------------------------------------------

# mode=bytes — `--porcelain` (default version = v1). Stable across git
# versions; scripts grep the byte-exact format.
# hash=sha1-only "gix cannot load sha256 repos: extensions.objectFormat=sha256 rejected (gix/src/config/tree/sections/extensions.rs)"
title "gix status --porcelain"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    # TODO: stage + untracked mix; expect_parity bytes -- status --porcelain
    true
  }
)

# mode=bytes — explicit `--porcelain=v1` (same format, explicit version).
# hash=sha1-only "gix cannot load sha256 repos: extensions.objectFormat=sha256 rejected (gix/src/config/tree/sections/extensions.rs)"
title "gix status --porcelain=v1"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    # TODO: expect_parity bytes -- status --porcelain=v1
    true
  }
)

# mode=bytes — `--porcelain=v2` adds branch/stash headers and extended
# per-entry fields (<XY> <sub> <mH> <mI> <mW> <hH> <hI> <path>). gix
# already implements PorcelainV2 under its `--format porcelain-v2` flag
# — this row verifies the git-compatible spelling and byte-exactness.
# hash=sha1-only "gix cannot load sha256 repos: extensions.objectFormat=sha256 rejected (gix/src/config/tree/sections/extensions.rs)"
title "gix status --porcelain=v2"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    # TODO: stage + untracked mix; expect_parity bytes -- status --porcelain=v2
    true
  }
)

# --- verbosity -----------------------------------------------------------

# mode=effect — `-v` / `--verbose` appends a `diff --cached`-style section
# for staged changes. `-vv` additionally appends an unstaged-diff section.
# hash=sha1-only "gix cannot load sha256 repos: extensions.objectFormat=sha256 rejected (gix/src/config/tree/sections/extensions.rs)"
title "gix status -v / --verbose"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior with -v" && {
    # TODO: stage a change; expect_parity effect -- status -v
    true
  }
  it "matches git behavior with --verbose" && {
    # TODO: expect_parity effect -- status --verbose
    true
  }
  it "matches git behavior with -vv" && {
    # TODO: stage + worktree changes; expect_parity effect -- status -vv
    true
  }
)

# --- untracked-files ----------------------------------------------------

# mode=effect — `-u<mode>` / `--untracked-files=<mode>` toggles untracked
# listing. The mode is stuck to the option (`-uno`, not `-u no`). Bare
# `-u` / `--untracked-files` default to `all`.
# hash=sha1-only "gix cannot load sha256 repos: extensions.objectFormat=sha256 rejected (gix/src/config/tree/sections/extensions.rs)"
title "gix status -u / --untracked-files[=<mode>]"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior with -uno" && {
    # TODO: untracked dir+files; expect_parity effect -- status -uno
    true
  }
  it "matches git behavior with --untracked-files=no" && {
    # TODO: expect_parity effect -- status --untracked-files=no
    true
  }
  it "matches git behavior with -unormal" && {
    # TODO: expect_parity effect -- status -unormal
    true
  }
  it "matches git behavior with --untracked-files=normal" && {
    # TODO: expect_parity effect -- status --untracked-files=normal
    true
  }
  it "matches git behavior with -uall" && {
    # TODO: expect_parity effect -- status -uall
    true
  }
  it "matches git behavior with --untracked-files=all" && {
    # TODO: expect_parity effect -- status --untracked-files=all
    true
  }
  it "matches git behavior with bare -u (defaults to all)" && {
    # TODO: expect_parity effect -- status -u
    true
  }
  it "matches git behavior with bare --untracked-files (defaults to all)" && {
    # TODO: expect_parity effect -- status --untracked-files
    true
  }
)

# --- submodule handling -------------------------------------------------

# mode=effect — `--ignore-submodules[=<when>]` (none/untracked/dirty/all,
# default=all when flag is bare). This fixture has no submodules so the
# row exercises the CLI parse path only; submodule-state variations are
# out of scope until a submodule fixture helper exists.
# hash=sha1-only "gix cannot load sha256 repos: extensions.objectFormat=sha256 rejected (gix/src/config/tree/sections/extensions.rs)"
title "gix status --ignore-submodules[=<when>]"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior with bare --ignore-submodules (all)" && {
    # TODO: expect_parity effect -- status --ignore-submodules
    true
  }
  it "matches git behavior with --ignore-submodules=none" && {
    # TODO: expect_parity effect -- status --ignore-submodules=none
    true
  }
  it "matches git behavior with --ignore-submodules=untracked" && {
    # TODO: expect_parity effect -- status --ignore-submodules=untracked
    true
  }
  it "matches git behavior with --ignore-submodules=dirty" && {
    # TODO: expect_parity effect -- status --ignore-submodules=dirty
    true
  }
  it "matches git behavior with --ignore-submodules=all" && {
    # TODO: expect_parity effect -- status --ignore-submodules=all
    true
  }
)

# --- ignored ------------------------------------------------------------

# mode=effect — `--ignored[=<mode>]` (traditional/no/matching, default=
# traditional when flag is bare). gix has `--ignored [collapsed|matching]`
# which does not map 1:1 to git's spelling — row will surface a
# Clap-surface gap (`traditional` missing, `no` missing, and gix's
# `collapsed` has no git equivalent).
# hash=sha1-only "gix cannot load sha256 repos: extensions.objectFormat=sha256 rejected (gix/src/config/tree/sections/extensions.rs)"
title "gix status --ignored[=<mode>]"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior with bare --ignored (traditional)" && {
    # TODO: .gitignore + matching file; expect_parity effect -- status --ignored
    true
  }
  it "matches git behavior with --ignored=traditional" && {
    # TODO: expect_parity effect -- status --ignored=traditional
    true
  }
  it "matches git behavior with --ignored=no" && {
    # TODO: expect_parity effect -- status --ignored=no
    true
  }
  it "matches git behavior with --ignored=matching" && {
    # TODO: expect_parity effect -- status --ignored=matching
    true
  }
)

# --- NUL-separated / column output --------------------------------------

# mode=bytes — `-z` terminates entries with NUL; implies `--porcelain=v1`
# if no other format is given. Scripts depend on exact NUL-separation and
# the field-order reversal for rename entries.
# hash=sha1-only "gix cannot load sha256 repos: extensions.objectFormat=sha256 rejected (gix/src/config/tree/sections/extensions.rs)"
title "gix status -z"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    # TODO: stage + untracked mix; expect_parity bytes -- status -z
    true
  }
)

# mode=effect — `--column[=<opts>]` displays untracked files in columns
# (default off in non-TTY output); `--no-column` disables explicitly.
# Honors `column.status` and `column.ui` config; without a TTY, gix and
# git typically both emit one-per-line, so effect-mode is sufficient.
# hash=sha1-only "gix cannot load sha256 repos: extensions.objectFormat=sha256 rejected (gix/src/config/tree/sections/extensions.rs)"
title "gix status --column / --no-column"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior with --column" && {
    # TODO: several untracked files; expect_parity effect -- status --column
    true
  }
  it "matches git behavior with --no-column" && {
    # TODO: expect_parity effect -- status --no-column
    true
  }
  it "matches git behavior with --column=always" && {
    # TODO: expect_parity effect -- status --column=always
    true
  }
)

# --- ahead-behind -------------------------------------------------------

# mode=effect — `--ahead-behind` (default) / `--no-ahead-behind` toggle
# upstream divergence counts in both the long and porcelain=v2 output.
# Requires an upstream branch pointing at a diverged commit.
# hash=sha1-only "gix cannot load sha256 repos: extensions.objectFormat=sha256 rejected (gix/src/config/tree/sections/extensions.rs)"
title "gix status --ahead-behind / --no-ahead-behind"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior with --ahead-behind" && {
    # TODO: fabricate upstream (bare clone + remote add + upstream-set);
    #       commit ahead; expect_parity effect -- status --ahead-behind
    true
  }
  it "matches git behavior with --no-ahead-behind" && {
    # TODO: same upstream setup; expect_parity effect -- status --no-ahead-behind
    true
  }
)

# --- renames ------------------------------------------------------------

# mode=effect — `--renames` / `--no-renames` force rename detection on/off
# regardless of `status.renames` config. Exercised via `git mv` of a
# tracked file, which leaves the index with an add/delete pair.
# hash=sha1-only "gix cannot load sha256 repos: extensions.objectFormat=sha256 rejected (gix/src/config/tree/sections/extensions.rs)"
title "gix status --renames / --no-renames"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior with --renames" && {
    # TODO: git mv a renamed-a; expect_parity effect -- status --renames
    true
  }
  it "matches git behavior with --no-renames" && {
    # TODO: same fixture; expect_parity effect -- status --no-renames
    true
  }
)

# mode=effect — `--find-renames[=<n>]` enables rename detection with
# optional similarity threshold. Bare form uses git's default (50%);
# explicit `=50` matches. gix has `--index-worktree-renames [<f32>]` but
# no `--find-renames` spelling.
# hash=sha1-only "gix cannot load sha256 repos: extensions.objectFormat=sha256 rejected (gix/src/config/tree/sections/extensions.rs)"
title "gix status --find-renames[=<n>]"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior with bare --find-renames" && {
    # TODO: git mv a renamed-a; expect_parity effect -- status --find-renames
    true
  }
  it "matches git behavior with --find-renames=50" && {
    # TODO: same fixture; expect_parity effect -- status --find-renames=50
    true
  }
)

# --- pathspec -----------------------------------------------------------

# mode=effect — a positional pathspec narrows status output to matching
# paths only. Exercised as positional arg and as explicit `-- <pathspec>`.
# hash=sha1-only "gix cannot load sha256 repos: extensions.objectFormat=sha256 rejected (gix/src/config/tree/sections/extensions.rs)"
title "gix status <pathspec>"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior with positional pathspec" && {
    # TODO: modify a + untracked b; expect_parity effect -- status a
    true
  }
  it "matches git behavior with -- <pathspec>" && {
    # TODO: same fixture; expect_parity effect -- status -- a
    true
  }
)

# End-of-file sentinel: when every row is `only_for_hash sha1-only` and the
# active hash is sha256, the last statement returns 1 (skip), which would
# propagate out of `source` and trip `set -e` in tests/parity.sh. A trailing
# `:` normalizes the exit code so a fully-skipped file still returns 0.
:
