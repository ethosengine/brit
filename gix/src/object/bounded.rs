//! Operation-owned object bytes with the repository's existing allocation policy.
use std::sync::Arc;

use gix_error::{OptionExt, ResultExt, bail, message};
use gix_object::{Data, FindExt, FindStream, Kind, find::Buffer};
use gix_utils::stream::Budget;

use crate::ext::ObjectIdExt;
use crate::{Repository, Result};

/// Reuse buffered reads below policy and verified immutable spools above it.
#[derive(Clone)]
pub struct Source<'repo> {
    repo: &'repo Repository,
    budget: Arc<Budget>,
    buffer_limit: u64,
}

impl<'repo> Source<'repo> {
    /// Share the operation's scratch quota and preserve the effective allocation limit.
    pub fn new(repo: &'repo Repository, budget: Arc<Budget>) -> Result<Self> {
        Ok(Self {
            repo,
            budget,
            buffer_limit: repo
                .allocation_limit()
                .map_or(repo.big_file_threshold()?, |limit| limit as u64),
        })
    }

    /// Read one object, replacing the previous bytes retained by `buffer`.
    pub fn load<'a>(&self, id: &gix_hash::oid, buffer: &'a mut Buffer) -> Result<Data<'a>> {
        gix_utils::stream::check(&crate::interrupt::IS_INTERRUPTED).or_error()?;
        if id == gix_hash::ObjectId::empty_tree(self.repo.object_hash()) {
            buffer.owned().clear();
            return Ok(Data::new(AsRef::<[u8]>::as_ref(&*buffer), Kind::Tree, id.kind()));
        }
        if self.repo.find_header(id)?.size() <= self.buffer_limit {
            return self
                .repo
                .objects
                .find(id, buffer.owned())
                .or_raise(|| message!("Could not read object {id}"));
        }
        buffer.clear();
        let (kind, spool) = self
            .repo
            .objects
            .find_stream(id, &self.budget, &crate::interrupt::IS_INTERRUPTED)
            .or_raise(|| message!("Could not stream object {id}"))?;
        Ok(Data::new(
            buffer.set_external(spool.into_mapped().or_raise(|| message!("Could not map object {id}"))?),
            kind,
            id.kind(),
        ))
    }

    /// Follow tags and commits to `kind`, reading only headers for the terminal object.
    pub fn peel(&self, mut id: gix_hash::ObjectId, kind: Kind) -> Result<gix_hash::ObjectId> {
        let mut buffer = Buffer::default();
        let mut seen = std::collections::HashSet::new();
        loop {
            let actual = self.repo.find_header(id)?.kind();
            if actual == kind {
                return Ok(id);
            }
            if !seen.insert(id) {
                bail!(gix_error::corruption(format!(
                    "Object cycle while peeling {id} to {kind}"
                )));
            }
            match actual {
                Kind::Commit => {
                    id = self
                        .load(&id, &mut buffer)?
                        .try_into_commit_iter()
                        .ok_or_raise(|| message!("Expected commit {id}").validation())?
                        .tree_id()?;
                }
                Kind::Tag => {
                    id = self
                        .load(&id, &mut buffer)?
                        .try_into_tag_iter()
                        .ok_or_raise(|| message!("Expected tag {id}").validation())?
                        .target_id()?;
                }
                Kind::Tree | Kind::Blob => {
                    bail!(gix_error::validation(format!(
                        "Last encountered object {} was {} while trying to peel to {kind}",
                        id.attach(self.repo).shorten().unwrap_or_else(|_| id.into()),
                        actual
                    )));
                }
            }
        }
    }

    /// Look up a repository-relative path using the existing sequential tree parser.
    pub fn lookup_entry(
        &self,
        mut tree_id: gix_hash::ObjectId,
        path: &std::path::Path,
    ) -> Result<Option<gix_object::tree::Entry>> {
        let mut buffer = Buffer::default();
        let mut components = path.components().map(|c| c.as_os_str().as_encoded_bytes()).peekable();
        loop {
            match gix_object::tree::next_entry(&mut components, self.load(&tree_id, &mut buffer)?) {
                std::ops::ControlFlow::Continue(id) => tree_id = id,
                std::ops::ControlFlow::Break(entry) => return Ok(entry.map(Into::into)),
            }
        }
    }
}

impl gix_traverse::commit::topo::source::Source for Source<'_> {
    fn find_commit<'a>(&self, id: &gix_hash::oid, buffer: &'a mut Buffer) -> Result<gix_object::CommitRefIter<'a>> {
        self.load(id, buffer)?
            .try_into_commit_iter()
            .ok_or_raise(|| message!("Expected commit {id}").validation())
    }
}

impl gix_traverse::tree::source::Source for Source<'_> {
    fn find_tree<'a>(&self, id: &gix_hash::oid, buffer: &'a mut Buffer) -> Result<gix_object::TreeRefIter<'a>> {
        self.load(id, buffer)?
            .try_into_tree_iter()
            .ok_or_raise(|| message!("Expected tree {id}").validation())
    }
}
