//! Immutable raw recovery payloads. Metadata remains checksummed JSON; file contents occur once.
use super::*;
use gix::utils::stream::{Charge, SCRATCH_LIMIT, ScratchLimit, Spool};
use std::{
    io::Seek,
    os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt},
    sync::Arc,
};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ScratchDirectory {
    path: Vec<u8>,
    identity: [u64; 2],
}

pub(super) fn attach_budget(repo: &gix::Repository, state: &mut Manifest) -> Result<()> {
    // Publication stays on the repository filesystem, even when TMPDIR is elsewhere.
    let budget = Budget::new(repo.git_dir())?;
    let directory = fs::canonicalize(budget.directory())?;
    state.scratch = Some(ScratchDirectory {
        path: raw(&directory)?,
        identity: directory_identity(&directory)?,
    });
    state.budget = Some(budget);
    Ok(())
}

/// Called only after proving the previous process dead and every ordinary lock owned.
pub(super) fn retire_scratch(repo: &gix::Repository, state: &Manifest) -> Result<()> {
    let Some(scratch) = &state.scratch else {
        return Ok(());
    };
    let directory = path(&scratch.path)?;
    ensure!(
        directory.parent() == Some(fs::canonicalize(repo.git_dir())?.as_path())
            && directory
                .file_name()
                .is_some_and(|name| name.as_encoded_bytes().starts_with(b".brit-checkout-")),
        "foreign switch scratch path; evidence retained"
    );
    let metadata = match fs::symlink_metadata(&directory) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error.into()),
    };
    ensure!(
        metadata.is_dir()
            && [metadata.dev(), metadata.ino()] == scratch.identity
            && metadata.uid() == fs::metadata(journal(repo))?.uid()
            && metadata.permissions().mode().trailing_zeros() >= 6,
        "foreign switch scratch directory; evidence retained"
    );
    let entries = fs::read_dir(&directory)?.collect::<std::io::Result<Vec<_>>>()?;
    for entry in &entries {
        let metadata = fs::symlink_metadata(entry.path())?;
        ensure!(
            entry.file_name().as_encoded_bytes().starts_with(b"blob-")
                && metadata.is_file()
                && metadata.uid() == fs::metadata(&directory)?.uid(),
            "foreign switch scratch member; evidence retained"
        );
    }
    for entry in entries {
        fs::remove_file(entry.path())?;
    }
    fs::remove_dir(&directory)?;
    sync_dir(repo.git_dir())
}

pub(crate) fn checksum(input: &mut impl Read, size: u64) -> Result<String> {
    Ok(gix::objs::compute_stream_hash(
        gix::hash::Kind::Sha1,
        gix::objs::Kind::Blob,
        input,
        size,
        &mut gix::progress::Discard,
        &gix::interrupt::IS_INTERRUPTED,
    )?
    .to_string())
}

impl PartialEq for State {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Absent, Self::Absent) => true,
            (
                Self::Directory {
                    mode: a,
                    device: b,
                    inode: c,
                },
                Self::Directory {
                    mode: d,
                    device: e,
                    inode: f,
                },
            ) => (a, b, c) == (d, e, f),
            (Self::Symlink(a), Self::Symlink(b)) => a == b,
            (Self::File { mode: a, bytes: b }, Self::File { mode: c, bytes: d }) => a == c && b == d,
            (
                Self::StoredFile {
                    mode: a,
                    size: b,
                    checksum: c,
                },
                Self::StoredFile {
                    mode: d,
                    size: e,
                    checksum: f,
                },
            ) => (a, b, c) == (d, e, f),
            (
                Self::File { mode, bytes },
                Self::StoredFile {
                    mode: expected,
                    size,
                    checksum: hash,
                },
            )
            | (
                Self::StoredFile {
                    mode: expected,
                    size,
                    checksum: hash,
                },
                Self::File { mode, bytes },
            ) => {
                mode == expected
                    && bytes.len() as u64 == *size
                    && gix::objs::compute_hash(gix::hash::Kind::Sha1, gix::objs::Kind::Blob, bytes)
                        .is_ok_and(|id| id.as_ref() == *hash)
            }
            _ => false,
        }
    }
}

fn usage(root: &Path) -> Result<u64> {
    let mut size = 0u64;
    for entry in fs::read_dir(root)? {
        let metadata = entry?.metadata()?;
        if metadata.is_file() {
            size = size
                .checked_add(metadata.len())
                .context("journal scratch size overflow")?;
        }
    }
    Ok(size)
}

pub(crate) fn check_growth(root: &Path, requested: u64) -> Result<()> {
    let live = usage(root)?;
    if live.checked_add(requested).is_none_or(|size| size > SCRATCH_LIMIT) {
        return Err(ScratchLimit {
            requested,
            live,
            path: root.to_owned(),
        }
        .into());
    }
    Ok(())
}

pub(crate) fn charge(budget: &Arc<Budget>, root: &Path) -> Result<Charge> {
    Ok(budget.charge(usage(root)?)?)
}

pub(crate) fn open(root: &Path, size: u64, hash: &str) -> Result<fs::File> {
    ensure!(
        hash.len() == 40 && hash.bytes().all(|byte| byte.is_ascii_hexdigit()),
        "invalid recovery payload identity"
    );
    let path = root.join(format!("payload-{hash}"));
    let mut file = fs::OpenOptions::new()
        .read(true)
        .custom_flags(rustix::fs::OFlags::NOFOLLOW.bits() as i32)
        .open(&path)?;
    let metadata = file.metadata()?;
    ensure!(
        metadata.is_file()
            && metadata.uid() == fs::metadata(root)?.uid()
            && metadata.permissions().mode().trailing_zeros() >= 6
            && metadata.len() == size,
        "unsafe recovery payload {}",
        path.display()
    );
    ensure!(
        checksum(&mut file, size)? == hash,
        "corrupt recovery payload {}; evidence retained",
        path.display()
    );
    file.rewind()?;
    Ok(file)
}

pub(super) fn validate(root: &Path, state: &Manifest) -> Result<()> {
    ensure!(
        state.version == VERSION || state.payloads.is_empty(),
        "legacy journal contains recovery payloads"
    );
    check_growth(root, 0)?;
    for (hash, size) in &state.payloads {
        ensure!(
            hash.len() == 40 && hash.bytes().all(|byte| byte.is_ascii_hexdigit()),
            "invalid recovery payload identity"
        );
        if state.preparing && !present(&root.join(format!("payload-{hash}")))? {
            continue;
        }
        open(root, *size, hash)?;
    }
    for change in &state.changes {
        for image in [&change.before, &change.after] {
            if let State::StoredFile { size, checksum, .. } = image {
                ensure!(
                    state.payloads.get(checksum) == Some(size),
                    "undeclared recovery payload"
                );
            }
        }
    }
    Ok(())
}

pub(super) fn snapshot(
    repo: &gix::Repository,
    state: &mut Manifest,
    path: &Path,
    budget: &Arc<Budget>,
) -> Result<State> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.is_file() => metadata,
        _ => return super::snapshot(path),
    };
    let _charge = charge(budget, &journal(repo))?;
    let mut source = budget.spool()?;
    stream::copy(&mut fs::File::open(path)?, &mut source, &gix::interrupt::IS_INTERRUPTED)?;
    source.rewind()?;
    store(repo, state, source, metadata.permissions().mode() & 0o777)
}

pub(super) fn store(repo: &gix::Repository, state: &mut Manifest, mut source: Spool, mode: u32) -> Result<State> {
    let size = source.len();
    source.rewind()?;
    let hash = checksum(&mut source, size)?;
    source.rewind()?;
    let root = journal(repo);
    state.payloads.insert(hash.clone(), size);
    // Intent licenses exactly this private name before any durable payload appears.
    save(repo, state)?;
    checkpoint("payload-intent")?;
    persist(&root, source, size, &hash)?;
    checkpoint("payload-persisted")?;
    Ok(State::StoredFile {
        mode,
        size,
        checksum: hash,
    })
}

/// Persist only after the caller has durably licensed this exact identity.
pub(crate) fn persist(root: &Path, mut source: Spool, size: u64, hash: &str) -> Result<()> {
    let destination = root.join(format!("payload-{hash}"));
    if present(&destination)? {
        open(root, size, hash)?;
    } else {
        check_growth(root, size)?;
        source.set_permissions(fs::Permissions::from_mode(0o600))?;
        let file = source.publish(&destination)?;
        sync_file(&file, &destination)?;
        sync_dir(root)?;
    }
    Ok(())
}

/// Observe regular files without retaining their contents. This is also comparable
/// with the legacy inline image, so old recovery journals keep their semantics.
pub(crate) fn fingerprint(path: &Path) -> Result<State> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.is_file() => metadata,
        _ => return super::snapshot(path),
    };
    let mut file = fs::OpenOptions::new()
        .read(true)
        .custom_flags(rustix::fs::OFlags::NOFOLLOW.bits() as i32)
        .open(path)?;
    let opened = file.metadata()?;
    ensure!(
        (metadata.dev(), metadata.ino(), metadata.len()) == (opened.dev(), opened.ino(), opened.len()),
        "file changed while observing {}",
        path.display()
    );
    Ok(State::StoredFile {
        mode: metadata.permissions().mode() & 0o777,
        size: metadata.len(),
        checksum: checksum(&mut file, metadata.len())?,
    })
}

pub(super) fn publish(
    root: &Path,
    staging: &Path,
    destination: &Path,
    mode: u32,
    size: u64,
    hash: &str,
    actual: &State,
) -> Result<()> {
    let mut source = open(root, size, hash)?;
    let staged = staging.join("publication");
    if fs::symlink_metadata(&staged).is_ok() {
        fs::remove_file(&staged)?;
    }
    check_growth(root, size)?;
    let mut file = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&staged)?;
    io_fault("write", &staged)?;
    let written = stream::copy(&mut source, &mut file, &gix::interrupt::IS_INTERRUPTED)?;
    file.rewind()?;
    ensure!(
        written == size && checksum(&mut file, size)? == hash,
        "recovery payload changed during publication"
    );
    file.set_permissions(fs::Permissions::from_mode(mode))?;
    sync_file(&file, &staged)?;
    sync_dir(staging)?;
    ensure!(
        fingerprint(destination)? == *actual,
        "filesystem changed before publication: {}",
        destination.display()
    );
    fs::rename(&staged, destination)?;
    sync_dir(staging)
}
