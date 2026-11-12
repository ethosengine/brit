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

/// The maximum live temporary content, shared by every stage of one checkout.
pub const SCRATCH_LIMIT: u64 = 1024 * 1024 * 1024;
/// Transfer size, independent of object sizes.
pub const CHUNK: usize = 64 * 1024;

/// Scratch accounting for a single operation. Completed destination files are no longer scratch.
pub struct Budget {
    directory: tempfile::TempDir,
    live: AtomicU64,
    peak: AtomicU64,
}
impl Budget {
    /// Create private scratch on the destination filesystem.
    pub fn new(parent: &Path) -> io::Result<Arc<Self>> {
        Ok(Arc::new(Self {
            directory: tempfile::Builder::new().prefix(".brit-checkout-").tempdir_in(parent)?,
            live: AtomicU64::new(0),
            peak: AtomicU64::new(0),
        }))
    }
    /// Open an empty, automatically removed spool.
    pub fn spool(self: &Arc<Self>) -> io::Result<Spool> {
        let mut builder = tempfile::Builder::new();
        builder.prefix("blob-");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            builder.permissions(std::fs::Permissions::from_mode(0o666));
        }
        Ok(Spool {
            file: Some(builder.tempfile_in(self.directory.path())?),
            budget: self.clone(),
            len: 0,
            position: 0,
        })
    }
    /// Current and peak live bytes, including unpublished destination content.
    pub fn usage(&self) -> (u64, u64) {
        (self.live.load(Ordering::Relaxed), self.peak.load(Ordering::Relaxed))
    }
    fn reserve(&self, bytes: u64) -> io::Result<()> {
        let mut live = self.live.load(Ordering::Relaxed);
        loop {
            let next = live
                .checked_add(bytes)
                .filter(|n| *n <= SCRATCH_LIMIT)
                .ok_or_else(|| io::Error::other("Checkout exceeds the hard 1 GiB aggregate scratch budget"))?;
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
/// A seekable temporary file whose growth is charged before any write.
pub struct Spool {
    file: Option<tempfile::NamedTempFile>,
    budget: Arc<Budget>,
    len: u64,
    position: u64,
}
impl Spool {
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
        Err(io::Error::other(gix_error::cancelled("Checkout interrupted")))
    } else {
        Ok(())
    }
}
/// Copy with fixed storage and cancellation between chunks.
pub fn copy(input: &mut impl Read, output: &mut impl Write, interrupt: &AtomicBool) -> io::Result<u64> {
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

#[cfg(test)]
mod tests {
    use super::*;
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
        assert!(second.write_all(&[2]).is_err());
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
    fn cancelled_copy_does_not_consume_input() -> io::Result<()> {
        let mut input = &b"retained"[..];
        let mut output = Vec::new();
        assert!(copy(&mut input, &mut output, &AtomicBool::new(true)).is_err());
        assert_eq!(input, b"retained");
        assert!(output.is_empty());
        Ok(())
    }
}
