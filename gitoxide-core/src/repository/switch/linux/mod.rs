//! Conservative local-branch switching with a private, explicitly recovered write-ahead journal.
//!
//! Durability means successful file and containing-directory `sync_all` calls on Linux.
//! Arbitrary external writers are detected by before/after comparisons, not isolated.
use super::super::index::authoring::{read_index, worktree_entry};
use anyhow::{Context, Result, bail, ensure};
use gix::bstr::{BStr, BString, ByteSlice};
use gix::index::entry::{Flags, Mode, Stat};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
};

const JOURNAL: &str = "brit-switch";
const VERSION: u32 = 2;

mod object_store;
pub(crate) use object_store::{validate_local_history, validate_object_boundaries};

#[cfg(feature = "blocking-client")]
mod pull;
#[cfg(feature = "blocking-client")]
pub use pull::{execute as pull_execute, recover as pull_recover};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) enum State {
    Absent,
    Directory { mode: u32, device: u64, inode: u64 },
    File { mode: u32, bytes: Vec<u8> },
    Symlink(Vec<u8>),
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Change {
    pub(crate) path: Vec<u8>,
    pub(crate) before: State,
    pub(crate) after: State,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Lock {
    pub(crate) path: Vec<u8>,
    pub(crate) device: u64,
    pub(crate) inode: u64,
    pub(crate) checksum: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Manifest {
    version: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pull: Option<Pull>,
    git_dir: Vec<u8>,
    root: Vec<u8>,
    common: Vec<u8>,
    git_identity: [u64; 2],
    root_identity: [u64; 2],
    common_identity: [u64; 2],
    pid: u32,
    process: String,
    locks: Vec<Lock>,
    source: Option<Vec<u8>>,
    source_oid: String,
    target: Vec<u8>,
    target_oid: String,
    started: BTreeSet<Vec<u8>>,
    changes: Vec<Change>,
    index: Change,
    head: Change,
    reflog: Change,
    log_directory: Change,
    preparing: bool,
    decision: bool,
    aborting: bool,
    hook: String,
}
/// Private external-Git state, never a protocol entity or object-database backup.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Pull {
    remote: String,
    remote_url: Vec<u8>,
    tracking: Option<(Vec<u8>, bool)>,
    tracking_oid: Option<String>,
    branch: Vec<u8>,
    branch_argument: Vec<u8>,
    pinned: bool,
    fetched: bool,
    no_op: bool,
    fetch_decision: bool,
    fetch_done: bool,
    fetch_metadata: Vec<Change>,
    metadata: Vec<Change>,
    directories: Vec<Change>,
    objects: Vec<(Vec<u8>, String)>,
    keeps: Vec<Lock>,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Envelope {
    checksum: String,
    payload: Vec<u8>,
}

fn raw(path: &Path) -> Result<Vec<u8>> {
    Ok(gix::path::into_bstr(path.to_owned())?.into_owned().into())
}
fn path(bytes: &[u8]) -> Result<PathBuf> {
    Ok(gix::path::from_bstr(bytes.as_bstr())?.into_owned())
}
pub(crate) fn digest(bytes: &[u8]) -> Result<String> {
    let mut hash = gix::hash::hasher(gix::hash::Kind::Sha1);
    hash.update(bytes);
    Ok(hash.try_finalize()?.to_string())
}
pub(crate) fn sync_dir(path: &Path) -> Result<()> {
    io_fault("directory-sync", path)?;
    fs::File::open(path)?
        .sync_all()
        .with_context(|| format!("synchronize directory {}", path.display()))?;
    #[cfg(test)]
    durability::record_dir(path)?;
    Ok(())
}
pub(crate) fn sync_path(path: &Path) -> Result<()> {
    sync_file(&fs::File::open(path)?, path)
}
/// Synchronize the already-open inode while recording its name for recovery models.
pub(crate) fn sync_file(file: &fs::File, path: &Path) -> Result<()> {
    io_fault("file-sync", path)?;
    file.sync_all()?;
    #[cfg(test)]
    durability::record_file(path)?;
    let _ = path;
    Ok(())
}
pub(crate) fn private_write(path: &Path, bytes: &[u8]) -> Result<()> {
    use std::os::unix::fs::OpenOptionsExt;
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)?;
    io_fault("write", path)?;
    file.write_all(bytes)?;
    sync_file(&file, path)?;
    Ok(())
}

/// Dedicated fault builds can fail a real persistence boundary without filling
/// the host disk. Delivery builds do not inspect this environment variable.
fn io_fault(operation: &str, path: &Path) -> Result<()> {
    #[cfg(feature = "switch-test-faults")]
    if let Ok(specification) = std::env::var("BRIT_REPLAY_IO_FAULT") {
        let mut parts = specification.splitn(4, '|');
        let selected = parts.next().context("missing I/O fault operation")?;
        let errno: i32 = parts.next().context("missing I/O fault errno")?.parse()?;
        let needle = parts.next().context("missing I/O fault path")?;
        let occurrence = parts.next().map(str::parse::<usize>).transpose()?.unwrap_or(1);
        ensure!(
            !needle.is_empty() && occurrence > 0 && matches!(errno, 5 | 28),
            "invalid replay I/O fault specification"
        );
        if selected == operation
            && path
                .as_os_str()
                .as_encoded_bytes()
                .windows(needle.len())
                .any(|bytes| bytes == needle.as_bytes())
        {
            thread_local! { static HITS: std::cell::RefCell<BTreeMap<String, usize>> = const { std::cell::RefCell::new(BTreeMap::new()) }; }
            let hit = HITS.with(|hits| {
                let mut hits = hits.borrow_mut();
                let hit = hits.entry(specification.clone()).or_default();
                *hit += 1;
                *hit
            });
            if hit != occurrence {
                return Ok(());
            }
            return Err(std::io::Error::from_raw_os_error(errno))
                .with_context(|| format!("injected {operation} failure at {}", path.display()));
        }
    }
    let _ = (operation, path);
    Ok(())
}
fn directory_identity(path: &Path) -> Result<[u64; 2]> {
    use std::os::unix::fs::MetadataExt;
    let metadata = fs::metadata(path)?;
    ensure!(metadata.is_dir(), "repository directory identity changed");
    Ok([metadata.dev(), metadata.ino()])
}

fn process(pid: u32) -> Result<String> {
    let stat = fs::read_to_string(format!("/proc/{pid}/stat"))?;
    let fields = stat.rsplit_once(')').context("malformed process stat")?.1;
    let start = fields
        .split_whitespace()
        .nth(19)
        .context("missing process start time")?;
    Ok(format!(
        "{}:{start}",
        fs::read_to_string("/proc/sys/kernel/random/boot_id")?.trim()
    ))
}
fn absolute(repo: &gix::Repository, name: &Path) -> PathBuf {
    repo.current_dir().join(name)
}
fn journal(repo: &gix::Repository) -> PathBuf {
    absolute(repo, repo.git_dir()).join(JOURNAL)
}
fn present(name: &Path) -> Result<bool> {
    match fs::symlink_metadata(name) {
        Ok(_) => Ok(true),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error.into()),
    }
}

/// Block mutations in every worktree while explicit operation recovery is outstanding.
pub fn ensure_no_pending(repo: &gix::Repository) -> Result<()> {
    for worktree in repo.worktrees_including_main()? {
        let worktree = worktree?;
        super::ensure_no_replay_cleanup(&absolute(&worktree, worktree.git_dir()))?;
        ensure!(
            !present(&absolute(&worktree, worktree.git_dir()).join("brit-rebase"))?,
            "pending replay in {}; recover its internal session",
            worktree.git_dir().display()
        );
        ensure!(
            !present(&absolute(&worktree, worktree.git_dir()).join("brit-authoring"))?,
            "pending conflict authoring in {}; recover its internal session",
            worktree.git_dir().display()
        );
        ensure!(
            !present(&journal(&worktree))?,
            "pending switch or pull in {}; use brit switch --status or brit pull --status, then the named --continue or --abort command",
            worktree.git_dir().display()
        );
    }
    Ok(())
}

/// Serialize cooperating Brit writers across the shared repository.
/// Keep this guard alive for the entire mutation, including its pending-journal check.
pub fn mutation_guard(repo: &gix::Repository) -> Result<fs::File> {
    let lock = shared_guard(repo)?;
    ensure_no_pending(repo)?;
    Ok(lock)
}
pub(crate) fn shared_guard(repo: &gix::Repository) -> Result<fs::File> {
    use std::os::unix::fs::OpenOptionsExt;
    let file = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .mode(0o600)
        .custom_flags(rustix::fs::OFlags::NOFOLLOW.bits() as i32)
        .open(repo.common_dir().join("brit-mutation.guard"))?;
    rustix::fs::flock(&file, rustix::fs::FlockOperation::NonBlockingLockExclusive)
        .context("another Brit mutation is active")?;
    Ok(file)
}

/// Inspect without mutating or automatically resuming an interrupted operation.
pub fn status(repo: &gix::Repository, out: &mut dyn Write) -> Result<()> {
    if !present(&journal(repo))? {
        writeln!(out, "No switch or pull recovery pending")?;
        return Ok(());
    }
    let state = load(repo)?;
    if let Some(pull) = &state.pull {
        writeln!(
            out,
            "Fetch: {}; integration recovery keeps published fetch results",
            if pull.fetch_done {
                "durably completed; continuation is offline"
            } else if pull.fetch_decision {
                "publication decision durable; completion required"
            } else if pull.fetched {
                "objects durable; publication not decided"
            } else {
                "incomplete; explicit continuation may reconnect"
            }
        )?;
    }
    writeln!(
        out,
        "{} recovery pending: {} -> {}; {}; hook {}; use brit {} --continue{}",
        if state.pull.is_some() { "Pull" } else { "Switch" },
        state.source_oid,
        state.target_oid,
        if state.decision && state.hook != "not-started" {
            if state.pull.is_some() {
                "pull committed; roll forward only"
            } else {
                "switch committed; roll forward only"
            }
        } else if state.decision {
            "commit decision durable; roll forward only"
        } else if state.aborting {
            "abort in progress"
        } else {
            "prepared; abort available"
        },
        state.hook,
        if state.pull.is_some() { "pull" } else { "switch" },
        if state.decision { "" } else { " or --abort" }
    )?;
    Ok(())
}

pub(crate) fn snapshot(path: &Path) -> Result<State> {
    use std::os::unix::fs::{MetadataExt, PermissionsExt};
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(err)
            if matches!(
                err.kind(),
                std::io::ErrorKind::NotFound | std::io::ErrorKind::NotADirectory
            ) =>
        {
            return Ok(State::Absent);
        }
        Err(err) => return Err(err.into()),
    };
    if metadata.file_type().is_symlink() {
        return Ok(State::Symlink(raw(&fs::read_link(path)?)?));
    }
    if metadata.is_dir() {
        return Ok(State::Directory {
            mode: metadata.permissions().mode() & 0o777,
            device: metadata.dev(),
            inode: metadata.ino(),
        });
    }
    ensure!(metadata.is_file(), "unsupported special file {}", path.display());
    Ok(State::File {
        mode: metadata.permissions().mode() & 0o777,
        bytes: fs::read(path)?,
    })
}
fn validate_path(bytes: &[u8]) -> Result<()> {
    ensure!(
        !bytes.is_empty() && !bytes.starts_with(b"/") && !bytes.contains(&0),
        "invalid journal path"
    );
    for part in bytes.split(|byte| *byte == b'/') {
        gix::validate::path::component(
            part.as_bstr(),
            None,
            super::super::index::authoring::component_options(),
        )?;
        ensure!(
            part != b"." && part != b".." && !part.eq_ignore_ascii_case(b".git"),
            "unsafe journal path"
        );
    }
    Ok(())
}
pub(crate) fn validate_ancestors(root: &Path, relative: &[u8]) -> Result<()> {
    validate_path(relative)?;
    let relative = path(relative)?;
    let mut current = root.to_owned();
    let components: Vec<_> = relative.components().collect();
    for component in components.iter().take(components.len().saturating_sub(1)) {
        current.push(component);
        let metadata = fs::symlink_metadata(&current)?;
        ensure!(
            metadata.is_dir() && !metadata.file_type().is_symlink(),
            "unsafe ancestor {}",
            current.display()
        );
        ensure!(
            !current.join(".git").exists(),
            "nested repository {}",
            current.display()
        );
    }
    Ok(())
}
fn save(repo: &gix::Repository, state: &Manifest) -> Result<()> {
    let payload = serde_json::to_vec(state)?;
    let bytes = serde_json::to_vec(&Envelope {
        checksum: digest(&payload)?,
        payload,
    })?;
    let root = journal(repo);
    let staged = root.join("manifest.next");
    if staged.exists() {
        fs::remove_file(&staged)?;
    }
    private_write(&staged, &bytes)?;
    fs::rename(staged, root.join("manifest.json"))?;
    sync_dir(&root)
}
fn load(repo: &gix::Repository) -> Result<Manifest> {
    use std::os::unix::fs::PermissionsExt;
    let root = journal(repo);
    let metadata = fs::symlink_metadata(&root)?;
    ensure!(
        metadata.is_dir() && metadata.permissions().mode().trailing_zeros() >= 6,
        "unsafe private switch journal permissions"
    );
    let manifest_path = root.join("manifest.json");
    let metadata = fs::symlink_metadata(&manifest_path)?;
    ensure!(
        metadata.is_file() && metadata.permissions().mode().trailing_zeros() >= 6,
        "unsafe switch manifest"
    );
    let envelope: Envelope = serde_json::from_slice(&fs::read(manifest_path)?)?;
    ensure!(
        digest(&envelope.payload)? == envelope.checksum,
        "corrupt switch journal checksum; evidence retained"
    );
    let state: Manifest = serde_json::from_slice(&envelope.payload)?;
    ensure!(
        (state.version == 1 && state.pull.is_none()) || state.version == VERSION,
        "unsupported recovery journal version"
    );
    ensure!(
        state.git_dir == raw(&fs::canonicalize(repo.git_dir())?)?
            && state.common == raw(&fs::canonicalize(repo.common_dir())?)?
            && state.root == raw(&fs::canonicalize(repo.workdir().context("worktree required")?)?)?,
        "foreign switch journal identity"
    );
    ensure!(
        state.git_identity == directory_identity(repo.git_dir())?
            && state.root_identity == directory_identity(repo.workdir().context("worktree required")?)?
            && state.common_identity == directory_identity(repo.common_dir())?,
        "foreign repository directory identity"
    );
    ensure!(
        !state.decision || (!state.aborting && !state.preparing),
        "invalid journal phase"
    );
    ensure!(
        ["not-started", "started; outcome unknown", "succeeded", "failed"].contains(&state.hook.as_str())
            && (state.hook == "not-started" || state.decision),
        "invalid hook state"
    );
    for oid in [&state.source_oid, &state.target_oid] {
        ensure!(
            gix::ObjectId::from_hex(oid.as_bytes())?.kind() == repo.object_hash(),
            "journal hash algorithm mismatch"
        );
    }
    for branch in state.source.iter().chain(std::iter::once(&state.target)) {
        let _: gix::refs::FullName = branch.as_bstr().try_into()?;
        ensure!(branch.starts_with(b"refs/heads/"), "nonlocal journal reference");
    }
    let mut paths = BTreeSet::new();
    for change in &state.changes {
        validate_path(&change.path)?;
        ensure!(paths.insert(&change.path), "duplicate journal path");
    }
    ensure!(
        state.started.iter().all(|name| paths.contains(name)),
        "journal write-ahead path is not part of transition"
    );
    ensure!(
        state.index.path == raw(&absolute(repo, &repo.index_path()))?
            && state.head.path == raw(&absolute(repo, repo.git_dir()).join("HEAD"))?
            && state.reflog.path == raw(&absolute(repo, repo.git_dir()).join("logs/HEAD"))?,
        "foreign journal metadata path"
    );
    ensure!(state.target.starts_with(b"refs/heads/"), "invalid journal branch");
    if let Some(pull) = &state.pull {
        validate_pull(repo, &state, pull)?;
    }
    ensure!(
        state.log_directory.path == raw(&absolute(repo, repo.git_dir()).join("logs"))?,
        "foreign reflog directory path"
    );
    let mut allowed: BTreeSet<Vec<u8>> = ["manifest.json", "manifest.next", "publication", "log-directory"]
        .into_iter()
        .map(|name| name.as_bytes().to_vec())
        .collect();
    let mut directory_states = BTreeMap::new();
    if let Some(pull) = &state.pull {
        for (i, change) in pull.directories.iter().enumerate() {
            let name = format!("metadata-dir-{i}").into_bytes();
            allowed.insert(name.clone());
            directory_states.insert(name, &change.after);
        }
    }
    for change in &state.changes {
        for (prefix, desired) in [("new-dir-", &change.after), ("saved-dir-", &change.before)] {
            let name = format!("{prefix}{}", digest(&change.path)?).into_bytes();
            allowed.insert(name.clone());
            directory_states.insert(name, desired);
        }
    }
    directory_states.insert(b"log-directory".to_vec(), &state.log_directory.after);
    for position in 0..resources(repo, &state)?.len() {
        allowed.insert(format!("guard-{position}.lock").into_bytes());
    }
    for entry in fs::read_dir(&root)? {
        let entry = entry?;
        let name = raw(Path::new(&entry.file_name()))?;
        ensure!(
            allowed.contains(&name),
            "unexpected private journal member; evidence retained"
        );
        let kind = entry.file_type()?;
        if kind.is_dir() {
            ensure!(
                state.preparing
                    || directory_states
                        .get(&name)
                        .is_some_and(|desired| snapshot(&entry.path()).ok().as_ref() == Some(*desired)),
                "foreign private directory state"
            );
            ensure!(
                fs::read_dir(entry.path())?.next().is_none(),
                "unexpected private directory contents"
            );
        } else {
            ensure!(
                kind.is_file() || (name == b"publication" && kind.is_symlink()),
                "unsafe private journal member"
            );
        }
    }
    let allowed_locks: BTreeSet<Vec<u8>> = resources(repo, &state)?
        .into_iter()
        .map(|resource| {
            let mut bytes = raw(&resource)?;
            bytes.extend_from_slice(b".lock");
            Ok(bytes)
        })
        .collect::<Result<_>>()?;
    let mut seen = BTreeSet::new();
    for lock in &state.locks {
        ensure!(
            allowed_locks.contains(&lock.path) && seen.insert(&lock.path),
            "invalid journal lock record"
        );
    }

    Ok(state)
}

fn tree_entries(index: &gix::index::State) -> BTreeMap<Vec<u8>, (Mode, gix::ObjectId)> {
    index
        .entries()
        .iter()
        .map(|entry| (entry.path(index).to_vec(), (entry.mode, entry.id)))
        .collect()
}
fn overlaps(a: &[u8], b: &[u8]) -> bool {
    a == b || (a.starts_with(b) && a.get(b.len()) == Some(&b'/')) || (b.starts_with(a) && b.get(a.len()) == Some(&b'/'))
}
fn branch_oid(repo: &gix::Repository, name: &[u8]) -> Result<String> {
    let reference = repo.find_reference(name.as_bstr())?;
    ensure!(
        matches!(reference.target(), gix::refs::TargetRef::Object(_)),
        "symbolic branch references require Git"
    );
    let oid = reference.id();
    oid.object()?.try_into_commit()?;
    Ok(oid.to_string())
}
fn occupancy(repo: &gix::Repository, branch: &[u8]) -> Result<()> {
    let root = fs::canonicalize(repo.workdir().context("worktree required")?)?;
    let full: gix::refs::FullName = branch.as_bstr().try_into()?;
    if let Some(occupants) = repo.checked_out_branches_without_namespace()?.get(&full) {
        for occupant in occupants {
            ensure!(
                fs::canonicalize(occupant).ok().as_ref() == Some(&root),
                "branch occupied or reserved by {}",
                occupant.display()
            );
        }
    }
    Ok(())
}
fn refs_match(repo: &gix::Repository, state: &Manifest) -> Result<()> {
    if let Some(pull) = &state.pull {
        if pull.fetch_done {
            for change in &pull.fetch_metadata {
                ensure!(
                    snapshot(&path(&change.path)?)? == change.after,
                    "completed fetch metadata changed; recovery evidence retained"
                );
            }
            if let Some((name, _)) = &pull.tracking {
                ensure!(
                    branch_oid(repo, name)? == state.target_oid,
                    "completed tracking reference changed; recovery evidence retained"
                );
            }
        }
        let actual_head = snapshot(&path(&state.head.path)?)?;
        ensure!(
            actual_head == state.head.before || (state.decision && actual_head == state.head.after),
            "HEAD attachment changed; recovery evidence retained"
        );
        if let Some(source) = &state.source {
            let actual = branch_oid(repo, source)?;
            ensure!(
                actual == state.source_oid || (state.decision && actual == state.target_oid),
                "source branch changed; recovery evidence retained"
            );
            occupancy(repo, source)?;
        }
        return Ok(());
    }
    ensure!(
        branch_oid(repo, &state.target)? == state.target_oid,
        "target branch changed; recovery evidence retained"
    );
    if let Some(source) = &state.source {
        ensure!(
            branch_oid(repo, source)? == state.source_oid,
            "source branch changed; recovery evidence retained"
        );
    }
    occupancy(repo, &state.target)
}

pub(crate) struct HeldLock {
    native: gix::lock::File,
    resource: PathBuf,
    external: PathBuf,
    checksum: String,
}
impl Drop for HeldLock {
    fn drop(&mut self) {
        use std::os::unix::fs::MetadataExt;
        if let (Ok(anchor), Ok(external)) = (
            fs::symlink_metadata(self.native.lock_path()),
            fs::symlink_metadata(&self.external),
        ) && anchor.dev() == external.dev()
            && anchor.ino() == external.ino()
            && fs::read(&self.external)
                .ok()
                .and_then(|bytes| digest(&bytes).ok())
                .as_ref()
                == Some(&self.checksum)
        {
            let _ = fs::remove_file(&self.external);
            if let Some(parent) = self.external.parent() {
                let _ = sync_dir(parent);
            }
        }
    }
}
fn resources(repo: &gix::Repository, state: &Manifest) -> Result<Vec<PathBuf>> {
    let mut resources = vec![
        repo.index_path(),
        repo.git_dir().join("HEAD"),
        repo.common_dir().join("packed-refs"),
    ];
    for name in state.source.iter().chain(state.pull.is_none().then_some(&state.target)) {
        resources.push(repo.common_dir().join(path(name)?));
    }
    if let Some(pull) = &state.pull {
        for change in pull.fetch_metadata.iter().chain(&pull.metadata) {
            resources.push(path(&change.path)?);
        }
    }
    let mut resources: Vec<_> = resources.into_iter().map(|name| absolute(repo, &name)).collect();
    resources.sort();
    resources.dedup();
    Ok(resources)
}
fn lock_resources(repo: &gix::Repository, state: &mut Manifest) -> Result<Vec<HeldLock>> {
    #[cfg(feature = "blocking-client")]
    if state.pull.is_some() {
        for resource in resources(repo, state)? {
            pull::validate_metadata_ancestors(repo, &resource)?;
        }
    }
    let resources = resources(repo, state)?;
    let owner = format!("brit-switch-owner:{}:{}", state.pid, state.process);
    acquire_recorded_locks(&journal(repo), &resources, &owner, |lock| {
        state.locks.push(lock);
        save(repo, state)
    })
}

/// Own ordinary Git locks through private anchors. The caller durably records
/// each inode before its external lock name becomes visible.
pub(crate) fn acquire_recorded_locks(
    staging: &Path,
    resources: &[PathBuf],
    owner: &str,
    mut record: impl FnMut(Lock) -> Result<()>,
) -> Result<Vec<HeldLock>> {
    use std::os::unix::fs::MetadataExt;
    let mut locks = Vec::new();
    for (position, resource) in resources.iter().cloned().enumerate() {
        let anchor_resource = staging.join(format!("guard-{position}"));
        let mut native = gix::lock::File::acquire_to_update_resource(
            &anchor_resource,
            gix::lock::acquire::Fail::Immediately,
            None,
            0,
        )?;
        let token = format!("{owner}:{position}");
        native.write_all(token.as_bytes())?;
        native.with_mut(|file| {
            use std::os::unix::fs::PermissionsExt;
            file.set_permissions(fs::Permissions::from_mode(0o600))?;
            file.sync_all()
        })?;
        #[cfg(test)]
        durability::record_file(native.lock_path())?;
        let metadata = fs::symlink_metadata(native.lock_path())?;
        let mut external = resource.as_os_str().to_os_string();
        external.push(".lock");
        let external = PathBuf::from(external);
        // Persist identity and ownership BEFORE the ordinary lock is made visible.
        record(Lock {
            path: raw(&external)?,
            device: metadata.dev(),
            inode: metadata.ino(),
            checksum: digest(token.as_bytes())?,
        })?;
        checkpoint("lock-intent")?;
        fs::hard_link(native.lock_path(), &external)
            .with_context(|| format!("acquire ordinary lock {}", external.display()))?;
        sync_dir(external.parent().context("lock parent")?)?;
        locks.push(HeldLock {
            native,
            resource,
            external,
            checksum: digest(token.as_bytes())?,
        });
        checkpoint("lock-acquired")?;
    }
    Ok(locks)
}
struct Preparation {
    directory: PathBuf,
    armed: bool,
}
impl Drop for Preparation {
    fn drop(&mut self) {
        if self.armed {
            use std::os::unix::fs::MetadataExt;
            // Keep ownership evidence if a normal lock release failed or an owned lock
            // was edited in place. A pre-existing contended lock has a different inode.
            let manifest = fs::read(self.directory.join("manifest.json"))
                .ok()
                .and_then(|bytes| serde_json::from_slice::<Envelope>(&bytes).ok())
                .and_then(|envelope| serde_json::from_slice::<Manifest>(&envelope.payload).ok());
            let Some(manifest) = manifest else {
                return;
            };
            if manifest.locks.iter().any(|lock| {
                path(&lock.path)
                    .ok()
                    .and_then(|path| fs::symlink_metadata(path).ok())
                    .is_some_and(|metadata| metadata.dev() == lock.device && metadata.ino() == lock.inode)
            }) {
                return;
            }
            // Only used on a normal preflight return. SIGKILL keeps all ownership evidence.
            if let Ok(entries) = fs::read_dir(&self.directory) {
                for entry in entries.flatten() {
                    if entry.file_type().is_ok_and(|kind| kind.is_dir()) {
                        let _ = fs::remove_dir(entry.path());
                    } else {
                        let _ = fs::remove_file(entry.path());
                    }
                }
            }
            let _ = fs::remove_dir(&self.directory);
            if let Some(parent) = self.directory.parent() {
                let _ = sync_dir(parent);
            }
        }
    }
}

/// Switch only to an existing, direct local branch. No automatic crash recovery occurs.
pub fn execute(repo: gix::Repository, branch: &BStr) -> Result<()> {
    let shared = shared_guard(&repo)?;
    execute_locked(repo, branch, shared, None)
}
fn execute_locked(repo: gix::Repository, branch: &BStr, _shared: fs::File, pinned: Option<Manifest>) -> Result<()> {
    ensure!(
        cfg!(target_os = "linux"),
        "durable switch currently requires Linux file and directory synchronization"
    );
    if pinned.is_none() {
        ensure_no_pending(&repo)?;
    }
    ensure!(
        std::env::var_os("GIT_INDEX_FILE").is_none(),
        "alternate indexes require Git"
    );
    ensure!(repo.namespace().is_none(), "reference namespaces require Git");
    let repo = repo.with_object_memory();
    let root = repo.workdir().context("worktree required")?;
    let git_dir = repo.git_dir();
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
            !git_dir.join(marker).exists(),
            "repository operation in progress: {marker}"
        );
    }
    let target = if branch.starts_with(b"refs/heads/") {
        branch.to_vec()
    } else {
        ensure!(
            !branch.starts_with(b"refs/") && branch != b"-".as_bstr() && !branch.starts_with(b"-"),
            "existing local branch name required"
        );
        [b"refs/heads/".as_slice(), branch.as_bytes()].concat()
    };
    let _: gix::refs::FullName = target.as_bstr().try_into()?;
    let pulling = pinned.as_ref().is_some_and(|state| state.pull.is_some());
    let target_oid = if pulling {
        pinned.as_ref().context("pull state")?.target_oid.clone()
    } else {
        branch_oid(&repo, &target)?
    };
    let head = repo.head()?;
    let source = head.referent_name().map(|name| name.as_bstr().to_vec());
    if let Some(source) = &source {
        ensure!(
            source.starts_with(b"refs/heads/"),
            "non-local HEAD attachment requires Git"
        );
        branch_oid(&repo, source)?;
    }
    let source_oid = repo.head_commit()?.id().to_string();
    if let Some(previous) = &pinned {
        ensure!(
            previous.source == source
                && previous.source_oid == source_oid
                && previous.target == target
                && previous.target_oid == target_oid,
            "pinned switch references changed during preparation recovery"
        );
        ensure!(
            snapshot(&repo.index_path())? == previous.index.before
                && snapshot(&repo.git_dir().join("HEAD"))? == previous.head.before
                && snapshot(&repo.git_dir().join("logs/HEAD"))? == previous.reflog.before,
            "pinned metadata changed during preparation recovery"
        );
    }
    if !pulling {
        occupancy(&repo, &target)?;
    }
    ensure!(
        gix::filter::Pipeline::options(&repo)?.drivers.is_empty(),
        "external filters require Git"
    );
    super::super::authoring::hook_path(&repo, if pulling { "post-merge" } else { "post-checkout" })?;
    let mut state = Manifest {
        version: VERSION,
        pull: pinned.as_ref().and_then(|state| state.pull.clone()),
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
        source_oid,
        target,
        target_oid,
        started: BTreeSet::new(),
        changes: Vec::new(),
        index: Change {
            path: raw(&absolute(&repo, &repo.index_path()))?,
            before: State::Absent,
            after: State::Absent,
        },
        head: Change {
            path: raw(&absolute(&repo, git_dir).join("HEAD"))?,
            before: snapshot(&git_dir.join("HEAD"))?,
            after: State::Absent,
        },
        reflog: Change {
            path: raw(&absolute(&repo, git_dir).join("logs/HEAD"))?,
            before: State::Absent,
            after: State::Absent,
        },
        log_directory: Change {
            path: raw(&absolute(&repo, git_dir).join("logs"))?,
            before: snapshot(&git_dir.join("logs"))?,
            after: snapshot(&git_dir.join("logs"))?,
        },
        preparing: true,
        decision: false,
        aborting: false,
        hook: "not-started".into(),
    };
    use std::os::unix::fs::DirBuilderExt;
    sync_dir(root)?;
    sync_dir(git_dir)?;
    if pinned.is_none() {
        fs::DirBuilder::new().mode(0o700).create(journal(&repo))?;
    }
    let mut preparation = Preparation {
        directory: journal(&repo),
        armed: pinned.is_none(),
    };
    state.index.before = snapshot(&repo.index_path())?;
    state.reflog.before = snapshot(&git_dir.join("logs/HEAD"))?;
    save(&repo, &state)?;
    sync_dir(git_dir)?;
    ensure!(
        matches!(state.log_directory.before, State::Absent | State::Directory { .. }),
        "unsafe reflog directory"
    );
    let mut locks = lock_resources(&repo, &mut state)?;
    refs_match(&repo, &state)?;
    ensure!(
        snapshot(&git_dir.join("HEAD"))? == state.head.before,
        "HEAD changed before lock acquisition"
    );
    let index = read_index(&repo)?;
    let source = repo.index_from_tree(&repo.head_commit()?.tree_id()?)?;
    let target_commit = repo
        .find_object(gix::ObjectId::from_hex(state.target_oid.as_bytes())?)?
        .try_into_commit()?;
    let destination = repo.index_from_tree(&target_commit.tree_id()?)?;
    let source_entries = tree_entries(&source);
    let destination_entries = tree_entries(&destination);
    let index_entries = tree_entries(&index);
    let changed: BTreeSet<_> = source_entries
        .keys()
        .chain(destination_entries.keys())
        .filter(|path| source_entries.get(*path) != destination_entries.get(*path))
        .cloned()
        .collect();
    for (name, entry) in &index_entries {
        if changed.iter().any(|changed| overlaps(name, changed)) {
            ensure!(
                source_entries.get(name) == Some(entry),
                "staged changes overlap switch: {}",
                name.as_bstr()
            );
        }
    }
    for name in &changed {
        validate_path(name)?;
        for entries in [&source_entries, &destination_entries, &index_entries] {
            if let Some((mode, _)) = entries.get(name) {
                ensure!(
                    matches!(*mode, Mode::FILE | Mode::FILE_EXECUTABLE | Mode::SYMLINK),
                    "affected gitlink or unsupported mode: {}",
                    name.as_bstr()
                );
            }
        }
        if let Some((mode, oid)) = source_entries.get(name) {
            ensure!(
                index_entries.get(name) == Some(&(*mode, *oid)),
                "staged changes overlap switch: {}",
                name.as_bstr()
            );
            let actual = worktree_entry(&repo, &index, &BString::from(name.clone()))?;
            let blob = repo.find_object(*oid)?.try_into_blob()?;
            ensure!(
                actual
                    .as_ref()
                    .is_some_and(|(actual_mode, bytes)| actual_mode == mode && *bytes == blob.data),
                "unstaged changes overlap switch: {}",
                name.as_bstr()
            );
        }
    }
    if repo.config_snapshot().try_boolean("core.ignorecase")? == Some(true) {
        bail!("case-insensitive index semantics require Git");
    }
    if repo.config_snapshot().try_boolean("core.precomposeunicode")? == Some(true)
        && changed.iter().any(|name| !name.is_ascii())
    {
        bail!("Unicode precomposition requires Git");
    }
    // Git checkout applies the intended index attributes; dirty unstaged attribute files
    // survive as neighbors but do not silently alter newly checked-out bytes.
    let mut new_index = gix::index::State::new(repo.object_hash());
    for entry in index.entries() {
        let name = entry.path(&index);
        if !changed.contains(name.as_bytes()) {
            new_index.dangerously_push_entry(
                entry.stat,
                entry.id,
                entry.flags - Flags::FSMONITOR_VALID,
                entry.mode,
                name,
            );
        }
    }
    for name in &changed {
        if let Some((mode, oid)) = destination_entries.get(name) {
            new_index.dangerously_push_entry(Stat::default(), *oid, Flags::empty(), *mode, name.as_bstr());
        }
    }
    new_index.sort_entries();
    let attrs = repo.attributes_only(&new_index, gix::worktree::stack::state::attributes::Source::IdMapping)?;
    let mut pipeline = gix::filter::Pipeline::new(&repo, attrs.detach())?;
    let mut changes = BTreeMap::<Vec<u8>, Change>::new();
    for name in &changed {
        let absolute = root.join(path(name)?);
        let before = snapshot(&absolute)?;
        if !source_entries.contains_key(name) {
            ensure!(
                matches!(before, State::Absent | State::Directory { .. }),
                "untracked or ignored collision: {}",
                name.as_bstr()
            );
        }
        let after = if let Some((mode, oid)) = destination_entries.get(name) {
            let blob = repo.find_object(*oid)?.try_into_blob()?;
            if *mode == Mode::SYMLINK {
                ensure!(
                    !blob.data.is_empty() && !blob.data.contains(&0),
                    "invalid symlink target"
                );
                if repo.config_snapshot().try_boolean("core.symlinks")? == Some(false) {
                    State::File {
                        mode: creation_mode(0o666)?,
                        bytes: blob.data.clone(),
                    }
                } else {
                    State::Symlink(blob.data.clone())
                }
            } else {
                let mut bytes = Vec::new();
                pipeline
                    .convert_to_worktree(&blob.data, name.as_bstr(), Default::default())?
                    .read_to_end(&mut bytes)?;
                State::File {
                    mode: creation_mode(if *mode == Mode::FILE_EXECUTABLE { 0o777 } else { 0o666 })?,
                    bytes,
                }
            }
        } else {
            State::Absent
        };
        changes.insert(
            name.clone(),
            Change {
                path: name.clone(),
                before,
                after,
            },
        );
    }
    complete_directory_ledger(
        root,
        &source_entries.keys().cloned().collect(),
        &destination_entries.keys().cloned().collect(),
        &mut changes,
    )?;
    state.changes = changes
        .into_values()
        .filter(|change| change.before != change.after)
        .collect();
    let mut index_bytes = Vec::new();
    gix::index::File::from_state(new_index, repo.index_path()).write_to(&mut index_bytes, Default::default())?;
    state.index.before = snapshot(&repo.index_path())?;
    state.index.after = State::File {
        mode: file_mode(&state.index.before, 0o644),
        bytes: index_bytes,
    };
    state.head.after = State::File {
        mode: file_mode(&state.head.before, 0o644),
        bytes: [b"ref: ".as_slice(), &state.target, b"\n"].concat(),
    };
    state.reflog.before = snapshot(&git_dir.join("logs/HEAD"))?;
    let mut log = match &state.reflog.before {
        State::Absent => Vec::new(),
        State::File { bytes, .. } => bytes.clone(),
        _ => bail!("unsupported HEAD reflog"),
    };
    if reflog_enabled(&repo)? || state.reflog.before != State::Absent {
        let signature = repo
            .committer()
            .context("committer identity required for checkout reflog")??
            .to_owned()?;
        let from = state
            .source
            .as_ref()
            .map(|name| name.strip_prefix(b"refs/heads/").unwrap_or(name).as_bstr().to_owned())
            .unwrap_or_else(|| state.source_oid.clone().into());
        gix::refs::log::Line {
            previous_oid: gix::ObjectId::from_hex(state.source_oid.as_bytes())?,
            new_oid: gix::ObjectId::from_hex(state.target_oid.as_bytes())?,
            signature,
            message: format!(
                "checkout: moving from {from} to {}",
                state
                    .target
                    .strip_prefix(b"refs/heads/")
                    .unwrap_or(&state.target)
                    .as_bstr()
            )
            .into(),
        }
        .write_to(&mut log)?;
        state.reflog.after = State::File {
            mode: file_mode(&state.reflog.before, 0o644),
            bytes: log,
        };
    } else {
        state.reflog.after = State::Absent;
    }
    if pulling {
        prepare_pull_metadata(&repo, &mut state)?;
    }
    // Prove directory fsync support before creating any durable live transition.
    sync_dir(root)?;
    sync_dir(git_dir)?;
    sync_dir(repo.common_dir())?;
    use std::os::unix::fs::MetadataExt;
    let journal_device = fs::metadata(git_dir)?.dev();
    for change in &state.changes {
        let mut parent = root.join(path(&change.path)?);
        parent.pop();
        while !parent.is_dir() {
            ensure!(parent.pop(), "missing filesystem ancestor");
        }
        ensure!(
            fs::metadata(&parent)?.dev() == journal_device,
            "cross-filesystem switch publication is unsupported: {}",
            change.path.as_bstr()
        );
    }
    // A preparation crash can leave private directories, so persist their reserved names first.
    save(&repo, &state)?;
    prepare_directories(&journal(&repo), &mut state.changes)?;
    if state.reflog.after != State::Absent && state.log_directory.before == State::Absent {
        let directory = journal(&repo).join("log-directory");
        fs::DirBuilder::new().mode(creation_mode(0o777)?).create(&directory)?;
        sync_dir(&directory)?;
        state.log_directory.after = snapshot(&directory)?;
    }
    state.preparing = false;
    save(&repo, &state)?;
    sync_dir(git_dir)?;
    preparation.armed = false;
    checkpoint("journal-prepared")?;
    finish(&repo, &mut state, &mut locks)
}
fn reflog_enabled(repo: &gix::Repository) -> Result<bool> {
    let config = repo.config_snapshot();
    if config
        .string("core.logAllRefUpdates")
        .is_some_and(|value| value.as_slice() == b"always")
    {
        return Ok(true);
    }
    Ok(config.try_boolean("core.logAllRefUpdates")? != Some(false))
}

pub(crate) fn creation_mode(mode: u32) -> Result<u32> {
    let status = fs::read_to_string("/proc/self/status")?;
    let mask = status
        .lines()
        .find_map(|line| line.strip_prefix("Umask:"))
        .context("cannot read process umask")?;
    Ok(mode & !u32::from_str_radix(mask.trim(), 8)?)
}

fn file_mode(state: &State, fallback: u32) -> u32 {
    if let State::File { mode, .. } = state {
        *mode
    } else {
        fallback
    }
}
/// Complete the explicit ancestor/descendant ledger before any live checkout.
/// Source and destination names come from the pinned trees, never a filesystem walk.
/// New directory identities are reserved separately before the publication decision.
pub(crate) fn complete_directory_ledger(
    root: &Path,
    source_paths: &BTreeSet<Vec<u8>>,
    destination_paths: &BTreeSet<Vec<u8>>,
    changes: &mut BTreeMap<Vec<u8>, Change>,
) -> Result<()> {
    let leaves: Vec<_> = changes.keys().cloned().collect();
    // Ancestors are explicit journal resources. This supports file/directory transitions and
    // retains unrelated directory contents rather than recursively deleting them.
    for name in &leaves {
        let mut ancestor = path(name)?;
        while ancestor.pop() && !ancestor.as_os_str().is_empty() {
            let relative = raw(&ancestor)?;
            let needs_directory = destination_paths
                .iter()
                .any(|other| other.starts_with(&relative) && other.get(relative.len()) == Some(&b'/'));
            let before = snapshot(&root.join(&ancestor))?;
            let replaced_ancestor = destination_paths
                .iter()
                .any(|other| relative.starts_with(other) && relative.get(other.len()) == Some(&b'/'));
            let after = if needs_directory || (matches!(before, State::Directory { .. }) && !replaced_ancestor) {
                if matches!(before, State::Directory { .. }) {
                    before.clone()
                } else {
                    State::Directory {
                        mode: creation_mode(0o777)?,
                        device: 0,
                        inode: 0,
                    }
                }
            } else {
                State::Absent
            };
            changes
                .entry(relative.clone())
                .and_modify(|change| {
                    if needs_directory {
                        change.after = after.clone();
                    }
                })
                .or_insert(Change {
                    path: relative,
                    before,
                    after,
                });
        }
    }
    for change in changes.values() {
        if change.before != change.after && matches!(change.before, State::File { .. } | State::Symlink(_)) {
            ensure!(
                source_paths.contains(&change.path),
                "untracked or ignored ancestor collision: {}",
                change.path.as_bstr()
            );
        }
        if matches!(change.before, State::Directory { .. }) && !matches!(change.after, State::Directory { .. }) {
            inspect_removed_directory(root, &change.path, changes)?;
        }
        let relative = path(&change.path)?;
        let mut ancestor = root.to_owned();
        for part in relative.components() {
            ancestor.push(part);
            if ancestor.join(".git").exists() {
                bail!("affected nested repository {}", ancestor.display());
            }
            if matches!(snapshot(&ancestor)?, State::Symlink(_)) && ancestor != root.join(&relative) {
                bail!("symlink ancestor {}", ancestor.display());
            }
        }
    }
    Ok(())
}

/// Materialize reserved empty directories while still in private preparation.
/// The caller pins their names durably first and persists the resulting identities
/// before permitting publication. Existing names refuse; recovery owns their disposition.
pub(crate) fn prepare_directories(staging: &Path, changes: &mut [Change]) -> Result<()> {
    for change in changes {
        if let State::Directory { mode, inode: 0, .. } = change.after {
            use std::os::unix::fs::PermissionsExt;
            let directory = staging.join(format!("new-dir-{}", digest(&change.path)?));
            fs::create_dir(&directory)?;
            fs::set_permissions(&directory, fs::Permissions::from_mode(mode))?;
            sync_dir(&directory)?;
            change.after = snapshot(&directory)?;
        }
    }
    Ok(())
}

fn inspect_removed_directory(root: &Path, name: &[u8], changes: &BTreeMap<Vec<u8>, Change>) -> Result<()> {
    for entry in fs::read_dir(root.join(path(name)?))? {
        let entry = entry?;
        let relative = raw(entry.path().strip_prefix(root)?)?;
        let change = changes
            .get(&relative)
            .with_context(|| format!("untracked or ignored collision: {}", relative.as_bstr()))?;
        ensure!(
            matches!(change.after, State::Absent),
            "directory collision: {}",
            relative.as_bstr()
        );
        if entry.file_type()?.is_dir() {
            inspect_removed_directory(root, &relative, changes)?;
        }
    }
    Ok(())
}

pub(crate) fn checkpoint(name: &str) -> Result<()> {
    #[cfg(feature = "switch-test-faults")]
    if std::env::var("BRIT_SWITCH_CHECKPOINT").ok().as_deref() == Some(name) {
        let directory =
            PathBuf::from(std::env::var_os("BRIT_SWITCH_CHECKPOINT_DIR").context("checkpoint directory required")?);
        fs::write(directory.join(format!("{name}.reached")), b"reached")?;
        while !directory.join(format!("{name}.release")).exists() {
            if gix::interrupt::is_triggered() {
                bail!("switch interrupted; explicit recovery required");
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
    }
    let _ = name;
    if gix::interrupt::is_triggered() {
        bail!("switch interrupted; explicit recovery required");
    }
    Ok(())
}

fn expected(actual: &State, change: &Change) -> bool {
    actual == &change.before
        || actual == &change.after
        || (*actual == State::Absent
            && (matches!(change.before, State::Directory { .. }) || matches!(change.after, State::Directory { .. })))
}
pub(crate) fn verify_files(root: &Path, changes: &[Change], started: &BTreeSet<Vec<u8>>) -> Result<()> {
    for change in changes {
        let actual = snapshot(&root.join(path(&change.path)?))?;
        ensure!(
            expected(&actual, change),
            "foreign filesystem state at {}; recovery evidence retained",
            change.path.as_bstr()
        );
        ensure!(
            actual != State::Absent
                || actual == change.before
                || actual == change.after
                || started.contains(&change.path),
            "unstarted directory transition disappeared: {}",
            change.path.as_bstr()
        );
    }
    Ok(())
}
pub(crate) fn publish_file(staging: &Path, root: &Path, change: &Change, desired: &State) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    validate_ancestors(root, &change.path)?;
    let absolute = root.join(path(&change.path)?);
    let actual = snapshot(&absolute)?;
    ensure!(
        expected(&actual, change),
        "foreign filesystem state at {}",
        change.path.as_bstr()
    );
    if &actual == desired {
        if matches!(actual, State::File { .. } | State::Directory { .. }) {
            sync_path(&absolute)?;
        }
        sync_dir(absolute.parent().context("file parent required")?)?;
        return Ok(());
    }
    let parent = absolute.parent().context("file parent required")?;
    match desired {
        State::Absent => {
            if matches!(actual, State::Directory { .. }) {
                ensure!(
                    fs::read_dir(&absolute)?.next().is_none(),
                    "directory contains foreign files: {}",
                    change.path.as_bstr()
                );
                let prefix = if actual == change.before {
                    "saved-dir-"
                } else {
                    "new-dir-"
                };
                let destination = staging.join(format!("{prefix}{}", digest(&change.path)?));
                ensure!(!present(&destination)?, "private directory state conflict");
                fs::rename(&absolute, destination)?;
                sync_dir(staging)?;
            } else if actual != State::Absent {
                fs::remove_file(&absolute)?;
            }
        }
        State::Directory { .. } => {
            ensure!(actual == State::Absent, "directory transition was not prepared");
            let prefix = if desired == &change.before {
                "saved-dir-"
            } else {
                "new-dir-"
            };
            let source = staging.join(format!("{prefix}{}", digest(&change.path)?));
            ensure!(snapshot(&source)? == *desired, "private directory state changed");
            fs::rename(source, &absolute)?;
            sync_dir(staging)?;
            sync_dir(&absolute)?;
        }
        State::File { mode, bytes } => {
            ensure!(
                !matches!(actual, State::Directory { .. }),
                "directory transition was not prepared"
            );
            let staged = staging.join("publication");
            if fs::symlink_metadata(&staged).is_ok() {
                fs::remove_file(&staged)?;
            }
            private_write(&staged, bytes)?;
            fs::set_permissions(&staged, fs::Permissions::from_mode(*mode))?;
            sync_path(&staged)?;
            sync_dir(staging)?;
            ensure!(
                snapshot(&absolute)? == actual,
                "filesystem changed before publication: {}",
                change.path.as_bstr()
            );
            fs::rename(&staged, &absolute)?;
            sync_dir(staging)?;
        }
        State::Symlink(bytes) => {
            ensure!(
                !matches!(actual, State::Directory { .. }),
                "directory transition was not prepared"
            );
            let staged = staging.join("publication");
            if fs::symlink_metadata(&staged).is_ok() {
                fs::remove_file(&staged)?;
            }
            std::os::unix::fs::symlink(path(bytes)?, &staged)?;
            sync_dir(staging)?;
            ensure!(
                snapshot(&absolute)? == actual,
                "filesystem changed before publication: {}",
                change.path.as_bstr()
            );
            fs::rename(&staged, &absolute)?;
            sync_dir(staging)?;
        }
    }
    sync_dir(parent)?;
    if let State::File { .. } = desired {
        sync_path(&absolute)?;
    }
    Ok(())
}
fn apply_files(repo: &gix::Repository, state: &mut Manifest) -> Result<()> {
    let root = path(&state.root)?;
    verify_files(&root, &state.changes, &state.started)?;
    let aborting = state.aborting;
    let desired = |change: &Change| {
        if aborting {
            change.before.clone()
        } else {
            change.after.clone()
        }
    };
    let mut ordered: Vec<_> = state.changes.clone();
    ordered.sort_by_key(|change| std::cmp::Reverse(change.path.len()));
    // Remove obsolete entries from children to parents. Empty-directory-only removal never
    // discards untracked children that arrived after preflight.
    for change in &ordered {
        let target = desired(change);
        let actual = snapshot(&root.join(path(&change.path)?))?;
        let must_remove = target == State::Absent
            || (actual != State::Absent
                && ((matches!(target, State::Directory { .. })) != (matches!(actual, State::Directory { .. }))));
        if must_remove && actual != State::Absent {
            if state.started.insert(change.path.clone()) {
                save(repo, state)?;
            }
            publish_file(&journal(repo), &root, change, &State::Absent)?;
            checkpoint(if state.aborting { "abort-file" } else { "file-published" })?;
        }
    }
    ordered.reverse();
    for change in &ordered {
        let target = desired(change);
        if target != State::Absent {
            if state.started.insert(change.path.clone()) {
                save(repo, state)?;
            }
            publish_file(&journal(repo), &root, change, &target)?;
            checkpoint(if state.aborting { "abort-file" } else { "file-published" })?;
        }
    }
    // Any fresh external edit before the durable decision is a conflict.
    for change in &state.changes {
        ensure!(
            snapshot(&root.join(path(&change.path)?))? == desired(change),
            "filesystem changed during switch: {}",
            change.path.as_bstr()
        );
    }
    sync_dir(&root)?;
    Ok(())
}
fn publish_metadata(repo: &gix::Repository, change: &Change, locks: &mut [HeldLock]) -> Result<()> {
    publish_metadata_at(repo, &journal(repo), change, locks)
}

/// Publish the exact metadata image using the shared owned-lock protocol.
pub(crate) fn publish_metadata_at(
    repo: &gix::Repository,
    staging: &Path,
    change: &Change,
    locks: &mut [HeldLock],
) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let resource = path(&change.path)?;
    #[cfg(feature = "blocking-client")]
    pull::validate_metadata_ancestors(repo, &resource)?;
    let actual = snapshot(&resource)?;
    ensure!(
        actual == change.before || actual == change.after,
        "foreign metadata state at {}",
        resource.display()
    );
    if actual == change.after {
        if actual != State::Absent {
            sync_path(&resource)?;
        }
        sync_dir(resource.parent().context("metadata parent")?)?;
        return Ok(());
    }
    let lock = locks
        .iter()
        .find(|lock| {
            lock.resource == resource
                || (resource == absolute(repo, repo.git_dir()).join("logs/HEAD")
                    && lock.resource == absolute(repo, repo.git_dir()).join("HEAD"))
        })
        .context("missing metadata guard")?;
    ensure!(
        digest(&fs::read(&lock.external)?)? == lock.checksum,
        "metadata lock changed"
    );
    if change.after == State::Absent {
        ensure!(snapshot(&resource)? == actual, "metadata changed before deletion");
        fs::remove_file(&resource)?;
        return sync_dir(resource.parent().context("metadata parent")?);
    }
    let State::File { mode, bytes } = &change.after else {
        bail!("invalid prepared metadata");
    };
    let staged = staging.join("publication");
    if fs::symlink_metadata(&staged).is_ok() {
        fs::remove_file(&staged)?;
    }
    private_write(&staged, bytes)?;
    fs::set_permissions(&staged, fs::Permissions::from_mode(*mode))?;
    sync_path(&staged)?;
    sync_dir(staging)?;
    ensure!(
        snapshot(&resource)? == actual,
        "metadata changed before publication: {}",
        resource.display()
    );
    fs::rename(&staged, &resource)?;
    sync_dir(staging)?;
    sync_dir(resource.parent().context("metadata parent")?)
}

fn cleanup(repo: &gix::Repository, state: &Manifest) -> Result<()> {
    checkpoint("cleanup")?;
    #[cfg(feature = "blocking-client")]
    if let Some(pull) = &state.pull {
        pull::check_keeps(repo, pull)?;
    }
    let active = journal(repo);
    let completed = repo.git_dir().join(format!("brit-switch-completed-{}", state.pid));
    ensure!(
        !completed.exists(),
        "completed journal tombstone already exists; evidence retained"
    );
    fs::rename(&active, &completed)?;
    sync_dir(repo.git_dir())?;
    if let Some(pull) = &state.pull {
        #[cfg(feature = "blocking-client")]
        pull::cleanup_keeps(repo, pull)?;
        for change in pull.directories.iter().rev() {
            let directory = path(&change.path)?;
            if change.after != State::Absent
                && snapshot(&directory)? == change.after
                && fs::read_dir(&directory)?.next().is_none()
            {
                fs::remove_dir(&directory)?;
                sync_dir(directory.parent().context("metadata parent")?)?;
            }
        }
    }
    checkpoint("keeps-cleaned")?;
    // Once the rename is durable, partial garbage collection is never mistaken for an active journal.
    fs::remove_file(completed.join("manifest.json"))?;
    for name in ["manifest.next", "publication"] {
        if fs::symlink_metadata(completed.join(name)).is_ok() {
            fs::remove_file(completed.join(name))?;
        }
    }
    for change in &state.changes {
        for prefix in ["new-dir-", "saved-dir-"] {
            let directory = completed.join(format!("{prefix}{}", digest(&change.path)?));
            if present(&directory)? {
                fs::remove_dir(directory)?;
            }
        }
    }
    if present(&completed.join("log-directory"))? {
        fs::remove_dir(completed.join("log-directory"))?;
    }
    if let Some(pull) = &state.pull {
        for (i, _) in pull.directories.iter().enumerate() {
            let directory = completed.join(format!("metadata-dir-{i}"));
            if present(&directory)? {
                fs::remove_dir(directory)?;
            }
        }
    }
    fs::remove_dir(&completed)?;
    sync_dir(repo.git_dir())
}
pub(crate) fn verify_locks(locks: &[HeldLock]) -> Result<()> {
    use std::os::unix::fs::MetadataExt;
    for lock in locks {
        let actual = fs::symlink_metadata(&lock.external)?;
        let original = fs::symlink_metadata(lock.native.lock_path())?;
        ensure!(
            actual.is_file()
                && actual.dev() == original.dev()
                && actual.ino() == original.ino()
                && digest(&fs::read(&lock.external)?)? == lock.checksum,
            "foreign lock state at {}",
            lock.external.display()
        );
    }
    Ok(())
}

fn release_locks(repo: &gix::Repository, locks: &mut Vec<HeldLock>) -> Result<()> {
    verify_locks(locks)?;
    for lock in locks.iter() {
        fs::remove_file(&lock.external)?;
        sync_dir(lock.external.parent().context("lock parent")?)?;
    }
    locks.clear();
    sync_dir(&journal(repo))
}

fn finish(repo: &gix::Repository, state: &mut Manifest, locks: &mut Vec<HeldLock>) -> Result<()> {
    verify_locks(locks)?;
    if state.hook != "not-started" {
        if state.hook.starts_with("started") {
            eprintln!(
                "{} committed; hook outcome unknown; hook was not replayed",
                if state.pull.is_some() { "pull" } else { "switch" }
            );
        }
        release_locks(repo, locks)?;
        cleanup(repo, state)?;
        ensure!(
            state.pull.is_some() || state.hook != "failed",
            "switch committed; post-checkout hook failed; hook was not replayed"
        );
        return Ok(());
    }
    refs_match(repo, state)?;
    for change in [&state.index, &state.head, &state.reflog]
        .into_iter()
        .chain(state.pull.iter().flat_map(|pull| pull.metadata.iter()))
    {
        let actual = snapshot(&path(&change.path)?)?;
        ensure!(
            actual == change.before || (state.decision && actual == change.after),
            "foreign metadata state at {}",
            change.path.as_bstr()
        );
    }
    #[cfg(feature = "blocking-client")]
    if let Some(pull) = &state.pull {
        pull::validate_objects(repo, pull)?;
    }
    apply_files(repo, state)?;
    if state.aborting {
        release_locks(repo, locks)?;
        cleanup(repo, state)?;
        return Ok(());
    }
    verify_locks(locks)?;
    if !state.decision {
        refs_match(repo, state)?;
        state.decision = true;
        save(repo, state)?;
    }
    checkpoint("commit-decision")?;
    #[cfg(feature = "blocking-client")]
    if let Some(pull) = &state.pull {
        pull::validate_objects(repo, pull)?;
    }
    refs_match(repo, state)?;
    publish_metadata(repo, &state.index, locks)?;
    checkpoint("index-published")?;
    refs_match(repo, state)?;
    verify_locks(locks)?;
    publish_metadata(repo, &state.head, locks)?;
    checkpoint("head-published")?;
    refs_match(repo, state)?;
    verify_locks(locks)?;
    let log_path = path(&state.log_directory.path)?;
    let actual = snapshot(&log_path)?;
    ensure!(
        actual == state.log_directory.before || actual == state.log_directory.after,
        "foreign reflog directory state"
    );
    if actual != state.log_directory.after {
        let prepared = journal(repo).join("log-directory");
        ensure!(
            snapshot(&prepared)? == state.log_directory.after,
            "private reflog directory changed"
        );
        fs::rename(prepared, &log_path)?;
        sync_dir(&log_path)?;
        sync_dir(repo.git_dir())?;
        sync_dir(&journal(repo))?;
    }
    publish_metadata(repo, &state.reflog, locks)?;
    if let Some(pull) = &state.pull {
        for change in &pull.metadata {
            refs_match(repo, state)?;
            verify_locks(locks)?;
            publish_metadata(repo, change, locks)?;
            checkpoint(if change.path.ends_with(b"ORIG_HEAD") {
                "orig-head-published"
            } else if change.path.windows(5).any(|part| part == b"logs/") {
                "branch-reflog-published"
            } else {
                "branch-published"
            })?;
        }
    }
    checkpoint("reflog-published")?;
    refs_match(repo, state)?;
    verify_locks(locks)?;
    sync_dir(repo.git_dir())?;
    let hook_result = if state.pull.as_ref().is_some_and(|pull| pull.no_op) {
        state.hook = "succeeded".into();
        save(repo, state)?;
        Ok(())
    } else if state.hook == "not-started" {
        state.hook = "started; outcome unknown".into();
        save(repo, state)?;
        release_locks(repo, locks)?;
        checkpoint("hook-started")?;
        let result = super::super::authoring::run_hook(
            repo,
            if state.pull.is_some() {
                "post-merge"
            } else {
                "post-checkout"
            },
            &if state.pull.is_some() {
                vec!["0".into()]
            } else {
                vec![
                    state.source_oid.clone().into(),
                    state.target_oid.clone().into(),
                    "1".into(),
                ]
            },
        );
        state.hook = if result.is_ok() { "succeeded" } else { "failed" }.into();
        save(repo, state)?;
        result
    } else {
        if state.hook.starts_with("started") {
            eprintln!(
                "{} committed; hook outcome unknown; hook was not replayed",
                if state.pull.is_some() { "pull" } else { "switch" }
            );
        }
        Ok(())
    };
    release_locks(repo, locks)?;
    cleanup(repo, state)?;
    if state.pull.is_some() {
        if let Err(error) = hook_result {
            eprintln!("pull committed; post-merge hook failed: {error}");
        }
        Ok(())
    } else {
        hook_result.context("switch committed; post-checkout hook failed")
    }
}

/// Explicitly continue or abort an interrupted switch. A durable decision only rolls forward.
pub fn recover(repo: gix::Repository, abort: bool) -> Result<()> {
    recover_operation(repo, abort, false)
}
fn recover_operation(repo: gix::Repository, abort: bool, pulling: bool) -> Result<()> {
    use std::os::unix::fs::MetadataExt;
    ensure!(cfg!(target_os = "linux"), "durable switch recovery requires Linux");
    let _shared = shared_guard(&repo)?;
    if !present(&journal(&repo))? {
        bail!("no switch recovery pending");
    }
    let mut state = load(&repo)?;
    ensure!(
        state.pull.is_some() == pulling,
        "pending {} operation; use brit {} --continue or --abort",
        if state.pull.is_some() { "pull" } else { "switch" },
        if state.pull.is_some() { "pull" } else { "switch" }
    );
    if abort {
        ensure!(
            !state.decision,
            "switch commit decision is durable; --abort cannot roll back; use --continue"
        );
    }
    let owner = process(state.pid);
    match owner {
        Ok(identity) => ensure!(
            identity != state.process,
            "switch owner is still alive; recovery refused"
        ),
        Err(error) => ensure!(
            error
                .downcast_ref::<std::io::Error>()
                .is_some_and(|error| error.kind() == std::io::ErrorKind::NotFound),
            "cannot establish switch owner death: {error}"
        ),
    }
    // Establish every lock's ownership before removing any. A changed inode is a foreign lock.
    let allowed_locks: BTreeSet<Vec<u8>> = resources(&repo, &state)?
        .into_iter()
        .map(|resource| {
            let mut bytes = raw(&resource)?;
            bytes.extend_from_slice(b".lock");
            Ok(bytes)
        })
        .collect::<Result<_>>()?;
    for lock in &state.locks {
        let lock_path = path(&lock.path)?;
        ensure!(allowed_locks.contains(&lock.path), "invalid journal lock path");
        match fs::symlink_metadata(&lock_path) {
            Ok(metadata) => ensure!(
                metadata.is_file()
                    && metadata.dev() == lock.device
                    && metadata.ino() == lock.inode
                    && digest(&fs::read(&lock_path)?)? == lock.checksum,
                "foreign lock {}; evidence retained",
                lock_path.display()
            ),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
    }
    for lock in &state.locks {
        let lock_path = path(&lock.path)?;
        if lock_path.exists() {
            fs::remove_file(&lock_path)?;
            sync_dir(lock_path.parent().context("lock parent")?)?;
        }
    }
    for entry in fs::read_dir(journal(&repo))? {
        let entry = entry?;
        if entry.file_name().to_string_lossy().starts_with("guard-") {
            fs::remove_file(entry.path())?;
        }
    }
    state.aborting |= abort;
    if state.aborting {
        save(&repo, &state)?;
    }
    let abort = state.aborting;
    #[cfg(feature = "blocking-client")]
    if state.pull.as_ref().is_some_and(|pull| !pull.fetch_done) {
        state.pid = std::process::id();
        state.process = process(state.pid)?;
        state.locks.clear();
        save(&repo, &state)?;
        return pull::resume_fetch(repo, state, _shared, abort);
    }
    if state.preparing {
        for change in [&state.index, &state.head, &state.reflog] {
            ensure!(
                snapshot(&path(&change.path)?)? == change.before,
                "metadata changed during interrupted preparation"
            );
        }
        refs_match(&repo, &state)?;
        if abort {
            cleanup(&repo, &state)?;
            return Ok(());
        }
        for change in &state.changes {
            let temporary = journal(&repo).join(format!("new-dir-{}", digest(&change.path)?));
            if present(&temporary)? {
                fs::remove_dir(temporary)?;
            }
        }
        if present(&journal(&repo).join("log-directory"))? {
            fs::remove_dir(journal(&repo).join("log-directory"))?;
        }
        sync_dir(&journal(&repo))?;
        #[cfg(feature = "blocking-client")]
        if state.pull.is_some() {
            return pull::integrate(repo, state, _shared, false);
        }
        let target = BString::from(state.target.clone());
        return execute_locked(repo, target.as_bstr(), _shared, Some(state));
    }
    state.pid = std::process::id();
    state.process = process(state.pid)?;
    state.locks.clear();
    state.aborting |= abort;
    save(&repo, &state)?;
    let mut locks = lock_resources(&repo, &mut state)?;
    finish(&repo, &mut state, &mut locks)
}

#[cfg(test)]
#[path = "../durability.rs"]
mod durability;

#[cfg(feature = "blocking-client")]
use pull::{prepare_pull_metadata, validate_pull};
#[cfg(not(feature = "blocking-client"))]
fn validate_pull(_: &gix::Repository, _: &Manifest, _: &Pull) -> Result<()> {
    bail!("pull recovery requires blocking-client support")
}
#[cfg(not(feature = "blocking-client"))]
fn prepare_pull_metadata(_: &gix::Repository, _: &mut Manifest) -> Result<()> {
    bail!("pull recovery requires blocking-client support")
}
