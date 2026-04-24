use anyhow::{anyhow, Context};
use gix::{diff::blob::ResourceKind, filter::plumbing::driver::apply::Delay, revision::Spec};

use crate::repository::revision::resolve::{BlobFormat, TreeMode};

pub fn display_object(
    repo: &gix::Repository,
    spec: Spec<'_>,
    tree_mode: TreeMode,
    cache: Option<(BlobFormat, &mut gix::diff::blob::Platform)>,
    mut out: impl std::io::Write,
) -> anyhow::Result<()> {
    let id = spec.single().context("rev-spec must resolve to a single object")?;
    let header = id.header()?;
    match header.kind() {
        gix::object::Kind::Tree if matches!(tree_mode, TreeMode::Pretty) => {
            for entry in id.object()?.into_tree().iter() {
                writeln!(out, "{}", entry?)?;
            }
        }
        gix::object::Kind::Blob if cache.is_some() && spec.path_and_mode().is_some() => {
            let (path, mode) = spec.path_and_mode().expect("is present");
            match cache.expect("is some") {
                (BlobFormat::Git, _) => unreachable!("no need for a cache when querying object db"),
                (BlobFormat::Worktree, cache) => {
                    let platform = cache.attr_stack.at_entry(path, Some(mode.into()), &repo.objects)?;
                    let object = id.object()?;
                    let mut converted = cache.filter.worktree_filter.convert_to_worktree(
                        &object.data,
                        path,
                        &mut |_path, attrs| {
                            let _ = platform.matching_attributes(attrs);
                        },
                        Delay::Forbid,
                    )?;
                    std::io::copy(&mut converted, &mut out)?;
                }
                (BlobFormat::Diff | BlobFormat::DiffOrGit, cache) => {
                    cache.set_resource(id.detach(), mode.kind(), path, ResourceKind::OldOrSource, &repo.objects)?;
                    let resource = cache.resource(ResourceKind::OldOrSource).expect("just set");
                    let data = resource
                        .data
                        .as_slice()
                        .ok_or_else(|| anyhow!("Binary data at {path} cannot be diffed"))?;
                    out.write_all(data)?;
                }
            }
        }
        _ => out.write_all(&id.object()?.data)?,
    }
    Ok(())
}

/// Outcome of `git cat-file -e <revspec>`. Dispatch translates each variant
/// to an exit code (0, 1, 128) and — for `InvalidSpec` — to git's exact
/// `fatal: Not a valid object name <spec>` stderr line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Existence {
    /// Spec resolved, object present in odb — `git cat-file -e` exit 0.
    Present,
    /// Spec resolved (or was a well-formed literal oid) but the object is
    /// absent — `git cat-file -e` exit 1, no stderr.
    Absent,
    /// Spec did not resolve to any oid — `git cat-file -e` exit 128,
    /// stderr `fatal: Not a valid object name <spec>`.
    InvalidSpec,
}

pub(super) mod function {
    use super::Existence;
    use crate::repository::revision::resolve::TreeMode;

    pub fn cat(repo: gix::Repository, revspec: &str, out: impl std::io::Write) -> anyhow::Result<()> {
        super::display_object(&repo, repo.rev_parse(revspec)?, TreeMode::Pretty, None, out)?;
        Ok(())
    }

    /// `git cat-file -e <revspec>` — report whether the object exists.
    ///
    /// Mirrors git's `case 'e'` branch in cat_one_file
    /// (vendor/git/builtin/cat-file.c) combined with the upstream
    /// `get_oid_with_context` parse contract: a well-formed full-length
    /// hex oid is accepted without an odb lookup, and `odb_has_object`
    /// decides existence. Anything else goes through revspec resolution
    /// and, if that fails, is reported as `InvalidSpec` so the caller
    /// can emit git's `fatal: Not a valid object name <spec>` line and
    /// exit 128.
    pub fn exists(repo: &gix::Repository, revspec: &str) -> Existence {
        if let Ok(id) = gix::hash::ObjectId::from_hex(revspec.as_bytes()) {
            return if repo.has_object(id) {
                Existence::Present
            } else {
                Existence::Absent
            };
        }
        match repo.rev_parse(revspec) {
            Ok(spec) => match spec.single() {
                Some(id) if repo.has_object(id) => Existence::Present,
                Some(_) => Existence::Absent,
                None => Existence::InvalidSpec,
            },
            Err(_) => Existence::InvalidSpec,
        }
    }
}
