//! `brit rev-parse`: Git's spelling of `brit revision resolve`, plus Git's repository path queries.
//!
//! The arguments are read in order, like `git rev-parse` reads them: a flag such as `--verify`,
//! `--short` or `--abbrev-ref` applies to the revisions after it, and the path queries print where
//! they appear. Output follows Git for the forms agents use: full or abbreviated object names, one
//! per line; `A..B` as `B` and `^A`; `A...B` with its merge bases; short ref names; `--` and the
//! paths after it; and Git's relative/absolute choices for the path queries in main and linked
//! worktrees. The common failures print Git's diagnostic and exit 128 (or 1 for `--verify --quiet`),
//! and the known differences are listed below.
//!
//! ### Known differences from Git
//!
//! - A relative `GIT_DIR` is printed as an absolute `--git-dir`.
//! - `--abbrev-ref` always shortens strictly, as if `core.warnAmbiguousRefs` were true.
//! - Parent shorthands (`A^@`, `A^!`, `A^-`) and flags other than the ones listed in the help are
//!   refused by name instead of being interpreted.
//! - A `--` given before every revision and followed by nothing, like a lone `--`, is not echoed.
//! - Flags Git echoes or expands, like `-1` or `--all`, are refused by name with status 128.
//! - An ambiguous object prefix loses the resolver's diagnostic: it is reported as an unknown
//!   revision or path.
//! - `--path-format relative` as two arguments is accepted, as Brit's clap parser always did; Git
//!   needs `--path-format=relative`. `--help` after a revision shows Brit's help unless it follows `--`.
//! - Inside a `.git` directory itself, Brit's discovery fails, so only `--show-toplevel` answers like Git
//!   (`must be run in a work tree`, 128); `--git-dir` or a revision there fails at discovery with
//!   status 1. Below it, like in `.git/refs`, the queries work.
//! - The `<rev>:<path>` diagnostic omits Git's hint for a path given relative to a subdirectory.

use std::{
    io::Write,
    path::{Component, Path, PathBuf},
};

use gix::{
    Result,
    bstr::{BStr, ByteSlice},
    error::ResultExt,
    prelude::ObjectIdExt,
};

use super::{exit, shorten_unambiguous};

/// How path queries are printed after `--path-format`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PathFormat {
    /// Canonical absolute paths.
    Absolute,
    /// Paths relative to the current directory.
    Relative,
}

/// Git's `filter` bits that `--verify` and `--short` clear: from then on, files and `--` are not printed.
#[derive(Debug, Clone, Copy)]
struct State {
    verify: bool,
    quiet: bool,
    abbrev_ref: bool,
    /// `Some(None)` uses `core.abbrev`, `Some(Some(n))` at least `n` digits.
    abbrev: Option<Option<usize>>,
    /// Files and `--` are echoed until `--verify` or `--short` turns that off.
    show_files: bool,
    format: Option<PathFormat>,
}

/// Print each argument like `git rev-parse` would, reading `args` in order as Git reads its argv.
///
/// A Git-shaped failure writes its diagnostic to `err` and returns [`super::Exit`] with Git's status.
pub fn rev_parse(repo: gix::Repository, args: Vec<String>, mut out: impl Write, mut err: impl Write) -> Result<()> {
    let cwd = repo.current_dir().to_owned();
    let has_dashdash = args.iter().any(|arg| arg == "--");
    let mut state = State {
        verify: false,
        quiet: false,
        abbrev_ref: false,
        abbrev: None,
        show_files: true,
        format: None,
    };
    // 0: revisions, 1: after a path (still verified to exist), 2: after `--`.
    let mut as_is = 0_u8;
    let mut revs_count = 0_usize;
    let mut last: Option<(bool, gix::ObjectId, String)> = None;
    let fail = |err: &mut dyn Write, message: &str, code: i32| -> gix::Error {
        if !message.is_empty() {
            writeln!(err, "{message}").ok();
        }
        exit(code)
    };
    let no_single_rev = |err: &mut dyn Write, quiet: bool| -> gix::Error {
        if quiet {
            fail(err, "", 1)
        } else {
            fail(err, "fatal: Needed a single revision", 128)
        }
    };

    let mut args = args.iter();
    while let Some(arg) = args.next() {
        if as_is > 0 {
            if state.show_files {
                writeln!(out, "{arg}").or_error()?;
                if as_is < 2 {
                    verify_filename(&cwd, arg, None).map_err(|message| fail(&mut err, &message, 128))?;
                }
            }
            continue;
        }
        if arg.starts_with('-') {
            match arg.as_str() {
                "--" => {
                    as_is = 2;
                    if state.show_files {
                        writeln!(out, "--").or_error()?;
                    }
                }
                "--verify" => {
                    state.verify = true;
                    state.show_files = false;
                }
                "-q" | "--quiet" => state.quiet = true,
                "--abbrev-ref" | "--abbrev-ref=strict" => state.abbrev_ref = true,
                "--show-toplevel" => match workdir(&repo, &cwd) {
                    Some(workdir) => print_path(
                        &mut out,
                        &realpath(&workdir),
                        &cwd,
                        state.format.unwrap_or(PathFormat::Absolute),
                    )?,
                    None => return Err(fail(&mut err, "fatal: this operation must be run in a work tree", 128)),
                },
                "--git-dir" => {
                    let git_dir = absolute(repo.git_dir(), &cwd);
                    match state.format {
                        Some(format) => print_path(&mut out, &realpath(&git_dir), &cwd, format)?,
                        None if git_dir == cwd.join(".git") && git_dir.is_dir() => writeln!(out, ".git").or_error()?,
                        None if git_dir == cwd => writeln!(out, ".").or_error()?,
                        None => writeln!(out, "{}", git_dir.display()).or_error()?,
                    }
                }
                "--absolute-git-dir" => {
                    let git_dir = absolute(repo.git_dir(), &cwd);
                    print_path(&mut out, &realpath(&git_dir), &cwd, PathFormat::Absolute)?;
                }
                "--git-common-dir" => {
                    let common_dir = absolute(repo.common_dir(), &cwd);
                    // Git prints the shared directory relative to the current one only when it found it as
                    // the `.git` directory of this worktree; a linked worktree or a submodule names it absolutely.
                    let dot_git_here = common_dir == absolute(repo.git_dir(), &cwd)
                        && repo
                            .workdir()
                            .is_some_and(|workdir| absolute(workdir, &cwd).join(".git") == common_dir);
                    match state.format {
                        Some(format) => print_path(&mut out, &realpath(&common_dir), &cwd, format)?,
                        None if common_dir == cwd => writeln!(out, ".").or_error()?,
                        None if dot_git_here => writeln!(out, "{}", relative_path(&common_dir, &cwd)).or_error()?,
                        None => writeln!(out, "{}", common_dir.display()).or_error()?,
                    }
                }
                "--path-format=absolute" => state.format = Some(PathFormat::Absolute),
                "--path-format=relative" => state.format = Some(PathFormat::Relative),
                // Brit's clap spelling `--path-format relative`, kept after the first revision too.
                "--path-format" => match args.next().map(String::as_str) {
                    Some("absolute") => state.format = Some(PathFormat::Absolute),
                    Some("relative") => state.format = Some(PathFormat::Relative),
                    value => {
                        return Err(fail(
                            &mut err,
                            &format!(
                                "fatal: unknown argument to --path-format: {}",
                                value.unwrap_or_default()
                            ),
                            128,
                        ));
                    }
                },
                other => match other.strip_prefix("--short") {
                    Some("") => {
                        state.verify = true;
                        state.show_files = false;
                        state.abbrev = Some(None);
                    }
                    Some(value) if value.starts_with('=') => {
                        state.verify = true;
                        state.show_files = false;
                        state.abbrev = Some(Some(git_abbrev_len(&value[1..])));
                    }
                    _ => {
                        return Err(fail(
                            &mut err,
                            &format!(
                                "brit rev-parse does not implement `{other}`; resolve revisions with `brit revision resolve`"
                            ),
                            128,
                        ));
                    }
                },
            }
            continue;
        }

        // Not a flag argument.
        if let Some(shown) = try_difference(&repo, arg).map_err(|message| fail(&mut err, &message, 128))? {
            for (reversed, id, name) in shown {
                show(&repo, &mut out, &mut err, state, reversed, id, name.as_deref())?;
            }
            continue;
        }
        let (reversed, name) = match arg.strip_prefix('^') {
            Some(name) => (true, name),
            None => (false, arg.as_str()),
        };
        if let Some(id) = resolve(&repo, name).map_err(|message| fail(&mut err, &message, 128))? {
            if state.verify {
                revs_count += 1;
                last = Some((reversed, id, name.to_owned()));
            } else {
                show(&repo, &mut out, &mut err, state, reversed, id, Some(name))?;
            }
            continue;
        }
        if state.verify {
            return Err(no_single_rev(&mut err, state.quiet));
        }
        if has_dashdash {
            return Err(fail(&mut err, &format!("fatal: bad revision '{arg}'"), 128));
        }
        as_is = 1;
        if !state.show_files {
            continue;
        }
        writeln!(out, "{arg}").or_error()?;
        verify_filename(&cwd, arg, Some(&repo)).map_err(|message| fail(&mut err, &message, 128))?;
    }
    if state.verify {
        return match last.filter(|_| revs_count == 1) {
            Some((reversed, id, name)) => show(&repo, &mut out, &mut err, state, reversed, id, Some(&name)),
            None => Err(no_single_rev(&mut err, state.quiet)),
        };
    }
    Ok(())
}

/// Git's answer from inside a Git directory where Brit's discovery failed: Git then works like in a bare
/// repository, so a `--show-toplevel` reached before any query that needs the repository is
/// `fatal: this operation must be run in a work tree` and 128. `None` leaves the discovery error standing.
///
/// This runs only after discovery failed, so it never changes which repository a command selects.
pub fn outside_work_tree(args: &[String], cwd: &Path, mut err: impl Write) -> Option<gix::Error> {
    gix::discover::is_git(cwd).ok()?;
    let mut args = args.iter();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--show-toplevel" => {
                writeln!(err, "fatal: this operation must be run in a work tree").ok();
                return Some(exit(128));
            }
            "--path-format" => {
                args.next();
            }
            "--verify" | "-q" | "--quiet" | "--abbrev-ref" | "--abbrev-ref=strict" => {}
            other if other.starts_with("--path-format=") || other.starts_with("--short") => {}
            _ => return None,
        }
    }
    None
}

/// Git's `--short=<n>`: `strtoul()` of the value, clamped to at least 4 digits, so an empty, negative or
/// non-numeric value means 4.
fn git_abbrev_len(value: &str) -> usize {
    let value = value.trim_start();
    let (negative, digits) = match value.as_bytes().first() {
        Some(b'-') => (true, &value[1..]),
        Some(b'+') => (false, &value[1..]),
        _ => (false, value),
    };
    let digits: String = digits.chars().take_while(char::is_ascii_digit).collect();
    let len = digits
        .parse::<usize>()
        .unwrap_or(if digits.is_empty() { 0 } else { usize::MAX });
    if negative && len != 0 { 0 } else { len }
}

/// The worktree Git would see from `cwd`: none when `cwd` is inside the Git directory, as Git then works
/// like in a bare repository (reached from a directory below a `.git` directory, like `.git/refs`).
fn workdir(repo: &gix::Repository, cwd: &Path) -> Option<PathBuf> {
    let workdir = absolute(repo.workdir()?, cwd);
    let inside_git_dir = realpath(cwd).starts_with(realpath(&absolute(repo.git_dir(), cwd)));
    (!inside_git_dir).then_some(workdir)
}

/// One revision without range syntax, or `None` if it does not name an object. `Err` refuses a form
/// that names several objects, like `A^@`, with the diagnostic to print.
fn resolve(repo: &gix::Repository, name: &str) -> std::result::Result<Option<gix::ObjectId>, String> {
    match repo
        .rev_parse(BStr::new(name.as_bytes()))
        .map(gix::revision::Spec::detach)
    {
        Ok(gix::revision::plumbing::Spec::Include(id)) => Ok(Some(id)),
        Ok(_) => Err(format!(
            "brit rev-parse: `{name}` is not implemented; list commits with `brit revision list` or find bases with `brit merge-base`"
        )),
        Err(_) => Ok(None),
    }
}

/// One printed row: whether it is excluded (`^`), the object, and the name `--abbrev-ref` shortens.
type Shown = (bool, gix::ObjectId, Option<String>);

/// Git's `try_difference()`: `A..B` shows `B` and `^A`, and `A...B` shows `B`, `A` and `^<base>` for each
/// merge base. An empty side means `HEAD`; `None` when `arg` is not such a range.
fn try_difference(repo: &gix::Repository, arg: &str) -> std::result::Result<Option<Vec<Shown>>, String> {
    let Some(dotdot) = arg.find("..") else {
        return Ok(None);
    };
    let symmetric = arg[dotdot + 2..].starts_with('.');
    let (start, end) = (&arg[..dotdot], &arg[dotdot + 2 + usize::from(symmetric)..]);
    if start.is_empty() && end.is_empty() && !symmetric {
        // Just `..` is the parent directory, not a range.
        return Ok(None);
    }
    let side = |name: &str| if name.is_empty() { "HEAD" } else { name }.to_owned();
    let (start, end) = (side(start), side(end));
    let (Some(start_id), Some(end_id)) = (resolve(repo, &start)?, resolve(repo, &end)?) else {
        return Ok(None);
    };
    let mut shown = vec![(false, end_id, Some(end)), (!symmetric, start_id, Some(start))];
    if symmetric {
        let source = gix::object::bounded::Source::deferred(repo);
        let (Ok(a), Ok(b)) = (
            source.peel(start_id, gix::object::Kind::Commit),
            source.peel(end_id, gix::object::Kind::Commit),
        ) else {
            return Ok(None);
        };
        let bases = repo
            .merge_bases_many(a, &[b])
            .map_err(|err| format!("fatal: cannot find the merge base of '{arg}': {err}"))?;
        for base in bases {
            shown.push((true, base.detach(), None));
        }
    }
    Ok(Some(shown))
}

fn show(
    repo: &gix::Repository,
    mut out: impl Write,
    mut err: impl Write,
    state: State,
    reversed: bool,
    id: gix::ObjectId,
    name: Option<&str>,
) -> Result<()> {
    let marker = if reversed { "^" } else { "" };
    if let (true, Some(name)) = (state.abbrev_ref, name) {
        // Git prints nothing for a revision that is not a ref, like `HEAD~1`.
        if let Some(full) = dwim_full_name(repo, name) {
            let short = shorten_unambiguous(&full, |candidate| ref_exists(repo, candidate));
            writeln!(out, "{marker}{short}").or_error()?;
        }
        return Ok(());
    }
    match state.abbrev {
        Some(len) => {
            let prefix = match len {
                None => id.attach(repo).shorten()?,
                Some(len) => {
                    let len = len.clamp(4, id.kind().len_in_hex());
                    let candidate = gix::odb::store::prefix::disambiguate::Candidate::new(id, len).or_error()?;
                    match repo.objects.disambiguate_prefix(candidate)? {
                        Some(prefix) => prefix,
                        None => {
                            writeln!(err, "fatal: {id}: object not found").ok();
                            id.into()
                        }
                    }
                }
            };
            writeln!(out, "{marker}{prefix}").or_error()?;
        }
        None => writeln!(out, "{marker}{id}").or_error()?,
    }
    Ok(())
}

/// Git's `verify_filename()`: `Err` holds the diagnostic for an argument that is neither a revision nor an
/// existing path. With `repo`, a misspelt `<rev>:<path>` is diagnosed like Git does first.
fn verify_filename(cwd: &Path, arg: &str, repo: Option<&gix::Repository>) -> std::result::Result<(), String> {
    if arg.starts_with('-') {
        return Err(format!("fatal: option '{arg}' must come before non-option arguments"));
    }
    let looks_like_pathspec = arg.contains(['*', '?', '[', '\\']) || arg.starts_with(":(");
    if looks_like_pathspec || cwd.join(arg).symlink_metadata().is_ok() {
        return Ok(());
    }
    let Some(repo) = repo else {
        return Err(format!(
            "fatal: {arg}: no such path in the working tree.\nUse 'git <command> -- <path>...' to specify paths that do not exist locally."
        ));
    };
    if let Some(diagnostic) = misspelt_object_path(repo, arg) {
        return Err(diagnostic);
    }
    Err(format!(
        "fatal: ambiguous argument '{arg}': unknown revision or path not in the working tree.\nUse '--' to separate paths from revisions, like this:\n'git <command> [<revision>...] -- [<file>...]'"
    ))
}

/// Git's `diagnose_invalid_oid_path()` and `diagnose_invalid_index_path()` for `<rev>:<path>` and `:<path>`.
fn misspelt_object_path(repo: &gix::Repository, arg: &str) -> Option<String> {
    let top = repo.workdir().map(|workdir| absolute(workdir, repo.current_dir()));
    let on_disk = |path: &str| {
        top.as_ref()
            .is_some_and(|top| top.join(path).symlink_metadata().is_ok())
    };
    if let Some(path) = arg.strip_prefix(':') {
        // Git leaves magic pathspecs like `:(icase)x` or `:/x` alone.
        if !path.starts_with(|c: char| c.is_ascii_alphanumeric()) {
            return None;
        }
        return Some(if on_disk(path) {
            format!("fatal: path '{path}' exists on disk, but not in the index")
        } else {
            format!("fatal: path '{path}' does not exist (neither on disk nor in the index)")
        });
    }
    let (object_name, path) = arg.split_once(':')?;
    let id = resolve(repo, object_name).ok()??;
    gix::object::bounded::Source::deferred(repo)
        .peel(id, gix::object::Kind::Tree)
        .ok()?;
    Some(if on_disk(path) {
        format!("fatal: path '{path}' exists on disk, but not in '{object_name}'")
    } else {
        format!("fatal: path '{path}' does not exist in '{object_name}'")
    })
}

/// The full ref name that `name` resolves to, following symbolic refs, or `None` for a non-ref.
fn dwim_full_name(repo: &gix::Repository, name: &str) -> Option<String> {
    let name = if name == "@" { "HEAD" } else { name };
    let lower = name.to_ascii_lowercase();
    if let Some(branch) = lower
        .strip_suffix("@{upstream}")
        .or_else(|| lower.strip_suffix("@{u}"))
        .map(|branch| &name[..branch.len()])
    {
        let branch = if branch.is_empty() || branch == "HEAD" || branch == "@" {
            repo.head_name().ok()??
        } else {
            repo.try_find_reference(branch).ok()??.name().to_owned()
        };
        let tracking = repo
            .branch_remote_tracking_ref_name(branch.as_ref(), gix::remote::Direction::Fetch)?
            .ok()?;
        return Some(tracking.as_bstr().to_string());
    }
    let mut reference = repo.try_find_reference(name).ok()??;
    for _ in 0..5 {
        match reference.follow() {
            Some(Ok(next)) => reference = next,
            Some(Err(_)) => return None,
            None => break,
        }
    }
    Some(reference.name().as_bstr().to_string())
}

/// Whether a ref with exactly this full name exists; the lookup rules never stand in for it.
pub(crate) fn ref_exists(repo: &gix::Repository, full_name: &str) -> bool {
    repo.try_find_reference(full_name)
        .ok()
        .flatten()
        .is_some_and(|reference| reference.name().as_bstr() == full_name.as_bytes().as_bstr())
}

fn absolute(path: &Path, cwd: &Path) -> PathBuf {
    let joined = if path.is_absolute() {
        path.to_owned()
    } else {
        cwd.join(path)
    };
    gix::path::normalize(joined.as_path().into(), cwd).map_or(joined.clone(), std::borrow::Cow::into_owned)
}

fn realpath(path: &Path) -> PathBuf {
    gix::path::realpath(path).unwrap_or_else(|_| path.to_owned())
}

fn print_path(mut out: impl Write, path: &Path, cwd: &Path, format: PathFormat) -> Result<()> {
    match format {
        PathFormat::Absolute => writeln!(out, "{}", path.display()),
        PathFormat::Relative => writeln!(out, "{}", relative_path(path, &realpath(cwd))),
    }
    .or_error()
}

/// Git's `relative_path()`: `./` for the directory itself, `../` steps (with a trailing slash) for
/// an ancestor, and a plain relative path otherwise, separated by `/` on every platform as Git
/// prints it.
///
/// With no component in common, which only happens on Windows (a different drive, or a UNC path
/// against a drive path), `rest` is the whole absolute path and every component of `base`, its
/// prefix and root included, counts as a `../`: `C:\x\y` against `D:\z` gives `../../../C:/x/y`
/// (before separators were converted, `../../../C:\x\y`). Git prints the absolute path unchanged
/// there; that difference remains.
fn relative_path(path: &Path, base: &Path) -> String {
    let path: Vec<Component<'_>> = path.components().collect();
    let base: Vec<Component<'_>> = base.components().collect();
    let common = path.iter().zip(&base).take_while(|(a, b)| a == b).count();
    let ups = base.len() - common;
    let rest: PathBuf = path[common..].iter().collect();
    match (ups, rest.as_os_str().is_empty()) {
        (0, true) => "./".into(),
        (0, false) => slash_separated(&rest),
        (ups, true) => "../".repeat(ups),
        (ups, false) => format!("{}{}", "../".repeat(ups), slash_separated(&rest)),
    }
}

/// `path` for display with `/` between its components on every platform; unchanged on Unix.
///
/// A Windows path that is not valid Unicode keeps `display()`'s native separators.
fn slash_separated(path: &Path) -> String {
    match gix::path::into_bstr(path) {
        Ok(bytes) => gix::path::to_unix_separators_on_windows(bytes).to_string(),
        Err(_) => path.display().to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::{git_abbrev_len, relative_path};
    use std::path::Path;

    #[test]
    fn relative_paths_match_git() {
        assert_eq!(relative_path(Path::new("/a/b"), Path::new("/a/b")), "./");
        assert_eq!(relative_path(Path::new("/a/b"), Path::new("/a/b/c")), "../");
        assert_eq!(relative_path(Path::new("/a"), Path::new("/a/b/c")), "../../");
        assert_eq!(relative_path(Path::new("/a/b/.git"), Path::new("/a/b")), ".git");
        assert_eq!(relative_path(Path::new("/a/b/.git"), Path::new("/a/b/c")), "../.git");
        assert_eq!(
            relative_path(Path::new("/a/main/.git"), Path::new("/a/wt")),
            "../main/.git"
        );
    }

    #[test]
    fn short_lengths_parse_like_strtoul() {
        for (value, len) in [
            ("", 0),
            ("-1", 0),
            ("abc", 0),
            ("12", 12),
            ("12abc", 12),
            ("+7", 7),
            (" 9", 9),
        ] {
            assert_eq!(git_abbrev_len(value), len, "{value:?}");
        }
    }
}
