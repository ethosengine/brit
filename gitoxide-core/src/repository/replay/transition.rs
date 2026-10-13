//! Replay publication reuses the switch engine's guarded old/desired writes.
use anyhow::{Context, Result, ensure};
use gix::bstr::ByteSlice;
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeSet,
    fs,
    os::unix::fs::MetadataExt,
    path::{Path, PathBuf},
};

use crate::repository::switch::{self, Change, State};

/// These are separate durable decisions, never inferred from a surviving HEAD.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(super) enum Kind {
    Checkout,
    Stop,
    Step,
    Skip,
    Final,
    Abort,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Transition {
    pub kind: Kind,
    pub work: Vec<Change>,
    pub metadata: Vec<Change>,
    pub branch: Option<(Vec<u8>, String)>,
    pub final_tip: Option<String>,
    pub packed_refs: State,
}

impl Transition {
    /// The caller has persisted this exact decision and holds the shared guard.
    /// No new desired bytes or timestamps are generated during roll-forward.
    pub(super) fn apply(
        &self,
        repo: &gix::Repository,
        staging: &Path,
        decide: impl FnOnce() -> Result<()>,
    ) -> Result<()> {
        let root = repo.workdir().context("worktree required")?;
        let mut started = self.started(staging)?;
        ensure!(
            self.work.iter().map(|c| &c.path).collect::<BTreeSet<_>>().len() == self.work.len()
                && self.metadata.iter().map(|c| &c.path).collect::<BTreeSet<_>>().len() == self.metadata.len(),
            "duplicate replay transition resource"
        );
        self.validate_branch(repo)?;
        self.verify_work(root, &started)?;
        for change in &self.metadata {
            self.validate_metadata(repo, change)?;
        }
        let mut resources: Vec<PathBuf> = self
            .metadata
            .iter()
            .map(|change| gix::path::from_bstr(change.path.as_bstr()).map(|p| p.into_owned()))
            .collect::<gix::Result<_>>()?;
        if let Some((name, _)) = &self.branch {
            resources.push(repo.common_dir().join(gix::path::from_bstr(name.as_bstr())?.as_ref()));
            resources.push(repo.common_dir().join("packed-refs"));
        }
        resources.sort();
        resources.dedup();
        reclaim_locks(staging, &resources)?;
        let owner = format!("brit-rebase-owner:{}", switch::digest(&serde_json::to_vec(self)?)?);
        let mut ordinal = 0;
        let mut locks = switch::acquire_recorded_locks(staging, &resources, &owner, |lock| {
            let payload = serde_json::to_vec(&lock)?;
            let bytes = serde_json::to_vec(&LockReceipt {
                checksum: switch::digest(&payload)?,
                payload,
            })?;
            switch::private_write(&staging.join(format!("metadata-lock-{ordinal}.json")), &bytes)?;
            switch::sync_dir(staging)?;
            ordinal += 1;
            Ok(())
        })?;
        switch::verify_locks(&locks)?;
        self.verify_work(root, &started)?;
        for change in &self.metadata {
            self.validate_metadata(repo, change)?;
        }
        self.validate_branch(repo)?;
        decide()?;
        let mut ordered: Vec<_> = self.work.iter().collect();
        ordered.sort_by_key(|change| std::cmp::Reverse(change.path.len()));
        for change in &ordered {
            let actual = switch::snapshot(&root.join(gix::path::from_bstr(change.path.as_bstr())?.as_ref()))?;
            let must_remove = self.replaced_ancestor(&change.path)
                || change.after == State::Absent
                || (actual != State::Absent
                    && actual != change.after
                    && (matches!(change.after, State::Directory { .. }) || matches!(actual, State::Directory { .. })));
            if must_remove && actual != State::Absent {
                self.mark_started(staging, &mut started, &change.path)?;
                switch::publish_file(staging, root, change, &State::Absent)?;
                switch::checkpoint("replay-worktree-removal")?;
            }
        }
        ordered.reverse();
        for change in ordered {
            if change.after != State::Absent {
                self.mark_started(staging, &mut started, &change.path)?;
                let mut publishing = (*change).clone();
                let actual = switch::snapshot(&root.join(gix::path::from_bstr(change.path.as_bstr())?.as_ref()))?;
                if actual == State::Absent && self.replaced_ancestor(&change.path) && started.contains(&change.path) {
                    publishing.before = State::Absent;
                }
                switch::publish_file(staging, root, &publishing, &change.after)?;
                switch::checkpoint("replay-worktree-publication")?;
            }
        }
        for change in &self.metadata {
            self.validate_metadata(repo, change)?;
            switch::verify_locks(&locks)?;
            switch::publish_metadata_at(repo, staging, change, &mut locks)?;
            switch::checkpoint("replay-metadata-publication")?;
        }
        switch::verify_locks(&locks)?;
        drop(locks);
        switch::sync_dir(staging)?;
        Ok(())
    }

    fn replaced_ancestor(&self, path: &[u8]) -> bool {
        self.work.iter().any(|ancestor| {
            ancestor.before != ancestor.after
                && (matches!(ancestor.before, State::Directory { .. })
                    || matches!(ancestor.after, State::Directory { .. }))
                && path.starts_with(&ancestor.path)
                && path.get(ancestor.path.len()) == Some(&b'/')
        })
    }

    fn verify_work(&self, root: &Path, started: &BTreeSet<Vec<u8>>) -> Result<()> {
        for change in &self.work {
            let actual = switch::snapshot(&root.join(gix::path::from_bstr(change.path.as_bstr())?.as_ref()))?;
            if actual == State::Absent && self.replaced_ancestor(&change.path) && started.contains(&change.path) {
                // Only this declared descendant may be temporarily absent while
                // its exact parent directory inode is replaced.
                continue;
            }
            switch::verify_files(root, std::slice::from_ref(change), started)?;
        }
        Ok(())
    }

    fn started(&self, staging: &Path) -> Result<BTreeSet<Vec<u8>>> {
        let path = staging.join("started.json");
        match fs::symlink_metadata(&path) {
            Ok(metadata) => {
                use std::os::unix::fs::PermissionsExt;
                ensure!(
                    metadata.is_file()
                        && metadata.uid() == fs::metadata(staging)?.uid()
                        && metadata.permissions().mode().trailing_zeros() >= 6,
                    "unsafe replay started ledger"
                );
                let envelope: LockReceipt = serde_json::from_slice(&fs::read(path)?)?;
                ensure!(
                    switch::digest(&envelope.payload)? == envelope.checksum,
                    "corrupt replay started ledger"
                );
                let (owner, paths): (String, BTreeSet<Vec<u8>>) = serde_json::from_slice(&envelope.payload)?;
                ensure!(
                    owner == switch::digest(&serde_json::to_vec(self)?)?
                        && paths.iter().all(|path| self.work.iter().any(|c| &c.path == path)),
                    "foreign replay started ledger"
                );
                switch::sync_path(&staging.join("started.json"))?;
                switch::sync_dir(staging)?;
                Ok(paths)
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(BTreeSet::new()),
            Err(error) => Err(error.into()),
        }
    }

    fn mark_started(&self, staging: &Path, started: &mut BTreeSet<Vec<u8>>, path: &[u8]) -> Result<()> {
        if started.insert(path.to_vec()) {
            let payload = serde_json::to_vec(&(switch::digest(&serde_json::to_vec(self)?)?, &started))?;
            let bytes = serde_json::to_vec(&LockReceipt {
                checksum: switch::digest(&payload)?,
                payload,
            })?;
            let pending = staging.join("started.pending");
            if pending.exists() {
                ensure!(fs::read(&pending)? == bytes, "foreign pending started ledger");
                switch::sync_path(&pending)?;
            } else {
                switch::private_write(&pending, &bytes)?;
            }
            fs::rename(pending, staging.join("started.json"))?;
            switch::sync_dir(staging)?;
            switch::checkpoint("replay-file-started")?;
        }
        Ok(())
    }

    pub(super) fn cleanup_inventory(&self, repo: &gix::Repository, staging: &Path) -> Result<BTreeSet<Vec<u8>>> {
        let mut resources = self
            .metadata
            .iter()
            .map(|change| gix::path::from_bstr(change.path.as_bstr()).map(|p| p.into_owned()))
            .collect::<gix::Result<Vec<_>>>()?;
        if let Some((name, _)) = &self.branch {
            resources.push(repo.common_dir().join(gix::path::from_bstr(name.as_bstr())?.as_ref()));
            resources.push(repo.common_dir().join("packed-refs"));
        }
        resources.sort();
        resources.dedup();
        let mut allowed = BTreeSet::from([b"owner".to_vec(), b"plan.json".to_vec()]);
        for change in &self.work {
            let digest = switch::digest(&change.path)?;
            for prefix in ["new-dir-", "saved-dir-"] {
                let name = format!("{prefix}{digest}");
                let path = staging.join(&name);
                let actual = switch::snapshot(&path)?;
                if actual != State::Absent {
                    ensure!(
                        matches!(actual, State::Directory { .. })
                            && (actual == change.before || actual == change.after)
                            && fs::read_dir(&path)?.next().is_none(),
                        "foreign saved replay directory"
                    );
                    allowed.insert(name.into_bytes());
                }
            }
            for prefix in ["directory-", "restore-directory-"] {
                let name = format!("{prefix}{digest}.json");
                let path = staging.join(&name);
                if path.exists() {
                    let envelope: LockReceipt = serde_json::from_slice(&fs::read(path)?)?;
                    ensure!(
                        switch::digest(&envelope.payload)? == envelope.checksum,
                        "corrupt directory receipt"
                    );
                    let (value, state): (Vec<u8>, State) = serde_json::from_slice(&envelope.payload)?;
                    ensure!(
                        state == change.after && (prefix == "restore-directory-" || value == change.path),
                        "foreign directory receipt"
                    );
                    allowed.insert(name.into_bytes());
                }
            }
        }
        self.started(staging)?;
        if staging.join("started.json").exists() {
            allowed.insert(b"started.json".to_vec());
        }
        ensure!(
            !staging.join("started.pending").exists(),
            "pending replay started receipt before cleanup"
        );
        for (ordinal, resource) in resources.iter().enumerate() {
            let name = format!("metadata-lock-{ordinal}.json");
            let path = staging.join(&name);
            if path.exists() {
                let envelope: LockReceipt = serde_json::from_slice(&fs::read(&path)?)?;
                ensure!(
                    switch::digest(&envelope.payload)? == envelope.checksum,
                    "corrupt cleanup lock receipt"
                );
                let lock: switch::Lock = serde_json::from_slice(&envelope.payload)?;
                let mut external = resource.as_os_str().to_os_string();
                external.push(".lock");
                ensure!(
                    gix::path::from_bstr(lock.path.as_bstr())?.as_ref() == Path::new(&external),
                    "foreign cleanup lock resource"
                );
                allowed.insert(name.into_bytes());
            }
            for path in [staging.join(format!("guard-{ordinal}.lock")), {
                let mut external = resource.as_os_str().to_os_string();
                external.push(".lock");
                PathBuf::from(external)
            }] {
                match fs::symlink_metadata(path) {
                    Ok(_) => anyhow::bail!("replay metadata lock remains; recover it before cleanup"),
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                    Err(error) => return Err(error.into()),
                }
            }
        }
        Ok(allowed)
    }

    fn validate_metadata(&self, repo: &gix::Repository, change: &Change) -> Result<()> {
        let path = gix::path::from_bstr(change.path.as_bstr())?;
        ensure!(path.is_absolute(), "replay metadata must be pinned absolutely");
        let allowed = [
            repo.index_path(),
            repo.git_dir().join("HEAD"),
            repo.git_dir().join("ORIG_HEAD"),
            repo.git_dir().join("logs/HEAD"),
        ];
        let branch = self
            .branch
            .as_ref()
            .map(|(name, _)| {
                let relative = gix::path::from_bstr(name.as_bstr())?;
                Ok::<_, gix::Error>(
                    path.as_ref() == repo.common_dir().join(relative.as_ref())
                        || path.as_ref() == repo.common_dir().join("logs").join(relative.as_ref()),
                )
            })
            .transpose()?
            .unwrap_or(false);
        ensure!(
            allowed.iter().any(|p| p == path.as_ref()) || (self.kind == Kind::Final && branch),
            "unowned replay metadata resource"
        );
        let parent = path.parent().context("metadata parent")?;
        let metadata = fs::symlink_metadata(parent)?;
        ensure!(
            metadata.is_dir() && fs::canonicalize(parent)? == parent,
            "unsafe metadata ancestor"
        );
        let current = switch::snapshot(&path)?;
        ensure!(
            current == change.before || current == change.after,
            "third-state replay metadata refuses recovery"
        );
        Ok(())
    }

    fn validate_branch(&self, repo: &gix::Repository) -> Result<()> {
        ensure!(
            switch::snapshot(&repo.common_dir().join("packed-refs"))? == self.packed_refs,
            "packed reference store changed during replay transition"
        );
        if let Some((name, original)) = &self.branch {
            let reference = repo.find_reference(name.as_bstr())?;
            ensure!(
                matches!(reference.target(), gix::refs::TargetRef::Object(_)),
                "original branch became symbolic"
            );
            let id = reference.id().to_string();
            ensure!(
                id == *original || (self.kind == Kind::Final && self.final_tip.as_ref() == Some(&id)),
                "effective original branch moved during replay transition"
            );
        }
        Ok(())
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct LockReceipt {
    payload: Vec<u8>,
    checksum: String,
}

fn reclaim_locks(staging: &Path, resources: &[PathBuf]) -> Result<()> {
    let mut owned = Vec::new();
    // Validate every external lock before removing any: a replacement is not
    // ours even if its contents happen to match the old ownership token.
    for (ordinal, resource) in resources.iter().enumerate() {
        let receipt = staging.join(format!("metadata-lock-{ordinal}.json"));
        let anchor = staging.join(format!("guard-{ordinal}.lock"));
        let mut external = resource.as_os_str().to_os_string();
        external.push(".lock");
        let external = PathBuf::from(external);
        match fs::symlink_metadata(&receipt) {
            Ok(metadata) => {
                use std::os::unix::fs::PermissionsExt;
                ensure!(
                    metadata.is_file()
                        && metadata.permissions().mode().trailing_zeros() >= 6
                        && metadata.uid() == fs::metadata(staging)?.uid(),
                    "unsafe replay lock receipt"
                );
                let envelope: LockReceipt = serde_json::from_slice(&fs::read(&receipt)?)?;
                ensure!(
                    switch::digest(&envelope.payload)? == envelope.checksum,
                    "corrupt replay lock receipt"
                );
                let lock: switch::Lock = serde_json::from_slice(&envelope.payload)?;
                ensure!(
                    gix::path::from_bstr(lock.path.as_bstr())?.as_ref() == external,
                    "replay lock receipt names another resource"
                );
                for path in [&anchor, &external] {
                    match fs::symlink_metadata(path) {
                        Ok(metadata) => ensure!(
                            metadata.is_file()
                                && metadata.dev() == lock.device
                                && metadata.ino() == lock.inode
                                && switch::digest(&fs::read(path)?)? == lock.checksum,
                            "foreign replay lock; ownership receipt retained"
                        ),
                        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                        Err(error) => return Err(error.into()),
                    }
                }
                owned.push((receipt, anchor, external));
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                for path in [&anchor, &external] {
                    match fs::symlink_metadata(path) {
                        Ok(_) => anyhow::bail!("unrecorded replay lock; inspect without reclaiming"),
                        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                        Err(error) => return Err(error.into()),
                    }
                }
            }
            Err(error) => return Err(error.into()),
        }
    }
    for (receipt, anchor, external) in owned {
        for path in [&external, &anchor] {
            match fs::remove_file(path) {
                Ok(()) => switch::sync_dir(path.parent().context("lock parent")?)?,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => return Err(error.into()),
            }
        }
        fs::remove_file(receipt)?;
        switch::sync_dir(staging)?;
    }
    Ok(())
}
