//! `gix rebase` plumbing CLI.
//!
//! Flag surface mirrors `vendor/git/builtin/rebase.c::cmd_rebase`
//! (`builtin_rebase_options[]` at `vendor/git/builtin/rebase.c:1120..1247`)
//! and `vendor/git/Documentation/git-rebase.adoc`. Parity coverage
//! lives in `tests/journey/parity/rebase.sh`.
//!
//! `git rebase` is a transplant operation — it computes a set of
//! commits between `<upstream>` and `<branch>` (defaulting to HEAD),
//! checks out `<onto>` (defaulting to `<upstream>`), and replays the
//! commits one by one. Every flag is currently parse-only — the
//! porcelain stub at `gitoxide_core::repository::rebase::porcelain`
//! emits a stub note and exits 0 (or, for the bare-no-upstream case,
//! emits git's verbatim "There is no tracking information for the
//! current branch." stanza on stdout and exits 1). Per-row entries in
//! `tests/journey/parity/rebase.sh` close each flag with
//! `compat_effect "deferred until rebase driver lands"` until the
//! real driver lands.

use gix::bstr::BString;

#[derive(Debug, clap::Parser)]
#[command(about = "Reapply commits on top of another base tip")]
pub struct Platform {
    // ── starting-point control ─────────────────────────────────────────
    /// Starting point at which to create the new commits. Defaults to
    /// `<upstream>`. Mirrors
    /// `OPT_STRING(0, "onto", ...)` at `builtin/rebase.c:1121`.
    #[clap(long, value_name = "newbase")]
    pub onto: Option<String>,

    /// Use the merge-base of `<upstream>` and `<branch>` as the
    /// starting point. Mirrors
    /// `OPT_BOOL(0, "keep-base", ...)` at `builtin/rebase.c:1124`.
    #[clap(long = "keep-base")]
    pub keep_base: bool,

    /// Rebase all commits reachable from `<branch>`, instead of
    /// limiting them with an `<upstream>`. Mirrors
    /// `OPT_BOOL(0, "root", ...)` at `builtin/rebase.c:1240`.
    #[clap(long)]
    pub root: bool,

    // ── mode (cmdmode group) ──────────────────────────────────────────
    /// Restart the rebasing process after having resolved a merge
    /// conflict. Mirrors
    /// `OPT_CMDMODE(0, "continue", ...)` at `builtin/rebase.c:1167`.
    #[clap(long = "continue")]
    pub continue_: bool,

    /// Restart the rebasing process by skipping the current patch.
    /// Mirrors `OPT_CMDMODE(0, "skip", ...)` at `builtin/rebase.c:1169`.
    #[clap(long)]
    pub skip: bool,

    /// Abort the rebase operation and reset HEAD to the original
    /// branch. Mirrors `OPT_CMDMODE(0, "abort", ...)` at
    /// `builtin/rebase.c:1171`.
    #[clap(long)]
    pub abort: bool,

    /// Abort the rebase operation but do not reset HEAD. Mirrors
    /// `OPT_CMDMODE(0, "quit", ...)` at `builtin/rebase.c:1174`.
    #[clap(long)]
    pub quit: bool,

    /// Edit the todo list during an interactive rebase. Mirrors
    /// `OPT_CMDMODE(0, "edit-todo", ...)` at `builtin/rebase.c:1176`.
    #[clap(long = "edit-todo")]
    pub edit_todo: bool,

    /// Show the current patch in an interactive rebase or when rebase
    /// is stopped because of conflicts. Mirrors
    /// `OPT_CMDMODE(0, "show-current-patch", ...)` at
    /// `builtin/rebase.c:1178`.
    #[clap(long = "show-current-patch")]
    pub show_current_patch: bool,

    // ── backend selection ─────────────────────────────────────────────
    /// Use the apply backend (calling `git-am` internally). Mirrors
    /// `OPT_CALLBACK_F(0, "apply", ...)` at `builtin/rebase.c:1181`.
    #[clap(long)]
    pub apply: bool,

    /// Use the merge backend (default). Mirrors
    /// `OPT_CALLBACK_F('m', "merge", ...)` at `builtin/rebase.c:1185`.
    #[clap(short = 'm', long)]
    pub merge: bool,

    /// Make a list of the commits which are about to be rebased and
    /// let the user edit it before rebasing. Mirrors
    /// `OPT_CALLBACK_F('i', "interactive", ...)` at
    /// `builtin/rebase.c:1189`.
    #[clap(short = 'i', long)]
    pub interactive: bool,

    // ── empty / cherry-pick handling ──────────────────────────────────
    /// How to handle commits that become empty after rebasing.
    /// Accepts `drop`, `keep`, or `stop`. Mirrors
    /// `OPT_CALLBACK_F(0, "empty", ...)` at `builtin/rebase.c:1198`.
    #[clap(long, value_name = "(drop|keep|stop)")]
    pub empty: Option<String>,

    /// Keep commits that start empty before the rebase. Mirrors
    /// `OPT_CALLBACK_F('k', "keep-empty", ...)` at
    /// `builtin/rebase.c:1201` (hidden in git, surfaced here for
    /// parity coverage).
    #[clap(short = 'k', long = "keep-empty")]
    pub keep_empty: bool,

    /// Do not keep commits that start empty before the rebase.
    /// Documented at `Documentation/git-rebase.adoc:287` as the
    /// negative form of `--keep-empty`.
    #[clap(long = "no-keep-empty")]
    pub no_keep_empty: bool,

    /// Reapply all clean cherry-picks of any upstream commit instead
    /// of preemptively dropping them. Mirrors
    /// `OPT_BOOL(0, "reapply-cherry-picks", ...)` at
    /// `builtin/rebase.c:1245`.
    #[clap(long = "reapply-cherry-picks", overrides_with = "no_reapply_cherry_picks")]
    pub reapply_cherry_picks: bool,

    /// Negative form of `--reapply-cherry-picks`.
    #[clap(long = "no-reapply-cherry-picks", overrides_with = "reapply_cherry_picks")]
    pub no_reapply_cherry_picks: bool,

    /// Allow rebasing commits with empty messages. Mirrors
    /// `OPT_BOOL_F(0, "allow-empty-message", ...)` at
    /// `builtin/rebase.c:1225` (hidden in git, surfaced here for
    /// parity coverage).
    #[clap(long = "allow-empty-message")]
    pub allow_empty_message: bool,

    // ── strategy / merge-driver tuning ────────────────────────────────
    /// Use the given merge strategy (implies `--merge`). Mirrors
    /// `OPT_STRING('s', "strategy", ...)` at `builtin/rebase.c:1234`.
    #[clap(short = 's', long, value_name = "strategy")]
    pub strategy: Option<String>,

    /// Pass an option through to the merge strategy (implies
    /// `--merge`). Mirrors
    /// `OPT_STRING_LIST('X', "strategy-option", ...)` at
    /// `builtin/rebase.c:1236`.
    #[clap(short = 'X', long = "strategy-option", value_name = "option=value", action = clap::ArgAction::Append)]
    pub strategy_option: Vec<String>,

    /// Try to preserve branching structure when rebasing merge
    /// commits. Accepts an optional `(rebase-cousins|no-rebase-cousins)`
    /// mode. Mirrors
    /// `OPT_CALLBACK_F('r', "rebase-merges", ...)` at
    /// `builtin/rebase.c:1229`. `require_equals = true` mirrors git's
    /// `PARSE_OPT_OPTARG`.
    #[clap(
        short = 'r',
        long = "rebase-merges",
        num_args = 0..=1,
        default_missing_value = "no-rebase-cousins",
        require_equals = true,
        value_name = "mode",
    )]
    pub rebase_merges: Option<String>,

    /// Negative form of `--rebase-merges`.
    #[clap(long = "no-rebase-merges")]
    pub no_rebase_merges: bool,

    // ── force / fork-point ────────────────────────────────────────────
    /// Cherry-pick all commits, even if unchanged (force a real
    /// rebase rather than fast-forwarding). Mirrors
    /// `OPT_BIT('f', "force-rebase", ...)` at `builtin/rebase.c:1161`.
    #[clap(short = 'f', long = "force-rebase", visible_alias = "no-ff")]
    pub force_rebase: bool,

    /// Use `merge-base --fork-point` to refine the upstream. Mirrors
    /// `OPT_BOOL(0, "fork-point", ...)` at `builtin/rebase.c:1232`.
    #[clap(long = "fork-point", overrides_with = "no_fork_point")]
    pub fork_point: bool,

    /// Disable the fork-point heuristic.
    #[clap(long = "no-fork-point", overrides_with = "fork_point")]
    pub no_fork_point: bool,

    // ── interactive companions ────────────────────────────────────────
    /// Append `exec <cmd>` after each line creating a commit. Mirrors
    /// `OPT_STRING_LIST('x', "exec", ...)` at `builtin/rebase.c:1222`.
    #[clap(short = 'x', long, value_name = "cmd", action = clap::ArgAction::Append)]
    pub exec: Vec<String>,

    /// Move commits beginning with `squash!`/`fixup!` under `-i`.
    /// Mirrors `OPT_BOOL(0, "autosquash", ...)` at
    /// `builtin/rebase.c:1205`.
    #[clap(long, overrides_with = "no_autosquash")]
    pub autosquash: bool,

    /// Negative form of `--autosquash`.
    #[clap(long = "no-autosquash", overrides_with = "autosquash")]
    pub no_autosquash: bool,

    /// Automatically reschedule `exec` commands that failed. Mirrors
    /// `OPT_BOOL(0, "reschedule-failed-exec", ...)` at
    /// `builtin/rebase.c:1242`.
    #[clap(long = "reschedule-failed-exec", overrides_with = "no_reschedule_failed_exec")]
    pub reschedule_failed_exec: bool,

    /// Negative form of `--reschedule-failed-exec`.
    #[clap(long = "no-reschedule-failed-exec", overrides_with = "reschedule_failed_exec")]
    pub no_reschedule_failed_exec: bool,

    /// Automatically force-update branches that point to commits that
    /// are being rebased. Mirrors
    /// `OPT_BOOL(0, "update-refs", ...)` at `builtin/rebase.c:1208`.
    #[clap(long = "update-refs", overrides_with = "no_update_refs")]
    pub update_refs: bool,

    /// Negative form of `--update-refs`.
    #[clap(long = "no-update-refs", overrides_with = "update_refs")]
    pub no_update_refs: bool,

    // ── verbosity / diffstat ──────────────────────────────────────────
    /// Be quiet. Implies `--no-stat`. Mirrors
    /// `OPT_NEGBIT('q', "quiet", ...)` at `builtin/rebase.c:1128`.
    #[clap(short = 'q', long, action = clap::ArgAction::Count)]
    pub quiet: u8,

    /// Be verbose. Implies `--stat`. Mirrors
    /// `OPT_BIT('v', "verbose", ...)` at `builtin/rebase.c:1131`.
    #[clap(short = 'v', long, action = clap::ArgAction::Count)]
    pub verbose: u8,

    /// Show a diffstat of what changed upstream since the last
    /// rebase. Documented at `Documentation/git-rebase.adoc:390`.
    #[clap(long = "stat")]
    pub stat: bool,

    /// Do not show a diffstat of what changed upstream. Mirrors the
    /// `OPTION_NEGBIT` `-n/--no-stat` entry at `builtin/rebase.c:1134`.
    #[clap(short = 'n', long = "no-stat")]
    pub no_stat: bool,

    // ── hooks ─────────────────────────────────────────────────────────
    /// Bypass the pre-rebase hook. Mirrors
    /// `OPT_BOOL(0, "no-verify", ...)` at `builtin/rebase.c:1126`
    /// (note the inverted spelling: the option is named `--no-verify`
    /// but its bool target stores "ok-to-skip-pre-rebase").
    #[clap(long = "no-verify", overrides_with = "verify")]
    pub no_verify: bool,

    /// Allow the pre-rebase hook to run (default). Documented at
    /// `Documentation/git-rebase.adoc:401` as the countermand for
    /// `--no-verify`.
    #[clap(long = "verify", overrides_with = "no_verify")]
    pub verify: bool,

    // ── apply-backend passthroughs ────────────────────────────────────
    /// Ensure at least `<n>` lines of surrounding context match.
    /// Implies `--apply`. Mirrors
    /// `OPT_PASSTHRU_ARGV('C', NULL, ...)` at `builtin/rebase.c:1155`.
    /// In git this is a sticky-only short (`-C5`); here clap requires
    /// the value as a separate token, matching git's effective
    /// surface for parity coverage.
    #[clap(short = 'C', value_name = "n")]
    pub context_lines: Option<String>,

    /// Ignore whitespace differences. Mirrors
    /// `OPT_BOOL(0, "ignore-whitespace", ...)` at
    /// `builtin/rebase.c:1157`.
    #[clap(long = "ignore-whitespace")]
    pub ignore_whitespace: bool,

    /// Passed through to `git apply`. Mirrors
    /// `OPT_PASSTHRU_ARGV(0, "whitespace", ...)` at
    /// `builtin/rebase.c:1159`.
    #[clap(long, value_name = "action")]
    pub whitespace: Option<String>,

    // ── trailers / signoff / authorship ───────────────────────────────
    /// Append the given trailer to every rebased commit message
    /// (repeatable). Mirrors `OPT_STRVEC(0, "trailer", ...)` at
    /// `builtin/rebase.c:1144`.
    #[clap(long, value_name = "trailer", action = clap::ArgAction::Append)]
    pub trailer: Vec<String>,

    /// Add a `Signed-off-by` trailer to all rebased commits. Mirrors
    /// `OPT_BOOL(0, "signoff", ...)` at `builtin/rebase.c:1146`.
    #[clap(long)]
    pub signoff: bool,

    /// Make the committer date match the author date. Mirrors
    /// `OPT_BOOL(0, "committer-date-is-author-date", ...)` at
    /// `builtin/rebase.c:1148`.
    #[clap(long = "committer-date-is-author-date")]
    pub committer_date_is_author_date: bool,

    /// Use the current time as the author date. Mirrors
    /// `OPT_BOOL(0, "reset-author-date", ...)` at
    /// `builtin/rebase.c:1151`.
    #[clap(long = "reset-author-date", visible_alias = "ignore-date")]
    pub reset_author_date: bool,

    // ── rerere ────────────────────────────────────────────────────────
    /// Allow the rerere mechanism to update the index automatically.
    /// Mirrors `OPT_RERERE_AUTOUPDATE` at `builtin/rebase.c:1197`
    /// (expands to both `--rerere-autoupdate` and
    /// `--no-rerere-autoupdate`).
    #[clap(long = "rerere-autoupdate", overrides_with = "no_rerere_autoupdate")]
    pub rerere_autoupdate: bool,

    /// Negative form of `--rerere-autoupdate`.
    #[clap(long = "no-rerere-autoupdate", overrides_with = "rerere_autoupdate")]
    pub no_rerere_autoupdate: bool,

    // ── autostash ─────────────────────────────────────────────────────
    /// Stash and re-apply local changes around the operation. Mirrors
    /// `OPT_AUTOSTASH` at `builtin/rebase.c:1221` (expands to both
    /// `--autostash` and `--no-autostash`).
    #[clap(long, overrides_with = "no_autostash")]
    pub autostash: bool,

    /// Negative form of `--autostash`.
    #[clap(long = "no-autostash", overrides_with = "autostash")]
    pub no_autostash: bool,

    // ── GPG signing ───────────────────────────────────────────────────
    /// GPG-sign commits. Mirrors the `OPTION_STRING` entry with
    /// `PARSE_OPT_OPTARG` at `builtin/rebase.c:1211..1220`. The
    /// `<key-id>` is optional and `require_equals = true` mirrors
    /// git's "must be stuck" semantic.
    #[clap(
        short = 'S',
        long = "gpg-sign",
        num_args = 0..=1,
        default_missing_value = "",
        require_equals = true,
        value_name = "key-id",
        overrides_with = "no_gpg_sign",
    )]
    pub gpg_sign: Option<String>,

    /// Negative form of `--gpg-sign`. Documented at
    /// `Documentation/git-rebase.adoc:375`.
    #[clap(long = "no-gpg-sign", overrides_with = "gpg_sign")]
    pub no_gpg_sign: bool,

    // ── positionals ───────────────────────────────────────────────────
    /// Upstream branch to compare against. May be any valid commit.
    /// Defaults to the configured upstream for the current branch.
    #[clap(value_parser = crate::shared::AsBString)]
    pub upstream: Option<BString>,

    /// Working branch; defaults to `HEAD`.
    #[clap(value_parser = crate::shared::AsBString)]
    pub branch: Option<BString>,
}
