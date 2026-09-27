//! Bare-form `gix mv` porcelain entry point.
//!
//! Mirrors `vendor/git/builtin/mv.c::cmd_mv`
//! (`vendor/git/builtin/mv.c:208`) — the porcelain shape that moves or
//! renames tracked files in the working tree and the index. Currently
//! a placeholder: the happy-path rows in `tests/journey/parity/mv.sh`
//! close with `compat_effect "deferred until mv driver lands"` until
//! the real driver lands.
//!
//! Behaviour:
//!
//! * Outside a repository: handled by the shared `repository(Mode::
//!   Lenient)` glue in `src/plumbing/main.rs`, which remaps
//!   `gix_discover::upwards::Error::NoGitRepository*` to git's exact
//!   "fatal: not a git repository (or any of the parent directories):
//!   .git" + exit 128.
//!
//! * Otherwise: walk through the cmd_mv entry-point preconditions
//!   that don't require the index walker:
//!     - `argc < 2` (covers the bare `git mv` and the `git mv <src>`
//!       cases which both go through `argc < 1` after the destination
//!       decrement at `vendor/git/builtin/mv.c:247..248`): emit the
//!       verbatim `usage_with_options` banner + exit 129.
//!     - Self-rename (`src == dst` byte-equal): mirrors the
//!       `!strncmp(src, dst, length) && (dst[length] == 0 || ...)`
//!       branch at `vendor/git/builtin/mv.c:346..350` — emit the
//!       verbatim `fatal: can not move directory into itself,
//!       source=X, destination=Y` + exit 128.
//!     - Multi-source with non-directory destination (mirrors
//!       `vendor/git/builtin/mv.c:278..279` "destination '%s' is not a
//!       directory"): emit verbatim + exit 128.
//!     - Source missing on disk (no index check yet): emit verbatim
//!       `fatal: bad source, source=X, destination=Y` + exit 128
//!       (mirrors the `bad = _("bad source")` branch at
//!       `vendor/git/builtin/mv.c:322..323`). Sparse-checkout files
//!       that are in the index but absent from the workdir are a
//!       false positive that the deferred driver will resolve.
//!
//! On the happy path emit a single-line stub note on stdout and exit
//! 0 so `compat_effect`-mode rows match git's exit code while the
//! real mv driver is unimplemented.
//!
//! Bytes parity on the happy path (real `internal_prefix_pathspec` +
//! per-source/destination resolve loop + `rename(2)` + index update +
//! per-file `Renaming X to Y\n` emission under `-v`/`-n` +
//! `--ignore-errors` short-circuit + sparse-checkout advice +
//! submodule gitfile rewrite) is deferred until the mv driver lands.
//! The shared deferral phrase is `"deferred until mv driver lands"`.

use anyhow::Result;
use gix::bstr::{BString, ByteSlice};

/// Subset of `mv::Platform` flags consumed by the porcelain stub.
#[derive(Debug, Default)]
pub struct Options {
    pub verbose: bool,
    pub dry_run: bool,
    pub force: bool,
    pub ignore_errors: bool,
    pub sparse: bool,
}

/// The verbatim `usage_with_options` banner emitted by
/// `vendor/git/builtin/mv.c:31..34` + the `builtin_mv_options` table at
/// `vendor/git/builtin/mv.c:215..223`. parse-options.c renders boolean
/// flags with the `--[no-]` prefix; mirrored here byte-for-byte.
const USAGE_BANNER: &str = "\
usage: git mv [<options>] <source>... <destination>

    -v, --[no-]verbose    be verbose
    -n, --[no-]dry-run    dry run
    -f, --[no-]force      force move/rename even if target exists
    -k                    skip move/rename errors
    --[no-]sparse         allow updating entries outside of the sparse-checkout cone

";

pub fn porcelain(
    repo: gix::Repository,
    out: &mut dyn std::io::Write,
    err: &mut dyn std::io::Write,
    args: Vec<BString>,
    paths: Vec<BString>,
    opts: Options,
) -> Result<()> {
    // Combine pre-`--` and post-`--` positionals into a single ordered
    // sequence — `git mv` doesn't distinguish them at the C layer
    // (parse-options consumes `--` as the option terminator).
    let positionals: Vec<&BString> = args.iter().chain(paths.iter()).collect();

    // 1. `argc < 2`: usage_with_options at vendor/git/builtin/mv.c:247..248.
    //    Both `git mv` (no positional) and `git mv <one>` route here
    //    because the C code decrements argc by 1 to peel off the
    //    destination, then checks `--argc < 1`.
    if positionals.len() < 2 {
        let _ = write!(err, "{USAGE_BANNER}");
        std::process::exit(129);
    }

    let last = positionals.len() - 1;
    let dst = positionals[last];

    // 2. Multi-source with non-directory destination: vendor/git/builtin/mv.c:278..279.
    //    Only fires for `argc != 1` (i.e. >= 2 sources, last positional is dst)
    //    when the destination doesn't lstat as a directory.
    if positionals.len() > 2 {
        let dst_str = dst.to_str_lossy();
        let workdir = repo.workdir();
        let dst_exists_as_dir = workdir.is_some_and(|wd| wd.join(dst_str.as_ref()).is_dir());
        if !dst_exists_as_dir {
            let _ = writeln!(err, "fatal: destination '{dst_str}' is not a directory");
            std::process::exit(128);
        }
    }

    // 3. Per-source gates that don't require the index walker. Single-source
    //    case (positionals.len() == 2) covers `git mv <src> <dst>`; multi-source
    //    case checks each source against the directory destination but the
    //    self-rename / bad-source semantics still apply per-source.
    let workdir = repo.workdir();
    for src in &positionals[..last] {
        let src_str = src.to_str_lossy();
        let dst_str = dst.to_str_lossy();

        // Self-rename: vendor/git/builtin/mv.c:346..350 "can not move
        // directory into itself". The C check is
        // `!strncmp(src, dst, length) && (dst[length] == 0 || dst[length] == '/')`
        // — when src and dst are byte-identical, dst[length] == 0 trivially
        // satisfies the second clause. The error wording carries
        // "directory" even when the path is a file; mirror git's wording.
        if src.as_slice() == dst.as_slice() {
            let _ = writeln!(
                err,
                "fatal: can not move directory into itself, source={src_str}, destination={dst_str}"
            );
            std::process::exit(128);
        }

        // Bad source: vendor/git/builtin/mv.c:322..323. The C path
        // checks `lstat(src) < 0 && index_name_pos(src) < 0` before
        // emitting `bad source`. Without index access in the placeholder
        // we approximate via workdir lstat alone — sparse-checkout
        // files that are in the index but absent from the workdir are a
        // false positive the deferred driver will resolve.
        let Some(workdir) = workdir else { continue };
        let exists = workdir.join(src_str.as_ref()).symlink_metadata().is_ok();
        if !exists && !opts.ignore_errors {
            let _ = writeln!(err, "fatal: bad source, source={src_str}, destination={dst_str}");
            std::process::exit(128);
        }
    }

    // Happy path placeholder: emit a stub note so the shape of stdout
    // is recognizable in failures, then exit 0 so `compat_effect`-mode
    // rows match git's exit code while the real mv driver is
    // unimplemented.
    let pos_names: Vec<String> = positionals.iter().map(|b| b.to_str_lossy().into_owned()).collect();
    let _ = writeln!(
        out,
        "[gix-mv] git_dir={} positionals={pos_names:?} verbose={} dry_run={} force={} ignore_errors={} sparse={}; mv driver not yet implemented",
        repo.git_dir().display(),
        opts.verbose,
        opts.dry_run,
        opts.force,
        opts.ignore_errors,
        opts.sparse,
    );
    Ok(())
}
