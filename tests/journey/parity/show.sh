# Must be sourced into tests/parity.sh or tests/journey.sh — see tests/parity.sh.
#
# Parity scaffold for `git show` ↔ `gix show`.
#
# One `title` + `it` block per flag derived from
# vendor/git/Documentation/git-show.adoc and
# vendor/git/builtin/log.c::cmd_show (entry at
# vendor/git/builtin/log.c:657). `cmd_show` calls `cmd_log_init` which
# loads the same flag table as `git log`, so the user-visible surface
# is the union of `pretty-options.adoc`, `diff-options.adoc`, and
# `diff-generate-patch.adoc`. The semantic difference is `rev.no_walk
# = 1` (no ancestry walk) and `rev.diff = 1` plus a per-object switch
# that renders blob / tag / tree / commit. Every `it` body starts as
# a TODO placeholder — iteration N of the ralph loop picks the next
# TODO, converts it to a real `expect_parity` (or `compat_effect`)
# assertion, and removes the TODO marker.
#
# Verdict modes (comment above each block):
#   bytes  — scriptable output: precondition error stanzas around
#            unborn HEAD, ambiguous arguments, and the verbatim
#            "fatal: ambiguous argument..." 3-line stanza emitted by
#            vendor/git/revision.c::handle_revision_arg.
#   effect — UX-level parity (exit-code match + optional prose check).
#            Default for the human-rendered flags whose pretty
#            rendering is not yet implemented in gix's show entry
#            point. Most rows close as `compat_effect` until the show
#            driver lands.
#
# Coverage on gix's current Clap surface (src/plumbing/options/show.rs):
#   gix show [OPTIONS] [<object>...]
# Today the porcelain driver is a placeholder
# (gitoxide-core/src/repository/show.rs::porcelain) that emits a stub
# note on stdout and exits 0, *except* for two precondition gates: the
# unborn-HEAD path emits git's verbatim "fatal: your current branch
# '<name>' does not have any commits yet" + exit 128 (matching
# vendor/git/builtin/log.c → cmd_log_walk → revision.c::die), and the
# bad-revspec path emits the verbatim 3-line ambiguous-argument
# stanza on stderr + exit 128 (matching
# vendor/git/revision.c::handle_revision_arg). The flag surface is
# clap-wired so `gix show <flag> ...` does not trip UnknownArgument;
# every flag-bearing row therefore closes as `compat_effect "<reason>"`
# under the shared deferral phrase "deferred until show driver lands"
# until the real driver implements the semantic. Closing this command
# requires (1) implementing the show driver in
# gitoxide-core/src/repository/show.rs (per-object dispatch on blob /
# tag / tree / commit; pretty-format and notes emission for commits;
# diff emission with `dense-combined` as the default for merges per
# git-show.adoc:54; tag header "tag <name>\n" framing and recursive
# tagged-object resolution; tree "tree <name>\n\n" + ls-tree --name-only
# emission; blob verbatim content), (2) translating C-side invariants
# in vendor/git/builtin/log.c::cmd_show (the OBJ_TAG recursion at
# L725-727 that re-pushes the tagged object onto the pending stack,
# the rev.shown_one preamble newline at L711-712 / L730-731, the
# per-object array swap-and-restore around cmd_log_walk_no_free at
# L744-757 for OBJ_COMMIT to keep the walk single-object).
#
# Hash coverage: `dual` rows never open a repo (--help, outside-of-repo,
# --bogus-flag pre-repo dispatch). Every row that opens a repository
# is `sha1-only` because gix-config rejects
# `extensions.objectFormat=sha256`
# (gix/src/config/tree/sections/extensions.rs try_into_object_format,
# sha1-only validator), blocking every sha256 fixture at open. Rows
# flip to `dual` once that validator accepts sha256.
#
# parity-defaults:
#   hash=sha1-only "gix cannot load sha256 repos: extensions.objectFormat=sha256 rejected (gix/src/config/tree/sections/extensions.rs)"
#   mode=effect

# --- meta / help --------------------------------------------------------

# mode=effect — clap --help short-circuits before repo load, exits 0.
# git's --help delegates to `man git-show` (exit 0 when man is
# available); gix returns clap's auto-generated help. Message text
# diverges; only the exit-code match is asserted.
# hash=dual
title "gix show --help"
only_for_hash dual && (sandbox
  it "matches git behavior" && {
    expect_parity effect -- show --help
  }
)

# mode=effect — unknown flag: git exits 129 (usage_msg_opt in
# parse-options.c). gix's Clap layer maps UnknownArgument to 129 via
# src/plumbing/main.rs. Tested inside a repo because git outside a
# repo dies 128 before reaching arg-parse, while clap in gix always
# runs first.
# hash=sha1-only
title "gix show --bogus-flag"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    expect_parity effect -- show --bogus-flag
  }
)

# mode=effect — outside any repo: git dies 128 with "fatal: not a git
# repository (or any of the parent directories): .git". gix's plumbing
# repository() closure already remaps gix_discover::upwards::Error::
# NoGitRepository* variants to git's exact wording + exit 128.
# hash=dual
title "gix show (outside a repository)"
only_for_hash dual && (sandbox
  it "matches git behavior" && {
    expect_parity effect -- show
  }
)

# --- synopsis: object resolution ---------------------------------------

# mode=effect — bare `git show` with HEAD pointing at a commit: git
# emits the medium pretty-format header + diff. gix's placeholder
# emits a stub note + exits 0; exit-code parity holds.
# hash=sha1-only
title "gix show (default HEAD)"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until show driver lands" -- show
  }
)

# mode=bytes — `git show` in an unborn-HEAD repo (fresh `git init`):
# git dies 128 with "fatal: your current branch '<name>' does not
# have any commits yet" via revision.c::die. gix's porcelain matches
# byte-exactly (see show.rs unborn-HEAD branch).
# hash=sha1-only
title "gix show (unborn HEAD)"
only_for_hash sha1-only && (sandbox
  it "matches git behavior" && {
    expect_parity bytes -- show
  }
)

# mode=effect — `git show <ref>` resolves to commit, emits commit.
# gix placeholder: stub note + exit 0.
# hash=sha1-only
title "gix show <ref>"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until show driver lands" -- show main
  }
)

# mode=effect — `git show <sha>` resolves a full hash to its object.
# gix placeholder: stub note + exit 0.
# hash=sha1-only
title "gix show <sha>"
only_for_hash sha1-only && (small-repo-in-sandbox
  sha=$(git rev-parse HEAD)
  it "matches git behavior" && {
    compat_effect "deferred until show driver lands" -- show "$sha"
  }
)

# mode=bytes — `git show <unknown>` dies 128 with the verbatim
# 3-line "fatal: ambiguous argument..." stanza from
# vendor/git/revision.c::handle_revision_arg. gix's porcelain matches
# byte-exactly.
# hash=sha1-only
title "gix show <unknown-ref>"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    expect_parity bytes -- show no-such-ref-exists
  }
)

# mode=effect — `git show <tag>` recurses through OBJ_TAG to render
# tag header + tagged object. gix placeholder: stub note + exit 0.
# hash=sha1-only
title "gix show <tag>"
only_for_hash sha1-only && (small-repo-in-sandbox
  git tag -m "anno" v0.1 HEAD
  it "matches git behavior" && {
    compat_effect "deferred until show driver lands" -- show v0.1
  }
)

# mode=effect — `git show <commit>^{tree}` resolves to a tree,
# renders "tree <name>\n\n" + ls-tree --name-only output.
# gix placeholder: stub note + exit 0.
# hash=sha1-only
title "gix show <tree>"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until show driver lands" -- show 'HEAD^{tree}'
  }
)

# mode=effect — `git show <commit>:<path>` resolves to a blob, emits
# blob bytes verbatim. gix placeholder: stub note + exit 0.
# hash=sha1-only
title "gix show <blob>"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until show driver lands" -- show 'HEAD:a'
  }
)

# mode=effect — multiple objects: git renders each with shown_one
# blank-line separator (vendor/git/builtin/log.c:711-712). gix
# placeholder: stub note + exit 0.
# hash=sha1-only
title "gix show <obj1> <obj2>"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until show driver lands" -- show HEAD HEAD
  }
)

# --- pretty-options.adoc -----------------------------------------------

# hash=sha1-only
title "gix show --pretty"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until show driver lands" -- show --pretty
  }
)

# hash=sha1-only
title "gix show --pretty=oneline"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until show driver lands" -- show --pretty=oneline
  }
)

# hash=sha1-only
title "gix show --format=%H"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until show driver lands" -- show --format=%H
  }
)

# hash=sha1-only
title "gix show --abbrev-commit"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until show driver lands" -- show --abbrev-commit
  }
)

# hash=sha1-only
title "gix show --no-abbrev-commit"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until show driver lands" -- show --no-abbrev-commit
  }
)

# hash=sha1-only
title "gix show --oneline"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until show driver lands" -- show --oneline
  }
)

# hash=sha1-only
title "gix show --encoding"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until show driver lands" -- show --encoding=UTF-8
  }
)

# hash=sha1-only
title "gix show --expand-tabs"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until show driver lands" -- show --expand-tabs
  }
)

# hash=sha1-only
title "gix show --no-expand-tabs"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until show driver lands" -- show --no-expand-tabs
  }
)

# hash=sha1-only
title "gix show --notes"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until show driver lands" -- show --notes
  }
)

# hash=sha1-only
title "gix show --no-notes"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until show driver lands" -- show --no-notes
  }
)

# hash=sha1-only
title "gix show --show-notes-by-default"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until show driver lands" -- show --show-notes-by-default
  }
)

# hash=sha1-only
title "gix show --show-signature"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until show driver lands" -- show --show-signature
  }
)

# --- diff-options / diff-generate-patch.adoc ---------------------------

# hash=sha1-only
title "gix show -p"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until show driver lands" -- show -p
  }
)

# hash=sha1-only
title "gix show --patch"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until show driver lands" -- show --patch
  }
)

# hash=sha1-only
title "gix show -s"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until show driver lands" -- show -s
  }
)

# hash=sha1-only
title "gix show --no-patch"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until show driver lands" -- show --no-patch
  }
)

# hash=sha1-only
title "gix show -U<n>"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until show driver lands" -- show -U5
  }
)

# hash=sha1-only
title "gix show --unified=<n>"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until show driver lands" -- show --unified=5
  }
)

# hash=sha1-only
title "gix show --output"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until show driver lands" -- show --output=out.txt HEAD
  }
)

# hash=sha1-only
title "gix show --output-indicator-new"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until show driver lands" -- show --output-indicator-new=N
  }
)

# hash=sha1-only
title "gix show --output-indicator-old"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until show driver lands" -- show --output-indicator-old=O
  }
)

# hash=sha1-only
title "gix show --output-indicator-context"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until show driver lands" -- show --output-indicator-context=C
  }
)

# hash=sha1-only
title "gix show --raw"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until show driver lands" -- show --raw
  }
)

# hash=sha1-only
title "gix show --patch-with-raw"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until show driver lands" -- show --patch-with-raw
  }
)

# hash=sha1-only
title "gix show --indent-heuristic"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until show driver lands" -- show --indent-heuristic
  }
)

# hash=sha1-only
title "gix show --no-indent-heuristic"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until show driver lands" -- show --no-indent-heuristic
  }
)

# hash=sha1-only
title "gix show --minimal"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until show driver lands" -- show --minimal
  }
)

# hash=sha1-only
title "gix show --patience"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until show driver lands" -- show --patience
  }
)

# hash=sha1-only
title "gix show --histogram"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until show driver lands" -- show --histogram
  }
)

# hash=sha1-only
title "gix show --anchored"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until show driver lands" -- show --anchored=foo
  }
)

# hash=sha1-only
title "gix show --diff-algorithm"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until show driver lands" -- show --diff-algorithm=histogram
  }
)

# hash=sha1-only
title "gix show --stat"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until show driver lands" -- show --stat
  }
)

# hash=sha1-only
title "gix show --numstat"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until show driver lands" -- show --numstat
  }
)

# hash=sha1-only
title "gix show --shortstat"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until show driver lands" -- show --shortstat
  }
)

# hash=sha1-only
title "gix show --compact-summary"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until show driver lands" -- show --compact-summary
  }
)

# hash=sha1-only
title "gix show --dirstat"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until show driver lands" -- show --dirstat
  }
)

# hash=sha1-only
title "gix show --cumulative"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until show driver lands" -- show --cumulative
  }
)

# hash=sha1-only
title "gix show --summary"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until show driver lands" -- show --summary
  }
)

# hash=sha1-only
title "gix show --patch-with-stat"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until show driver lands" -- show --patch-with-stat
  }
)

# hash=sha1-only
title "gix show -z"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until show driver lands" -- show -z
  }
)

# hash=sha1-only
title "gix show --name-only"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until show driver lands" -- show --name-only
  }
)

# hash=sha1-only
title "gix show --name-status"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until show driver lands" -- show --name-status
  }
)

# hash=sha1-only
title "gix show --submodule"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until show driver lands" -- show --submodule=log
  }
)

# hash=sha1-only
title "gix show --color"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until show driver lands" -- show --color=always
  }
)

# hash=sha1-only
title "gix show --no-color"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until show driver lands" -- show --no-color
  }
)

# hash=sha1-only
title "gix show --color-moved"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until show driver lands" -- show --color-moved
  }
)

# hash=sha1-only
title "gix show --no-color-moved"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until show driver lands" -- show --no-color-moved
  }
)

# hash=sha1-only
title "gix show --color-moved-ws"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until show driver lands" -- show --color-moved-ws=allow-indentation-change
  }
)

# hash=sha1-only
title "gix show --no-color-moved-ws"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until show driver lands" -- show --no-color-moved-ws
  }
)

# hash=sha1-only
title "gix show --word-diff"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until show driver lands" -- show --word-diff
  }
)

# hash=sha1-only
title "gix show --word-diff-regex"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until show driver lands" -- show --word-diff-regex='\\w+'
  }
)

# hash=sha1-only
title "gix show --color-words"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until show driver lands" -- show --color-words
  }
)

# hash=sha1-only
title "gix show --no-renames"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until show driver lands" -- show --no-renames
  }
)

# hash=sha1-only
title "gix show --rename-empty"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until show driver lands" -- show --rename-empty
  }
)

# hash=sha1-only
title "gix show --no-rename-empty"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until show driver lands" -- show --no-rename-empty
  }
)

# hash=sha1-only
title "gix show --check"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until show driver lands" -- show --check
  }
)

# hash=sha1-only
title "gix show --ws-error-highlight"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until show driver lands" -- show --ws-error-highlight=all
  }
)

# hash=sha1-only
title "gix show --full-index"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until show driver lands" -- show --full-index
  }
)

# hash=sha1-only
title "gix show --binary"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until show driver lands" -- show --binary
  }
)

# hash=sha1-only
title "gix show --abbrev"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until show driver lands" -- show --abbrev=12
  }
)

# hash=sha1-only
title "gix show -B"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until show driver lands" -- show -B
  }
)

# hash=sha1-only
title "gix show -M"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until show driver lands" -- show -M
  }
)

# hash=sha1-only
title "gix show --find-renames"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until show driver lands" -- show --find-renames=80
  }
)

# hash=sha1-only
title "gix show -C"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until show driver lands" -- show -C
  }
)

# hash=sha1-only
title "gix show --find-copies"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until show driver lands" -- show --find-copies=80
  }
)

# hash=sha1-only
title "gix show --find-copies-harder"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until show driver lands" -- show --find-copies-harder
  }
)

# hash=sha1-only
title "gix show -D"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until show driver lands" -- show -D
  }
)

# hash=sha1-only
title "gix show --irreversible-delete"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until show driver lands" -- show --irreversible-delete
  }
)

# hash=sha1-only
title "gix show -l"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until show driver lands" -- show -l 200
  }
)

# hash=sha1-only
title "gix show --diff-filter"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until show driver lands" -- show --diff-filter=AM
  }
)

# hash=sha1-only
title "gix show -S"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until show driver lands" -- show -S foo
  }
)

# hash=sha1-only
title "gix show -G"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until show driver lands" -- show -G foo
  }
)

# hash=sha1-only
title "gix show --find-object"
only_for_hash sha1-only && (small-repo-in-sandbox
  oid=$(git rev-parse HEAD)
  it "matches git behavior" && {
    compat_effect "deferred until show driver lands" -- show --find-object="$oid"
  }
)

# hash=sha1-only
title "gix show --pickaxe-all"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until show driver lands" -- show -S foo --pickaxe-all
  }
)

# hash=sha1-only
title "gix show --pickaxe-regex"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until show driver lands" -- show -S '.*' --pickaxe-regex
  }
)

# hash=sha1-only
title "gix show -O"
only_for_hash sha1-only && (small-repo-in-sandbox
  : >ord
  it "matches git behavior" && {
    compat_effect "deferred until show driver lands" -- show -O ord
  }
)

# hash=sha1-only
title "gix show --skip-to"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until show driver lands" -- show --skip-to=a
  }
)

# hash=sha1-only
title "gix show --rotate-to"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until show driver lands" -- show --rotate-to=a
  }
)

# hash=sha1-only
title "gix show -R"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until show driver lands" -- show -R
  }
)

# hash=sha1-only
title "gix show --relative"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until show driver lands" -- show --relative
  }
)

# hash=sha1-only
title "gix show --no-relative"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until show driver lands" -- show --no-relative
  }
)

# hash=sha1-only
title "gix show -a"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until show driver lands" -- show -a
  }
)

# hash=sha1-only
title "gix show --text"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until show driver lands" -- show --text
  }
)

# hash=sha1-only
title "gix show --ignore-cr-at-eol"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until show driver lands" -- show --ignore-cr-at-eol
  }
)

# hash=sha1-only
title "gix show --ignore-space-at-eol"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until show driver lands" -- show --ignore-space-at-eol
  }
)

# hash=sha1-only
title "gix show -b"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until show driver lands" -- show -b
  }
)

# hash=sha1-only
title "gix show --ignore-space-change"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until show driver lands" -- show --ignore-space-change
  }
)

# hash=sha1-only
title "gix show -w"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until show driver lands" -- show -w
  }
)

# hash=sha1-only
title "gix show --ignore-all-space"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until show driver lands" -- show --ignore-all-space
  }
)

# hash=sha1-only
title "gix show --ignore-blank-lines"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until show driver lands" -- show --ignore-blank-lines
  }
)

# hash=sha1-only
title "gix show --ignore-matching-lines"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until show driver lands" -- show --ignore-matching-lines='^#'
  }
)

# hash=sha1-only
title "gix show -I"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until show driver lands" -- show -I '^#'
  }
)

# hash=sha1-only
title "gix show --inter-hunk-context"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until show driver lands" -- show --inter-hunk-context=2
  }
)

# hash=sha1-only
title "gix show -W"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until show driver lands" -- show -W
  }
)

# hash=sha1-only
title "gix show --function-context"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until show driver lands" -- show --function-context
  }
)

# hash=sha1-only
title "gix show --exit-code"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    # Run against a tree (which produces no diff) so the diff-presence
    # exit-code bit doesn't fire; gix's stub also exits 0 here.
    compat_effect "deferred until show driver lands" -- show --exit-code 'HEAD^{tree}'
  }
)

# hash=sha1-only
title "gix show --quiet"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until show driver lands" -- show --quiet
  }
)

# hash=sha1-only
title "gix show --ext-diff"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until show driver lands" -- show --ext-diff
  }
)

# hash=sha1-only
title "gix show --no-ext-diff"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until show driver lands" -- show --no-ext-diff
  }
)

# hash=sha1-only
title "gix show --textconv"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until show driver lands" -- show --textconv
  }
)

# hash=sha1-only
title "gix show --no-textconv"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until show driver lands" -- show --no-textconv
  }
)

# hash=sha1-only
title "gix show --ignore-submodules"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until show driver lands" -- show --ignore-submodules
  }
)

# hash=sha1-only
title "gix show --src-prefix"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until show driver lands" -- show --src-prefix=A/
  }
)

# hash=sha1-only
title "gix show --dst-prefix"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until show driver lands" -- show --dst-prefix=B/
  }
)

# hash=sha1-only
title "gix show --no-prefix"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until show driver lands" -- show --no-prefix
  }
)

# hash=sha1-only
title "gix show --default-prefix"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until show driver lands" -- show --default-prefix
  }
)

# hash=sha1-only
title "gix show --line-prefix"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until show driver lands" -- show --line-prefix=PRE
  }
)

# hash=sha1-only
title "gix show --ita-invisible-in-index"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until show driver lands" -- show --ita-invisible-in-index
  }
)

# --- merge-diff family (git-show defaults dense-combined) --------------

# hash=sha1-only
title "gix show --diff-merges"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until show driver lands" -- show --diff-merges=combined
  }
)

# hash=sha1-only
title "gix show --no-diff-merges"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until show driver lands" -- show --no-diff-merges
  }
)

# hash=sha1-only
title "gix show -c"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until show driver lands" -- show -c
  }
)

# hash=sha1-only
title "gix show --cc"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until show driver lands" -- show --cc
  }
)

# hash=sha1-only
title "gix show -m"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until show driver lands" -- show -m
  }
)

# hash=sha1-only
title "gix show --combined-all-paths"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    # git rejects --combined-all-paths without -c/--cc/-m: must pair with one.
    compat_effect "deferred until show driver lands" -- show -c --combined-all-paths
  }
)

# hash=sha1-only
title "gix show --remerge-diff"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until show driver lands" -- show --remerge-diff
  }
)

# hash=sha1-only
title "gix show -t"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until show driver lands" -- show -t
  }
)

# hash=sha1-only
title "gix show --dd"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until show driver lands" -- show --dd
  }
)

# End-of-file sentinel: when every row is `only_for_hash sha1-only` and the
# active hash is sha256, the last statement returns 1 (skip), which would
# propagate out of `source` and trip `set -e` in tests/parity.sh. A trailing
# `:` normalizes the exit code so a fully-skipped file still returns 0.
:
