//! Tree byte ownership, independent of traversal order and entry parsing.
use gix_error::Result;
use gix_object::{FindExt, TreeRefIter, find::Buffer};

/// Read a tree while letting the caller retain its immutable byte storage.
pub trait Source {
    /// Return the requested tree, retaining its backing bytes in `buffer`.
    fn find_tree<'a>(&self, id: &gix_hash::oid, buffer: &'a mut Buffer) -> Result<TreeRefIter<'a>>;
}

impl<T: gix_object::Find> Source for T {
    fn find_tree<'a>(&self, id: &gix_hash::oid, buffer: &'a mut Buffer) -> Result<TreeRefIter<'a>> {
        self.find_tree_iter(id, buffer.owned())
    }
}
