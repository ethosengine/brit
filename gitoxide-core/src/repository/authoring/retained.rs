//! Immutable authored-object receipts: standard loose headers and streamed bodies.
//! Existing JSON receipts remain readable; interrupted `.new` files keep their
//! existing opaque-retention and cleanup-refusal semantics.
use super::{Envelope, RetainedObject, digest};
use crate::repository::switch;
use anyhow::{Context, Result, ensure};
use gix::{
    objs::FindStream,
    utils::stream::{self, Budget, Spool},
};
use std::{
    fs,
    io::{BufReader, Read, Seek, SeekFrom, Write},
    os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt},
    path::Path,
    sync::Arc,
};

pub(super) struct Object {
    pub id: gix::ObjectId,
    pub kind: gix::objs::Kind,
    pub data: Spool,
}
impl Object {
    fn verify(&mut self) -> Result<()> {
        self.data.rewind()?;
        let size = self.data.len();
        ensure!(
            gix::objs::compute_stream_hash(
                self.id.kind(),
                self.kind,
                &mut self.data,
                size,
                &mut gix::progress::Discard,
                &gix::interrupt::IS_INTERRUPTED
            )? == self.id,
            "corrupt authored object receipt {}",
            self.id
        );
        self.data.rewind()?;
        Ok(())
    }

    /// Only structural metadata needs contiguous access. Preserve the repository's
    /// effective allocation policy, including its ownership-derived trust default.
    pub fn metadata(&mut self, repo: &gix::Repository) -> Result<Vec<u8>> {
        ensure!(self.kind != gix::objs::Kind::Blob, "blob receipts must remain streamed");
        let size = self.data.len();
        if let Some(limit) = repo.allocation_limit().filter(|limit| size > *limit as u64) {
            return Err(gix::error::allocation_limit(format!(
                "Decode retained {} {}: requested {size} bytes, configured allocation limit is {limit} bytes",
                self.kind, self.id
            ))
            .into());
        }
        let size = usize::try_from(size)?;
        let mut bytes = Vec::new();
        bytes.try_reserve_exact(size).map_err(|error| {
            gix::error::allocation_failure(format!(
                "Decode retained {} {}: allocation of {size} bytes failed: {error}",
                self.kind, self.id
            ))
        })?;
        bytes.resize(size, 0);
        self.data.rewind()?;
        self.data.read_exact(&mut bytes)?;
        self.data.rewind()?;
        Ok(bytes)
    }

    pub fn restore(&mut self, repo: &gix::Repository) -> Result<()> {
        use gix::objs::Write;
        let restored = if self.kind == gix::objs::Kind::Blob {
            self.data.rewind()?;
            repo.objects.write_stream(self.kind, self.data.len(), &mut self.data)?
        } else {
            let bytes = self.metadata(repo)?;
            repo.write_object(&gix::objs::ObjectRef::from_bytes(&bytes, self.kind, self.id.kind())?)?
                .detach()
        };
        ensure!(restored == self.id, "restored authored object changed identity");
        Ok(())
    }
}

pub(super) fn open(repo: &gix::Repository, path: &Path, budget: &Arc<Budget>) -> Result<Object> {
    let name = path.file_name().context("authored object filename")?;
    let id = gix::ObjectId::from_hex(name.as_encoded_bytes())?;
    ensure!(id.kind() == repo.object_hash(), "authored object uses a foreign hash");
    let directory = path.parent().context("authored object directory")?;
    let _existing = switch::payload::charge(budget, directory)?;
    let metadata = fs::symlink_metadata(path)?;
    ensure!(
        metadata.is_file()
            && metadata.uid() == fs::metadata(directory)?.uid()
            && metadata.permissions().mode().trailing_zeros() >= 6,
        "unsafe authored object receipt"
    );
    let mut file = fs::OpenOptions::new()
        .read(true)
        .custom_flags(rustix::fs::OFlags::NOFOLLOW.bits() as i32)
        .open(path)?;
    let opened = file.metadata()?;
    ensure!(
        (opened.dev(), opened.ino(), opened.len()) == (metadata.dev(), metadata.ino(), metadata.len()),
        "authored object receipt changed while opening"
    );
    let mut prefix = Vec::new();
    Read::by_ref(&mut file).take(32).read_to_end(&mut prefix)?;
    let mut data = budget.spool()?;
    let raw_header = [b"blob ".as_slice(), b"tree ", b"commit ", b"tag "]
        .iter()
        .any(|kind| prefix.starts_with(kind));
    let kind = if !raw_header {
        file.rewind()?;
        let envelope: Envelope = serde_json::from_reader(BufReader::new(gix::interrupt::Read::new(&mut file)))?;
        ensure!(
            digest(&envelope.payload)? == envelope.checksum,
            "corrupt journal checksum"
        );
        let legacy: RetainedObject =
            serde_json::from_reader(BufReader::new(gix::interrupt::Read::new(envelope.payload.as_slice())))?;
        drop(envelope);
        ensure!(
            legacy.id.as_bytes() == name.as_encoded_bytes(),
            "foreign authored object receipt"
        );
        let kind = gix::objs::Kind::from_bytes(legacy.kind.as_bytes())?;
        stream::copy(&mut legacy.data.as_slice(), &mut data, &gix::interrupt::IS_INTERRUPTED)?;
        kind
    } else {
        let (kind, size, header_len) = gix::objs::decode::loose_header(&prefix)?;
        ensure!(
            size.checked_add(header_len as u64) == Some(metadata.len()),
            "authored object receipt length mismatch at {}",
            path.display()
        );
        ensure!(
            &prefix[..header_len] == gix::objs::encode::loose_header(kind, size).as_slice(),
            "noncanonical authored object header"
        );
        file.seek(SeekFrom::Start(header_len as u64))?;
        ensure!(
            stream::copy(&mut file, &mut data, &gix::interrupt::IS_INTERRUPTED)? == size,
            "authored object receipt changed while reading"
        );
        kind
    };
    let mut object = Object { id, kind, data };
    object.verify()?;
    Ok(object)
}

pub(super) fn persist(repo: &gix::Repository, id: gix::ObjectId, path: &Path, budget: &Arc<Budget>) -> Result<Object> {
    let directory = path.parent().context("authored object directory")?;
    let _existing = switch::payload::charge(budget, directory)?;
    let (kind, data) = repo.objects.find_stream(&id, budget, &gix::interrupt::IS_INTERRUPTED)?;
    let mut object = Object { id, kind, data };
    object.verify()?;
    let header = gix::objs::encode::loose_header(kind, object.data.len());
    let _output = budget.charge(object.data.len() + header.len() as u64)?;
    let temp = path.with_extension("new");
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&temp)?;
    switch::checkpoint("replay-child-object-temp-created")?;
    file.write_all(&header)?;
    ensure!(
        stream::copy(&mut object.data, &mut file, &gix::interrupt::IS_INTERRUPTED)? == object.data.len(),
        "authored object source changed during receipt publication"
    );
    switch::checkpoint("replay-child-object-temp-written")?;
    switch::sync_file(&file, &temp)?;
    switch::checkpoint("replay-child-object-temp-synced")?;
    fs::rename(temp, path)?;
    switch::checkpoint("replay-child-object-renamed")?;
    switch::sync_dir(directory)?;
    object.data.rewind()?;
    Ok(object)
}
