# Must be sourced into tests/parity.sh or tests/journey.sh — see tests/parity.sh.
#
# Parity scaffold for `git reset` ↔ `gix reset`.
#
# One `title` + `it` block per flag derived from
# vendor/git/Documentation/git-reset.adoc and
# vendor/git/builtin/reset.c::cmd_reset (entry at
# vendor/git/builtin/reset.c:336). The flag surface (per
# vendor/git/builtin/reset.c:350..382) is:
#   -q / --quiet, --no-refresh (with implicit --refresh inverse),
#   --mixed / --soft / --hard / --merge / --keep (PARSE_OPT_NONEG,
#   mutually exclusive at the C site), --recurse-submodules[=mode] /
#   --no-recurse-submodules, -p / --patch, --auto-advance, -U / --unified,
#   --inter-hunk-context, -N / --intent-to-add, --pathspec-from-file,
#   --pathspec-file-nul.
#
# Synopsis forms (vendor/git/builtin/reset.c:44):
#   git reset [--mixed | --soft | --hard | --merge | --keep] [-q] [<commit>]
#   git reset [-q] [<tree-ish>] [--] <pathspec>...
#   git reset [-q] [--pathspec-from-file=<file> [--pathspec-file-nul]] [<tree-ish>]
#   git reset --patch [<tree-ish>] [--] [<pathspec>...]
#
# Verdict modes (comment above each block):
#   bytes  — scriptable output: precondition error stanzas around
#            mode mutual-exclusion, --mixed-in-bare-repo, non-mixed +
#            paths, -N-without-mixed, --pathspec-file-nul-without-
#            --pathspec-from-file, the bad-revspec stanza, and the
#            outside-of-repo "fatal: not a git repository..." stanza.
#            Wired in gitoxide-core/src/repository/reset.rs::porcelain.
#   effect — UX-level parity (exit-code match). Default for the
#            human-rendered flags whose semantics are not yet
#            implemented in gix's reset entry point. Most rows close
#            as `compat_effect "deferred until reset driver lands"`.
#
# Coverage on gix's current Clap surface (src/plumbing/options/reset.rs):
#   gix reset [OPTIONS] [ARGS]... [-- <PATHS>...]
# Today the porcelain driver is a placeholder
# (gitoxide-core/src/repository/reset.rs::porcelain) that emits a
# stub note on stdout and exits 0. The flag surface is clap-wired so
# `gix reset <flag> ...` does not trip UnknownArgument; every
# flag-bearing row therefore closes as
# `compat_effect "deferred until reset driver lands"` until the real
# driver implements the semantic. Closing this command requires
# implementing the reset driver in
# gitoxide-core/src/repository/reset.rs:
#   * MIXED (default): three-way unpack with
#     UNPACK_RESET_PROTECT_UNTRACKED + index refresh + new-HEAD
#     ORIG_HEAD bookkeeping, mirroring vendor/git/builtin/reset.c:90..94.
#   * SOFT: HEAD-only update, no index/working-tree changes
#     (vendor/git/builtin/reset.c:486..488).
#   * HARD: three-way unpack with UNPACK_RESET_OVERWRITE_UNTRACKED +
#     working-tree update + the
#     "HEAD is now at <abbrev> <subject>" emission via
#     `print_new_head_line` at vendor/git/builtin/reset.c:137..149.
#   * MERGE / KEEP: oneway/twoway merge with disallowed-state guards
#     per the discussion tables in
#     vendor/git/Documentation/git-reset.adoc:386..506.
#   * --pathspec-from-file: parse_pathspec_file at
#     vendor/git/builtin/reset.c:398.
#   * --patch: run_add_p reset replay at
#     vendor/git/builtin/reset.c:436..442.
# Plus the precondition gates:
#   * --mixed in bare repo dies 128 (vendor/git/builtin/reset.c:473).
#   * -N without --mixed dies 128 (vendor/git/builtin/reset.c:477).
#   * --pathspec-file-nul without --pathspec-from-file dies 128
#     (vendor/git/builtin/reset.c:401).
#   * non-mixed + paths dies 128 (vendor/git/builtin/reset.c:458).
#   * --mixed + paths emits a deprecation warning, not an error
#     (vendor/git/builtin/reset.c:457).
#
# Hash coverage: `dual` rows never open a repo (--help, outside-of-repo).
# Every row that opens a repository is `sha1-only` because gix-config
# rejects `extensions.objectFormat=sha256`
# (gix/src/config/tree/sections/extensions.rs try_into_object_format,
# sha1-only validator), blocking every sha256 fixture at open. Rows
# flip to `dual` once that validator accepts sha256.
#
# parity-defaults:
#   hash=sha1-only "gix cannot load sha256 repos: extensions.objectFormat=sha256 rejected (gix/src/config/tree/sections/extensions.rs)"
#   mode=effect

# --- meta / help --------------------------------------------------------

# mode=effect — clap --help short-circuits before repo load, exits 0.
# git's --help delegates to `man git-reset` (exit 0 when man is
# available); gix returns clap's auto-generated help. Message text
# diverges; only the exit-code match is asserted.
# hash=dual
title "gix reset --help"
only_for_hash dual && (sandbox
  it "matches git behavior" && {
    expect_parity effect -- reset --help
  }
)

# mode=effect — unknown flag: git exits 129 (usage_msg_opt in
# parse-options.c). gix's Clap layer maps UnknownArgument to 129 via
# src/plumbing/main.rs. Tested inside a repo because git outside a
# repo dies 128 before reaching arg-parse, while clap in gix always
# runs first.
# hash=sha1-only
title "gix reset --bogus-flag"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    expect_parity effect -- reset --bogus-flag
  }
)

# mode=bytes — outside any repo: git dies 128 with "fatal: not a git
# repository (or any of the parent directories): .git". gix's plumbing
# repository() closure already remaps gix_discover::upwards::Error::
# NoGitRepository* variants to git's exact wording + exit 128.
# hash=dual
title "gix reset (outside a repository)"
only_for_hash dual && (sandbox
  it "matches git behavior" && {
    expect_parity bytes -- reset
  }
)

# --- synopsis: default mixed against HEAD ------------------------------

# mode=effect — bare `git reset` defaults to MIXED against HEAD: index
# refreshed to HEAD's tree, working tree untouched, "Unstaged changes
# after reset:" emitted. gix's placeholder emits a stub note + exits 0;
# exit-code parity holds.
# hash=sha1-only
title "gix reset (default mixed, no args)"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until reset driver lands" -- reset
  }
)

# mode=effect — `git reset HEAD` is the explicit form of the bare reset.
# hash=sha1-only
title "gix reset HEAD"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until reset driver lands" -- reset HEAD
  }
)

# mode=effect — `git reset HEAD~1` rewinds HEAD one commit, MIXED.
# hash=sha1-only
title "gix reset HEAD~1"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until reset driver lands" -- reset HEAD~1
  }
)

# mode=effect — `git reset` on an unborn-HEAD repo (fresh `git init`).
# Per vendor/git/builtin/reset.c:407, unborn=true sets the target tree
# to the empty-tree OID, and the reset proceeds against that empty
# tree. gix's placeholder emits a stub note + exits 0.
# hash=sha1-only
title "gix reset (unborn HEAD)"
only_for_hash sha1-only && (sandbox
  git-init-hash-aware
  it "matches git behavior" && {
    compat_effect "deferred until reset driver lands" -- reset
  }
)

# mode=bytes — `git reset <bad-revspec>` dies 128 via
# vendor/git/builtin/reset.c:247 `parse_args` →
# `verify_filename` → `die_for_pseudoref` with the verbatim 3-line
# "ambiguous argument" stanza. gix's placeholder mirrors the stanza
# byte-exactly (gitoxide-core/src/repository/reset.rs::porcelain
# bad-revspec gate).
# hash=sha1-only
title "gix reset <bad-revspec>"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    expect_parity bytes -- reset no-such-rev
  }
)

# --- mode flags --------------------------------------------------------

# mode=effect — `--mixed` (explicit form of the default).
# hash=sha1-only
title "gix reset --mixed HEAD~1"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until reset driver lands" -- reset --mixed HEAD~1
  }
)

# mode=effect — `--soft HEAD~1` keeps both the index and working tree.
# Per vendor/git/builtin/reset.c:486..488, SOFT is the only mode that
# does not unpack trees; only the HEAD ref moves.
# hash=sha1-only
title "gix reset --soft HEAD~1"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until reset driver lands" -- reset --soft HEAD~1
  }
)

# mode=effect — `--hard HEAD~1` overwrites the index AND working tree.
# Per vendor/git/builtin/reset.c:85..89, HARD enables
# UNPACK_RESET_OVERWRITE_UNTRACKED. The success line
# "HEAD is now at <abbrev> <subject>" is emitted via
# `print_new_head_line` at vendor/git/builtin/reset.c:137..149.
# hash=sha1-only
title "gix reset --hard HEAD~1"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until reset driver lands" -- reset --hard HEAD~1
  }
)

# mode=effect — `--merge HEAD~1` exists for resetting out of a
# conflicted merge: keeps unstaged working-tree changes that don't
# overlap, errors otherwise. Per
# vendor/git/Documentation/git-reset.adoc:58..66.
# hash=sha1-only
title "gix reset --merge HEAD~1"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until reset driver lands" -- reset --merge HEAD~1
  }
)

# mode=effect — `--keep HEAD~1` resets index + worktree to target,
# preserving local working-tree changes that don't conflict. Per
# vendor/git/Documentation/git-reset.adoc:68..72.
# hash=sha1-only
title "gix reset --keep HEAD~1"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until reset driver lands" -- reset --keep HEAD~1
  }
)

# --- mode interactions -------------------------------------------------

# mode=effect — combining two mode flags (`--soft --hard`): git's
# parse-options layer sets reset_type to the *last* OPT_SET_INT_F seen
# (no error). gix's Clap surface accepts both bools without validating
# mutual exclusion; the eventual reset driver should mirror git's
# last-wins semantic. Today the placeholder exits 0 either way.
# hash=sha1-only
title "gix reset --soft --hard HEAD~1"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until reset driver lands" -- reset --soft --hard HEAD~1
  }
)

# mode=bytes — `--soft` + paths errors at
# vendor/git/builtin/reset.c:458 ("Cannot do <mode> reset with paths.").
# gix's placeholder mirrors the verbatim wording.
# hash=sha1-only
title "gix reset --soft -- a"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    expect_parity bytes -- reset --soft -- a
  }
)

# mode=effect — `--mixed` + paths is the deprecated form per
# vendor/git/builtin/reset.c:457: emits a warning ("--mixed with
# paths is deprecated; use 'git reset -- <paths>' instead.") and
# proceeds with path-mode reset (i.e., `read_from_tree`). gix's
# placeholder skips the warning; bytes-mode parity is deferred.
# hash=sha1-only
title "gix reset --mixed -- a"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until reset driver lands" -- reset --mixed -- a
  }
)

# --- verbosity / refresh ----------------------------------------------

# mode=effect — `-q` suppresses the progress-message branch (refresh
# uses REFRESH_QUIET) and the "HEAD is now at..." line for HARD.
# hash=sha1-only
title "gix reset -q HEAD~1"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until reset driver lands" -- reset -q HEAD~1
  }
)

# mode=effect — long alias for `-q`.
# hash=sha1-only
title "gix reset --quiet HEAD~1"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until reset driver lands" -- reset --quiet HEAD~1
  }
)

# mode=effect — `--refresh` is the implicit default (per
# vendor/git/Documentation/git-reset.adoc:115); explicit form is a
# no-op. Accepted by git's parse-options as the inverse of
# `--no-refresh`.
# hash=sha1-only
title "gix reset --refresh HEAD"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until reset driver lands" -- reset --refresh HEAD
  }
)

# mode=effect — `--no-refresh` skips the `refresh_index` call after a
# MIXED reset (vendor/git/builtin/reset.c:500..511). Useful when the
# refresh is expensive on large repos.
# hash=sha1-only
title "gix reset --no-refresh HEAD"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until reset driver lands" -- reset --no-refresh HEAD
  }
)

# --- recurse-submodules -----------------------------------------------

# mode=effect — bare `--recurse-submodules` (no value) defaults to
# on-demand per `parse_update_recurse_submodules_arg`. Threads through
# the unpack-trees opts.
# hash=sha1-only
title "gix reset --recurse-submodules HEAD"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until reset driver lands" -- reset --recurse-submodules HEAD
  }
)

# mode=effect — `--recurse-submodules=yes` is an explicit form
# accepted by `parse_update_recurse_submodules_arg` (the worktree
# updater used by reset accepts only yes/no/true/false/1/0; the
# on-demand mode is a fetch-only spelling).
# hash=sha1-only
title "gix reset --recurse-submodules=yes HEAD"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until reset driver lands" -- reset --recurse-submodules=yes HEAD
  }
)

# mode=effect — `--no-recurse-submodules` disables the recurse path.
# hash=sha1-only
title "gix reset --no-recurse-submodules HEAD"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until reset driver lands" -- reset --no-recurse-submodules HEAD
  }
)

# --- patch / interactive ----------------------------------------------

# mode=effect — `--patch` triggers the interactive add-p replay at
# vendor/git/builtin/reset.c:436..442. With no TTY (test fixture
# context) git's interactive prompt aborts immediately. gix's
# placeholder accepts the flag and exits 0.
# hash=sha1-only
title "gix reset --patch"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until reset driver lands" -- reset --patch
  }
)

# mode=effect — short alias for `--patch`.
# hash=sha1-only
title "gix reset -p"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until reset driver lands" -- reset -p
  }
)

# mode=effect — `--auto-advance` only meaningful under `--patch`. Per
# vendor/git/builtin/reset.c:374. system git 2.47.3 predates the
# auto-advance / unified / inter-hunk-context additions to reset's
# parse-options table; vendor/git v2.54.0 has them. Row reactivates
# when CI git catches up with vendor/git.
# hash=sha1-only
title "gix reset --auto-advance --patch"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    shortcoming "system git 2.47.3 lacks --auto-advance; vendor/git v2.54.0 has it"
  }
)

# mode=effect — `--no-auto-advance --patch` errors at
# vendor/git/builtin/reset.c:449 when not paired with `--patch`,
# because PARSE_OPT_NONEG is unset on auto-advance. Same version
# skew as `--auto-advance`.
# hash=sha1-only
title "gix reset --no-auto-advance --patch"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    shortcoming "system git 2.47.3 lacks --auto-advance; vendor/git v2.54.0 has it"
  }
)

# mode=effect — `-U <n>` / `--unified=<n>` requires `--patch` per
# vendor/git/builtin/reset.c:444. Same version skew as
# `--auto-advance` (the OPT_DIFF_UNIFIED line lands in v2.54.0).
# hash=sha1-only
title "gix reset --unified=3 --patch"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    shortcoming "system git 2.47.3 lacks --unified for reset; vendor/git v2.54.0 has it"
  }
)

# mode=effect — short form of `--unified`. Same version skew.
# hash=sha1-only
title "gix reset -U 5 --patch"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    shortcoming "system git 2.47.3 lacks -U for reset; vendor/git v2.54.0 has it"
  }
)

# mode=effect — `--inter-hunk-context=<n>` requires `--patch` per
# vendor/git/builtin/reset.c:446. Same version skew as
# `--auto-advance`.
# hash=sha1-only
title "gix reset --inter-hunk-context=2 --patch"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    shortcoming "system git 2.47.3 lacks --inter-hunk-context for reset; vendor/git v2.54.0 has it"
  }
)

# --- intent-to-add ----------------------------------------------------

# mode=effect — `-N` records removed paths as intent-to-add markers.
# Only meaningful under `--mixed`; pairing with non-mixed dies 128 at
# vendor/git/builtin/reset.c:477.
# hash=sha1-only
title "gix reset -N"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until reset driver lands" -- reset -N
  }
)

# mode=effect — long form of `-N`.
# hash=sha1-only
title "gix reset --intent-to-add"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until reset driver lands" -- reset --intent-to-add
  }
)

# --- pathspec sources -------------------------------------------------

# mode=effect — `--pathspec-from-file=<file>` reads pathspec entries
# from a file (per vendor/git/builtin/reset.c:398..400). Conflicts
# with `--patch` per vendor/git/builtin/reset.c:392.
# hash=sha1-only
title "gix reset --pathspec-from-file=spec.txt"
only_for_hash sha1-only && (small-repo-in-sandbox
  echo a > spec.txt
  it "matches git behavior" && {
    compat_effect "deferred until reset driver lands" -- reset --pathspec-from-file=spec.txt
  }
)

# mode=effect — `--pathspec-file-nul` is only meaningful with
# `--pathspec-from-file`; pairing alone dies 128 at
# vendor/git/builtin/reset.c:401..403.
# hash=sha1-only
title "gix reset --pathspec-from-file=spec.txt --pathspec-file-nul"
only_for_hash sha1-only && (small-repo-in-sandbox
  printf 'a\0' > spec.txt
  it "matches git behavior" && {
    compat_effect "deferred until reset driver lands" -- reset --pathspec-from-file=spec.txt --pathspec-file-nul
  }
)

# mode=bytes — `--pathspec-file-nul` without `--pathspec-from-file`
# is an error (vendor/git/builtin/reset.c:401..403). gix's placeholder
# mirrors the verbatim wording.
# hash=sha1-only
title "gix reset --pathspec-file-nul"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    expect_parity bytes -- reset --pathspec-file-nul
  }
)

# --- pathspec positional ----------------------------------------------

# mode=effect — `git reset HEAD -- <path>` unstages a single file by
# path (the "opposite of git add" mode). Per
# vendor/git/Documentation/git-reset.adoc:82..89.
# hash=sha1-only
title "gix reset HEAD -- a"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until reset driver lands" -- reset HEAD -- a
  }
)

# mode=effect — `git reset HEAD a` (no `--`) is disambiguated by
# vendor/git/builtin/reset.c:247 `parse_args` — the second arg is
# checked against `repo_get_oid_treeish` and falls through to the
# pathspec branch.
# hash=sha1-only
title "gix reset HEAD a"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until reset driver lands" -- reset HEAD a
  }
)

# mode=effect — `git reset -- <path>` defaults the tree-ish to HEAD.
# hash=sha1-only
title "gix reset -- a"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until reset driver lands" -- reset -- a
  }
)

# --- precondition gates ----------------------------------------------

# mode=bytes — `--mixed` (default) in a bare repo dies 128 at
# vendor/git/builtin/reset.c:473 ("mixed reset is not allowed in a
# bare repository"). gix's placeholder mirrors the verbatim wording.
# hash=sha1-only
title "gix reset (in bare repo)"
only_for_hash sha1-only && (sandbox
  git-init-hash-aware --bare
  it "matches git behavior" && {
    expect_parity bytes -- reset
  }
)

# mode=bytes — `-N --hard` errors at vendor/git/builtin/reset.c:477
# ("the option '-N' requires '--mixed'"). gix's placeholder mirrors
# the verbatim wording.
# hash=sha1-only
title "gix reset -N --hard"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    expect_parity bytes -- reset -N --hard
  }
)

# mode=bytes — `--patch --hard` errors at
# vendor/git/builtin/reset.c:438 ("options '--patch' and
# '--{hard,mixed,soft}' cannot be used together"). gix's placeholder
# mirrors the verbatim wording.
# hash=sha1-only
title "gix reset --patch --hard"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    expect_parity bytes -- reset --patch --hard
  }
)

# mode=bytes — `--patch --pathspec-from-file=spec.txt` errors at
# vendor/git/builtin/reset.c:392..393 ("options '--pathspec-from-file'
# and '--patch' cannot be used together"). gix's placeholder mirrors
# the verbatim wording.
# hash=sha1-only
title "gix reset --patch --pathspec-from-file=spec.txt"
only_for_hash sha1-only && (small-repo-in-sandbox
  echo a > spec.txt
  it "matches git behavior" && {
    expect_parity bytes -- reset --patch --pathspec-from-file=spec.txt
  }
)

# mode=bytes — `--pathspec-from-file=spec.txt -- a` errors at
# vendor/git/builtin/reset.c:395..396 ("'--pathspec-from-file' and
# pathspec arguments cannot be used together"). gix's placeholder
# mirrors the verbatim wording.
# hash=sha1-only
title "gix reset --pathspec-from-file=spec.txt -- a"
only_for_hash sha1-only && (small-repo-in-sandbox
  echo a > spec.txt
  it "matches git behavior" && {
    expect_parity bytes -- reset --pathspec-from-file=spec.txt -- a
  }
)

# End-of-file sentinel: when every row is `only_for_hash sha1-only` and the
# active hash is sha256, the last statement returns 1 (skip), which would
# propagate out of `source` and trip `set -e` in tests/parity.sh. A trailing
# `:` normalizes the exit code so a fully-skipped file still returns 0.
:
