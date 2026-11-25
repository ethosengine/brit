//! Shared ancestry and metadata preparation for fast-forward integration.
use super::*;

pub(super) fn unchanged(repo: &gix::Repository, resource: &Path) -> Result<Change> {
    let before = snapshot(resource)?;
    ensure!(
        matches!(before, State::Absent | State::File { .. })
            || (resource == repo.git_dir().join("logs") && matches!(before, State::Directory { .. })),
        "unsafe metadata resource {}",
        resource.display()
    );
    Ok(Change {
        path: raw(&absolute(repo, resource))?,
        after: before.clone(),
        before,
    })
}
pub(super) fn set_bytes(change: &mut Change, bytes: Vec<u8>) {
    change.after = State::File {
        mode: file_mode(&change.before, 0o644),
        bytes,
    };
}
pub(super) fn append_log(
    repo: &gix::Repository,
    change: &mut Change,
    old: &str,
    new: &str,
    message: &str,
) -> Result<()> {
    let mut bytes = match &change.before {
        State::Absent => Vec::new(),
        State::File { bytes, .. } => bytes.clone(),
        _ => bail!("unsupported reflog"),
    };
    gix::refs::log::Line {
        previous_oid: gix::ObjectId::from_hex(old.as_bytes())?,
        new_oid: gix::ObjectId::from_hex(new.as_bytes())?,
        signature: repo
            .committer()
            .context("committer identity required for integration reflog")??
            .to_owned()?,
        message: message.into(),
    }
    .write_to(&mut bytes)?;
    set_bytes(change, bytes);
    Ok(())
}

pub(super) fn prepare_metadata_directories(repo: &gix::Repository, state: &mut Manifest) -> Result<()> {
    use std::os::unix::fs::DirBuilderExt;
    let mut missing = BTreeSet::new();
    for change in state
        .fetch_metadata()
        .iter()
        .chain(state.integration_metadata())
        .chain(std::iter::once(&state.reflog))
    {
        validate_metadata_ancestors(repo, &path(&change.path)?)?;
        let mut parent = path(&change.path)?;
        parent.pop();
        while snapshot(&parent)? == State::Absent {
            missing.insert(parent.clone());
            ensure!(parent.pop(), "missing metadata root");
        }
        ensure!(
            matches!(snapshot(&parent)?, State::Directory { .. }),
            "unsafe metadata ancestor"
        );
    }
    for directory in missing {
        let directories = state.metadata_directories_mut()?;
        if !directories
            .iter()
            .any(|change| path(&change.path).ok().as_ref() == Some(&directory))
        {
            directories.push(Change {
                path: raw(&directory)?,
                before: State::Absent,
                after: State::Absent,
            });
            save(repo, state)?;
        }
    }
    let count = state.metadata_directories().len();
    for i in 0..count {
        let change = state.metadata_directories()[i].clone();
        let directory = path(&change.path)?;
        let temporary = journal(repo).join(format!("metadata-dir-{i}"));
        if change.after == State::Absent {
            ensure!(
                snapshot(&directory)? == State::Absent,
                "foreign metadata directory appeared"
            );
            if present(&temporary)? {
                fs::remove_dir(&temporary)?;
            }
            fs::DirBuilder::new().mode(creation_mode(0o777)?).create(&temporary)?;
            sync_dir(&temporary)?;
            state.metadata_directories_mut()?[i].after = snapshot(&temporary)?;
            save(repo, state)?;
        }
        let change = &state.metadata_directories()[i];
        let actual = snapshot(&directory)?;
        ensure!(
            actual == State::Absent || actual == change.after,
            "foreign metadata directory"
        );
        if actual == State::Absent {
            ensure!(
                snapshot(&temporary)? == change.after,
                "private metadata directory changed"
            );
            fs::rename(&temporary, &directory)?;
            sync_dir(&directory)?;
            sync_dir(directory.parent().context("metadata directory parent")?)?;
            sync_dir(&journal(repo))?;
        }
    }
    // The shared transition's HEAD-log directory was captured before metadata preparation.
    if state.log_directory.before == State::Absent && present(&repo.git_dir().join("logs"))? {
        state.log_directory.before = snapshot(&repo.git_dir().join("logs"))?;
        state.log_directory.after = state.log_directory.before.clone();
    }
    save(repo, state)
}

/// What a walk of local history proved about one commit's ancestry.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Reach {
    /// A path of local commits reaches the ancestor.
    Found,
    /// The complete local history holds no such path.
    Absent,
    /// No path above the shallow boundary; one may run below this boundary commit.
    Hidden(gix::ObjectId),
}

/// The commits `.git/shallow` declares parentless; empty for a complete repository.
pub(super) fn shallow_boundary(repo: &gix::Repository) -> Result<BTreeSet<gix::ObjectId>> {
    Ok(repo
        .shallow_commits()?
        .map(|commits| commits.iter().copied().collect())
        .unwrap_or_default())
}

/// Walk the commits reachable from `tip` without reading a parent of a `boundary` commit.
/// Return whether `stop` was reached, and the boundary commits the walk reached.
fn visible_history(
    repo: &gix::Repository,
    tip: gix::ObjectId,
    boundary: &BTreeSet<gix::ObjectId>,
    stop: Option<gix::ObjectId>,
) -> Result<(bool, BTreeSet<gix::ObjectId>)> {
    let mut pending = vec![tip];
    let mut seen = BTreeSet::new();
    let mut reached = BTreeSet::new();
    while let Some(commit_id) = pending.pop() {
        if Some(commit_id) == stop {
            return Ok((true, reached));
        }
        if !seen.insert(commit_id) {
            continue;
        }
        if boundary.contains(&commit_id) {
            reached.insert(commit_id);
            continue;
        }
        let commit = repo.find_object(commit_id)?.try_into_commit()?;
        let decoded = commit.decode()?;
        pending.extend(decoded.parents());
    }
    Ok((false, reached))
}

/// Whether `ancestor` is reachable from `descendant`, never reading past `boundary`.
///
/// The walk from `descendant` sees every commit reachable above the boundary, so any path to
/// `ancestor` it misses runs through one of the boundary commits H it stopped at, with
/// `ancestor` strictly below that h (h != `ancestor`, or the walk had found it). If every h in
/// H is itself reachable from `ancestor` by a path one walk from `ancestor` proves above the
/// boundary, `ancestor` would lie both above and below h, a cycle, so no such path exists:
/// Absent. Every h must be proven: a single h the walk cannot reach may be exactly the one a
/// real path runs through, so it leaves the question open as Hidden(h), never a guess.
pub(super) fn reach(
    repo: &gix::Repository,
    ancestor: &str,
    descendant: &str,
    boundary: &BTreeSet<gix::ObjectId>,
) -> Result<Reach> {
    let ancestor = gix::ObjectId::from_hex(ancestor.as_bytes())?;
    let descendant = gix::ObjectId::from_hex(descendant.as_bytes())?;
    let (found, hidden) = visible_history(repo, descendant, boundary, Some(ancestor))?;
    if found {
        return Ok(Reach::Found);
    }
    if hidden.is_empty() {
        return Ok(Reach::Absent);
    }
    let (_, below_ancestor) = visible_history(repo, ancestor, boundary, None)?;
    Ok(match hidden.difference(&below_ancestor).next() {
        Some(&h) => Reach::Hidden(h),
        None => Reach::Absent,
    })
}

/// Check every component, not just the nearest parent: a refs/remotes symlink
/// must not let the publication escape through an ordinary directory beneath it.
pub(super) fn validate_metadata_ancestors(repo: &gix::Repository, resource: &Path) -> Result<()> {
    use std::os::unix::fs::MetadataExt;
    let common = absolute(repo, repo.common_dir());
    let git_dir = absolute(repo, repo.git_dir());
    // A linked worktree's common root can be spelled `git_dir/../..`.
    // Match that declared root before interpreting its descendants.
    let root = [&git_dir, &common]
        .into_iter()
        .filter(|root| resource.starts_with(root))
        .max_by_key(|root| root.components().count())
        .context("foreign metadata resource")?;
    let relative = resource.strip_prefix(root).context("foreign metadata resource")?;
    let mut cursor = root.clone();
    let device = fs::metadata(repo.git_dir())?.dev();
    for component in relative.parent().context("metadata parent")?.components() {
        ensure!(
            matches!(component, std::path::Component::Normal(_)),
            "unsafe metadata component"
        );
        cursor.push(component);
        match fs::symlink_metadata(&cursor) {
            Ok(metadata) => ensure!(
                metadata.is_dir() && metadata.dev() == device,
                "unsafe or cross-filesystem metadata ancestor {}",
                cursor.display()
            ),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
    }
    Ok(())
}

pub(super) fn prepare_merge(
    repo: &gix::Repository,
    branch: &[u8],
    source_oid: &str,
    revision: &BStr,
) -> Result<(String, Merge)> {
    let target_oid = repo
        .rev_parse_single(revision)?
        .object()?
        .peel_to_commit()?
        .id
        .to_string();
    let boundary = shallow_boundary(repo)?;
    let fast_forward = reach(repo, source_oid, &target_oid, &boundary)?;
    let target_oid = if fast_forward == Reach::Found {
        target_oid
    } else {
        match (fast_forward, reach(repo, &target_oid, source_oid, &boundary)?) {
            (_, Reach::Found) => source_oid.to_owned(),
            (Reach::Hidden(boundary), _) | (_, Reach::Hidden(boundary)) => {
                bail!(
                    "fast-forward needs history below the shallow boundary at {boundary}; deepen the repository first"
                );
            }
            (_, Reach::Absent) => bail!("Not possible to fast-forward"),
        }
    };

    let orig_head = repo.git_dir().join("ORIG_HEAD");
    let branch_path = repo.common_dir().join(path(branch)?);
    let branch_log = repo.common_dir().join("logs").join(path(branch)?);
    for resource in [&orig_head, &branch_path, &branch_log, &repo.git_dir().join("logs/HEAD")] {
        validate_metadata_ancestors(repo, &absolute(repo, resource))?;
    }
    let mut orig_head = unchanged(repo, &orig_head)?;
    set_bytes(&mut orig_head, format!("{source_oid}\n").into_bytes());
    let mut branch = unchanged(repo, &branch_path)?;
    let message = (target_oid != source_oid).then(|| format!("merge {revision}: Fast-forward"));
    let mut log = None;
    if let Some(message) = &message {
        set_bytes(&mut branch, format!("{target_oid}\n").into_bytes());
        if reflog_enabled(repo)? || present(&branch_log)? {
            let mut change = unchanged(repo, &branch_log)?;
            append_log(repo, &mut change, source_oid, &target_oid, message)?;
            log = Some(change);
        }
    }
    let mut metadata = vec![orig_head, branch];
    metadata.extend(log);
    Ok((
        target_oid,
        Merge {
            message,
            metadata,
            directories: Vec::new(),
        },
    ))
}

pub(super) fn validate_merge(repo: &gix::Repository, state: &Manifest, merge: &Merge) -> Result<()> {
    ensure!(
        state.pull.is_none() && state.source.as_ref() == Some(&state.target),
        "invalid merge journal attachment"
    );
    let git_dir = absolute(repo, repo.git_dir());
    let common = absolute(repo, repo.common_dir());
    let branch = path(&state.target)?;
    let mut resources = vec![git_dir.join("ORIG_HEAD"), common.join(&branch)];
    if merge.metadata.len() == 3 {
        resources.push(common.join("logs").join(branch));
    }
    ensure!(merge.metadata.len() == resources.len(), "invalid merge metadata count");
    for (change, expected) in merge.metadata.iter().zip(resources) {
        ensure!(path(&change.path)? == expected, "foreign merge metadata");
        validate_metadata_ancestors(repo, &expected)?;
    }
    validate_metadata_directories(repo, state)
}

pub(super) fn validate_metadata_directories(repo: &gix::Repository, state: &Manifest) -> Result<()> {
    let common = absolute(repo, repo.common_dir());
    let git_dir = absolute(repo, repo.git_dir());
    let mut unique = BTreeSet::new();
    for change in state.metadata_directories() {
        let directory = path(&change.path)?;
        ensure!(
            state
                .fetch_metadata()
                .iter()
                .chain(state.integration_metadata())
                .chain(std::iter::once(&state.reflog))
                .any(|metadata| path(&metadata.path)
                    .ok()
                    .is_some_and(|resource| resource != directory && resource.starts_with(&directory))),
            "foreign metadata directory"
        );
        ensure!(
            directory.starts_with(&common) || directory.starts_with(&git_dir),
            "foreign metadata directory root"
        );
        ensure!(
            change.before == State::Absent
                && matches!(change.after, State::Absent | State::Directory { .. })
                && unique.insert(directory),
            "invalid owned directory state"
        );
    }
    Ok(())
}
