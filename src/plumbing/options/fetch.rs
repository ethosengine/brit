//! `gix fetch` plumbing CLI.
//!
//! Flag surface mirrors `vendor/git/builtin/fetch.c::cmd_fetch`
//! `builtin_fetch_options[]` and `vendor/git/Documentation/fetch-options.adoc`.
//! Parity coverage lives in `tests/journey/parity/fetch.sh`.
//!
//! Most flags are currently parse-only (accepted by Clap but unused at
//! dispatch). Row-by-row iterations wire them through
//! `gitoxide_core::repository::fetch` as parity rows close.

use std::ffi::OsString;

use gix::remote::fetch::Shallow;

#[derive(Debug, clap::Parser)]
pub struct Platform {
    // ── ref-selection / scope ───────────────────────────────────────────
    /// Fetch from all configured remotes.
    ///
    /// Incompatible with a positional repository and with refspecs —
    /// validated at dispatch to match git's exit code (128) rather than
    /// Clap's default (2).
    #[clap(long)]
    pub all: bool,

    /// Set branch.<name>.{remote,merge} upstream tracking for the fetched
    /// branch.
    #[clap(long)]
    pub set_upstream: bool,

    /// Append to `.git/FETCH_HEAD` instead of overwriting it.
    #[clap(long, short = 'a')]
    pub append: bool,

    /// Atomic ref transaction — all refs update or none do.
    #[clap(long)]
    pub atomic: bool,

    /// Path to the upload-pack program on the remote end.
    #[clap(long, value_name = "PATH")]
    pub upload_pack: Option<OsString>,

    /// Force non-fast-forward ref updates.
    #[clap(long, short = 'f')]
    pub force: bool,

    /// Allow several repository / group positionals (needed for groups).
    #[clap(long, short = 'm')]
    pub multiple: bool,

    /// Fetch all tags (`refs/tags/*`) in addition to whatever else is
    /// fetched.
    #[clap(long, short = 't', overrides_with = "no_tags")]
    pub tags: bool,

    /// Disable automatic tag auto-following. Short `-n` mirrors git's
    /// `OPT_SET_INT('n', NULL, &tags, ..., TAGS_UNSET)`.
    #[clap(long, short = 'n', overrides_with = "tags")]
    pub no_tags: bool,

    /// Number of parallel submodule / multi-remote fetches.
    #[clap(long, short = 'j', value_name = "N")]
    pub jobs: Option<i64>,

    /// Rewrite the effective refspec to place all refs under `refs/prefetch/`.
    #[clap(long)]
    pub prefetch: bool,

    /// Remove remote-tracking refs that no longer exist on the remote.
    #[clap(long, short = 'p')]
    pub prune: bool,

    /// Also prune local tags no longer on the remote (requires --prune).
    #[clap(long, short = 'P')]
    pub prune_tags: bool,

    /// Control recursive fetching of submodules. Accepts `yes`, `on-demand`,
    /// or `no`. Bare form defaults to `yes` per git's `PARSE_OPT_OPTARG`.
    #[clap(
        long,
        value_name = "MODE",
        num_args = 0..=1,
        default_missing_value = "yes",
        require_equals = true,
    )]
    pub recurse_submodules: Option<String>,

    /// Disable recursive fetching of submodules (alias for
    /// `--recurse-submodules=no`).
    #[clap(long)]
    pub no_recurse_submodules: bool,

    // ── output / verbosity ──────────────────────────────────────────────
    /// Don't change the local repository, but otherwise try to be as
    /// accurate as possible.
    #[clap(long)]
    pub dry_run: bool,

    /// Machine-readable output format.
    #[clap(long)]
    pub porcelain: bool,

    /// Write fetched refs to `FETCH_HEAD` (the default).
    #[clap(long, overrides_with = "no_write_fetch_head")]
    pub write_fetch_head: bool,

    /// Do not write `FETCH_HEAD`.
    #[clap(long, overrides_with = "write_fetch_head")]
    pub no_write_fetch_head: bool,

    /// Keep the downloaded pack rather than exploding / discarding.
    #[clap(long, short = 'k')]
    pub keep: bool,

    /// Allow updating of `HEAD` ref (pull-internal).
    #[clap(long, short = 'u')]
    pub update_head_ok: bool,

    /// Force progress output even when stderr isn't a TTY.
    #[clap(long, overrides_with = "no_progress")]
    pub progress: bool,

    /// Disable progress output.
    #[clap(long, overrides_with = "progress")]
    pub no_progress: bool,

    /// Verbose output (inherited from git's `OPT__VERBOSITY`). Currently
    /// parsed but not consumed.
    #[clap(short = 'v', long)]
    pub verbose: bool,

    /// Suppress non-error output (inherited from git's `OPT__VERBOSITY`).
    #[clap(short = 'q', long)]
    pub quiet: bool,

    // ── shallow / history ───────────────────────────────────────────────
    #[clap(flatten)]
    pub shallow: ShallowOptions,

    /// Refetch all objects from scratch, ignoring local contents.
    #[clap(long)]
    pub refetch: bool,

    /// Accept refs that update `.git/shallow` during fetch.
    #[clap(long)]
    pub update_shallow: bool,

    /// Override the configured refmap with one or more refspecs.
    #[clap(long, value_name = "REFSPEC")]
    pub refmap: Vec<String>,

    // ── server / transport ──────────────────────────────────────────────
    /// Transmit a server-specific option (protocol v2).
    #[clap(long, short = 'o', value_name = "OPTION")]
    pub server_option: Vec<String>,

    /// Force IPv4 connections. Mutually overrides `-6`.
    #[clap(short = '4', long, overrides_with = "ipv6")]
    pub ipv4: bool,

    /// Force IPv6 connections. Mutually overrides `-4`.
    #[clap(short = '6', long, overrides_with = "ipv4")]
    pub ipv6: bool,

    /// Narrow the negotiation to commits reachable from this tip (repeatable).
    #[clap(long, value_name = "COMMIT_OR_GLOB")]
    pub negotiation_tip: Vec<String>,

    /// Do not fetch a pack; only print common ancestors with the server.
    #[clap(long)]
    pub negotiate_only: bool,

    /// Object-filter spec for partial clone (e.g. `blob:none`).
    #[clap(long, value_name = "FILTER_SPEC")]
    pub filter: Option<String>,

    /// Run `git maintenance --auto` after fetch.
    #[clap(long, overrides_with = "no_auto_maintenance")]
    pub auto_maintenance: bool,

    /// Suppress post-fetch auto-maintenance.
    #[clap(long, overrides_with = "auto_maintenance")]
    pub no_auto_maintenance: bool,

    /// Alias for `--auto-maintenance`.
    #[clap(long, overrides_with = "no_auto_gc")]
    pub auto_gc: bool,

    /// Alias for `--no-auto-maintenance`.
    #[clap(long, overrides_with = "auto_gc")]
    pub no_auto_gc: bool,

    /// Force the forced-update check on fetched refs.
    #[clap(long, overrides_with = "no_show_forced_updates")]
    pub show_forced_updates: bool,

    /// Skip the forced-update check (perf).
    #[clap(long, overrides_with = "show_forced_updates")]
    pub no_show_forced_updates: bool,

    /// Write commit-graph after fetch.
    #[clap(long, overrides_with = "no_write_commit_graph")]
    pub write_commit_graph: bool,

    /// Do not write commit-graph after fetch.
    #[clap(long, overrides_with = "write_commit_graph")]
    pub no_write_commit_graph: bool,

    /// Read refspecs from stdin in addition to those on the command line.
    #[clap(long)]
    pub stdin: bool,

    // ── gix-native extensions (no git equivalent) ────────────────────────
    /// (gix) Output server handshake information on stderr.
    #[clap(long, short = 'H')]
    pub handshake_info: bool,

    /// (gix) Print statistics about the negotiation phase.
    #[clap(long, short = 's')]
    pub negotiation_info: bool,

    /// (gix) Open the commit graph used for negotiation and write an SVG
    /// file to PATH.
    #[clap(long, value_name = "PATH", short = 'g')]
    pub open_negotiation_graph: Option<std::path::PathBuf>,

    /// (gix) Named remote or URL to connect to. Takes precedence over the
    /// positional `<repository>`. Kept for backwards compatibility with
    /// the pre-parity CLI surface.
    #[clap(long, short = 'r')]
    pub remote: Option<String>,

    // ── positionals ─────────────────────────────────────────────────────
    /// Named remote or URL of the repository to fetch from.
    ///
    /// When unset, `--remote` is consulted, then the upstream of the current
    /// branch.
    pub repository: Option<String>,

    /// Refspecs to fetch (override the remote's configured refspecs).
    #[clap(value_parser = crate::shared::AsBString)]
    pub refspec: Vec<gix::bstr::BString>,
}

#[derive(Debug, clap::Parser)]
pub struct ShallowOptions {
    /// Fetch with the history truncated to the given number of commits as
    /// seen from the remote.
    ///
    /// Accepted as a string (not a parsed number) so that `--depth=0` and
    /// negative values parse at the Clap layer and are rejected with git's
    /// exact error at dispatch (exit 128), not Clap's (exit 2).
    #[clap(long, help_heading = Some("SHALLOW"), value_name = "DEPTH", conflicts_with_all = ["shallow_since", "shallow_exclude", "deepen", "unshallow"])]
    pub depth: Option<String>,

    /// Extend the current shallow boundary by the given number of commits.
    ///
    /// Accepted as a string so negative values parse at Clap and are
    /// rejected with git's exact error at dispatch (exit 128).
    #[clap(long, help_heading = Some("SHALLOW"), value_name = "DEPTH", conflicts_with_all = ["depth", "shallow_since", "shallow_exclude", "unshallow"])]
    pub deepen: Option<String>,

    /// Cut off all history past the given date. Can be combined with
    /// `--shallow-exclude`.
    #[clap(long, help_heading = Some("SHALLOW"), value_parser = crate::shared::AsTime, value_name = "DATE", conflicts_with_all = ["depth", "deepen", "unshallow"])]
    pub shallow_since: Option<gix::date::Time>,

    /// Cut off all history past the given tag or ref.
    #[clap(long, help_heading = Some("SHALLOW"), value_parser = crate::shared::AsPartialRefName, value_name = "REF_NAME", conflicts_with_all = ["depth", "deepen", "unshallow"])]
    pub shallow_exclude: Vec<gix::refs::PartialName>,

    /// Remove the shallow boundary and fetch the entire history available
    /// on the remote.
    #[clap(long, help_heading = Some("SHALLOW"), conflicts_with_all = ["shallow_since", "shallow_exclude", "depth", "deepen"])]
    pub unshallow: bool,
}

/// Resolve the [`ShallowOptions`] block into a [`Shallow`] semantic. Returns
/// the parsed numeric depth / deepen values so callers can still die-128 on
/// bad inputs with git's exact message before the protocol layer rejects
/// them.
pub fn resolve_shallow(opts: &ShallowOptions) -> anyhow::Result<Shallow> {
    use std::num::NonZeroU32;

    if let Some(depth) = opts.depth.as_deref() {
        let parsed: i64 = depth
            .parse()
            .map_err(|_| anyhow::anyhow!("depth {depth} is not a positive number"))?;
        if parsed < 1 {
            anyhow::bail!("depth {depth} is not a positive number");
        }
        let nz = NonZeroU32::new(u32::try_from(parsed).unwrap_or(u32::MAX))
            .ok_or_else(|| anyhow::anyhow!("depth {depth} is not a positive number"))?;
        return Ok(Shallow::DepthAtRemote(nz));
    }
    if !opts.shallow_exclude.is_empty() {
        return Ok(Shallow::Exclude {
            remote_refs: opts.shallow_exclude.clone(),
            since_cutoff: opts.shallow_since,
        });
    }
    if let Some(cutoff) = opts.shallow_since {
        return Ok(Shallow::Since { cutoff });
    }
    if let Some(deepen) = opts.deepen.as_deref() {
        let parsed: i64 = deepen
            .parse()
            .map_err(|_| anyhow::anyhow!("cannot parse --deepen value '{deepen}'"))?;
        if parsed < 0 {
            anyhow::bail!("negative depth in --deepen is not supported");
        }
        let d = u32::try_from(parsed).unwrap_or(u32::MAX);
        return Ok(Shallow::Deepen(d));
    }
    if opts.unshallow {
        return Ok(Shallow::undo());
    }
    Ok(Shallow::default())
}
