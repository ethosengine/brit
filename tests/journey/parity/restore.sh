#!/usr/bin/env bash
# Must be sourced into tests/parity.sh or tests/journey.sh — see tests/parity.sh.
#
# Parity scaffold for `git restore` ↔ `gix restore`.
#
# Iteration 1: scaffold only — TODO placeholders, real assertions land row-
# by-row in iteration 2..N. Each `it` block today is a `:` no-op; closing a
# row means swapping `:` for the real `expect_parity` / `compat_effect` /
# `shortcoming` invocation and dropping the leading `# TODO:` marker.
#
# git's `restore` builtin shares its dispatcher with `checkout` and `switch`;
# entry point is `vendor/git/builtin/checkout.c::cmd_restore` at
# `vendor/git/builtin/checkout.c:2128..2162`. The flag surface is the union
# of the local `restore_options` table (:2135..2145), `add_common_options`
# (:1712..1730), and `add_checkout_path_options` (:1756..1778):
#
#   restore_options:
#     -s <tree-ish> / --source=<tree-ish>   (string)
#     -S / --staged                          (bool)
#     -W / --worktree                        (bool, default on)
#     --ignore-unmerged                      (bool)
#     --overlay / --no-overlay               (bool, default off; --no-overlay
#                                              documented at git-restore.adoc:122..127)
#
#   add_common_options:
#     -q / --quiet
#     --recurse-submodules[=mode]            (OPTARG)
#     --progress / --no-progress
#     -m / --merge
#     --conflict <style>                     (merge | diff3 | zdiff3)
#
#   add_checkout_path_options:
#     --ours                                 (alias for `-2`; PARSE_OPT_NONEG)
#     --theirs                               (alias for `-3`; PARSE_OPT_NONEG)
#     -p / --patch
#     -U <n> / --unified=<n>                 (OPT_DIFF_UNIFIED)
#     --inter-hunk-context=<n>               (OPT_DIFF_INTERHUNK_CONTEXT)
#     --ignore-skip-worktree-bits
#     --pathspec-from-file=<file>
#     --pathspec-file-nul
#
# Synopsis (vendor/git/Documentation/git-restore.adoc:8..14):
#   git restore [<options>] [--source=<tree>] [--staged] [--worktree] [--] <pathspec>...
#   git restore [<options>] [--source=<tree>] [--staged] [--worktree] --pathspec-from-file=<file> [--pathspec-file-nul]
#   git restore (-p|--patch) [<options>] [--source=<tree>] [--staged] [--worktree] [--] [<pathspec>...]
#
# Verdict modes (set per row in iterations 2..N):
#   bytes  — meta + die paths (usage banner emitted by `--bogus-flag`,
#            "fatal: you must specify path(s) to restore",
#            "fatal: '--pathspec-from-file' and pathspec arguments cannot
#            be used together", outside-of-repo).
#   effect — UX-level flag rows whose semantics are not yet implemented in
#            gix's restore placeholder.
#
# `git restore` mutates the working tree (and/or the index when --staged is
# set). Mutating rows in iterations 2..N use `expect_parity_reset` so each
# binary starts from a fresh per-binary fixture (mirrors rm.sh's
# `_rm-fixture` pattern); read-only meta + die rows can stay on plain
# `expect_parity`.
#
# Coverage on gix's current Clap surface (src/plumbing/options/restore.rs):
#   gix restore [OPTIONS] [ARGS]... [-- <paths>...]
# Today the porcelain driver is a placeholder
# (gitoxide-core/src/repository/restore.rs::porcelain) that wires
#   * the empty-pathspec die ("fatal: you must specify path(s) to restore"
#     + exit 128, mirroring checkout_main's `accept_pathspec=1,
#     empty_pathspec_ok=0` gate at vendor/git/builtin/checkout.c:2148..2155),
#   * the pathspec-source mutual-exclusion gates from
#     `parse_opt_pathspec_from_file` and `parse_opt_pathspec_file_nul`
#     (`--pathspec-from-file` ⇄ positional pathspec; `--pathspec-file-nul`
#     ⇒ `--pathspec-from-file`),
# and emits a stub note + exits 0 on every other path. Closing this command
# requires implementing the restore driver in
# gitoxide-core/src/repository/restore.rs:
#   * --source=<tree-ish> tree-ish parsing + tree walk.
#   * --staged / --worktree index/worktree update split (default --worktree).
#   * --ignore-unmerged unmerged-skip path.
#   * --overlay / --no-overlay tracked-file removal toggle (default
#     no-overlay: remove tracked files not present in --source=<tree>).
#   * --ours / --theirs stage-pick for unmerged paths.
#   * --merge / --conflict three-way merge of working-tree changes.
#   * --patch (with -U / --inter-hunk-context) interactive hunk select.
#   * --ignore-skip-worktree-bits sparse-checkout pathspec override.
#   * --pathspec-from-file / --pathspec-file-nul pathspec source parser.
#   * --recurse-submodules submodule tree update.
#   * --quiet / --progress / --no-progress feedback shaping.
#
# Hash coverage: `dual` rows never open a repo (--help, outside-of-repo).
# Every row that opens a repository is `sha1-only` because gix-config
# rejects `extensions.objectFormat=sha256`
# (gix/src/config/tree/sections/extensions.rs try_into_object_format,
# sha1-only validator), blocking every sha256 fixture at open.
#
# parity-defaults:
#   hash=sha1-only "gix cannot load sha256 repos: extensions.objectFormat=sha256 rejected (gix/src/config/tree/sections/extensions.rs)"
#   mode=effect

# --- meta / help --------------------------------------------------------

# mode=effect — clap --help short-circuits before repo load, exits 0.
# git's --help delegates to `man git-restore` (exit 0); gix returns clap's
# auto-generated help. Message text diverges; only the exit-code match is
# asserted.
# hash=dual
title "gix restore --help"
only_for_hash dual && (sandbox
  it "matches git behavior" && {
    expect_parity effect -- restore --help
  }
)

# mode=effect — unknown flag: git exits 129 (usage_msg_opt in
# parse-options.c). gix's Clap layer maps UnknownArgument to 129 via
# src/plumbing/main.rs. Tested inside a repo because git outside a repo
# dies 128 before reaching arg-parse, while clap in gix always runs first.
title "gix restore --bogus-flag"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    expect_parity effect -- restore --bogus-flag
  }
)

# mode=bytes — outside any repo: git dies 128 with "fatal: not a git
# repository (or any of the parent directories): .git". gix's plumbing
# repository() closure already remaps gix_discover::upwards::Error::
# NoGitRepository* variants to git's exact wording + exit 128. The
# positional `a` is needed to bypass clap's required-arg gate before
# the shared repo-open glue runs.
# hash=dual
title "gix restore (outside a repository)"
only_for_hash dual && (sandbox
  it "matches git behavior" && {
    expect_parity bytes -- restore a
  }
)

# --- argument-count gates ----------------------------------------------

# mode=bytes — bare `git restore` (no positional, no
# `--pathspec-from-file`) dies 128 with "fatal: you must specify path(s)
# to restore" — emitted by checkout_main's empty-pathspec gate when
# `accept_pathspec=1` (vendor/git/builtin/checkout.c:2149) and
# `empty_pathspec_ok=0` (:2150). gix porcelain placeholder mirrors the
# wording verbatim.
title "gix restore (no positional)"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    expect_parity bytes -- restore
  }
)

# mode=effect — missing pathspec: git emits `error: pathspec '<x>' did
# not match any file(s) known to git` + exit 1 (note: `error()` not
# `die()`, exit 1 not 128 — distinguishes restore from rm/add, which
# both `die()` with exit 128). The error fires from the checkout
# pathspec walker (part of the deferred restore driver), so the
# placeholder cannot match the exit code today.
title "gix restore missing-file"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    shortcoming "deferred until restore driver implements pathspec walker (exit-code 1 mismatch)"
  }
)

# --- happy-path single-pathspec ----------------------------------------

# mode=effect — `git restore a` restores `a` from the index (default
# --worktree only). On a clean fixture `a` is at the index version, so
# both binaries exit 0 (gix via the stub-note path). Bytes parity
# deferred until the restore driver lands.
title "gix restore <pathspec>"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until restore driver lands" -- restore a
  }
)

# mode=effect — explicit pathspec terminator. `git restore -- a` is the
# canonical synopsis form (vendor/git/Documentation/git-restore.adoc:11).
title "gix restore -- <pathspec>"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until restore driver lands" -- restore -- a
  }
)

# --- source ------------------------------------------------------------

# mode=effect — `-s <tree-ish>` short form. Mirrors restore_options[0]
# `OPT_STRING('s', "source", ...)` at vendor/git/builtin/checkout.c:2136.
title "gix restore -s"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until restore driver lands" -- restore -s HEAD a
  }
)

# mode=effect — `--source=<tree-ish>` long form.
title "gix restore --source"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until restore driver lands" -- restore --source HEAD a
  }
)

# mode=effect — `--source=HEAD` (the canonical default for --staged).
# Exercises the rev-parse path on the porcelain driver once it lands.
title "gix restore --source=HEAD"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until restore driver lands" -- restore --source=HEAD a
  }
)

# --- staged / worktree -------------------------------------------------

# mode=effect — `-S` short for `--staged`. Restores the index from HEAD
# by default. Mirrors restore_options[1] OPT_BOOL at
# vendor/git/builtin/checkout.c:2138.
title "gix restore -S"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until restore driver lands" -- restore -S a
  }
)

# mode=effect — `--staged` long form.
title "gix restore --staged"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until restore driver lands" -- restore --staged a
  }
)

# mode=effect — `-W` short for `--worktree` (the default location).
# Mirrors restore_options[2] OPT_BOOL at
# vendor/git/builtin/checkout.c:2140.
title "gix restore -W"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until restore driver lands" -- restore -W a
  }
)

# mode=effect — `--worktree` long form.
title "gix restore --worktree"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until restore driver lands" -- restore --worktree a
  }
)

# mode=effect — `--staged --worktree` together — restore both index
# and worktree per vendor/git/Documentation/git-restore.adoc:60..61.
title "gix restore --staged --worktree"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until restore driver lands" -- restore --staged --worktree a
  }
)

# --- ignore-unmerged ---------------------------------------------------

# mode=effect — `--ignore-unmerged` — restore_options[3] OPT_BOOL at
# vendor/git/builtin/checkout.c:2142. On a clean fixture there are no
# unmerged entries; both 0.
title "gix restore --ignore-unmerged"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until restore driver lands" -- restore --ignore-unmerged a
  }
)

# --- overlay / no-overlay ----------------------------------------------

# mode=effect — `--overlay` — restore_options[4] OPT_BOOL at
# vendor/git/builtin/checkout.c:2144. Default is no-overlay; explicit
# overlay is rare.
title "gix restore --overlay"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until restore driver lands" -- restore --overlay a
  }
)

# mode=effect — `--no-overlay` — documented inverse at
# vendor/git/Documentation/git-restore.adoc:122..127. This is the
# default; explicit pass is a no-op on a clean fixture.
title "gix restore --no-overlay"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until restore driver lands" -- restore --no-overlay a
  }
)

# --- quiet / progress -------------------------------------------------

# mode=effect — `-q` / `--quiet` — add_common_options OPT__QUIET at
# vendor/git/builtin/checkout.c:1716. Both 0.
title "gix restore -q"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until restore driver lands" -- restore -q a
  }
)

# mode=effect — long form of `-q`.
title "gix restore --quiet"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until restore driver lands" -- restore --quiet a
  }
)

# mode=effect — `--progress` — add_common_options OPT_BOOL at
# vendor/git/builtin/checkout.c:1720. Force progress reporting on
# non-TTY.
title "gix restore --progress"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until restore driver lands" -- restore --progress a
  }
)

# mode=effect — `--no-progress` disables progress reporting.
title "gix restore --no-progress"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until restore driver lands" -- restore --no-progress a
  }
)

# --- merge / conflict -------------------------------------------------

# mode=effect — `-m` short for `--merge` — add_common_options OPT_BOOL
# at vendor/git/builtin/checkout.c:1721. Recreate conflicted merge in
# unmerged paths. On a clean fixture there are no conflicts; both 0.
title "gix restore -m"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until restore driver lands" -- restore -m a
  }
)

# mode=effect — `--merge` long form.
title "gix restore --merge"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until restore driver lands" -- restore --merge a
  }
)

# mode=effect — `--conflict=merge` style. add_common_options
# OPT_CALLBACK at vendor/git/builtin/checkout.c:1722..1724. On a clean
# fixture the merge degenerates to a no-op; both 0.
title "gix restore --conflict=merge"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until restore driver lands" -- restore --conflict=merge a
  }
)

# mode=effect — `--conflict=diff3` style.
title "gix restore --conflict=diff3"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until restore driver lands" -- restore --conflict=diff3 a
  }
)

# mode=effect — `--conflict=zdiff3` style.
title "gix restore --conflict=zdiff3"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until restore driver lands" -- restore --conflict=zdiff3 a
  }
)

# --- ours / theirs ----------------------------------------------------

# mode=effect — `--ours` — add_checkout_path_options
# `OPT_SET_INT_F('2', "ours", ..., 2, PARSE_OPT_NONEG)` at
# vendor/git/builtin/checkout.c:1760..1762. Default writeout_stage is
# 0; --ours sets it to stage #2. On a clean fixture there are no
# unmerged paths; both 0.
title "gix restore --ours"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until restore driver lands" -- restore --ours a
  }
)

# mode=effect — `--theirs` — `OPT_SET_INT_F('3', "theirs", ..., 3,
# PARSE_OPT_NONEG)` at vendor/git/builtin/checkout.c:1763..1765.
title "gix restore --theirs"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until restore driver lands" -- restore --theirs a
  }
)

# --- patch / unified / inter-hunk-context -----------------------------

# mode=effect — `-p` short for `--patch` — add_checkout_path_options
# OPT_BOOL at vendor/git/builtin/checkout.c:1766. Interactive hunk
# select. The test environment has no TTY so git's add_p_init reads
# nothing and returns 0; gix-placeholder also exits 0.
title "gix restore -p"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until restore driver lands" -- restore -p a
  }
)

# mode=effect — `--patch` long form.
title "gix restore --patch"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until restore driver lands" -- restore --patch a
  }
)

# mode=effect — `-U <n>` short for `--unified=<n>` — diff context.
# `OPT_DIFF_UNIFIED` was added to add_checkout_path_options after git
# 2.47.3 — vendor/git v2.54.0 has it but the test runtime's system
# git lacks it (rejects with `unknown switch 'U'` + exit 129). Defer
# as a hard system constraint until the test runtime upgrades.
# hash=sha1-only
title "gix restore -U --patch"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    shortcoming "system git 2.47.3 lacks -U for restore; vendor/git v2.54.0 has it"
  }
)

# mode=effect — long form of `-U`. Same version skew.
# hash=sha1-only
title "gix restore --unified --patch"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    shortcoming "system git 2.47.3 lacks --unified for restore; vendor/git v2.54.0 has it"
  }
)

# mode=effect — `--inter-hunk-context=<n>` — show context between diff
# hunks. `OPT_DIFF_INTERHUNK_CONTEXT` was added after git 2.47.3.
# Same version skew as `--unified`.
# hash=sha1-only
title "gix restore --inter-hunk-context --patch"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    shortcoming "system git 2.47.3 lacks --inter-hunk-context for restore; vendor/git v2.54.0 has it"
  }
)

# --- ignore-skip-worktree-bits ----------------------------------------

# mode=effect — `--ignore-skip-worktree-bits` — add_checkout_path_options
# OPT_BOOL at vendor/git/builtin/checkout.c:1769..1770. On a fixture
# without sparse checkout this is a no-op; both 0.
title "gix restore --ignore-skip-worktree-bits"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until restore driver lands" -- restore --ignore-skip-worktree-bits a
  }
)

# --- pathspec sources -------------------------------------------------

# mode=effect — `--pathspec-from-file=<file>` reads pathspecs from
# `<file>`. add_checkout_path_options OPT_PATHSPEC_FROM_FILE at
# vendor/git/builtin/checkout.c:1771.
title "gix restore --pathspec-from-file"
only_for_hash sha1-only && (small-repo-in-sandbox
  echo a > spec.txt
  it "matches git behavior" && {
    compat_effect "deferred until restore driver lands" -- restore --pathspec-from-file=spec.txt
  }
)

# mode=effect — `--pathspec-from-file=<file> --pathspec-file-nul` —
# NUL-separated. add_checkout_path_options OPT_PATHSPEC_FILE_NUL at
# vendor/git/builtin/checkout.c:1772.
title "gix restore --pathspec-from-file --pathspec-file-nul"
only_for_hash sha1-only && (small-repo-in-sandbox
  printf 'a' > spec.txt
  it "matches git behavior" && {
    compat_effect "deferred until restore driver lands" -- restore --pathspec-from-file=spec.txt --pathspec-file-nul
  }
)

# --- recurse-submodules -----------------------------------------------

# mode=effect — `--recurse-submodules` — add_common_options
# `OPT_CALLBACK_F(0, "recurse-submodules", ..., PARSE_OPT_OPTARG, ...)`
# at vendor/git/builtin/checkout.c:1717..1719. On a fixture without
# submodules, no-op; both 0.
title "gix restore --recurse-submodules"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until restore driver lands" -- restore --recurse-submodules a
  }
)

# mode=effect — `--no-recurse-submodules` disables submodule recursion.
title "gix restore --no-recurse-submodules"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until restore driver lands" -- restore --no-recurse-submodules a
  }
)

# --- precondition gates ----------------------------------------------

# mode=bytes — `--pathspec-from-file=<file> -- <pathspec>` — the parse-
# options gate dies with "fatal: '--pathspec-from-file' and pathspec
# arguments cannot be used together" + exit 128. Bytes-perfect parity
# (gix porcelain mirrors verbatim).
title "gix restore --pathspec-from-file -- a"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    expect_parity bytes -- restore --pathspec-from-file=spec.txt -- a
  }
)

# mode=bytes — `--pathspec-file-nul` without `--pathspec-from-file` dies
# 128 with "fatal: the option '--pathspec-file-nul' requires
# '--pathspec-from-file'". Bytes-perfect parity.
title "gix restore --pathspec-file-nul (no --pathspec-from-file)"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    expect_parity bytes -- restore --pathspec-file-nul
  }
)

# End-of-file sentinel: when every row is `only_for_hash sha1-only` and the
# active hash is sha256, the last statement returns 1 (skip), which would
# propagate out of `source` and trip `set -e` in tests/parity.sh. A trailing
# `:` normalizes the exit code so a fully-skipped file still returns 0.
:
