use std::{
    borrow::Cow,
    sync::{Arc, atomic::AtomicBool},
};

use gix_error::{Result, ResultExt, bail, message};
use gix_object::{FindExt, bstr::BStr};
use gix_utils::stream::{self, Budget, Mapped};

/// Operation-owned storage for blame inputs and comparison resources.
pub struct Bounded<'a> {
    /// Shared quota for initial blobs, conversions and historical comparisons.
    pub budget: Arc<Budget>,
    /// Largest buffered comparison resource; larger resources use immutable mappings.
    pub buffer_limit: usize,
    /// Cancellation flag checked during object reads and conversion.
    pub interrupt: &'a AtomicBool,
}

pub(super) trait Storage<O> {
    type Bytes: AsRef<[u8]>;
    fn check(&self) -> Result {
        Ok(())
    }
    fn load(&self, odb: &O, id: &gix_hash::oid, buffer: &mut Vec<u8>) -> Result<Self::Bytes>;
    fn retain(&self, contents: Cow<'_, [u8]>) -> Result<Self::Bytes>;
    fn resource(
        &self,
        odb: &O,
        platform: &mut gix_diff::blob::Platform,
        id: gix_hash::ObjectId,
        mode: gix_object::tree::EntryKind,
        path: &BStr,
        kind: gix_diff::blob::ResourceKind,
    ) -> Result;
}

pub(super) struct Buffered;

impl<O: gix_object::Find + gix_object::FindHeader> Storage<O> for Buffered {
    type Bytes = Vec<u8>;

    fn load(&self, odb: &O, id: &gix_hash::oid, buffer: &mut Vec<u8>) -> Result<Self::Bytes> {
        odb.find_blob(id, buffer)
            .or_raise(|| message("Could not find existing blob or commit"))?;
        Ok(std::mem::take(buffer))
    }

    fn retain(&self, contents: Cow<'_, [u8]>) -> Result<Self::Bytes> {
        Ok(contents.into_owned())
    }

    fn resource(
        &self,
        odb: &O,
        platform: &mut gix_diff::blob::Platform,
        id: gix_hash::ObjectId,
        mode: gix_object::tree::EntryKind,
        path: &BStr,
        kind: gix_diff::blob::ResourceKind,
    ) -> Result {
        platform.set_resource(id, mode, path, kind, odb)?;
        Ok(())
    }
}

impl<O: gix_object::Find + gix_object::FindHeader + gix_object::FindStream> Storage<O> for Bounded<'_> {
    type Bytes = Mapped;

    fn check(&self) -> Result {
        stream::check(self.interrupt).or_error()
    }

    fn load(&self, odb: &O, id: &gix_hash::oid, _buffer: &mut Vec<u8>) -> Result<Self::Bytes> {
        let (kind, spool) = odb
            .find_stream(id, &self.budget, self.interrupt)
            .or_raise(|| message!("Could not stream blame blob {id}"))?;
        if kind != gix_object::Kind::Blob {
            bail!(gix_error::validation(format!("Expected blame blob {id}, found {kind}")));
        }
        spool.into_mapped().or_error()
    }

    fn retain(&self, contents: Cow<'_, [u8]>) -> Result<Self::Bytes> {
        let mut spool = self.budget.spool().or_error()?;
        stream::copy(&mut contents.as_ref(), &mut spool, self.interrupt).or_error()?;
        spool.into_mapped().or_error()
    }

    fn resource(
        &self,
        odb: &O,
        platform: &mut gix_diff::blob::Platform,
        id: gix_hash::ObjectId,
        mode: gix_object::tree::EntryKind,
        path: &BStr,
        kind: gix_diff::blob::ResourceKind,
    ) -> Result {
        platform.set_resource_bounded(
            id,
            mode,
            path,
            kind,
            odb,
            &self.budget,
            self.buffer_limit,
            self.interrupt,
        )?;
        Ok(())
    }
}
