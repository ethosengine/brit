//! Operation-owned scratch with a hard aggregate quota and bounded transfers.
use std::{
    fs::File,
    io::{self, Read, Seek, SeekFrom, Write},
    path::Path,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
};

/// The maximum live temporary content, shared by every stage of one operation.
pub const SCRATCH_LIMIT: u64 = 1024 * 1024 * 1024;
/// Transfer size, independent of object sizes.
pub const CHUNK: usize = 64 * 1024;

mod command;
pub use command::{CommandOutput, command_output};

/// Scratch accounting for a single operation. Completed destination files are no longer scratch.
#[derive(Debug)]
pub struct Budget {
    directory: tempfile::TempDir,
    live: AtomicU64,
    peak: AtomicU64,
}
impl Budget {
    /// Create private scratch on the destination filesystem.
    pub fn new(parent: &Path) -> io::Result<Arc<Self>> {
        let mut builder = tempfile::Builder::new();
        builder.prefix(".brit-checkout-");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            builder.permissions(std::fs::Permissions::from_mode(0o700));
        }
        Ok(Arc::new(Self {
            directory: builder.tempdir_in(parent)?,
            live: AtomicU64::new(0),
            peak: AtomicU64::new(0),
        }))
    }
    /// Open an empty, automatically removed spool.
    pub fn spool(self: &Arc<Self>) -> io::Result<Spool> {
        self.spool_in(self.directory.path(), "blob-", None)
    }
    /// Open charged temporary content on a destination filesystem. Permissions are
    /// passed to file creation, so the process umask remains authoritative.
    pub fn spool_in(
        self: &Arc<Self>,
        parent: &Path,
        prefix: &str,
        permissions: Option<std::fs::Permissions>,
    ) -> io::Result<Spool> {
        let mut builder = tempfile::Builder::new();
        builder.prefix(prefix);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            builder.permissions(std::fs::Permissions::from_mode(0o666));
        }
        if let Some(permissions) = permissions {
            builder.permissions(permissions);
        }
        Ok(Spool {
            file: Some(builder.tempfile_in(parent)?),
            budget: self.clone(),
            len: 0,
            position: 0,
        })
    }
    /// Current and peak live bytes, including unpublished destination content.
    pub fn usage(&self) -> (u64, u64) {
        (self.live.load(Ordering::Relaxed), self.peak.load(Ordering::Relaxed))
    }
    /// Private directory identity for an owner's durable crash-recovery receipt.
    pub fn directory(&self) -> &Path {
        self.directory.path()
    }
    /// Charge already-persisted recovery content while it participates in an operation.
    /// The caller must keep this reservation alive until its next inventory or reclamation.
    pub fn charge(self: &Arc<Self>, bytes: u64) -> io::Result<Charge> {
        self.reserve(bytes)?;
        Ok(Charge {
            budget: self.clone(),
            bytes,
        })
    }
    fn reserve(&self, bytes: u64) -> io::Result<()> {
        let mut live = self.live.load(Ordering::Relaxed);
        loop {
            let next = live.checked_add(bytes).filter(|n| *n <= SCRATCH_LIMIT).ok_or_else(|| {
                io::Error::other(ScratchLimit {
                    requested: bytes,
                    live,
                    path: self.directory.path().to_owned(),
                })
            })?;
            match self
                .live
                .compare_exchange_weak(live, next, Ordering::Relaxed, Ordering::Relaxed)
            {
                Ok(_) => {
                    self.peak.fetch_max(next, Ordering::Relaxed);
                    return Ok(());
                }
                Err(current) => live = current,
            }
        }
    }
}
/// A reservation for durable temporary content owned outside a [`Spool`].
pub struct Charge {
    budget: Arc<Budget>,
    bytes: u64,
}
impl Drop for Charge {
    fn drop(&mut self) {
        self.budget.live.fetch_sub(self.bytes, Ordering::Relaxed);
    }
}
/// A scratch quota refusal, distinct from allocator failure and filesystem errors.
#[derive(Debug)]
pub struct ScratchLimit {
    /// Additional bytes requested by the operation.
    pub requested: u64,
    /// Bytes already charged, including sparse file holes.
    pub live: u64,
    /// Operation-owned directory whose quota was exhausted.
    pub path: std::path::PathBuf,
}
impl std::fmt::Display for ScratchLimit {
    fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            out,
            "Scratch write requested {} bytes with {} bytes live: hard 1 GiB aggregate scratch budget ({} bytes) at {} exceeded",
            self.requested,
            self.live,
            SCRATCH_LIMIT,
            self.path.display()
        )
    }
}
impl std::error::Error for ScratchLimit {}

/// Immutable bytes backed by an operation-owned file. Mapping is dropped before its file.
pub struct Mapped {
    map: Option<memmap2::Mmap>,
    _spool: Spool,
}
impl Mapped {
    /// Borrow the bytes without allocating another complete copy.
    pub fn as_slice(&self) -> &[u8] {
        self.map.as_deref().unwrap_or_default()
    }
}
impl AsRef<[u8]> for Mapped {
    fn as_ref(&self) -> &[u8] {
        self.as_slice()
    }
}
/// A seekable temporary file whose growth is charged before any write.
pub struct Spool {
    file: Option<tempfile::NamedTempFile>,
    budget: Arc<Budget>,
    len: u64,
    position: u64,
}

/// A named converter input. Exposing its path permanently relinquishes mapping access.
pub struct NamedInput(Spool);
impl NamedInput {
    /// The private path supplied to an owned converter for this input's lifetime.
    pub fn path(&self) -> &Path {
        self.0.file.as_ref().expect("spool owns its file").path()
    }
}
impl Spool {
    /// Transfer ownership to a named converter input which can never be mapped.
    /// Converter output must use a different, unexposed spool before mapping.
    pub fn into_named_input(self) -> NamedInput {
        NamedInput(self)
    }
    /// Freeze this spool, retaining its quota until the read-only mapping is dropped.
    #[allow(
        unsafe_code,
        reason = "Mapping an exclusively owned immutable spool requires memmap2's unsafe constructor"
    )]
    pub fn into_mapped(self) -> io::Result<Mapped> {
        let map = if self.is_empty() {
            None
        } else {
            // SAFETY: the file was created exclusively in our private directory. This
            // consumes its only writable handle, and Mapped exposes neither that handle
            // nor its path. No code can truncate or write it while the mapping is alive.
            Some(unsafe { memmap2::Mmap::map(self.file.as_ref().expect("spool owns its file").as_file())? })
        };
        Ok(Mapped { map, _spool: self })
    }
    /// Logical length, including holes.
    pub fn len(&self) -> u64 {
        self.len
    }
    /// Whether this file is empty.
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }
    /// Reclaim an obsolete suffix. Growing through truncation is forbidden.
    pub fn truncate(&mut self, len: u64) -> io::Result<()> {
        if len > self.len {
            return Err(io::Error::other("Scratch truncation cannot grow a file"));
        }
        self.file
            .as_ref()
            .expect("spool owns its file")
            .as_file()
            .set_len(len)?;
        self.budget.live.fetch_sub(self.len - len, Ordering::Relaxed);
        self.len = len;
        Ok(())
    }
    /// Set publication permissions without exposing the writable backing file.
    pub fn set_permissions(&mut self, permissions: std::fs::Permissions) -> io::Result<()> {
        self.file
            .as_ref()
            .expect("spool owns its file")
            .as_file()
            .set_permissions(permissions)
    }
    /// Publish without overwriting an existing path, releasing temporary quota.
    pub fn publish(mut self, path: &Path) -> io::Result<File> {
        let file = self.file.take().expect("spool owns its file");
        match file.persist_noclobber(path) {
            Ok(file) => Ok(file),
            Err(err) => {
                self.file = Some(err.file);
                Err(err.error)
            }
        }
    }
    /// Atomically replace a destination with this completed spool, releasing its quota.
    pub fn publish_replace(mut self, path: &Path) -> io::Result<File> {
        let file = self.file.take().expect("spool owns its file");
        match file.persist(path) {
            Ok(file) => Ok(file),
            Err(err) => {
                self.file = Some(err.file);
                Err(err.error)
            }
        }
    }
}
impl Drop for Spool {
    fn drop(&mut self) {
        // A failed unlink must not make still-live scratch available to later stages.
        if self.file.take().is_some_and(|file| file.close().is_err()) {
            return;
        }
        self.budget.live.fetch_sub(self.len, Ordering::Relaxed);
    }
}
impl Read for Spool {
    fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
        let n = self.file.as_mut().expect("spool owns its file").read(out)?;
        self.position += n as u64;
        Ok(n)
    }
}
impl Seek for Spool {
    fn seek(&mut self, position: SeekFrom) -> io::Result<u64> {
        self.position = self.file.as_mut().expect("spool owns its file").seek(position)?;
        Ok(self.position)
    }
}
impl Write for Spool {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if bytes.is_empty() {
            return Ok(0);
        }
        let end = self
            .position
            .checked_add(bytes.len() as u64)
            .ok_or_else(|| io::Error::other("scratch size overflow"))?;
        let growth = end.saturating_sub(self.len);
        self.budget.reserve(growth)?;
        let result = self.file.as_mut().expect("spool owns its file").write(bytes);
        let written = result.as_ref().copied().unwrap_or(0) as u64;
        self.position += written;
        let actual = if written == 0 {
            0
        } else {
            self.position.saturating_sub(self.len)
        };
        self.len += actual;
        self.budget.live.fetch_sub(growth - actual, Ordering::Relaxed);
        result
    }
    fn flush(&mut self) -> io::Result<()> {
        self.file.as_mut().expect("spool owns its file").flush()
    }
}
/// Poll cancellation without `Interrupted`, which standard I/O helpers retry indefinitely.
pub fn check(interrupt: &AtomicBool) -> io::Result<()> {
    if interrupt.load(Ordering::Relaxed) {
        Err(io::Error::other(gix_error::cancelled("Object processing interrupted")))
    } else {
        Ok(())
    }
}
/// Copy with fixed storage and cancellation between chunks.
pub fn copy(
    input: &mut (impl Read + ?Sized),
    output: &mut (impl Write + ?Sized),
    interrupt: &AtomicBool,
) -> io::Result<u64> {
    let mut buf = [0; CHUNK];
    let mut total = 0;
    loop {
        check(interrupt)?;
        let n = input.read(&mut buf)?;
        if n == 0 {
            return Ok(total);
        }
        output.write_all(&buf[..n])?;
        total += n as u64;
    }
}

/// Compare streams with bounded buffers, including readers that return short chunks.
pub fn equal(left: impl Read, right: impl Read, interrupt: &AtomicBool) -> io::Result<bool> {
    use io::BufRead;
    let mut left = io::BufReader::with_capacity(CHUNK, left);
    let mut right = io::BufReader::with_capacity(CHUNK, right);
    loop {
        check(interrupt)?;
        let a = left.fill_buf()?;
        let b = right.fill_buf()?;
        if a.is_empty() || b.is_empty() {
            return Ok(a.is_empty() && b.is_empty());
        }
        let len = a.len().min(b.len());
        if a[..len] != b[..len] {
            return Ok(false);
        }
        left.consume(len);
        right.consume(len);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn durable_content_shares_spool_quota() -> io::Result<()> {
        let dir = tempfile::tempdir()?;
        let budget = Budget::new(dir.path())?;
        let retained = budget.charge(SCRATCH_LIMIT)?;
        let mut spool = budget.spool()?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(budget.directory().metadata()?.permissions().mode() & 0o077, 0);
        }
        assert!(
            spool.write_all(b"x").is_err(),
            "durable recovery content counts alongside new scratch"
        );
        drop(retained);
        spool.write_all(b"x")?;
        assert_eq!(budget.usage().0, 1);
        Ok(())
    }

    #[test]
    fn streamed_equality_handles_short_reads_lengths_and_cancellation() -> io::Result<()> {
        struct Short<'a>(&'a [u8]);
        impl Read for Short<'_> {
            fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
                let len = bytes.len().min(3);
                self.0.read(&mut bytes[..len])
            }
        }
        let interrupt = AtomicBool::new(false);
        assert!(equal(Short(b"matching content"), &b"matching content"[..], &interrupt)?);
        assert!(!equal(
            Short(b"matching content"),
            &b"matching contents"[..],
            &interrupt
        )?);
        assert!(!equal(
            Short(b"matching content"),
            &b"matching conTent"[..],
            &interrupt
        )?);
        interrupt.store(true, Ordering::Relaxed);
        assert!(equal(&b""[..], &b""[..], &interrupt).is_err());
        Ok(())
    }
    #[test]
    fn mapping_keeps_quota_and_releases_it_after_readers() -> io::Result<()> {
        let dir = tempfile::tempdir()?;
        let budget = Budget::new(dir.path())?;
        for bytes in [&b""[..], &b"immutable bytes"[..]] {
            let mut spool = budget.spool()?;
            spool.write_all(bytes)?;
            let mapped = spool.into_mapped()?;
            assert_eq!(mapped.as_slice(), bytes);
            assert_eq!(budget.usage().0, bytes.len() as u64);
            drop(mapped);
            assert_eq!(budget.usage().0, 0);
        }
        Ok(())
    }
    #[test]
    fn hard_quota_counts_holes_and_is_reclaimed() -> io::Result<()> {
        let dir = tempfile::tempdir()?;
        let budget = Budget::new(dir.path())?;
        let mut first = budget.spool()?;
        first.seek(SeekFrom::Start(SCRATCH_LIMIT - 1))?;
        first.write_all(&[1])?;
        assert_eq!(budget.usage(), (SCRATCH_LIMIT, SCRATCH_LIMIT));
        let mut second = budget.spool()?;
        second.seek(SeekFrom::Start(SCRATCH_LIMIT + 1))?;
        assert_eq!(second.write(&[])?, 0);
        second.rewind()?;
        let error = second.write_all(&[2]).expect_err("aggregate scratch is full");
        let refusal = error
            .get_ref()
            .and_then(|error| error.downcast_ref::<ScratchLimit>())
            .expect("quota refusal is distinct from a filesystem error");
        assert_eq!(refusal.requested, 1);
        assert_eq!(refusal.live, SCRATCH_LIMIT);
        first.rewind()?;
        first.write_all(&[3])?; // overwriting is not growth
        assert_eq!(budget.usage().0, SCRATCH_LIMIT);
        drop(first);
        second.write_all(&[2])?;
        assert_eq!(budget.usage().0, 1);
        let file = second.publish(&dir.path().join("published"))?;
        assert_eq!(file.metadata()?.len(), 1);
        assert_eq!(budget.usage().0, 0);
        Ok(())
    }
    #[test]
    fn publication_never_replaces_foreign_content() -> io::Result<()> {
        let dir = tempfile::tempdir()?;
        let path = dir.path().join("foreign");
        std::fs::write(&path, b"retained")?;
        let budget = Budget::new(dir.path())?;
        let mut spool = budget.spool()?;
        spool.write_all(b"ours")?;
        assert!(spool.publish(&path).is_err());
        assert_eq!(std::fs::read(path)?, b"retained");
        assert_eq!(budget.usage().0, 0);
        Ok(())
    }
    #[test]
    fn destination_temporary_is_charged_until_replacement() -> io::Result<()> {
        let dir = tempfile::tempdir()?;
        let destination = tempfile::tempdir()?;
        let path = destination.path().join("file");
        std::fs::write(&path, b"before")?;
        let budget = Budget::new(dir.path())?;
        let mut source = budget.spool()?;
        source.write_all(b"after")?;
        source.rewind()?;
        let mut staged = budget.spool_in(destination.path(), ".brit-restore-", None)?;
        copy(&mut source, &mut staged, &AtomicBool::new(false))?;
        assert_eq!(budget.usage(), (10, 10));
        drop(source);
        staged.publish_replace(&path)?;
        assert_eq!(budget.usage(), (0, 10));
        assert_eq!(std::fs::read(path)?, b"after");
        Ok(())
    }
    #[test]
    fn cancelled_copy_does_not_consume_input() -> io::Result<()> {
        let mut input = &b"retained"[..];
        let mut output = Vec::new();
        assert!(copy(&mut input, &mut output, &AtomicBool::new(true)).is_err());
        assert_eq!(input, b"retained");
        assert!(output.is_empty());
        Ok(())
    }
}
