use gix_error::Result;
use gix_error::ResultExt;

/// Object access with caller-owned buffered or immutable external storage.
///
/// Existing [`crate::Find`] implementations use the ordinary reusable byte buffer.
pub trait Source {
    /// Find an object, retaining its bytes in `buffer`, or return `None` for a missing object.
    fn try_find<'a>(&self, id: &gix_hash::oid, buffer: &'a mut Buffer) -> Result<Option<crate::Data<'a>>>;

    /// Find a blob with the same empty-object, decoding and kind semantics as [`crate::FindExt`].
    fn find_blob<'a>(&self, id: &gix_hash::oid, buffer: &'a mut Buffer) -> Result<crate::BlobRef<'a>> {
        if id == gix_hash::ObjectId::empty_blob(id.kind()) {
            return Ok(crate::BlobRef { data: &[] });
        }
        self.try_find(id, buffer)?
            .ok_or_else(|| not_found(id))
            .and_then(|data| {
                data.decode()
                    .or_raise(|| gix_error::message!("Could not decode object at {id}").corrupted())
            })
            .and_then(|object| match object {
                crate::ObjectRef::Blob(blob) => Ok(blob),
                object => Err(wrong_kind(id, object.kind(), crate::Kind::Blob)),
            })
    }

    /// Find a commit with the same missing-object and kind diagnostics as [`crate::FindExt`].
    fn find_commit<'a>(&self, id: &gix_hash::oid, buffer: &'a mut Buffer) -> Result<crate::CommitRefIter<'a>> {
        self.try_find(id, buffer)?
            .ok_or_else(|| not_found(id))
            .and_then(|data| {
                data.try_into_commit_iter()
                    .ok_or_else(|| wrong_kind(id, data.kind, crate::Kind::Commit))
            })
    }
}

impl<T: crate::Find> Source for T {
    fn try_find<'a>(&self, id: &gix_hash::oid, buffer: &'a mut Buffer) -> Result<Option<crate::Data<'a>>> {
        crate::Find::try_find(self, id, buffer.owned())
    }
}

impl Source for &dyn Source {
    fn try_find<'a>(&self, id: &gix_hash::oid, buffer: &'a mut Buffer) -> Result<Option<crate::Data<'a>>> {
        (**self).try_find(id, buffer)
    }
}

pub(crate) fn not_found(id: &gix_hash::oid) -> gix_error::Error {
    gix_error::message!("An object with id {id} could not be found").not_found_error()
}

pub(crate) fn wrong_kind(id: &gix_hash::oid, actual: crate::Kind, expected: crate::Kind) -> gix_error::Error {
    gix_error::message!("Expected object of kind {expected} but got {actual} at {id}").validation_error()
}

/// Reusable object storage, optionally backed by immutable operation-owned bytes.
#[derive(Default, Clone)]
pub struct Buffer {
    owned: Vec<u8>,
    external: Option<std::sync::Arc<dyn AsRef<[u8]> + Send + Sync>>,
}

impl Buffer {
    /// Release the previous object before acquiring its replacement.
    pub fn clear(&mut self) {
        self.owned = Vec::new();
        self.external = None;
    }

    /// Reuse the ordinary object buffer, releasing any previous external storage.
    pub fn owned(&mut self) -> &mut Vec<u8> {
        self.external = None;
        &mut self.owned
    }

    /// Retain immutable bytes until the next lookup or until this buffer is dropped.
    pub fn set_external(&mut self, data: impl AsRef<[u8]> + Send + Sync + 'static) -> &[u8] {
        self.clear();
        self.external = Some(std::sync::Arc::new(data));
        self.external.as_deref().expect("just set").as_ref()
    }

    /// Release external storage and return the ordinary buffer for reuse.
    pub fn into_owned(self) -> Vec<u8> {
        self.owned
    }
}

impl From<Vec<u8>> for Buffer {
    fn from(owned: Vec<u8>) -> Self {
        Self { owned, external: None }
    }
}

impl AsRef<[u8]> for Buffer {
    fn as_ref(&self) -> &[u8] {
        self.external.as_deref().map_or(&self.owned, AsRef::as_ref)
    }
}

/// An implementation of object access traits that stores nothing and finds nothing.
/// Use [`Never::panic_on_access()`] to panic on object access instead.
#[derive(Debug, Copy, Clone)]
pub struct Never;

impl Never {
    /// Return an implementation that panics whenever an object access trait method is called.
    /// Useful for asserting that an operation does not access the object database.
    pub const fn panic_on_access() -> PanicAlways {
        PanicAlways
    }
}

/// An implementation of object access traits that panics on every call.
/// Obtain it with [`Never::panic_on_access()`].
#[derive(Debug, Copy, Clone)]
pub struct PanicAlways;

impl super::FindHeader for PanicAlways {
    fn try_header(&self, _id: &gix_hash::oid) -> Result<Option<crate::Header>> {
        panic!("object header lookups are forbidden");
    }
}

impl super::Find for PanicAlways {
    fn try_find<'a>(&self, _id: &gix_hash::oid, _buffer: &'a mut Vec<u8>) -> Result<Option<crate::Data<'a>>> {
        panic!("object lookups are forbidden");
    }
}

impl super::Exists for PanicAlways {
    fn exists(&self, _id: &gix_hash::oid) -> bool {
        panic!("object existence checks are forbidden");
    }
}

#[cfg(feature = "stream")]
impl super::FindStream for PanicAlways {
    fn find_stream(
        &self,
        _id: &gix_hash::oid,
        _budget: &std::sync::Arc<gix_utils::stream::Budget>,
        _interrupt: &std::sync::atomic::AtomicBool,
    ) -> Result<(crate::Kind, gix_utils::stream::Spool)> {
        panic!("object streams are forbidden");
    }
}

impl super::Write for PanicAlways {
    fn write(&self, _object: &dyn crate::WriteTo) -> Result<gix_hash::ObjectId> {
        panic!("object writes are forbidden");
    }

    fn write_buf_with_known_id(
        &self,
        _object: crate::Kind,
        _from: &[u8],
        _id: gix_hash::ObjectId,
    ) -> Result<gix_hash::ObjectId> {
        panic!("object writes are forbidden");
    }

    fn write_stream(
        &self,
        _kind: crate::Kind,
        _size: u64,
        _from: &mut dyn std::io::Read,
    ) -> Result<gix_hash::ObjectId> {
        panic!("object writes are forbidden");
    }

    fn write_stream_with_known_id(
        &self,
        _kind: crate::Kind,
        _size: u64,
        _from: &mut dyn std::io::Read,
        _id: gix_hash::ObjectId,
    ) -> Result<gix_hash::ObjectId> {
        panic!("object writes are forbidden");
    }
}

impl super::FindHeader for Never {
    fn try_header(&self, _id: &gix_hash::oid) -> Result<Option<crate::Header>> {
        Ok(None)
    }
}

impl super::Find for Never {
    fn try_find<'a>(&self, _id: &gix_hash::oid, _buffer: &'a mut Vec<u8>) -> Result<Option<crate::Data<'a>>> {
        Ok(None)
    }
}

#[cfg(feature = "stream")]
impl super::FindStream for Never {
    fn find_stream(
        &self,
        id: &gix_hash::oid,
        _budget: &std::sync::Arc<gix_utils::stream::Budget>,
        _interrupt: &std::sync::atomic::AtomicBool,
    ) -> Result<(crate::Kind, gix_utils::stream::Spool)> {
        use gix_error::ErrorExt;
        Err(gix_error::message!("Object {id} was not found").not_found().raise())
    }
}

impl super::Exists for Never {
    fn exists(&self, _id: &gix_hash::oid) -> bool {
        false
    }
}

impl super::Write for Never {
    fn write_buf(&self, object: crate::Kind, from: &[u8]) -> Result<gix_hash::ObjectId> {
        crate::compute_hash(gix_hash::Kind::default(), object, from)
    }

    fn write_buf_with_known_id(
        &self,
        _object: crate::Kind,
        _from: &[u8],
        id: gix_hash::ObjectId,
    ) -> Result<gix_hash::ObjectId> {
        Ok(id)
    }

    fn write_stream(&self, kind: crate::Kind, size: u64, from: &mut dyn std::io::Read) -> Result<gix_hash::ObjectId> {
        crate::compute_stream_hash(
            gix_hash::Kind::default(),
            kind,
            from,
            size,
            &mut gix_utils::progress::Discard,
            &std::sync::atomic::AtomicBool::new(false),
        )
    }

    fn write_stream_with_known_id(
        &self,
        _kind: crate::Kind,
        mut size: u64,
        from: &mut dyn std::io::Read,
        id: gix_hash::ObjectId,
    ) -> Result<gix_hash::ObjectId> {
        let mut buf = [0u8; u16::MAX as usize];
        while size != 0 {
            let bytes = (size as usize).min(buf.len());
            from.read_exact(&mut buf[..bytes]).or_error()?;
            size -= bytes as u64;
        }
        Ok(id)
    }
}
