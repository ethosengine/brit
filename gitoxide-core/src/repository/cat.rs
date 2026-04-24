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
            // Match `git cat-file -p <tree>` (which delegates to `git ls-tree
            // <tree>` in cat-file.c case 'p' OBJ_TREE): one line per entry,
            //   `<mode:06o> SP <type> SP <full-hex-oid> TAB <name> LF`
            // where `<type>` is `blob` for regular + executable blobs AND
            // symlinks (the *object* type), `tree` for subtrees, `commit`
            // for submodules (gitlinks). gix's EntryKind::as_str emits
            // "exe"/"link" for executable-blob/symlink which git never
            // uses — it only shows the *object* type here, not the mode
            // classification.
            for entry in id.object()?.into_tree().iter() {
                let entry = entry?;
                let type_name = match entry.mode().kind() {
                    gix::object::tree::EntryKind::Tree => "tree",
                    gix::object::tree::EntryKind::Commit => "commit",
                    _ => "blob",
                };
                writeln!(
                    out,
                    "{mode:06o} {type_name} {oid}\t{name}",
                    mode = entry.mode().value(),
                    oid = entry.oid().to_hex(),
                    name = entry.filename(),
                )?;
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

/// Outcome of the `-t` / `-s` / `-p` (non-existence) query modes. Dispatch
/// maps variants to exit codes + git's exact fatal wording:
///   * `Ok`              → exit 0, content already written to stdout
///   * `InvalidSpec`     → exit 128, stderr `fatal: Not a valid object name <spec>`
///   * `MissingObject`   → exit 128, stderr `fatal: git cat-file: could not get object info`
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrintOutcome {
    Ok,
    InvalidSpec,
    MissingObject,
}

/// Outcome of the positional `<type> <object>` form. Dispatch maps variants:
///
/// * `Ok` — exit 0, raw object bytes written to stdout.
/// * `InvalidType` — exit 128, stderr `fatal: invalid object type "<type>"`.
/// * `InvalidSpec` — exit 128, stderr `fatal: Not a valid object name <spec>`.
/// * `BadFile` — exit 128, stderr `fatal: git cat-file <spec>: bad file`.
///   Fires when the object is absent from the odb OR present but can't
///   peel to the requested type — git's case 0 collapses both to the same
///   `die("git cat-file %s: bad file", obj_name)` wording.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TypedOutcome {
    Ok,
    InvalidType,
    InvalidSpec,
    BadFile,
}

/// Mirror git's `get_oid_with_context(..., GET_OID_HASH_ANY, ...)` contract:
/// accept a full-length hex oid as a literal identifier *without* requiring
/// the object to be present, and fall back to full revspec resolution for
/// everything else. Returns `None` when the spec is neither — dispatch
/// reports that as `InvalidSpec`.
fn resolve_oid(repo: &gix::Repository, revspec: &str) -> Option<gix::hash::ObjectId> {
    if let Ok(id) = gix::hash::ObjectId::from_hex(revspec.as_bytes()) {
        return Some(id);
    }
    repo.rev_parse(revspec).ok()?.single().map(gix::Id::detach)
}

pub(super) mod function {
    use super::{resolve_oid, Existence, PrintOutcome, TypedOutcome};
    use crate::repository::revision::resolve::TreeMode;

    pub fn cat(repo: gix::Repository, revspec: &str, out: impl std::io::Write) -> anyhow::Result<()> {
        super::display_object(&repo, repo.rev_parse(revspec)?, TreeMode::Pretty, None, out)?;
        Ok(())
    }

    /// `git cat-file -t <revspec>` — write the object's type name
    /// (one of `blob`, `tree`, `commit`, `tag`) followed by a newline.
    ///
    /// Mirrors `case 't'` in cat_one_file (vendor/git/builtin/cat-file.c):
    /// `odb_read_object_info_extended` → `type_name(type)` →
    /// `printf("%s\n", ...)`. Two failure paths:
    ///   * spec does not resolve (and is not a literal full-hex oid)
    ///     → `InvalidSpec` → fatal `Not a valid object name <spec>`
    ///   * spec resolved / literal oid accepted, but odb has no such
    ///     object → `MissingObject` → fatal `git cat-file: could not
    ///     get object info`
    pub fn print_type(
        repo: &gix::Repository,
        revspec: &str,
        mut out: impl std::io::Write,
    ) -> anyhow::Result<PrintOutcome> {
        let Some(id) = resolve_oid(repo, revspec) else {
            return Ok(PrintOutcome::InvalidSpec);
        };
        match repo.find_header(id) {
            Ok(header) => {
                writeln!(out, "{}", header.kind())?;
                Ok(PrintOutcome::Ok)
            }
            Err(_) => Ok(PrintOutcome::MissingObject),
        }
    }

    /// `git cat-file <type> <revspec>` — assert the object at `revspec`
    /// can be peeled to `<type>` (blob/tree/commit/tag), then write its
    /// raw bytes (uncompressed, the canonical loose-object body).
    ///
    /// Tree output here is **binary** (sequence of `<mode> SP <name> NUL
    /// <20-byte-oid>`), not the pretty `ls-tree` format produced by -p.
    /// Commit / tag / blob outputs are identical to -p because their
    /// "raw" and "pretty" forms coincide.
    ///
    /// Mirrors `case 0` in cat_one_file (vendor/git/builtin/cat-file.c):
    /// `odb_read_object_peeled` with the caller-supplied type hint. Tags
    /// deref to their targets, commits deref to their trees — so
    /// `cat-file tree <commit>` and `cat-file commit <tag>` succeed even
    /// though the stored type differs from the requested one.
    pub fn cat_typed(
        repo: &gix::Repository,
        type_str: &str,
        revspec: &str,
        mut out: impl std::io::Write,
    ) -> anyhow::Result<TypedOutcome> {
        let Ok(kind) = gix::object::Kind::from_bytes(type_str.as_bytes()) else {
            return Ok(TypedOutcome::InvalidType);
        };
        let Some(id) = resolve_oid(repo, revspec) else {
            return Ok(TypedOutcome::InvalidSpec);
        };
        let Ok(object) = repo.find_object(id) else {
            return Ok(TypedOutcome::BadFile);
        };
        let Ok(peeled) = object.peel_to_kind(kind) else {
            return Ok(TypedOutcome::BadFile);
        };
        out.write_all(&peeled.data)?;
        Ok(TypedOutcome::Ok)
    }

    /// `git cat-file -s <revspec>` — write the object's size in bytes
    /// (decimal, followed by a newline).
    ///
    /// Mirrors `case 's'` in cat_one_file (vendor/git/builtin/cat-file.c):
    /// `odb_read_object_info_extended` → `printf("%"PRIuMAX"\n", size)`.
    /// Same two failure paths as -t: invalid spec / missing object.
    ///
    /// Note: git's `--use-mailmap -s` rewrites the size to reflect the
    /// mailmap-replaced identities. That path is deferred to the
    /// --use-mailmap iteration.
    pub fn print_size(
        repo: &gix::Repository,
        revspec: &str,
        mut out: impl std::io::Write,
    ) -> anyhow::Result<PrintOutcome> {
        let Some(id) = resolve_oid(repo, revspec) else {
            return Ok(PrintOutcome::InvalidSpec);
        };
        match repo.find_header(id) {
            Ok(header) => {
                writeln!(out, "{}", header.size())?;
                Ok(PrintOutcome::Ok)
            }
            Err(_) => Ok(PrintOutcome::MissingObject),
        }
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
        match resolve_oid(repo, revspec) {
            Some(id) if repo.has_object(id) => Existence::Present,
            Some(_) => Existence::Absent,
            None => Existence::InvalidSpec,
        }
    }
}
