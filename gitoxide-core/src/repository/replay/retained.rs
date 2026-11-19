//! Independent private copies. No active journal is needed to validate their bytes.
//! Logical paths are metadata only; every payload has an opaque numeric filename.
use std::{
    fs,
    io::Read,
    os::unix::fs::{DirBuilderExt, MetadataExt, PermissionsExt},
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};

use crate::repository::{authoring::conflict::file_fingerprint, switch};

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
pub struct Member<D = Vec<u8>> {
    pub area: Area,
    pub path: Vec<u8>,
    pub stage: Option<u8>,
    pub kind: Kind,
    pub mode: u32,
    pub data: D,
}

/// A repeatable source. Capture pins its digest before publishing and checks it
/// again after copying, so a changing source cannot authorize restoration.
pub trait Content {
    fn size(&self) -> Result<u64>;
    fn reader(&self) -> Result<Box<dyn Read + '_>>;
}
impl Content for Vec<u8> {
    fn size(&self) -> Result<u64> {
        Ok(self.len() as u64)
    }
    fn reader(&self) -> Result<Box<dyn Read + '_>> {
        Ok(Box::new(self.as_slice()))
    }
}

pub(super) enum Source<'a> {
    Bytes(Vec<u8>),
    File(PathBuf),
    Object(&'a gix::Repository, gix::ObjectId),
}
impl Content for Source<'_> {
    fn size(&self) -> Result<u64> {
        Ok(match self {
            Self::Bytes(bytes) => bytes.len() as u64,
            Self::File(path) => {
                let metadata = fs::symlink_metadata(path)?;
                ensure!(metadata.is_file(), "retained source is not a regular file");
                metadata.len()
            }
            Self::Object(repo, id) => repo.find_header(*id)?.size(),
        })
    }
    fn reader(&self) -> Result<Box<dyn Read + '_>> {
        Ok(match self {
            Self::Bytes(bytes) => Box::new(bytes.as_slice()),
            Self::File(path) => {
                use std::os::unix::fs::OpenOptionsExt;
                Box::new(
                    fs::OpenOptions::new()
                        .read(true)
                        .custom_flags(rustix::fs::OFlags::NOFOLLOW.bits() as i32)
                        .open(path)?,
                )
            }
            Self::Object(repo, id) => {
                use gix::objs::FindStream;
                let budget = gix::utils::stream::Budget::new(&std::env::temp_dir())?;
                Box::new(
                    repo.objects
                        .find_stream(id, &budget, &gix::interrupt::IS_INTERRUPTED)?
                        .1,
                )
            }
        })
    }
}

/// An upper bound of the `manifest.json` a copy of members with these logical
/// path lengths writes. An entry's JSON is at most 4 bytes per path byte (a
/// decimal array) plus 340 for its other fields; the checksummed envelope spells
/// each JSON byte as a decimal number of at most 4 bytes.
pub(crate) fn manifest_bound(paths: impl Iterator<Item = usize>) -> u64 {
    let entries = paths.fold(1024u64, |total, length| {
        total.saturating_add((length as u64).saturating_mul(4).saturating_add(340))
    });
    entries.saturating_mul(4).saturating_add(256)
}

fn fingerprint(source: &impl Content) -> Result<String> {
    crate::repository::authoring::conflict::file_fingerprint_reader(&mut source.reader()?, source.size()?)
}

fn validate_content(kind: &Kind, source: &impl Content, hash: gix::hash::Kind) -> Result<()> {
    match kind {
        Kind::Deleted | Kind::Directory { .. } => {
            ensure!(source.size()? == 0, "directory or deletion cannot carry file bytes");
        }
        Kind::Object { id, kind } => {
            let id = gix::ObjectId::from_hex(id.as_bytes())?;
            let kind = gix::objs::Kind::from_bytes(kind.as_bytes())?;
            ensure!(
                id.kind() == hash
                    && gix::objs::compute_stream_hash(
                        hash,
                        kind,
                        &mut source.reader()?,
                        source.size()?,
                        &mut gix::progress::Discard,
                        &gix::interrupt::IS_INTERRUPTED
                    )? == id,
                "retained Git object hash mismatch"
            );
        }
        Kind::File | Kind::Symlink => {}
    }
    Ok(())
}

fn persist_content(path: &Path, owner: u32, source: &impl Content, entry: &Entry, complete: bool) -> Result<()> {
    use std::os::unix::fs::OpenOptionsExt;
    let present = match fs::symlink_metadata(path) {
        Ok(metadata) => {
            ensure!(
                metadata.is_file() && metadata.uid() == owner && metadata.permissions().mode().trailing_zeros() >= 6,
                "unsafe retained member"
            );
            // Members are copied with exactly `entry.size` bytes, so in a copy
            // not yet complete a shorter file is this copy's own write cut short
            // (a kill; a failed write removes its own file). Its source is still
            // pinned by the manifest, and the bytes are rewritten and verified.
            if !complete && metadata.len() < entry.size {
                fs::remove_file(path)?;
                false
            } else {
                true
            }
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => false,
        Err(error) => return Err(error.into()),
    };
    if !present {
        switch::payload::check_growth(path.parent().context("retained member parent")?, entry.size)?;
        let mut reader = source.reader()?;
        switch::private_copy(path, &mut reader.by_ref().take(entry.size))?;
        let mut extra = [0];
        ensure!(
            reader.read(&mut extra)? == 0,
            "retained-copy source grew during capture"
        );
    }
    let mut file = fs::OpenOptions::new()
        .read(true)
        .custom_flags(rustix::fs::OFlags::NOFOLLOW.bits() as i32)
        .open(path)?;
    ensure!(
        file.metadata()?.len() == entry.size
            && crate::repository::authoring::conflict::file_fingerprint_reader(&mut file, entry.size)?
                == entry.checksum,
        "retained-copy source or member changed"
    );
    switch::sync_file(&file, path)?;
    switch::sync_dir(path.parent().context("retained member parent")?)
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

/// Write `bytes` as the record `path` of a copy, or accept the identical record a
/// previous attempt wrote. A record is published whole: the bytes go to a private
/// spool in a scratch directory of the Git directory (the same filesystem; a
/// name in the copy would make the copy unreadable), which is renamed onto `path`
/// and synchronized with its directory. So a kill leaves the record absent or
/// whole, never partial; a loss of power before synchronization can still leave
/// a prefix, which `capture` treats as its own write in a copy not yet complete.
fn persist_exact(path: &Path, owner: u32, bytes: &[u8], scratch: &Path) -> Result<()> {
    match fs::symlink_metadata(path) {
        Ok(_) => ensure!(
            read_member(path, owner)? == bytes,
            "retained-copy source or member changed"
        ),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            use std::io::Write;
            let budget = gix::utils::stream::Budget::new(scratch)?;
            let mut spool = budget.spool()?;
            spool.write_all(bytes)?;
            spool.flush()?;
            switch::private_publish(path, spool)?;
        }
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

#[cfg(test)]
thread_local! {
    static VALIDATED_ENTRIES: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
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
        #[cfg(test)]
        VALIDATED_ENTRIES.with(|count| count.set(count.get() + 1));
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
pub fn capture<D: Content>(
    repo: &gix::Repository,
    nonce: &str,
    sequence: u64,
    original: gix::ObjectId,
    replay: gix::ObjectId,
    observed: gix::ObjectId,
    members: &[Member<D>],
) -> Result<Receipt> {
    let owner = fs::metadata(repo.git_dir())?.uid();
    // Outside recovery every member, and the manifest written before them, is
    // copied under the scratch quota: a copy whose member bytes plus an upper
    // bound of its manifest exceed it is refused before anything is written. The
    // copy an abort or a cleanup needs is written whatever its size (see
    // `switch::payload::Recovery`); only the disk bounds it.
    if switch::payload::metered() {
        let mut planned = manifest_bound(members.iter().map(|member| member.path.len()));
        for member in members {
            planned = planned.saturating_add(member.data.size()?);
        }
        let limit = switch::payload::scratch_limit();
        ensure!(
            planned <= limit,
            "retained copy cannot be written: its {} members and manifest need up to {planned} bytes, over the {limit}-byte scratch limit every retained copy lives under",
            members.len()
        );
    }
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
                    size: member.data.size()?,
                    checksum: fingerprint(&member.data)?,
                })
            })
            .collect::<Result<_>>()?,
    };
    let hash = validate_manifest(&manifest)?;
    for member in members {
        validate_content(&member.kind, &member.data, hash)?;
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
    let marker = format!("{digest}\n");
    let complete = match fs::symlink_metadata(leaf.join("COPY_COMPLETE")) {
        Ok(_) => {
            let written = read_member(&leaf.join("COPY_COMPLETE"), owner)?;
            if written.len() < marker.len()
                && marker.as_bytes().starts_with(&written)
                && read_member(&leaf.join("manifest.json"), owner)? == bytes
            {
                // This capture's own marker, cut short before it was synchronized:
                // the copy never completed. It is verified and marked again below.
                fs::remove_file(leaf.join("COPY_COMPLETE"))?;
                switch::sync_dir(&leaf)?;
                false
            } else {
                ensure!(
                    inspect(&leaf)?.receipt.digest == digest,
                    "retained copy already completes a different capture"
                );
                true
            }
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => false,
        Err(error) => return Err(error.into()),
    };
    match fs::symlink_metadata(leaf.join("manifest.json")) {
        Ok(_) if complete => ensure!(
            read_member(&leaf.join("manifest.json"), owner)? == bytes,
            "retained-copy source changed"
        ),
        Ok(_) => {
            let existing = read_member(&leaf.join("manifest.json"), owner)?;
            if existing != bytes {
                // A whole manifest pins its sources: one that differs means a
                // source changed while the capture was interrupted, and that
                // still refuses. A manifest that is not a whole checksummed record
                // (cut short while written, or lost with the page cache) pins
                // nothing; in a copy not yet complete it is replaced, and any
                // member beside it is copied again.
                let pins = serde_json::from_slice::<Envelope>(&existing)
                    .ok()
                    .is_some_and(|envelope| switch::digest(&envelope.payload).is_ok_and(|d| d == envelope.checksum));
                ensure!(!pins, "retained-copy source changed");
                for entry in fs::read_dir(&leaf)? {
                    let entry = entry?;
                    let name = entry.file_name();
                    if name == "manifest.json"
                        || name
                            .to_str()
                            .is_some_and(|name| name.len() == 8 && name.bytes().all(|byte| byte.is_ascii_digit()))
                    {
                        read_member(&entry.path(), owner)?;
                        fs::remove_file(entry.path())?;
                    }
                }
                switch::sync_dir(&leaf)?;
            }
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error.into()),
    }
    // Pin all source digests before the first payload, including sources not yet
    // copied when an interruption occurs. COPY_COMPLETE distinguishes intent
    // from a copy that has actually been verified and synchronized.
    let scratch = fs::canonicalize(repo.git_dir())?;
    persist_exact(&leaf.join("manifest.json"), owner, &bytes, &scratch)?;
    switch::checkpoint("replay-retained-manifest")?;
    for (ordinal, member) in members.iter().enumerate() {
        persist_content(
            &leaf.join(payload_name(ordinal)),
            owner,
            &member.data,
            &manifest.entries[ordinal],
            complete,
        )?;
        switch::checkpoint("replay-retained-member")?;
    }
    persist_exact(&leaf.join("COPY_COMPLETE"), owner, marker.as_bytes(), &scratch)?;
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
        let source = Source::File(path.join(name));
        let Source::File(member) = &source else { unreachable!() };
        let metadata = fs::symlink_metadata(member)?;
        ensure!(
            metadata.is_file() && metadata.uid() == owner && metadata.permissions().mode().trailing_zeros() >= 6,
            "unsafe retained-copy member"
        );
        ensure!(
            source.size()? == entry.size && fingerprint(&source)? == entry.checksum,
            "retained payload checksum mismatch"
        );
        validate_content(&entry.kind, &source, hash)?;
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
        self.payload_reader()?(ordinal)
    }

    /// Validate the manifest once while its immutable borrow prevents changes.
    /// Each read still checks live path ownership, bytes and Git object identity.
    pub(crate) fn payload_reader(&self) -> Result<impl Fn(usize) -> Result<Vec<u8>> + '_> {
        let hash = validate_manifest(&self.manifest)?;
        Ok(move |ordinal| self.read_payload(ordinal, hash))
    }

    pub(crate) fn payload_verifier(&self) -> Result<impl Fn(usize) -> Result<()> + '_> {
        let hash = validate_manifest(&self.manifest)?;
        Ok(move |ordinal| {
            checked_path(&self.receipt.path)?;
            check_directory(&self.receipt.path, self.manifest.owner_uid)?;
            let entry = self
                .manifest
                .entries
                .get(ordinal)
                .context("unknown retained payload ordinal")?;
            let path = self.receipt.path.join(payload_name(ordinal));
            let metadata = fs::symlink_metadata(&path)?;
            ensure!(
                metadata.is_file()
                    && metadata.uid() == self.manifest.owner_uid
                    && metadata.permissions().mode().trailing_zeros() >= 6,
                "unsafe retained-copy member"
            );
            let source = Source::File(path);
            ensure!(
                source.size()? == entry.size && fingerprint(&source)? == entry.checksum,
                "retained payload changed"
            );
            validate_content(&entry.kind, &source, hash)
        })
    }

    fn read_payload(&self, ordinal: usize, hash: gix::hash::Kind) -> Result<Vec<u8>> {
        checked_path(&self.receipt.path)?;
        check_directory(&self.receipt.path, self.manifest.owner_uid)?;
        let entry = self
            .manifest
            .entries
            .get(ordinal)
            .context("unknown retained payload ordinal")?;
        let data = read_member(&self.receipt.path.join(payload_name(ordinal)), self.manifest.owner_uid)?;
        ensure!(
            data.len() as u64 == entry.size && file_fingerprint(&data)? == entry.checksum,
            "retained payload changed"
        );
        validate_member(&entry.kind, &data, hash)?;
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
    let verify_payload = copy.payload_verifier()?;
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
        verify_payload(ordinal)?;
        let source = Source::File(copy.receipt.path.join(payload_name(ordinal)));
        let mut reader = source.reader()?;
        switch::private_copy(&output, &mut reader.by_ref().take(entry.size))?;
        let mut extra = [0];
        ensure!(reader.read(&mut extra)? == 0, "retained payload grew during extraction");
        let destination_source = Source::File(output);
        ensure!(
            destination_source.size()? == entry.size && fingerprint(&destination_source)? == entry.checksum,
            "retained payload changed during extraction"
        );
        outputs.push(relative);
    }
    drop(verify_payload);
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_growing_source_cannot_exceed_its_licensed_size_or_complete_a_copy() -> Result<()> {
        struct Growing(std::cell::Cell<bool>);
        impl Content for Growing {
            fn size(&self) -> Result<u64> {
                Ok(3)
            }
            fn reader(&self) -> Result<Box<dyn Read + '_>> {
                let bytes: &[u8] = if self.0.replace(true) {
                    b"old plus unlicensed growth"
                } else {
                    b"old"
                };
                Ok(Box::new(bytes))
            }
        }
        let temp = tempfile::tempdir()?;
        let repo = gix::init(temp.path())?;
        let id = gix::hash::Kind::Sha1.null();
        let nonce = "a".repeat(40);
        let member = Member {
            area: Area::Worktree,
            path: b"file".to_vec(),
            stage: None,
            kind: Kind::File,
            mode: 0o600,
            data: Growing(std::cell::Cell::new(false)),
        };
        let error = capture(&repo, &nonce, 1, id, id, id, &[member]).expect_err("source growth must refuse");
        assert!(error.to_string().contains("source grew"));
        let leaf = repo.git_dir().join("brit-rebase-retained").join(nonce).join("1");
        assert_eq!(fs::read(leaf.join(payload_name(0)))?, b"old");
        assert!(!leaf.join("COPY_COMPLETE").exists());
        assert!(inspect(&leaf).is_err());
        Ok(())
    }

    fn two_members() -> Vec<Member> {
        ["first", "second"]
            .into_iter()
            .map(|name| Member {
                area: Area::Worktree,
                path: name.as_bytes().to_vec(),
                stage: None,
                kind: Kind::File,
                mode: 0o600,
                data: format!("{name} payload").into_bytes(),
            })
            .collect()
    }

    fn cut(path: &Path, length: u64) -> Result<()> {
        fs::OpenOptions::new().write(true).open(path)?.set_len(length)?;
        Ok(())
    }

    /// M2: a copy not yet complete whose manifest is cut short (a kill inside its
    /// write, or a loss of power before it was synchronized), or whose completion
    /// marker is, is captured again to a complete copy instead of refused.
    #[test]
    fn a_torn_manifest_or_marker_in_an_incomplete_copy_is_written_again() -> Result<()> {
        let id = gix::hash::Kind::Sha1.null();
        let nonce = "a".repeat(40);
        for record in ["manifest.json", "COPY_COMPLETE"] {
            for length in [Some(0), Some(1), None] {
                let temp = tempfile::tempdir()?;
                let repo = gix::init(temp.path())?;
                let receipt = capture(&repo, &nonce, 1, id, id, id, &two_members())?;
                let leaf = receipt.path.clone();
                if record == "manifest.json" {
                    fs::remove_file(leaf.join("COPY_COMPLETE"))?;
                }
                let full = fs::metadata(leaf.join(record))?.len();
                cut(&leaf.join(record), length.unwrap_or(full - 1))?;
                assert!(inspect(&leaf).is_err(), "{record}: the cut copy is not complete");
                let again = capture(&repo, &nonce, 1, id, id, id, &two_members())?;
                assert_eq!(again.digest, receipt.digest, "{record} {length:?}");
                inspect(&leaf)?;
            }
        }
        Ok(())
    }

    /// A complete copy is immutable: a manifest that differs in it still refuses.
    #[test]
    fn a_differing_manifest_in_a_complete_copy_still_refuses() -> Result<()> {
        let id = gix::hash::Kind::Sha1.null();
        let nonce = "a".repeat(40);
        let temp = tempfile::tempdir()?;
        let repo = gix::init(temp.path())?;
        let receipt = capture(&repo, &nonce, 1, id, id, id, &two_members())?;
        let full = fs::metadata(receipt.path.join("manifest.json"))?.len();
        cut(&receipt.path.join("manifest.json"), full - 1)?;
        let before = fs::read(receipt.path.join("manifest.json"))?;
        assert!(capture(&repo, &nonce, 1, id, id, id, &two_members()).is_err());
        assert_eq!(fs::read(receipt.path.join("manifest.json"))?, before, "left as found");
        assert!(receipt.path.join(payload_name(0)).exists());
        Ok(())
    }

    #[test]
    fn borrowed_reader_validates_each_manifest_entry_once_and_rechecks_live_payloads() -> Result<()> {
        if gix_testtools::run_in_isolated_process().map_err(anyhow::Error::from_boxed)? {
            return Ok(());
        }
        for count in [64, 128] {
            let temp = tempfile::tempdir()?;
            let repo = gix::init(temp.path())?;
            let id = gix::hash::Kind::Sha1.null();
            let members = (0..count)
                .map(|ordinal| Member {
                    area: Area::Worktree,
                    path: ordinal.to_string().into_bytes(),
                    stage: None,
                    kind: Kind::File,
                    mode: 0o600,
                    data: b"private payload".to_vec(),
                })
                .collect::<Vec<_>>();
            let receipt = capture(&repo, &"a".repeat(40), 1, id, id, id, &members)?;
            let mut copy = inspect(&receipt.path)?;
            VALIDATED_ENTRIES.with(|visits| visits.set(0));
            {
                let read = copy.payload_reader()?;
                for (ordinal, member) in members.iter().enumerate() {
                    assert_eq!(read(ordinal)?, member.data);
                }
                assert_eq!(VALIDATED_ENTRIES.with(std::cell::Cell::get), count);
                fs::write(receipt.path.join(payload_name(0)), b"changed payload")?;
                assert!(read(0).is_err(), "a borrowed reader must not cache payload bytes");
            }
            fs::write(receipt.path.join(payload_name(0)), &members[0].data)?;
            copy.manifest.entries[0].kind = Kind::Object {
                id: id.to_string(),
                kind: "blob".into(),
            };
            assert!(
                copy.payload_reader()?(0).is_err(),
                "Git object verification remains mandatory"
            );
            copy.manifest.entries.push(copy.manifest.entries[0].clone());
            assert!(
                copy.payload_reader().is_err(),
                "a new borrow must validate edited metadata"
            );
        }
        Ok(())
    }
}
