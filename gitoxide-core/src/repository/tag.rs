use std::{collections::HashSet, ffi::OsString};

use gix::{
    bstr::{BString, ByteSlice},
    prelude::ObjectIdExt,
};

use crate::OutputFormat;

/// Options driving `tag::list`. Mirrors the filter-and-format subset
/// of git-tag's listing mode from vendor/git/builtin/tag.c cmd_tag.
#[derive(Debug, Default)]
pub struct ListOptions {
    /// Shell-glob patterns (fnmatch(3), OR'd). Empty = match everything.
    pub patterns: Vec<OsString>,
    /// When true, pattern-match and sort are case-insensitive
    /// (`-i`/`--ignore-case`).
    pub ignore_case: bool,
    /// When `Some`, only list tags that point (after peeling tag
    /// chains) at the resolved object. Git's `--points-at <object>`;
    /// `None` means "no filter".
    pub points_at: Option<OsString>,
    /// When `Some`, only keep tags whose tagged commit has the
    /// resolved commit in its ancestry. Git's `--contains <commit>`.
    pub contains: Option<OsString>,
    /// When `Some`, only keep tags whose tagged commit does NOT have
    /// the resolved commit in its ancestry. Git's `--no-contains`.
    pub no_contains: Option<OsString>,
    /// When `Some`, only keep tags whose tagged commit is reachable
    /// from the resolved commit. Git's `--merged <commit>`.
    pub merged: Option<OsString>,
    /// When `Some`, only keep tags whose tagged commit is NOT
    /// reachable from the resolved commit. Git's `--no-merged <commit>`.
    pub no_merged: Option<OsString>,
}

/// `git tag -d <name>...` in effect mode: for each name, try to
/// remove `refs/tags/<name>`. Missing names emit the exact git error
/// phrasing on stderr and contribute to a final non-zero exit; the
/// "Deleted tag ..." success line is emitted on stdout for parity
/// with git (callers of `expect_parity effect` don't compare bytes).
///
/// On any failure the function flushes its streams and calls
/// `std::process::exit(1)` — matches the direct `exit(1)` pattern
/// used for in-process error paths elsewhere in the gix plumbing
/// (e.g. cat-file's -e missing-object), avoiding an anyhow
/// backtrace from escaping through `prepare_and_run`.
pub fn delete(
    repo: gix::Repository,
    names: Vec<OsString>,
    out: &mut dyn std::io::Write,
    err: &mut dyn std::io::Write,
) -> anyhow::Result<()> {
    if names.is_empty() {
        writeln!(err, "error: tag name required to delete a tag")?;
        out.flush().ok();
        err.flush().ok();
        std::process::exit(129);
    }
    let mut had_error = false;
    for name in &names {
        let short_name = gix::path::os_str_into_bstr(name)?;
        let full_name = format!("refs/tags/{}", short_name.to_str_lossy());
        let reference = match repo.find_reference(full_name.as_str()) {
            Ok(r) => r,
            Err(_) => {
                writeln!(err, "error: tag '{}' not found.", short_name.to_str_lossy())?;
                had_error = true;
                continue;
            }
        };
        let target_short = match reference.inner.target.clone() {
            gix::refs::Target::Object(oid) => oid.attach(&repo).shorten().map(|s| s.to_string()).unwrap_or_default(),
            gix::refs::Target::Symbolic(_) => String::new(),
        };
        if let Err(error) = reference.delete() {
            writeln!(
                err,
                "error: could not delete tag '{}': {}",
                short_name.to_str_lossy(),
                error
            )?;
            had_error = true;
            continue;
        }
        writeln!(
            out,
            "Deleted tag '{}' (was {})",
            short_name.to_str_lossy(),
            target_short
        )?;
    }
    if had_error {
        out.flush().ok();
        err.flush().ok();
        std::process::exit(1);
    }
    Ok(())
}

/// `git tag -v <name>...` in effect mode: for each name, locate the
/// `refs/tags/<name>` ref and inspect the tagged object. Lightweight
/// refs (target points directly at a non-tag) → die 128 with
/// `error: <name>: cannot verify a non-tag object of type <T>.`.
/// Annotated tags without an embedded `-----BEGIN PGP SIGNATURE-----`
/// block → die 1 with `error: no signature found`. Actual GPG
/// signature verification requires a signing backend and is tracked
/// as a shortcoming; this implementation only covers the two error
/// paths that don't depend on a signer.
pub fn verify(
    repo: gix::Repository,
    names: Vec<OsString>,
    _out: &mut dyn std::io::Write,
    err: &mut dyn std::io::Write,
) -> anyhow::Result<()> {
    if names.is_empty() {
        writeln!(err, "error: tag name required to verify a tag")?;
        anyhow::bail!("no tag names given");
    }
    let mut had_error = false;
    for name in &names {
        let short_name = gix::path::os_str_into_bstr(name)?;
        let full_name = format!("refs/tags/{}", short_name.to_str_lossy());
        let reference = match repo.find_reference(full_name.as_str()) {
            Ok(r) => r,
            Err(_) => {
                writeln!(err, "error: tag '{}' not found.", short_name.to_str_lossy())?;
                had_error = true;
                continue;
            }
        };
        // Immediate target (NOT peeled): for annotated tags this is
        // the tag object oid; for lightweight tags it's the commit
        // (or whatever non-tag object) the ref points at directly.
        let direct_id = match reference.inner.target.clone() {
            gix::refs::Target::Object(oid) => oid,
            gix::refs::Target::Symbolic(_) => {
                had_error = true;
                continue;
            }
        };
        let header = match repo.find_header(direct_id) {
            Ok(h) => h,
            Err(error) => {
                writeln!(err, "error: cannot read tag '{}': {}", short_name.to_str_lossy(), error)?;
                had_error = true;
                continue;
            }
        };
        if header.kind() != gix::object::Kind::Tag {
            // Lightweight tag — direct target is not a tag object.
            // git prints "error: <name>: cannot verify a non-tag
            // object of type <T>." and exits 1.
            writeln!(
                err,
                "error: {}: cannot verify a non-tag object of type {}.",
                short_name.to_str_lossy(),
                header.kind()
            )?;
            had_error = true;
            continue;
        }
        let tag_object = match repo.find_object(direct_id) {
            Ok(obj) => obj,
            Err(error) => {
                writeln!(err, "error: cannot read tag '{}': {}", short_name.to_str_lossy(), error)?;
                had_error = true;
                continue;
            }
        };
        let decoded = match tag_object.try_to_tag_ref() {
            Ok(d) => d,
            Err(error) => {
                writeln!(
                    err,
                    "error: cannot decode tag '{}': {}",
                    short_name.to_str_lossy(),
                    error
                )?;
                had_error = true;
                continue;
            }
        };
        if decoded.pgp_signature.is_none() {
            writeln!(err, "error: no signature found")?;
            had_error = true;
            continue;
        }
        writeln!(
            err,
            "error: tag signature verification requires a signing backend (not yet implemented)"
        )?;
        had_error = true;
    }
    if had_error {
        _out.flush().ok();
        err.flush().ok();
        std::process::exit(1);
    }
    Ok(())
}

/// Return `true` iff `haystack_commit` has `needle` anywhere in its
/// ancestry (walking parents). Inclusive — a commit contains itself.
fn commit_contains(
    repo: &gix::Repository,
    haystack_commit: gix::ObjectId,
    needle: gix::ObjectId,
) -> anyhow::Result<bool> {
    if haystack_commit == needle {
        return Ok(true);
    }
    for res in haystack_commit.attach(repo).ancestors().all()? {
        if res?.id == needle {
            return Ok(true);
        }
    }
    Ok(false)
}

fn ancestors_of(repo: &gix::Repository, rev: &OsString) -> anyhow::Result<HashSet<gix::ObjectId>> {
    let rev_bstr = gix::path::os_str_into_bstr(rev)?;
    let oid = repo.rev_parse_single(rev_bstr)?.detach();
    let mut set = HashSet::new();
    set.insert(oid);
    for res in oid.attach(repo).ancestors().all()? {
        let info = res?;
        set.insert(info.id);
    }
    Ok(set)
}

/// git-compat `tag` listing: one shortened refname per line, sorted
/// lexicographically by refname. Matches git's default format
/// `%(refname:strip=2)` from `git tag` / `git tag -l` with no
/// `--format` override (see vendor/git/builtin/tag.c list_tags and
/// vendor/git/Documentation/git-tag.adoc OPTIONS/`--format`).
///
/// `opts.patterns` are fnmatch(3)-style shell globs; a ref is shown
/// if its shortened name matches any pattern (OR), or unconditionally
/// when the list is empty. Matches the `[<pattern>...]` positional of
/// `git tag -l`.
pub fn list(
    repo: gix::Repository,
    out: &mut dyn std::io::Write,
    format: OutputFormat,
    opts: ListOptions,
) -> anyhow::Result<()> {
    if format != OutputFormat::Human {
        anyhow::bail!("JSON output isn't supported");
    }

    let platform = repo.references()?;

    // Resolve --points-at once up front to an ObjectId. The filter
    // then keeps tags whose peeled target equals this oid.
    let points_at_oid: Option<gix::ObjectId> = match opts.points_at.as_deref() {
        Some(rev) => {
            let rev_bstr = gix::path::os_str_into_bstr(rev)?;
            Some(repo.rev_parse_single(rev_bstr)?.detach())
        }
        None => None,
    };

    // Precompute ancestor sets for --merged / --no-merged. Each is the
    // closure-of-ancestors of the resolved commit, including the commit
    // itself. A tag is kept iff its peeled commit is/is not in the set.
    let merged_set = opts.merged.as_ref().map(|rev| ancestors_of(&repo, rev)).transpose()?;
    let no_merged_set = opts
        .no_merged
        .as_ref()
        .map(|rev| ancestors_of(&repo, rev))
        .transpose()?;

    // --contains / --no-contains resolve once; the per-tag walk runs
    // inside the filter_map below (each tag_commit gets its own
    // ancestry walk until the needle is found or exhausted).
    let contains_oid: Option<gix::ObjectId> = match opts.contains.as_deref() {
        Some(rev) => Some(repo.rev_parse_single(gix::path::os_str_into_bstr(rev)?)?.detach()),
        None => None,
    };
    let no_contains_oid: Option<gix::ObjectId> = match opts.no_contains.as_deref() {
        Some(rev) => Some(repo.rev_parse_single(gix::path::os_str_into_bstr(rev)?)?.detach()),
        None => None,
    };

    let need_peel = points_at_oid.is_some()
        || merged_set.is_some()
        || no_merged_set.is_some()
        || contains_oid.is_some()
        || no_contains_oid.is_some();

    let mut names: Vec<BString> = platform
        .tags()?
        .flatten()
        .filter_map(|mut reference| {
            let peeled = if need_peel {
                Some(reference.peel_to_id().ok()?.detach())
            } else {
                None
            };

            if let (Some(target_oid), Some(peeled)) = (points_at_oid, peeled) {
                if peeled != target_oid {
                    return None;
                }
            }
            if let (Some(set), Some(peeled)) = (merged_set.as_ref(), peeled) {
                if !set.contains(&peeled) {
                    return None;
                }
            }
            if let (Some(set), Some(peeled)) = (no_merged_set.as_ref(), peeled) {
                if set.contains(&peeled) {
                    return None;
                }
            }
            if let (Some(needle), Some(peeled)) = (contains_oid, peeled) {
                if !commit_contains(&repo, peeled, needle).ok()? {
                    return None;
                }
            }
            if let (Some(needle), Some(peeled)) = (no_contains_oid, peeled) {
                if commit_contains(&repo, peeled, needle).ok()? {
                    return None;
                }
            }

            Some(reference.name().shorten().to_owned())
        })
        .collect();

    if opts.ignore_case {
        names.sort_by_key(|a| a.to_ascii_lowercase());
    } else {
        names.sort();
    }

    let patterns: Vec<BString> = opts
        .patterns
        .iter()
        .map(|p| gix::path::os_str_into_bstr(p).map(BString::from))
        .map(|res| {
            res.map(|bs| {
                if opts.ignore_case {
                    bs.to_ascii_lowercase().into()
                } else {
                    bs
                }
            })
        })
        .collect::<Result<_, _>>()?;

    let wildmatch_mode = if opts.ignore_case {
        gix::glob::wildmatch::Mode::IGNORE_CASE
    } else {
        gix::glob::wildmatch::Mode::empty()
    };

    for name in &names {
        if !patterns.is_empty()
            && !patterns
                .iter()
                .any(|pat| gix::glob::wildmatch(pat.as_ref(), name.as_ref(), wildmatch_mode))
        {
            continue;
        }
        writeln!(out, "{name}", name = name.to_str_lossy())?;
    }

    Ok(())
}
