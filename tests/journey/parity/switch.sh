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

# mode=effect — switch to an existing local branch. git's first run moves
# HEAD to feat; gix's back-to-back run still finds feat (try_find_reference
# succeeds) and falls through to the placeholder stub. Both exit 0. Bytes
# parity (the "Switched to branch 'feat'" stderr emission) is deferred
# until the real switch driver lands.
title "gix switch <existing-branch>"
only_for_hash sha1-only && (small-repo-in-sandbox
  git branch feat
  it "matches git behavior" && {
    compat_effect "deferred until switch driver lands" -- switch feat
  }
)

# mode=effect — `git switch -` resolves @{-1}. Fixture creates a previous
# branch in HEAD's reflog by switching to feat then back to main; both git
# and gix-placeholder then exit 0 on `switch -`. Bytes parity (the
# "Switched to branch 'feat'" stderr emission and the actual @{-1}
# resolution by gix) is deferred.
title "gix switch -"
only_for_hash sha1-only && (small-repo-in-sandbox
  git checkout -b feat >/dev/null 2>&1
  git checkout - >/dev/null 2>&1
  it "matches git behavior" && {
    compat_effect "deferred until switch driver lands" -- switch -
  }
)

# --- create / force-create --------------------------------------------

# mode=effect — `-c <new-branch>` (vendor/git/builtin/checkout.c:2097..2098
# OPT_STRING). git creates `newbranch` and switches; gix-placeholder skips
# ref resolution when create.is_some() and exits 0. Both 0.
title "gix switch -c"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until switch driver lands" -- switch -c newbranch
  }
)

# mode=effect — long form of -c.
title "gix switch --create"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until switch driver lands" -- switch --create newbranch
  }
)

# mode=effect — `-C <existing>` (vendor/git/builtin/checkout.c:2099..2100
# OPT_STRING force_create). git resets `feat` to HEAD and switches;
# gix-placeholder skips ref check via force_create.is_some() and exits 0.
title "gix switch -C"
only_for_hash sha1-only && (small-repo-in-sandbox
  git branch feat
  it "matches git behavior" && {
    compat_effect "deferred until switch driver lands" -- switch -C feat
  }
)

# mode=effect — long form of -C.
title "gix switch --force-create"
only_for_hash sha1-only && (small-repo-in-sandbox
  git branch feat
  it "matches git behavior" && {
    compat_effect "deferred until switch driver lands" -- switch --force-create feat
  }
)

# --- detach -----------------------------------------------------------

# mode=effect — `-d HEAD` (vendor/git/builtin/checkout.c:1736 OPT_BOOL
# force_detach). git detaches HEAD at the named commit; gix-placeholder
# skips ref resolution when detach=true and exits 0.
title "gix switch -d"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until switch driver lands" -- switch -d HEAD
  }
)

# mode=effect — long form of -d.
title "gix switch --detach"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until switch driver lands" -- switch --detach HEAD
  }
)

# --- guess ------------------------------------------------------------

# mode=effect — `--guess` (vendor/git/builtin/checkout.c:2101..2102 OPT_BOOL
# dwim_new_local_branch). Default is on; explicit pass is a no-op when the
# positional resolves locally. Both binaries exit 0 on a fixture with the
# target branch present.
title "gix switch --guess"
only_for_hash sha1-only && (small-repo-in-sandbox
  git branch feat
  it "matches git behavior" && {
    compat_effect "deferred until switch driver lands" -- switch --guess feat
  }
)

# mode=effect — `--no-guess` disables the dwim-from-remote second-guess.
# When the positional resolves locally, the flag is a no-op; both 0.
title "gix switch --no-guess"
only_for_hash sha1-only && (small-repo-in-sandbox
  git branch feat
  it "matches git behavior" && {
    compat_effect "deferred until switch driver lands" -- switch --no-guess feat
  }
)

# --- discard-changes / force / merge ---------------------------------

# mode=effect — `--discard-changes` (vendor/git/builtin/checkout.c:2103..2104
# OPT_BOOL). On a clean fixture the flag is a no-op; both 0.
title "gix switch --discard-changes"
only_for_hash sha1-only && (small-repo-in-sandbox
  git branch feat
  it "matches git behavior" && {
    compat_effect "deferred until switch driver lands" -- switch --discard-changes feat
  }
)

# mode=effect — `-f` is documented as an alias for `--discard-changes`
# (vendor/git/Documentation/git-switch.adoc:113..115). Wired via OPT__FORCE
# at vendor/git/builtin/checkout.c:1741..1742.
title "gix switch -f"
only_for_hash sha1-only && (small-repo-in-sandbox
  git branch feat
  it "matches git behavior" && {
    compat_effect "deferred until switch driver lands" -- switch -f feat
  }
)

# mode=effect — long form of -f.
title "gix switch --force"
only_for_hash sha1-only && (small-repo-in-sandbox
  git branch feat
  it "matches git behavior" && {
    compat_effect "deferred until switch driver lands" -- switch --force feat
  }
)

# mode=effect — `-m` / `--merge` (vendor/git/builtin/checkout.c:1721
# OPT_BOOL). On a clean fixture the 3-way merge degenerates to a fast-
# forward; both 0.
title "gix switch -m"
only_for_hash sha1-only && (small-repo-in-sandbox
  git branch feat
  it "matches git behavior" && {
    compat_effect "deferred until switch driver lands" -- switch -m feat
  }
)

# mode=effect — long form of -m.
title "gix switch --merge"
only_for_hash sha1-only && (small-repo-in-sandbox
  git branch feat
  it "matches git behavior" && {
    compat_effect "deferred until switch driver lands" -- switch --merge feat
  }
)

# mode=effect — `--conflict=<style>` (vendor/git/builtin/checkout.c:1722..
# 1724 OPT_CALLBACK parse_opt_conflict). On a clean fixture there are no
# conflicts; both 0 regardless of style.
title "gix switch --conflict=merge"
only_for_hash sha1-only && (small-repo-in-sandbox
  git branch feat
  it "matches git behavior" && {
    compat_effect "deferred until switch driver lands" -- switch --conflict=merge feat
  }
)

# mode=effect — `--conflict=diff3` style.
title "gix switch --conflict=diff3"
only_for_hash sha1-only && (small-repo-in-sandbox
  git branch feat
  it "matches git behavior" && {
    compat_effect "deferred until switch driver lands" -- switch --conflict=diff3 feat
  }
)

# mode=effect — `--conflict=zdiff3` style.
title "gix switch --conflict=zdiff3"
only_for_hash sha1-only && (small-repo-in-sandbox
  git branch feat
  it "matches git behavior" && {
    compat_effect "deferred until switch driver lands" -- switch --conflict=zdiff3 feat
  }
)

# --- quiet / progress -------------------------------------------------

# mode=effect — `-q` / `--quiet` (vendor/git/builtin/checkout.c:1716
# OPT__QUIET) suppresses progress reporting. Both 0; bytes deferred.
title "gix switch -q"
only_for_hash sha1-only && (small-repo-in-sandbox
  git branch feat
  it "matches git behavior" && {
    compat_effect "deferred until switch driver lands" -- switch -q feat
  }
)

# mode=effect — long form of -q.
title "gix switch --quiet"
only_for_hash sha1-only && (small-repo-in-sandbox
  git branch feat
  it "matches git behavior" && {
    compat_effect "deferred until switch driver lands" -- switch --quiet feat
  }
)

# mode=effect — `--progress` (vendor/git/builtin/checkout.c:1720 OPT_BOOL
# show_progress) forces progress reporting even when stderr isn't a TTY.
# Both 0; the actual progress emission is deferred.
title "gix switch --progress"
only_for_hash sha1-only && (small-repo-in-sandbox
  git branch feat
  it "matches git behavior" && {
    compat_effect "deferred until switch driver lands" -- switch --progress feat
  }
)

# mode=effect — `--no-progress` is the inverse.
title "gix switch --no-progress"
only_for_hash sha1-only && (small-repo-in-sandbox
  git branch feat
  it "matches git behavior" && {
    compat_effect "deferred until switch driver lands" -- switch --no-progress feat
  }
)

# --- track / no-track ------------------------------------------------

# mode=bytes — `-t` / `--track` family (vendor/git/builtin/checkout.c:1737..
# 1740 OPT_CALLBACK parse_opt_tracking_mode). When the start-point doesn't
# resolve as a ref, both binaries die 128 with "fatal: invalid reference:
# <name>" — bytes-perfect parity. The shape `<remote>/<branch>` is required
# for the dwim-from-remote derivation per vendor/git/builtin/checkout.c:1917..
# 1922 (skip_prefix refs/, then remotes/, then derive name after the slash);
# `origin/feat` doesn't exist as a ref in the fixture, so both die before
# tracking-config setup.
title "gix switch -t"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    expect_parity bytes -- switch -t origin/feat
  }
)

# mode=bytes — long form of -t.
title "gix switch --track"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    expect_parity bytes -- switch --track origin/feat
  }
)

# mode=bytes — explicit `direct` tracking mode.
title "gix switch --track=direct"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    expect_parity bytes -- switch --track=direct origin/feat
  }
)

# mode=bytes — `inherit` tracking mode.
title "gix switch --track=inherit"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    expect_parity bytes -- switch --track=inherit origin/feat
  }
)

# mode=bytes — `--no-track` (vendor/git/builtin/checkout.c:1737..1740 sets
# track to BRANCH_TRACK_NEVER, a non-default value, so the dwim-derive
# branch at vendor/git/builtin/checkout.c:1913..1923 still fires). Same
# "invalid reference" die path on an unresolvable start-point.
title "gix switch --no-track"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    expect_parity bytes -- switch --no-track origin/feat
  }
)

# --- orphan ---------------------------------------------------------

# mode=effect — `--orphan <new-branch>` (vendor/git/builtin/checkout.c:1743
# OPT_STRING new_orphan_branch). git creates an unborn branch with an
# empty index; gix-placeholder skips ref resolution when orphan.is_some()
# and exits 0.
title "gix switch --orphan"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until switch driver lands" -- switch --orphan neworphan
  }
)

# --- overwrite-ignore / ignore-other-worktrees -----------------------

# mode=effect — `--overwrite-ignore` (vendor/git/builtin/checkout.c:1744..
# 1746 OPT_BOOL_F overwrite_ignore) is the default; explicit pass is a
# no-op on a fixture with no ignored files in the way. Both 0.
title "gix switch --overwrite-ignore"
only_for_hash sha1-only && (small-repo-in-sandbox
  git branch feat
  it "matches git behavior" && {
    compat_effect "deferred until switch driver lands" -- switch --overwrite-ignore feat
  }
)

# mode=effect — `--no-overwrite-ignore` preserves ignored files in the
# working tree during the switch. On a clean fixture, no-op; both 0.
title "gix switch --no-overwrite-ignore"
only_for_hash sha1-only && (small-repo-in-sandbox
  git branch feat
  it "matches git behavior" && {
    compat_effect "deferred until switch driver lands" -- switch --no-overwrite-ignore feat
  }
)

# mode=effect — `--ignore-other-worktrees` (vendor/git/builtin/checkout.c:
# 1747..1748 OPT_BOOL ignore_other_worktrees). On a fixture without
# additional worktrees, no-op; both 0.
title "gix switch --ignore-other-worktrees"
only_for_hash sha1-only && (small-repo-in-sandbox
  git branch feat
  it "matches git behavior" && {
    compat_effect "deferred until switch driver lands" -- switch --ignore-other-worktrees feat
  }
)

# --- recurse-submodules --------------------------------------------

# mode=effect — `--recurse-submodules` (vendor/git/builtin/checkout.c:1717..
# 1719 OPT_CALLBACK_F PARSE_OPT_OPTARG). On a fixture without submodules,
# no-op; both 0.
title "gix switch --recurse-submodules"
only_for_hash sha1-only && (small-repo-in-sandbox
  git branch feat
  it "matches git behavior" && {
    compat_effect "deferred until switch driver lands" -- switch --recurse-submodules feat
  }
)

# mode=effect — `--no-recurse-submodules` disables submodule recursion.
title "gix switch --no-recurse-submodules"
only_for_hash sha1-only && (small-repo-in-sandbox
  git branch feat
  it "matches git behavior" && {
    compat_effect "deferred until switch driver lands" -- switch --no-recurse-submodules feat
  }
)

# End-of-file sentinel: when every row is `only_for_hash sha1-only` and the
# active hash is sha256, the last statement returns 1 (skip), which would
# propagate out of `source` and trip `set -e` in tests/parity.sh. A trailing
# `:` normalizes the exit code so a fully-skipped file still returns 0.
:
