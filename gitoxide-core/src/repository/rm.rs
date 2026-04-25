//! Bare-form `gix rm` porcelain entry point.
//!
//! Mirrors `vendor/git/builtin/rm.c::cmd_rm`
//! (`vendor/git/builtin/rm.c:266`) — the porcelain shape that removes
//! tracked files from the working tree and the index. Currently a
//! placeholder: every flag-bearing parity row in
//! `tests/journey/parity/rm.sh` closes with
//! `compat_effect "deferred until rm driver lands"` until the real
//! driver lands.
//!
//! Behaviour:
//!
//! * Outside a repository: handled by the shared `repository(Mode::
//!   Lenient)` glue in `src/plumbing/main.rs`, which remaps
//!   `gix_discover::upwards::Error::NoGitRepository*` to git's exact
//!   "fatal: not a git repository (or any of the parent directories):
//!   .git" + exit 128.
//!
//! * Otherwise: walk through git's pre-pathspec validation matrix
//!   (`vendor/git/builtin/rm.c:286..299`) and emit the verbatim
//!   wording for each precondition gate. On the happy path emit a
//!   single-line stub note on stdout and exit 0 so
//!   `compat_effect`-mode rows match git's exit code while the real
//!   rm driver is unimplemented.
//!
//! Bytes parity (real `remove_file_from_index` walk + working-tree
//! deletion via `remove_path` + per-file `rm '<path>'` emission +
//! up-to-date-check error stanzas + submodule absorb + sparse-checkout
//! advice + `not removing '<x>' recursively without -r` enforcement)
//! is deferred until the rm driver lands. The shared deferral phrase
//! is `"deferred until rm driver lands"`.

use anyhow::Result;
use gix::bstr::{BString, ByteSlice};

/// Subset of `rm::Platform` flags consumed by the porcelain stub.
#[derive(Debug, Default)]
pub struct Options {
    pub dry_run: bool,
    pub quiet: bool,
    pub cached: bool,
    pub force: bool,
    pub recursive: bool,
    pub ignore_unmatch: bool,
    pub sparse: bool,
    pub pathspec_from_file: Option<BString>,
    pub pathspec_file_nul: bool,
}

pub fn porcelain(
    repo: gix::Repository,
    out: &mut dyn std::io::Write,
    err: &mut dyn std::io::Write,
    args: Vec<BString>,
    paths: Vec<BString>,
    opts: Options,
) -> Result<()> {
    // Precondition gates run in the same order as
    // `vendor/git/builtin/rm.c::cmd_rm` (lines 286..299).

    let pathspec_present = !args.is_empty() || !paths.is_empty();

    // 1. `--pathspec-from-file` and pathspec arguments are mutually
    //    exclusive (vendor/git/builtin/rm.c:286..287).
    if opts.pathspec_from_file.is_some() && pathspec_present {
        let _ = writeln!(
            err,
            "fatal: '--pathspec-from-file' and pathspec arguments cannot be used together"
        );
        std::process::exit(128);
    }

    // 2. `--pathspec-file-nul` requires `--pathspec-from-file`
    //    (vendor/git/builtin/rm.c:294..295).
    if opts.pathspec_file_nul && opts.pathspec_from_file.is_none() {
        let _ = writeln!(
            err,
            "fatal: the option '--pathspec-file-nul' requires '--pathspec-from-file'"
        );
        std::process::exit(128);
    }

    // 3. Empty pathspec dies 128 with a verbatim die() message
    //    (vendor/git/builtin/rm.c:298..299). The check considers both
    //    cmdline pathspecs and the `--pathspec-from-file` source; we
    //    treat the file as opaque (the deferred driver does the real
    //    parse) and only fail when neither source is present.
    if !pathspec_present && opts.pathspec_from_file.is_none() {
        let _ = writeln!(err, "fatal: No pathspec was given. Which files should I remove?");
        std::process::exit(128);
    }

    // 4. Mirror git's "pathspec '<x>' did not match any files" path
    //    (vendor/git/builtin/rm.c:357) for non-magic positional
    //    pathspecs that don't exist in the workdir, unless
    //    `--ignore-unmatch` is set (vendor/git/builtin/rm.c:354..355).
    //    Magic-char pathspecs (`*`/`?`/`[`) and the `:` magic-prefix
    //    forms always reach the full pathspec walker, which is part
    //    of the deferred rm driver.
    if !opts.ignore_unmatch {
        let workdir = repo.workdir();
        let combined_paths: Vec<&BString> = args.iter().chain(paths.iter()).collect();
        for spec in combined_paths {
            let bytes: &[u8] = spec.as_ref();
            if bytes.is_empty() {
                continue;
            }
            if bytes.iter().any(|&b| b == b'*' || b == b'?' || b == b'[' || b == b':') {
                continue;
            }
            let Some(workdir) = workdir else { continue };
            let rel = match bytes.to_str() {
                Ok(s) => s,
                Err(_) => continue,
            };
            if !workdir.join(rel).exists() {
                let _ = writeln!(err, "fatal: pathspec '{}' did not match any files", spec.to_str_lossy());
                std::process::exit(128);
            }
        }
    }

    // Happy path placeholder: emit a stub note so the shape of stdout
    // is recognizable in failures, then exit 0 so `compat_effect`-mode
    // rows match git's exit code while the real rm driver is
    // unimplemented.
    let arg_names: Vec<String> = args.iter().map(|b| b.to_str_lossy().into_owned()).collect();
    let path_names: Vec<String> = paths.iter().map(|b| b.to_str_lossy().into_owned()).collect();
    let _ = writeln!(
        out,
        "[gix-rm] git_dir={} args={arg_names:?} paths={path_names:?}; rm driver not yet implemented",
        repo.git_dir().display(),
    );
    Ok(())
}
