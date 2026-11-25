//! Consolidate local packs without making reachability decisions.
//!
//! A push pins the common object store through send-pack, including its pre-push hook.
//! Consolidation defers or refuses when it cannot take the pin exclusively.

#[cfg(unix)]
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::{
    cmp::Reverse,
    fs,
    io::{self, BufReader, Write},
    path::{Path, PathBuf},
    sync::atomic::Ordering,
};

use gix::{
    Result,
    error::{OptionExt, ResultExt, bail, message, validation},
    interrupt::IS_INTERRUPTED,
    odb::pack::{self, data::output},
    progress::Discard,
};

#[cfg(target_os = "linux")]
use super::switch::describe_object_pin_holders as describe_pin_holder;

const TARGET: usize = 8;
const FETCH_THRESHOLD: usize = 24;
const FETCH_BATCH: usize = 24;
const OBJECT_PIN: &str = "brit-objects.pin";

/// The invoking repository's local pack directory and maintenance settings.
pub struct Store {
    pack_dir: PathBuf,
    pin_path: PathBuf,
    object_hash: gix::hash::Kind,
    target: usize,
    threshold: usize,
}

#[derive(Clone, Copy)]
struct Input {
    id: gix::ObjectId,
    bytes: u64,
    access: [ReadAccess; 2],
}

/// Keep packs readable by the same users after consolidation. A single pack
/// cannot preserve different per-object access permissions.
#[derive(Clone, Copy, Eq, Ord, PartialEq, PartialOrd)]
struct ReadAccess {
    #[cfg(unix)]
    owner: u32,
    #[cfg(unix)]
    group: u32,
    #[cfg(unix)]
    mode: u32,
}

struct Plan {
    before: usize,
    inputs: Vec<Input>,
}

enum Consolidation {
    Complete(Plan, usize),
    Skipped,
    Pinned(String),
}

struct Transaction {
    replacement: gix::ObjectId,
    sources: Vec<gix::ObjectId>,
}

fn pack_path(dir: &Path, id: gix::ObjectId, ext: &str) -> PathBuf {
    dir.join(format!("pack-{id}.{ext}"))
}

fn open_pin(path: &Path) -> Result<fs::File> {
    let mut options = fs::OpenOptions::new();
    options.read(true).write(true).create(true).truncate(false);
    #[cfg(target_os = "linux")]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(rustix::fs::OFlags::NOFOLLOW.bits() as i32);
    }
    options
        .open(path)
        .or_raise(|| message!("could not open object pin {}", path.display()))
}

#[cfg(feature = "blocking-client")]
pub(crate) fn pin_for_push(repo: &gix::Repository) -> Result<fs::File> {
    let path = repo.common_dir().join(OBJECT_PIN);
    let pin = open_pin(&path)?;
    pin.lock_shared()
        .or_raise(|| message!("could not take shared object pin {}", path.display()))?;
    Ok(pin)
}

#[cfg(not(target_os = "linux"))]
fn describe_pin_holder(_: &fs::File) -> String {
    "a process holding the object pin (holder identity unavailable on this platform)".into()
}

impl Store {
    /// Read the repository's object format, pack location and Brit settings.
    pub fn from_repo(repo: &gix::Repository) -> Result<Self> {
        let config = repo.config_snapshot();
        let count = |key, default, min| -> Result<usize> {
            let Some(value) = config.try_integer(key)? else {
                return Ok(default);
            };
            let max = gix::open::REDUCED_TRUST_MAX_PACK_INDEXES - 1;
            usize::try_from(value)
                .ok()
                .filter(|v| (min..=max).contains(v))
                .ok_or_raise(|| validation(format!("{key} must be between {min} and {max}")))
        };
        Ok(Self {
            pack_dir: repo.objects.store_ref().path().join("pack"),
            pin_path: repo.common_dir().join(OBJECT_PIN),
            object_hash: repo.object_hash(),
            target: count("brit.maintenanceTarget", TARGET, 1)?,
            threshold: count("brit.maintenanceFetchThreshold", FETCH_THRESHOLD, 2)?,
        })
    }

    /// Plan a read-only run or consolidate local packs under the caller's repository guard.
    pub fn run(&self, dry_run: bool, out: impl Write) -> Result<()> {
        if dry_run {
            return self.plan(false)?.print("Plan", out);
        }
        match self.consolidate(false)? {
            Consolidation::Complete(plan, after) => plan.print_with_count("Maintenance", after, out),
            Consolidation::Skipped => Ok(()),
            Consolidation::Pinned(holder) => {
                bail!(message!(
                    "cannot consolidate packs because object pin {} is held by {holder}",
                    self.pin_path.display()
                ));
            }
        }
    }

    /// Consolidate at most one batch, under the same guard as the successful fetch.
    pub fn after_fetch(&self) -> Result<()> {
        self.after_fetch_with_notice().map(|_| ())
    }

    /// Like [`Self::after_fetch`], returning a notice when the object pin defers consolidation.
    pub fn after_fetch_with_notice(&self) -> Result<Option<String>> {
        Ok(match self.consolidate(true)? {
            Consolidation::Pinned(holder) => Some(format!(
                "pack consolidation deferred because a push is in progress ({} held by {holder})",
                self.pin_path.display()
            )),
            Consolidation::Complete(..) | Consolidation::Skipped => None,
        })
    }

    fn plan(&self, automatic: bool) -> Result<Plan> {
        let Some(entries) = missing_ok(fs::read_dir(&self.pack_dir)).or_error()? else {
            return Ok(Plan {
                before: 0,
                inputs: Vec::new(),
            });
        };
        let mut before = 0;
        let mut inputs = Vec::new();
        for entry in entries {
            let entry = entry.or_error()?;
            let index = entry.path();
            if index.extension().is_none_or(|ext| ext != "idx") || !entry.file_type().or_error()?.is_file() {
                continue;
            }
            let Some(data) = missing_ok(fs::symlink_metadata(index.with_extension("pack"))).or_error()? else {
                continue;
            };
            if !data.is_file() {
                continue;
            }
            before += 1;
            let Some(id) = index
                .file_stem()
                .and_then(|name| name.to_str())
                .and_then(|name| name.strip_prefix("pack-"))
                .filter(|name| name.len() == self.object_hash.len_in_hex())
                .and_then(|name| gix::ObjectId::from_hex(name.as_bytes()).ok())
                .filter(|id| pack_path(&self.pack_dir, *id, "idx") == index)
            else {
                continue;
            };
            if !protected(&index)? {
                let index = entry.metadata().or_error()?;
                inputs.push(Input {
                    id,
                    bytes: data.len() + index.len(),
                    access: [ReadAccess::of(&data), ReadAccess::of(&index)],
                });
            }
        }
        let limit = if automatic { self.threshold } else { self.target };
        if before <= limit {
            inputs.clear();
        } else {
            let target = if automatic {
                self.target.min(self.threshold - 1)
            } else {
                self.target
            };
            let batch = if automatic { FETCH_BATCH } else { usize::MAX };
            let wanted = (before - target + 1).min(batch);
            inputs.sort_unstable_by_key(|input| (input.access, input.bytes, input.id));
            inputs = inputs
                .chunk_by(|a, b| a.access == b.access)
                .filter(|group| group.len() >= 2)
                .map(|group| &group[..wanted.min(group.len())])
                .max_by_key(|group| (group.len(), Reverse(group.iter().map(|input| input.bytes).sum::<u64>())))
                .unwrap_or_default()
                .to_vec();
        }
        Ok(Plan { before, inputs })
    }

    fn consolidate(&self, automatic: bool) -> Result<Consolidation> {
        let stage = self.pack_dir.join(".brit-maintenance");
        // Avoid creating a lock file on the normal below-threshold fetch path.
        if automatic
            && missing_ok(fs::symlink_metadata(&stage)).or_error()?.is_none()
            && self.plan(true)?.inputs.is_empty()
        {
            return Ok(Consolidation::Skipped);
        }
        if !self.pack_dir.is_dir() {
            let plan = self.plan(automatic)?;
            return Ok(Consolidation::Complete(plan, 0));
        }
        // The caller holds Brit's repository guard. An OS lock also serializes
        // maintenance on systems where the shared mutation guard is a stub.
        let lock = fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(false)
            .open(self.pack_dir.join(".brit-maintenance.lock"))
            .or_error()?;
        lock.try_lock()
            .or_raise(|| message("another pack maintenance operation may be running"))?;
        let pin = open_pin(&self.pin_path)?;
        match pin.try_lock() {
            Ok(()) => {}
            Err(fs::TryLockError::WouldBlock) => return Ok(Consolidation::Pinned(describe_pin_holder(&pin))),
            Err(fs::TryLockError::Error(err)) => {
                return Err(err)
                    .or_raise(|| message!("could not take exclusive object pin {}", self.pin_path.display()));
            }
        }
        let resumed = self.resume(&stage, automatic)?;
        let plan = self.plan(automatic)?;
        if plan.inputs.is_empty() || (automatic && resumed) {
            let after = plan.before;
            return Ok(Consolidation::Complete(plan, after));
        }
        if plan.after() > gix::open::REDUCED_TRUST_MAX_PACK_INDEXES
            && missing_ok(fs::symlink_metadata(self.pack_dir.join("multi-pack-index")))
                .or_error()?
                .is_some()
        {
            bail!(message(
                "consolidation would leave more standalone indexes than reduced trust allows; use a lower target or run `brit maintenance`"
            ));
        }

        fs::create_dir(&stage).or_error()?;
        let replacement = self.write_bundle(&stage, &plan.inputs)?;
        checkpoint("maintenance-pack-prepared")?;
        let receipt = Transaction {
            replacement,
            sources: plan.inputs.iter().map(|input| input.id).collect(),
        };
        receipt.record(&stage, &self.pack_dir)?;
        checkpoint("maintenance-staged")?;
        self.publish(&stage, &receipt)?;
        fs::remove_dir_all(&stage).or_error()?;
        sync_dir(&self.pack_dir)?;
        Ok(Consolidation::Complete(plan, self.plan(automatic)?.before))
    }

    fn resume(&self, stage: &Path, automatic: bool) -> Result<bool> {
        let Some(metadata) = missing_ok(fs::symlink_metadata(stage)).or_error()? else {
            return Ok(false);
        };
        if !metadata.is_dir() {
            bail!(validation(format!(
                "expected a maintenance directory at {}",
                stage.display()
            )));
        }
        let receipt = Transaction::read(stage, self.object_hash)?;
        if let Some(receipt) = &receipt {
            if automatic && receipt.sources.len() > FETCH_BATCH {
                bail!(message(
                    "run `brit maintenance` to resume the interrupted manual consolidation"
                ));
            }
            self.publish(stage, receipt)?;
        }
        fs::remove_dir_all(stage).or_error()?;
        sync_dir(&self.pack_dir)?;
        Ok(receipt.is_some())
    }

    fn write_bundle(&self, stage: &Path, inputs: &[Input]) -> Result<gix::ObjectId> {
        // Isolate the source set so the pack writer only looks in the selected
        // packs, even when objects are duplicated elsewhere in the live store.
        let source = stage.join("source");
        let source_packs = source.join("pack");
        fs::create_dir_all(&source_packs).or_error()?;
        // The source set is the union of the selected indexes, including objects
        // that only a reflog, another worktree or an unknown borrower can see.
        let mut counts = Vec::new();
        for input in inputs {
            let Some(index) = self.source_index(input.id)? else {
                bail!(gix::error::message!("source pack {} disappeared", input.id));
            };
            let data = pack::data::File::at(index.path().with_extension("pack"), self.object_hash)?;
            if data.checksum() != input.id || data.num_objects() != index.num_objects() {
                bail!(validation(format!("source pack and index disagree for {}", input.id)));
            }
            for ext in ["pack", "idx"] {
                let from = pack_path(&self.pack_dir, input.id, ext);
                let to = pack_path(&source_packs, input.id, ext);
                stage_input(&from, &to)?;
            }
            counts.extend(index.iter().map(|entry| output::Count {
                id: entry.oid,
                entry_pack_location: output::count::PackLocation::NotLookedUp,
            }));
        }
        counts.sort_unstable_by_key(|count| count.id);
        counts.dedup_by_key(|count| count.id);
        let entries = u32::try_from(counts.len()).or_raise(|| message("too many objects for one pack"))?;
        let objects = gix::odb::at(&source, self.object_hash).or_error()?;
        let mut objects = objects.into_arc().or_error()?;
        objects.prevent_pack_unload();
        let generated = tempfile::NamedTempFile::new_in(stage).or_error()?;
        let mut writer = io::BufWriter::new(generated.as_file());
        let version = pack::data::Version::default();
        let chunks = output::entry::iter_from_counts(
            counts,
            objects,
            Box::new(Discard),
            output::entry::iter_from_counts::Options {
                version,
                mode: output::entry::iter_from_counts::Mode::PackCopyAndBaseObjects,
                allow_thin_pack: false,
                thread_limit: Some(1),
                chunk_size: 64,
                compression: gix::zlib::Compression::BEST_SPEED,
            },
        )?;
        let mut chunks = gix::parallel::InOrderIter::from(chunks);
        for written in output::bytes::FromEntriesIter::new(&mut chunks, &mut writer, entries, version, self.object_hash)
        {
            written?;
            if IS_INTERRUPTED.load(Ordering::Relaxed) {
                bail!(gix::error::cancelled("pack maintenance interrupted"));
            }
        }
        drop(chunks);
        drop(writer);
        fs::remove_dir_all(source).or_error()?;

        let result = pack::Bundle::write_to_directory(
            &mut BufReader::new(generated.reopen().or_error()?),
            Some(stage),
            &mut Discard,
            &IS_INTERRUPTED,
            None::<gix::objs::find::Never>,
            self.object_hash,
            pack::bundle::write::Options {
                thread_limit: Some(1),
                ..Default::default()
            },
        )?;
        if result.index.num_objects != entries {
            bail!(validation(
                "replacement pack did not contain the expected number of objects"
            ));
        }
        for (ext, access) in ["pack", "idx"].into_iter().zip(inputs[0].access) {
            access.sync_replacement(&pack_path(stage, result.index.data_hash, ext))?;
        }
        Ok(result.index.data_hash)
    }

    fn source_index(&self, id: gix::ObjectId) -> Result<Option<pack::index::File>> {
        let path = pack_path(&self.pack_dir, id, "idx");
        let Some(metadata) = missing_ok(fs::symlink_metadata(&path)).or_error()? else {
            return Ok(None);
        };
        if !metadata.is_file() {
            bail!(validation(format!(
                "expected a regular pack index at {}",
                path.display()
            )));
        }
        let index = pack::index::File::at(path, self.object_hash)?;
        if index.pack_checksum() != id {
            bail!(validation(format!(
                "index checksum trailer names a different pack than {id}"
            )));
        }
        index.verify_checksum(&mut Discard, &IS_INTERRUPTED)?;
        Ok(Some(index))
    }

    fn publish(&self, stage: &Path, receipt: &Transaction) -> Result<()> {
        let id = receipt.replacement;
        for ext in ["pack", "idx"] {
            let staged = pack_path(stage, id, ext);
            let live = pack_path(&self.pack_dir, id, ext);
            if let (Some(new), Some(old)) = (
                missing_ok(fs::symlink_metadata(&staged)).or_error()?,
                missing_ok(fs::symlink_metadata(&live)).or_error()?,
            ) && (!old.is_file() || ReadAccess::of(&new) != ReadAccess::of(&old))
            {
                bail!(validation(format!(
                    "existing replacement {} has different file access",
                    live.display()
                )));
            }
        }
        for (ext, point) in [
            ("pack", "maintenance-pack-published"),
            ("idx", "maintenance-index-published"),
        ] {
            let staged = pack_path(stage, id, ext);
            if staged.is_file() {
                let destination = pack_path(&self.pack_dir, id, ext);
                if let Err(err) = fs::rename(&staged, &destination) {
                    clear_readonly(&destination, err).or_error()?;
                    fs::rename(staged, destination).or_error()?;
                }
                sync_dir(&self.pack_dir)?;
                sync_dir(stage)?;
            }
            checkpoint(point)?;
        }

        let replacement = self
            .source_index(id)?
            .ok_or_raise(|| message("replacement pack index is missing"))?;
        let data = pack::data::File::at(replacement.path().with_extension("pack"), self.object_hash)?;
        if data.num_objects() != replacement.num_objects() || data.verify_checksum(&mut Discard, &IS_INTERRUPTED)? != id
        {
            bail!(validation("replacement pack and index disagree"));
        }
        let access = [
            ReadAccess::of(&data.path().metadata().or_error()?),
            ReadAccess::of(&replacement.path().metadata().or_error()?),
        ];
        // A repository can use a Git-written MIDX. Drop the optional cache
        // before retiring packs so neither Git nor gix follows stale offsets.
        remove_if_present(&self.pack_dir.join("multi-pack-index"))?;
        let midx_layers = self.pack_dir.join("multi-pack-index.d");
        if missing_ok(fs::symlink_metadata(&midx_layers))
            .or_error()?
            .is_some_and(|meta| meta.is_dir())
        {
            remove_if_present(&midx_layers.join("multi-pack-index-chain"))?;
            sync_dir(&midx_layers)?;
        }
        sync_dir(&self.pack_dir)?;
        checkpoint("maintenance-midx-invalidated")?;
        for &source in receipt.sources.iter().filter(|&&source| source != id) {
            let Some(index) = self.source_index(source)? else {
                continue;
            };
            // A missing pack with its index still present is a completed
            // data removal interrupted before index cleanup.
            if protected(index.path())? {
                continue;
            }
            let source_path = pack_path(&self.pack_dir, source, "pack");
            let source_access = missing_ok(source_path.metadata())
                .or_error()?
                .map(|meta| ReadAccess::of(&meta));
            if ReadAccess::of(&index.path().metadata().or_error()?) != access[1]
                || source_access.is_some_and(|source_access| source_access != access[0])
            {
                continue;
            }
            if let Some(entry) = index.iter().find(|entry| replacement.lookup(entry.oid).is_none()) {
                bail!(validation(format!(
                    "replacement pack is missing {} from {source}",
                    entry.oid
                )));
            }
            drop(index);
            remove_if_present(&source_path)?;
            checkpoint("maintenance-retiring")?;
            for ext in ["rev", "bitmap", "idx"] {
                remove_if_present(&pack_path(&self.pack_dir, source, ext))?;
            }
        }
        sync_dir(&self.pack_dir)?;
        Ok(())
    }
}

impl Plan {
    fn after(&self) -> usize {
        self.before - self.inputs.len().saturating_sub(1)
    }

    fn print(&self, heading: &str, out: impl Write) -> Result<()> {
        self.print_with_count(heading, self.after(), out)
    }

    fn print_with_count(&self, heading: &str, after: usize, mut out: impl Write) -> Result<()> {
        let bytes: u64 = self.inputs.iter().map(|input| input.bytes).sum();
        writeln!(
            out,
            "{heading}: {} packs now, {after} after; {bytes} bytes of input packs and indexes",
            self.before
        )
        .or_error()
    }
}

impl Transaction {
    fn record(&self, stage: &Path, pack_dir: &Path) -> Result<()> {
        let mut file = tempfile::NamedTempFile::new_in(stage).or_error()?;
        writeln!(file, "{}", self.replacement).or_error()?;
        for source in &self.sources {
            writeln!(file, "{source}").or_error()?;
        }
        file.as_file().sync_all().or_error()?;
        file.persist(stage.join("transaction")).or_error()?;
        sync_dir(stage)?;
        sync_dir(pack_dir)
    }

    fn read(stage: &Path, hash: gix::hash::Kind) -> Result<Option<Self>> {
        let Some(text) = missing_ok(fs::read_to_string(stage.join("transaction"))).or_error()? else {
            return Ok(None);
        };
        let ids = text.lines().map(|line| {
            let id = gix::ObjectId::from_hex(line.as_bytes())?;
            if id.kind() != hash {
                bail!(validation("maintenance transaction uses the wrong object hash"));
            }
            Ok(id)
        });
        let mut ids = ids;
        let replacement = ids
            .next()
            .transpose()?
            .ok_or_raise(|| validation("empty maintenance transaction"))?;
        Ok(Some(Self {
            replacement,
            sources: ids.collect::<Result<_>>()?,
        }))
    }
}

fn protected(index: &Path) -> Result<bool> {
    for ext in ["keep", "promisor", "mtimes"] {
        if missing_ok(fs::symlink_metadata(index.with_extension(ext)))
            .or_error()?
            .is_some()
        {
            return Ok(true);
        }
    }
    Ok(false)
}

impl ReadAccess {
    fn of(metadata: &fs::Metadata) -> Self {
        #[cfg(not(unix))]
        let _ = metadata;
        Self {
            #[cfg(unix)]
            owner: metadata.uid(),
            #[cfg(unix)]
            group: metadata.gid(),
            #[cfg(unix)]
            mode: metadata.mode() & 0o444,
        }
    }

    fn sync_replacement(self, path: &Path) -> Result<()> {
        let file = fs::OpenOptions::new().write(true).open(path).or_error()?;
        #[cfg(unix)]
        {
            let access = Self::of(&file.metadata().or_error()?);
            let owner = (access.owner != self.owner).then_some(self.owner);
            let group = (access.group != self.group).then_some(self.group);
            if owner.is_some() || group.is_some() {
                std::os::unix::fs::chown(path, owner, group).or_error()?;
            }
            file.set_permissions(fs::Permissions::from_mode(self.mode)).or_error()?;
        }
        file.sync_all().or_error()
    }
}

fn stage_input(from: &Path, to: &Path) -> Result<()> {
    // On Windows, a hard link would share the source's read-only attribute
    // and could prevent the interrupted transaction's scratch from being removed.
    #[cfg(not(windows))]
    if fs::hard_link(from, to).is_ok() {
        return Ok(());
    }
    io::copy(
        &mut fs::File::open(from).or_error()?,
        &mut fs::File::create(to).or_error()?,
    )
    .or_error()?;
    Ok(())
}

fn sync_dir(path: &Path) -> Result<()> {
    #[cfg(unix)]
    fs::File::open(path).or_error()?.sync_all().or_error()?;
    #[cfg(not(unix))]
    let _ = path; // File sync and atomic renames still provide process-interruption safety.
    Ok(())
}

fn missing_ok<T>(result: io::Result<T>) -> io::Result<Option<T>> {
    match result {
        Ok(value) => Ok(Some(value)),
        Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(err) => Err(err),
    }
}

fn remove_if_present(path: &Path) -> Result<()> {
    if let Err(err) = missing_ok(fs::remove_file(path)) {
        clear_readonly(path, err).or_error()?;
        missing_ok(fs::remove_file(path)).or_error()?;
    }
    Ok(())
}

fn clear_readonly(path: &Path, error: io::Error) -> io::Result<()> {
    // Symlinks are never followed when changing attributes.
    if cfg!(windows)
        && error.kind() == io::ErrorKind::PermissionDenied
        && fs::symlink_metadata(path).is_ok_and(|meta| meta.is_file() && meta.permissions().readonly())
    {
        return gix::fs::clear_readonly_attribute(path);
    }
    Err(error)
}

fn checkpoint(point: &str) -> Result<()> {
    #[cfg(feature = "switch-test-faults")]
    if std::env::var("BRIT_SWITCH_CHECKPOINT").ok().as_deref() == Some(point) {
        let dir = PathBuf::from(
            std::env::var_os("BRIT_SWITCH_CHECKPOINT_DIR").ok_or_raise(|| message("checkpoint directory required"))?,
        );
        fs::write(dir.join(format!("{point}.reached")), b"reached").or_error()?;
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
        while !dir.join(format!("{point}.release")).exists() {
            if std::time::Instant::now() >= deadline {
                bail!(message("maintenance checkpoint release timed out"));
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
    }
    let _ = point;
    Ok(())
}
