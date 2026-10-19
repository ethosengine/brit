//! Independent private copies. No active journal is needed to validate their bytes.
//! Logical paths are metadata only; every payload has an opaque numeric filename.
use std::{
    fs,
    os::unix::fs::{DirBuilderExt, MetadataExt, PermissionsExt},
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};

use crate::repository::switch;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub enum Area {
    Original,
    Before,
    Staged,
    Worktree,
    After,
    Messages,
    Invocations,
    Objects,
    Indexes,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub enum Kind {
    File,
    Symlink,
    Directory { device: u64, inode: u64 },
    Deleted,
    Object { id: String, kind: String },
}

/// The caller selects only operation-touched resources. This module never walks
/// a worktree or imports raw working files into the Git object database.
#[derive(Clone, Debug)]
pub struct Member {
    pub area: Area,
    pub path: Vec<u8>,
    pub stage: Option<u8>,
    pub kind: Kind,
    pub mode: u32,
    pub data: Vec<u8>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Entry {
    pub area: Area,
    pub path: Vec<u8>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stage: Option<u8>,
    pub kind: Kind,
    pub mode: u32,
    pub size: u64,
    pub checksum: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    pub version: u32,
    pub nonce: String,
    pub sequence: u64,
    pub owner_uid: u32,
    pub original: String,
    pub replay: String,
    pub observed: String,
    pub entries: Vec<Entry>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Envelope {
    payload: Vec<u8>,
    checksum: String,
}

#[derive(Clone, Debug)]
pub struct Receipt {
    pub path: PathBuf,
    pub digest: String,
}

/// A verified independent copy. Payloads are reread and checked on access so a
/// change after inspection cannot silently reach a later extraction.
pub struct Copy {
    pub receipt: Receipt,
    pub manifest: Manifest,
}

/// Resolve without following links. System ancestors such as `/tmp` may be
/// shared; the copy/destination itself has a separate ownership check.
fn checked_path(path: &Path) -> Result<PathBuf> {
    use std::path::Component;
    let absolute = if path.is_absolute() {
        path.to_owned()
    } else {
        std::env::current_dir()?.join(path)
    };
    let mut checked = PathBuf::new();
    for component in absolute.components() {
        match component {
            Component::RootDir => checked.push(component),
            Component::Normal(name) => {
                checked.push(name);
                ensure!(
                    !fs::symlink_metadata(&checked)?.file_type().is_symlink(),
                    "symlink in retained-copy path: {}",
                    checked.display()
                );
            }
            Component::CurDir => {}
            _ => anyhow::bail!("unsafe retained-copy path: {}", path.display()),
        }
    }
    Ok(checked)
}

fn check_directory(path: &Path, owner: u32) -> Result<()> {
    let metadata = fs::symlink_metadata(path)?;
    ensure!(
        metadata.is_dir() && metadata.uid() == owner && metadata.permissions().mode().trailing_zeros() >= 6,
        "unsafe retained-copy directory {}",
        path.display()
    );
    Ok(())
}

fn directory(path: &Path, owner: u32) -> Result<()> {
    match fs::DirBuilder::new().mode(0o700).create(path) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
        Err(error) => return Err(error.into()),
    }
    check_directory(path, owner)?;
    switch::sync_dir(path)?;
    switch::sync_dir(path.parent().context("retained directory parent")?)
}

fn read_member(path: &Path, owner: u32) -> Result<Vec<u8>> {
    let metadata = fs::symlink_metadata(path)?;
    ensure!(
        metadata.is_file() && metadata.uid() == owner && metadata.permissions().mode().trailing_zeros() >= 6,
        "unsafe retained-copy member {}",
        path.display()
    );
    Ok(fs::read(path)?)
}

fn persist_exact(path: &Path, owner: u32, bytes: &[u8]) -> Result<()> {
    match fs::symlink_metadata(path) {
        Ok(_) => ensure!(
            read_member(path, owner)? == bytes,
            "retained-copy source or member changed"
        ),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => switch::private_write(path, bytes)?,
        Err(error) => return Err(error.into()),
    }
    // A prior invocation may have created this name without synchronizing it.
    // Reuse must establish durability again before any restoration decision.
    switch::sync_path(path)?;
    switch::sync_dir(path.parent().context("retained member parent")?)
}

fn payload_name(ordinal: usize) -> String {
    format!("{ordinal:08}")
}

fn validate_member(kind: &Kind, data: &[u8], hash: gix::hash::Kind) -> Result<()> {
    match kind {
        Kind::Deleted | Kind::Directory { .. } => {
            ensure!(data.is_empty(), "directory or deletion cannot carry file bytes");
        }
        Kind::Object { id, kind } => {
            let id = gix::ObjectId::from_hex(id.as_bytes())?;
            let kind = gix::objs::Kind::from_bytes(kind.as_bytes())?;
            ensure!(
                id.kind() == hash && gix::objs::compute_hash(hash, kind, data)? == id,
                "retained Git object hash mismatch"
            );
        }
        Kind::File | Kind::Symlink => {}
    }
    Ok(())
}

fn validate_manifest(manifest: &Manifest) -> Result<gix::hash::Kind> {
    ensure!(manifest.version == 1, "unsupported retained-copy schema");
    ensure!(
        manifest.nonce.len() == 40 && manifest.nonce.as_bytes().iter().all(u8::is_ascii_hexdigit),
        "invalid retained-copy operation nonce"
    );
    let original = gix::ObjectId::from_hex(manifest.original.as_bytes())?;
    for id in [&manifest.replay, &manifest.observed] {
        ensure!(
            gix::ObjectId::from_hex(id.as_bytes())?.kind() == original.kind(),
            "mixed retained object hashes"
        );
    }
    let mut paths = std::collections::BTreeSet::new();
    for entry in &manifest.entries {
        ensure!(
            entry
                .stage
                .is_none_or(|stage| entry.area == Area::Staged && (1..=3).contains(&stage)),
            "invalid retained conflict stage"
        );
        ensure!(
            paths.insert((entry.area.clone(), entry.path.clone(), entry.stage)),
            "duplicate retained logical member"
        );
    }
    Ok(original.kind())
}

/// Resume only a byte-identical capture at the same nonce/sequence. The caller
/// must durably record skip/abort intent and validate live sources before calling.
/// The returned receipt alone licenses a later restoration decision.
pub fn capture(
    repo: &gix::Repository,
    nonce: &str,
    sequence: u64,
    original: gix::ObjectId,
    replay: gix::ObjectId,
    observed: gix::ObjectId,
    members: &[Member],
) -> Result<Receipt> {
    let owner = fs::metadata(repo.git_dir())?.uid();
    let manifest = Manifest {
        version: 1,
        nonce: nonce.into(),
        sequence,
        owner_uid: owner,
        original: original.to_string(),
        replay: replay.to_string(),
        observed: observed.to_string(),
        entries: members
            .iter()
            .map(|member| {
                Ok(Entry {
                    area: member.area.clone(),
                    path: member.path.clone(),
                    stage: member.stage,
                    kind: member.kind.clone(),
                    mode: member.mode,
                    size: member.data.len() as u64,
                    checksum: switch::digest(&member.data)?,
                })
            })
            .collect::<Result<_>>()?,
    };
    let hash = validate_manifest(&manifest)?;
    for member in members {
        validate_member(&member.kind, &member.data, hash)?;
    }
    let payload = serde_json::to_vec(&manifest)?;
    let digest = switch::digest(&payload)?;
    let bytes = serde_json::to_vec(&Envelope {
        payload,
        checksum: digest.clone(),
    })?;
    let root = fs::canonicalize(repo.git_dir())?.join("brit-rebase-retained");
    directory(&root, owner)?;
    let operation = root.join(nonce);
    directory(&operation, owner)?;
    let leaf = operation.join(sequence.to_string());
    directory(&leaf, owner)?;
    // A completed copy is immutable. Refuse a different capture before adding
    // even one member, which would otherwise invalidate the existing good copy.
    match fs::symlink_metadata(leaf.join("COPY_COMPLETE")) {
        Ok(_) => ensure!(
            inspect(&leaf)?.receipt.digest == digest,
            "retained copy already completes a different capture"
        ),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error.into()),
    }
    match fs::symlink_metadata(leaf.join("manifest.json")) {
        Ok(_) => ensure!(
            read_member(&leaf.join("manifest.json"), owner)? == bytes,
            "retained-copy source changed"
        ),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error.into()),
    }
    // Pin all source digests before the first payload, including sources not yet
    // copied when an interruption occurs. COPY_COMPLETE distinguishes intent
    // from a copy that has actually been verified and synchronized.
    persist_exact(&leaf.join("manifest.json"), owner, &bytes)?;
    switch::checkpoint("replay-retained-manifest")?;
    for (ordinal, member) in members.iter().enumerate() {
        persist_exact(&leaf.join(payload_name(ordinal)), owner, &member.data)?;
        switch::checkpoint("replay-retained-member")?;
    }
    persist_exact(&leaf.join("COPY_COMPLETE"), owner, format!("{digest}\n").as_bytes())?;
    for path in [&leaf, &operation, &root, &fs::canonicalize(repo.git_dir())?] {
        switch::sync_dir(path)?;
    }
    switch::checkpoint("replay-copy-complete")?;
    let copy = inspect(&leaf)?;
    Ok(copy.receipt)
}

/// Inspect only: never repair a copy, create files, follow a payload symlink, or
/// require an active journal. Incomplete copies cannot authorize restoration.
pub fn inspect(path: &Path) -> Result<Copy> {
    let checked = checked_path(path)?;
    let path = checked.as_path();
    let metadata = fs::symlink_metadata(path)?;
    let owner = metadata.uid();
    ensure!(
        owner == rustix::process::getuid().as_raw(),
        "retained copy belongs to another user"
    );
    check_directory(path, owner)?;
    let envelope: Envelope = serde_json::from_slice(&read_member(&path.join("manifest.json"), owner)?)?;
    ensure!(
        switch::digest(&envelope.payload)? == envelope.checksum,
        "retained manifest checksum mismatch"
    );
    let manifest: Manifest = serde_json::from_slice(&envelope.payload)?;
    let hash = validate_manifest(&manifest)?;
    ensure!(manifest.owner_uid == owner, "retained-copy ownership mismatch");
    ensure!(
        read_member(&path.join("COPY_COMPLETE"), owner)? == format!("{}\n", envelope.checksum).as_bytes(),
        "retained copy is not complete"
    );
    let mut expected = std::collections::BTreeSet::from(["manifest.json".to_owned(), "COPY_COMPLETE".to_owned()]);
    for (ordinal, entry) in manifest.entries.iter().enumerate() {
        let name = payload_name(ordinal);
        expected.insert(name.clone());
        let data = read_member(&path.join(name), owner)?;
        ensure!(
            data.len() as u64 == entry.size && switch::digest(&data)? == entry.checksum,
            "retained payload checksum mismatch"
        );
        validate_member(&entry.kind, &data, hash)?;
    }
    for entry in fs::read_dir(path)? {
        let name = entry?.file_name();
        ensure!(
            name.to_str().is_some_and(|name| expected.contains(name)),
            "unknown retained-copy member"
        );
    }
    Ok(Copy {
        receipt: Receipt {
            path: checked,
            digest: envelope.checksum,
        },
        manifest,
    })
}

impl Copy {
    /// Lossless output even when the enclosing repository path is not UTF-8.
    pub fn inspection_json(&self) -> Result<String> {
        use std::os::unix::ffi::OsStrExt;
        Ok(serde_json::to_string_pretty(&serde_json::json!({
            "copy":self.receipt.path.to_str(),
            "copyPathBytes":self.receipt.path.as_os_str().as_bytes(),
            "receipt":self.receipt.digest,"manifest":self.manifest
        }))?)
    }

    pub fn payload(&self, ordinal: usize) -> Result<Vec<u8>> {
        checked_path(&self.receipt.path)?;
        check_directory(&self.receipt.path, self.manifest.owner_uid)?;
        let entry = self
            .manifest
            .entries
            .get(ordinal)
            .context("unknown retained payload ordinal")?;
        let data = read_member(&self.receipt.path.join(payload_name(ordinal)), self.manifest.owner_uid)?;
        ensure!(
            data.len() as u64 == entry.size && switch::digest(&data)? == entry.checksum,
            "retained payload changed"
        );
        validate_member(&entry.kind, &data, validate_manifest(&self.manifest)?)?;
        Ok(data)
    }
}

/// Export verified payloads into an existing empty owned directory. Logical
/// Git paths remain lossless metadata; numeric output names cannot escape it.
/// Links are ordinary payload files, never recreated or followed.
pub fn extract(copy: &Path, destination: &Path) -> Result<Receipt> {
    let copy = inspect(copy)?;
    for entry in &copy.manifest.entries {
        ensure!(
            !entry.path.is_empty()
                && !entry.path.starts_with(b"/")
                && !entry.path.contains(&0)
                && !entry.path.split(|byte| *byte == b'/').any(|part| part == b".."),
            "unsafe retained logical path"
        );
    }
    let destination = checked_path(destination)?;
    let metadata = fs::symlink_metadata(&destination)?;
    ensure!(
        metadata.is_dir() && metadata.uid() == rustix::process::getuid().as_raw() && metadata.mode() & 0o022 == 0,
        "extraction destination must be an owned safe directory"
    );
    ensure!(
        fs::read_dir(&destination)?.next().is_none(),
        "extraction destination must be empty"
    );
    let identity = (metadata.dev(), metadata.ino());
    switch::checkpoint("replay-extract-destination-validated")?;
    let validate = || -> Result<()> {
        checked_path(&destination)?;
        let now = fs::symlink_metadata(&destination)?;
        ensure!(
            (now.dev(), now.ino()) == identity,
            "extraction destination was replaced"
        );
        Ok(())
    };
    let areas = [
        "original",
        "before",
        "staged",
        "worktree",
        "after",
        "messages",
        "invocations",
        "objects",
        "indexes",
    ];
    for area in areas {
        validate()?;
        fs::DirBuilder::new().mode(0o700).create(destination.join(area))?;
    }
    let mut outputs = Vec::new();
    for (ordinal, entry) in copy.manifest.entries.iter().enumerate() {
        validate()?;
        let area = match entry.area {
            Area::Original => "original",
            Area::Before => "before",
            Area::Staged => "staged",
            Area::Worktree => "worktree",
            Area::After => "after",
            Area::Messages => "messages",
            Area::Invocations => "invocations",
            Area::Objects => "objects",
            Area::Indexes => "indexes",
        };
        let suffix = match entry.kind {
            Kind::Symlink => ".symlink-payload",
            Kind::Deleted => ".deleted",
            Kind::Directory { .. } => ".directory",
            _ => ".payload",
        };
        let relative = format!("{area}/{ordinal:08}{suffix}");
        let output = destination.join(&relative);
        checked_path(output.parent().context("extracted member parent")?)?;
        switch::private_write(&output, &copy.payload(ordinal)?)?;
        outputs.push(relative);
    }
    validate()?;
    let manifest = serde_json::json!({"version":1,"sourceReceipt":copy.receipt.digest,"manifest":copy.manifest,"payloads":outputs});
    switch::private_write(
        &destination.join("manifest.json"),
        &serde_json::to_vec_pretty(&manifest)?,
    )?;
    for area in areas {
        switch::sync_dir(&destination.join(area))?;
    }
    switch::sync_dir(&destination)?;
    Ok(copy.receipt)
}
