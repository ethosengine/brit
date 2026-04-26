//! `gix restore` plumbing CLI.
//!
//! Flag surface mirrors `vendor/git/builtin/checkout.c::cmd_restore`
//! (entry point at `vendor/git/builtin/checkout.c:2128..2162`) plus the
//! two shared option helpers `add_common_options`
//! (`vendor/git/builtin/checkout.c:1712..1730`) and
//! `add_checkout_path_options`
//! (`vendor/git/builtin/checkout.c:1756..1778`), and the synopsis at
//! `vendor/git/Documentation/git-restore.adoc:8..14`.
//!
//! Synopsis:
//!
//! * `git restore [<options>] [--source=<tree>] [--staged] [--worktree] [--] <pathspec>...`
//! * `git restore [<options>] [--source=<tree>] [--staged] [--worktree] --pathspec-from-file=<file> [--pathspec-file-nul]`
//! * `git restore (-p|--patch) [<options>] [--source=<tree>] [--staged] [--worktree] [--] [<pathspec>...]`
//!
//! Today the porcelain stub at
//! `gitoxide_core::repository::restore::porcelain` is a placeholder
//! that wires the precondition matrix mirroring
//! `vendor/git/builtin/checkout.c::cmd_restore` argument-parse + the
//! `checkout_main` `accept_pathspec=1, empty_pathspec_ok=0` empty-
//! pathspec gate, then emits a stub note + exits 0 on the happy path.
//! Per-row entries in `tests/journey/parity/restore.sh` close each
//! flag-bearing happy-path row with
//! `compat_effect "deferred until restore driver lands"` until the real
//! driver lands.

use gix::bstr::BString;

#[derive(Debug, clap::Parser)]
#[command(about = "Restore working tree files")]
pub struct Platform {
    // ── restore_options (vendor/git/builtin/checkout.c:2135..2145) ──
    /// Restore the working tree files with the content from the given
    /// tree. Mirrors `vendor/git/builtin/checkout.c:2136..2137`
    /// `OPT_STRING('s', "source", &opts.from_treeish, ...)`.
    #[clap(short = 's', long = "source", value_name = "tree-ish")]
    pub source: Option<String>,

    /// Restore the index (only). Mirrors
    /// `vendor/git/builtin/checkout.c:2138..2139`
    /// `OPT_BOOL('S', "staged", &opts.checkout_index, ...)`.
    #[clap(short = 'S', long)]
    pub staged: bool,

    /// Restore the working tree (default). Mirrors
    /// `vendor/git/builtin/checkout.c:2140..2141`
    /// `OPT_BOOL('W', "worktree", &opts.checkout_worktree, ...)`.
    #[clap(short = 'W', long)]
    pub worktree: bool,

    /// Ignore unmerged entries. Mirrors
    /// `vendor/git/builtin/checkout.c:2142..2143`
    /// `OPT_BOOL(0, "ignore-unmerged", &opts.ignore_unmerged, ...)`.
    #[clap(long = "ignore-unmerged")]
    pub ignore_unmerged: bool,

    /// Use overlay mode (default no-overlay). Mirrors
    /// `vendor/git/builtin/checkout.c:2144` `OPT_BOOL(0, "overlay",
    /// &opts.overlay_mode, ...)`.
    #[clap(long, overrides_with = "no_overlay")]
    pub overlay: bool,

    /// Disable overlay mode (the default; remove tracked files that do
    /// not appear in `--source=<tree>`). Documented at
    /// `vendor/git/Documentation/git-restore.adoc:122..127`.
    #[clap(long = "no-overlay", overrides_with = "overlay")]
    pub no_overlay: bool,

    // ── add_common_options (vendor/git/builtin/checkout.c:1712..1730) ──
    /// Suppress feedback messages. Mirrors
    /// `vendor/git/builtin/checkout.c:1716` `OPT__QUIET`.
    #[clap(short = 'q', long)]
    pub quiet: bool,

    /// Control recursive updating of submodules. Mirrors
    /// `vendor/git/builtin/checkout.c:1717..1719`
    /// `OPT_CALLBACK_F(0, "recurse-submodules", ..., PARSE_OPT_OPTARG, ...)`.
    /// Optional value, e.g. `--recurse-submodules=on-demand`.
    #[clap(
        long = "recurse-submodules",
        value_name = "checkout",
        num_args = 0..=1,
        default_missing_value = "",
        require_equals = true,
    )]
    pub recurse_submodules: Option<String>,

    /// Disable submodule recursion.
    #[clap(long = "no-recurse-submodules")]
    pub no_recurse_submodules: bool,

    /// Force progress reporting. Mirrors
    /// `vendor/git/builtin/checkout.c:1720`
    /// `OPT_BOOL(0, "progress", &opts->show_progress, ...)`.
    #[clap(long, overrides_with = "no_progress")]
    pub progress: bool,

    /// Disable progress reporting (overrides `--progress`).
    #[clap(long = "no-progress", overrides_with = "progress")]
    pub no_progress: bool,

    /// Recreate the conflicted merge in unmerged paths. Mirrors
    /// `vendor/git/builtin/checkout.c:1721`
    /// `OPT_BOOL('m', "merge", &opts->merge, ...)`.
    #[clap(short = 'm', long)]
    pub merge: bool,

    /// Conflict style (merge, diff3, or zdiff3). Mirrors
    /// `vendor/git/builtin/checkout.c:1722..1724`
    /// `OPT_CALLBACK(0, "conflict", ...)`.
    #[clap(long, value_name = "style")]
    pub conflict: Option<String>,

    // ── add_checkout_path_options (vendor/git/builtin/checkout.c:1756..1778) ──
    /// Use stage #2 (ours) for unmerged paths. Mirrors
    /// `vendor/git/builtin/checkout.c:1760..1762`
    /// `OPT_SET_INT_F('2', "ours", &opts->writeout_stage, ..., 2, PARSE_OPT_NONEG)`.
    /// Wired as long-only on the gix side because `-2` is not a valid
    /// Clap short flag — git accepts both `--ours` and the (long-form)
    /// short `-2`, but the long form is the documented surface.
    #[clap(long, conflicts_with = "theirs")]
    pub ours: bool,

    /// Use stage #3 (theirs) for unmerged paths. Mirrors
    /// `vendor/git/builtin/checkout.c:1763..1765`
    /// `OPT_SET_INT_F('3', "theirs", &opts->writeout_stage, ..., 3, PARSE_OPT_NONEG)`.
    /// Wired as long-only on the gix side because `-3` is not a valid
    /// Clap short flag.
    #[clap(long, conflicts_with = "ours")]
    pub theirs: bool,

    /// Select hunks interactively. Mirrors
    /// `vendor/git/builtin/checkout.c:1766`
    /// `OPT_BOOL('p', "patch", &opts->patch_mode, ...)`.
    #[clap(short = 'p', long)]
    pub patch: bool,

    /// Generate diffs with `<n>` lines of context. Mirrors
    /// `vendor/git/builtin/checkout.c:1767`
    /// `OPT_DIFF_UNIFIED(&opts->patch_context)` —
    /// `OPT_INTEGER_F('U', "unified", ..., PARSE_OPT_NONEG)`.
    #[clap(short = 'U', long = "unified", value_name = "n")]
    pub unified: Option<u32>,

    /// Show context between diff hunks up to the specified number of
    /// lines. Mirrors `vendor/git/builtin/checkout.c:1768`
    /// `OPT_DIFF_INTERHUNK_CONTEXT(&opts->patch_interhunk_context)` —
    /// `OPT_INTEGER_F(0, "inter-hunk-context", ..., PARSE_OPT_NONEG)`.
    #[clap(long = "inter-hunk-context", value_name = "n")]
    pub inter_hunk_context: Option<u32>,

    /// Do not limit pathspecs to sparse entries only. Mirrors
    /// `vendor/git/builtin/checkout.c:1769..1770`
    /// `OPT_BOOL(0, "ignore-skip-worktree-bits", ...)`.
    #[clap(long = "ignore-skip-worktree-bits")]
    pub ignore_skip_worktree_bits: bool,

    /// Pathspec is passed in `<file>` instead of commandline args.
    /// Mirrors `vendor/git/builtin/checkout.c:1771`
    /// `OPT_PATHSPEC_FROM_FILE(&opts->pathspec_from_file)`.
    #[clap(
        long = "pathspec-from-file",
        value_parser = crate::shared::AsBString,
        value_name = "file"
    )]
    pub pathspec_from_file: Option<BString>,

    /// Pathspec elements in `--pathspec-from-file` are NUL-separated.
    /// Mirrors `vendor/git/builtin/checkout.c:1772`
    /// `OPT_PATHSPEC_FILE_NUL(&opts->pathspec_file_nul)`.
    #[clap(long = "pathspec-file-nul")]
    pub pathspec_file_nul: bool,

    // ── positionals ─────────────────────────────────────────────────
    /// `<pathspec>...` — files to restore. Mirrors the trailing
    /// `[<pathspec>...]` of `vendor/git/Documentation/git-restore.adoc:11..13`.
    #[clap(value_parser = crate::shared::AsBString)]
    pub args: Vec<BString>,

    /// Pathspec after the `--` separator.
    #[clap(last = true, value_parser = crate::shared::AsBString)]
    pub paths: Vec<BString>,
}
