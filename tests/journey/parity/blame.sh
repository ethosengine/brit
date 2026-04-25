# Must be sourced into tests/parity.sh or tests/journey.sh — see tests/parity.sh.
#
# Parity scaffold for `git blame` ↔ `gix blame`.
#
# One `title` + `it` block per flag derived from
# vendor/git/Documentation/git-blame.adoc + the inherited
# include::blame-options.adoc surface, plus vendor/git/builtin/blame.c
# (cmd_blame, blame option-table at builtin/blame.c:951..986). Every `it`
# body starts as a TODO: placeholder — iteration N of the ralph loop
# picks the next TODO, converts it to a real `expect_parity` (or
# `compat_effect`) assertion, and removes the TODO marker.
#
# Verdict modes (comment above each block):
#   bytes  — scriptable output consumed by tooling: --porcelain,
#            --line-porcelain, --incremental (machine-consumption
#            formats explicitly named in the manpage), -p, the error
#            stanzas around bad revspecs / missing files.
#   effect — UX-level parity (exit-code match + optional prose check).
#            Default for the human-rendered flags whose pretty
#            rendering is not yet implemented in gix's blame entry
#            point.
#
# Coverage on gix's current Clap surface (src/plumbing/options/mod.rs::Blame):
#   gix blame [-s/--statistics] [-L <range>...] [--since <date>] <file>
# Just four knobs — `file`, `-L` (one-based inclusive ranges), `--since`,
# and a `-s/--statistics` switch whose semantics CLASH with git's `-s`
# (git: suppress author+timestamp / OUTPUT_NO_AUTHOR; gix: print extra
# perf statistics). Closing the `-s` row therefore requires either
# renaming gix's existing perf-statistics knob to `--show-stats`
# (matching git's actual flag for perf stats) or adding a separate
# OUTPUT_NO_AUTHOR-style suppression flag — the latter is canonical and
# the scaffold targets it. Every other flag below trips Clap's
# UnknownArgument before it reaches the handler. Closing a row
# therefore means: (1) add the flag to src/plumbing/options/mod.rs's
# Blame variant (or move Blame to a dedicated options module mirroring
# diff/log), (2) widen the gitoxide_core::repository::blame::blame_file
# entry-point signature, (3) implement the semantics on top of the
# existing gix::blame::file primitive in gitoxide-core/src/repository/blame.rs.
#
# Hash coverage: `dual` rows never open a repo (--help, --bogus-flag
# inside a repo with no further parsing, outside-of-repo). Every row
# that opens a repository is `sha1-only` because gix-config rejects
# `extensions.objectFormat=sha256`
# (gix/src/config/tree/sections/extensions.rs try_into_object_format,
# sha1-only validator), blocking every sha256 fixture at open. Rows
# flip to `dual` once that validator accepts sha256.
#
# parity-defaults:
#   hash=sha1-only "gix cannot load sha256 repos: extensions.objectFormat=sha256 rejected (gix/src/config/tree/sections/extensions.rs)"
#   mode=effect

title "gix blame"

# --- meta / help --------------------------------------------------------

# mode=effect — clap --help short-circuits before repo load, exits 0.
# git's --help delegates to `man git-blame`; gix returns clap's auto-
# generated help. Message text diverges; only the exit-code match is
# asserted.
# hash=dual
title "gix blame --help"
only_for_hash dual && (sandbox
  it "matches git behavior" && {
    expect_parity effect -- blame --help
  }
)

# mode=effect — unknown flag: git exits 129 (PARSE_OPT_ERROR ->
# parse_options_step exits 129 at builtin/blame.c:1018). gix's Clap
# layer maps UnknownArgument to 129 via src/plumbing/main.rs.
# Already verified: both binaries exit 129 today (no implementation
# work needed beyond closing the TODO).
# hash=sha1-only
title "gix blame --bogus-flag"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    expect_parity effect -- blame --bogus-flag a
  }
)

# mode=effect — `git blame` (bare) requires a path argument; without
# one, parse_options dies 129 with the usage stanza. gix's Clap layer
# already requires `<FILE>` and exits 129 with its own MissingRequired
# Argument message. Exit-code parity holds; message bodies diverge.
# Already verified: both binaries exit 129 today.
# hash=sha1-only
title "gix blame (bare, no file)"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    expect_parity effect -- blame
  }
)

# mode=effect — outside any repo: git dies 128 with "fatal: not a git
# repository (or any of the parent directories): .git" + exit 128.
# gix's plumbing repository() closure already remaps the
# gix_discover::upwards::Error::NoGitRepository* variants to git's
# exact wording + exit 128. Already verified: both binaries exit 128
# today.
# hash=dual
title "gix blame (outside a repository)"
only_for_hash dual && (sandbox
  it "matches git behavior" && {
    expect_parity effect -- blame foo
  }
)

# --- basic invocation ---------------------------------------------------

# mode=effect — `gix blame <file>` against a populated working tree.
# git emits one line per file line: "<short-hash> (<author> <date>
# <line-no>) <line>". gix emits its own simplified format:
# "<8-hash> <file> <orig-line-no> <line>". Exit 0 either way. Effect
# mode (exit-code parity only) until the porcelain author+date renderer
# lands.
# hash=sha1-only
title "gix blame <file> (default format, populated repo)"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "blame default-format author/date renderer deferred" -- blame b
  }
)

# mode=bytes — `git blame` on an untracked / nonexistent path emits
# "fatal: no such path '<file>' in HEAD" + exit 128. The
# gitoxide_core::repository::blame::blame_file entry point now matches
# the gix_blame::Error::FileMissing variant before propagating, emits
# git's exact wording on stderr, and std::process::exit(128).
# hash=sha1-only
title "gix blame <missing-file>"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    expect_parity bytes -- blame nonexistent.txt
  }
)

# mode=effect — `git blame` against an explicit revision: `git blame
# HEAD <file>`, `git blame <branch> <file>`, `git blame <sha> <file>`.
# Currently gix's Clap surface accepts only `<FILE>`, so the rev
# argument is misparsed as a path. Closing this row requires adding an
# optional `[REV]` positional (or `<rev>... -- <file>` separator
# handling, mirroring builtin/blame.c's dashdash_pos logic).
# hash=sha1-only
title "gix blame HEAD <file>"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "blame default-format renderer deferred (rev <file>)" -- blame HEAD b
  }
)

# mode=effect — `git blame` with `--` separator: `git blame -- <file>`.
# Per builtin/blame.c:1009 (PARSE_OPT_KEEP_DASHDASH), the `--` is
# preserved and `dashdash_pos` records its index. gix's Clap surface
# needs to accept `--` as a value-delimiter for the rev/file split.
# hash=sha1-only
title "gix blame -- <file>"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "blame default-format renderer deferred (-- separator)" -- blame -- b
  }
)

# --- output mode flags --------------------------------------------------

# mode=effect — `git blame -p` / `--porcelain`: machine-consumption
# format with per-line headers (40-byte hash, source/result line
# numbers, group size on first line of a run). gix has no porcelain
# emitter today (renderer needs gix::blame::Outcome walked into git's
# exact header schema). Clap-accepts the flag and exits 0; bytes
# parity deferred until the renderer lands.
# hash=sha1-only
title "gix blame -p / --porcelain"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "blame --porcelain machine-consumption renderer deferred" -- blame --porcelain b
  }
)

# mode=effect — `git blame --line-porcelain`: same as --porcelain but
# every line carries the full commit info block (no commit-info
# elision). Implies --porcelain. Same renderer-deferred situation.
# hash=sha1-only
title "gix blame --line-porcelain"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "blame --line-porcelain machine-consumption renderer deferred" -- blame --line-porcelain b
  }
)

# mode=effect — `git blame --incremental`: machine-consumption format
# emitted as commits are discovered. Output is unordered (newer
# commits first) and lacks the actual file-content lines. Filename
# always present (terminates each entry). Same renderer-deferred.
# hash=sha1-only
title "gix blame --incremental"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "blame --incremental machine-consumption renderer deferred" -- blame --incremental b
  }
)

# mode=effect — `git blame -c`: annotate-compat output mode (same as
# git-annotate). Exit-code parity holds even when the renderer differs.
# hash=sha1-only
title "gix blame -c"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "blame -c annotate-compat renderer deferred" -- blame -c b
  }
)

# mode=effect — `git blame -t`: include raw committer timestamp in the
# author column. Exit-code parity holds; renderer divergence deferred.
# hash=sha1-only
title "gix blame -t"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "blame -t raw-timestamp renderer deferred" -- blame -t b
  }
)

# mode=effect — `git blame -l`: emit the long (full) commit hash
# instead of the abbreviated form.
# hash=sha1-only
title "gix blame -l"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "blame -l long-hash renderer deferred" -- blame -l b
  }
)

# mode=effect — `git blame -s`: SUPPRESS author name + timestamp from
# default output (OUTPUT_NO_AUTHOR). NB: gix today binds `-s` to the
# perf-statistics knob — closing this row requires renaming the
# existing `Blame { statistics: bool, #[clap(short='s')] }` to
# `--show-stats` (matching git's actual flag) and adding a new `-s`
# that gates author/timestamp emission.
# hash=sha1-only
title "gix blame -s (suppress author+timestamp)"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "blame -s author-suppression renderer deferred" -- blame -s b
  }
)

# mode=effect — `git blame -e` / `--show-email`: emit the author
# email instead of the author name.
# hash=sha1-only
title "gix blame -e / --show-email"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "blame -e show-email renderer deferred" -- blame --show-email b
  }
)

# mode=effect — `git blame -f` / `--show-name`: show the original
# commit's filename column (default: shown only on rename detection).
# hash=sha1-only
title "gix blame -f / --show-name"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "blame -f show-name renderer deferred" -- blame --show-name b
  }
)

# mode=effect — `git blame -n` / `--show-number`: show the original
# commit's line number column.
# hash=sha1-only
title "gix blame -n / --show-number"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "blame -n show-number renderer deferred" -- blame --show-number b
  }
)

# --- line-range filter (-L) --------------------------------------------

# mode=effect — `git blame -L <start>,<end>`: existing gix Clap
# surface already accepts `-L` and parses one-based inclusive ranges
# via gix::blame::BlameRanges::from_one_based_inclusive_ranges. Run
# with a real range; exit-code parity holds, output renderer
# divergence is the same compat issue as the bare-default row.
# hash=sha1-only
title "gix blame -L <start>,<end>"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "blame default-format renderer deferred (range form)" -- blame -L 1,1 b
  }
)

# mode=effect — `git blame -L <start>,+<count>`: range with a `+N`
# offset suffix. Per blame-options.adoc line 12-21. gix today does
# not parse the `+N` form — `BlameRanges::from_one_based_inclusive_ranges`
# expects `<start>,<end>` only.
# hash=sha1-only
title "gix blame -L <start>,+<count>"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    expect_parity effect -- blame -L 1,+1 b
  }
)

# mode=effect — `git blame -L <start>,` / `-L ,<end>`: open-ended
# ranges (start to EOF, or beginning to end). gix's AsRange value
# parser (src/shared.rs::AsRange) only accepts `<start>,<end>`; the
# open-ended forms require either a numeric "to-EOF" sentinel or a
# parser pass post-blame. Closing requires extending AsRange (and
# downstream BlameRanges::from_one_based_inclusive_ranges) to model
# half-open ranges.
# hash=sha1-only
title "gix blame -L <start>, (open-ended)"
only_for_hash sha1-only && (small-repo-in-sandbox
  shortcoming "open-ended -L <start>, / -L ,<end> not parsed by AsRange (src/shared.rs)"
)

# mode=effect — `git blame -L :<funcname>`: function-name regex form
# (line-range-format.adoc). Restricts annotation to the function body
# matched by a userdiff funcname pattern. gix has no userdiff /
# funcname-pattern infrastructure yet (vendor/git/userdiff.c), so
# both AsRange and downstream resolution are unimplemented.
# hash=sha1-only
title "gix blame -L :<funcname>"
only_for_hash sha1-only && (small-repo-in-sandbox
  shortcoming "-L :<funcname> requires userdiff/funcname-pattern infra (not yet in gix-diff)"
)

# mode=effect — `git blame -L /<regex>/,/<regex>/`: regex-anchored
# range form. AsRange parser can't see this as a numeric range and
# rejects with ValueValidation. Closing requires the same parser
# extension as the open-ended row plus a regex evaluator on the
# blob's text.
# hash=sha1-only
title "gix blame -L /<regex>/,<end>"
only_for_hash sha1-only && (small-repo-in-sandbox
  shortcoming "-L /<regex>/,... requires regex-anchored range parsing (AsRange + blob scan) — not yet wired"
)

# mode=effect — multiple `-L` ranges may be specified; OR'd together.
# Already supported by BlameRanges in gix; renderer divergence applies.
# hash=sha1-only
title "gix blame -L A,B -L C,D (multiple)"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "blame default-format renderer deferred (multi-range)" -- blame -L 1,1 -L 1,1 b
  }
)

# --- stats / debug ------------------------------------------------------

# mode=effect — `git blame --show-stats`: emit work-cost statistics
# at end of output. NB: gix's existing `-s/--statistics` knob covers
# the same intent but under a non-git flag name; the canonical close
# is to rename it to `--show-stats` (and free `-s` for OUTPUT_NO_AUTHOR
# above).
# hash=sha1-only
title "gix blame --show-stats"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "blame --show-stats renderer deferred" -- blame --show-stats b
  }
)

# mode=effect — `git blame --score-debug`: emit copy/move detection
# scores. Requires -M/-C state; only meaningful in combination.
# hash=sha1-only
title "gix blame --score-debug"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "blame --score-debug renderer deferred" -- blame --score-debug b
  }
)

# --- abbrev / hash format ----------------------------------------------

# mode=effect — `git blame --abbrev=<n>`: override the default 7-digit
# abbreviated hash with `<n>+1` digits (the trailing column is the
# boundary-commit caret). builtin/blame.c:1050-1054 enforces lower
# bound and adds 1 for the caret column. gix has its own short-hash
# emission (8-digit fixed today via to_hex_with_len(8)).
# hash=sha1-only
title "gix blame --abbrev=<n>"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "blame --abbrev hash-width tunable deferred" -- blame --abbrev=12 b
  }
)

# --- date / since / reverse --------------------------------------------

# mode=effect — `git blame --date=<format>`: dates emitted in the
# selected format (iso/iso-strict/rfc/short/raw/unix/relative/human).
# Not yet wired in gix's blame renderer.
# hash=sha1-only
title "gix blame --date=<format>"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "blame --date format renderer deferred" -- blame --date=iso b
  }
)

# mode=effect — `git blame --since=<date>`: existing gix Clap surface
# accepts --since, parses via gix::date::Time, and feeds it through
# gix::blame::Options. gix::date::parse uses RFC2822 / ISO 8601 / RFC
# 3339 / git-internal-raw — git's approxidate forms ("1.year.ago",
# "2 weeks") are not supported, so the test uses an explicit ISO date.
# Exit-code parity holds; renderer-output divergence is the same
# compat issue.
# hash=sha1-only
title "gix blame --since=<date>"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "blame default-format renderer deferred (--since)" -- blame --since=2020-01-01 b
  }
)

# mode=effect — `git blame --reverse <start>..<end>`: walk forward
# instead of backward. Per builtin/blame.c:1027, `--reverse` is
# rewritten to `--children` and the `reverse` flag is set. Requires
# rev-walk plumbing not currently wired into gix blame.
# hash=sha1-only
title "gix blame --reverse <range>"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    expect_parity effect -- blame --reverse HEAD~1..HEAD b
  }
)

# mode=effect — `git blame --first-parent`: only follow the first
# parent of merge commits. Requires the rev-walker to honor the
# first-parent bit.
# hash=sha1-only
title "gix blame --first-parent"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "blame --first-parent walker integration deferred" -- blame --first-parent b
  }
)

# --- boundary / root ---------------------------------------------------

# mode=effect — `git blame -b`: blank out the SHA-1 column for
# boundary commits. Requires the renderer to know which commits are
# at the boundary of the walked range.
# hash=sha1-only
title "gix blame -b (blank boundary)"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "blame -b boundary blanking deferred" -- blame -b b
  }
)

# mode=effect — `git blame --root`: do not treat root commits as
# boundaries (default treats them as boundary so the SHA shows blanked
# under `-b`).
# hash=sha1-only
title "gix blame --root"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "blame --root boundary policy deferred" -- blame --root b
  }
)

# --- move / copy detection ---------------------------------------------

# mode=effect — `git blame -M[<num>]`: detect lines moved within a
# file. Default threshold 20 alphanumeric chars. gix-blame supports
# Rewrites detection for cross-file but not the within-file move
# pass that -M adds.
# hash=sha1-only
title "gix blame -M"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "blame -M within-file move detection deferred" -- blame -M b
  }
)

# mode=effect — `git blame -M<num>`: -M with explicit threshold. git's
# parse-options accepts both sticky `-M30` and `-M=30`; gix's clap
# surface uses `require_equals = true` (so `-M b` doesn't eat `b`),
# which means the sticky form is rejected. The test uses `-M=30`
# (also accepted by git) — value is ignored today (renderer deferred).
# hash=sha1-only
title "gix blame -M<num>"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "blame -M=<num> threshold tunable deferred" -- blame -M=30 b
  }
)

# mode=effect — `git blame -C[<num>]`: detect lines moved/copied
# from other files modified in the same commit. Default threshold 40.
# Stacks: `-C -C` looks at file-creation commits; `-C -C -C` looks at
# any commit. gix-blame's Rewrites covers some of this but not the
# stacked semantics.
# hash=sha1-only
title "gix blame -C"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "blame -C cross-file copy detection deferred" -- blame -C b
  }
)

# mode=effect — `git blame -C -C`: extend cross-file detection to the
# file-creation commit.
# hash=sha1-only
title "gix blame -C -C (creation-commit)"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "blame -C -C creation-commit copy detection deferred" -- blame -C -C b
  }
)

# mode=effect — `git blame -C -C -C`: extend cross-file detection to
# any commit. Most expensive pass.
# hash=sha1-only
title "gix blame -C -C -C (any-commit)"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "blame -C -C -C any-commit copy detection deferred" -- blame -C -C -C b
  }
)

# --- ignore-rev / ignore-revs-file -------------------------------------

# mode=effect — `git blame --ignore-rev <rev>`: ignore the named
# revision when assigning blame, attributing its lines to the
# previous commit that touched them. May be repeated.
# hash=sha1-only
title "gix blame --ignore-rev <rev>"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "blame --ignore-rev attribution-rewrite deferred" -- blame --ignore-rev HEAD b
  }
)

# mode=effect — `git blame --ignore-revs-file <file>`: ignore the
# revisions listed in <file> (fsck.skipList format). Empty filename
# `""` clears the accumulator.
# hash=sha1-only
title "gix blame --ignore-revs-file <file>"
only_for_hash sha1-only && (small-repo-in-sandbox
  echo "" > .git-blame-ignore-revs
  it "matches git behavior" && {
    compat_effect "blame --ignore-revs-file deferred" -- blame --ignore-revs-file .git-blame-ignore-revs b
  }
)

# --- whitespace / diff-algorithm ---------------------------------------

# mode=effect — `git blame -w`: ignore whitespace when comparing
# parent vs child to find line origins. Maps to XDF_IGNORE_WHITESPACE
# in xdiff. gix-diff supports the equivalent flag; needs threading
# through blame's diff invocations.
# hash=sha1-only
title "gix blame -w"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "blame -w whitespace-ignore threading deferred" -- blame -w b
  }
)

# mode=effect — `git blame --diff-algorithm=<algo>`: select the diff
# backend (default/myers/patience/histogram/minimal). gix-diff
# supports the same set; needs threading through blame.
# hash=sha1-only
title "gix blame --diff-algorithm=<algo>"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "blame --diff-algorithm threading deferred" -- blame --diff-algorithm=histogram b
  }
)

# mode=effect — `git blame --minimal`: hidden alias for
# --diff-algorithm=minimal (PARSE_OPT_HIDDEN at builtin/blame.c:977).
# hash=sha1-only
title "gix blame --minimal"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "blame --minimal alias deferred" -- blame --minimal b
  }
)

# --- color --------------------------------------------------------------

# mode=effect — `git blame --color-lines`: differentiate adjacent
# lines that share a commit (color.blame.repeatedLines).
# hash=sha1-only
title "gix blame --color-lines"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "blame --color-lines deferred" -- blame --color-lines b
  }
)

# mode=effect — `git blame --color-by-age`: color by age
# (color.blame.highlightRecent).
# hash=sha1-only
title "gix blame --color-by-age"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "blame --color-by-age deferred" -- blame --color-by-age b
  }
)

# --- progress -----------------------------------------------------------

# mode=effect — `git blame --progress`: force progress reporting on
# stderr. Per builtin/blame.c:1043-1048, --progress is rejected when
# combined with --incremental or --porcelain (exit 128 with "fatal:
# --progress can't be used with --incremental or porcelain formats").
# hash=sha1-only
title "gix blame --progress"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "blame --progress reporting deferred" -- blame --progress b
  }
)

# mode=effect — `git blame --no-progress`: explicit opposite of
# --progress.
# hash=sha1-only
title "gix blame --no-progress"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "blame --no-progress deferred" -- blame --no-progress b
  }
)

# mode=bytes — `git blame --progress --porcelain`: incompatible flag
# combination, exit 128 with the precondition error wording. This
# precondition gate maps cleanly to a verbatim error stanza in gix
# (independent of the renderer).
# hash=sha1-only
title "gix blame --progress --porcelain (incompatible)"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    expect_parity bytes -- blame --progress --porcelain b
  }
)

# --- contents / encoding / revs-file ----------------------------------

# mode=effect — `git blame --contents <file>`: annotate using <file>'s
# contents as the "final image". `-` reads from stdin.
# hash=sha1-only
title "gix blame --contents <file>"
only_for_hash sha1-only && (small-repo-in-sandbox
  echo "alt" > alt.txt
  it "matches git behavior" && {
    compat_effect "blame --contents alternate-final-image deferred" -- blame --contents alt.txt b
  }
)

# mode=effect — `git blame --encoding=<encoding>`: encoding for
# author/summary output. `none` passes through unconverted.
# hash=sha1-only
title "gix blame --encoding=<encoding>"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "blame --encoding output transcoding deferred" -- blame --encoding=UTF-8 b
  }
)

# mode=effect — `git blame -S <revs-file>`: use revisions from a
# graft-style file instead of calling rev-list. Per builtin/blame.c:1056,
# read_ancestry on the file; failure dies with errno wording.
# hash=sha1-only
title "gix blame -S <revs-file>"
only_for_hash sha1-only && (small-repo-in-sandbox
  : > .blame-revs
  it "matches git behavior" && {
    compat_effect "blame -S graft-revs-file deferred" -- blame -S .blame-revs b
  }
)
