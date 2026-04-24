# Must be sourced into tests/parity.sh or tests/journey.sh — see tests/parity.sh.
#
# Parity scaffold for `git log` ↔ `gix log`.
#
# One `title` + `it` block per flag derived from
# vendor/git/Documentation/git-log.adoc and vendor/git/builtin/log.c
# (builtin_log_options[], cmd_log). The rev-list traversal flags and
# pretty-format flags are inherited via include::rev-list-options.adoc
# and include::pretty-formats.adoc, and the diff flags via
# include::diff-options.adoc — those are the bulk of the surface.
# Every `it` body starts as a TODO: placeholder — iteration N of the
# ralph loop picks the next TODO, converts it to a real `expect_parity`
# assertion, and removes the TODO marker.
#
# Verdict modes (comment above each block):
#   bytes  — scriptable output (e.g. --format=%H, --oneline); byte-exact
#            match required
#   effect — exit-code + UX; output diff reported but not fatal. Default
#            for most rows because `gix log` today emits only "<8-hash>
#            <subject>" while git's default is medium pretty-format.
#
# Coverage on gix's current Clap surface (src/plumbing/options/mod.rs::log):
#   gix log [PATHSPEC]
# That's the entire surface. Every non-pathspec flag below will fail its
# first parity attempt by tripping Clap's UnknownArgument path (remapped
# to exit 129 in src/plumbing/main.rs — already matching git's usage exit
# code). Closing a row therefore means: (1) add the flag to
# src/plumbing/options/mod.rs::log::Platform, (2) widen the
# gitoxide_core::repository::log::log signature, (3) implement the
# semantics in gitoxide-core/src/repository/log.rs. Plumbing helpers
# (gix::traverse, gix::revision, gix::diff) already exist for most cases.
#
# Hash coverage: `dual` rows never open a repo (--help, --bogus-flag
# outside any repo, not-a-repo) — those exercise Clap/env wiring only.
# Every row that opens a repository is `sha1-only` because gix-config
# rejects `extensions.objectFormat=sha256`
# (gix/src/config/tree/sections/extensions.rs try_into_object_format,
# sha1-only validator), blocking every sha256 fixture at open. Rows flip
# to `dual` once that validator accepts sha256.
#
# parity-defaults:
#   hash=sha1-only "gix cannot load sha256 repos: extensions.objectFormat=sha256 rejected (gix/src/config/tree/sections/extensions.rs)"
#   mode=effect

title "gix log"

# --- meta / help --------------------------------------------------------

# mode=effect — clap --help short-circuits before repo load, exits 0.
# git's --help delegates to `man git-log`; gix returns clap's
# auto-generated help. Message text diverges wildly; only the exit-code
# match is asserted.
# hash=dual
title "gix log --help"
only_for_hash dual && (sandbox
  it "matches git: --help exits 0" && {
    expect_parity effect -- log --help
  }
)

# --- argument-parsing error paths --------------------------------------

# mode=effect — unknown flag: git log specifically calls parse_options
# with PARSE_OPT_KEEP_UNKNOWN_OPT (vendor/git/builtin/log.c:307) and then
# die()s on argc>1 at line 316 — exit 128, not the usual 129 that
# usage_msg_opt would emit. src/plumbing/main.rs::detect_subcommand_from_argv
# recognizes `log` specifically and remaps UnknownArgument to 128 for it.
# hash=sha1-only
title "gix log --bogus-flag"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git: --bogus-flag dies 128 (log-specific)" && {
    expect_parity effect -- log --bogus-flag
  }
)

# mode=effect — `git log` outside any repo dies 128 with
# "fatal: not a git repository (or any of the parent directories): .git".
# gix's plumbing repository() closure already remaps the
# gix_discover::upwards::Error::NoGitRepository* variants to git's exact
# wording + exit 128 (see status.sh's analogous row).
# hash=dual
title "gix log (outside a repository)"
only_for_hash dual && (sandbox
  # TODO — expect_parity effect -- log
  it "matches git behavior" && { :; }
)

# --- basic traversal ----------------------------------------------------

# mode=effect — default `log` in a repo with commits. git prints medium
# pretty-format (full hash + Author + Date + blank + subject); gix emits
# a simplified "<8-hash> <subject>" line. Exit 0 either way. Effect mode
# (exit-code parity only) until pretty-format support lands.
# hash=sha1-only
title "gix log (default, populated repo)"
only_for_hash sha1-only && (small-repo-in-sandbox
  # TODO — expect_parity effect -- log
  it "matches git behavior" && { :; }
)

# mode=effect — empty repo (HEAD points at unborn branch). git exits 128
# with "fatal: your current branch 'main' does not have any commits yet".
# gix today bails from repo.head()?.peel_to_commit()? with its own
# wording; exit code divergence expected until remapped.
# hash=sha1-only
title "gix log (empty repo, unborn HEAD)"
only_for_hash sha1-only && (sandbox
  git-init-hash-aware
  # TODO — expect_parity effect -- log
  it "matches git behavior" && { :; }
)

# mode=effect — log from a named branch tip. git walks ancestors of
# <branch>; gix walks head's ancestors regardless of arg (arg is treated
# as pathspec by current Clap surface). Expect UnknownArgument-style
# failure until revspec argument is introduced.
# hash=sha1-only
title "gix log <ref>"
only_for_hash sha1-only && (small-repo-in-sandbox
  # TODO — expect_parity effect -- log main
  it "matches git behavior" && { :; }
)

# mode=effect — log from a specific commit sha. Same wiring gap as <ref>;
# current gix mis-parses the sha as a pathspec.
# hash=sha1-only
title "gix log <sha>"
only_for_hash sha1-only && (small-repo-in-sandbox
  # TODO — expect_parity effect -- log HEAD~1
  it "matches git behavior" && { :; }
)

# mode=effect — unknown revision: git exits 128 with
# "fatal: ambiguous argument '<rev>': unknown revision or path not in the
# working tree". gix's current pathspec-only surface returns success with
# empty output. Row closes once revspec is wired + error mapped.
# hash=sha1-only
title "gix log <unknown-ref>"
only_for_hash sha1-only && (small-repo-in-sandbox
  # TODO — expect_parity effect -- log no-such-ref
  it "matches git behavior" && { :; }
)

# --- range syntax -------------------------------------------------------

# mode=effect — two-dot range A..B: commits reachable from B but not A.
# gix must grow revspec parsing with RangeSpec; gix-revision already has
# the plumbing. Exit 0, output empty if range is empty.
# hash=sha1-only
title "gix log A..B"
only_for_hash sha1-only && (small-repo-in-sandbox
  # TODO — expect_parity effect -- log dev..main
  it "matches git behavior" && { :; }
)

# mode=effect — three-dot range A...B: symmetric difference. Same
# infrastructure as A..B; different traversal semantics.
# hash=sha1-only
title "gix log A...B"
only_for_hash sha1-only && (small-repo-in-sandbox
  # TODO — expect_parity effect -- log dev...main
  it "matches git behavior" && { :; }
)

# mode=effect — --all: traverse every ref. gix-revision already exposes
# Walk::all_references(); plumbing is ready.
# hash=sha1-only
title "gix log --all"
only_for_hash sha1-only && (small-repo-in-sandbox
  # TODO — expect_parity effect -- log --all
  it "matches git behavior" && { :; }
)

# mode=effect — --branches: include only refs/heads/*.
# hash=sha1-only
title "gix log --branches"
only_for_hash sha1-only && (small-repo-in-sandbox
  # TODO — expect_parity effect -- log --branches
  it "matches git behavior" && { :; }
)

# mode=effect — --tags: include only refs/tags/*.
# hash=sha1-only
title "gix log --tags"
only_for_hash sha1-only && (small-repo-in-sandbox
  # TODO — expect_parity effect -- log --tags
  it "matches git behavior" && { :; }
)

# mode=effect — --remotes: include only refs/remotes/*.
# hash=sha1-only
title "gix log --remotes"
only_for_hash sha1-only && (small-repo-in-sandbox
  # TODO — expect_parity effect -- log --remotes
  it "matches git behavior" && { :; }
)

# mode=effect — --not <ref>: invert traversal starting from the next
# following ref. Composable with other ref selectors.
# hash=sha1-only
title "gix log --not <ref>"
only_for_hash sha1-only && (small-repo-in-sandbox
  # TODO — expect_parity effect -- log main --not dev
  it "matches git behavior" && { :; }
)

# --- pathspec -----------------------------------------------------------

# mode=effect — log limited to commits touching <path>. gix already
# parses [PATHSPEC] but the traverser ignores it today (falls back to
# log_all).
# hash=sha1-only
title "gix log -- <path>"
only_for_hash sha1-only && (small-repo-in-sandbox
  # TODO — expect_parity effect -- log -- a
  it "matches git behavior" && { :; }
)

# mode=effect — <ref> -- <path>: composition of revspec + pathspec.
# Depends on revspec-argument wiring (above).
# hash=sha1-only
title "gix log <ref> -- <path>"
only_for_hash sha1-only && (small-repo-in-sandbox
  # TODO — expect_parity effect -- log main -- a
  it "matches git behavior" && { :; }
)

# mode=effect — nonexistent path: git exits 128 with
# "fatal: ambiguous argument '<path>': unknown revision or path not in the
# working tree".
# hash=sha1-only
title "gix log -- <missing-path>"
only_for_hash sha1-only && (small-repo-in-sandbox
  # TODO — expect_parity effect -- log -- no-such-file
  it "matches git behavior" && { :; }
)

# --- commit limiting ----------------------------------------------------

# mode=effect — -n <count>: cap total commits printed. Wires straight
# into the topo-walk iterator as .take(n).
# hash=sha1-only
title "gix log -n <count>"
only_for_hash sha1-only && (small-repo-in-sandbox
  # TODO — expect_parity effect -- log -n 1
  it "matches git behavior" && { :; }
)

# mode=effect — --max-count=<n>: long form of -n.
# hash=sha1-only
title "gix log --max-count=<n>"
only_for_hash sha1-only && (small-repo-in-sandbox
  # TODO — expect_parity effect -- log --max-count=1
  it "matches git behavior" && { :; }
)

# mode=effect — -<number>: compact form (e.g. -3). Clap needs to accept
# numeric short flags; revision.c handles this as a positional shortcut.
# hash=sha1-only
title "gix log -<number>"
only_for_hash sha1-only && (small-repo-in-sandbox
  # TODO — expect_parity effect -- log -2
  it "matches git behavior" && { :; }
)

# mode=effect — --skip=<n>: drop first n commits before printing.
# hash=sha1-only
title "gix log --skip=<n>"
only_for_hash sha1-only && (small-repo-in-sandbox
  # TODO — expect_parity effect -- log --skip=1
  it "matches git behavior" && { :; }
)

# mode=effect — --since=<time>: limit to commits newer than <time>. Date
# parsing is approxidate; gix-date handles a compatible subset.
# hash=sha1-only
title "gix log --since=<time>"
only_for_hash sha1-only && (small-repo-in-sandbox
  # TODO — expect_parity effect -- log --since=2000-01-01
  it "matches git behavior" && { :; }
)

# mode=effect — --until=<time>: limit to commits older than <time>.
# hash=sha1-only
title "gix log --until=<time>"
only_for_hash sha1-only && (small-repo-in-sandbox
  # TODO — expect_parity effect -- log --until=2100-01-01
  it "matches git behavior" && { :; }
)

# mode=effect — --author=<pattern>: filter by author (regex by default,
# matched against "<name> <email>").
# hash=sha1-only
title "gix log --author=<pattern>"
only_for_hash sha1-only && (small-repo-in-sandbox
  # TODO — expect_parity effect -- log --author=Sebastian
  it "matches git behavior" && { :; }
)

# mode=effect — --committer=<pattern>: mirror of --author for committer.
# hash=sha1-only
title "gix log --committer=<pattern>"
only_for_hash sha1-only && (small-repo-in-sandbox
  # TODO — expect_parity effect -- log --committer=Sebastian
  it "matches git behavior" && { :; }
)

# mode=effect — --grep=<pattern>: filter by commit message (regex).
# hash=sha1-only
title "gix log --grep=<pattern>"
only_for_hash sha1-only && (small-repo-in-sandbox
  # TODO — expect_parity effect -- log --grep=first
  it "matches git behavior" && { :; }
)

# mode=effect — -i --grep: case-insensitive message filter.
# hash=sha1-only
title "gix log -i --grep=<pattern>"
only_for_hash sha1-only && (small-repo-in-sandbox
  # TODO — expect_parity effect -- log -i --grep=FIRST
  it "matches git behavior" && { :; }
)

# mode=effect — --invert-grep --grep: exclude matching commits.
# hash=sha1-only
title "gix log --invert-grep --grep=<pattern>"
only_for_hash sha1-only && (small-repo-in-sandbox
  # TODO — expect_parity effect -- log --invert-grep --grep=first
  it "matches git behavior" && { :; }
)

# mode=effect — --all-match with multiple --grep: require all to match.
# hash=sha1-only
title "gix log --all-match --grep=<p1> --grep=<p2>"
only_for_hash sha1-only && (small-repo-in-sandbox
  # TODO — expect_parity effect -- log --all-match --grep=first --grep=second
  it "matches git behavior" && { :; }
)

# mode=effect — -E: use POSIX extended regex for --grep.
# hash=sha1-only
title "gix log -E --grep=<regex>"
only_for_hash sha1-only && (small-repo-in-sandbox
  # TODO — expect_parity effect -- log -E --grep='^(first|second)$'
  it "matches git behavior" && { :; }
)

# mode=effect — -F: treat --grep pattern as a literal string.
# hash=sha1-only
title "gix log -F --grep=<literal>"
only_for_hash sha1-only && (small-repo-in-sandbox
  # TODO — expect_parity effect -- log -F --grep='first'
  it "matches git behavior" && { :; }
)

# mode=effect — --no-merges: skip merge commits (nparents > 1).
# hash=sha1-only
title "gix log --no-merges"
only_for_hash sha1-only && (small-repo-in-sandbox
  # TODO — expect_parity effect -- log --no-merges
  it "matches git behavior" && { :; }
)

# mode=effect — --merges: only merge commits.
# hash=sha1-only
title "gix log --merges"
only_for_hash sha1-only && (small-repo-in-sandbox
  # TODO — expect_parity effect -- log --merges
  it "matches git behavior" && { :; }
)

# --- pretty / format ----------------------------------------------------

# mode=bytes — --oneline: equivalent to --pretty=oneline --abbrev-commit.
# Canonical scriptable format; byte parity required. gix's current
# default output looks similar ("<8-hash> <subject>") but uses 8-char
# abbrev vs git's default 7-char — exact divergence.
# hash=sha1-only
title "gix log --oneline"
only_for_hash sha1-only && (small-repo-in-sandbox
  # TODO — expect_parity bytes -- log --oneline
  it "matches git behavior" && { :; }
)

# mode=bytes — --pretty=oneline: full hash + subject, no abbreviation.
# hash=sha1-only
title "gix log --pretty=oneline"
only_for_hash sha1-only && (small-repo-in-sandbox
  # TODO — expect_parity bytes -- log --pretty=oneline
  it "matches git behavior" && { :; }
)

# mode=effect — --pretty=short: short pretty (Author + subject only).
# hash=sha1-only
title "gix log --pretty=short"
only_for_hash sha1-only && (small-repo-in-sandbox
  # TODO — expect_parity effect -- log --pretty=short
  it "matches git behavior" && { :; }
)

# mode=effect — --pretty=medium: the default format.
# hash=sha1-only
title "gix log --pretty=medium"
only_for_hash sha1-only && (small-repo-in-sandbox
  # TODO — expect_parity effect -- log --pretty=medium
  it "matches git behavior" && { :; }
)

# mode=effect — --pretty=full: adds Commit author + committer.
# hash=sha1-only
title "gix log --pretty=full"
only_for_hash sha1-only && (small-repo-in-sandbox
  # TODO — expect_parity effect -- log --pretty=full
  it "matches git behavior" && { :; }
)

# mode=effect — --pretty=fuller: medium + commit date line.
# hash=sha1-only
title "gix log --pretty=fuller"
only_for_hash sha1-only && (small-repo-in-sandbox
  # TODO — expect_parity effect -- log --pretty=fuller
  it "matches git behavior" && { :; }
)

# mode=effect — --pretty=raw: raw commit bytes (tree, parent, author,
# committer, blank, message).
# hash=sha1-only
title "gix log --pretty=raw"
only_for_hash sha1-only && (small-repo-in-sandbox
  # TODO — expect_parity effect -- log --pretty=raw
  it "matches git behavior" && { :; }
)

# mode=effect — --pretty=reference: hash + subject + (author, date).
# hash=sha1-only
title "gix log --pretty=reference"
only_for_hash sha1-only && (small-repo-in-sandbox
  # TODO — expect_parity effect -- log --pretty=reference
  it "matches git behavior" && { :; }
)

# mode=bytes — --format=%H: just the full hash per commit. Canonical
# scripting use case; byte parity required.
# hash=sha1-only
title "gix log --format=%H"
only_for_hash sha1-only && (small-repo-in-sandbox
  # TODO — expect_parity bytes -- log --format=%H
  it "matches git behavior" && { :; }
)

# mode=bytes — --format=%h %s: short hash + subject, custom format.
# hash=sha1-only
title "gix log --format='%h %s'"
only_for_hash sha1-only && (small-repo-in-sandbox
  # TODO — expect_parity bytes -- log '--format=%h %s'
  it "matches git behavior" && { :; }
)

# mode=effect — --abbrev-commit: abbreviate shown hashes (default in
# --oneline).
# hash=sha1-only
title "gix log --abbrev-commit"
only_for_hash sha1-only && (small-repo-in-sandbox
  # TODO — expect_parity effect -- log --abbrev-commit
  it "matches git behavior" && { :; }
)

# mode=effect — --no-abbrev-commit: disable abbreviation even under
# --oneline.
# hash=sha1-only
title "gix log --no-abbrev-commit"
only_for_hash sha1-only && (small-repo-in-sandbox
  # TODO — expect_parity effect -- log --no-abbrev-commit --oneline
  it "matches git behavior" && { :; }
)

# mode=effect — --abbrev=<n>: set abbreviation length (used with
# --abbrev-commit).
# hash=sha1-only
title "gix log --abbrev=<n>"
only_for_hash sha1-only && (small-repo-in-sandbox
  # TODO — expect_parity effect -- log --abbrev-commit --abbrev=8
  it "matches git behavior" && { :; }
)

# --- traversal order ----------------------------------------------------

# mode=effect — --reverse: emit commits in reverse order of the traversal.
# hash=sha1-only
title "gix log --reverse"
only_for_hash sha1-only && (small-repo-in-sandbox
  # TODO — expect_parity effect -- log --reverse
  it "matches git behavior" && { :; }
)

# mode=effect — --topo-order: topological ordering (gix default).
# hash=sha1-only
title "gix log --topo-order"
only_for_hash sha1-only && (small-repo-in-sandbox
  # TODO — expect_parity effect -- log --topo-order
  it "matches git behavior" && { :; }
)

# mode=effect — --date-order: sort by commit date, still ancestor-
# respecting.
# hash=sha1-only
title "gix log --date-order"
only_for_hash sha1-only && (small-repo-in-sandbox
  # TODO — expect_parity effect -- log --date-order
  it "matches git behavior" && { :; }
)

# mode=effect — --author-date-order: sort by author date.
# hash=sha1-only
title "gix log --author-date-order"
only_for_hash sha1-only && (small-repo-in-sandbox
  # TODO — expect_parity effect -- log --author-date-order
  it "matches git behavior" && { :; }
)

# mode=effect — --first-parent: follow only the first parent of merges.
# hash=sha1-only
title "gix log --first-parent"
only_for_hash sha1-only && (small-repo-in-sandbox
  # TODO — expect_parity effect -- log --first-parent
  it "matches git behavior" && { :; }
)

# --- parent filtering ---------------------------------------------------

# mode=effect — --min-parents=<n>: require at least n parents.
# hash=sha1-only
title "gix log --min-parents=<n>"
only_for_hash sha1-only && (small-repo-in-sandbox
  # TODO — expect_parity effect -- log --min-parents=2
  it "matches git behavior" && { :; }
)

# mode=effect — --max-parents=<n>: require at most n parents (0 = roots).
# hash=sha1-only
title "gix log --max-parents=<n>"
only_for_hash sha1-only && (small-repo-in-sandbox
  # TODO — expect_parity effect -- log --max-parents=0
  it "matches git behavior" && { :; }
)

# --- decoration ---------------------------------------------------------

# mode=effect — --decorate: append ref names to each commit line
# (default auto; short form in non-TTY).
# hash=sha1-only
title "gix log --decorate"
only_for_hash sha1-only && (small-repo-in-sandbox
  # TODO — expect_parity effect -- log --decorate
  it "matches git behavior" && { :; }
)

# mode=effect — --decorate=short: strip refs/heads/ etc prefixes.
# hash=sha1-only
title "gix log --decorate=short"
only_for_hash sha1-only && (small-repo-in-sandbox
  # TODO — expect_parity effect -- log --decorate=short
  it "matches git behavior" && { :; }
)

# mode=effect — --decorate=full: include the full ref name.
# hash=sha1-only
title "gix log --decorate=full"
only_for_hash sha1-only && (small-repo-in-sandbox
  # TODO — expect_parity effect -- log --decorate=full
  it "matches git behavior" && { :; }
)

# mode=effect — --decorate=no: disable decoration.
# hash=sha1-only
title "gix log --decorate=no"
only_for_hash sha1-only && (small-repo-in-sandbox
  # TODO — expect_parity effect -- log --decorate=no
  it "matches git behavior" && { :; }
)

# mode=effect — --no-decorate: same as --decorate=no.
# hash=sha1-only
title "gix log --no-decorate"
only_for_hash sha1-only && (small-repo-in-sandbox
  # TODO — expect_parity effect -- log --no-decorate
  it "matches git behavior" && { :; }
)

# mode=effect — --decorate-refs=<pattern>: include only matching refs in
# decoration.
# hash=sha1-only
title "gix log --decorate-refs=<pattern>"
only_for_hash sha1-only && (small-repo-in-sandbox
  # TODO — expect_parity effect -- log --decorate --decorate-refs=refs/tags/*
  it "matches git behavior" && { :; }
)

# mode=effect — --decorate-refs-exclude=<pattern>: exclude matching refs
# from decoration.
# hash=sha1-only
title "gix log --decorate-refs-exclude=<pattern>"
only_for_hash sha1-only && (small-repo-in-sandbox
  # TODO — expect_parity effect -- log --decorate --decorate-refs-exclude=refs/tags/*
  it "matches git behavior" && { :; }
)

# mode=effect — --clear-decorations: reset prior --decorate-refs[-exclude]
# filters.
# hash=sha1-only
title "gix log --clear-decorations"
only_for_hash sha1-only && (small-repo-in-sandbox
  # TODO — expect_parity effect -- log --decorate --clear-decorations
  it "matches git behavior" && { :; }
)

# mode=effect — --source: prepend the ref name each commit was reached
# through.
# hash=sha1-only
title "gix log --source"
only_for_hash sha1-only && (small-repo-in-sandbox
  # TODO — expect_parity effect -- log --source --all
  it "matches git behavior" && { :; }
)

# --- graph --------------------------------------------------------------

# mode=effect — --graph: ASCII commit graph alongside each entry.
# hash=sha1-only
title "gix log --graph"
only_for_hash sha1-only && (small-repo-in-sandbox
  # TODO — expect_parity effect -- log --graph
  it "matches git behavior" && { :; }
)

# --- diff output --------------------------------------------------------

# mode=effect — -p / --patch: show the diff each commit introduces.
# hash=sha1-only
title "gix log -p"
only_for_hash sha1-only && (small-repo-in-sandbox
  # TODO — expect_parity effect -- log -p
  it "matches git behavior" && { :; }
)

# mode=effect — -s / --no-patch: suppress any diff (cancels -p/--stat).
# hash=sha1-only
title "gix log -s"
only_for_hash sha1-only && (small-repo-in-sandbox
  # TODO — expect_parity effect -- log -s -p
  it "matches git behavior" && { :; }
)

# mode=effect — --stat: diffstat per commit.
# hash=sha1-only
title "gix log --stat"
only_for_hash sha1-only && (small-repo-in-sandbox
  # TODO — expect_parity effect -- log --stat
  it "matches git behavior" && { :; }
)

# mode=effect — --shortstat: last line of --stat only.
# hash=sha1-only
title "gix log --shortstat"
only_for_hash sha1-only && (small-repo-in-sandbox
  # TODO — expect_parity effect -- log --shortstat
  it "matches git behavior" && { :; }
)

# mode=effect — --numstat: machine-friendly diffstat.
# hash=sha1-only
title "gix log --numstat"
only_for_hash sha1-only && (small-repo-in-sandbox
  # TODO — expect_parity effect -- log --numstat
  it "matches git behavior" && { :; }
)

# mode=effect — --name-only: list affected paths only.
# hash=sha1-only
title "gix log --name-only"
only_for_hash sha1-only && (small-repo-in-sandbox
  # TODO — expect_parity effect -- log --name-only
  it "matches git behavior" && { :; }
)

# mode=effect — --name-status: paths with status letters.
# hash=sha1-only
title "gix log --name-status"
only_for_hash sha1-only && (small-repo-in-sandbox
  # TODO — expect_parity effect -- log --name-status
  it "matches git behavior" && { :; }
)

# mode=effect — --raw: git-diff --raw output.
# hash=sha1-only
title "gix log --raw"
only_for_hash sha1-only && (small-repo-in-sandbox
  # TODO — expect_parity effect -- log --raw
  it "matches git behavior" && { :; }
)

# mode=effect — -M / --find-renames: detect renames in diff output.
# hash=sha1-only
title "gix log -M"
only_for_hash sha1-only && (small-repo-in-sandbox
  # TODO — expect_parity effect -- log -M -p
  it "matches git behavior" && { :; }
)

# --- file-specific ------------------------------------------------------

# mode=effect — --follow <file>: keep the file's history across renames
# (single-file only per manpage).
# hash=sha1-only
title "gix log --follow <file>"
only_for_hash sha1-only && (small-repo-in-sandbox
  # TODO — expect_parity effect -- log --follow a
  it "matches git behavior" && { :; }
)

# mode=effect — --full-diff: with pathspec, show full commit diff not
# just the path's diff.
# hash=sha1-only
title "gix log --full-diff -- <path>"
only_for_hash sha1-only && (small-repo-in-sandbox
  # TODO — expect_parity effect -- log --full-diff -p -- a
  it "matches git behavior" && { :; }
)

# mode=effect — -L <start>,<end>:<file>: line-range log.
# hash=sha1-only
title "gix log -L <start>,<end>:<file>"
only_for_hash sha1-only && (small-repo-in-sandbox
  # TODO — expect_parity effect -- log -L 1,1:a
  it "matches git behavior" && { :; }
)

# --- date formatting ----------------------------------------------------

# mode=effect — --date=relative: "N days ago" style.
# hash=sha1-only
title "gix log --date=relative"
only_for_hash sha1-only && (small-repo-in-sandbox
  # TODO — expect_parity effect -- log --date=relative
  it "matches git behavior" && { :; }
)

# mode=effect — --date=iso: ISO 8601 local dates.
# hash=sha1-only
title "gix log --date=iso"
only_for_hash sha1-only && (small-repo-in-sandbox
  # TODO — expect_parity effect -- log --date=iso
  it "matches git behavior" && { :; }
)

# mode=effect — --date=short: YYYY-MM-DD.
# hash=sha1-only
title "gix log --date=short"
only_for_hash sha1-only && (small-repo-in-sandbox
  # TODO — expect_parity effect -- log --date=short
  it "matches git behavior" && { :; }
)

# mode=effect — --date=raw: unix timestamp + timezone.
# hash=sha1-only
title "gix log --date=raw"
only_for_hash sha1-only && (small-repo-in-sandbox
  # TODO — expect_parity effect -- log --date=raw
  it "matches git behavior" && { :; }
)

# mode=effect — --date=unix: unix timestamp only.
# hash=sha1-only
title "gix log --date=unix"
only_for_hash sha1-only && (small-repo-in-sandbox
  # TODO — expect_parity effect -- log --date=unix
  it "matches git behavior" && { :; }
)

# mode=effect — --date=format:<strftime>: strftime-style format.
# hash=sha1-only
title "gix log --date=format:<strftime>"
only_for_hash sha1-only && (small-repo-in-sandbox
  # TODO — expect_parity effect -- log --date=format:%Y-%m-%d
  it "matches git behavior" && { :; }
)

# --- diff-merges --------------------------------------------------------

# mode=effect — -m: show diffs against each parent for merges.
# hash=sha1-only
title "gix log -m"
only_for_hash sha1-only && (small-repo-in-sandbox
  # TODO — expect_parity effect -- log -m -p
  it "matches git behavior" && { :; }
)

# mode=effect — -c: combined diff for merges.
# hash=sha1-only
title "gix log -c"
only_for_hash sha1-only && (small-repo-in-sandbox
  # TODO — expect_parity effect -- log -c -p
  it "matches git behavior" && { :; }
)

# mode=effect — --cc: dense combined diff (only interesting hunks).
# hash=sha1-only
title "gix log --cc"
only_for_hash sha1-only && (small-repo-in-sandbox
  # TODO — expect_parity effect -- log --cc -p
  it "matches git behavior" && { :; }
)

# mode=effect — --diff-merges=off: never show merge diffs.
# hash=sha1-only
title "gix log --diff-merges=off"
only_for_hash sha1-only && (small-repo-in-sandbox
  # TODO — expect_parity effect -- log --diff-merges=off -p
  it "matches git behavior" && { :; }
)

# mode=effect — --diff-merges=first-parent: diff against first parent.
# hash=sha1-only
title "gix log --diff-merges=first-parent"
only_for_hash sha1-only && (small-repo-in-sandbox
  # TODO — expect_parity effect -- log --diff-merges=first-parent -p
  it "matches git behavior" && { :; }
)

# --- misc log-specific --------------------------------------------------

# mode=effect — --mailmap / --use-mailmap: rewrite names via .mailmap.
# hash=sha1-only
title "gix log --mailmap"
only_for_hash sha1-only && (small-repo-in-sandbox
  # TODO — expect_parity effect -- log --mailmap
  it "matches git behavior" && { :; }
)

# mode=effect — --no-mailmap: ignore .mailmap even if configured.
# hash=sha1-only
title "gix log --no-mailmap"
only_for_hash sha1-only && (small-repo-in-sandbox
  # TODO — expect_parity effect -- log --no-mailmap
  it "matches git behavior" && { :; }
)

# mode=effect — --log-size: add "log size N" line per commit.
# hash=sha1-only
title "gix log --log-size"
only_for_hash sha1-only && (small-repo-in-sandbox
  # TODO — expect_parity effect -- log --log-size
  it "matches git behavior" && { :; }
)

# mode=effect — --notes: include notes from refs/notes/commits.
# hash=sha1-only
title "gix log --notes"
only_for_hash sha1-only && (small-repo-in-sandbox
  # TODO — expect_parity effect -- log --notes
  it "matches git behavior" && { :; }
)

# mode=effect — --no-notes: suppress notes even if a default is
# configured.
# hash=sha1-only
title "gix log --no-notes"
only_for_hash sha1-only && (small-repo-in-sandbox
  # TODO — expect_parity effect -- log --no-notes
  it "matches git behavior" && { :; }
)

# mode=effect — --show-signature: verify and print commit signatures.
# hash=sha1-only
title "gix log --show-signature"
only_for_hash sha1-only && (small-repo-in-sandbox
  # TODO — expect_parity effect -- log --show-signature
  it "matches git behavior" && { :; }
)

# --- color --------------------------------------------------------------

# mode=effect — --color=always: force color codes even when piped.
# hash=sha1-only
title "gix log --color=always"
only_for_hash sha1-only && (small-repo-in-sandbox
  # TODO — expect_parity effect -- log --color=always
  it "matches git behavior" && { :; }
)

# mode=effect — --no-color: suppress color codes unconditionally.
# hash=sha1-only
title "gix log --no-color"
only_for_hash sha1-only && (small-repo-in-sandbox
  # TODO — expect_parity effect -- log --no-color
  it "matches git behavior" && { :; }
)

# --- boundary / ancestry-path ------------------------------------------

# mode=effect — --boundary: mark excluded-range endpoints with "-".
# hash=sha1-only
title "gix log --boundary"
only_for_hash sha1-only && (small-repo-in-sandbox
  # TODO — expect_parity effect -- log --boundary main --not dev
  it "matches git behavior" && { :; }
)

# mode=effect — --ancestry-path: commits on A..B paths from A to B.
# hash=sha1-only
title "gix log --ancestry-path"
only_for_hash sha1-only && (small-repo-in-sandbox
  # TODO — expect_parity effect -- log --ancestry-path dev..main
  it "matches git behavior" && { :; }
)

# End-of-file sentinel: when every row is `only_for_hash sha1-only` and the
# active hash is sha256, the last statement returns 1 (skip), which would
# propagate out of `source` and trip `set -e` in tests/parity.sh. A trailing
# `:` normalizes the exit code so a fully-skipped file still returns 0.
:
