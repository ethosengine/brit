//! `gix show` plumbing CLI.
//!
//! Flag surface mirrors `vendor/git/builtin/log.c::cmd_show` (entry
//! point at `vendor/git/builtin/log.c:657`) and
//! `vendor/git/Documentation/git-show.adoc`. `cmd_show` calls
//! `cmd_log_init` which loads the same flag table as `git log`, so
//! the user-visible surface is the union of `pretty-options.adoc`,
//! `diff-options.adoc`, and `diff-generate-patch.adoc`. Parity
//! coverage lives in `tests/journey/parity/show.sh`.
//!
//! `git show` defaults to the `HEAD` revision when no `<object>` is
//! given (see `setup_revision_opt::def = "HEAD"` at
//! `vendor/git/builtin/log.c:688`). Walk-mode is suppressed via
//! `rev.no_walk = 1` (`vendor/git/builtin/log.c:684`); a per-object
//! switch then renders blob / tag / tree / commit. Today the
//! porcelain stub at `gitoxide_core::repository::show::porcelain`
//! emits a placeholder note + exits 0 except for the
//! ambiguous-argument and unborn-HEAD precondition gates which match
//! git byte-exactly. Per-row entries in
//! `tests/journey/parity/show.sh` close each flag with
//! `compat_effect "deferred until show driver lands"` until the real
//! driver lands.

use gix::bstr::BString;

#[derive(Debug, clap::Parser)]
#[command(about = "Show various types of objects")]
pub struct Platform {
    // ── pretty-options.adoc ──────────────────────────────────────────
    /// Pretty-print the contents of the commit logs in a given format.
    /// Mirrors `pretty-options.adoc:1` `--pretty[=<format>]`.
    #[clap(long, value_parser = crate::shared::AsBString, value_name = "format", num_args = 0..=1, default_missing_value = "medium", require_equals = true)]
    pub pretty: Option<BString>,

    /// Custom format string (shorthand for `--pretty=format:<fmt>`).
    /// Mirrors `pretty-options.adoc:2` `--format=<format>`.
    #[clap(long, value_parser = crate::shared::AsBString, value_name = "format")]
    pub format: Option<BString>,

    /// Abbreviate shown commit hashes.
    /// Mirrors `pretty-options.adoc:16` `--abbrev-commit`.
    #[clap(long, overrides_with = "no_abbrev_commit")]
    pub abbrev_commit: bool,

    /// Show full commit hashes (cancels --abbrev-commit).
    /// Mirrors `pretty-options.adoc:25` `--no-abbrev-commit`.
    #[clap(long = "no-abbrev-commit", overrides_with = "abbrev_commit")]
    pub no_abbrev_commit: bool,

    /// Shorthand for `--pretty=oneline --abbrev-commit`.
    /// Mirrors `pretty-options.adoc:30` `--oneline`.
    #[clap(long)]
    pub oneline: bool,

    /// Re-encode commit log messages in the given encoding.
    /// Mirrors `pretty-options.adoc:34` `--encoding=<encoding>`.
    #[clap(long, value_parser = crate::shared::AsBString, value_name = "encoding")]
    pub encoding: Option<BString>,

    /// Tab expansion in commit messages.
    /// Mirrors `pretty-options.adoc:46` `--expand-tabs[=<n>]`.
    #[clap(long, value_name = "n", num_args = 0..=1, default_missing_value = "8", require_equals = true)]
    pub expand_tabs: Option<usize>,

    /// Disable --expand-tabs (alias for --expand-tabs=0).
    /// Mirrors `pretty-options.adoc:48` `--no-expand-tabs`.
    #[clap(long = "no-expand-tabs")]
    pub no_expand_tabs: bool,

    /// Show notes annotating shown commits. Optional `<ref>` selects
    /// a non-default notes ref. Mirrors `pretty-options.adoc:60`
    /// `--notes[=<ref>]`.
    #[clap(long, value_parser = crate::shared::AsBString, value_name = "ref", num_args = 0..=1, default_missing_value = "", require_equals = true)]
    pub notes: Option<BString>,

    /// Suppress notes display. Mirrors `pretty-options.adoc:81`
    /// `--no-notes`.
    #[clap(long = "no-notes")]
    pub no_notes: bool,

    /// Show notes by default unless overridden. Mirrors
    /// `pretty-options.adoc:88` `--show-notes-by-default`.
    #[clap(long = "show-notes-by-default")]
    pub show_notes_by_default: bool,

    /// Deprecated alias for `--notes`. Mirrors
    /// `pretty-options.adoc:91` `--show-notes[=<ref>]`.
    #[clap(long = "show-notes", value_parser = crate::shared::AsBString, value_name = "ref", num_args = 0..=1, default_missing_value = "", require_equals = true)]
    pub show_notes: Option<BString>,

    /// Show only the standard notes refs. Deprecated alias.
    /// Mirrors `pretty-options.adoc` (`--standard-notes`).
    #[clap(long = "standard-notes")]
    pub standard_notes: bool,

    /// Suppress standard notes refs. Deprecated alias.
    /// Mirrors `pretty-options.adoc` (`--no-standard-notes`).
    #[clap(long = "no-standard-notes")]
    pub no_standard_notes: bool,

    /// Verify and print commit signatures. Mirrors
    /// `pretty-options.adoc` `--show-signature`.
    #[clap(long = "show-signature")]
    pub show_signature: bool,

    // ── diff-options.adoc / diff-generate-patch.adoc ─────────────────
    /// Show the diff (default for commits). Mirrors
    /// `diff-generate-patch.adoc` `-p` / `--patch`.
    #[clap(short = 'p', long, overrides_with = "no_patch")]
    pub patch: bool,

    /// Suppress diff output. Mirrors `diff-generate-patch.adoc`
    /// `-s` / `--no-patch`.
    #[clap(short = 's', long = "no-patch", overrides_with = "patch")]
    pub no_patch: bool,

    /// Number of context lines for unified diff. Mirrors
    /// `diff-options.adoc` `-U<n>` / `--unified=<n>`.
    #[clap(short = 'U', long, value_name = "n")]
    pub unified: Option<usize>,

    /// Send diff output to <file>. Mirrors `diff-options.adoc`
    /// `--output=<file>`.
    #[clap(long, value_parser = crate::shared::AsBString, value_name = "file")]
    pub output: Option<BString>,

    /// Override the "+" marker for added lines.
    #[clap(long, value_parser = crate::shared::AsBString, value_name = "char")]
    pub output_indicator_new: Option<BString>,

    /// Override the "-" marker for removed lines.
    #[clap(long, value_parser = crate::shared::AsBString, value_name = "char")]
    pub output_indicator_old: Option<BString>,

    /// Override the " " marker for context lines.
    #[clap(long, value_parser = crate::shared::AsBString, value_name = "char")]
    pub output_indicator_context: Option<BString>,

    /// Emit git-diff --raw output.
    #[clap(long)]
    pub raw: bool,

    /// Emit patch alongside --raw.
    #[clap(long = "patch-with-raw")]
    pub patch_with_raw: bool,

    /// Use indent-based heuristic for hunk boundaries.
    #[clap(long = "indent-heuristic", overrides_with = "no_indent_heuristic")]
    pub indent_heuristic: bool,

    /// Disable --indent-heuristic.
    #[clap(long = "no-indent-heuristic", overrides_with = "indent_heuristic")]
    pub no_indent_heuristic: bool,

    /// Spend extra time to ensure the smallest possible diff.
    #[clap(long)]
    pub minimal: bool,

    /// Use the "patience diff" algorithm.
    #[clap(long)]
    pub patience: bool,

    /// Use the "histogram diff" algorithm.
    #[clap(long)]
    pub histogram: bool,

    /// Anchored-diff anchor text (may be repeated).
    #[clap(long, value_parser = crate::shared::AsBString, value_name = "text", action = clap::ArgAction::Append)]
    pub anchored: Vec<BString>,

    /// Diff algorithm selection. Mirrors `diff-options.adoc`
    /// `--diff-algorithm=<algo>`.
    #[clap(long, value_parser = crate::shared::AsBString, value_name = "algo")]
    pub diff_algorithm: Option<BString>,

    /// Print diffstat per commit.
    #[clap(long, value_parser = crate::shared::AsBString, value_name = "spec", num_args = 0..=1, default_missing_value = "", require_equals = true)]
    pub stat: Option<BString>,

    /// Print machine-friendly diffstat.
    #[clap(long)]
    pub numstat: bool,

    /// Print only the summary line of --stat.
    #[clap(long)]
    pub shortstat: bool,

    /// Compact summary (--summary + per-commit compacting).
    #[clap(long = "compact-summary")]
    pub compact_summary: bool,

    /// Cumulative dirstat.
    #[clap(long)]
    pub cumulative: bool,

    /// Dirstat rendering mode (--dirstat[=<param>,…]).
    #[clap(long, value_parser = crate::shared::AsBString, value_name = "spec", num_args = 0..=1, default_missing_value = "", require_equals = true)]
    pub dirstat: Option<BString>,

    /// Print a condensed summary of extended header information.
    #[clap(long)]
    pub summary: bool,

    /// Emit patch alongside --stat.
    #[clap(long = "patch-with-stat")]
    pub patch_with_stat: bool,

    /// NUL-terminate output records.
    #[clap(short = 'z')]
    pub z: bool,

    /// List only affected file names.
    #[clap(long = "name-only")]
    pub name_only: bool,

    /// List affected file names with status letters.
    #[clap(long = "name-status")]
    pub name_status: bool,

    /// Submodule diff rendering mode.
    #[clap(long, value_parser = crate::shared::AsBString, value_name = "format", num_args = 0..=1, default_missing_value = "log", require_equals = true)]
    pub submodule: Option<BString>,

    /// Color output control.
    #[clap(long, value_parser = crate::shared::AsBString, value_name = "when", num_args = 0..=1, default_missing_value = "always", require_equals = true)]
    pub color: Option<BString>,

    /// Disable color output (alias for --color=never).
    #[clap(long = "no-color")]
    pub no_color: bool,

    /// Highlight moved lines within diffs.
    #[clap(long = "color-moved", value_parser = crate::shared::AsBString, value_name = "mode", num_args = 0..=1, default_missing_value = "default", require_equals = true)]
    pub color_moved: Option<BString>,

    /// Disable --color-moved.
    #[clap(long = "no-color-moved")]
    pub no_color_moved: bool,

    /// Whitespace-handling for moved-line coloring.
    #[clap(long = "color-moved-ws", value_parser = crate::shared::AsBString, value_name = "mode")]
    pub color_moved_ws: Option<BString>,

    /// Disable --color-moved-ws.
    #[clap(long = "no-color-moved-ws")]
    pub no_color_moved_ws: bool,

    /// Emit word-level diff.
    #[clap(long = "word-diff", value_parser = crate::shared::AsBString, value_name = "mode", num_args = 0..=1, default_missing_value = "plain", require_equals = true)]
    pub word_diff: Option<BString>,

    /// Regex defining word boundaries for --word-diff.
    #[clap(long = "word-diff-regex", value_parser = crate::shared::AsBString, value_name = "regex")]
    pub word_diff_regex: Option<BString>,

    /// Word-based diff coloring with optional regex.
    #[clap(long = "color-words", value_parser = crate::shared::AsBString, value_name = "regex", num_args = 0..=1, default_missing_value = "", require_equals = true)]
    pub color_words: Option<BString>,

    /// Disable rename detection.
    #[clap(long = "no-renames")]
    pub no_renames: bool,

    /// Treat empty files as the empty-content sentinel for rename detection.
    #[clap(long = "rename-empty", overrides_with = "no_rename_empty")]
    pub rename_empty: bool,

    /// Disable --rename-empty.
    #[clap(long = "no-rename-empty", overrides_with = "rename_empty")]
    pub no_rename_empty: bool,

    /// Complain about whitespace / conflict markers introduced.
    #[clap(long)]
    pub check: bool,

    /// Whitespace-error highlight kind.
    #[clap(long = "ws-error-highlight", value_parser = crate::shared::AsBString, value_name = "kind")]
    pub ws_error_highlight: Option<BString>,

    /// Emit full index hashes in diff output.
    #[clap(long = "full-index")]
    pub full_index: bool,

    /// Emit binary patches.
    #[clap(long)]
    pub binary: bool,

    /// Length of abbreviated hashes.
    #[clap(long, value_name = "n", num_args = 0..=1, default_missing_value = "", require_equals = true)]
    pub abbrev: Option<String>,

    /// Break rewrites into delete-then-create pairs.
    #[clap(short = 'B', long = "break-rewrites", value_parser = crate::shared::AsBString, value_name = "n/m", num_args = 0..=1, default_missing_value = "", require_equals = true)]
    pub break_rewrites: Option<BString>,

    /// Detect renames (short `-M[<n>]`, long `--find-renames[=<n>]`).
    #[clap(short = 'M', long = "find-renames", value_parser = crate::shared::AsBString, value_name = "n", num_args = 0..=1, default_missing_value = "", require_equals = true)]
    pub find_renames: Option<BString>,

    /// Detect copies (short `-C[<n>]`, long `--find-copies[=<n>]`).
    #[clap(short = 'C', long = "find-copies", value_parser = crate::shared::AsBString, value_name = "n", num_args = 0..=1, default_missing_value = "", require_equals = true)]
    pub find_copies: Option<BString>,

    /// Try harder to detect copies.
    #[clap(long = "find-copies-harder")]
    pub find_copies_harder: bool,

    /// Drop deletion-only hunks in repeatable order.
    #[clap(short = 'D', long = "irreversible-delete")]
    pub irreversible_delete: bool,

    /// Rename-detection exhaustive scan cap.
    #[clap(short = 'l', value_name = "num")]
    pub rename_detection_limit: Option<usize>,

    /// Filter diff output by status letters.
    #[clap(long = "diff-filter", value_parser = crate::shared::AsBString, value_name = "filter")]
    pub diff_filter: Option<BString>,

    /// Show only commits whose diff changes the occurrence count of <string>.
    #[clap(short = 'S', value_parser = crate::shared::AsBString, value_name = "string")]
    pub pickaxe_string_s: Option<BString>,

    /// Show only commits that add or remove a line matching <regex>.
    #[clap(short = 'G', value_parser = crate::shared::AsBString, value_name = "regex")]
    pub pickaxe_regex_g: Option<BString>,

    /// Filter commits that change an object matching <oid>.
    #[clap(long = "find-object", value_parser = crate::shared::AsBString, value_name = "oid")]
    pub find_object: Option<BString>,

    /// Include merge commits when pickaxe-matching.
    #[clap(long = "pickaxe-all")]
    pub pickaxe_all: bool,

    /// Treat -S <string> as a regex (implied by -G).
    #[clap(long = "pickaxe-regex")]
    pub pickaxe_regex: bool,

    /// Path-ordering file. git show accepts only the short `-O <file>`
    /// form (vendor/git/diff.c::diff_opt_orderfile is wired with
    /// `OPT_FILENAME('O', NULL, ...)` — no long alias).
    #[clap(short = 'O', value_parser = crate::shared::AsBString, value_name = "file")]
    pub orderfile: Option<BString>,

    /// Skip to first-matching path when emitting diffs.
    #[clap(long = "skip-to", value_parser = crate::shared::AsBString, value_name = "path")]
    pub skip_to: Option<BString>,

    /// Rotate the diff output so <path> appears first.
    #[clap(long = "rotate-to", value_parser = crate::shared::AsBString, value_name = "path")]
    pub rotate_to: Option<BString>,

    /// Reverse the sense of old/new in diff output.
    #[clap(short = 'R')]
    pub reverse_diff: bool,

    /// Make pathnames relative to <dir>.
    #[clap(long, value_parser = crate::shared::AsBString, value_name = "path", num_args = 0..=1, default_missing_value = "", require_equals = true)]
    pub relative: Option<BString>,

    /// Disable --relative.
    #[clap(long = "no-relative")]
    pub no_relative: bool,

    /// Treat all files as text.
    #[clap(short = 'a', long)]
    pub text: bool,

    /// Ignore CR at end of line.
    #[clap(long = "ignore-cr-at-eol")]
    pub ignore_cr_at_eol: bool,

    /// Ignore whitespace at end of line.
    #[clap(long = "ignore-space-at-eol")]
    pub ignore_space_at_eol: bool,

    /// Ignore amount of whitespace (but not presence).
    #[clap(short = 'b', long = "ignore-space-change")]
    pub ignore_space_change: bool,

    /// Ignore any line containing only whitespace.
    #[clap(short = 'w', long = "ignore-all-space")]
    pub ignore_all_space: bool,

    /// Ignore changes whose lines are blank.
    #[clap(long = "ignore-blank-lines")]
    pub ignore_blank_lines: bool,

    /// Ignore lines matching the regex (long form).
    #[clap(long = "ignore-matching-lines", value_parser = crate::shared::AsBString, value_name = "regex")]
    pub ignore_matching_lines_long: Option<BString>,

    /// Ignore lines matching the regex (short -I form).
    #[clap(short = 'I', value_parser = crate::shared::AsBString, value_name = "regex", action = clap::ArgAction::Append)]
    pub ignore_matching_lines_short: Vec<BString>,

    /// Minimum lines between merged hunks.
    #[clap(long = "inter-hunk-context", value_name = "lines")]
    pub inter_hunk_context: Option<usize>,

    /// Include enclosing function in each hunk (-W / --function-context).
    #[clap(short = 'W', long = "function-context")]
    pub function_context: bool,

    /// Exit 0 if no changes, 1 if changes.
    #[clap(long = "exit-code")]
    pub exit_code: bool,

    /// --exit-code without diff emission.
    #[clap(long, short = 'q')]
    pub quiet: bool,

    /// Use a configured external diff program.
    #[clap(long = "ext-diff")]
    pub ext_diff: bool,

    /// Do not use a configured external diff program.
    #[clap(long = "no-ext-diff")]
    pub no_ext_diff: bool,

    /// Apply textconv filters before diffing.
    #[clap(long, overrides_with = "no_textconv")]
    pub textconv: bool,

    /// Do not apply textconv filters.
    #[clap(long = "no-textconv", overrides_with = "textconv")]
    pub no_textconv: bool,

    /// Submodule-diff rendering control (distinct from --submodule).
    #[clap(long = "ignore-submodules", value_parser = crate::shared::AsBString, value_name = "when", num_args = 0..=1, default_missing_value = "all", require_equals = true)]
    pub ignore_submodules: Option<BString>,

    /// Prefix for source side of diff.
    #[clap(long = "src-prefix", value_parser = crate::shared::AsBString, value_name = "prefix")]
    pub src_prefix: Option<BString>,

    /// Prefix for destination side of diff.
    #[clap(long = "dst-prefix", value_parser = crate::shared::AsBString, value_name = "prefix")]
    pub dst_prefix: Option<BString>,

    /// Suppress a/ and b/ prefixes in diff output.
    #[clap(long = "no-prefix")]
    pub no_prefix: bool,

    /// Emit diffs with the default `a/` and `b/` prefixes.
    #[clap(long = "default-prefix")]
    pub default_prefix: bool,

    /// Prepend <prefix> to every diff output line.
    #[clap(long = "line-prefix", value_parser = crate::shared::AsBString, value_name = "prefix")]
    pub line_prefix: Option<BString>,

    /// Treat intent-to-add paths as absent in the index.
    #[clap(long = "ita-invisible-in-index")]
    pub ita_invisible_in_index: bool,

    // ── merge-diff family (git-show defaults dense-combined) ────────
    /// Merge-diff mode selector. `git show`'s default is
    /// `dense-combined` (see `git-show.adoc:54`).
    #[clap(long = "diff-merges", value_parser = crate::shared::AsBString, value_name = "mode")]
    pub diff_merges: Option<BString>,

    /// Suppress merge-commit diffs.
    #[clap(long = "no-diff-merges")]
    pub no_diff_merges: bool,

    /// Short form of `--diff-merges=combined`.
    #[clap(short = 'c')]
    pub diff_combined: bool,

    /// Short form of `--diff-merges=dense-combined`.
    #[clap(long = "cc")]
    pub diff_cc: bool,

    /// Short form of `--diff-merges=separate`.
    #[clap(short = 'm')]
    pub diff_all_merge_parents: bool,

    /// In combined diffs, emit paths from each parent.
    #[clap(long = "combined-all-paths")]
    pub combined_all_paths: bool,

    /// For merges, re-merge and show the diff against the recorded merge.
    #[clap(long = "remerge-diff")]
    pub remerge_diff: bool,

    /// Show tree objects in diff output (-t).
    #[clap(short = 't')]
    pub show_tree_objects: bool,

    /// Alias for --diff-merges=dd.
    #[clap(long = "dd")]
    pub dd: bool,

    // ── positionals ──────────────────────────────────────────────────
    /// The objects to show. Defaults to `HEAD` when none are given.
    /// Mirrors `git-show.adoc:37` `<object>...` and the
    /// `setup_revision_opt::def = "HEAD"` fallback at
    /// `vendor/git/builtin/log.c:688`.
    #[clap(value_parser = crate::shared::AsBString)]
    pub objects: Vec<BString>,
}
