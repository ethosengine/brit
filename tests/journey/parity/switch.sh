#!/usr/bin/env bash
# Must be sourced into tests/parity.sh or tests/journey.sh — see tests/parity.sh.
#
# Parity scaffold for `git switch` ↔ `gix switch`.
#
# Iteration 1: scaffold only — TODO placeholders, real assertions land row-
# by-row in iteration 2..N. Each `it` block today is a `:` no-op; closing a
# row means swapping `:` for the real `expect_parity` / `compat_effect` /
# `shortcoming` invocation and dropping the leading `# TODO:` marker.
#
# git's `switch` builtin shares its dispatcher with `checkout`; entry point
# is `vendor/git/builtin/checkout.c::cmd_switch` at
# `vendor/git/builtin/checkout.c:2089..2126`. The flag surface is the union
# of the local `switch_options` table (`vendor/git/builtin/checkout.c:2096..
# 2105`), `add_common_options` (`vendor/git/builtin/checkout.c:1712..1730`),
# and `add_common_switch_branch_options`
# (`vendor/git/builtin/checkout.c:1732..1754`):
#
#   switch_options:
#     -c / --create <branch>            (string)
#     -C / --force-create <branch>      (string)
#     --guess / --no-guess              (bool, default on)
#     --discard-changes                 (bool)
#
#   add_common_options:
#     -q / --quiet
#     --recurse-submodules[=mode]       (OPTARG)
#     --progress / --no-progress
#     -m / --merge
#     --conflict <style>                (merge | diff3 | zdiff3)
#
#   add_common_switch_branch_options:
#     -d / --detach
#     -t / --track[=(direct|inherit)]   (OPTARG)
#     -f / --force                      (alias for --discard-changes per
#                                        vendor/git/Documentation/git-switch.adoc:113..115)
#     --orphan <new-branch>             (string)
#     --overwrite-ignore / --no-overwrite-ignore
#     --ignore-other-worktrees
#
# Synopsis (vendor/git/Documentation/git-switch.adoc:8..14):
#   git switch [<options>] [--no-guess] <branch>
#   git switch [<options>] --detach [<start-point>]
#   git switch [<options>] (-c|-C) <new-branch> [<start-point>]
#   git switch [<options>] --orphan <new-branch>
#
# Verdict modes (set per row in iterations 2..N):
#   bytes  — meta + die paths (usage banner emitted by `--bogus-flag`,
#            "fatal: missing branch or commit argument" / "fatal: invalid
#            reference: <name>" / outside-of-repo).
#   effect — UX-level flag rows whose semantics are not yet implemented in
#            gix's switch placeholder.
#
# `git switch` mutates HEAD and (typically) the working tree. Mutating rows
# in iterations 2..N will use `expect_parity_reset` so each binary starts
# from a fresh per-binary fixture (mirrors mv.sh's `_mv-fixture` pattern);
# read-only meta rows can stay on plain `expect_parity`.
#
# Coverage on gix's current Clap surface (src/plumbing/options/switch.rs):
#   gix switch [OPTIONS] [ARGS]...
# Today the porcelain driver is a placeholder
# (gitoxide-core/src/repository/switch.rs::porcelain) that wires the
# no-target die ("fatal: missing branch or commit argument" + exit 128
# when there's no positional and no -c / -C / --orphan / --detach), and
# emits a stub note + exits 0 on every other path. The flag surface is
# clap-wired so `gix switch <flag> ...` does not trip UnknownArgument;
# every flag-bearing happy-path row therefore exits 0 with a stub note
# until the real switch driver lands. Closing this command requires
# implementing the switch driver in gitoxide-core/src/repository/switch.rs:
#   * Ref resolution for <branch> (positional) and <start-point>.
#   * --create / --force-create / --orphan branch creation.
#   * --detach HEAD-detach path.
#   * --merge / --conflict three-way merge of working-tree changes.
#   * --discard-changes / --force tree+index hard-reset.
#   * --track / --no-track / --guess tracking-config + DWIM-from-remote.
#   * --recurse-submodules submodule tree update.
#   * --quiet / --progress / --no-progress feedback shaping.
#   * --overwrite-ignore / --ignore-other-worktrees safety knobs.
#   * "Switched to a new branch '<name>'" / "Switched to branch '<name>'"
#     stdout/stderr emission.
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
# git's --help delegates to `man git-switch` (exit 0); gix returns clap's
# auto-generated help. Message text diverges; only the exit-code match is
# asserted.
# hash=dual
title "gix switch --help"
only_for_hash dual && (sandbox
  it "matches git behavior" && {
    expect_parity effect -- switch --help
  }
)

# mode=effect — unknown flag: git exits 129 (usage_msg_opt in
# parse-options.c). gix's Clap layer maps UnknownArgument to 129 via
# src/plumbing/main.rs. Tested inside a repo because git outside a repo
# dies 128 before reaching arg-parse, while clap in gix always runs first.
title "gix switch --bogus-flag"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    expect_parity effect -- switch --bogus-flag
  }
)

# mode=bytes — outside any repo: git dies 128 with "fatal: not a git
# repository (or any of the parent directories): .git". gix's plumbing
# repository() closure already remaps gix_discover::upwards::Error::
# NoGitRepository* variants to git's exact wording + exit 128. The
# positional `xyz` is needed to bypass clap's required-arg gate before
# the shared repo-open glue runs.
# hash=dual
title "gix switch (outside a repository)"
only_for_hash dual && (sandbox
  it "matches git behavior" && {
    expect_parity bytes -- switch xyz
  }
)

# --- argument-count gates ----------------------------------------------

# mode=bytes — bare `git switch` (no positional, no -c/-C/--orphan/--detach)
# dies 128 with "fatal: missing branch or commit argument" — emitted by
# checkout_main's argument-count gate when accept_pathspec=0. gix's
# porcelain placeholder mirrors the wording verbatim.
title "gix switch (no positional)"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    expect_parity bytes -- switch
  }
)

# mode=bytes — nonexistent ref: git dies 128 with
# "fatal: invalid reference: <name>". The die fires from
# vendor/git/builtin/checkout.c:1463..1465 because switch sets
# accept_pathspec=0, so has_dash_dash is forced to 1 at
# vendor/git/builtin/checkout.c:1399..1403, which routes the dwim-fail
# branch into die() instead of falling through. gix's porcelain stub
# wires `try_find_reference()` and emits the verbatim error on Ok(None)
# / Err.
title "gix switch nosuchbranch"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    expect_parity bytes -- switch nosuchbranch
  }
)

# --- happy-path ref switching -----------------------------------------

# TODO: switch to an existing local branch on a fixture with at least two
# branches. Mutating: needs expect_parity_reset.
title "gix switch <existing-branch>"
only_for_hash sha1-only && (sandbox
  it "matches git behavior" && {
    :
  }
)

# TODO: `git switch -` switches to @{-1} (the previous branch). Mutating.
title "gix switch -"
only_for_hash sha1-only && (sandbox
  it "matches git behavior" && {
    :
  }
)

# --- create / force-create --------------------------------------------

# TODO: -c <new-branch>: branch creation + switch.
title "gix switch -c"
only_for_hash sha1-only && (sandbox
  it "matches git behavior" && {
    :
  }
)

# TODO: --create <new-branch>: long form of -c.
title "gix switch --create"
only_for_hash sha1-only && (sandbox
  it "matches git behavior" && {
    :
  }
)

# TODO: -C <existing>: force create / reset.
title "gix switch -C"
only_for_hash sha1-only && (sandbox
  it "matches git behavior" && {
    :
  }
)

# TODO: --force-create <existing>: long form of -C.
title "gix switch --force-create"
only_for_hash sha1-only && (sandbox
  it "matches git behavior" && {
    :
  }
)

# --- detach -----------------------------------------------------------

# TODO: -d HEAD: detach at HEAD.
title "gix switch -d"
only_for_hash sha1-only && (sandbox
  it "matches git behavior" && {
    :
  }
)

# TODO: --detach HEAD: long form of -d.
title "gix switch --detach"
only_for_hash sha1-only && (sandbox
  it "matches git behavior" && {
    :
  }
)

# --- guess ------------------------------------------------------------

# TODO: --guess: enable DWIM-from-remote (default; explicit pass).
title "gix switch --guess"
only_for_hash sha1-only && (sandbox
  it "matches git behavior" && {
    :
  }
)

# TODO: --no-guess: disable DWIM-from-remote.
title "gix switch --no-guess"
only_for_hash sha1-only && (sandbox
  it "matches git behavior" && {
    :
  }
)

# --- discard-changes / force / merge ---------------------------------

# TODO: --discard-changes: hard-reset over local mods.
title "gix switch --discard-changes"
only_for_hash sha1-only && (sandbox
  it "matches git behavior" && {
    :
  }
)

# TODO: -f: alias for --discard-changes (per docs:113..115).
title "gix switch -f"
only_for_hash sha1-only && (sandbox
  it "matches git behavior" && {
    :
  }
)

# TODO: --force: long form of -f.
title "gix switch --force"
only_for_hash sha1-only && (sandbox
  it "matches git behavior" && {
    :
  }
)

# TODO: -m: 3-way merge with the new branch.
title "gix switch -m"
only_for_hash sha1-only && (sandbox
  it "matches git behavior" && {
    :
  }
)

# TODO: --merge: long form of -m.
title "gix switch --merge"
only_for_hash sha1-only && (sandbox
  it "matches git behavior" && {
    :
  }
)

# TODO: --conflict=merge (default style).
title "gix switch --conflict=merge"
only_for_hash sha1-only && (sandbox
  it "matches git behavior" && {
    :
  }
)

# TODO: --conflict=diff3.
title "gix switch --conflict=diff3"
only_for_hash sha1-only && (sandbox
  it "matches git behavior" && {
    :
  }
)

# TODO: --conflict=zdiff3.
title "gix switch --conflict=zdiff3"
only_for_hash sha1-only && (sandbox
  it "matches git behavior" && {
    :
  }
)

# --- quiet / progress -------------------------------------------------

# TODO: -q: suppress progress.
title "gix switch -q"
only_for_hash sha1-only && (sandbox
  it "matches git behavior" && {
    :
  }
)

# TODO: --quiet: long form of -q.
title "gix switch --quiet"
only_for_hash sha1-only && (sandbox
  it "matches git behavior" && {
    :
  }
)

# TODO: --progress: force progress.
title "gix switch --progress"
only_for_hash sha1-only && (sandbox
  it "matches git behavior" && {
    :
  }
)

# TODO: --no-progress: suppress progress.
title "gix switch --no-progress"
only_for_hash sha1-only && (sandbox
  it "matches git behavior" && {
    :
  }
)

# --- track / no-track ------------------------------------------------

# TODO: -t: tracking-config (implies -c).
title "gix switch -t"
only_for_hash sha1-only && (sandbox
  it "matches git behavior" && {
    :
  }
)

# TODO: --track: long form of -t.
title "gix switch --track"
only_for_hash sha1-only && (sandbox
  it "matches git behavior" && {
    :
  }
)

# TODO: --track=direct.
title "gix switch --track=direct"
only_for_hash sha1-only && (sandbox
  it "matches git behavior" && {
    :
  }
)

# TODO: --track=inherit.
title "gix switch --track=inherit"
only_for_hash sha1-only && (sandbox
  it "matches git behavior" && {
    :
  }
)

# TODO: --no-track.
title "gix switch --no-track"
only_for_hash sha1-only && (sandbox
  it "matches git behavior" && {
    :
  }
)

# --- orphan ---------------------------------------------------------

# TODO: --orphan <new-branch>: create unborn branch.
title "gix switch --orphan"
only_for_hash sha1-only && (sandbox
  it "matches git behavior" && {
    :
  }
)

# --- overwrite-ignore / ignore-other-worktrees -----------------------

# TODO: --overwrite-ignore: clobber ignored files (default).
title "gix switch --overwrite-ignore"
only_for_hash sha1-only && (sandbox
  it "matches git behavior" && {
    :
  }
)

# TODO: --no-overwrite-ignore: preserve ignored files.
title "gix switch --no-overwrite-ignore"
only_for_hash sha1-only && (sandbox
  it "matches git behavior" && {
    :
  }
)

# TODO: --ignore-other-worktrees.
title "gix switch --ignore-other-worktrees"
only_for_hash sha1-only && (sandbox
  it "matches git behavior" && {
    :
  }
)

# --- recurse-submodules --------------------------------------------

# TODO: --recurse-submodules.
title "gix switch --recurse-submodules"
only_for_hash sha1-only && (sandbox
  it "matches git behavior" && {
    :
  }
)

# TODO: --no-recurse-submodules.
title "gix switch --no-recurse-submodules"
only_for_hash sha1-only && (sandbox
  it "matches git behavior" && {
    :
  }
)

# End-of-file sentinel: when every row is `only_for_hash sha1-only` and the
# active hash is sha256, the last statement returns 1 (skip), which would
# propagate out of `source` and trip `set -e` in tests/parity.sh. A trailing
# `:` normalizes the exit code so a fully-skipped file still returns 0.
:
