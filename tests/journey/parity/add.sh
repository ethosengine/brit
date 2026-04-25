# Must be sourced into tests/parity.sh or tests/journey.sh — see tests/parity.sh.
#
# Parity scaffold for `git add` ↔ `gix add`.
#
# One `title` + `it` block per flag derived from
# vendor/git/Documentation/git-add.adoc and
# vendor/git/builtin/add.c::cmd_add (entry at
# vendor/git/builtin/add.c:381). The flag surface (per
# vendor/git/builtin/add.c:253..283) is:
#   -n / --dry-run, -v / --verbose,
#   -i / --interactive, -p / --patch, --auto-advance / --no-auto-advance,
#   -U / --unified, --inter-hunk-context,
#   -e / --edit,
#   -f / --force, -u / --update, --renormalize,
#   -N / --intent-to-add,
#   -A / --all (alias --no-ignore-removal), --no-all / --ignore-removal,
#   --refresh, --ignore-errors, --ignore-missing,
#   --sparse, --chmod=(+|-)x,
#   --pathspec-from-file=<file>, --pathspec-file-nul,
#   `--` separator + `<pathspec>...`.
#
# Synopsis (vendor/git/builtin/add.c:29):
#   git add [<options>] [--] <pathspec>...
#
# Verdict modes (comment above each block):
#   bytes  — scriptable output: precondition error stanzas around
#            mode mutual-exclusion (--dry-run vs --interactive/--patch,
#            --pathspec-from-file vs --interactive/--patch and --edit,
#            -A vs -u, --ignore-missing without --dry-run, --chmod
#            argument validation, --pathspec-from-file + pathspec args,
#            --pathspec-file-nul without --pathspec-from-file,
#            --unified/--inter-hunk-context/--no-auto-advance without
#            --interactive/--patch), the empty-pathspec "Nothing
#            specified, nothing added." stanza, and the outside-of-repo
#            "fatal: not a git repository..." stanza. Wired in
#            gitoxide-core/src/repository/add.rs::porcelain.
#   effect — UX-level parity (exit-code match). Default for the
#            human-rendered flags whose semantics are not yet
#            implemented in gix's add entry point. Most rows close as
#            `compat_effect "deferred until add driver lands"`.
#
# Coverage on gix's current Clap surface (src/plumbing/options/add.rs):
#   gix add [OPTIONS] [ARGS]... [-- <PATHS>...]
# Today the porcelain driver is a placeholder
# (gitoxide-core/src/repository/add.rs::porcelain) that mirrors git's
# pre-pathspec validation matrix and emits a stub note on stdout for
# the happy path. The flag surface is clap-wired so `gix add <flag>
# ...` does not trip UnknownArgument; every flag-bearing happy-path
# row therefore closes as `compat_effect "deferred until add driver
# lands"` until the real driver implements the index-update semantic.
# Closing this command requires implementing the add driver in
# gitoxide-core/src/repository/add.rs:
#   * add_files_to_cache: walk pathspec across worktree + index, hash
#     each file's blob via gix-object, write the corresponding cache
#     entry (mirroring vendor/git/builtin/add.c:589..591 add_files_to_cache).
#   * fill_directory + prune_directory: untracked-file walking (mirroring
#     vendor/git/builtin/add.c:510..513).
#   * --intent-to-add: empty-blob entries via ADD_CACHE_INTENT (mirroring
#     vendor/git/builtin/add.c:489).
#   * --refresh: stat-only refresh path via refresh_index helper (mirroring
#     vendor/git/builtin/add.c:516..517).
#   * --renormalize: clean-filter replay via renormalize_tracked_files
#     (mirroring vendor/git/builtin/add.c:72..96 + 587).
#   * --chmod: index-entry mode toggle via chmod_pathspec (mirroring
#     vendor/git/builtin/add.c:41..70 + 600..601).
#   * --pathspec-from-file: parse_pathspec_file at
#     vendor/git/builtin/add.c:464..473.
#   * --interactive / --patch / --edit: add-interactive subprocess
#     replay at vendor/git/builtin/add.c:417 + 430.
#   * Embedded-repo advisory at vendor/git/builtin/add.c:317..339.
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
# git's --help delegates to `man git-add` (exit 0 when man is
# available); gix returns clap's auto-generated help. Message text
# diverges; only the exit-code match is asserted.
# hash=dual
title "gix add --help"
only_for_hash dual && (sandbox
  it "matches git behavior" && {
    expect_parity effect -- add --help
  }
)

# mode=effect — unknown flag: git exits 129 (usage_msg_opt in
# parse-options.c). gix's Clap layer maps UnknownArgument to 129 via
# src/plumbing/main.rs. Tested inside a repo because git outside a
# repo dies 128 before reaching arg-parse, while clap in gix always
# runs first.
# hash=sha1-only
title "gix add --bogus-flag"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    expect_parity effect -- add --bogus-flag
  }
)

# mode=bytes — outside any repo: git dies 128 with "fatal: not a git
# repository (or any of the parent directories): .git". gix's plumbing
# repository() closure already remaps gix_discover::upwards::Error::
# NoGitRepository* variants to git's exact wording + exit 128.
# hash=dual
title "gix add (outside a repository)"
only_for_hash dual && (sandbox
  it "matches git behavior" && {
    expect_parity bytes -- add file
  }
)

# --- empty pathspec ----------------------------------------------------

# mode=bytes — bare `git add` (no pathspec, no -u/-A) emits "Nothing
# specified, nothing added." + the addEmptyPathspec advice on stderr
# and returns 0. Per vendor/git/builtin/add.c:476..481. gix's
# porcelain stub mirrors the verbatim wording.
# hash=sha1-only
title "gix add (no pathspec)"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    expect_parity bytes -- add
  }
)

# --- pathspec positional ----------------------------------------------

# mode=effect — `git add <existing-tracked-file>` is a no-op when the
# file matches HEAD. Per vendor/git/builtin/add.c:589 add_files_to_cache.
# hash=sha1-only
title "gix add a"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until add driver lands" -- add a
  }
)

# mode=effect — `git add <new-file>` stages an untracked file.
# hash=sha1-only
title "gix add new-file"
only_for_hash sha1-only && (small-repo-in-sandbox
  echo content > new-file
  it "matches git behavior" && {
    compat_effect "deferred until add driver lands" -- add new-file
  }
)

# mode=effect — `git add .` stages everything in the current directory.
# hash=sha1-only
title "gix add ."
only_for_hash sha1-only && (small-repo-in-sandbox
  echo content > new-file
  it "matches git behavior" && {
    compat_effect "deferred until add driver lands" -- add .
  }
)

# mode=effect — `git add -- <path>` is the explicit form.
# hash=sha1-only
title "gix add -- a"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until add driver lands" -- add -- a
  }
)

# mode=effect — `git add <missing-file>` errors at
# vendor/git/builtin/add.c:566..568 with "pathspec '<x>' did not match
# any files" and exits 128.
# hash=sha1-only
title "gix add missing-file"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until add driver lands" -- add missing-file
  }
)

# --- verbosity / dry-run ----------------------------------------------

# mode=effect — `-n` / `--dry-run` triggers ADD_CACHE_PRETEND. Per
# vendor/git/builtin/add.c:254 OPT__DRY_RUN + 488.
# hash=sha1-only
title "gix add -n a"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until add driver lands" -- add -n a
  }
)

# mode=effect — long form of -n.
# hash=sha1-only
title "gix add --dry-run a"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until add driver lands" -- add --dry-run a
  }
)

# mode=effect — `-v` / `--verbose` triggers ADD_CACHE_VERBOSE. Per
# vendor/git/builtin/add.c:255 OPT__VERBOSE + 487.
# hash=sha1-only
title "gix add -v a"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until add driver lands" -- add -v a
  }
)

# mode=effect — long form of -v.
# hash=sha1-only
title "gix add --verbose a"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until add driver lands" -- add --verbose a
  }
)

# --- force / sparse / update / all / no-all ---------------------------

# mode=effect — `-f` / `--force` allows adding ignored files. Per
# vendor/git/builtin/add.c:264 OPT__FORCE.
# hash=sha1-only
title "gix add -f a"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until add driver lands" -- add -f a
  }
)

# mode=effect — long form of -f.
# hash=sha1-only
title "gix add --force a"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until add driver lands" -- add --force a
  }
)

# mode=effect — `-u` / `--update` updates only tracked files. Per
# vendor/git/builtin/add.c:265 OPT_BOOL('u', "update", ...).
# hash=sha1-only
title "gix add -u"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until add driver lands" -- add -u
  }
)

# mode=effect — long form of -u.
# hash=sha1-only
title "gix add --update"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until add driver lands" -- add --update
  }
)

# mode=effect — `-A` / `--all` adds, modifies, removes index entries.
# Per vendor/git/builtin/add.c:268.
# hash=sha1-only
title "gix add -A"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until add driver lands" -- add -A
  }
)

# mode=effect — long form of -A.
# hash=sha1-only
title "gix add --all"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until add driver lands" -- add --all
  }
)

# mode=effect — `--no-all` (alias --ignore-removal). Per
# vendor/git/builtin/add.c:269.
# hash=sha1-only
title "gix add --no-all"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until add driver lands" -- add --no-all
  }
)

# mode=effect — long form of --no-all.
# hash=sha1-only
title "gix add --ignore-removal"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until add driver lands" -- add --ignore-removal
  }
)

# mode=effect — `--no-ignore-removal` is the inverted form of
# `--ignore-removal` (i.e., re-enables removals = same as -A).
# hash=sha1-only
title "gix add --no-ignore-removal"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until add driver lands" -- add --no-ignore-removal
  }
)

# mode=effect — `--sparse` allows updating index entries outside of
# the sparse-checkout cone. Per vendor/git/builtin/add.c:276.
# hash=sha1-only
title "gix add --sparse a"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until add driver lands" -- add --sparse a
  }
)

# --- intent-to-add ----------------------------------------------------

# mode=effect — `-N` / `--intent-to-add` triggers ADD_CACHE_INTENT.
# Per vendor/git/builtin/add.c:267 + 489.
# hash=sha1-only
title "gix add -N new-file"
only_for_hash sha1-only && (small-repo-in-sandbox
  echo content > new-file
  it "matches git behavior" && {
    compat_effect "deferred until add driver lands" -- add -N new-file
  }
)

# mode=effect — long form of -N.
# hash=sha1-only
title "gix add --intent-to-add new-file"
only_for_hash sha1-only && (small-repo-in-sandbox
  echo content > new-file
  it "matches git behavior" && {
    compat_effect "deferred until add driver lands" -- add --intent-to-add new-file
  }
)

# --- renormalize ------------------------------------------------------

# mode=effect — `--renormalize` re-applies the clean filter to every
# tracked file. Per vendor/git/builtin/add.c:266 + 587.
# hash=sha1-only
title "gix add --renormalize"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until add driver lands" -- add --renormalize
  }
)

# --- refresh / ignore-errors ------------------------------------------

# mode=effect — `--refresh` triggers the stat-only refresh path. Per
# vendor/git/builtin/add.c:273 + 515..517.
# hash=sha1-only
title "gix add --refresh a"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until add driver lands" -- add --refresh a
  }
)

# mode=effect — `--ignore-errors` keeps going on add failure. Per
# vendor/git/builtin/add.c:274.
# hash=sha1-only
title "gix add --ignore-errors a"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until add driver lands" -- add --ignore-errors a
  }
)

# --- chmod ------------------------------------------------------------

# mode=effect — `--chmod=+x` overrides the executable bit. Per
# vendor/git/builtin/add.c:277..278 + 600..601.
# hash=sha1-only
title "gix add --chmod=+x a"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until add driver lands" -- add --chmod=+x a
  }
)

# mode=effect — `--chmod=-x` clears the executable bit.
# hash=sha1-only
title "gix add --chmod=-x a"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until add driver lands" -- add --chmod=-x a
  }
)

# --- pathspec sources -------------------------------------------------

# mode=effect — `--pathspec-from-file=<file>` reads pathspec entries
# from a file. Per vendor/git/builtin/add.c:281 + 464..471.
# hash=sha1-only
title "gix add --pathspec-from-file=spec.txt"
only_for_hash sha1-only && (small-repo-in-sandbox
  echo a > spec.txt
  it "matches git behavior" && {
    compat_effect "deferred until add driver lands" -- add --pathspec-from-file=spec.txt
  }
)

# mode=effect — `--pathspec-file-nul` is only meaningful with
# `--pathspec-from-file`; pairing alone dies 128 at
# vendor/git/builtin/add.c:472..474. Already gated in the porcelain
# stub; combined-flag form deferred.
# hash=sha1-only
title "gix add --pathspec-from-file=spec.txt --pathspec-file-nul"
only_for_hash sha1-only && (small-repo-in-sandbox
  printf 'a\0' > spec.txt
  it "matches git behavior" && {
    compat_effect "deferred until add driver lands" -- add --pathspec-from-file=spec.txt --pathspec-file-nul
  }
)

# --- interactive / patch / edit ---------------------------------------

# mode=effect — `-i` / `--interactive` opens the interactive add menu
# (vendor/git/builtin/add.c:257 + 412..417). Without a TTY (test
# fixture context) git's interactive prompt aborts immediately. gix's
# placeholder accepts the flag and exits 0.
# hash=sha1-only
title "gix add -i"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until add driver lands" -- add -i
  }
)

# mode=effect — long form of -i.
# hash=sha1-only
title "gix add --interactive"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until add driver lands" -- add --interactive
  }
)

# mode=effect — `-p` / `--patch` triggers the interactive add-p
# replay (vendor/git/builtin/add.c:258 + 410..417).
# hash=sha1-only
title "gix add -p"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until add driver lands" -- add -p
  }
)

# mode=effect — long form of -p.
# hash=sha1-only
title "gix add --patch"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until add driver lands" -- add --patch
  }
)

# mode=effect — `-e` / `--edit` opens the diff in EDITOR (per
# vendor/git/builtin/add.c:263 + 427..431). The test fixture sets
# EDITOR=true (tests/utilities.sh set-static-git-environment) so the
# editor exits 0 without touching the patch. To avoid git's "empty
# patch. aborted" 128 path (vendor/git/apply.c::parse_chunk dies
# when there are zero hunks), the fixture mutates `a` so the diff has
# content. EDITOR=true preserves the patch verbatim, git apply
# replays it cleanly, both binaries exit 0.
# hash=sha1-only
title "gix add -e a"
only_for_hash sha1-only && (small-repo-in-sandbox
  echo modified >> a
  it "matches git behavior" && {
    compat_effect "deferred until add driver lands" -- add -e a
  }
)

# mode=effect — long form of -e.
# hash=sha1-only
title "gix add --edit a"
only_for_hash sha1-only && (small-repo-in-sandbox
  echo modified >> a
  it "matches git behavior" && {
    compat_effect "deferred until add driver lands" -- add --edit a
  }
)

# mode=effect — `--auto-advance` only meaningful under
# `--interactive` / `--patch`. Per vendor/git/builtin/add.c:259.
# system git 2.47.3 predates the auto-advance / unified /
# inter-hunk-context additions to add's parse-options table; vendor/
# git v2.54.0 has them. Row reactivates when CI git catches up with
# vendor/git.
# hash=sha1-only
title "gix add --auto-advance --patch"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    shortcoming "system git 2.47.3 lacks --auto-advance; vendor/git v2.54.0 has it"
  }
)

# mode=effect — `-U <n>` / `--unified=<n>` requires `--interactive` /
# `--patch`. Per vendor/git/builtin/add.c:261 + 419..420. Same
# version skew as `--auto-advance` (the OPT_DIFF_UNIFIED line lands
# in v2.54.0).
# hash=sha1-only
title "gix add -U 3 --patch"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    shortcoming "system git 2.47.3 lacks -U for add; vendor/git v2.54.0 has it"
  }
)

# mode=effect — long form of -U. Same version skew.
# hash=sha1-only
title "gix add --unified=3 --patch"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    shortcoming "system git 2.47.3 lacks --unified for add; vendor/git v2.54.0 has it"
  }
)

# mode=effect — `--inter-hunk-context=<n>` requires `--interactive` /
# `--patch`. Per vendor/git/builtin/add.c:262 + 421..422. Same
# version skew as `--auto-advance`.
# hash=sha1-only
title "gix add --inter-hunk-context=2 --patch"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    shortcoming "system git 2.47.3 lacks --inter-hunk-context for add; vendor/git v2.54.0 has it"
  }
)

# --- ignore-missing (dry-run companion) -------------------------------

# mode=effect — `--ignore-missing` requires `--dry-run` (per
# vendor/git/builtin/add.c:443..444). The pairing closes as deferred
# until the dry-run path is wired.
# hash=sha1-only
title "gix add --dry-run --ignore-missing missing-file"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until add driver lands" -- add --dry-run --ignore-missing missing-file
  }
)

# --- precondition gates ----------------------------------------------

# mode=bytes — `-A -u` errors at vendor/git/builtin/add.c:440..441
# ("options '-A' and '-u' cannot be used together"). gix's porcelain
# stub mirrors the verbatim wording.
# hash=sha1-only
title "gix add -A -u"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    expect_parity bytes -- add -A -u
  }
)

# mode=bytes — `--ignore-missing` without `--dry-run` errors at
# vendor/git/builtin/add.c:443..444 ("the option '--ignore-missing'
# requires '--dry-run'"). gix's porcelain stub mirrors the verbatim
# wording.
# hash=sha1-only
title "gix add --ignore-missing"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    expect_parity bytes -- add --ignore-missing
  }
)

# mode=bytes — `--chmod=foo` (invalid arg) errors at
# vendor/git/builtin/add.c:446..448 ("--chmod param 'foo' must be
# either -x or +x"). gix's porcelain stub mirrors the verbatim wording.
# hash=sha1-only
title "gix add --chmod=foo a"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    expect_parity bytes -- add --chmod=foo a
  }
)

# mode=bytes — `--pathspec-from-file=spec.txt -- a` errors at
# vendor/git/builtin/add.c:464..466 ("'--pathspec-from-file' and
# pathspec arguments cannot be used together"). gix's porcelain stub
# mirrors the verbatim wording.
# hash=sha1-only
title "gix add --pathspec-from-file=spec.txt -- a"
only_for_hash sha1-only && (small-repo-in-sandbox
  echo a > spec.txt
  it "matches git behavior" && {
    expect_parity bytes -- add --pathspec-from-file=spec.txt -- a
  }
)

# mode=bytes — `--pathspec-file-nul` without `--pathspec-from-file` is
# an error (vendor/git/builtin/add.c:472..474). gix's porcelain stub
# mirrors the verbatim wording.
# hash=sha1-only
title "gix add --pathspec-file-nul"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    expect_parity bytes -- add --pathspec-file-nul
  }
)

# mode=bytes — `--pathspec-from-file=spec.txt --interactive` errors at
# vendor/git/builtin/add.c:415..416 ("options '--pathspec-from-file'
# and '--interactive/--patch' cannot be used together"). gix's
# porcelain stub mirrors the verbatim wording.
# hash=sha1-only
title "gix add --pathspec-from-file=spec.txt --interactive"
only_for_hash sha1-only && (small-repo-in-sandbox
  echo a > spec.txt
  it "matches git behavior" && {
    expect_parity bytes -- add --pathspec-from-file=spec.txt --interactive
  }
)

# mode=bytes — `--dry-run --interactive` errors at
# vendor/git/builtin/add.c:413..414 ("options '--dry-run' and
# '--interactive/--patch' cannot be used together"). gix's porcelain
# stub mirrors the verbatim wording.
# hash=sha1-only
title "gix add --dry-run --interactive"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    expect_parity bytes -- add --dry-run --interactive
  }
)

# mode=bytes — `--pathspec-from-file=spec.txt --edit` errors at
# vendor/git/builtin/add.c:428..429 ("options '--pathspec-from-file'
# and '--edit' cannot be used together"). gix's porcelain stub mirrors
# the verbatim wording.
# hash=sha1-only
title "gix add --pathspec-from-file=spec.txt --edit"
only_for_hash sha1-only && (small-repo-in-sandbox
  echo a > spec.txt
  it "matches git behavior" && {
    expect_parity bytes -- add --pathspec-from-file=spec.txt --edit
  }
)

# mode=bytes — `--unified=3` without `--interactive`/`--patch` errors
# at vendor/git/builtin/add.c:419..420 ("the option '--unified'
# requires '--interactive/--patch'"). gix's porcelain stub mirrors the
# verbatim wording. Same version skew as `--auto-advance` (system git
# 2.47.3 emits a 129 unknown-switch error before reaching the
# precondition gate; vendor/git v2.54.0 reaches the gate).
# hash=sha1-only
title "gix add --unified=3"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    shortcoming "system git 2.47.3 lacks --unified for add; vendor/git v2.54.0 has it"
  }
)

# mode=bytes — `--inter-hunk-context=2` without `--interactive` /
# `--patch` errors at vendor/git/builtin/add.c:421..422 ("the option
# '--inter-hunk-context' requires '--interactive/--patch'"). Same
# version skew as `--auto-advance`.
# hash=sha1-only
title "gix add --inter-hunk-context=2"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    shortcoming "system git 2.47.3 lacks --inter-hunk-context for add; vendor/git v2.54.0 has it"
  }
)

# mode=bytes — `--no-auto-advance` without `--interactive` /
# `--patch` errors at vendor/git/builtin/add.c:423..424 ("the option
# '--no-auto-advance' requires '--interactive/--patch'"). Same
# version skew as `--auto-advance`.
# hash=sha1-only
title "gix add --no-auto-advance"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    shortcoming "system git 2.47.3 lacks --no-auto-advance; vendor/git v2.54.0 has it"
  }
)

# End-of-file sentinel: when every row is `only_for_hash sha1-only` and the
# active hash is sha256, the last statement returns 1 (skip), which would
# propagate out of `source` and trip `set -e` in tests/parity.sh. A trailing
# `:` normalizes the exit code so a fully-skipped file still returns 0.
:
