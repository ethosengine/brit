//! Terminal replay cleanup. A sibling receipt outlives the last journal member.
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    os::unix::fs::{MetadataExt, PermissionsExt},
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, ensure};
use gix::bstr::ByteSlice;
use serde::{Deserialize, Serialize};

use crate::repository::switch;

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Member {
    path: Vec<u8>,
    device: u64,
    inode: u64,
    mode: u32,
    directory: bool,
    checksum: Option<String>,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Ledger {
    version: u32,
    nonce: String,
    owner: Vec<u8>,
    common: Vec<u8>,
    root: Vec<u8>,
    identities: Vec<(u64, u64)>,
    directory: (u64, u64),
    terminal: Vec<u8>,
    refs: Vec<(String, String)>,
    retained_path: Vec<u8>,
    retained_digest: String,
    members: Vec<Member>,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Envelope {
    payload: Vec<u8>,
    checksum: String,
}

fn raw(path: &Path) -> Result<Vec<u8>> {
    Ok(gix::path::into_bstr(path)?.to_vec())
}
fn identities(repo: &gix::Repository) -> Result<Vec<(u64, u64)>> {
    [
        repo.git_dir(),
        repo.common_dir(),
        repo.workdir().context("worktree required")?,
    ]
    .into_iter()
    .map(|p| {
        let m = fs::symlink_metadata(p)?;
        ensure!(m.is_dir(), "cleanup repository directory changed");
        Ok((m.dev(), m.ino()))
    })
    .collect()
}
fn present(path: &Path) -> Result<bool> {
    match fs::symlink_metadata(path) {
        Ok(_) => Ok(true),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(e) => Err(e.into()),
    }
}
fn relative(bytes: &[u8]) -> Result<PathBuf> {
    let path = gix::path::from_bstr(bytes.as_bstr())?.into_owned();
    ensure!(
        !path.as_os_str().is_empty() && path.components().all(|c| matches!(c, std::path::Component::Normal(_))),
        "invalid cleanup member path"
    );
    Ok(path)
}
fn inventory(root: &Path, directory: &Path, out: &mut BTreeSet<Vec<u8>>) -> Result<()> {
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        let metadata = fs::symlink_metadata(entry.path())?;
        ensure!(metadata.is_file() || metadata.is_dir(), "unsafe cleanup journal member");
        out.insert(raw(entry.path().strip_prefix(root)?)?);
        if metadata.is_dir() {
            inventory(root, &entry.path(), out)?;
        }
    }
    Ok(())
}
fn sync_tree(path: &Path) -> Result<()> {
    if !present(path)? {
        return Ok(());
    }
    let metadata = fs::symlink_metadata(path)?;
    if metadata.is_dir() {
        for entry in fs::read_dir(path)? {
            sync_tree(&entry?.path())?;
        }
        switch::sync_dir(path)
    } else {
        ensure!(metadata.is_file(), "unsafe cleanup synchronization path");
        switch::sync_path(path)
    }
}

/// Prove that every object reachable from a released ref is independently
/// recoverable. This proof reads only the copy, so aggressive GC cannot weaken it.
fn validate_release(ledger: &Ledger) -> Result<()> {
    let path = gix::path::from_bstr(ledger.retained_path.as_bstr())?;
    let copy = super::retained::inspect(&path)?;
    ensure!(
        copy.receipt.digest == ledger.retained_digest && copy.manifest.nonce == ledger.nonce,
        "retained-copy release proof changed"
    );
    let mut objects = BTreeMap::new();
    for (ordinal, entry) in copy.manifest.entries.iter().enumerate() {
        if let super::retained::Kind::Object { id, kind } = &entry.kind {
            let id = gix::ObjectId::from_hex(id.as_bytes())?;
            objects.insert(id, (gix::objs::Kind::from_bytes(kind.as_bytes())?, ordinal));
        }
    }
    let mut pending = ledger
        .refs
        .iter()
        .map(|(_, id)| gix::ObjectId::from_hex(id.as_bytes()))
        .collect::<std::result::Result<Vec<_>, _>>()?;
    let mut seen = BTreeSet::new();
    while let Some(id) = pending.pop() {
        if !seen.insert(id) {
            continue;
        }
        let (kind, ordinal) = objects
            .get(&id)
            .context("retained copy lacks a released object closure")?;
        let data = copy.payload(*ordinal)?;
        match kind {
            gix::objs::Kind::Commit => {
                let commit = gix::objs::CommitRef::from_bytes(&data, id.kind())?;
                pending.push(commit.tree());
                pending.extend(commit.parents());
            }
            gix::objs::Kind::Tree => {
                for entry in gix::objs::TreeRefIter::from_bytes(&data, id.kind()) {
                    let entry = entry?;
                    if !entry.mode.is_commit() {
                        pending.push(entry.oid.to_owned());
                    }
                }
            }
            gix::objs::Kind::Blob => {}
            gix::objs::Kind::Tag => anyhow::bail!("unexpected retained tag release root"),
        }
    }
    Ok(())
}

/// Caller holds the shared mutation guard, validates a terminal parent decision,
/// and supplies its schema-derived exact member inventory (including directories).
pub(super) fn run(
    repo: &gix::Repository,
    parent: &Path,
    nonce: &str,
    terminal: &[u8],
    refs: &[(String, String)],
    allowed: &BTreeSet<Vec<u8>>,
    retained: &super::retained::Receipt,
) -> Result<()> {
    ensure!(
        nonce.len() == 40 && nonce.bytes().all(|b| b.is_ascii_hexdigit()),
        "invalid cleanup nonce"
    );
    ensure!(parent == repo.git_dir().join("brit-rebase"), "foreign cleanup parent");
    let receipt = repo.git_dir().join(format!("brit-rebase-cleanup-{nonce}.json"));
    if present(&receipt)? {
        return resume(repo, &receipt);
    }
    let mut observed = BTreeSet::new();
    inventory(parent, parent, &mut observed)?;
    ensure!(&observed == allowed, "unknown or missing journal member before cleanup");
    ensure!(
        fs::read(parent.join("state.json"))? == terminal,
        "terminal replay state changed"
    );
    let uid = fs::metadata(repo.git_dir())?.uid();
    let directory = fs::symlink_metadata(parent)?;
    ensure!(
        directory.is_dir() && directory.uid() == uid && directory.permissions().mode().trailing_zeros() >= 6,
        "unsafe cleanup parent"
    );
    let mut members = Vec::new();
    for name in allowed {
        let path = parent.join(relative(name)?);
        let m = fs::symlink_metadata(&path)?;
        ensure!(m.uid() == uid && (m.is_file() || m.is_dir()), "foreign cleanup member");
        members.push(Member {
            path: name.clone(),
            device: m.dev(),
            inode: m.ino(),
            mode: m.mode(),
            directory: m.is_dir(),
            checksum: if m.is_file() {
                Some(switch::digest(&fs::read(path)?)?)
            } else {
                None
            },
        });
    }
    sync_tree(parent)?;
    let ledger = Ledger {
        version: 1,
        nonce: nonce.into(),
        owner: raw(repo.git_dir())?,
        common: raw(repo.common_dir())?,
        root: raw(repo.workdir().context("worktree required")?)?,
        identities: identities(repo)?,
        directory: (directory.dev(), directory.ino()),
        terminal: terminal.to_vec(),
        refs: refs.to_vec(),
        retained_path: raw(&retained.path)?,
        retained_digest: retained.digest.clone(),
        members,
    };
    let payload = serde_json::to_vec(&ledger)?;
    let bytes = serde_json::to_vec(&Envelope {
        checksum: switch::digest(&payload)?,
        payload,
    })?;
    switch::private_write(&receipt, &bytes)?;
    switch::sync_dir(repo.git_dir())?;
    switch::checkpoint("replay-cleanup-receipt-durable")?;
    resume(repo, &receipt)
}

/// Resume only the explicitly named receipt; never sweep similarly named paths.
fn read_receipt(repo: &gix::Repository, receipt: &Path) -> Result<(Ledger, bool)> {
    let m = fs::symlink_metadata(receipt)?;
    let uid = fs::metadata(repo.git_dir())?.uid();
    ensure!(
        m.is_file() && m.uid() == uid && m.permissions().mode().trailing_zeros() >= 6,
        "unsafe cleanup receipt"
    );
    let envelope: Envelope = serde_json::from_slice(&fs::read(receipt)?)?;
    ensure!(
        switch::digest(&envelope.payload)? == envelope.checksum,
        "corrupt cleanup receipt"
    );
    let ledger: Ledger = serde_json::from_slice(&envelope.payload)?;
    ensure!(
        ledger.version == 1 && ledger.nonce.len() == 40 && ledger.nonce.bytes().all(|b| b.is_ascii_hexdigit()),
        "unsupported cleanup receipt"
    );
    let retired = receipt
        == repo
            .git_dir()
            .join(format!("brit-rebase-cleanup-{}.retired.json", ledger.nonce));
    ensure!(
        retired
            || receipt
                == repo
                    .git_dir()
                    .join(format!("brit-rebase-cleanup-{}.json", ledger.nonce)),
        "foreign cleanup receipt path"
    );
    ensure!(
        ledger.owner == raw(repo.git_dir())?
            && ledger.common == raw(repo.common_dir())?
            && ledger.root == raw(repo.workdir().context("worktree required")?)?
            && ledger.identities == identities(repo)?,
        "cleanup repository identity changed"
    );
    Ok((ledger, retired))
}

/// Retired receipts no longer reserve the repository. Unknown cleanup files
/// still refuse mutation, and a remaining tombstone must have its pinned inode.
pub(crate) fn ensure_cleanup_retired(git_dir: &Path) -> Result<()> {
    let repo = gix::open(git_dir)?;
    for entry in fs::read_dir(git_dir)? {
        let entry = entry?;
        let name = entry.file_name();
        let name = name.as_encoded_bytes();
        if !name.starts_with(b"brit-rebase-cleanup-") {
            continue;
        }
        let path = entry.path();
        let receipt = if name.ends_with(b".retired.json") {
            path.clone()
        } else {
            let name = std::str::from_utf8(name)?;
            git_dir.join(format!("{name}.retired.json"))
        };
        let (ledger, retired) = read_receipt(&repo, &receipt)
            .with_context(|| format!("replay cleanup pending at {}; recover its receipt", path.display()))?;
        ensure!(retired, "replay cleanup has not retired its reservation");
        let tombstone = git_dir.join(format!("brit-rebase-cleanup-{}", ledger.nonce));
        ensure!(path == receipt || path == tombstone, "unknown cleanup member");
        if present(&tombstone)? {
            let m = fs::symlink_metadata(tombstone)?;
            ensure!(
                m.is_dir() && (m.dev(), m.ino()) == ledger.directory,
                "foreign cleanup tombstone"
            );
        }
    }
    Ok(())
}

pub(super) fn resume(repo: &gix::Repository, receipt: &Path) -> Result<()> {
    let (ledger, retired) = read_receipt(repo, receipt)?;
    let uid = fs::metadata(repo.git_dir())?.uid();
    switch::sync_path(receipt)?;
    switch::sync_dir(repo.git_dir())?;
    validate_release(&ledger)?;
    let active = repo.git_dir().join("brit-rebase");
    let tombstone = repo.git_dir().join(format!("brit-rebase-cleanup-{}", ledger.nonce));
    let is_active = if retired {
        if let Some(nonce) = super::active_nonce(repo)? {
            ensure!(nonce != ledger.nonce, "retired replay still occupies active journal");
        }
        false
    } else {
        present(&active)?
    };
    ensure!(!(is_active && present(&tombstone)?), "ambiguous cleanup directories");
    let directory = if is_active { &active } else { &tombstone };
    let mut known = BTreeSet::new();
    for member in &ledger.members {
        relative(&member.path)?;
        ensure!(known.insert(member.path.clone()), "duplicate cleanup member");
    }
    if present(directory)? {
        let m = fs::symlink_metadata(directory)?;
        ensure!(
            m.is_dir() && m.uid() == uid && (m.dev(), m.ino()) == ledger.directory,
            "foreign cleanup directory"
        );
        let mut observed = BTreeSet::new();
        inventory(directory, directory, &mut observed)?;
        ensure!(
            observed.is_subset(&known) && (!is_active || observed == known),
            "unknown or prematurely removed cleanup member"
        );
        if is_active {
            ensure!(
                fs::read(directory.join("state.json"))? == ledger.terminal,
                "terminal state changed before cleanup"
            );
        }
        for member in &ledger.members {
            let path = directory.join(relative(&member.path)?);
            if !present(&path)? {
                continue;
            }
            let m = fs::symlink_metadata(&path)?;
            ensure!(
                m.uid() == uid
                    && m.dev() == member.device
                    && m.ino() == member.inode
                    && m.mode() == member.mode
                    && m.is_dir() == member.directory,
                "cleanup member ownership changed"
            );
            if let Some(checksum) = &member.checksum {
                ensure!(
                    m.is_file() && switch::digest(&fs::read(path)?)? == *checksum,
                    "cleanup member bytes changed"
                );
            }
        }
    }
    // Check every retained ref before releasing any. Each deletion then uses
    // gix's MustExistAndMatch transaction, including packed-ref semantics.
    for (label, id) in &ledger.refs {
        ensure!(
            !label.is_empty() && label.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-'),
            "invalid retained label"
        );
        if let Some(reference) =
            repo.try_find_reference(format!("refs/brit-rebase-retain/{}/{label}", ledger.nonce).as_str())?
        {
            ensure!(
                matches!(reference.target(), gix::refs::TargetRef::Object(_))
                    && reference.id() == gix::ObjectId::from_hex(id.as_bytes())?,
                "foreign retained ref during cleanup"
            );
        }
    }
    for (label, id) in &ledger.refs {
        if let Some(reference) =
            repo.try_find_reference(format!("refs/brit-rebase-retain/{}/{label}", ledger.nonce).as_str())?
        {
            ensure!(
                matches!(reference.target(), gix::refs::TargetRef::Object(_))
                    && reference.id() == gix::ObjectId::from_hex(id.as_bytes())?,
                "retained ref changed before cleanup deletion"
            );
            reference.delete()?;
        }
    }
    for path in [
        repo.common_dir().join("refs"),
        repo.common_dir().join("logs/refs"),
        repo.common_dir().join("packed-refs"),
    ] {
        sync_tree(&path)?;
    }
    switch::sync_dir(repo.common_dir())?;
    switch::checkpoint("replay-cleanup-retention-released")?;
    if is_active {
        fs::rename(&active, &tombstone)?;
        switch::sync_dir(repo.git_dir())?;
        switch::checkpoint("replay-cleanup-directory-renamed")?;
    }
    let retired_receipt = repo
        .git_dir()
        .join(format!("brit-rebase-cleanup-{}.retired.json", ledger.nonce));
    if !retired {
        ensure!(!present(&retired_receipt)?, "retired receipt already exists");
        fs::rename(receipt, &retired_receipt)?;
        switch::sync_dir(repo.git_dir())?;
    }
    switch::checkpoint("replay-cleanup-tombstoned")?;
    let mut members: Vec<_> = ledger.members.iter().collect();
    members.sort_by_key(|member| std::cmp::Reverse(member.path.len()));
    for member in members {
        let path = tombstone.join(relative(&member.path)?);
        if present(&path)? {
            if member.directory {
                fs::remove_dir(&path)?;
            } else {
                fs::remove_file(&path)?;
            }
            switch::sync_dir(path.parent().context("cleanup member parent")?)?;
            switch::checkpoint("replay-cleanup-member-removed")?;
        }
    }
    if present(&tombstone)? {
        fs::remove_dir(&tombstone)?;
        switch::sync_dir(repo.git_dir())?;
    }
    switch::checkpoint("replay-cleanup-last-member-removed")?;
    fs::remove_file(retired_receipt)?;
    switch::sync_dir(repo.git_dir())
}

/// Validate a cleanup receipt for read-only diagnostics, without touching its members.
pub(super) fn inspect(repo: &gix::Repository, receipt: &Path) -> Result<(bool, PathBuf)> {
    let (ledger, retired) = read_receipt(repo, receipt)?;
    Ok((
        retired,
        gix::path::from_bstr(ledger.retained_path.as_bstr())?.into_owned(),
    ))
}
