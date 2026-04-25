use anyhow::Context;
use gix::{
    bstr::{BString, ByteSlice},
    objs::tree::EntryMode,
    odb::store::RefreshMode,
    prelude::ObjectIdExt,
    ObjectId,
};

/// Detects two-dot (`A..B`) and three-dot (`A...B`) range syntax in a
/// single positional argument. Returns `(left, right, three_dot)` —
/// empty `left` / `right` is the implicit-HEAD form (`..B` /
/// `A..`). Returns `None` for non-range tokens.
///
/// Skips `..` occurrences that look like path traversal (`./..`,
/// `../foo`) — those are pathspecs, not ranges. git's
/// revision.c::handle_dotdot_1 leans on the same heuristic.
fn parse_range(arg: &[u8]) -> Option<(&[u8], &[u8], bool)> {
    // `..` cannot appear at offset 0 if the token starts with `.` or `/`
    // (would be a path).
    if arg.starts_with(b".") || arg.starts_with(b"/") {
        return None;
    }
    // Search for the first `..`; if followed by a third `.`, it's a
    // three-dot range; otherwise two-dot.
    for i in 0..arg.len().saturating_sub(1) {
        if arg[i] == b'.' && arg[i + 1] == b'.' {
            let three_dot = arg.get(i + 2) == Some(&b'.');
            let sep_len = if three_dot { 3 } else { 2 };
            return Some((&arg[..i], &arg[i + sep_len..], three_dot));
        }
    }
    None
}

/// Porcelain `git diff` entry point — routes the bare-form invocation
/// (no subcommand) by positional-arg count, matching cmd_diff's
/// classifier in vendor/git/builtin/diff.c.
///
/// Routing:
///   0 args → `worktree_index` (diff-files: worktree vs index);
///   1 arg  → worktree vs `<commit>` (diff-index path) — currently a
///            stub that resolves the revspec and emits a placeholder
///            note; bytes parity deferred until the patch renderer
///            lands;
///   2 args → tree-vs-tree, delegating to the existing `tree` helper.
pub fn porcelain(
    repo: gix::Repository,
    out: &mut dyn std::io::Write,
    progress: impl gix::NestedProgress + 'static,
    args: Vec<BString>,
    paths: Vec<BString>,
) -> anyhow::Result<()> {
    if !paths.is_empty() {
        // Pathspec filtering is not yet wired into any of the diff
        // helpers; emit a placeholder note so callers see the paths
        // were recognized, then continue without filtering. Bytes
        // parity (filtered patch output) is deferred via the row's
        // compat_effect marker until path-filtering lands.
        use std::io::Write;
        let mut stderr = std::io::stderr().lock();
        let _ = writeln!(
            stderr,
            "[gix-diff] pathspec filter for {} path(s) recognized — filtering not yet implemented",
            paths.len()
        );
    }
    // Detect range syntax in 1-arg form: `A..B` (two-dot) is shorthand
    // for `A B`; `A...B` (three-dot) is shorthand for `merge-base(A,B) B`.
    // Empty endpoints default to HEAD: `..B` → `HEAD B`, `A..` → `A HEAD`.
    // Mirrors git's revision.c handle_dotdot_1 (see gitrevisions(7)).
    if args.len() == 1 {
        if let Some((left, right, three_dot)) = parse_range(args[0].as_slice()) {
            let l: BString = if left.is_empty() { "HEAD".into() } else { left.into() };
            let r: BString = if right.is_empty() { "HEAD".into() } else { right.into() };
            let new_args = if three_dot {
                let l_id = match repo.rev_parse_single(l.as_bstr()) {
                    Ok(id) => id,
                    Err(_) => {
                        eprintln!(
                            "fatal: ambiguous argument '{l}': unknown revision or path not in the working tree.\n\
                             Use '--' to separate paths from revisions, like this:\n\
                             'git <command> [<revision>...] -- [<file>...]'"
                        );
                        std::process::exit(128);
                    }
                };
                let r_id = match repo.rev_parse_single(r.as_bstr()) {
                    Ok(id) => id,
                    Err(_) => {
                        eprintln!(
                            "fatal: ambiguous argument '{r}': unknown revision or path not in the working tree.\n\
                             Use '--' to separate paths from revisions, like this:\n\
                             'git <command> [<revision>...] -- [<file>...]'"
                        );
                        std::process::exit(128);
                    }
                };
                let mb = repo
                    .merge_base(l_id, r_id)
                    .with_context(|| format!("could not compute merge-base for {l}...{r}"))?;
                vec![mb.to_string().into(), r]
            } else {
                vec![l, r]
            };
            return porcelain(repo, out, progress, new_args, paths);
        }
    }
    match args.len() {
        0 => worktree_index(repo, out, progress),
        1 => {
            let revspec = args.into_iter().next().context("missing revspec")?;
            let id = match repo.rev_parse_single(revspec.as_bstr()) {
                Ok(id) => id,
                Err(_) => {
                    // Parity with git's setup_revisions: unknown revision/path
                    // dies 128 (vendor/git/revision.c::handle_revision_arg →
                    // die). Wording is git's exact 3-line stanza.
                    eprintln!(
                        "fatal: ambiguous argument '{revspec}': unknown revision or path not in the working tree.\n\
                         Use '--' to separate paths from revisions, like this:\n\
                         'git <command> [<revision>...] -- [<file>...]'"
                    );
                    std::process::exit(128);
                }
            };
            use std::io::Write;
            let mut stderr = std::io::stderr().lock();
            let _ = writeln!(
                stderr,
                "[gix-diff] worktree vs `{revspec}` ({}) — patch output not yet implemented",
                id.shorten_or_id()
            );
            Ok(())
        }
        2 => {
            let mut it = args.into_iter();
            let old_treeish = it.next().context("missing old revspec")?;
            let new_treeish = it.next().context("missing new revspec")?;
            // Same exact-stanza ambiguous-arg parity as the 1-arg path.
            let mut ids = Vec::with_capacity(2);
            for spec in [&old_treeish, &new_treeish] {
                match repo.rev_parse_single(spec.as_bstr()) {
                    Ok(id) => ids.push(id),
                    Err(_) => {
                        eprintln!(
                            "fatal: ambiguous argument '{spec}': unknown revision or path not in the working tree.\n\
                             Use '--' to separate paths from revisions, like this:\n\
                             'git <command> [<revision>...] -- [<file>...]'"
                        );
                        std::process::exit(128);
                    }
                }
            }
            // Detect blob-vs-blob form (vendor/git/builtin/diff.c
            // builtin_diff_blobs): if both args resolve to blob objects
            // directly, emit a placeholder note and exit 0. Mixed
            // blob/tree is rejected by git too.
            let kinds: Vec<gix::object::Kind> = ids
                .iter()
                .map(|id| repo.find_object(*id).map(|o| o.kind))
                .collect::<Result<Vec<_>, _>>()?;
            if kinds.iter().all(|k| *k == gix::object::Kind::Blob) {
                use std::io::Write;
                let mut stderr = std::io::stderr().lock();
                let _ = writeln!(
                    stderr,
                    "[gix-diff] blob {} vs blob {} — patch output not yet implemented",
                    ids[0].shorten_or_id(),
                    ids[1].shorten_or_id()
                );
                return Ok(());
            }
            tree(repo, out, old_treeish, new_treeish)
        }
        n => anyhow::bail!("`gix diff` with {n} positional args not yet implemented"),
    }
}

/// Bare `git diff` (no subcommand, no revspec): worktree vs index.
///
/// The git C path runs `run_diff_files` over the index-vs-worktree
/// comparison and renders patches. gix has no patch renderer wired
/// yet — this helper walks the status iterator and exits 0
/// regardless: with no tracked changes nothing is emitted (matching
/// git's clean-tree behavior); with tracked changes a single
/// placeholder line is emitted to stderr listing the modified-file
/// count, and the patch body is suppressed until a renderer lands.
/// Untracked-file emission is suppressed (untracked files do not
/// appear in `git diff` output, only in `git status`).
pub fn worktree_index(
    repo: gix::Repository,
    _out: &mut dyn std::io::Write,
    progress: impl gix::NestedProgress + 'static,
) -> anyhow::Result<()> {
    use gix::status::{self, index_worktree};
    let iter = repo
        .status(progress)?
        .should_interrupt_shared(&gix::interrupt::IS_INTERRUPTED)
        .into_iter(None)?;

    let mut tracked_changes = 0usize;
    for item in iter {
        let item = item?;
        match item {
            status::Item::TreeIndex(_) => {
                // index-vs-HEAD differences (i.e. staged changes) are not
                // visible in bare `git diff`; they belong to `--cached`.
                // Suppress them in the bare-form check.
            }
            status::Item::IndexWorktree(
                index_worktree::Item::Modification { .. } | index_worktree::Item::Rewrite { .. },
            ) => {
                tracked_changes += 1;
            }
            status::Item::IndexWorktree(index_worktree::Item::DirectoryContents { .. }) => {
                // Untracked / ignored — skipped: `git diff` ignores these.
            }
        }
    }

    if tracked_changes != 0 {
        use std::io::Write;
        let mut stderr = std::io::stderr().lock();
        let _ = writeln!(
            stderr,
            "[gix-diff] {tracked_changes} tracked file(s) modified — patch output not yet implemented"
        );
    }
    Ok(())
}

pub fn tree(
    mut repo: gix::Repository,
    out: &mut dyn std::io::Write,
    old_treeish: BString,
    new_treeish: BString,
) -> anyhow::Result<()> {
    repo.object_cache_size_if_unset(repo.compute_object_cache_size_for_tree_diffs(&**repo.index_or_empty()?));
    repo.objects.refresh = RefreshMode::Never;

    let old_tree_id = repo.rev_parse_single(old_treeish.as_bstr())?;
    let new_tree_id = repo.rev_parse_single(new_treeish.as_bstr())?;

    let old_tree = old_tree_id.object()?.peel_to_tree()?;
    let new_tree = new_tree_id.object()?.peel_to_tree()?;

    let changes = repo.diff_tree_to_tree(&old_tree, &new_tree, None)?;

    writeln!(
        out,
        "Diffing trees `{old_treeish}` ({old_tree_id}) -> `{new_treeish}` ({new_tree_id})\n"
    )?;
    write_changes(&repo, out, changes)?;

    Ok(())
}

fn write_changes(
    repo: &gix::Repository,
    mut out: impl std::io::Write,
    changes: Vec<gix::diff::tree_with_rewrites::Change>,
) -> Result<(), std::io::Error> {
    for change in changes {
        match change {
            gix::diff::tree_with_rewrites::Change::Addition {
                location,
                id,
                entry_mode,
                ..
            } => {
                writeln!(out, "A: {}", typed_location(location, entry_mode))?;
                writeln!(out, "  {}", id.attach(repo).shorten_or_id())?;
                writeln!(out, "  -> {entry_mode:o}")?;
            }
            gix::diff::tree_with_rewrites::Change::Deletion {
                location,
                id,
                entry_mode,
                ..
            } => {
                writeln!(out, "D: {}", typed_location(location, entry_mode))?;
                writeln!(out, "  {}", id.attach(repo).shorten_or_id())?;
                writeln!(out, "  {entry_mode:o} ->")?;
            }
            gix::diff::tree_with_rewrites::Change::Modification {
                location,
                previous_id,
                id,
                previous_entry_mode,
                entry_mode,
            } => {
                writeln!(out, "M: {}", typed_location(location, entry_mode))?;
                writeln!(
                    out,
                    "  {previous_id} -> {id}",
                    previous_id = previous_id.attach(repo).shorten_or_id(),
                    id = id.attach(repo).shorten_or_id()
                )?;
                if previous_entry_mode != entry_mode {
                    writeln!(out, "  {previous_entry_mode:o} -> {entry_mode:o}")?;
                }
            }
            gix::diff::tree_with_rewrites::Change::Rewrite {
                source_location,
                source_id,
                id,
                location,
                source_entry_mode,
                entry_mode,
                ..
            } => {
                writeln!(
                    out,
                    "R: {source} -> {dest}",
                    source = typed_location(source_location, source_entry_mode),
                    dest = typed_location(location, entry_mode)
                )?;
                writeln!(
                    out,
                    "  {source_id} -> {id}",
                    source_id = source_id.attach(repo).shorten_or_id(),
                    id = id.attach(repo).shorten_or_id()
                )?;
                if source_entry_mode != entry_mode {
                    writeln!(out, "  {source_entry_mode:o} -> {entry_mode:o}")?;
                }
            }
        }
    }

    Ok(())
}

fn typed_location(mut location: BString, mode: EntryMode) -> BString {
    if mode.is_tree() {
        location.push(b'/');
    }
    location
}

fn resolve_revspec(
    repo: &gix::Repository,
    revspec: BString,
) -> Result<(ObjectId, Option<std::path::PathBuf>, BString), anyhow::Error> {
    let result = repo.rev_parse(revspec.as_bstr());

    match result {
        Err(err) => {
            // When the revspec is just a name, the delegate tries to resolve a reference which fails.
            // We extract the error from the tree to learn the name, and treat it as file.
            let not_found = err
                .sources()
                .find_map(|err| err.downcast_ref::<gix::refs::file::find::existing::Error>());
            if let Some(gix::refs::file::find::existing::Error::NotFound { name }) = not_found {
                let root = repo.workdir().map(ToOwned::to_owned);
                let name = gix::path::os_string_into_bstring(name.into())?;

                Ok((ObjectId::null(gix::hash::Kind::Sha1), root, name))
            } else {
                Err(err.into())
            }
        }
        Ok(resolved_revspec) => {
            let blob_id = resolved_revspec
                .single()
                .context(format!("rev-spec '{revspec}' must resolve to a single object"))?;

            let (path, _) = resolved_revspec
                .path_and_mode()
                .context(format!("rev-spec '{revspec}' must contain a path"))?;

            Ok((blob_id.into(), None, path.into()))
        }
    }
}

pub fn file(
    mut repo: gix::Repository,
    out: &mut dyn std::io::Write,
    old_revspec: BString,
    new_revspec: BString,
) -> Result<(), anyhow::Error> {
    repo.object_cache_size_if_unset(repo.compute_object_cache_size_for_tree_diffs(&**repo.index_or_empty()?));
    repo.objects.refresh = RefreshMode::Never;

    let (old_blob_id, old_root, old_path) = resolve_revspec(&repo, old_revspec)?;
    let (new_blob_id, new_root, new_path) = resolve_revspec(&repo, new_revspec)?;

    let worktree_roots = gix::diff::blob::pipeline::WorktreeRoots { old_root, new_root };

    let mut resource_cache = repo.diff_resource_cache(
        gix::diff::blob::pipeline::Mode::ToGitUnlessBinaryToTextIsPresent,
        worktree_roots,
    )?;

    resource_cache.set_resource(
        old_blob_id,
        gix::object::tree::EntryKind::Blob,
        old_path.as_ref(),
        gix::diff::blob::ResourceKind::OldOrSource,
        &repo.objects,
    )?;
    resource_cache.set_resource(
        new_blob_id,
        gix::object::tree::EntryKind::Blob,
        new_path.as_ref(),
        gix::diff::blob::ResourceKind::NewOrDestination,
        &repo.objects,
    )?;

    let outcome = resource_cache.prepare_diff()?;

    use gix::diff::blob::platform::prepare_diff::Operation;

    let algorithm = match outcome.operation {
        Operation::InternalDiff { algorithm } => algorithm,
        Operation::ExternalCommand { .. } => {
            unreachable!("We disabled that")
        }
        Operation::SourceOrDestinationIsBinary => {
            anyhow::bail!("Source or destination is binary and we can't diff that")
        }
    };

    let interner = gix::diff::blob::InternedInput::new(
        tokens_for_diffing(outcome.old.data.as_slice().unwrap_or_default()),
        tokens_for_diffing(outcome.new.data.as_slice().unwrap_or_default()),
    );

    let diff = gix::diff::blob::diff_with_slider_heuristics(algorithm, &interner);
    let rendered = gix::diff::blob::UnifiedDiff::new(
        &diff,
        &interner,
        gix::diff::blob::unified_diff::ConsumeBinaryHunk::new(BString::default(), "\n"),
        gix::diff::blob::unified_diff::ContextSize::symmetrical(3),
    )
    .consume()?;
    write!(out, "{rendered}")?;

    Ok(())
}

pub(crate) fn tokens_for_diffing(data: &[u8]) -> gix::diff::blob::platform::resource::ByteLinesWithoutTerminator<'_> {
    gix::diff::blob::platform::resource::ByteLinesWithoutTerminator::new(data)
}
