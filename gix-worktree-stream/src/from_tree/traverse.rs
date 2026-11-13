use std::{
    collections::VecDeque,
    sync::{Arc, atomic::AtomicBool},
};

use gix_filter::pipeline::{convert::to_worktree, stream::Outcome};
use gix_object::{
    bstr::{BStr, BString, ByteSlice, ByteVec},
    tree,
};
use gix_traverse::tree::{Visit, visit::Action};

use gix_error::{ErrorExt, Result, ResultExt, message};

use crate::{SharedErrorSlot, protocol};

pub struct Delegate<'a, AttributesFn, Find>
where
    Find: gix_object::Find + gix_object::FindStream,
{
    pub(crate) out: &'a mut gix_utils::io::pipe::Writer,
    pub(crate) err: SharedErrorSlot,
    pub(crate) path_deque: VecDeque<BString>,
    pub(crate) path: BString,
    pub(crate) pipeline: gix_filter::Pipeline,
    pub(crate) attrs: gix_attributes::search::Outcome,
    pub(crate) fetch_attributes: AttributesFn,
    pub(crate) objects: Find,
    pub(crate) budget: Arc<gix_utils::stream::Budget>,
}

impl<AttributesFn, Find> Delegate<'_, AttributesFn, Find>
where
    Find: gix_object::Find + gix_object::FindStream,
    AttributesFn: FnMut(&BStr, gix_object::tree::EntryMode, &mut gix_attributes::search::Outcome) -> Result + 'static,
{
    fn pop_element(&mut self) {
        if let Some(pos) = self.path.rfind_byte(b'/') {
            self.path.resize(pos, 0);
        } else {
            self.path.clear();
        }
    }

    fn push_element(&mut self, name: &BStr) {
        if name.is_empty() {
            return;
        }
        if !self.path.is_empty() {
            self.path.push(b'/');
        }
        self.path.push_str(name);
    }
    /// Return the state of the `export-ignore` attribute.
    fn ignore_state(&self) -> gix_attributes::StateRef<'_> {
        self.attrs
            .iter_selected()
            .next()
            .expect("initialized with one attr")
            .assignment
            .state
    }

    fn handle_entry(&mut self, entry: &tree::EntryRef<'_>) -> Result<Action> {
        if !entry.mode.is_blob_or_symlink() {
            return Ok(std::ops::ControlFlow::Continue(true));
        }
        (self.fetch_attributes)(self.path.as_ref(), entry.mode, &mut self.attrs)?;
        if self.ignore_state().is_set() {
            return Ok(std::ops::ControlFlow::Continue(true));
        }
        let interrupt = AtomicBool::new(false);
        let (kind, source) = self
            .objects
            .find_stream(entry.oid, &self.budget, &interrupt)
            .or_raise(|| message!("Could not read archive entry {} ({})", self.path, entry.oid))?;
        if kind != gix_object::Kind::Blob {
            return Err(gix_error::corruption("Archive leaf is not a blob").raise());
        }
        self.pipeline.driver_context_mut().blob = Some(entry.oid.into());
        let converted = self
            .pipeline
            .convert_to_worktree_stream(
                source,
                self.path.as_ref(),
                &mut |a, b| {
                    (self.fetch_attributes)(a, entry.mode, b).ok();
                },
                to_worktree::Options {
                    can_delay: gix_filter::driver::apply::Delay::Forbid,
                    unknown_encoding: to_worktree::UnknownEncoding::Fail,
                },
                &self.budget,
                &interrupt,
            )
            .or_raise(|| message("Could not convert to worktree representation"))?;
        let Outcome::Ready(mut source) = converted else {
            unreachable!("delay is forbidden")
        };
        let size = usize::try_from(source.len()).or_error()?;
        protocol::write_entry_header_and_path(self.path.as_ref(), entry.oid, entry.mode, Some(size), self.out)
            .or_raise(|| message("Could not write entry header"))?;
        gix_utils::stream::copy(&mut source, self.out, &interrupt)
            .or_raise(|| message("Could not write archive entry data"))?;
        Ok(std::ops::ControlFlow::Continue(true))
    }
}

impl<AttributesFn, Find> Visit for Delegate<'_, AttributesFn, Find>
where
    Find: gix_object::Find + gix_object::FindStream,
    AttributesFn: FnMut(&BStr, gix_object::tree::EntryMode, &mut gix_attributes::search::Outcome) -> Result + 'static,
{
    fn pop_back_tracked_path_and_set_current(&mut self) {
        self.path = self.path_deque.pop_back().unwrap_or_default();
    }

    fn pop_front_tracked_path_and_set_current(&mut self) {
        self.path = self
            .path_deque
            .pop_front()
            .expect("every call is matched with push_tracked_path_component");
    }

    fn push_back_tracked_path_component(&mut self, component: &BStr) {
        self.push_element(component);
        self.path_deque.push_back(self.path.clone());
    }

    fn push_path_component(&mut self, component: &BStr) {
        self.push_element(component);
    }

    fn pop_path_component(&mut self) {
        self.pop_element();
    }

    fn visit_tree(&mut self, entry: &tree::EntryRef<'_>) -> Action {
        if let Err(err) = (self.fetch_attributes)(self.path.as_ref(), entry.mode, &mut self.attrs) {
            *self.err.lock() = Some(err);
            std::ops::ControlFlow::Break(())
        } else if self.ignore_state().is_set() {
            std::ops::ControlFlow::Continue(false)
        } else {
            std::ops::ControlFlow::Continue(true)
        }
    }

    fn visit_nontree(&mut self, entry: &tree::EntryRef<'_>) -> Action {
        match self.handle_entry(entry) {
            Ok(action) => action,
            Err(err) => {
                *self.err.lock() = Some(err);
                std::ops::ControlFlow::Break(())
            }
        }
    }
}
