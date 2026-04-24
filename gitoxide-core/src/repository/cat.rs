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
    use std::io::BufRead;

    use super::{resolve_oid, Existence, PrintOutcome, TypedOutcome};
    use crate::repository::revision::resolve::TreeMode;

    /// Default `--batch-check` format (per `DEFAULT_FORMAT` in
    /// vendor/git/builtin/cat-file.c).
    const DEFAULT_BATCH_CHECK_FORMAT: &str = "%(objectname) %(objecttype) %(objectsize)";

    /// Expand `%(atom)` tokens in `format` using the given oid / kind / size.
    /// Unknown atoms are written verbatim (git's strbuf_expand_bad_format
    /// dies instead; we degrade gracefully since no parity row currently
    /// exercises unknown atoms).
    fn expand_atoms(format: &str, oid: &gix::hash::oid, kind: gix::object::Kind, size: u64, out: &mut Vec<u8>) {
        let bytes = format.as_bytes();
        let mut i = 0;
        while i < bytes.len() {
            if bytes[i..].starts_with(b"%(") {
                if let Some(end_off) = bytes[i + 2..].iter().position(|&b| b == b')') {
                    let atom = &bytes[i + 2..i + 2 + end_off];
                    match atom {
                        b"objectname" => out.extend_from_slice(oid.to_hex().to_string().as_bytes()),
                        b"objecttype" => out.extend_from_slice(kind.as_bytes()),
                        b"objectsize" => out.extend_from_slice(size.to_string().as_bytes()),
                        _ => out.extend_from_slice(&bytes[i..i + 2 + end_off + 1]),
                    }
                    i += 2 + end_off + 1;
                    continue;
                }
            }
            out.push(bytes[i]);
            i += 1;
        }
    }

    /// Which batch mode to run — whether per-input output carries just the
    /// info line (`--batch-check`) or info + `<contents>` (`--batch`).
    #[derive(Clone, Copy)]
    pub enum BatchMode {
        /// `--batch-check[=<format>]` — one info line per input.
        Info,
        /// `--batch[=<format>]` — info line + `<contents>` + delimiter per input.
        Contents,
    }

    /// `git cat-file --batch[=<fmt>]` / `--batch-check[=<fmt>]` — stdin-driven
    /// loop. Per input line, resolve the object and emit a formatted info
    /// line. Under `BatchMode::Contents`, an object body + trailing
    /// delimiter follows.
    ///
    /// Mirrors `batch_objects` + `batch_one_object` in
    /// vendor/git/builtin/cat-file.c, collapsing `BATCH_MODE_CONTENTS` and
    /// `BATCH_MODE_INFO` into one entrypoint. Per-input delimiters (`-z`
    /// for input NUL, `-Z` for input+output NUL) are threaded via the
    /// explicit `input_delim` / `output_delim` bytes — matching git's
    /// batch_options.input_delim / output_delim fields.
    ///
    /// Deferred status paths: `ambiguous` (short-sha resolving to multiple
    /// objects), `submodule` (gitlink tree entry), `dangling` / `loop` /
    /// `notdir` (only reached under --follow-symlinks). Every non-resolvable
    /// input currently collapses to `<input> missing`.
    pub fn batch(
        repo: &gix::Repository,
        mode: BatchMode,
        format: Option<&str>,
        input_delim: u8,
        output_delim: u8,
        mut stdin: impl BufRead,
        mut out: impl std::io::Write,
    ) -> anyhow::Result<()> {
        let format = format.unwrap_or(DEFAULT_BATCH_CHECK_FORMAT);
        let mut line = Vec::new();
        loop {
            line.clear();
            let n = stdin.read_until(input_delim, &mut line)?;
            if n == 0 {
                break;
            }
            // Strip the trailing delimiter and any CR (git's
            // strbuf_getdelim_strip_crlf trims both).
            if line.last() == Some(&input_delim) {
                line.pop();
            }
            if input_delim == b'\n' && line.last() == Some(&b'\r') {
                line.pop();
            }
            let input = std::str::from_utf8(&line)?;
            let Some(id) = resolve_oid(repo, input) else {
                write!(out, "{input} missing")?;
                out.write_all(std::slice::from_ref(&output_delim))?;
                continue;
            };
            let Ok(header) = repo.find_header(id) else {
                write!(out, "{input} missing")?;
                out.write_all(std::slice::from_ref(&output_delim))?;
                continue;
            };
            let mut buf = Vec::with_capacity(format.len() + 64);
            expand_atoms(format, &id, header.kind(), header.size(), &mut buf);
            out.write_all(&buf)?;
            out.write_all(std::slice::from_ref(&output_delim))?;
            if matches!(mode, BatchMode::Contents) {
                let object = repo.find_object(id)?;
                out.write_all(&object.data)?;
                out.write_all(std::slice::from_ref(&output_delim))?;
            }
        }
        Ok(())
    }

    /// Thin wrapper kept for callers that pre-date the delimiter/mode
    /// parameterization. New code should call `batch` directly.
    pub fn batch_check(
        repo: &gix::Repository,
        format: Option<&str>,
        stdin: impl BufRead,
        out: impl std::io::Write,
    ) -> anyhow::Result<()> {
        batch(repo, BatchMode::Info, format, b'\n', b'\n', stdin, out)
    }

    /// `git cat-file --batch[-check] --batch-all-objects` — enumerate every
    /// object in the odb (loose + packed + alternates) and emit the info
    /// (and optionally contents) for each. stdin is ignored.
    ///
    /// Ordering: by default git sorts by hash; gix's `.iter()` already yields
    /// "pack-lexicographical then loose-lexicographical", which for a
    /// repo with only loose objects (most fixture cases) is already
    /// hash-sorted. We additionally collect-sort-dedupe to match git's
    /// contract on mixed-storage repos. Under `unordered=true` we skip
    /// the sort+dedupe — effect-mode rows accept whatever order the
    /// backend yields.
    pub fn batch_all_objects(
        repo: &gix::Repository,
        mode: BatchMode,
        format: Option<&str>,
        unordered: bool,
        output_delim: u8,
        mut out: impl std::io::Write,
    ) -> anyhow::Result<()> {
        let format = format.unwrap_or(DEFAULT_BATCH_CHECK_FORMAT);
        let iter = repo.objects.iter()?;
        let ids: Vec<gix::hash::ObjectId> = if unordered {
            iter.filter_map(Result::ok).collect()
        } else {
            let mut v: Vec<_> = iter.filter_map(Result::ok).collect();
            v.sort();
            v.dedup();
            v
        };
        for id in ids {
            let Ok(header) = repo.find_header(id) else {
                continue;
            };
            let mut buf = Vec::with_capacity(format.len() + 64);
            expand_atoms(format, &id, header.kind(), header.size(), &mut buf);
            out.write_all(&buf)?;
            out.write_all(std::slice::from_ref(&output_delim))?;
            if matches!(mode, BatchMode::Contents) {
                let object = repo.find_object(id)?;
                out.write_all(&object.data)?;
                out.write_all(std::slice::from_ref(&output_delim))?;
            }
        }
        Ok(())
    }

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
    /// When `use_mailmap` is set and the object is a commit or tag, the
    /// size is recomputed after applying `.mailmap` ident rewrites to
    /// the author/committer/tagger headers — matching git's
    /// `replace_idents_using_mailmap` path. Blobs and trees are
    /// unaffected (git's mailmap only touches headers).
    pub fn print_size(
        repo: &gix::Repository,
        revspec: &str,
        use_mailmap: bool,
        mut out: impl std::io::Write,
    ) -> anyhow::Result<PrintOutcome> {
        let Some(id) = resolve_oid(repo, revspec) else {
            return Ok(PrintOutcome::InvalidSpec);
        };

        // Fast path: no mailmap → header-only lookup (cheaper, no full
        // object read).
        if !use_mailmap {
            return match repo.find_header(id) {
                Ok(header) => {
                    writeln!(out, "{}", header.size())?;
                    Ok(PrintOutcome::Ok)
                }
                Err(_) => Ok(PrintOutcome::MissingObject),
            };
        }

        // Mailmap path: commits / tags get their author/committer/tagger
        // idents rewritten, so the size can shrink or grow. Read full
        // bytes, re-encode after rewrite, measure.
        let Ok(object) = repo.find_object(id) else {
            return Ok(PrintOutcome::MissingObject);
        };
        let size = match object.kind {
            gix::object::Kind::Commit => {
                let snapshot = repo.open_mailmap();
                let commit_ref = gix::objs::CommitRef::from_bytes(&object.data)?;
                let mut commit = gix::objs::Commit::try_from(commit_ref)?;
                let mut buf = gix::date::parse::TimeBuf::default();
                commit.author = snapshot.resolve_cow(commit.author.to_ref(&mut buf)).into();
                let mut buf = gix::date::parse::TimeBuf::default();
                commit.committer = snapshot.resolve_cow(commit.committer.to_ref(&mut buf)).into();
                let mut encoded = Vec::with_capacity(object.data.len());
                gix::objs::WriteTo::write_to(&commit, &mut encoded)?;
                encoded.len() as u64
            }
            gix::object::Kind::Tag => {
                let snapshot = repo.open_mailmap();
                let tag_ref = gix::objs::TagRef::from_bytes(&object.data)?;
                let mut tag = gix::objs::Tag::try_from(tag_ref)?;
                if let Some(tagger) = tag.tagger.take() {
                    let mut buf = gix::date::parse::TimeBuf::default();
                    tag.tagger = Some(snapshot.resolve_cow(tagger.to_ref(&mut buf)).into());
                }
                let mut encoded = Vec::with_capacity(object.data.len());
                gix::objs::WriteTo::write_to(&tag, &mut encoded)?;
                encoded.len() as u64
            }
            _ => object.data.len() as u64,
        };
        writeln!(out, "{size}")?;
        Ok(PrintOutcome::Ok)
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
