mod simple;
mod topo;

struct MappedSource<'a> {
    odb: &'a gix_odb::Handle,
    budget: std::sync::Arc<gix_utils::stream::Budget>,
}

impl gix_object::find::Source for MappedSource<'_> {
    fn try_find<'a>(
        &self,
        id: &gix_hash::oid,
        buffer: &'a mut gix_object::find::Buffer,
    ) -> gix_error::Result<Option<gix_object::Data<'a>>> {
        use gix_error::ResultExt;
        use gix_object::{FindHeader, FindStream};
        buffer.clear();
        if self.odb.try_header(id)?.is_none() {
            return Ok(None);
        }
        let (kind, bytes) = self
            .odb
            .find_stream(id, &self.budget, &std::sync::atomic::AtomicBool::new(false))?;
        Ok(Some(gix_object::Data::new(
            buffer.set_external(bytes.into_mapped().or_error()?),
            kind,
            id.kind(),
        )))
    }
}

impl gix_traverse::commit::topo::source::Source for MappedSource<'_> {
    fn find_commit<'a>(
        &self,
        id: &gix_hash::oid,
        buffer: &'a mut gix_object::find::Buffer,
    ) -> gix_error::Result<gix_object::CommitRefIter<'a>> {
        gix_object::find::Source::find_commit(self, id, buffer)
    }
}
