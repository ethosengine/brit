//! Commit byte ownership for the topological walker, independent of traversal ordering.
use gix_error::Result;
use gix_object::{CommitRefIter, FindExt};

/// Reusable storage for one commit, optionally backed by immutable operation-owned bytes.
#[derive(Default)]
pub struct Buffer {
    owned: Vec<u8>,
    external: Option<Box<dyn AsRef<[u8]> + Send + Sync>>,
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
        self.external = Some(Box::new(data));
        self.external.as_deref().expect("just set").as_ref()
    }
}

/// Read a commit using the existing parser while letting the caller own its byte storage.
pub trait Source {
    /// Return the requested commit, retaining its backing bytes in `buffer`.
    fn find_commit<'a>(&self, id: &gix_hash::oid, buffer: &'a mut Buffer) -> Result<CommitRefIter<'a>>;
}

impl<T: gix_object::Find> Source for T {
    fn find_commit<'a>(&self, id: &gix_hash::oid, buffer: &'a mut Buffer) -> Result<CommitRefIter<'a>> {
        self.find_commit_iter(id, buffer.owned())
    }
}

pub(super) fn find<'cache, 'buf>(
    cache: Option<&'cache gix_commitgraph::Graph>,
    objects: &impl Source,
    id: &gix_hash::oid,
    buffer: &'buf mut Buffer,
) -> Result<crate::commit::Either<'buf, 'cache>> {
    use crate::commit::Either;
    match cache.and_then(|cache| cache.commit_by_id(id).map(Either::CachedCommit)) {
        Some(commit) => {
            buffer.clear();
            Ok(commit)
        }
        None => objects.find_commit(id, buffer).map(Either::CommitRefIter),
    }
}
