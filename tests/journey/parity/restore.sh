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

# TODO: clap --help short-circuits before repo load, exits 0. git's --help
# delegates to `man git-restore`. Close as `expect_parity effect`.
# hash=dual
title "gix restore --help"
only_for_hash dual && (sandbox
  it "matches git behavior" && {
    :
  }
)

# TODO: unknown flag: git exits 129 (usage_msg_opt). gix's Clap layer maps
# UnknownArgument to 129. Close as `expect_parity effect` against
# small-repo-in-sandbox.
title "gix restore --bogus-flag"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    :
  }
)

# TODO: outside any repo: git dies 128 with "fatal: not a git repository".
# gix's plumbing repository() closure already remaps. The positional `a`
# is needed to bypass clap's required-arg gate before the shared repo-open
# glue runs. Close as `expect_parity bytes`.
# hash=dual
title "gix restore (outside a repository)"
only_for_hash dual && (sandbox
  it "matches git behavior" && {
    :
  }
)

# --- argument-count gates ----------------------------------------------

# TODO: bare `git restore` (no positional, no `--pathspec-from-file`) dies
# 128 with "fatal: you must specify path(s) to restore" — emitted by
# checkout_main's empty-pathspec gate when `accept_pathspec=1` and
# `empty_pathspec_ok=0`. gix porcelain placeholder mirrors verbatim.
# Close as `expect_parity bytes`.
title "gix restore (no positional)"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    :
  }
)

# TODO: missing pathspec — git dies 128 with "fatal: pathspec '<x>' did not
# match any files" once the pathspec walker fires. gix placeholder skips
# the walker (deferred until restore driver lands), so this is a
# `compat_effect` row today. Close as
# `compat_effect "deferred until restore driver lands"`.
title "gix restore missing-file"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    :
  }
)

# --- happy-path single-pathspec ----------------------------------------

# TODO: `git restore a` restores `a` from the index (default --worktree
# only). gix-placeholder accepts the pathspec and exits 0 on the stub.
# Close as `compat_effect "deferred until restore driver lands"` against
# `expect_parity_reset _restore-fixture` so the second binary sees the
# pre-mutation fixture.
title "gix restore <pathspec>"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    :
  }
)

# TODO: explicit pathspec terminator — `git restore -- a` is the canonical
# form of the previous row.
title "gix restore -- <pathspec>"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    :
  }
)

# --- source ------------------------------------------------------------

# TODO: `-s <tree-ish>` short form. Mirrors restore_options[0] OPT_STRING.
title "gix restore -s"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    :
  }
)

# TODO: `--source=<tree-ish>` long form.
title "gix restore --source"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    :
  }
)

# TODO: `--source=HEAD` (the canonical default for --staged) — exercises
# the rev-parse path on the porcelain driver once it lands.
title "gix restore --source=HEAD"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    :
  }
)

# --- staged / worktree -------------------------------------------------

# TODO: `-S` short for `--staged`. Restores the index from HEAD by default.
title "gix restore -S"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    :
  }
)

# TODO: `--staged` long form.
title "gix restore --staged"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    :
  }
)

# TODO: `-W` short for `--worktree` (the default location).
title "gix restore -W"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    :
  }
)

# TODO: `--worktree` long form.
title "gix restore --worktree"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    :
  }
)

# TODO: `--staged --worktree` together — restore both index and worktree.
title "gix restore --staged --worktree"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    :
  }
)

# --- ignore-unmerged ---------------------------------------------------

# TODO: `--ignore-unmerged` — restore_options[3] OPT_BOOL. On a clean
# fixture there are no unmerged entries; both 0.
title "gix restore --ignore-unmerged"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    :
  }
)

# --- overlay / no-overlay ----------------------------------------------

# TODO: `--overlay` — restore_options[4] OPT_BOOL. Default is no-overlay;
# explicit overlay is rare.
title "gix restore --overlay"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    :
  }
)

# TODO: `--no-overlay` — documented inverse at git-restore.adoc:122..127.
# This is the default; explicit pass is a no-op on a clean fixture.
title "gix restore --no-overlay"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    :
  }
)

# --- quiet / progress -------------------------------------------------

# TODO: `-q` / `--quiet` — add_common_options OPT__QUIET. Both 0.
title "gix restore -q"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    :
  }
)

# TODO: long form of `-q`.
title "gix restore --quiet"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    :
  }
)

# TODO: `--progress` — force progress reporting on non-TTY.
title "gix restore --progress"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    :
  }
)

# TODO: `--no-progress` — disable progress reporting.
title "gix restore --no-progress"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    :
  }
)

# --- merge / conflict -------------------------------------------------

# TODO: `-m` short for `--merge` — recreate conflicted merge in unmerged
# paths. On a clean fixture there are no conflicts; both 0.
title "gix restore -m"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    :
  }
)

# TODO: `--merge` long form.
title "gix restore --merge"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    :
  }
)

# TODO: `--conflict=merge` style. On a clean fixture the merge degenerates
# to a no-op; both 0.
title "gix restore --conflict=merge"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    :
  }
)

# TODO: `--conflict=diff3` style.
title "gix restore --conflict=diff3"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    :
  }
)

# TODO: `--conflict=zdiff3` style.
title "gix restore --conflict=zdiff3"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    :
  }
)

# --- ours / theirs ----------------------------------------------------

# TODO: `--ours` — add_checkout_path_options OPT_SET_INT_F('2', "ours",
# ..., 2, PARSE_OPT_NONEG). Default writeout_stage is 0; --ours sets it to
# stage #2. On a clean fixture there are no unmerged paths; both 0.
title "gix restore --ours"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    :
  }
)

# TODO: `--theirs` — OPT_SET_INT_F('3', "theirs", ..., 3, PARSE_OPT_NONEG).
title "gix restore --theirs"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    :
  }
)

# --- patch / unified / inter-hunk-context -----------------------------

# TODO: `-p` short for `--patch` — interactive hunk select. Closes as
# `compat_effect` because gix has no interactive driver yet (and the test
# environment has no TTY).
title "gix restore -p"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    :
  }
)

# TODO: `--patch` long form.
title "gix restore --patch"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    :
  }
)

# TODO: `-U <n>` short for `--unified=<n>` — diff context. Without
# `--patch` git emits a value-still-required hint; with `--patch` it
# accepts. Test under `--patch` to keep parity.
title "gix restore -U --patch"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    :
  }
)

# TODO: `--unified=<n>` long form.
title "gix restore --unified --patch"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    :
  }
)

# TODO: `--inter-hunk-context=<n>` — show context between diff hunks.
title "gix restore --inter-hunk-context --patch"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    :
  }
)

# --- ignore-skip-worktree-bits ----------------------------------------

# TODO: `--ignore-skip-worktree-bits` — OPT_BOOL. On a fixture without
# sparse checkout this is a no-op; both 0.
title "gix restore --ignore-skip-worktree-bits"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    :
  }
)

# --- pathspec sources -------------------------------------------------

# TODO: `--pathspec-from-file=<file>` — read pathspecs from `<file>`.
title "gix restore --pathspec-from-file"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    :
  }
)

# TODO: `--pathspec-from-file=<file> --pathspec-file-nul` — NUL-separated.
title "gix restore --pathspec-from-file --pathspec-file-nul"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    :
  }
)

# --- recurse-submodules -----------------------------------------------

# TODO: `--recurse-submodules` — add_common_options OPT_CALLBACK_F
# PARSE_OPT_OPTARG. On a fixture without submodules, no-op; both 0.
title "gix restore --recurse-submodules"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    :
  }
)

# TODO: `--no-recurse-submodules` disables submodule recursion.
title "gix restore --no-recurse-submodules"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    :
  }
)

# --- precondition gates ----------------------------------------------

# TODO: `--pathspec-from-file=spec.txt -- a` — the parse-options gate dies
# with "fatal: '--pathspec-from-file' and pathspec arguments cannot be
# used together" + exit 128. Bytes-perfect parity (gix porcelain mirrors
# verbatim).
title "gix restore --pathspec-from-file -- a"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    :
  }
)

# TODO: `--pathspec-file-nul` without `--pathspec-from-file` dies 128 with
# "fatal: the option '--pathspec-file-nul' requires '--pathspec-from-file'".
title "gix restore --pathspec-file-nul (no --pathspec-from-file)"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    :
  }
)

# End-of-file sentinel: when every row is `only_for_hash sha1-only` and the
# active hash is sha256, the last statement returns 1 (skip), which would
# propagate out of `source` and trip `set -e` in tests/parity.sh. A trailing
# `:` normalizes the exit code so a fully-skipped file still returns 0.
:
