//! Operation-owned object bytes with the repository's existing allocation policy.
use std::{cell::OnceCell, rc::Rc, sync::Arc};

use gix_error::{OptionExt, ResultExt, bail, message};
use gix_object::{Data, FindExt, FindStream, Kind, find::Buffer};
use gix_odb::{Header, HeaderExt};
use gix_utils::stream::Budget;

use crate::ext::ObjectIdExt;
use crate::{Repository, Result};

/// Reuse buffered reads below policy and verified immutable spools above it.
#[derive(Clone)]
pub struct Source<'repo> {
    repo: &'repo Repository,
    budget: Rc<OnceCell<Arc<Budget>>>,
    buffer_limit: OnceCell<u64>,
    synthesize_empty_tree: bool,
}

impl<'repo> Source<'repo> {
    /// Share the operation's scratch quota and preserve the effective allocation limit.
    pub fn new(repo: &'repo Repository, budget: Arc<Budget>) -> Result<Self> {
        let source = Self {
            repo,
            budget: Rc::new(OnceCell::from(budget)),
            buffer_limit: OnceCell::new(),
            synthesize_empty_tree: true,
        };
        source.buffer_limit()?;
        Ok(source)
    }

    /// Resolve buffering policy on first read and create scratch only when mapping is required.
    ///
    /// This keeps infallible graph construction and cached/header-only operations independent of
    /// temporary storage. Reads return policy or scratch setup failures. Clones share the budget.
    pub fn deferred(repo: &'repo Repository) -> Self {
        Self {
            repo,
            budget: Rc::new(OnceCell::new()),
            buffer_limit: OnceCell::new(),
            synthesize_empty_tree: true,
        }
    }

    /// Require stored bytes, including for the canonical empty tree, when checking connectivity.
    pub fn stored_only(mut self) -> Self {
        self.synthesize_empty_tree = false;
        self
    }

    fn buffer_limit(&self) -> Result<u64> {
        if let Some(limit) = self.buffer_limit.get() {
            return Ok(*limit);
        }
        let limit = self
            .repo
            .allocation_limit()
            .map_or_else(|| self.repo.big_file_threshold(), |limit| Ok(limit as u64))?;
        Ok(*self.buffer_limit.get_or_init(|| limit))
    }

    fn budget(&self) -> Result<&Arc<Budget>> {
        if let Some(budget) = self.budget.get() {
            return Ok(budget);
        }
        let budget = Budget::new(&std::env::temp_dir()).or_error()?;
        Ok(self.budget.get_or_init(|| budget))
    }

    /// Read one object, replacing the previous bytes retained by `buffer`.
    pub fn load<'a>(&self, id: &gix_hash::oid, buffer: &'a mut Buffer) -> Result<Data<'a>> {
        gix_utils::stream::check(&crate::interrupt::IS_INTERRUPTED).or_error()?;
        if self.synthesize_empty_tree && id == gix_hash::ObjectId::empty_tree(self.repo.object_hash()) {
            buffer.owned().clear();
            return Ok(Data::new(AsRef::<[u8]>::as_ref(&*buffer), Kind::Tree, id.kind()));
        }
        if self.repo.objects.header(id)?.size() <= self.buffer_limit()? {
            match self.repo.objects.find(id, buffer.owned()).map(|data| data.kind) {
                Ok(kind) => return Ok(Data::new(AsRef::<[u8]>::as_ref(&*buffer), kind, id.kind())),
                // A small result may need a larger delta base or instruction buffer.
                // Only a configured allocation limit selects the streamed retry.
                Err(error)
                    if error.classify().any(|class| {
                        class.class()
                            == gix_error::Class::ResourceExhaustion(gix_error::ResourceExhaustionKind::AllocationLimit)
                    }) => {}
                Err(error) => return Err(error).or_raise(|| message!("Could not read object {id}")),
            }
        }
        buffer.clear();
        let budget = self
            .budget()
            .or_raise(|| message!("Could not create scratch for object {id}"))?;
        let (kind, spool) = self
            .repo
            .objects
            .find_stream(id, budget, &crate::interrupt::IS_INTERRUPTED)
            .or_raise(|| message!("Could not stream object {id}"))?;
        Ok(Data::new(
            buffer.set_external(spool.into_mapped().or_raise(|| message!("Could not map object {id}"))?),
            kind,
            id.kind(),
        ))
    }

    /// Follow tags and commits to `kind`, reading only headers for the terminal object.
    pub fn peel(&self, id: gix_hash::ObjectId, kind: Kind) -> Result<gix_hash::ObjectId> {
        self.peel_inner(id, Some(kind))
    }

    /// Follow tags to the first non-tag object, reading only its header.
    pub fn peel_tags(&self, id: gix_hash::ObjectId) -> Result<gix_hash::ObjectId> {
        self.peel_inner(id, None)
    }

    fn peel_inner(&self, mut id: gix_hash::ObjectId, kind: Option<Kind>) -> Result<gix_hash::ObjectId> {
        let mut buffer = Buffer::default();
        let mut seen = std::collections::HashSet::new();
        loop {
            let actual = self.repo.find_header(id)?.kind();
            if kind.map_or(actual != Kind::Tag, |kind| actual == kind) {
                return Ok(id);
            }
            if !seen.insert(id) {
                bail!(gix_error::corruption(format!(
                    "Object cycle while peeling {id} to {}",
                    kind.map_or_else(|| "a non-tag object".to_owned(), |kind| kind.to_string())
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
                        "Last encountered object {} was {} while trying to peel to {}",
                        id.attach(self.repo).shorten().unwrap_or_else(|_| id.into()),
                        actual,
                        kind.expect("non-tag objects already returned when peeling tags")
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

impl gix_object::Exists for Source<'_> {
    fn exists(&self, id: &gix_hash::oid) -> bool {
        gix_object::Exists::exists(&self.repo.objects, id)
    }
    fn try_exists(&self, id: &gix_hash::oid) -> Result<bool> {
        gix_utils::stream::check(&crate::interrupt::IS_INTERRUPTED).or_error()?;
        self.repo.objects.try_header(id).map(|header| header.is_some())
    }
}

impl gix_object::find::Source for Source<'_> {
    fn try_find<'a>(&self, id: &gix_hash::oid, buffer: &'a mut Buffer) -> Result<Option<Data<'a>>> {
        if !(self.synthesize_empty_tree && id == gix_hash::ObjectId::empty_tree(self.repo.object_hash()))
            && self.repo.objects.try_header(id)?.is_none()
        {
            buffer.clear();
            return Ok(None);
        }
        self.load(id, buffer).map(Some)
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
