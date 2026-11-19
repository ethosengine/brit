use gix_error::{Result, ResultExt};
use gix_object::{bstr::BStr, tree::EntryKind};
use gix_utils::stream::{self, Budget};
use std::sync::{Arc, atomic::AtomicBool};

/// Operation-owned storage for merge inputs and rename comparisons.
pub struct Bounded<'a> {
    /// Shared quota for all participating blobs and conversions.
    pub budget: Arc<Budget>,
    /// Larger converted resources use immutable mappings instead of buffers.
    pub buffer_limit: usize,
    /// Cancellation flag observed during reads and between merge steps.
    pub interrupt: &'a AtomicBool,
}

pub(crate) trait Storage<O> {
    /// Whether rename similarity loads its candidates through [`Self::diff()`].
    /// Otherwise the rewrite tracker reads them as it does without a storage policy.
    fn loads_diff_resources(&self) -> bool {
        false
    }
    fn finish_diff(&self, _cache: &mut gix_diff::blob::Platform) {}
    fn finish_merge(&self, _cache: &mut crate::blob::Platform, _output: &mut Vec<u8>) {}
    fn check(&self) -> Result {
        Ok(())
    }
    fn diff(
        &self,
        cache: &mut gix_diff::blob::Platform,
        id: gix_hash::ObjectId,
        mode: EntryKind,
        path: &BStr,
        kind: gix_diff::blob::ResourceKind,
        objects: &O,
    ) -> Result;
    fn merge(
        &self,
        cache: &mut crate::blob::Platform,
        id: Option<gix_hash::ObjectId>,
        mode: EntryKind,
        path: &BStr,
        kind: crate::blob::ResourceKind,
        objects: &O,
    ) -> Result;
}
pub(crate) struct Buffered;
impl<O: gix_object::FindObjectOrHeader> Storage<O> for Buffered {
    fn diff(
        &self,
        cache: &mut gix_diff::blob::Platform,
        id: gix_hash::ObjectId,
        mode: EntryKind,
        path: &BStr,
        kind: gix_diff::blob::ResourceKind,
        objects: &O,
    ) -> Result {
        cache.set_resource((!id.is_null()).then_some(id), mode, path, kind, objects)?;
        Ok(())
    }
    fn merge(
        &self,
        cache: &mut crate::blob::Platform,
        id: Option<gix_hash::ObjectId>,
        mode: EntryKind,
        path: &BStr,
        kind: crate::blob::ResourceKind,
        objects: &O,
    ) -> Result {
        cache.set_resource(id, mode, path, kind, objects)
    }
}
impl<O: gix_object::FindObjectOrHeader + gix_object::FindStream> Storage<O> for Bounded<'_> {
    fn loads_diff_resources(&self) -> bool {
        true
    }
    fn finish_diff(&self, cache: &mut gix_diff::blob::Platform) {
        cache.clear_resource_cache();
    }
    fn finish_merge(&self, cache: &mut crate::blob::Platform, output: &mut Vec<u8>) {
        cache.clear_resources();
        *output = Vec::new();
    }
    fn check(&self) -> Result {
        stream::check(self.interrupt).or_error()
    }
    fn diff(
        &self,
        cache: &mut gix_diff::blob::Platform,
        id: gix_hash::ObjectId,
        mode: EntryKind,
        path: &BStr,
        kind: gix_diff::blob::ResourceKind,
        objects: &O,
    ) -> Result {
        cache.set_resource_bounded(
            id,
            mode,
            path,
            kind,
            objects,
            &self.budget,
            self.buffer_limit,
            self.interrupt,
        )?;
        Ok(())
    }
    fn merge(
        &self,
        cache: &mut crate::blob::Platform,
        id: Option<gix_hash::ObjectId>,
        mode: EntryKind,
        path: &BStr,
        kind: crate::blob::ResourceKind,
        objects: &O,
    ) -> Result {
        cache.set_resource_bounded(id, mode, path, kind, objects, self)
    }
}
