//! `gix reset` plumbing CLI.
//!
//! Flag surface mirrors `vendor/git/builtin/reset.c` (entry point
//! `cmd_reset` at `vendor/git/builtin/reset.c:336`) and
//! `vendor/git/Documentation/git-reset.adoc`. Parity coverage lives
//! in `tests/journey/parity/reset.sh`.
//!
//! The four canonical synopsis forms (per `git_reset_usage` at
//! `vendor/git/builtin/reset.c:44`):
//!
//! * `git reset [--mixed | --soft | --hard | --merge | --keep] [-q] [<commit>]`
//! * `git reset [-q] [<tree-ish>] [--] <pathspec>...`
//! * `git reset [-q] [--pathspec-from-file=<file> [--pathspec-file-nul]] [<tree-ish>]`
//! * `git reset --patch [<tree-ish>] [--] [<pathspec>...]`
//!
//! Today the porcelain stub at
//! `gitoxide_core::repository::reset::porcelain` is a placeholder that
//! emits a stub note + exits 0 (except for the outside-of-repo gate
//! handled by the shared `repository(Mode::Lenient)` glue, which
//! matches git's "fatal: not a git repository" + exit 128). Per-row
//! entries in `tests/journey/parity/reset.sh` close each flag with
//! `compat_effect "deferred until reset driver lands"` until the real
//! driver lands.

use gix::bstr::BString;

#[derive(Debug, clap::Parser)]
#[command(about = "Set HEAD or the index to a known state")]
pub struct Platform {
    // ── verbosity / refresh ─────────────────────────────────────────
    /// Be quiet, only report errors. Mirrors
    /// `vendor/git/builtin/reset.c:351` `OPT__QUIET`.
    #[clap(short = 'q', long)]
    pub quiet: bool,

    /// Refresh the index after a mixed reset. Default. Mirrors the
    /// inverted-bool surface of `OPT_BOOL(0, "no-refresh", ...)` at
    /// `vendor/git/builtin/reset.c:352`.
    #[clap(long, overrides_with = "no_refresh")]
    pub refresh: bool,

    /// Skip refreshing the index after a mixed reset. Mirrors
    /// `vendor/git/builtin/reset.c:352` `OPT_BOOL(0, "no-refresh", ...)`.
    #[clap(long = "no-refresh", overrides_with = "refresh")]
    pub no_refresh: bool,

    // ── reset modes (PARSE_OPT_NONEG; mutually exclusive at C site) ─
    /// Reset HEAD and index (default). Mirrors
    /// `vendor/git/builtin/reset.c:354` `OPT_SET_INT_F(MIXED, ...)`.
    #[clap(long)]
    pub mixed: bool,

    /// Reset only HEAD. Mirrors `vendor/git/builtin/reset.c:357`
    /// `OPT_SET_INT_F(SOFT, ...)`.
    #[clap(long)]
    pub soft: bool,

    /// Reset HEAD, index, and working tree. Mirrors
    /// `vendor/git/builtin/reset.c:360` `OPT_SET_INT_F(HARD, ...)`.
    #[clap(long)]
    pub hard: bool,

    /// Reset HEAD, index, and working tree (3-way variant). Mirrors
    /// `vendor/git/builtin/reset.c:363` `OPT_SET_INT_F(MERGE, ...)`.
    #[clap(long)]
    pub merge: bool,

    /// Reset HEAD but keep local changes. Mirrors
    /// `vendor/git/builtin/reset.c:366` `OPT_SET_INT_F(KEEP, ...)`.
    #[clap(long)]
    pub keep: bool,

    // ── submodules ──────────────────────────────────────────────────
    /// Recursively reset submodules' working trees. Optional `<mode>`
    /// is one of `yes`/`no`/`true`/`false`/`1`/`0` — the value set
    /// accepted by `parse_update_recurse_submodules_arg` for the
    /// worktree updater path used by `git reset`. The bare flag
    /// (no `<mode>`) corresponds to `RECURSE_SUBMODULES_ON` per
    /// `vendor/git/submodule.c::option_parse_recurse_submodules_worktree_updater`.
    /// Mirrors `vendor/git/builtin/reset.c:369`
    /// `OPT_CALLBACK_F("recurse-submodules", ..., PARSE_OPT_OPTARG, ...)`.
    #[clap(
        long = "recurse-submodules",
        value_parser = crate::shared::AsBString,
        value_name = "mode",
        num_args = 0..=1,
        default_missing_value = "yes",
        require_equals = true,
        overrides_with = "no_recurse_submodules"
    )]
    pub recurse_submodules: Option<BString>,

    /// Disable recursive submodule reset. Mirrors the inverted form
    /// of `vendor/git/builtin/reset.c:369`.
    #[clap(long = "no-recurse-submodules", overrides_with = "recurse_submodules")]
    pub no_recurse_submodules: bool,

    // ── interactive / patch ─────────────────────────────────────────
    /// Select hunks interactively. Mirrors
    /// `vendor/git/builtin/reset.c:373` `OPT_BOOL('p', "patch", ...)`.
    #[clap(short = 'p', long)]
    pub patch: bool,

    /// Auto-advance to the next file when selecting hunks
    /// interactively. Mirrors `vendor/git/builtin/reset.c:374`
    /// `OPT_BOOL(0, "auto-advance", ...)`.
    #[clap(long = "auto-advance", overrides_with = "no_auto_advance")]
    pub auto_advance: bool,

    /// Disable `--auto-advance`. Mirrors the inverted form.
    #[clap(long = "no-auto-advance", overrides_with = "auto_advance")]
    pub no_auto_advance: bool,

    /// Number of context lines for `--patch`'s unified diff. Mirrors
    /// `vendor/git/builtin/reset.c:376` `OPT_DIFF_UNIFIED`.
    #[clap(short = 'U', long, value_name = "n")]
    pub unified: Option<i32>,

    /// Minimum lines between merged hunks for `--patch`. Mirrors
    /// `vendor/git/builtin/reset.c:377` `OPT_DIFF_INTERHUNK_CONTEXT`.
    #[clap(long = "inter-hunk-context", value_name = "n")]
    pub inter_hunk_context: Option<i32>,

    // ── intent-to-add ───────────────────────────────────────────────
    /// Record only the fact that removed paths will be added later.
    /// Mirrors `vendor/git/builtin/reset.c:378` `OPT_BOOL('N',
    /// "intent-to-add", ...)`.
    #[clap(short = 'N', long = "intent-to-add")]
    pub intent_to_add: bool,

    // ── pathspec sources ────────────────────────────────────────────
    /// Pathspec is passed in `<file>` instead of commandline args.
    /// Mirrors `vendor/git/builtin/reset.c:380`
    /// `OPT_PATHSPEC_FROM_FILE`.
    #[clap(
        long = "pathspec-from-file",
        value_parser = crate::shared::AsBString,
        value_name = "file"
    )]
    pub pathspec_from_file: Option<BString>,

    /// Pathspec elements in `--pathspec-from-file` are NUL-separated.
    /// Mirrors `vendor/git/builtin/reset.c:381`
    /// `OPT_PATHSPEC_FILE_NUL`.
    #[clap(long = "pathspec-file-nul")]
    pub pathspec_file_nul: bool,

    // ── positionals ─────────────────────────────────────────────────
    /// `<commit>` / `<tree-ish>` followed by optional `<pathspec>...`.
    /// Disambiguated against the working tree by
    /// `vendor/git/builtin/reset.c:247` `parse_args` (single arg
    /// committish vs. multi-arg treeish + paths).
    #[clap(value_parser = crate::shared::AsBString)]
    pub args: Vec<BString>,

    /// Pathspec after the `--` separator. Mirrors the
    /// `PARSE_OPT_KEEP_DASHDASH` semantics at
    /// `vendor/git/builtin/reset.c:387`.
    #[clap(last = true, value_parser = crate::shared::AsBString)]
    pub paths: Vec<BString>,
}
