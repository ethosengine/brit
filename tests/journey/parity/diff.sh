# Must be sourced into tests/parity.sh or tests/journey.sh — see tests/parity.sh.
#
# Parity scaffold for `git diff` ↔ `gix diff`.
#
# One `title` + `it` block per flag derived from
# vendor/git/Documentation/git-diff.adoc and the inherited
# include::diff-options.adoc surface, plus vendor/git/builtin/diff.c
# (cmd_diff). Every `it` body starts as a TODO: placeholder — iteration
# N of the ralph loop picks the next TODO, converts it to a real
# `expect_parity` (or `compat_effect`) assertion, and removes the TODO
# marker.
#
# Verdict modes (comment above each block):
#   bytes  — scriptable output consumed by tooling: --raw, --name-only,
#            --name-status, --numstat, --shortstat, --diff-filter, the
#            error stanzas around bad revspecs / blob-vs-blob mismatches.
#   effect — UX-level parity (exit-code match + optional prose check).
#            Default for output-format flags whose pretty/patch
#            rendering is not yet implemented in gix-diff (the bulk).
#
# Coverage on gix's current Clap surface (src/plumbing/options/mod.rs::diff):
#   Subcommands::Diff(diff::Platform { cmd: SubCommands })
#     SubCommands::Tree { old_treeish, new_treeish }
#     SubCommands::File { old_revspec, new_revspec }
# That's a plumbing-only shape: bare `gix diff` errors with
# "missing required <COMMAND>", and every git-diff flag trips Clap's
# UnknownArgument before it reaches a handler. Closing this command
# requires (1) reshaping `Diff(diff::Platform)` into a flag-bearing
# top-level struct mirroring git's git-diff invocation forms (working
# tree vs index, --cached, two-dot/three-dot ranges, --no-index,
# --merge-base, blob-vs-blob), with the existing Tree/File subcommands
# either kept as cmdmode-style escape hatches or relocated to a
# different plumbing subcommand (gix diff-tree, gix diff-files in git's
# own plumbing taxonomy); (2) wiring the porcelain flow in
# gitoxide_core::repository::diff: index-vs-worktree (diff-files), tree-
# vs-index (diff-index), tree-vs-tree (diff-tree), tree-vs-worktree, the
# combined-diff variants, and the no-index file-vs-file path; (3)
# translating C-side invariants in vendor/git/builtin/diff.c
# (cmd_diff's classifier on rev.pending objects: N trees / M blobs / P
# pathspecs → which builtin_diff_* path runs).
#
# Hash coverage: `dual` rows never open a repo (--help, outside-of-repo,
# --bogus-flag pre-repo). Every row that opens a repository is
# `sha1-only` because gix-config rejects `extensions.objectFormat=sha256`
# (gix/src/config/tree/sections/extensions.rs try_into_object_format,
# sha1-only validator). Rows flip to `dual` once that validator accepts
# sha256.
#
# parity-defaults:
#   hash=sha1-only "gix cannot load sha256 repos: extensions.objectFormat=sha256 rejected (gix/src/config/tree/sections/extensions.rs)"
#   mode=effect

title "gix diff"

# --- meta / help --------------------------------------------------------

# mode=effect — clap --help short-circuits before repo load, exits 0.
# git's --help delegates to `man git-diff`; gix returns clap's auto-
# generated help. Message text diverges; only the exit-code match is
# asserted.
# hash=dual
title "gix diff --help"
only_for_hash dual && (sandbox
  it "matches git behavior" && {
    expect_parity effect -- diff --help
  }
)

# mode=effect — unknown flag: git exits 129 (usage_msg_opt). gix's Clap
# layer maps UnknownArgument to 129 via src/plumbing/main.rs.
# hash=sha1-only
title "gix diff --bogus-flag"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    expect_parity effect -- diff --bogus-flag
  }
)

# mode=effect — `git diff` (bare, no args) outside any repo emits
# "warning: Not a git repository. Use --no-index to compare two paths
# outside a working tree" + the usage stanza, then exits 129 via
# usage_msg_opt (vendor/git/builtin/diff.c falls through to the
# no-index usage path with zero paths). gix dispatches the bare form
# in src/plumbing/main.rs Subcommands::Diff(None): a manual
# gix::discover::upwards() check intercepts the NoGitRepository case
# before the standard repository() closure (which exits 128) is
# called, so the bare-diff path can emit 129 verbatim.
# hash=dual
title "gix diff (outside a repository)"
only_for_hash dual && (sandbox
  it "matches git behavior" && {
    expect_parity effect -- diff
  }
)

# --- synopsis forms -----------------------------------------------------

# mode=effect — bare `gix diff`: working-tree vs index. Default form,
# diff-files path in builtin/diff.c. Empty repo / clean working-tree
# exits 0 with no output.
# hash=sha1-only
title "gix diff (no args, clean working tree)"
only_for_hash sha1-only && (small-repo-in-sandbox
  # TODO — expect_parity effect -- diff
  it "matches git behavior" && { :; }
)

# mode=effect — bare `gix diff` with a modified tracked file. Default
# patch output. Currently gix-diff has no working-tree-vs-index path;
# this row will likely close as compat_effect with a "diff worktree-
# vs-index pending" reason until the diff-files primitive lands.
# hash=sha1-only
title "gix diff (no args, dirty working tree)"
only_for_hash sha1-only && (small-repo-in-sandbox
  # TODO — expect_parity effect -- diff (after touching a tracked file)
  it "matches git behavior" && { :; }
)

# mode=effect — `gix diff <commit>`: working-tree vs <commit>. diff-
# index path in builtin/diff.c (option without --cached).
# hash=sha1-only
title "gix diff <commit>"
only_for_hash sha1-only && (small-repo-in-sandbox
  # TODO — expect_parity effect -- diff HEAD
  it "matches git behavior" && { :; }
)

# mode=effect — `gix diff <commit> <commit>`: tree-vs-tree (diff-tree).
# This maps onto the existing `gix diff tree` subcommand semantics
# but with the porcelain positional shape.
# hash=sha1-only
title "gix diff <commit> <commit>"
only_for_hash sha1-only && (small-repo-in-sandbox
  # TODO — expect_parity effect -- diff HEAD~1 HEAD
  it "matches git behavior" && { :; }
)

# mode=bytes — `git diff <unknown-rev>`: setup_revisions dies 128 with
# the standard ambiguous-argument 3-line stanza. gix-diff's revspec
# resolution must mirror that wording for byte parity.
# hash=sha1-only
title "gix diff <unknown-rev>"
only_for_hash sha1-only && (small-repo-in-sandbox
  # TODO — expect_parity bytes -- diff bogus-rev-name
  it "matches git behavior" && { :; }
)

# mode=effect — `gix diff <commit>..<commit>`: two-dot range (synonym
# for two-arg form). gix_revision::Spec::Range parses, then resolves
# both endpoints.
# hash=sha1-only
title "gix diff A..B"
only_for_hash sha1-only && (small-repo-in-sandbox
  # TODO — expect_parity effect -- diff HEAD~1..HEAD
  it "matches git behavior" && { :; }
)

# mode=effect — `gix diff A...B`: three-dot symmetric (merge-base of
# A,B vs B). Equivalent to `git diff $(git merge-base A B) B`.
# hash=sha1-only
title "gix diff A...B"
only_for_hash sha1-only && (small-repo-in-sandbox
  # TODO — expect_parity effect -- diff HEAD~1...HEAD
  it "matches git behavior" && { :; }
)

# mode=effect — `gix diff <blob> <blob>`: raw blob-object comparison
# (builtin_diff_blobs). Both args resolve to blob objects directly.
# hash=sha1-only
title "gix diff <blob> <blob>"
only_for_hash sha1-only && (small-repo-in-sandbox
  # TODO — expect_parity effect -- diff <blob-sha-1> <blob-sha-2>
  it "matches git behavior" && { :; }
)

# mode=effect — `gix diff -- <path>...`: path filter trailing the diff
# spec. The `--` separator is a parse-options sentinel; everything
# after it is treated as paths even if it begins with `-`.
# hash=sha1-only
title "gix diff -- <path>"
only_for_hash sha1-only && (small-repo-in-sandbox
  # TODO — expect_parity effect -- diff HEAD -- file
  it "matches git behavior" && { :; }
)

# --- --cached / --staged / --merge-base / --no-index ------------------

# mode=effect — `gix diff --cached`: index vs HEAD (diff-index --cached).
# `--staged` is a synonym.
# hash=sha1-only
title "gix diff --cached"
only_for_hash sha1-only && (small-repo-in-sandbox
  # TODO — expect_parity effect -- diff --cached
  it "matches git behavior" && { :; }
)

# mode=effect — `gix diff --staged`: alias of --cached.
# hash=sha1-only
title "gix diff --staged"
only_for_hash sha1-only && (small-repo-in-sandbox
  # TODO — expect_parity effect -- diff --staged
  it "matches git behavior" && { :; }
)

# mode=effect — `gix diff --cached <commit>`: index vs <commit> (rather
# than vs HEAD). Note unborn-HEAD case: --cached without a commit on an
# unborn branch shows all staged changes (no error).
# hash=sha1-only
title "gix diff --cached <commit>"
only_for_hash sha1-only && (small-repo-in-sandbox
  # TODO — expect_parity effect -- diff --cached HEAD
  it "matches git behavior" && { :; }
)

# mode=effect — `gix diff --merge-base <commit>`: equivalent to
# `git diff $(git merge-base HEAD <commit>)`. Single-commit form.
# hash=sha1-only
title "gix diff --merge-base <commit>"
only_for_hash sha1-only && (small-repo-in-sandbox
  # TODO — expect_parity effect -- diff --merge-base HEAD
  it "matches git behavior" && { :; }
)

# mode=effect — `gix diff --merge-base A B`: merge-base of A,B vs B.
# Two-commit form.
# hash=sha1-only
title "gix diff --merge-base A B"
only_for_hash sha1-only && (small-repo-in-sandbox
  # TODO — expect_parity effect -- diff --merge-base HEAD~1 HEAD
  it "matches git behavior" && { :; }
)

# mode=effect — `gix diff --cached --merge-base A`: index vs merge-base
# of A and HEAD.
# hash=sha1-only
title "gix diff --cached --merge-base"
only_for_hash sha1-only && (small-repo-in-sandbox
  # TODO — expect_parity effect -- diff --cached --merge-base HEAD
  it "matches git behavior" && { :; }
)

# mode=effect — `gix diff --no-index <path-a> <path-b>`: compare two
# files on disk. Implies --exit-code; works outside a repo. Implicit
# --no-index also applies when only one path is inside-repo.
# hash=dual
title "gix diff --no-index <path-a> <path-b>"
only_for_hash dual && (sandbox
  # TODO — expect_parity effect -- diff --no-index file-a file-b
  it "matches git behavior" && { :; }
)

# --- output formats: patch family --------------------------------------

# mode=effect — `-p` / `-u` / `--patch`: generate patch (default for
# git diff). gix-diff currently has no patch renderer; close as
# compat_effect until a renderer lands or with byte-mode once it does.
# hash=sha1-only
title "gix diff -p / -u / --patch"
only_for_hash sha1-only && (small-repo-in-sandbox
  # TODO — expect_parity effect -- diff -p HEAD~1 HEAD
  it "matches git behavior" && { :; }
)

# mode=effect — `-s` / `--no-patch`: suppress diff output. Useful in
# combination with --stat / --exit-code. With no other format flag,
# yields empty stdout.
# hash=sha1-only
title "gix diff -s / --no-patch"
only_for_hash sha1-only && (small-repo-in-sandbox
  # TODO — expect_parity effect -- diff -s HEAD~1 HEAD
  it "matches git behavior" && { :; }
)

# mode=bytes — `--raw`: scriptable raw diff format (mode/sha/status).
# This is the canonical diff-tree/diff-index/diff-files output and is
# byte-stable across git versions.
# hash=sha1-only
title "gix diff --raw"
only_for_hash sha1-only && (small-repo-in-sandbox
  # TODO — expect_parity bytes -- diff --raw HEAD~1 HEAD
  it "matches git behavior" && { :; }
)

# mode=effect — `--patch-with-raw`: synonym for `-p --raw`.
# hash=sha1-only
title "gix diff --patch-with-raw"
only_for_hash sha1-only && (small-repo-in-sandbox
  # TODO — expect_parity effect -- diff --patch-with-raw HEAD~1 HEAD
  it "matches git behavior" && { :; }
)

# mode=effect — `-t`: show tree entries themselves (recurse into
# subdirectories). git-diff specific extra over diff-options.
# hash=sha1-only
title "gix diff -t"
only_for_hash sha1-only && (small-repo-in-sandbox
  # TODO — expect_parity effect -- diff -t HEAD~1 HEAD
  it "matches git behavior" && { :; }
)

# --- output formats: name / status / stat ------------------------------

# mode=bytes — `--name-only`: one path per line, byte-stable.
# hash=sha1-only
title "gix diff --name-only"
only_for_hash sha1-only && (small-repo-in-sandbox
  # TODO — expect_parity bytes -- diff --name-only HEAD~1 HEAD
  it "matches git behavior" && { :; }
)

# mode=bytes — `--name-status`: status letter + path per line.
# hash=sha1-only
title "gix diff --name-status"
only_for_hash sha1-only && (small-repo-in-sandbox
  # TODO — expect_parity bytes -- diff --name-status HEAD~1 HEAD
  it "matches git behavior" && { :; }
)

# mode=effect — `--stat[=<width>[,<name-width>[,<count>]]]`: file-by-file
# diffstat. Width-tunable; defaults to terminal-width.
# hash=sha1-only
title "gix diff --stat"
only_for_hash sha1-only && (small-repo-in-sandbox
  # TODO — expect_parity effect -- diff --stat HEAD~1 HEAD
  it "matches git behavior" && { :; }
)

# mode=effect — `--compact-summary`: condensed file-mode-change summary.
# hash=sha1-only
title "gix diff --compact-summary"
only_for_hash sha1-only && (small-repo-in-sandbox
  # TODO — expect_parity effect -- diff --compact-summary HEAD~1 HEAD
  it "matches git behavior" && { :; }
)

# mode=bytes — `--shortstat`: single summary line.
# hash=sha1-only
title "gix diff --shortstat"
only_for_hash sha1-only && (small-repo-in-sandbox
  # TODO — expect_parity bytes -- diff --shortstat HEAD~1 HEAD
  it "matches git behavior" && { :; }
)

# mode=bytes — `--numstat`: tab-separated numeric stat (added/removed/path).
# hash=sha1-only
title "gix diff --numstat"
only_for_hash sha1-only && (small-repo-in-sandbox
  # TODO — expect_parity bytes -- diff --numstat HEAD~1 HEAD
  it "matches git behavior" && { :; }
)

# mode=effect — `--dirstat[=<param>,...]`: per-directory percentage stat.
# Parameters: changes, lines, files, cumulative, <limit>.
# hash=sha1-only
title "gix diff --dirstat"
only_for_hash sha1-only && (small-repo-in-sandbox
  # TODO — expect_parity effect -- diff --dirstat HEAD~1 HEAD
  it "matches git behavior" && { :; }
)

# mode=effect — `--cumulative`: synonym for --dirstat=cumulative.
# hash=sha1-only
title "gix diff --cumulative"
only_for_hash sha1-only && (small-repo-in-sandbox
  # TODO — expect_parity effect -- diff --cumulative HEAD~1 HEAD
  it "matches git behavior" && { :; }
)

# mode=effect — `--dirstat-by-file[=<param>,...]`: synonym for
# --dirstat=files,<param>.
# hash=sha1-only
title "gix diff --dirstat-by-file"
only_for_hash sha1-only && (small-repo-in-sandbox
  # TODO — expect_parity effect -- diff --dirstat-by-file HEAD~1 HEAD
  it "matches git behavior" && { :; }
)

# mode=effect — `--summary`: file creation/deletion/rename/copy/mode
# change summary lines.
# hash=sha1-only
title "gix diff --summary"
only_for_hash sha1-only && (small-repo-in-sandbox
  # TODO — expect_parity effect -- diff --summary HEAD~1 HEAD
  it "matches git behavior" && { :; }
)

# mode=effect — `--patch-with-stat`: synonym for `-p --stat`.
# hash=sha1-only
title "gix diff --patch-with-stat"
only_for_hash sha1-only && (small-repo-in-sandbox
  # TODO — expect_parity effect -- diff --patch-with-stat HEAD~1 HEAD
  it "matches git behavior" && { :; }
)

# mode=bytes — `-z`: NUL-terminated paths in raw / name-only / name-
# status / numstat outputs (suppresses pathname-quoting).
# hash=sha1-only
title "gix diff -z"
only_for_hash sha1-only && (small-repo-in-sandbox
  # TODO — expect_parity bytes -- diff -z --raw HEAD~1 HEAD
  it "matches git behavior" && { :; }
)

# --- output controls ---------------------------------------------------

# mode=effect — `-U<n>` / `--unified=<n>`: number of context lines in
# patch output. Default 3.
# hash=sha1-only
title "gix diff -U / --unified"
only_for_hash sha1-only && (small-repo-in-sandbox
  # TODO — expect_parity effect -- diff -U5 HEAD~1 HEAD
  it "matches git behavior" && { :; }
)

# mode=effect — `--output=<file>`: write diff to file instead of stdout.
# hash=sha1-only
title "gix diff --output"
only_for_hash sha1-only && (small-repo-in-sandbox
  # TODO — expect_parity effect -- diff --output=out.patch HEAD~1 HEAD
  it "matches git behavior" && { :; }
)

# mode=effect — `--output-indicator-{new,old,context}=<char>`: per-line
# leading character override (default + - <space>).
# hash=sha1-only
title "gix diff --output-indicator-{new,old,context}"
only_for_hash sha1-only && (small-repo-in-sandbox
  # TODO — expect_parity effect -- diff --output-indicator-new=! ...
  it "matches git behavior" && { :; }
)

# mode=effect — `--abbrev[=<n>]`: hash abbreviation in raw / patch
# headers.
# hash=sha1-only
title "gix diff --abbrev"
only_for_hash sha1-only && (small-repo-in-sandbox
  # TODO — expect_parity effect -- diff --abbrev=12 --raw HEAD~1 HEAD
  it "matches git behavior" && { :; }
)

# mode=effect — `--binary`: emit base85-encoded binary patches that
# git-apply can consume.
# hash=sha1-only
title "gix diff --binary"
only_for_hash sha1-only && (small-repo-in-sandbox
  # TODO — expect_parity effect -- diff --binary HEAD~1 HEAD
  it "matches git behavior" && { :; }
)

# mode=effect — `--full-index`: full SHAs in patch headers (override
# --abbrev's abbreviation).
# hash=sha1-only
title "gix diff --full-index"
only_for_hash sha1-only && (small-repo-in-sandbox
  # TODO — expect_parity effect -- diff --full-index HEAD~1 HEAD
  it "matches git behavior" && { :; }
)

# mode=effect — `--line-prefix=<prefix>`: prepend text to each output
# line (used by submodule rendering and external tooling).
# hash=sha1-only
title "gix diff --line-prefix"
only_for_hash sha1-only && (small-repo-in-sandbox
  # TODO — expect_parity effect -- diff --line-prefix='> ' HEAD~1 HEAD
  it "matches git behavior" && { :; }
)

# mode=effect — `--src-prefix=<prefix>` / `--dst-prefix=<prefix>`:
# override the `a/` / `b/` patch-header prefixes.
# hash=sha1-only
title "gix diff --src-prefix / --dst-prefix"
only_for_hash sha1-only && (small-repo-in-sandbox
  # TODO — expect_parity effect -- diff --src-prefix=old/ --dst-prefix=new/ HEAD~1 HEAD
  it "matches git behavior" && { :; }
)

# mode=effect — `--no-prefix`: drop the a/b prefixes entirely.
# hash=sha1-only
title "gix diff --no-prefix"
only_for_hash sha1-only && (small-repo-in-sandbox
  # TODO — expect_parity effect -- diff --no-prefix HEAD~1 HEAD
  it "matches git behavior" && { :; }
)

# mode=effect — `--default-prefix`: restore default a/b prefixes after
# a prior alias / config override.
# hash=sha1-only
title "gix diff --default-prefix"
only_for_hash sha1-only && (small-repo-in-sandbox
  # TODO — expect_parity effect -- diff --default-prefix HEAD~1 HEAD
  it "matches git behavior" && { :; }
)

# --- color / word-diff --------------------------------------------------

# mode=effect — `--color[=<when>]`: always|auto|never. Pipe defaults to
# never (TTY-detection); fixture run is non-TTY so default = never.
# hash=sha1-only
title "gix diff --color"
only_for_hash sha1-only && (small-repo-in-sandbox
  # TODO — expect_parity effect -- diff --color=always HEAD~1 HEAD
  it "matches git behavior" && { :; }
)

# mode=effect — `--no-color`: disable color even on a TTY.
# hash=sha1-only
title "gix diff --no-color"
only_for_hash sha1-only && (small-repo-in-sandbox
  # TODO — expect_parity effect -- diff --no-color HEAD~1 HEAD
  it "matches git behavior" && { :; }
)

# mode=effect — `--color-moved[=<mode>]`: highlight blocks moved within
# a diff. Modes: no, default, plain, blocks, zebra, dimmed-zebra.
# hash=sha1-only
title "gix diff --color-moved"
only_for_hash sha1-only && (small-repo-in-sandbox
  # TODO — expect_parity effect -- diff --color-moved=zebra HEAD~1 HEAD
  it "matches git behavior" && { :; }
)

# mode=effect — `--no-color-moved`: turn it off.
# hash=sha1-only
title "gix diff --no-color-moved"
only_for_hash sha1-only && (small-repo-in-sandbox
  # TODO — expect_parity effect -- diff --no-color-moved HEAD~1 HEAD
  it "matches git behavior" && { :; }
)

# mode=effect — `--color-moved-ws=<mode>,...`: how to handle whitespace
# when scoring moves (no, ignore-space-at-eol, ignore-space-change,
# ignore-all-space, allow-indentation-change).
# hash=sha1-only
title "gix diff --color-moved-ws"
only_for_hash sha1-only && (small-repo-in-sandbox
  # TODO — expect_parity effect -- diff --color-moved-ws=ignore-all-space ...
  it "matches git behavior" && { :; }
)

# mode=effect — `--no-color-moved-ws`: revert color-moved-ws to default.
# hash=sha1-only
title "gix diff --no-color-moved-ws"
only_for_hash sha1-only && (small-repo-in-sandbox
  # TODO — expect_parity effect -- diff --no-color-moved-ws ...
  it "matches git behavior" && { :; }
)

# mode=effect — `--word-diff[=<mode>]`: color|plain|porcelain|none.
# hash=sha1-only
title "gix diff --word-diff"
only_for_hash sha1-only && (small-repo-in-sandbox
  # TODO — expect_parity effect -- diff --word-diff=plain HEAD~1 HEAD
  it "matches git behavior" && { :; }
)

# mode=effect — `--word-diff-regex=<regex>`: word-token regex for
# --word-diff (default \S+).
# hash=sha1-only
title "gix diff --word-diff-regex"
only_for_hash sha1-only && (small-repo-in-sandbox
  # TODO — expect_parity effect -- diff --word-diff-regex='\\w+' HEAD~1 HEAD
  it "matches git behavior" && { :; }
)

# mode=effect — `--color-words[=<regex>]`: shortcut for
# `--word-diff=color --word-diff-regex=<regex>`.
# hash=sha1-only
title "gix diff --color-words"
only_for_hash sha1-only && (small-repo-in-sandbox
  # TODO — expect_parity effect -- diff --color-words HEAD~1 HEAD
  it "matches git behavior" && { :; }
)

# --- algorithm / heuristic ---------------------------------------------

# mode=effect — `--minimal`: spend extra time to minimize diff output.
# hash=sha1-only
title "gix diff --minimal"
only_for_hash sha1-only && (small-repo-in-sandbox
  # TODO — expect_parity effect -- diff --minimal HEAD~1 HEAD
  it "matches git behavior" && { :; }
)

# mode=effect — `--patience`: patience-diff algorithm.
# hash=sha1-only
title "gix diff --patience"
only_for_hash sha1-only && (small-repo-in-sandbox
  # TODO — expect_parity effect -- diff --patience HEAD~1 HEAD
  it "matches git behavior" && { :; }
)

# mode=effect — `--histogram`: histogram-diff algorithm.
# hash=sha1-only
title "gix diff --histogram"
only_for_hash sha1-only && (small-repo-in-sandbox
  # TODO — expect_parity effect -- diff --histogram HEAD~1 HEAD
  it "matches git behavior" && { :; }
)

# mode=effect — `--anchored=<text>`: anchored-diff (lines containing
# text are anchored — uses generalized patience under the hood).
# hash=sha1-only
title "gix diff --anchored"
only_for_hash sha1-only && (small-repo-in-sandbox
  # TODO — expect_parity effect -- diff --anchored=foo HEAD~1 HEAD
  it "matches git behavior" && { :; }
)

# mode=effect — `--diff-algorithm=<algo>`: myers|minimal|patience|histogram.
# hash=sha1-only
title "gix diff --diff-algorithm"
only_for_hash sha1-only && (small-repo-in-sandbox
  # TODO — expect_parity effect -- diff --diff-algorithm=histogram HEAD~1 HEAD
  it "matches git behavior" && { :; }
)

# mode=effect — `--indent-heuristic`: shift hunk boundaries to make
# patches easier to read (default in modern git).
# hash=sha1-only
title "gix diff --indent-heuristic"
only_for_hash sha1-only && (small-repo-in-sandbox
  # TODO — expect_parity effect -- diff --indent-heuristic HEAD~1 HEAD
  it "matches git behavior" && { :; }
)

# mode=effect — `--no-indent-heuristic`: opt out.
# hash=sha1-only
title "gix diff --no-indent-heuristic"
only_for_hash sha1-only && (small-repo-in-sandbox
  # TODO — expect_parity effect -- diff --no-indent-heuristic HEAD~1 HEAD
  it "matches git behavior" && { :; }
)

# --- whitespace ---------------------------------------------------------

# mode=effect — `-a` / `--text`: treat all files as text (don't binary-
# detect).
# hash=sha1-only
title "gix diff -a / --text"
only_for_hash sha1-only && (small-repo-in-sandbox
  # TODO — expect_parity effect -- diff -a HEAD~1 HEAD
  it "matches git behavior" && { :; }
)

# mode=effect — `--ignore-cr-at-eol`: ignore CR at line ends.
# hash=sha1-only
title "gix diff --ignore-cr-at-eol"
only_for_hash sha1-only && (small-repo-in-sandbox
  # TODO — expect_parity effect -- diff --ignore-cr-at-eol HEAD~1 HEAD
  it "matches git behavior" && { :; }
)

# mode=effect — `--ignore-space-at-eol`: ignore trailing whitespace
# differences.
# hash=sha1-only
title "gix diff --ignore-space-at-eol"
only_for_hash sha1-only && (small-repo-in-sandbox
  # TODO — expect_parity effect -- diff --ignore-space-at-eol HEAD~1 HEAD
  it "matches git behavior" && { :; }
)

# mode=effect — `-b` / `--ignore-space-change`: ignore amount of
# whitespace.
# hash=sha1-only
title "gix diff -b / --ignore-space-change"
only_for_hash sha1-only && (small-repo-in-sandbox
  # TODO — expect_parity effect -- diff -b HEAD~1 HEAD
  it "matches git behavior" && { :; }
)

# mode=effect — `-w` / `--ignore-all-space`: ignore whitespace entirely.
# hash=sha1-only
title "gix diff -w / --ignore-all-space"
only_for_hash sha1-only && (small-repo-in-sandbox
  # TODO — expect_parity effect -- diff -w HEAD~1 HEAD
  it "matches git behavior" && { :; }
)

# mode=effect — `--ignore-blank-lines`: ignore changes whose lines are
# all blank.
# hash=sha1-only
title "gix diff --ignore-blank-lines"
only_for_hash sha1-only && (small-repo-in-sandbox
  # TODO — expect_parity effect -- diff --ignore-blank-lines HEAD~1 HEAD
  it "matches git behavior" && { :; }
)

# mode=effect — `--ignore-matching-lines=<regex>`: ignore changes whose
# inserted/deleted lines all match.
# hash=sha1-only
title "gix diff --ignore-matching-lines"
only_for_hash sha1-only && (small-repo-in-sandbox
  # TODO — expect_parity effect -- diff --ignore-matching-lines='^#' HEAD~1 HEAD
  it "matches git behavior" && { :; }
)

# mode=effect — `--ws-error-highlight=<kind>`: kinds = (none|default|
# old|new|context)+.
# hash=sha1-only
title "gix diff --ws-error-highlight"
only_for_hash sha1-only && (small-repo-in-sandbox
  # TODO — expect_parity effect -- diff --ws-error-highlight=all HEAD~1 HEAD
  it "matches git behavior" && { :; }
)

# mode=effect — `--check`: warn on whitespace errors. Sets exit-code 2
# when a problem is detected.
# hash=sha1-only
title "gix diff --check"
only_for_hash sha1-only && (small-repo-in-sandbox
  # TODO — expect_parity effect -- diff --check HEAD~1 HEAD
  it "matches git behavior" && { :; }
)

# mode=effect — `--inter-hunk-context=<n>`: combine hunks closer than n
# lines.
# hash=sha1-only
title "gix diff --inter-hunk-context"
only_for_hash sha1-only && (small-repo-in-sandbox
  # TODO — expect_parity effect -- diff --inter-hunk-context=3 HEAD~1 HEAD
  it "matches git behavior" && { :; }
)

# mode=effect — `-W` / `--function-context`: show whole enclosing
# function in patch.
# hash=sha1-only
title "gix diff -W / --function-context"
only_for_hash sha1-only && (small-repo-in-sandbox
  # TODO — expect_parity effect -- diff -W HEAD~1 HEAD
  it "matches git behavior" && { :; }
)

# --- detection ---------------------------------------------------------

# mode=effect — `--no-renames`: turn off rename detection.
# hash=sha1-only
title "gix diff --no-renames"
only_for_hash sha1-only && (small-repo-in-sandbox
  # TODO — expect_parity effect -- diff --no-renames HEAD~1 HEAD
  it "matches git behavior" && { :; }
)

# mode=effect — `--rename-empty` / `--no-rename-empty`: whether empty
# files participate in rename detection.
# hash=sha1-only
title "gix diff --rename-empty / --no-rename-empty"
only_for_hash sha1-only && (small-repo-in-sandbox
  # TODO — expect_parity effect -- diff --no-rename-empty HEAD~1 HEAD
  it "matches git behavior" && { :; }
)

# mode=effect — `-B[<n>][/<m>]` / `--break-rewrites`: split modify-
# rewrites into delete+create when similarity below n / dissimilarity
# above m.
# hash=sha1-only
title "gix diff -B / --break-rewrites"
only_for_hash sha1-only && (small-repo-in-sandbox
  # TODO — expect_parity effect -- diff -B50 HEAD~1 HEAD
  it "matches git behavior" && { :; }
)

# mode=effect — `-M[<n>]` / `--find-renames[=<n>]`: detect renames at
# similarity threshold n%.
# hash=sha1-only
title "gix diff -M / --find-renames"
only_for_hash sha1-only && (small-repo-in-sandbox
  # TODO — expect_parity effect -- diff -M HEAD~1 HEAD
  it "matches git behavior" && { :; }
)

# mode=effect — `-C[<n>]` / `--find-copies[=<n>]`: detect copies.
# hash=sha1-only
title "gix diff -C / --find-copies"
only_for_hash sha1-only && (small-repo-in-sandbox
  # TODO — expect_parity effect -- diff -C HEAD~1 HEAD
  it "matches git behavior" && { :; }
)

# mode=effect — `--find-copies-harder`: also inspect unmodified files.
# hash=sha1-only
title "gix diff --find-copies-harder"
only_for_hash sha1-only && (small-repo-in-sandbox
  # TODO — expect_parity effect -- diff --find-copies-harder HEAD~1 HEAD
  it "matches git behavior" && { :; }
)

# mode=bytes — `--diff-filter=<mask>`: select by status letter set
# (e.g. ACMR, lowercase = exclude). Affects raw / name-only / patch.
# hash=sha1-only
title "gix diff --diff-filter"
only_for_hash sha1-only && (small-repo-in-sandbox
  # TODO — expect_parity bytes -- diff --diff-filter=AM --raw HEAD~1 HEAD
  it "matches git behavior" && { :; }
)

# mode=effect — `-D` / `--irreversible-delete`: omit pre-image of
# deletion (cuts patch size; resulting diff is no longer applicable).
# hash=sha1-only
title "gix diff -D / --irreversible-delete"
only_for_hash sha1-only && (small-repo-in-sandbox
  # TODO — expect_parity effect -- diff -D HEAD~1 HEAD
  it "matches git behavior" && { :; }
)

# --- pickaxe -----------------------------------------------------------

# mode=effect — `-S<string>`: search for changes that alter the
# occurrence count of <string>.
# hash=sha1-only
title "gix diff -S"
only_for_hash sha1-only && (small-repo-in-sandbox
  # TODO — expect_parity effect -- diff -Sfoo HEAD~1 HEAD
  it "matches git behavior" && { :; }
)

# mode=effect — `-G<regex>`: search for changes whose added/removed
# line matches <regex>.
# hash=sha1-only
title "gix diff -G"
only_for_hash sha1-only && (small-repo-in-sandbox
  # TODO — expect_parity effect -- diff -Gfoo HEAD~1 HEAD
  it "matches git behavior" && { :; }
)

# mode=effect — `--find-object=<oid>`: changes that touch the named
# object id.
# hash=sha1-only
title "gix diff --find-object"
only_for_hash sha1-only && (small-repo-in-sandbox
  # TODO — expect_parity effect -- diff --find-object=<oid> HEAD~1 HEAD
  it "matches git behavior" && { :; }
)

# mode=effect — `--pickaxe-all`: when -S/-G triggers, show the full
# diff, not just the affected file.
# hash=sha1-only
title "gix diff --pickaxe-all"
only_for_hash sha1-only && (small-repo-in-sandbox
  # TODO — expect_parity effect -- diff --pickaxe-all -Sfoo HEAD~1 HEAD
  it "matches git behavior" && { :; }
)

# mode=effect — `--pickaxe-regex`: treat -S argument as POSIX ERE.
# hash=sha1-only
title "gix diff --pickaxe-regex"
only_for_hash sha1-only && (small-repo-in-sandbox
  # TODO — expect_parity effect -- diff --pickaxe-regex -Sfo+ HEAD~1 HEAD
  it "matches git behavior" && { :; }
)

# --- path control ------------------------------------------------------

# mode=effect — `-R`: swap old/new (output reverse diff).
# hash=sha1-only
title "gix diff -R"
only_for_hash sha1-only && (small-repo-in-sandbox
  # TODO — expect_parity effect -- diff -R HEAD~1 HEAD
  it "matches git behavior" && { :; }
)

# mode=effect — `--relative[=<path>]`: emit paths relative to <path>
# (or cwd if omitted), excluding paths outside.
# hash=sha1-only
title "gix diff --relative"
only_for_hash sha1-only && (small-repo-in-sandbox
  # TODO — expect_parity effect -- diff --relative HEAD~1 HEAD
  it "matches git behavior" && { :; }
)

# mode=effect — `--no-relative`: counter-flag.
# hash=sha1-only
title "gix diff --no-relative"
only_for_hash sha1-only && (small-repo-in-sandbox
  # TODO — expect_parity effect -- diff --no-relative HEAD~1 HEAD
  it "matches git behavior" && { :; }
)

# mode=effect — `--skip-to=<file>`: skip output before <file>.
# hash=sha1-only
title "gix diff --skip-to"
only_for_hash sha1-only && (small-repo-in-sandbox
  # TODO — expect_parity effect -- diff --skip-to=somefile HEAD~1 HEAD
  it "matches git behavior" && { :; }
)

# mode=effect — `--rotate-to=<file>`: rotate the file list so <file>
# leads.
# hash=sha1-only
title "gix diff --rotate-to"
only_for_hash sha1-only && (small-repo-in-sandbox
  # TODO — expect_parity effect -- diff --rotate-to=somefile HEAD~1 HEAD
  it "matches git behavior" && { :; }
)

# mode=effect — `<path>...`: pathspec filter trailing the diff spec
# (without `--`).
# hash=sha1-only
title "gix diff <path>"
only_for_hash sha1-only && (small-repo-in-sandbox
  # TODO — expect_parity effect -- diff HEAD~1 HEAD somefile
  it "matches git behavior" && { :; }
)

# --- submodule / textconv / ext-diff -----------------------------------

# mode=effect — `--submodule[=<format>]`: short|log|diff. Submodule
# rendering mode in diff output.
# hash=sha1-only
title "gix diff --submodule"
only_for_hash sha1-only && (small-repo-in-sandbox
  # TODO — expect_parity effect -- diff --submodule=log HEAD~1 HEAD
  it "matches git behavior" && { :; }
)

# mode=effect — `--ignore-submodules[=<when>]`: none|untracked|dirty|all.
# hash=sha1-only
title "gix diff --ignore-submodules"
only_for_hash sha1-only && (small-repo-in-sandbox
  # TODO — expect_parity effect -- diff --ignore-submodules=all HEAD~1 HEAD
  it "matches git behavior" && { :; }
)

# mode=effect — `--ita-invisible-in-index`: hide intent-to-add entries
# from the index side of the diff.
# hash=sha1-only
title "gix diff --ita-invisible-in-index"
only_for_hash sha1-only && (small-repo-in-sandbox
  # TODO — expect_parity effect -- diff --ita-invisible-in-index HEAD
  it "matches git behavior" && { :; }
)

# mode=effect — `--textconv` / `--no-textconv`: run user-defined
# textconv filters before diff.
# hash=sha1-only
title "gix diff --textconv / --no-textconv"
only_for_hash sha1-only && (small-repo-in-sandbox
  # TODO — expect_parity effect -- diff --textconv HEAD~1 HEAD
  it "matches git behavior" && { :; }
)

# mode=effect — `--ext-diff` / `--no-ext-diff`: enable/disable user-
# configured external diff drivers.
# hash=sha1-only
title "gix diff --ext-diff / --no-ext-diff"
only_for_hash sha1-only && (small-repo-in-sandbox
  # TODO — expect_parity effect -- diff --ext-diff HEAD~1 HEAD
  it "matches git behavior" && { :; }
)

# --- exit-code / quiet -------------------------------------------------

# mode=effect — `--exit-code`: exit 1 when changes, 0 otherwise (like
# the `diff` program). --no-index implies it.
# hash=sha1-only
title "gix diff --exit-code"
only_for_hash sha1-only && (small-repo-in-sandbox
  # TODO — expect_parity effect -- diff --exit-code HEAD~1 HEAD
  it "matches git behavior" && { :; }
)

# mode=effect — `--quiet`: --exit-code + suppress diff output.
# hash=sha1-only
title "gix diff --quiet"
only_for_hash sha1-only && (small-repo-in-sandbox
  # TODO — expect_parity effect -- diff --quiet HEAD~1 HEAD
  it "matches git behavior" && { :; }
)

# --- merge stage selection (-1 / -2 / -3 / -0) -------------------------

# mode=effect — `-1` / `--base`: compare working tree vs unmerged stage 1
# (only meaningful while resolving conflicts).
# hash=sha1-only
title "gix diff -1 / --base"
only_for_hash sha1-only && (small-repo-in-sandbox
  # TODO — expect_parity effect -- diff -1
  it "matches git behavior" && { :; }
)

# mode=effect — `-2` / `--ours`.
# hash=sha1-only
title "gix diff -2 / --ours"
only_for_hash sha1-only && (small-repo-in-sandbox
  # TODO — expect_parity effect -- diff -2
  it "matches git behavior" && { :; }
)

# mode=effect — `-3` / `--theirs`.
# hash=sha1-only
title "gix diff -3 / --theirs"
only_for_hash sha1-only && (small-repo-in-sandbox
  # TODO — expect_parity effect -- diff -3
  it "matches git behavior" && { :; }
)

# mode=effect — `-0`: omit diff output for unmerged entries; print
# "Unmerged" instead. Working-tree-vs-index only.
# hash=sha1-only
title "gix diff -0"
only_for_hash sha1-only && (small-repo-in-sandbox
  # TODO — expect_parity effect -- diff -0
  it "matches git behavior" && { :; }
)

# --- combined-diff (merge commit) --------------------------------------

# mode=effect — `gix diff <merge> <merge>^@`: combined-diff output for
# a merge commit (synonymous with `git show <merge>` for the diff
# portion). Triggers builtin_diff_combined.
# hash=sha1-only
title "gix diff <merge> <merge>^@"
only_for_hash sha1-only && (small-repo-in-sandbox
  # TODO — expect_parity effect -- diff HEAD HEAD^@
  it "matches git behavior" && { :; }
)

# mode=effect — `--combined-all-paths`: list paths from each parent in
# combined-diff output.
# hash=sha1-only
title "gix diff --combined-all-paths"
only_for_hash sha1-only && (small-repo-in-sandbox
  # TODO — expect_parity effect -- diff --combined-all-paths <merge> <merge>^@
  it "matches git behavior" && { :; }
)
