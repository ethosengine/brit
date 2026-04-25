//! `gix pull` plumbing CLI.
//!
//! Flag surface mirrors `vendor/git/builtin/pull.c::cmd_pull` options[]
//! and `vendor/git/Documentation/git-pull.adoc` (which itself
//! includes `merge-options.adoc` + `fetch-options.adoc`).
//! Parity coverage lives in `tests/journey/parity/pull.sh`.
//!
//! `git pull` is a composite — it runs `git fetch` then integrates
//! (merge or rebase). The flag surface here is the union of fetch +
//! merge flags that pull itself accepts (i.e. it is NOT a mechanical
//! flatten of the two sub-commands; e.g. fetch's `--multiple` is not
//! exposed by pull). Every flag is currently parse-only — the
//! porcelain stub at `gitoxide_core::repository::pull::porcelain`
//! emits a stub note and exits 0 until a real pull driver lands.
//! Per-row entries in `tests/journey/parity/pull.sh` close each flag
//! with `compat_effect "deferred until pull driver lands"` until then.

use std::ffi::OsString;

#[derive(Debug, clap::Parser)]
#[command(about = "Fetch from and integrate with another repository or a local branch")]
pub struct Platform {
    // ── shared verbosity / progress ───────────────────────────────────
    /// Pass `--verbose` to git-fetch and git-merge.
    #[clap(short = 'v', long, action = clap::ArgAction::Count)]
    pub verbose: u8,

    /// Squelch reporting during transfer and during merging.
    #[clap(short = 'q', long, action = clap::ArgAction::Count)]
    pub quiet: u8,

    /// Force progress output even when stderr isn't a TTY.
    #[clap(long, overrides_with = "no_progress")]
    pub progress: bool,

    /// Disable progress output.
    #[clap(long, overrides_with = "progress")]
    pub no_progress: bool,

    /// Recurse into submodules. Accepts `yes`, `on-demand`, or `no`.
    /// Bare form defaults to `yes` per git's `PARSE_OPT_OPTARG`.
    #[clap(
        long,
        value_name = "MODE",
        num_args = 0..=1,
        default_missing_value = "yes",
        require_equals = true,
    )]
    pub recurse_submodules: Option<String>,

    /// Disable recursive submodule fetching (alias for
    /// `--recurse-submodules=no`).
    #[clap(long)]
    pub no_recurse_submodules: bool,

    // ── merge options ─────────────────────────────────────────────────
    /// Rebase the current branch on top of the upstream branch after
    /// fetching. Accepts `true`, `false`, `merges`, or `interactive`.
    /// `require_equals = true` so `-r dev` parses as `-r` (default) +
    /// positional `dev`, mirroring git's `PARSE_OPT_OPTARG` semantics.
    #[clap(
        short = 'r',
        long,
        value_name = "VALUE",
        num_args = 0..=1,
        default_missing_value = "true",
        require_equals = true,
    )]
    pub rebase: Option<String>,

    /// Shorthand for `--rebase=false` (use merge to integrate).
    #[clap(long)]
    pub no_rebase: bool,

    /// Suppress the diffstat at the end of the merge.
    #[clap(short = 'n', long = "no-stat", action = clap::ArgAction::SetTrue)]
    pub no_stat: bool,

    /// Show a diffstat at the end of the merge.
    #[clap(long)]
    pub stat: bool,

    /// Synonym for `--stat` (deprecated).
    #[clap(long)]
    pub summary: bool,

    /// Synonym for `--no-stat` (deprecated).
    #[clap(long)]
    pub no_summary: bool,

    /// Show a condensed diffstat with file moves/renames.
    #[clap(long = "compact-summary")]
    pub compact_summary: bool,

    /// Populate the merge message with one-line shortlog entries (up to
    /// `<n>`, default 20). `require_equals = true` so `--log dev` parses
    /// as `--log` + positional `dev`.
    #[clap(long, num_args = 0..=1, default_missing_value = "20", require_equals = true, value_name = "n")]
    pub log: Option<String>,

    /// Suppress the merge-message shortlog.
    #[clap(long)]
    pub no_log: bool,

    /// Sign-off the resulting merge commit.
    #[clap(long, num_args = 0..=1, default_missing_value = "true", require_equals = true, value_name = "VALUE")]
    pub signoff: Option<String>,

    /// Do not sign-off the resulting merge commit.
    #[clap(long)]
    pub no_signoff: bool,

    /// Squash the merge: produce the working-tree state of a real merge
    /// without making a commit or moving HEAD.
    #[clap(long)]
    pub squash: bool,

    /// Disable squash mode.
    #[clap(long)]
    pub no_squash: bool,

    /// Make a merge commit even when the merge resolves as a fast-forward.
    #[clap(long)]
    pub commit: bool,

    /// Do not commit after the merge resolves; leave MERGE_HEAD in place.
    #[clap(long)]
    pub no_commit: bool,

    /// Open the editor on the merge commit message.
    ///
    /// `git pull` does NOT take a `-e` short (see vendor/git/builtin/pull.c
    /// OPT_PASSTHRU(0, "edit", ...)) — only the long form. The journey
    /// row for `gix pull -e` therefore tests the unknown-short-flag
    /// error path on both binaries (exit 129).
    #[clap(long)]
    pub edit: bool,

    /// Skip editing the merge commit message.
    #[clap(long)]
    pub no_edit: bool,

    /// Override the cleanup mode applied to the merge commit message.
    #[clap(long, value_name = "mode")]
    pub cleanup: Option<String>,

    /// Allow a fast-forward merge.
    #[clap(long)]
    pub ff: bool,

    /// Forbid fast-forward merges; always create a merge commit.
    #[clap(long)]
    pub no_ff: bool,

    /// Refuse to merge unless the merge resolves as a fast-forward.
    #[clap(long = "ff-only")]
    pub ff_only: bool,

    /// Run the `commit-msg` / `pre-merge-commit` hooks.
    #[clap(long)]
    pub verify: bool,

    /// Bypass the `commit-msg` / `pre-merge-commit` hooks.
    #[clap(long)]
    pub no_verify: bool,

    /// Verify GPG signatures on the merged commits and abort on failure.
    #[clap(long = "verify-signatures")]
    pub verify_signatures: bool,

    /// Disable signature verification.
    #[clap(long = "no-verify-signatures")]
    pub no_verify_signatures: bool,

    /// Stash and re-apply local changes around the integration step.
    #[clap(long, overrides_with = "no_autostash")]
    pub autostash: bool,

    /// Disable autostashing.
    #[clap(long, overrides_with = "autostash")]
    pub no_autostash: bool,

    /// Use the named merge strategy.
    #[clap(short = 's', long, value_name = "strategy", action = clap::ArgAction::Append)]
    pub strategy: Vec<String>,

    /// Pass an option through to the merge strategy.
    #[clap(short = 'X', long = "strategy-option", value_name = "option=value", action = clap::ArgAction::Append)]
    pub strategy_option: Vec<String>,

    /// GPG-sign the resulting merge commit. Accepts an optional `<key-id>`.
    #[clap(
        short = 'S',
        long = "gpg-sign",
        num_args = 0..=1,
        default_missing_value = "",
        require_equals = true,
        value_name = "key-id",
    )]
    pub gpg_sign: Option<String>,

    /// Disable GPG signing of the merge commit.
    #[clap(long = "no-gpg-sign")]
    pub no_gpg_sign: bool,

    /// Allow merging two histories that share no common ancestor.
    #[clap(long = "allow-unrelated-histories")]
    pub allow_unrelated_histories: bool,

    // ── fetch options ─────────────────────────────────────────────────
    /// Fetch all configured remotes (passed through to fetch).
    #[clap(long)]
    pub all: bool,

    /// Append fetched refs to `.git/FETCH_HEAD` instead of overwriting.
    #[clap(short = 'a', long)]
    pub append: bool,

    /// Path to the upload-pack program on the remote end.
    #[clap(long, value_name = "PATH")]
    pub upload_pack: Option<OsString>,

    /// Force non-fast-forward ref updates during fetch.
    #[clap(short = 'f', long)]
    pub force: bool,

    /// Fetch all tags in addition to whatever else is fetched.
    #[clap(short = 't', long)]
    pub tags: bool,

    /// Remove remote-tracking refs that no longer exist on the remote.
    #[clap(short = 'p', long)]
    pub prune: bool,

    /// Number of parallel submodule / multi-remote fetches.
    #[clap(short = 'j', long, value_name = "N", num_args = 0..=1, default_missing_value = "0", require_equals = true)]
    pub jobs: Option<String>,

    /// Show what would be fetched without actually fetching.
    #[clap(long)]
    pub dry_run: bool,

    /// Keep the downloaded pack rather than exploding / discarding.
    #[clap(short = 'k', long)]
    pub keep: bool,

    /// Limit the fetched history to the last `<depth>` commits.
    #[clap(long, value_name = "DEPTH")]
    pub depth: Option<String>,

    /// Cut off all fetched history past the given date.
    #[clap(long, value_name = "DATE")]
    pub shallow_since: Option<String>,

    /// Cut off fetched history past the given tag/ref (repeatable).
    #[clap(long, value_name = "REF", action = clap::ArgAction::Append)]
    pub shallow_exclude: Vec<String>,

    /// Extend the current shallow boundary by `<n>` commits.
    #[clap(long, value_name = "N")]
    pub deepen: Option<String>,

    /// Remove the shallow boundary and fetch the full history.
    #[clap(long)]
    pub unshallow: bool,

    /// Accept refs that update `.git/shallow` during fetch.
    #[clap(long)]
    pub update_shallow: bool,

    /// Override the configured refmap with one or more refspecs.
    #[clap(long, value_name = "REFSPEC")]
    pub refmap: Vec<String>,

    /// Transmit a server-specific option (protocol v2).
    #[clap(short = 'o', long, value_name = "OPTION")]
    pub server_option: Vec<String>,

    /// Force IPv4 connections to the remote.
    #[clap(short = '4', long, overrides_with = "ipv6")]
    pub ipv4: bool,

    /// Force IPv6 connections to the remote.
    #[clap(short = '6', long, overrides_with = "ipv4")]
    pub ipv6: bool,

    /// Narrow negotiation to commits reachable from this tip (repeatable).
    #[clap(long, value_name = "COMMIT_OR_GLOB")]
    pub negotiation_tip: Vec<String>,

    /// Force the forced-update check on fetched refs.
    #[clap(long, overrides_with = "no_show_forced_updates")]
    pub show_forced_updates: bool,

    /// Skip the forced-update check (perf).
    #[clap(long, overrides_with = "show_forced_updates")]
    pub no_show_forced_updates: bool,

    /// Set up upstream tracking on the integrated branch.
    #[clap(long)]
    pub set_upstream: bool,

    // ── positionals ───────────────────────────────────────────────────
    /// The "remote" repository to pull from. Defaults to the configured
    /// upstream for the current branch.
    pub repository: Option<String>,

    /// Refspecs (branches, tags, …) to fetch and integrate.
    #[clap(value_parser = crate::shared::AsBString)]
    pub refspec: Vec<gix::bstr::BString>,
}
