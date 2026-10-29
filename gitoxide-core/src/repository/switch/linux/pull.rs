//! Explicit remote-branch fast-forward integration, sharing switch's private durable transition.
use super::*;
use crate::repository::fetch::{pull_receive, pull_tracking};

pub fn execute(repo: gix::Repository, remote: &str, branch: &BStr) -> Result<()> {
    let shared = shared_guard(&repo)?;
    ensure_no_pending(&repo)?;
    preflight(&repo)?;
    ensure!(
        !remote.is_empty() && !remote.starts_with('-') && !remote.contains(['\n', '\r', '\0']),
        "explicit remote required"
    );
    let branch_argument = branch.to_vec();
    let branch = if branch.starts_with(b"refs/heads/") {
        branch.to_vec()
    } else {
        ensure!(
            !branch.starts_with(b"refs/") && !branch.starts_with(b"-") && !branch.is_empty(),
            "existing remote branch name required"
        );
        [b"refs/heads/".as_slice(), branch.as_bytes()].concat()
    };
    let _: gix::refs::FullName = branch.as_bstr().try_into()?;
    let tracking = pull_tracking(&repo, remote, branch.as_bstr())?;
    let remote_url = remote_url(&repo, remote)?;
    let tracking_oid = tracking
        .as_ref()
        .map(|(name, _)| tracking_oid(&repo, name.as_bstr()))
        .transpose()?
        .flatten();
    let source = repo.head()?.referent_name().map(|name| name.as_bstr().to_vec());
    if let Some(source) = &source {
        ensure!(
            source.starts_with(b"refs/heads/"),
            "non-local HEAD attachment requires Git"
        );
        branch_oid(&repo, source)?;
        occupancy(&repo, source)?;
    }
    let source_oid = repo.head_commit()?.id().to_string();
    let root = repo.workdir().context("worktree required")?;
    let git_dir = repo.git_dir();
    let mut fetch_metadata = vec![unchanged(&repo, &git_dir.join("FETCH_HEAD"))?];
    if let Some((name, _)) = &tracking {
        fetch_metadata.push(unchanged(&repo, &repo.common_dir().join(path(name)?))?);
        let log = repo.common_dir().join("logs").join(path(name)?);
        if reflog_enabled(&repo)? || present(&log)? {
            fetch_metadata.push(unchanged(&repo, &log)?);
        }
    }
    let mut metadata = vec![unchanged(&repo, &git_dir.join("ORIG_HEAD"))?];
    if let Some(source) = &source {
        metadata.push(unchanged(&repo, &repo.common_dir().join(path(source)?))?);
        let log = repo.common_dir().join("logs").join(path(source)?);
        if reflog_enabled(&repo)? || present(&log)? {
            metadata.push(unchanged(&repo, &log)?);
        }
    }
    if reflog_enabled(&repo)?
        || present(&git_dir.join("logs/HEAD"))?
        || fetch_metadata.iter().chain(&metadata).any(|change| {
            path(&change.path)
                .ok()
                .is_some_and(|resource| resource.starts_with(absolute(&repo, repo.common_dir()).join("logs")))
        })
    {
        repo.committer()
            .context("committer identity required for pull reflogs")??
            .to_owned()?;
    }
    let mut state = Manifest {
        version: VERSION,
        pull: Some(Pull {
            remote: remote.into(),
            remote_url,
            tracking_oid,
            tracking: tracking.as_ref().map(|(name, force)| (name.to_vec(), *force)),
            branch: branch.clone(),
            branch_argument,
            pinned: false,
            fetched: false,
            no_op: false,
            fetch_decision: false,
            fetch_done: false,
            fetch_metadata,
            metadata,
            directories: Vec::new(),
            objects: Vec::new(),
            keeps: Vec::new(),
        }),
        git_dir: raw(&fs::canonicalize(git_dir)?)?,
        root: raw(&fs::canonicalize(root)?)?,
        common: raw(&fs::canonicalize(repo.common_dir())?)?,
        git_identity: directory_identity(git_dir)?,
        root_identity: directory_identity(root)?,
        common_identity: directory_identity(repo.common_dir())?,
        pid: std::process::id(),
        process: process(std::process::id())?,
        locks: Vec::new(),
        source,
        source_oid: source_oid.clone(),
        target: branch,
        target_oid: source_oid,
        started: BTreeSet::new(),
        changes: Vec::new(),
        index: unchanged(&repo, &repo.index_path())?,
        head: unchanged(&repo, &git_dir.join("HEAD"))?,
        reflog: unchanged(&repo, &git_dir.join("logs/HEAD"))?,
        log_directory: unchanged(&repo, &git_dir.join("logs"))?,
        preparing: true,
        decision: false,
        aborting: false,
        hook: "not-started".into(),
    };
    use std::os::unix::fs::DirBuilderExt;
    fs::DirBuilder::new().mode(0o700).create(journal(&repo))?;
    save(&repo, &state)?;
    sync_dir(git_dir)?;
    // Normal errors before receive remove only private preparation; interruptions retain intent.
    let result = resume_fetch_inner(&repo, &mut state, false);
    match result {
        Ok(()) => integrate(repo, state, shared, false),
        Err(error) => {
            if !gix::interrupt::is_triggered() && !state.pull.as_ref().context("pull state")?.pinned {
                cleanup(&repo, &state)?;
            }
            Err(error.context("pull interrupted or refused; use brit pull --status, --continue or --abort"))
        }
    }
}

fn preflight(repo: &gix::Repository) -> Result<()> {
    let root = repo.workdir().context("pull requires a populated worktree")?;
    validate_local_history(repo)?;
    for marker in [
        "MERGE_HEAD",
        "CHERRY_PICK_HEAD",
        "REVERT_HEAD",
        "BISECT_START",
        "rebase-merge",
        "rebase-apply",
        "sequencer",
    ] {
        ensure!(
            !present(&repo.git_dir().join(marker))?,
            "repository operation in progress: {marker}"
        );
    }
    validate_object_boundaries(repo)?;
    read_index(repo)?;
    repo.head_commit()?;
    super::super::super::authoring::hook_path(repo, "post-merge")?;
    for directory in [
        root,
        repo.git_dir(),
        repo.common_dir(),
        &repo.common_dir().join("objects"),
    ] {
        sync_dir(directory)?;
    }
    Ok(())
}
fn unchanged(repo: &gix::Repository, resource: &Path) -> Result<Change> {
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
fn set_bytes(change: &mut Change, bytes: Vec<u8>) {
    change.after = State::File {
        mode: file_mode(&change.before, 0o644),
        bytes,
    };
}
fn append_log(repo: &gix::Repository, change: &mut Change, old: &str, new: &str, message: &str) -> Result<()> {
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
            .context("committer identity required for pull reflog")??
            .to_owned()?,
        message: message.into(),
    }
    .write_to(&mut bytes)?;
    set_bytes(change, bytes);
    Ok(())
}

pub(super) fn prepare_pull_metadata(repo: &gix::Repository, state: &mut Manifest) -> Result<()> {
    let pull = state.pull.as_mut().context("pull state")?;
    state.head.after = if state.source.is_some() {
        state.head.before.clone()
    } else {
        State::File {
            mode: file_mode(&state.head.before, 0o644),
            bytes: format!("{}\n", state.target_oid).into_bytes(),
        }
    };
    let message = format!(
        "pull --ff-only {} {}: Fast-forward",
        display_remote(pull.remote.as_bytes())?,
        pull.branch_argument.as_bstr()
    );
    if reflog_enabled(repo)? || state.reflog.before != State::Absent {
        append_log(repo, &mut state.reflog, &state.source_oid, &state.target_oid, &message)?;
    } else {
        state.reflog.after = State::Absent;
    }
    for change in &mut pull.metadata {
        if change.path.ends_with(b"ORIG_HEAD") {
            set_bytes(change, format!("{}\n", state.source_oid).into_bytes());
        } else if path(&change.path)?.starts_with(absolute(repo, repo.common_dir()).join("logs")) {
            append_log(repo, change, &state.source_oid, &state.target_oid, &message)?;
        } else {
            set_bytes(change, format!("{}\n", state.target_oid).into_bytes());
        }
    }
    Ok(())
}

fn prepare_directories(repo: &gix::Repository, state: &mut Manifest) -> Result<()> {
    use std::os::unix::fs::DirBuilderExt;
    let mut missing = BTreeSet::new();
    let pull = state.pull.as_ref().context("pull state")?;
    for change in pull
        .fetch_metadata
        .iter()
        .chain(&pull.metadata)
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
        let pull = state.pull.as_mut().context("pull state")?;
        if !pull
            .directories
            .iter()
            .any(|change| path(&change.path).ok().as_ref() == Some(&directory))
        {
            pull.directories.push(Change {
                path: raw(&directory)?,
                before: State::Absent,
                after: State::Absent,
            });
            save(repo, state)?;
        }
    }
    let count = state.pull.as_ref().context("pull state")?.directories.len();
    for i in 0..count {
        let change = state.pull.as_ref().context("pull state")?.directories[i].clone();
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
            state.pull.as_mut().context("pull state")?.directories[i].after = snapshot(&temporary)?;
            save(repo, state)?;
        }
        let change = &state.pull.as_ref().context("pull state")?.directories[i];
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
    // The shared transition's HEAD-log directory was captured before fetch preparation.
    if state.log_directory.before == State::Absent && present(&repo.git_dir().join("logs"))? {
        state.log_directory.before = snapshot(&repo.git_dir().join("logs"))?;
        state.log_directory.after = state.log_directory.before.clone();
    }
    save(repo, state)
}

pub(super) fn resume_fetch(repo: gix::Repository, mut state: Manifest, shared: fs::File, abort: bool) -> Result<()> {
    preflight(&repo)?;
    resume_fetch_inner(&repo, &mut state, abort)?;
    integrate(repo, state, shared, abort)
}
fn resume_fetch_inner(repo: &gix::Repository, state: &mut Manifest, abort: bool) -> Result<()> {
    refs_match(repo, state)?;
    ensure!(
        snapshot(&repo.git_dir().join("HEAD"))? == state.head.before
            && snapshot(&repo.index_path())? == state.index.before,
        "source metadata changed during fetch"
    );
    if (abort || state.aborting) && !state.pull.as_ref().context("pull state")?.fetch_decision {
        return Ok(());
    }
    prepare_directories(repo, state)?;
    let mut locks = lock_resources(repo, state)?;
    refs_match(repo, state)?;
    if !state.pull.as_ref().context("pull state")?.fetched {
        let pull = state.pull.as_ref().context("pull state")?;
        let remote = pull.remote.clone();
        let branch = BString::from(pull.branch.clone());
        ensure!(
            remote_url(repo, &remote)? == pull.remote_url,
            "remote URL changed since preparation; recovery refused"
        );
        ensure!(
            pull_tracking(repo, &remote, branch.as_bstr())?.map(|(name, force)| (name.to_vec(), force))
                == pull.tracking,
            "tracking mapping changed since preparation; recovery refused"
        );
        let result = pull_receive(repo, &remote, branch.as_bstr(), |plan| {
            let pull = state.pull.as_mut().context("pull state")?;
            ensure!(
                plan.tracking.as_ref().map(|(name, force)| (name.to_vec(), *force)) == pull.tracking,
                "tracking mapping changed during receive preparation"
            );
            if pull.pinned {
                ensure!(
                    gix::ObjectId::from_hex(state.target_oid.as_bytes())? == plan.target_oid,
                    "remote branch moved after interruption; pinned target retained"
                );
            }
            state.target_oid = plan.target_oid.to_string();
            pull.pinned = true;
            let short = pull
                .branch
                .strip_prefix(b"refs/heads/")
                .context("branch prefix")?
                .as_bstr();
            set_bytes(
                &mut pull.fetch_metadata[0],
                format!(
                    "{}\t\tbranch '{}' of {}\n",
                    plan.target_oid,
                    short,
                    display_remote(&pull.remote_url)?
                )
                .into_bytes(),
            );
            if let Some((name, _force)) = &plan.tracking {
                let old = tracking_oid(repo, name.as_bstr())?;
                ensure!(
                    old == pull.tracking_oid,
                    "tracking reference changed during fetch preparation"
                );
                if old.as_ref() != Some(&state.target_oid) {
                    set_bytes(
                        pull.fetch_metadata
                            .get_mut(1)
                            .context("missing pinned tracking metadata")?,
                        format!("{}\n", plan.target_oid).into_bytes(),
                    );
                    if let Some(log) = pull.fetch_metadata.get_mut(2) {
                        append_log(
                            repo,
                            log,
                            old.as_deref().unwrap_or(&repo.object_hash().null().to_string()),
                            &state.target_oid,
                            "pull: fetch",
                        )?;
                    }
                }
            }
            save(repo, state)?;
            checkpoint("fetch-pinned")?;
            validate_object_boundaries(repo)
        })?;
        checkpoint("fetch-received")?;
        if let Some((keep, (device, inode))) = result.keep_path.zip(result.keep_identity) {
            use std::os::unix::fs::MetadataExt;
            let metadata = fs::symlink_metadata(&keep)?;
            ensure!(
                metadata.is_file()
                    && metadata.dev() == device
                    && metadata.ino() == inode
                    && fs::read(&keep)?.is_empty(),
                "received pack protection changed; evidence retained"
            );
            state.pull.as_mut().context("pull state")?.keeps.push(Lock {
                path: raw(&absolute(repo, &keep))?,
                device,
                inode,
                checksum: digest(b"")?,
            });
            sync_path(&keep)?;
            sync_dir(keep.parent().context("keep parent")?)?;
            save(repo, state)?;
        }
        let _ = result.object_paths;
        repo.find_object(gix::ObjectId::from_hex(state.target_oid.as_bytes())?)?
            .try_into_commit()?;
        validate_connectivity(repo, &state.target_oid)?;
        let pull = state.pull.as_mut().context("pull state")?;
        if let Some((name, force)) = result.plan.tracking {
            let old = tracking_oid(repo, name.as_bstr())?;
            ensure!(old == pull.tracking_oid, "tracking reference changed during receive");
            if !force && let Some(old) = old {
                ensure!(
                    is_ancestor(repo, &old, &state.target_oid)?,
                    "non-fast-forward remote tracking update refused"
                );
            }
        }
        pull.objects = synchronize_objects(repo)?;
        pull.fetched = true;
        save(repo, state)?;
        checkpoint("objects-durable")?;
    }
    validate_objects(repo, state.pull.as_ref().context("pull state")?)?;
    verify_locks(&locks)?;
    let pull = state.pull.as_ref().context("pull state")?;
    for change in &pull.fetch_metadata {
        let actual = snapshot(&path(&change.path)?)?;
        ensure!(
            actual == change.before || (pull.fetch_decision && actual == change.after),
            "foreign fetch metadata before publication decision"
        );
    }
    if let Some((name, _)) = &pull.tracking {
        let actual = tracking_oid(repo, name.as_bstr())?;
        ensure!(
            actual == pull.tracking_oid || (pull.fetch_decision && actual.as_ref() == Some(&state.target_oid)),
            "tracking reference changed before publication decision"
        );
    }
    if !state.pull.as_ref().context("pull state")?.fetch_decision {
        state.pull.as_mut().context("pull state")?.fetch_decision = true;
        save(repo, state)?;
    }
    checkpoint("fetch-decision")?;
    for (i, change) in state
        .pull
        .as_ref()
        .context("pull state")?
        .fetch_metadata
        .iter()
        .enumerate()
    {
        verify_locks(&locks)?;
        refs_match(repo, state)?;
        publish_metadata(repo, change, &mut locks)?;
        checkpoint(if i == 0 {
            "fetch-head-published"
        } else {
            "fetch-ref-published"
        })?;
    }
    state.pull.as_mut().context("pull state")?.fetch_done = true;
    save(repo, state)?;
    release_locks(repo, &mut locks)?;
    state.locks.clear();
    save(repo, state)?;
    checkpoint("fetch-completed")
}

pub(super) fn integrate(repo: gix::Repository, mut state: Manifest, shared: fs::File, abort: bool) -> Result<()> {
    if abort || state.aborting {
        state.aborting = true;
        save(&repo, &state)?;
        cleanup(&repo, &state)?;
        return Ok(());
    }
    refs_match(&repo, &state)?;
    validate_objects(&repo, state.pull.as_ref().context("pull state")?)?;
    if is_ancestor(&repo, &state.target_oid, &state.source_oid)? {
        // No movement: only ORIG_HEAD is part of the integration decision.
        let source_oid = state.source_oid.clone();
        let pull = state.pull.as_mut().context("pull state")?;
        pull.no_op = true;
        for change in &mut pull.metadata {
            if change.path.ends_with(b"ORIG_HEAD") {
                set_bytes(change, format!("{source_oid}\n").into_bytes());
            }
        }
        state.preparing = false;
        let mut locks = lock_resources(&repo, &mut state)?;
        save(&repo, &state)?;
        finish(&repo, &mut state, &mut locks)?;
        println!("Already up to date.");
        return Ok(());
    }
    if !is_ancestor(&repo, &state.source_oid, &state.target_oid)? {
        cleanup(&repo, &state)?;
        bail!("Not possible to fast-forward; fetched results retained, branch and worktree unchanged");
    }
    let branch = BString::from(state.target.clone());
    // A normal preservation refusal leaves fetch results and no active integration.
    let retained = repo.clone();
    let result = execute_locked(repo, branch.as_bstr(), shared, Some(state));
    if result.is_err() && !gix::interrupt::is_triggered() {
        let pending = load(&retained)?;
        if pending.preparing {
            cleanup(&retained, &pending)?;
        }
    }
    result
}

pub fn recover(repo: gix::Repository, abort: bool) -> Result<()> {
    recover_operation(repo, abort, true)
}

fn is_ancestor(repo: &gix::Repository, ancestor: &str, descendant: &str) -> Result<bool> {
    let ancestor = gix::ObjectId::from_hex(ancestor.as_bytes())?;
    let mut pending = vec![gix::ObjectId::from_hex(descendant.as_bytes())?];
    let mut seen = BTreeSet::new();
    while let Some(commit_id) = pending.pop() {
        if commit_id == ancestor {
            return Ok(true);
        }
        if seen.insert(commit_id) {
            let commit = repo.find_object(commit_id)?.try_into_commit()?;
            let decoded = commit.decode()?;
            pending.extend(decoded.parents());
        }
    }
    Ok(false)
}
fn validate_connectivity(repo: &gix::Repository, target: &str) -> Result<()> {
    let mut pending = vec![gix::ObjectId::from_hex(target.as_bytes())?];
    let mut seen = BTreeSet::new();
    while let Some(object_id) = pending.pop() {
        if !seen.insert(object_id) {
            continue;
        }
        let object = repo.find_object(object_id)?;
        // ODB lookup decompresses objects but does not establish that their bytes
        // match the requested OID, including objects reused by a no-pack fetch.
        gix::objs::Data::new(&object.data, object.kind, object_id.kind()).verify_checksum(&object_id)?;
        match object.kind {
            gix::objs::Kind::Commit => {
                let commit = object.try_into_commit()?;
                let decoded = commit.decode()?;
                pending.push(decoded.tree());
                pending.extend(decoded.parents());
            }
            gix::objs::Kind::Tree => {
                for entry in object.try_into_tree()?.decode()?.entries {
                    if entry.mode.kind() != gix::objs::tree::EntryKind::Commit {
                        pending.push(entry.oid.to_owned());
                    }
                }
            }
            gix::objs::Kind::Blob => {}
            _ => bail!("unexpected reachable object kind"),
        }
    }
    Ok(())
}
fn synchronize_objects(repo: &gix::Repository) -> Result<Vec<(Vec<u8>, String)>> {
    let root = absolute(repo, repo.common_dir()).join("objects");
    ensure!(
        fs::symlink_metadata(&root)?.is_dir(),
        "symlink object root requires Git"
    );
    let mut records = Vec::new();
    let mut directories = vec![root.clone()];
    let mut i = 0;
    while i < directories.len() {
        for entry in fs::read_dir(&directories[i])? {
            let entry = entry?;
            let kind = entry.file_type()?;
            ensure!(!kind.is_symlink(), "symlink object storage requires Git");
            if kind.is_dir() {
                directories.push(entry.path());
            } else if kind.is_file() {
                sync_path(&entry.path())?;
                let relative = raw(entry.path().strip_prefix(&root)?)?;
                if object_member(&relative) {
                    records.push((relative, digest_file(&entry.path())?));
                }
            } else {
                bail!("unsupported object storage member");
            }
        }
        i += 1;
    }
    for directory in directories.iter().rev() {
        sync_dir(directory)?;
    }
    sync_dir(repo.common_dir())?;
    Ok(records)
}
pub(super) fn validate_objects(repo: &gix::Repository, pull: &Pull) -> Result<()> {
    validate_object_boundaries(repo)?;
    for (name, checksum) in &pull.objects {
        validate_path(name)?;
        let object = repo.common_dir().join("objects").join(path(name)?);
        ensure!(
            fs::symlink_metadata(&object)?.is_file() && digest_file(&object)? == *checksum,
            "fetched object storage changed: {}",
            name.as_bstr()
        );
    }
    Ok(())
}

pub(super) fn validate_pull(repo: &gix::Repository, state: &Manifest, pull: &Pull) -> Result<()> {
    ensure!(!pull.fetch_done || pull.fetch_decision, "invalid fetch completion");
    ensure!(!pull.fetch_decision || pull.fetched, "invalid fetch decision");
    ensure!(
        !state.decision || pull.fetch_done,
        "integration before fetch completion"
    );
    if let Some((name, _)) = &pull.tracking {
        let _: gix::refs::FullName = name.as_bstr().try_into()?;
        ensure!(name.starts_with(b"refs/remotes/"), "invalid pinned tracking reference");
    }
    ensure!(
        pull.fetch_metadata.len()
            == if pull.tracking.is_some() {
                2 + usize::from(pull.fetch_metadata.len() == 3)
            } else {
                1
            },
        "invalid fetch metadata count"
    );
    ensure!(
        pull.fetch_metadata
            .first()
            .is_some_and(|change| path(&change.path).ok() == Some(absolute(repo, repo.git_dir()).join("FETCH_HEAD"))),
        "missing FETCH_HEAD metadata"
    );
    if let Some((name, _)) = &pull.tracking {
        ensure!(
            path(&pull.fetch_metadata[1].path)? == absolute(repo, repo.common_dir()).join(path(name)?),
            "tracking metadata mismatch"
        );
        if let Some(log) = pull.fetch_metadata.get(2) {
            ensure!(
                path(&log.path)? == absolute(repo, repo.common_dir()).join("logs").join(path(name)?),
                "tracking reflog mismatch"
            );
        }
    }
    let branch: gix::refs::FullName = pull.branch.as_bstr().try_into()?;
    ensure!(
        branch.as_bstr().starts_with(b"refs/heads/") && state.target == pull.branch,
        "invalid remote branch"
    );
    let common = absolute(repo, repo.common_dir());
    let git_dir = absolute(repo, repo.git_dir());
    let mut unique = BTreeSet::new();
    for change in &pull.fetch_metadata {
        let resource = path(&change.path)?;
        validate_metadata_ancestors(repo, &resource)?;
        ensure!(
            resource == git_dir.join("FETCH_HEAD")
                || resource.starts_with(common.join("refs/remotes"))
                || resource.starts_with(common.join("logs/refs/remotes")),
            "foreign fetch metadata"
        );
        ensure!(unique.insert(resource.clone()), "duplicate fetch metadata");
        if resource != git_dir.join("FETCH_HEAD") {
            validate_path(&raw(resource.strip_prefix(&common)?)?)?;
        }
    }
    unique.clear();
    for change in &pull.metadata {
        let resource = path(&change.path)?;
        validate_metadata_ancestors(repo, &resource)?;
        ensure!(
            resource == git_dir.join("ORIG_HEAD")
                || state.source.as_ref().is_some_and(|name| path(name)
                    .ok()
                    .is_some_and(|name| resource == common.join(&name) || resource == common.join("logs").join(name))),
            "foreign integration metadata"
        );
        ensure!(unique.insert(resource), "duplicate integration metadata");
    }
    for change in &pull.directories {
        let directory = path(&change.path)?;
        ensure!(
            pull.fetch_metadata
                .iter()
                .chain(&pull.metadata)
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
            change.before == State::Absent && matches!(change.after, State::Absent | State::Directory { .. }),
            "invalid owned directory state"
        );
    }
    for (name, _) in &pull.objects {
        validate_path(name)?;
        ensure!(object_member(name), "invalid object recovery path");
    }
    for keep in &pull.keeps {
        let keep_path = path(&keep.path)?;
        ensure!(
            keep_path.parent() == Some(common.join("objects/pack").as_path()),
            "foreign pack protection path"
        );
        let name = keep_path.file_name().context("keep name")?.as_encoded_bytes();
        ensure!(
            name.starts_with(b"pack-")
                && name.ends_with(b".keep")
                && name[5..name.len() - 5].iter().all(u8::is_ascii_hexdigit),
            "invalid pack protection name"
        );
        ensure!(keep.checksum == digest(b"")?, "invalid pack protection checksum");
    }
    Ok(())
}

/// Check every component, not just the nearest parent: a refs/remotes symlink
/// must not let the publication escape through an ordinary directory beneath it.
pub(super) fn validate_metadata_ancestors(repo: &gix::Repository, resource: &Path) -> Result<()> {
    use std::os::unix::fs::MetadataExt;
    let common = absolute(repo, repo.common_dir());
    let git_dir = absolute(repo, repo.git_dir());
    let root = if resource.starts_with(&git_dir) {
        &git_dir
    } else {
        &common
    };
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

fn object_member(name: &[u8]) -> bool {
    (name.starts_with(b"pack/pack-") && (name.ends_with(b".pack") || name.ends_with(b".idx")))
        || (name.len() > 3
            && name[2] == b'/'
            && name[..2].iter().all(u8::is_ascii_hexdigit)
            && name[3..].iter().all(u8::is_ascii_hexdigit))
}
fn digest_file(path: &Path) -> Result<String> {
    let mut hash = gix::hash::hasher(gix::hash::Kind::Sha1);
    let mut file = fs::File::open(path)?;
    let mut buffer = [0; 65536];
    loop {
        let count = file.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        hash.update(&buffer[..count]);
    }
    Ok(hash.try_finalize()?.to_string())
}
pub(super) fn check_keeps(repo: &gix::Repository, pull: &Pull) -> Result<()> {
    use std::os::unix::fs::MetadataExt;
    validate_object_boundaries(repo)?;
    for keep in &pull.keeps {
        let resource = path(&keep.path)?;
        match fs::symlink_metadata(&resource) {
            Ok(metadata) => {
                ensure!(
                    metadata.is_file()
                        && metadata.dev() == keep.device
                        && metadata.ino() == keep.inode
                        && digest_file(&resource)? == keep.checksum,
                    "foreign pack protection {}; evidence retained",
                    resource.display()
                );
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
    }
    Ok(())
}

pub(super) fn cleanup_keeps(repo: &gix::Repository, pull: &Pull) -> Result<()> {
    check_keeps(repo, pull)?;
    for keep in &pull.keeps {
        let resource = path(&keep.path)?;
        if present(&resource)? {
            fs::remove_file(&resource)?;
            sync_dir(resource.parent().context("pack protection parent")?)?;
        }
    }
    Ok(())
}

fn remote_url(repo: &gix::Repository, name: &str) -> Result<Vec<u8>> {
    Ok(crate::repository::remote::by_name_or_url(repo, Some(name))?
        .url(gix::remote::Direction::Fetch)
        .context("remote fetch URL required")?
        .to_bstring()
        .to_vec())
}

fn display_remote(bytes: &[u8]) -> Result<String> {
    // Credentials may be pinned privately, but never copy a password to public Git metadata.
    Ok(gix::url::parse(bytes.as_bstr())?.to_string())
}

fn tracking_oid(repo: &gix::Repository, name: &BStr) -> Result<Option<String>> {
    repo.try_find_reference(name)?
        .map(|reference| {
            ensure!(
                matches!(reference.target(), gix::refs::TargetRef::Object(_)),
                "symbolic tracking references require Git"
            );
            Ok(reference.id().to_string())
        })
        .transpose()
}
