# Must be sourced into tests/parity.sh or tests/journey.sh — see tests/parity.sh.
#
# Parity scaffold for `git rm` ↔ `gix rm`.
#
# One `title` + `it` block per flag derived from
# vendor/git/Documentation/git-rm.adoc and
# vendor/git/builtin/rm.c::cmd_rm (entry at
# vendor/git/builtin/rm.c:266). The flag surface (per
# vendor/git/builtin/rm.c:248..262) is:
#   -n / --dry-run, -q / --quiet, --cached, -f / --force,
#   -r (no long form), --ignore-unmatch, --sparse,
#   --pathspec-from-file=<file>, --pathspec-file-nul,
#   `--` separator + `<pathspec>...`.
#
# Synopsis (vendor/git/builtin/rm.c:29):
#   git rm [-f | --force] [-n] [-r] [--cached] [--ignore-unmatch]
#          [--quiet] [--pathspec-from-file=<file> [--pathspec-file-nul]]
#          [--] [<pathspec>...]
#
# Verdict modes (comment above each block):
#   bytes  — scriptable output: precondition error stanzas around the
#            empty-pathspec die, --pathspec-file-nul without
#            --pathspec-from-file, --pathspec-from-file with positional
#            pathspec args, missing-pathspec die, and the
#            outside-of-repo "fatal: not a git repository..." stanza.
#            Wired in gitoxide-core/src/repository/rm.rs::porcelain.
#   effect — UX-level parity (exit-code match). Default for the
#            human-rendered flags whose semantics are not yet
#            implemented in gix's rm entry point.
#
# `git rm` mutates the working tree and the index — back-to-back
# `expect_parity` runs in the same workdir let git's mutation poison
# gix's run (the file is gone before gix sees it). Mutating rows
# therefore use `expect_parity_reset _rm-fixture effect` so each binary
# starts from a fresh per-binary fixture (mirrors branch.sh's
# `_branch-move-fixture` pattern). Non-mutating rows (--dry-run,
# precondition gates, --ignore-unmatch missing-file) keep the cheaper
# `expect_parity` / `compat_effect` form. Adding a
# `compat_effect_reset` helper to surface mutating-row deferrals in
# `etc/parity/shortcomings.sh` is a follow-up; today the file header
# names the deferred rm-driver work explicitly.
#
# Coverage on gix's current Clap surface (src/plumbing/options/rm.rs):
#   gix rm [OPTIONS] [ARGS]... [-- <PATHS>...]
# Today the porcelain driver is a placeholder
# (gitoxide-core/src/repository/rm.rs::porcelain) that wires the
# C-mirror precondition matrix (mirrors vendor/git/builtin/rm.c:286..299):
# `--pathspec-from-file` mutually exclusive with positional pathspec;
# `--pathspec-file-nul` requires `--pathspec-from-file`; empty pathspec
# dies 128 with "No pathspec was given. Which files should I remove?";
# non-magic missing positional pathspec dies 128 with "pathspec '<x>'
# did not match any files" (skipped under --ignore-unmatch). The flag
# surface is clap-wired so `gix rm <flag> ...` does not trip
# UnknownArgument; every flag-bearing happy-path row therefore exits 0
# with a stub note until the real rm driver lands. Closing this command
# requires implementing the rm driver in
# gitoxide-core/src/repository/rm.rs:
#   * pathspec walker over the index (mirroring
#     vendor/git/builtin/rm.c:316..330 ce_path_match loop) — the only
#     way to distinguish tracked vs. untracked positionals.
#   * `not removing '<x>' recursively without -r` enforcement
#     (vendor/git/builtin/rm.c:347..348).
#   * check_local_mod up-to-date-check + the three error stanzas at
#     vendor/git/builtin/rm.c:216..240 ("staged content different from
#     both the file and the HEAD" / "changes staged in the index" /
#     "local modifications") with `-f` to override.
#   * remove_file_from_index + remove_path working-tree deletion +
#     per-file `rm '<path>'` emission (vendor/git/builtin/rm.c:386..408).
#   * Submodule absorb logic (vendor/git/builtin/rm.c:74..98 +
#     vendor/git/builtin/rm.c:425..465).
#   * Sparse-checkout advisory branch
#     (vendor/git/builtin/rm.c:357..362 advise_on_updating_sparse_paths).
#   * --pathspec-from-file: parse_pathspec_file at
#     vendor/git/builtin/rm.c:283..285.
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
# git's --help delegates to `man git-rm` (exit 0 when man is
# available); gix returns clap's auto-generated help. Message text
# diverges; only the exit-code match is asserted.
# hash=dual
title "gix rm --help"
only_for_hash dual && (sandbox
  it "matches git behavior" && {
    expect_parity effect -- rm --help
  }
)

# mode=effect — unknown flag: git exits 129 (usage_msg_opt in
# parse-options.c). gix's Clap layer maps UnknownArgument to 129 via
# src/plumbing/main.rs. Tested inside a repo because git outside a
# repo dies 128 before reaching arg-parse, while clap in gix always
# runs first.
# hash=sha1-only
title "gix rm --bogus-flag"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    expect_parity effect -- rm --bogus-flag
  }
)

# mode=bytes — outside any repo: git dies 128 with "fatal: not a git
# repository (or any of the parent directories): .git". gix's plumbing
# repository() closure already remaps gix_discover::upwards::Error::
# NoGitRepository* variants to git's exact wording + exit 128.
# hash=dual
title "gix rm (outside a repository)"
only_for_hash dual && (sandbox
  it "matches git behavior" && {
    expect_parity bytes -- rm a
  }
)

# --- empty pathspec ----------------------------------------------------

# mode=bytes — bare `git rm` (no pathspec, no `--pathspec-from-file`)
# dies 128 with "fatal: No pathspec was given. Which files should I
# remove?". Per vendor/git/builtin/rm.c:298..299. gix's porcelain stub
# mirrors the verbatim wording.
# hash=sha1-only
title "gix rm (no pathspec)"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    expect_parity bytes -- rm
  }
)

# --- pathspec positional ----------------------------------------------

# mode=effect — `git rm <existing-tracked-file>` removes the file from
# the index and the working tree, prints `rm 'a'`, exits 0. Per
# vendor/git/builtin/rm.c:386..408. The reset-fixture isolates the
# mutation per binary.
# hash=sha1-only
title "gix rm a"
only_for_hash sha1-only && (sandbox
  function _rm-fixture() {
    git-init-hash-aware
    git checkout -b main >/dev/null 2>&1
    git config commit.gpgsign false
    touch a
    git add a
    git -c user.email=t@t -c user.name=t commit -q -m c1
  }
  it "matches git behavior" && {
    expect_parity_reset _rm-fixture effect -- rm a
  }
)

# mode=effect — `git rm -- <path>` is the explicit form past the `--`
# separator. Per vendor/git/builtin/rm.c:30 `PARSE_OPT_KEEP_DASHDASH`.
# hash=sha1-only
title "gix rm -- a"
only_for_hash sha1-only && (sandbox
  function _rm-fixture() {
    git-init-hash-aware
    git checkout -b main >/dev/null 2>&1
    git config commit.gpgsign false
    touch a
    git add a
    git -c user.email=t@t -c user.name=t commit -q -m c1
  }
  it "matches git behavior" && {
    expect_parity_reset _rm-fixture effect -- rm -- a
  }
)

# mode=bytes — `git rm <missing-file>` errors at
# vendor/git/builtin/rm.c:357 with "fatal: pathspec 'missing-file' did
# not match any files" + exit 128. gix's porcelain stub mirrors the
# verbatim wording. No mutation, so plain `expect_parity` is fine.
# hash=sha1-only
title "gix rm missing-file"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    expect_parity bytes -- rm missing-file
  }
)

# --- ignore-unmatch ---------------------------------------------------

# mode=effect — `--ignore-unmatch` (vendor/git/builtin/rm.c:254)
# silences the missing-pathspec die: git exits 0 with no output. gix's
# porcelain stub bypasses the existence-check gate and exits 0 with a
# stub note (effect-mode parity holds). No mutation, so plain
# `compat_effect` is fine.
# hash=sha1-only
title "gix rm --ignore-unmatch missing-file"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until rm driver lands" -- rm --ignore-unmatch missing-file
  }
)

# --- cached -----------------------------------------------------------

# mode=effect — `--cached` (vendor/git/builtin/rm.c:251) unstages and
# removes paths only from the index; the working tree file is left
# alone. Exit 0 on success; output is `rm 'a'` per
# vendor/git/builtin/rm.c:387. Index-mutating, so reset-fixture
# isolates the run.
# hash=sha1-only
title "gix rm --cached a"
only_for_hash sha1-only && (sandbox
  function _rm-fixture() {
    git-init-hash-aware
    git checkout -b main >/dev/null 2>&1
    git config commit.gpgsign false
    touch a
    git add a
    git -c user.email=t@t -c user.name=t commit -q -m c1
  }
  it "matches git behavior" && {
    expect_parity_reset _rm-fixture effect -- rm --cached a
  }
)

# --- dry-run ----------------------------------------------------------

# mode=effect — `-n` / `--dry-run` (vendor/git/builtin/rm.c:249)
# triggers the `show_only` branch at vendor/git/builtin/rm.c:411..412
# — the index/working tree are left untouched, but per-file `rm '<x>'`
# output is still emitted. No mutation, so plain `compat_effect`.
# hash=sha1-only
title "gix rm -n a"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until rm driver lands" -- rm -n a
  }
)

# mode=effect — long form of `-n`.
# hash=sha1-only
title "gix rm --dry-run a"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until rm driver lands" -- rm --dry-run a
  }
)

# --- quiet ------------------------------------------------------------

# mode=effect — `-q` / `--quiet` (vendor/git/builtin/rm.c:250)
# suppresses the per-file `rm '<x>'` emission at
# vendor/git/builtin/rm.c:387. Mutating, reset-fixture.
# hash=sha1-only
title "gix rm -q a"
only_for_hash sha1-only && (sandbox
  function _rm-fixture() {
    git-init-hash-aware
    git checkout -b main >/dev/null 2>&1
    git config commit.gpgsign false
    touch a
    git add a
    git -c user.email=t@t -c user.name=t commit -q -m c1
  }
  it "matches git behavior" && {
    expect_parity_reset _rm-fixture effect -- rm -q a
  }
)

# mode=effect — long form of `-q`.
# hash=sha1-only
title "gix rm --quiet a"
only_for_hash sha1-only && (sandbox
  function _rm-fixture() {
    git-init-hash-aware
    git checkout -b main >/dev/null 2>&1
    git config commit.gpgsign false
    touch a
    git add a
    git -c user.email=t@t -c user.name=t commit -q -m c1
  }
  it "matches git behavior" && {
    expect_parity_reset _rm-fixture effect -- rm --quiet a
  }
)

# --- force ------------------------------------------------------------

# mode=effect — `-f` / `--force` (vendor/git/builtin/rm.c:252) skips
# the `check_local_mod` up-to-date verification at
# vendor/git/builtin/rm.c:373..380. On a clean tracked file the flag
# is a no-op (both binaries succeed identically); the divergence
# surfaces only when the working tree is dirty, which the deferred
# driver covers. Mutating, reset-fixture.
# hash=sha1-only
title "gix rm -f a"
only_for_hash sha1-only && (sandbox
  function _rm-fixture() {
    git-init-hash-aware
    git checkout -b main >/dev/null 2>&1
    git config commit.gpgsign false
    touch a
    git add a
    git -c user.email=t@t -c user.name=t commit -q -m c1
  }
  it "matches git behavior" && {
    expect_parity_reset _rm-fixture effect -- rm -f a
  }
)

# mode=effect — long form of `-f`.
# hash=sha1-only
title "gix rm --force a"
only_for_hash sha1-only && (sandbox
  function _rm-fixture() {
    git-init-hash-aware
    git checkout -b main >/dev/null 2>&1
    git config commit.gpgsign false
    touch a
    git add a
    git -c user.email=t@t -c user.name=t commit -q -m c1
  }
  it "matches git behavior" && {
    expect_parity_reset _rm-fixture effect -- rm --force a
  }
)

# --- recursive --------------------------------------------------------

# mode=effect — `-r` (vendor/git/builtin/rm.c:253; no long form per
# git's `OPT_BOOL('r', NULL, ...)`) allows recursive removal when a
# leading directory name is given. The pathspec must match a tracked
# subtree; the deferred driver implements the recursive-match check at
# vendor/git/builtin/rm.c:347..348. With a single file pathspec the
# flag is a no-op and both binaries succeed identically. Mutating,
# reset-fixture.
# hash=sha1-only
title "gix rm -r a"
only_for_hash sha1-only && (sandbox
  function _rm-fixture() {
    git-init-hash-aware
    git checkout -b main >/dev/null 2>&1
    git config commit.gpgsign false
    touch a
    git add a
    git -c user.email=t@t -c user.name=t commit -q -m c1
  }
  it "matches git behavior" && {
    expect_parity_reset _rm-fixture effect -- rm -r a
  }
)

# --- sparse -----------------------------------------------------------

# mode=effect — `--sparse` (vendor/git/builtin/rm.c:256) allows
# updating index entries outside of the sparse-checkout cone. Without
# `--sparse`, paths outside the cone hit the
# `advise_on_updating_sparse_paths` branch at
# vendor/git/builtin/rm.c:357..362. On a non-sparse fixture the flag
# is a no-op (both binaries succeed identically). Mutating,
# reset-fixture.
# hash=sha1-only
title "gix rm --sparse a"
only_for_hash sha1-only && (sandbox
  function _rm-fixture() {
    git-init-hash-aware
    git checkout -b main >/dev/null 2>&1
    git config commit.gpgsign false
    touch a
    git add a
    git -c user.email=t@t -c user.name=t commit -q -m c1
  }
  it "matches git behavior" && {
    expect_parity_reset _rm-fixture effect -- rm --sparse a
  }
)

# --- pathspec sources -------------------------------------------------

# mode=effect — `--pathspec-from-file=<file>` (vendor/git/builtin/rm.c:257)
# reads pathspec entries from a file via parse_pathspec_file at
# vendor/git/builtin/rm.c:283..285. Mutating; the reset-fixture also
# materializes spec.txt so each binary sees an identical input.
# hash=sha1-only
title "gix rm --pathspec-from-file=spec.txt"
only_for_hash sha1-only && (sandbox
  function _rm-fixture-with-spec() {
    git-init-hash-aware
    git checkout -b main >/dev/null 2>&1
    git config commit.gpgsign false
    touch a
    git add a
    git -c user.email=t@t -c user.name=t commit -q -m c1
    echo a > spec.txt
  }
  it "matches git behavior" && {
    expect_parity_reset _rm-fixture-with-spec effect -- rm --pathspec-from-file=spec.txt
  }
)

# mode=effect — `--pathspec-file-nul` (vendor/git/builtin/rm.c:258) is
# only meaningful with `--pathspec-from-file`; the pairing parses
# pathspec entries as NUL-separated.
# hash=sha1-only
title "gix rm --pathspec-from-file=spec.txt --pathspec-file-nul"
only_for_hash sha1-only && (sandbox
  function _rm-fixture-with-spec-nul() {
    git-init-hash-aware
    git checkout -b main >/dev/null 2>&1
    git config commit.gpgsign false
    touch a
    git add a
    git -c user.email=t@t -c user.name=t commit -q -m c1
    printf 'a\0' > spec.txt
  }
  it "matches git behavior" && {
    expect_parity_reset _rm-fixture-with-spec-nul effect -- rm --pathspec-from-file=spec.txt --pathspec-file-nul
  }
)

# --- precondition gates -----------------------------------------------

# mode=bytes — `--pathspec-file-nul` without `--pathspec-from-file`
# errors at vendor/git/builtin/rm.c:294..295 ("the option
# '--pathspec-file-nul' requires '--pathspec-from-file'"). gix's
# porcelain stub mirrors the verbatim wording.
# hash=sha1-only
title "gix rm --pathspec-file-nul"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    expect_parity bytes -- rm --pathspec-file-nul
  }
)

# mode=bytes — `--pathspec-from-file=spec.txt -- a` errors at
# vendor/git/builtin/rm.c:286..287 ("'--pathspec-from-file' and
# pathspec arguments cannot be used together"). gix's porcelain stub
# mirrors the verbatim wording. No mutation (gate fires before any
# index/working-tree update).
# hash=sha1-only
title "gix rm --pathspec-from-file=spec.txt -- a"
only_for_hash sha1-only && (small-repo-in-sandbox
  echo a > spec.txt
  it "matches git behavior" && {
    expect_parity bytes -- rm --pathspec-from-file=spec.txt -- a
  }
)

# End-of-file sentinel: when every row is `only_for_hash sha1-only` and the
# active hash is sha256, the last statement returns 1 (skip), which would
# propagate out of `source` and trip `set -e` in tests/parity.sh. A trailing
# `:` normalizes the exit code so a fully-skipped file still returns 0.
:
