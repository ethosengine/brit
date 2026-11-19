//! `brit rev-parse`: Git's spelling of `brit revision resolve`, plus Git's repository path queries.
//!
//! Output follows `git rev-parse` for the forms agents use: full or abbreviated object names, one
//! per line; `A..B` as `B` and `^A`; `--abbrev-ref` short ref names; and the path queries in the
//! order they were given, with Git's relative/absolute choices in main and linked worktrees.
//! Failures print Git's messages and exit 128 (or 1 for `--verify --quiet`).

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

use super::shorten_unambiguous;

/// One ordered command-line item.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Arg {
    /// A revision to resolve.
    Rev(String),
    /// `--show-toplevel`
    ShowToplevel,
    /// `--git-dir`
    GitDir,
    /// `--absolute-git-dir`
    AbsoluteGitDir,
    /// `--git-common-dir`
    GitCommonDir,
    /// `--path-format=…`, applying to the path queries after it.
    PathFormat(PathFormat),
    /// Everything after `--`, printed verbatim.
    Paths(Vec<String>),
}

/// How path queries are printed after `--path-format`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PathFormat {
    /// Canonical absolute paths.
    Absolute,
    /// Paths relative to the current directory.
    Relative,
}

/// Modes that apply to every revision.
#[derive(Debug, Default, Clone, Copy)]
pub struct Options {
    /// Require exactly one revision.
    pub verify: bool,
    /// With `verify`, fail silently with exit code 1.
    pub quiet: bool,
    /// Print short ref names instead of object names.
    pub abbrev_ref: bool,
    /// Abbreviate object names: `Some(None)` uses `core.abbrev`, `Some(Some(n))` at least `n` digits.
    pub short: Option<Option<usize>>,
}

/// Print each argument like `git rev-parse` would.
pub fn rev_parse(
    repo: gix::Repository,
    args: Vec<Arg>,
    mut out: impl Write,
    mut err: impl Write,
    options: Options,
) -> Result<()> {
    let verify = options.verify || options.short.is_some();
    let cwd = repo.current_dir().to_owned();
    let mut format = None;
    let mut as_is = false;
    let mut single: Option<(bool, gix::ObjectId, String)> = None;
    for arg in args {
        match arg {
            Arg::PathFormat(wanted) => format = Some(wanted),
            Arg::ShowToplevel => match repo.workdir() {
                Some(workdir) => {
                    let workdir = absolute(workdir, &cwd);
                    print_path(
                        &mut out,
                        &realpath(&workdir),
                        &cwd,
                        format.unwrap_or(PathFormat::Absolute),
                    )?;
                }
                None => die(
                    &mut out,
                    &mut err,
                    "fatal: this operation must be run in a work tree",
                    128,
                ),
            },
            Arg::GitDir => {
                let git_dir = absolute(repo.git_dir(), &cwd);
                match format {
                    Some(format) => print_path(&mut out, &realpath(&git_dir), &cwd, format)?,
                    None if git_dir == cwd.join(".git") && git_dir.is_dir() => writeln!(out, ".git").or_error()?,
                    None if git_dir == cwd => writeln!(out, ".").or_error()?,
                    None => writeln!(out, "{}", git_dir.display()).or_error()?,
                }
            }
            Arg::AbsoluteGitDir => {
                let git_dir = absolute(repo.git_dir(), &cwd);
                print_path(&mut out, &realpath(&git_dir), &cwd, PathFormat::Absolute)?;
            }
            Arg::GitCommonDir => {
                let common_dir = absolute(repo.common_dir(), &cwd);
                // Git prints the shared directory relative to the current one only when it found it as
                // the `.git` directory of this worktree; a linked worktree or a submodule names it absolutely.
                let dot_git_here = common_dir == absolute(repo.git_dir(), &cwd)
                    && repo
                        .workdir()
                        .is_some_and(|workdir| absolute(workdir, &cwd).join(".git") == common_dir);
                match format {
                    Some(format) => print_path(&mut out, &realpath(&common_dir), &cwd, format)?,
                    None if common_dir == cwd => writeln!(out, ".").or_error()?,
                    None if dot_git_here => writeln!(out, "{}", relative_path(&common_dir, &cwd)).or_error()?,
                    None => writeln!(out, "{}", common_dir.display()).or_error()?,
                }
            }
            Arg::Paths(paths) => {
                writeln!(out, "--").or_error()?;
                for path in paths {
                    writeln!(out, "{path}").or_error()?;
                }
            }
            Arg::Rev(rev) if as_is => {
                if !cwd.join(&rev).exists() {
                    die(
                        &mut out,
                        &mut err,
                        &format!(
                            "fatal: {rev}: no such path in the working tree.\nUse 'git <command> -- <path>...' to specify paths that do not exist locally."
                        ),
                        128,
                    );
                }
                writeln!(out, "{rev}").or_error()?;
            }
            Arg::Rev(rev) => {
                let spec = repo
                    .rev_parse(BStr::new(rev.as_bytes()))
                    .ok()
                    .map(gix::revision::Spec::detach);
                let shown = match spec {
                    Some(gix::revision::plumbing::Spec::Include(id)) if verify => {
                        if single.replace((false, id, rev.clone())).is_some() {
                            die_no_single_rev(&mut out, &mut err, options.quiet);
                        }
                        continue;
                    }
                    Some(_) if verify => die_no_single_rev(&mut out, &mut err, options.quiet),
                    None if verify => die_no_single_rev(&mut out, &mut err, options.quiet),
                    Some(gix::revision::plumbing::Spec::Include(id)) => vec![(false, id, rev.clone())],
                    Some(gix::revision::plumbing::Spec::Exclude(id)) => {
                        vec![(true, id, rev.trim_start_matches('^').to_owned())]
                    }
                    Some(gix::revision::plumbing::Spec::Range { from, to }) => {
                        let (left, right) = rev.split_once("..").unwrap_or(("", ""));
                        let name = |side: &str| if side.is_empty() { "HEAD" } else { side }.to_owned();
                        vec![(false, to, name(right)), (true, from, name(left))]
                    }
                    Some(_) => die(
                        &mut out,
                        &mut err,
                        &format!(
                            "brit rev-parse: `{rev}` is not implemented; list commits with `brit revision list` or find bases with `brit merge-base`"
                        ),
                        128,
                    ),
                    None => {
                        writeln!(out, "{rev}").or_error()?;
                        if !cwd.join(&rev).exists() {
                            die(
                                &mut out,
                                &mut err,
                                &format!(
                                    "fatal: ambiguous argument '{rev}': unknown revision or path not in the working tree.\nUse '--' to separate paths from revisions, like this:\n'git <command> [<revision>...] -- [<file>...]'"
                                ),
                                128,
                            );
                        }
                        as_is = true;
                        continue;
                    }
                };
                for (reversed, id, name) in shown {
                    show(&repo, &mut out, &mut err, options, reversed, id, &name)?;
                }
            }
        }
    }
    if verify {
        match single {
            Some((reversed, id, name)) => show(&repo, &mut out, &mut err, options, reversed, id, &name)?,
            None => die_no_single_rev(&mut out, &mut err, options.quiet),
        }
    }
    Ok(())
}

fn show(
    repo: &gix::Repository,
    mut out: impl Write,
    mut err: impl Write,
    options: Options,
    reversed: bool,
    id: gix::ObjectId,
    name: &str,
) -> Result<()> {
    let marker = if reversed { "^" } else { "" };
    if options.abbrev_ref {
        // Git prints nothing for a revision that is not a ref, like `HEAD~1`.
        if let Some(full) = dwim_full_name(repo, name) {
            let short = shorten_unambiguous(&full, |candidate| ref_exists(repo, candidate));
            writeln!(out, "{marker}{short}").or_error()?;
        }
        return Ok(());
    }
    match options.short {
        Some(len) => {
            let prefix = match len {
                None => id.attach(repo).shorten()?,
                Some(len) => {
                    let len = len.clamp(4, id.kind().len_in_hex());
                    let candidate = gix::odb::store::prefix::disambiguate::Candidate::new(id, len).or_error()?;
                    match repo.objects.disambiguate_prefix(candidate)? {
                        Some(prefix) => prefix,
                        None => {
                            writeln!(err, "fatal: {id}: object not found").or_error()?;
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

fn die_no_single_rev(mut out: impl Write, mut err: impl Write, quiet: bool) -> ! {
    if quiet {
        die(&mut out, &mut err, "", 1)
    }
    die(&mut out, &mut err, "fatal: Needed a single revision", 128)
}

fn die(mut out: impl Write, mut err: impl Write, message: &str, code: i32) -> ! {
    out.flush().ok();
    if !message.is_empty() {
        writeln!(err, "{message}").ok();
    }
    err.flush().ok();
    std::process::exit(code)
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
/// an ancestor, and a plain relative path otherwise.
fn relative_path(path: &Path, base: &Path) -> String {
    let path: Vec<Component<'_>> = path.components().collect();
    let base: Vec<Component<'_>> = base.components().collect();
    let common = path.iter().zip(&base).take_while(|(a, b)| a == b).count();
    let ups = base.len() - common;
    let rest: PathBuf = path[common..].iter().collect();
    match (ups, rest.as_os_str().is_empty()) {
        (0, true) => "./".into(),
        (0, false) => rest.display().to_string(),
        (ups, true) => "../".repeat(ups),
        (ups, false) => format!("{}{}", "../".repeat(ups), rest.display()),
    }
}

#[cfg(test)]
mod tests {
    use super::relative_path;
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
}
