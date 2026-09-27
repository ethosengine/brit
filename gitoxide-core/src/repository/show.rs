//! Bare-form `gix show` porcelain entry point.
//!
//! Mirrors `vendor/git/builtin/log.c::cmd_show` (`vendor/git/builtin/log.c:657`)
//! — the porcelain shape that renders one or more named objects (blobs,
//! trees, tags, commits) without walking ancestry. Currently a
//! placeholder: every flag-bearing parity row in
//! `tests/journey/parity/show.sh` closes with
//! `compat_effect "deferred until show driver lands"` until the real
//! driver lands.
//!
//! Behaviour:
//!
//! * No `<object>` given: defaults to `HEAD`
//!   (`vendor/git/builtin/log.c:688` `setup_revision_opt::def = "HEAD"`).
//!   Unborn-HEAD is gated upstream by `cmd_log_walk → revision.c::die`
//!   to "fatal: your current branch '<name>' does not have any commits
//!   yet" + exit 128.
//!
//! * `<object>` that does not resolve: `vendor/git/revision.c::handle_revision_arg`
//!   emits the exact 3-line "ambiguous argument" stanza and `die(128)`.
//!
//! * Otherwise: emit a single-line stub note on stderr and exit 0 so
//!   `compat_effect`-mode rows match git's exit code while the real
//!   show driver is unimplemented.
//!
//! Bytes parity (real header + body emission for blob / tag / tree /
//! commit, including `tag <name>\n` / `tree <name>\n\n` framing,
//! `diff-tree --cc`-style merge rendering, pretty-format and diff
//! options threading) is deferred until the show driver lands. The
//! shared deferral phrase is `"deferred until show driver lands"`.

use anyhow::Result;
use gix::bstr::{BStr, BString, ByteSlice};

/// Subset of `show::Platform` flags consumed by the porcelain stub.
///
/// Today the stub only consults the positionals to decide between the
/// default-HEAD path and the bad-revspec gate. Once the real show
/// driver lands the rest of the flag surface gets threaded in here.
#[derive(Debug, Default)]
pub struct Options {}

pub fn porcelain(
    repo: gix::Repository,
    out: &mut dyn std::io::Write,
    err: &mut dyn std::io::Write,
    objects: Vec<BString>,
    _opts: Options,
) -> Result<()> {
    // Default-to-HEAD path: `cmd_show` calls `cmd_log_init` with
    // `setup_revision_opt::def = "HEAD"` (vendor/git/builtin/log.c:688).
    // When no positional is given, the resolution still runs against
    // HEAD; if HEAD is unborn that resolution dies 128.
    let resolved: Vec<BString> = if objects.is_empty() {
        let head = repo.head()?;
        if let gix::head::Kind::Unborn(name) = &head.kind {
            // Mirror cmd_log_walk → revision.c::die wording for the
            // unborn-HEAD precondition. git emits this on stderr.
            let _ = writeln!(
                err,
                "fatal: your current branch '{}' does not have any commits yet",
                name.shorten()
            );
            std::process::exit(128);
        }
        vec![BString::from("HEAD")]
    } else {
        objects
    };

    // Bad-revspec gate: any object that does not resolve dies 128 with
    // git's exact 3-line stanza (vendor/git/revision.c::handle_revision_arg).
    for spec in &resolved {
        let spec_bstr: &BStr = spec.as_ref();
        if repo.rev_parse_single(spec_bstr).is_err() {
            let _ = writeln!(
                err,
                "fatal: ambiguous argument '{}': unknown revision or path not in the working tree.\n\
                 Use '--' to separate paths from revisions, like this:\n\
                 'git <command> [<revision>...] -- [<file>...]'",
                spec_bstr.to_str_lossy()
            );
            std::process::exit(128);
        }
    }

    // Happy path placeholder: emit a stub note so the shape of stderr
    // is recognizable in failures, then exit 0 so `compat_effect`-mode
    // rows match git's exit code while the real show driver is
    // unimplemented.
    let names: Vec<String> = resolved.iter().map(|b| b.to_str_lossy().into_owned()).collect();
    let _ = writeln!(
        out,
        "[gix-show] received objects={names:?}; show driver not yet implemented",
    );
    Ok(())
}
