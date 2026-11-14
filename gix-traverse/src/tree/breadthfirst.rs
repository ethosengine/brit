use std::collections::VecDeque;

use gix_hash::ObjectId;

/// The state used and potentially shared by multiple tree traversals.
#[derive(Default, Clone)]
pub struct State {
    next: VecDeque<ObjectId>,
    buf: gix_object::find::Buffer,
}

impl State {
    fn clear(&mut self) {
        self.next.clear();
        self.buf.owned().clear();
    }
}

pub(super) mod function {
    use gix_error::Result;
    use std::borrow::BorrowMut;

    use gix_error::{ResultExt, bail, cancelled};
    use gix_object::TreeRefIter;

    use super::State;
    use crate::tree::Visit;

    /// Start a breadth-first iteration over the `root` trees entries.
    ///
    /// Note that non-trees will be listed first, so the natural order of entries within a tree is lost.
    ///
    /// * `root`
    ///   * the tree to iterate in a nested fashion.
    /// * `state` - all state used for the iteration. If multiple iterations are performed, allocations can be minimized by reusing
    ///   this state.
    /// * `find` - a way to lookup new object data during traversal by their `ObjectId`, writing their data into buffer and returning
    ///   an iterator over entries if the object is present and is a tree. Caching should be implemented within this function
    ///   as needed. The return value is `Option<TreeIter>` which degenerates all error information. Not finding a commit should also
    ///   be considered an error as all objects in the tree DAG should be present in the database. Hence lookup errors should
    ///   be escalated into a more specific error if encountered by the caller.
    /// * `delegate` - A way to observe entries and control the iteration while allowing the optimizer to let you pay only for what you use.
    pub fn breadthfirst<StateMut, Find, V>(
        root: TreeRefIter<'_>,
        mut state: StateMut,
        objects: Find,
        delegate: &mut V,
    ) -> Result
    where
        Find: crate::tree::source::Source,
        StateMut: BorrowMut<State>,
        V: Visit,
    {
        let state = state.borrow_mut();
        state.clear();
        let result = traverse(root, state, objects, delegate);
        // Retain reusable Vec capacity, but never retain an operation-owned mapping
        // after completion, cancellation or a failed lookup in a borrowed state.
        state.buf.owned();
        result
    }

    fn traverse<Find: crate::tree::source::Source, V: Visit>(
        root: TreeRefIter<'_>,
        state: &mut State,
        objects: Find,
        delegate: &mut V,
    ) -> Result {
        let mut tree = root;
        loop {
            for entry in tree {
                let entry = entry.or_raise(|| gix_error::corruption("A tree could not be decoded during traversal"))?;
                if entry.mode.is_tree() {
                    delegate.push_path_component(entry.filename);
                    let action = delegate.visit_tree(&entry);
                    match action {
                        std::ops::ControlFlow::Continue(false) => {}
                        std::ops::ControlFlow::Continue(true) => {
                            delegate.pop_path_component();
                            delegate.push_back_tracked_path_component(entry.filename);
                            state.next.push_back(entry.oid.to_owned());
                        }
                        std::ops::ControlFlow::Break(()) => {
                            bail!(cancelled("The delegate cancelled the operation"));
                        }
                    }
                } else {
                    delegate.push_path_component(entry.filename);
                    if delegate.visit_nontree(&entry).is_break() {
                        bail!(cancelled("The delegate cancelled the operation"));
                    }
                }
                delegate.pop_path_component();
            }
            match state.next.pop_front() {
                Some(oid) => {
                    delegate.pop_front_tracked_path_and_set_current();
                    tree = objects.find_tree(&oid, &mut state.buf)?;
                }
                None => break Ok(()),
            }
        }
    }
}
