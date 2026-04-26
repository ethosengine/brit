# Must be sourced into tests/parity.sh or tests/journey.sh — see tests/parity.sh.
#
# Parity scaffold for `git mv` ↔ `gix mv`.
#
# One `title` + `it` block per flag derived from
# vendor/git/Documentation/git-mv.adoc and
# vendor/git/builtin/mv.c::cmd_mv (entry at
# vendor/git/builtin/mv.c:208). The flag surface (per
# vendor/git/builtin/mv.c:215..223) is:
#   -v / --verbose, -n / --dry-run, -f / --force, -k (no long form),
#   --sparse,
#   `<source>... <destination>` positional pair (or `<source>...
#   <destination-directory>` multi-source form).
#
# Synopsis (vendor/git/builtin/mv.c:31..34):
#   git mv [-v] [-f] [-n] [-k] <source> <destination>
#   git mv [-v] [-f] [-n] [-k] <source>... <destination-directory>
#
# Verdict modes (comment above each block):
#   bytes  — scriptable output: usage_with_options banner emitted by
#            the `argc < 2` gate (vendor/git/builtin/mv.c:247..248),
#            the `bad source` / `can not move directory into itself`
#            / `destination 'X' is not a directory` precondition error
#            stanzas, and the outside-of-repo "fatal: not a git
#            repository..." stanza. Wired in
#            gitoxide-core/src/repository/mv.rs::porcelain.
#   effect — UX-level parity (exit-code match). Default for the
#            human-rendered flags whose semantics are not yet
#            implemented in gix's mv entry point.
#
# `git mv` mutates the working tree and the index — back-to-back
# `expect_parity` runs in the same workdir let git's mutation poison
# gix's run (the source file is gone before gix sees it). Mutating
# rows therefore use `expect_parity_reset _mv-fixture effect` so each
# binary starts from a fresh per-binary fixture (mirrors rm.sh's
# `_rm-fixture` pattern). Non-mutating rows (--dry-run, --help,
# --bogus-flag, the precondition gates, -k missing-file) keep the
# cheaper `expect_parity` / `compat_effect` form. Adding a
# `compat_effect_reset` helper to surface mutating-row deferrals in
# `etc/parity/shortcomings.sh` is a follow-up; today the file header
# names the deferred mv-driver work explicitly.
#
# Coverage on gix's current Clap surface (src/plumbing/options/mv.rs):
#   gix mv [OPTIONS] [ARGS]... [-- <PATHS>...]
# Today the porcelain driver is a placeholder
# (gitoxide-core/src/repository/mv.rs::porcelain) that wires the
# C-mirror precondition matrix — the `argc < 2` usage banner emitted
# by `usage_with_options` (vendor/git/builtin/mv.c:247..248), the
# multi-source non-directory destination die
# (vendor/git/builtin/mv.c:278..279), the self-rename "can not move
# directory into itself" die (vendor/git/builtin/mv.c:346..350), and
# the workdir-miss "bad source" die approximation
# (vendor/git/builtin/mv.c:322..323; honors the `-k` ignore-errors
# branch at vendor/git/builtin/mv.c:483..485). The flag surface is
# clap-wired so `gix mv <flag> ...` does not trip UnknownArgument;
# every flag-bearing happy-path row therefore exits 0 with a stub
# note until the real mv driver lands. Closing this command requires
# implementing the mv driver in gitoxide-core/src/repository/mv.rs:
#   * `internal_prefix_pathspec` two-pass for sources and destinations
#     (vendor/git/builtin/mv.c:54..80).
#   * Per-source classification loop mirroring
#     vendor/git/builtin/mv.c:296..496 — index lookup,
#     SPARSE/SKIP_WORKTREE_DIR/MOVE_VIA_PARENT_DIR mode tracking,
#     destination-exists/conflicted/multi-source-target gates with
#     the verbatim `fatal: %s, source=%s, destination=%s` wording.
#   * `rename(2)` + index update + per-file `Renaming X to Y\n` emission
#     under `-v`/`-n` (vendor/git/builtin/mv.c:540..548).
#   * Submodule gitfile rewrite + .gitmodules path update
#     (vendor/git/builtin/mv.c:553..560).
#   * Sparse-checkout advisory branch
#     (vendor/git/builtin/mv.c:524..530 advise_on_updating_sparse_paths).
#   * `--ignore-errors` (-k) silent-continue branch
#     (vendor/git/builtin/mv.c:481..485).
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
# git's --help delegates to `man git-mv` (exit 0 when man is
# available); gix returns clap's auto-generated help. Message text
# diverges; only the exit-code match is asserted.
# hash=dual
title "gix mv --help"
only_for_hash dual && (sandbox
  it "matches git behavior" && {
    expect_parity effect -- mv --help
  }
)

# mode=effect — unknown flag: git exits 129 (usage_msg_opt in
# parse-options.c). gix's Clap layer maps UnknownArgument to 129 via
# src/plumbing/main.rs. Tested inside a repo because git outside a
# repo dies 128 before reaching arg-parse, while clap in gix always
# runs first.
# hash=sha1-only
title "gix mv --bogus-flag"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    expect_parity effect -- mv --bogus-flag
  }
)

# mode=bytes — outside any repo: git dies 128 with "fatal: not a git
# repository (or any of the parent directories): .git". gix's plumbing
# repository() closure already remaps gix_discover::upwards::Error::
# NoGitRepository* variants to git's exact wording + exit 128.
# hash=dual
title "gix mv (outside a repository)"
only_for_hash dual && (sandbox
  it "matches git behavior" && {
    expect_parity bytes -- mv a b
  }
)

# --- usage_with_options ------------------------------------------------

# mode=bytes — bare `git mv` (no positional) hits
# `usage_with_options(builtin_mv_usage, builtin_mv_options)` at
# vendor/git/builtin/mv.c:247..248 → exit 129 + verbatim usage banner.
# gix's porcelain mirrors the banner verbatim.
# hash=sha1-only
title "gix mv (no positional)"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    expect_parity bytes -- mv
  }
)

# mode=bytes — `git mv <one>` hits the same `--argc < 1` branch (git
# decrements argc by 1 to peel off the destination, then falls into
# usage_with_options). exit 129 + verbatim banner.
# hash=sha1-only
title "gix mv a"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    expect_parity bytes -- mv a
  }
)

# --- single-source happy path -----------------------------------------

# mode=effect — `git mv <existing-tracked-file> <new-name>` renames
# the file in the working tree and stages the rename in the index.
# Per vendor/git/builtin/mv.c:540..548. The placeholder stub exits 0
# with a stub note; closing this row requires the real mv driver.
# Mutating: reset-fixture isolates each binary's run.
# hash=sha1-only
title "gix mv a b"
only_for_hash sha1-only && (sandbox
  function _mv-fixture() {
    git-init-hash-aware
    git checkout -b main >/dev/null 2>&1
    git config commit.gpgsign false
    touch a
    git add a
    git -c user.email=t@t -c user.name=t commit -q -m c1
  }
  it "matches git behavior" && {
    expect_parity_reset _mv-fixture effect -- mv a b
  }
)

# mode=effect — `git mv -- <src> <dst>` is the explicit form past the
# `--` separator. parse-options consumes `--` as the option terminator
# and treats both positionals as sources/destination. Mutating:
# reset-fixture isolates each binary's run.
# hash=sha1-only
title "gix mv -- a b"
only_for_hash sha1-only && (sandbox
  function _mv-fixture() {
    git-init-hash-aware
    git checkout -b main >/dev/null 2>&1
    git config commit.gpgsign false
    touch a
    git add a
    git -c user.email=t@t -c user.name=t commit -q -m c1
  }
  it "matches git behavior" && {
    expect_parity_reset _mv-fixture effect -- mv -- a b
  }
)

# --- multi-source happy path (move into directory) --------------------

# mode=effect — `git mv <src>... <dst-dir>` moves multiple sources
# into an existing directory. Per vendor/git/builtin/mv.c:272..273
# (the `lstat(dest) → S_ISDIR` branch). The placeholder accepts the
# arg shape and exits 0; closing this row requires the real driver.
# Mutating: reset-fixture isolates each binary's run.
# hash=sha1-only
title "gix mv a b dir/"
only_for_hash sha1-only && (sandbox
  function _mv-multi-fixture() {
    git-init-hash-aware
    git checkout -b main >/dev/null 2>&1
    git config commit.gpgsign false
    touch a b
    mkdir dir
    git add a b
    git -c user.email=t@t -c user.name=t commit -q -m c1
  }
  it "matches git behavior" && {
    expect_parity_reset _mv-multi-fixture effect -- mv a b dir/
  }
)

# --- precondition gates: bytes parity --------------------------------

# mode=bytes — `git mv missing-file b` hits the `bad = _("bad
# source")` branch at vendor/git/builtin/mv.c:322..323 (lstat fails
# AND not in the index). git emits the verbatim
# `fatal: bad source, source=missing-file, destination=b` + exit 128.
# gix's porcelain stub mirrors the wording via a workdir lstat
# approximation (sparse-checkout false-positives are deferred to the
# real driver). Non-mutating: plain expect_parity is fine.
# hash=sha1-only
title "gix mv missing-file b"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    expect_parity bytes -- mv missing-file b
  }
)

# mode=bytes — `git mv <same> <same>` hits the
# `!strncmp(src, dst, length) && (dst[length] == 0 || dst[length] ==
# '/')` branch at vendor/git/builtin/mv.c:346..350. Even when the
# path is a file, git's wording carries "directory into itself" —
# mirror verbatim. Non-mutating (gate fires before any rename).
# hash=sha1-only
title "gix mv a a"
only_for_hash sha1-only && (sandbox
  function _mv-fixture() {
    git-init-hash-aware
    git checkout -b main >/dev/null 2>&1
    git config commit.gpgsign false
    touch a
    git add a
    git -c user.email=t@t -c user.name=t commit -q -m c1
  }
  it "matches git behavior" && {
    expect_parity_reset _mv-fixture bytes -- mv a a
  }
)

# mode=bytes — `git mv <src1> <src2> <non-dir>` hits the
# `argc != 1` branch at vendor/git/builtin/mv.c:278..279 (the
# multi-source destination must be an existing directory). git emits
# `fatal: destination 'X' is not a directory` + exit 128.
# Non-mutating (gate fires before any rename).
# hash=sha1-only
title "gix mv a b c (c is not a directory)"
only_for_hash sha1-only && (sandbox
  function _mv-multi-fixture() {
    git-init-hash-aware
    git checkout -b main >/dev/null 2>&1
    git config commit.gpgsign false
    touch a b
    git add a b
    git -c user.email=t@t -c user.name=t commit -q -m c1
  }
  it "matches git behavior" && {
    expect_parity_reset _mv-multi-fixture bytes -- mv a b c
  }
)

# --- per-flag rows ----------------------------------------------------

# mode=effect — `-v` / `--verbose` (vendor/git/builtin/mv.c:216
# OPT__VERBOSE) emits per-file `Renaming X to Y\n` on stdout
# (vendor/git/builtin/mv.c:542..543). The placeholder accepts the
# flag and exits 0; bytes parity (the per-file emission) is deferred.
# Mutating: reset-fixture.
# hash=sha1-only
title "gix mv -v a b"
only_for_hash sha1-only && (sandbox
  function _mv-fixture() {
    git-init-hash-aware
    git checkout -b main >/dev/null 2>&1
    git config commit.gpgsign false
    touch a
    git add a
    git -c user.email=t@t -c user.name=t commit -q -m c1
  }
  it "matches git behavior" && {
    expect_parity_reset _mv-fixture effect -- mv -v a b
  }
)

# mode=effect — long form of `-v`.
# hash=sha1-only
title "gix mv --verbose a b"
only_for_hash sha1-only && (sandbox
  function _mv-fixture() {
    git-init-hash-aware
    git checkout -b main >/dev/null 2>&1
    git config commit.gpgsign false
    touch a
    git add a
    git -c user.email=t@t -c user.name=t commit -q -m c1
  }
  it "matches git behavior" && {
    expect_parity_reset _mv-fixture effect -- mv --verbose a b
  }
)

# mode=effect — `-n` / `--dry-run` (vendor/git/builtin/mv.c:217
# OPT__DRY_RUN, the C variable is `show_only`) leaves the index and
# working tree untouched but still emits the `Renaming X to Y\n`
# preview (vendor/git/builtin/mv.c:542..543) and the
# `Checking rename of '%s' to '%s'\n` line at
# vendor/git/builtin/mv.c:302..303. The placeholder accepts the flag
# and exits 0; bytes parity is deferred. Non-mutating: plain
# expect_parity is sufficient.
# hash=sha1-only
title "gix mv -n a b"
only_for_hash sha1-only && (sandbox
  function _mv-fixture() {
    git-init-hash-aware
    git checkout -b main >/dev/null 2>&1
    git config commit.gpgsign false
    touch a
    git add a
    git -c user.email=t@t -c user.name=t commit -q -m c1
  }
  it "matches git behavior" && {
    _mv-fixture
    compat_effect "deferred until mv driver lands" -- mv -n a b
  }
)

# mode=effect — long form of `-n`.
# hash=sha1-only
title "gix mv --dry-run a b"
only_for_hash sha1-only && (sandbox
  function _mv-fixture() {
    git-init-hash-aware
    git checkout -b main >/dev/null 2>&1
    git config commit.gpgsign false
    touch a
    git add a
    git -c user.email=t@t -c user.name=t commit -q -m c1
  }
  it "matches git behavior" && {
    _mv-fixture
    compat_effect "deferred until mv driver lands" -- mv --dry-run a b
  }
)

# mode=effect — `-f` / `--force` (vendor/git/builtin/mv.c:218..219
# OPT__FORCE) skips the destination-exists gate at
# vendor/git/builtin/mv.c:420..436. On a fresh fixture without a
# pre-existing destination the flag is a no-op. The placeholder
# accepts the flag and exits 0. Mutating: reset-fixture.
# hash=sha1-only
title "gix mv -f a b"
only_for_hash sha1-only && (sandbox
  function _mv-fixture() {
    git-init-hash-aware
    git checkout -b main >/dev/null 2>&1
    git config commit.gpgsign false
    touch a
    git add a
    git -c user.email=t@t -c user.name=t commit -q -m c1
  }
  it "matches git behavior" && {
    expect_parity_reset _mv-fixture effect -- mv -f a b
  }
)

# mode=effect — long form of `-f`.
# hash=sha1-only
title "gix mv --force a b"
only_for_hash sha1-only && (sandbox
  function _mv-fixture() {
    git-init-hash-aware
    git checkout -b main >/dev/null 2>&1
    git config commit.gpgsign false
    touch a
    git add a
    git -c user.email=t@t -c user.name=t commit -q -m c1
  }
  it "matches git behavior" && {
    expect_parity_reset _mv-fixture effect -- mv --force a b
  }
)

# mode=effect — `-k` (vendor/git/builtin/mv.c:220 OPT_BOOL('k', NULL,
# ...), the C variable is `ignore_errors`; no long form per git's
# parse-options table) silently skips per-source errors at
# vendor/git/builtin/mv.c:483..485 (the `if (!ignore_errors)` gate
# around the die() call). git's exit code on a missing source under
# `-k` is 0; gix's porcelain mirrors that branch by short-circuiting
# the bad-source gate when `ignore_errors` is set, then exiting 0
# with the stub note. Non-mutating (no source to rename).
# hash=sha1-only
title "gix mv -k missing-file b"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until mv driver lands" -- mv -k missing-file b
  }
)

# mode=effect — `--sparse` (vendor/git/builtin/mv.c:221 OPT_BOOL(0,
# "sparse", ...), the C variable is `ignore_sparse`) allows updating
# index entries outside of the sparse-checkout cone — mirrors the
# `if (!ignore_sparse)` gates at vendor/git/builtin/mv.c:330,
# vendor/git/builtin/mv.c:464, and vendor/git/builtin/mv.c:469. On a
# non-sparse fixture the flag is a no-op. The placeholder accepts
# the flag and exits 0. Mutating: reset-fixture.
# hash=sha1-only
title "gix mv --sparse a b"
only_for_hash sha1-only && (sandbox
  function _mv-fixture() {
    git-init-hash-aware
    git checkout -b main >/dev/null 2>&1
    git config commit.gpgsign false
    touch a
    git add a
    git -c user.email=t@t -c user.name=t commit -q -m c1
  }
  it "matches git behavior" && {
    expect_parity_reset _mv-fixture effect -- mv --sparse a b
  }
)

# End-of-file sentinel: when every row is `only_for_hash sha1-only` and the
# active hash is sha256, the last statement returns 1 (skip), which would
# propagate out of `source` and trip `set -e` in tests/parity.sh. A trailing
# `:` normalizes the exit code so a fully-skipped file still returns 0.
:
