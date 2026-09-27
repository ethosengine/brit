//! `gix add` plumbing CLI.
//!
//! Flag surface mirrors `vendor/git/builtin/add.c` (entry point
//! `cmd_add` at `vendor/git/builtin/add.c:381`) and
//! `vendor/git/Documentation/git-add.adoc`. Parity coverage lives in
//! `tests/journey/parity/add.sh`.
//!
//! Synopsis (`vendor/git/builtin/add.c:29`):
//!
//! * `git add [<options>] [--] <pathspec>...`
//!
//! Today the porcelain stub at `gitoxide_core::repository::add::porcelain`
//! is a placeholder that emits a stub note + exits 0 (except for the
//! outside-of-repo gate handled by the shared `repository(Mode::Lenient)`
//! glue, which matches git's "fatal: not a git repository" + exit 128,
//! and the precondition matrix mirroring
//! `vendor/git/builtin/add.c:405..474` for byte-exact error stanzas).
//! Per-row entries in `tests/journey/parity/add.sh` close each flag with
//! `compat_effect "deferred until add driver lands"` until the real
//! driver lands.

use gix::bstr::BString;

#[derive(Debug, clap::Parser)]
#[command(about = "Add file contents to the index")]
pub struct Platform {
    // ── verbosity / dry-run ─────────────────────────────────────────
    /// Don't actually add the file(s), just show if they exist and/or
    /// will be ignored. Mirrors `vendor/git/builtin/add.c:254`
    /// `OPT__DRY_RUN`.
    #[clap(short = 'n', long = "dry-run")]
    pub dry_run: bool,

    /// Be verbose. Mirrors `vendor/git/builtin/add.c:255`
    /// `OPT__VERBOSE`.
    #[clap(short = 'v', long)]
    pub verbose: bool,

    // ── interactive / patch / edit ──────────────────────────────────
    /// Add modified contents in the working tree interactively to the
    /// index. Mirrors `vendor/git/builtin/add.c:257` `OPT_BOOL('i',
    /// "interactive", ...)`.
    #[clap(short = 'i', long)]
    pub interactive: bool,

    /// Interactively choose hunks of patch between the index and the
    /// work tree. Mirrors `vendor/git/builtin/add.c:258` `OPT_BOOL('p',
    /// "patch", ...)`.
    #[clap(short = 'p', long)]
    pub patch: bool,

    /// Auto-advance to the next file when selecting hunks
    /// interactively. Mirrors `vendor/git/builtin/add.c:259`
    /// `OPT_BOOL(0, "auto-advance", ...)`. Requires `--interactive` /
    /// `--patch`.
    #[clap(long = "auto-advance", overrides_with = "no_auto_advance")]
    pub auto_advance: bool,

    /// Disable `--auto-advance`. Mirrors the inverted form (negation
    /// of `vendor/git/builtin/add.c:259`).
    #[clap(long = "no-auto-advance", overrides_with = "auto_advance")]
    pub no_auto_advance: bool,

    /// Number of context lines for `--patch`'s unified diff. Mirrors
    /// `vendor/git/builtin/add.c:261` `OPT_DIFF_UNIFIED`.
    #[clap(short = 'U', long, value_name = "n")]
    pub unified: Option<i32>,

    /// Minimum lines between merged hunks for `--patch`. Mirrors
    /// `vendor/git/builtin/add.c:262` `OPT_DIFF_INTERHUNK_CONTEXT`.
    #[clap(long = "inter-hunk-context", value_name = "n")]
    pub inter_hunk_context: Option<i32>,

    /// Open the diff vs. the index in an editor and apply the result.
    /// Mirrors `vendor/git/builtin/add.c:263` `OPT_BOOL('e', "edit",
    /// ...)`.
    #[clap(short = 'e', long)]
    pub edit: bool,

    // ── force / sparse / update / all / no-all ──────────────────────
    /// Allow adding otherwise ignored files. Mirrors
    /// `vendor/git/builtin/add.c:264` `OPT__FORCE`.
    #[clap(short = 'f', long)]
    pub force: bool,

    /// Update the index just where it already has an entry matching
    /// `<pathspec>`. Mirrors `vendor/git/builtin/add.c:265` `OPT_BOOL(
    /// 'u', "update", ...)`.
    #[clap(short = 'u', long)]
    pub update: bool,

    /// Renormalize EOL of tracked files (implies `-u`). Mirrors
    /// `vendor/git/builtin/add.c:266` `OPT_BOOL(0, "renormalize",
    /// ...)`.
    #[clap(long)]
    pub renormalize: bool,

    /// Record only the fact that the path will be added later. Mirrors
    /// `vendor/git/builtin/add.c:267` `OPT_BOOL('N', "intent-to-add",
    /// ...)`.
    #[clap(short = 'N', long = "intent-to-add")]
    pub intent_to_add: bool,

    /// Update the index not only where the working tree has a file
    /// matching `<pathspec>` but also where the index already has an
    /// entry. Mirrors `vendor/git/builtin/add.c:268` `OPT_BOOL('A',
    /// "all", ...)`. `--no-ignore-removal` is the same flag spelled
    /// inversely (see `vendor/git/builtin/add.c:269`).
    #[clap(short = 'A', long, alias = "no-ignore-removal", overrides_with = "ignore_removal")]
    pub all: bool,

    /// Update the index by adding new files and modifications, but
    /// ignore files removed from the working tree (`--no-all` /
    /// `--ignore-removal`). Mirrors `vendor/git/builtin/add.c:269`
    /// `OPT_CALLBACK_F(0, "ignore-removal", ...)` plus the `--no-all`
    /// inverse spelled at `vendor/git/builtin/add.c:268`.
    #[clap(long = "ignore-removal", alias = "no-all", overrides_with = "all")]
    pub ignore_removal: bool,

    // ── refresh / ignore-errors / ignore-missing / sparse ───────────
    /// Don't add the file(s), but only refresh their stat() info in
    /// the index. Mirrors `vendor/git/builtin/add.c:273` `OPT_BOOL(0,
    /// "refresh", ...)`.
    #[clap(long)]
    pub refresh: bool,

    /// Skip files which cannot be added because of errors. Mirrors
    /// `vendor/git/builtin/add.c:274` `OPT_BOOL(0, "ignore-errors",
    /// ...)`.
    #[clap(long = "ignore-errors")]
    pub ignore_errors: bool,

    /// In `--dry-run`, check if even missing files are ignored.
    /// Mirrors `vendor/git/builtin/add.c:275` `OPT_BOOL(0,
    /// "ignore-missing", ...)`. Only meaningful with `--dry-run`.
    #[clap(long = "ignore-missing")]
    pub ignore_missing: bool,

    /// Allow updating index entries outside of the sparse-checkout
    /// cone. Mirrors `vendor/git/builtin/add.c:276` `OPT_BOOL(0,
    /// "sparse", ...)`.
    #[clap(long)]
    pub sparse: bool,

    // ── chmod ───────────────────────────────────────────────────────
    /// Override the executable bit of the added files. Value must be
    /// `(+|-)x`. Mirrors `vendor/git/builtin/add.c:277` `OPT_STRING(0,
    /// "chmod", ...)`.
    #[clap(long, value_name = "(+|-)x")]
    pub chmod: Option<String>,

    // ── pathspec sources ────────────────────────────────────────────
    /// Pathspec is passed in `<file>` instead of commandline args.
    /// Mirrors `vendor/git/builtin/add.c:281` `OPT_PATHSPEC_FROM_FILE`.
    #[clap(
        long = "pathspec-from-file",
        value_parser = crate::shared::AsBString,
        value_name = "file"
    )]
    pub pathspec_from_file: Option<BString>,

    /// Pathspec elements in `--pathspec-from-file` are NUL-separated.
    /// Mirrors `vendor/git/builtin/add.c:282` `OPT_PATHSPEC_FILE_NUL`.
    #[clap(long = "pathspec-file-nul")]
    pub pathspec_file_nul: bool,

    // ── positionals ─────────────────────────────────────────────────
    /// `<pathspec>...` — files to add content from. Mirrors the
    /// trailing `[<pathspec>...]` of the synopsis at
    /// `vendor/git/builtin/add.c:30`.
    #[clap(value_parser = crate::shared::AsBString)]
    pub args: Vec<BString>,

    /// Pathspec after the `--` separator.
    #[clap(last = true, value_parser = crate::shared::AsBString)]
    pub paths: Vec<BString>,
}
