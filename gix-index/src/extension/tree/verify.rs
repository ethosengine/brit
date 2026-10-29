use gix_error::{Result, corruption};
use std::cmp::Ordering;

use bstr::ByteSlice;
use gix_error::{OptionExt, ResultExt, bail};
use gix_object::FindExt;

use crate::extension::Tree;

impl Tree {
    /// Validate the correctness of this instance. If `use_objects` is true, then `objects` will be used to access all objects.
    pub fn verify(&self, use_objects: bool, objects: impl gix_object::Find) -> Result {
        fn verify_recursive(
            parent_id: gix_hash::ObjectId,
            parent_num_entries: Option<u32>,
            children: &[Tree],
            mut object_buf: Option<&mut Vec<u8>>,
            objects: &impl gix_object::Find,
        ) -> Result<Option<u32>> {
            if children.is_empty() {
                return Ok(None);
            }
            let mut entries = 0u32;
            let mut prev = None::<&Tree>;
            for child in children {
                entries = entries
                    .checked_add(child.num_entries.unwrap_or(0))
                    .ok_or_raise(|| corruption("The combined TREE entry count exceeds the supported maximum"))?;
                if let Some(prev) = prev
                    && prev.name.cmp(&child.name) != Ordering::Less
                {
                    bail!(corruption(format!(
                        "Parent tree '{parent_id}' contained out-of order trees prev = \"{}\" and next = \"{}\"",
                        prev.name.as_bstr(),
                        child.name.as_bstr()
                    )));
                }
                prev = Some(child);
            }
            if parent_num_entries.is_some()
                && let Some(buf) = object_buf.as_mut()
            {
                let tree_entries = objects
                    .find_tree_iter(&parent_id, buf)
                    .or_raise(|| corruption("Tree node could not be found"))?;
                let mut num_entries = 0;
                for entry in tree_entries {
                    let entry =
                        entry.or_raise(|| corruption(format!("Could not decode an entry in tree {parent_id}")))?;
                    if !entry.mode.is_tree() {
                        continue;
                    }
                    children
                        .binary_search_by(|e| e.name.as_bstr().cmp(entry.filename))
                        .map_err(|position| {
                            gix_error::message!(
                                "The entry {} at path \"{}\" in parent tree {parent_id} wasn't found at child position {position}, making it incomplete",
                                entry.oid, entry.filename
                            ).corrupted_error()
                        })?;
                    num_entries += 1;
                }

                if num_entries != children.len() {
                    bail!(
                        "The tree with id {parent_id} should have {num_entries} children, but its cached representation had {} of them"
                            .corrupted(),
                        children.len()
                    );
                }
            }
            for child in children {
                // This is actually needed here as it's a mut ref, which isn't copy. We do a re-borrow here.
                let actual_num_entries = verify_recursive(
                    child.id,
                    child.num_entries,
                    &child.children,
                    object_buf.as_deref_mut(),
                    objects,
                )?;
                if let Some((actual, num_entries)) = actual_num_entries.zip(child.num_entries)
                    && actual > num_entries
                {
                    bail!(
                        "Expected not more than {num_entries} entries to be reachable from the top-level, but actual count was {actual}"
                            .corrupted()
                    );
                }
            }
            Ok(entries.into())
        }
        let _span = gix_trace::coarse!("gix_index::extension::Tree::verify()");

        if !self.name.is_empty() {
            bail!(corruption(format!(
                "The root tree was named \"{}\", even though it should be empty",
                self.name.as_bstr()
            )));
        }

        let mut buf = Vec::new();
        let declared_entries = verify_recursive(
            self.id,
            self.num_entries,
            &self.children,
            use_objects.then_some(&mut buf),
            &objects,
        )?;
        if let Some((actual, num_entries)) = declared_entries.zip(self.num_entries)
            && actual > num_entries
        {
            bail!(
                "Expected not more than {num_entries} entries to be reachable from the top-level, but actual count was {actual}"
                    .corrupted()
            );
        }

        Ok(())
    }

    /// Reject impossible cached entry counts using the total number of index entries as an upper bound.
    ///
    /// This is a cheap heuristic: it doesn't prove each cached subtree count matches its actual path range,
    /// but no TREE node can describe more entries than the entire index contains.
    pub(crate) fn verify_entries_count(&self, num_index_entries: usize) -> Result {
        if let Some(actual) = self.num_entries
            && actual as usize > num_index_entries
        {
            bail!(
                "TREE entry \"{}\" declared {actual} entries, but the index only contains {num_index_entries} entries"
                    .corrupted(),
                self.name.as_bstr()
            );
        }

        for child in &self.children {
            child.verify_entries_count(num_index_entries)?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::Tree;
    use gix_error::Result;
    use std::cell::Cell;

    struct OneTree {
        id: gix_hash::ObjectId,
        data: Vec<u8>,
        lookups: Cell<usize>,
    }

    impl OneTree {
        fn with_child(id: gix_hash::ObjectId, child_name: &[u8], child_id: gix_hash::ObjectId) -> Self {
            let mut data = b"40000 ".to_vec();
            data.extend_from_slice(child_name);
            data.push(0);
            data.extend_from_slice(child_id.as_bytes());
            OneTree {
                id,
                data,
                lookups: Cell::new(0),
            }
        }
    }

    impl gix_object::Find for OneTree {
        fn try_find<'a>(&self, id: &gix_hash::oid, buffer: &'a mut Vec<u8>) -> Result<Option<gix_object::Data<'a>>> {
            self.lookups.set(self.lookups.get() + 1);
            if id != self.id {
                return Ok(None);
            }
            buffer.clear();
            buffer.extend_from_slice(&self.data);
            Ok(Some(gix_object::Data::new(buffer, gix_object::Kind::Tree, id.kind())))
        }
    }

    fn object_id(hex: &[u8; 40]) -> gix_hash::ObjectId {
        gix_hash::ObjectId::from_hex(hex).expect("test object IDs are valid SHA-1 hex")
    }

    fn tree(name: &[u8], id: gix_hash::ObjectId, num_entries: Option<u32>, children: Vec<Tree>) -> Tree {
        Tree {
            name: name.into(),
            id,
            num_entries,
            children,
        }
    }

    #[test]
    fn invalidated_parent_skips_its_object_but_verifies_valid_children() {
        let child_id = object_id(b"1111111111111111111111111111111111111111");
        let grandchild_id = object_id(b"2222222222222222222222222222222222222222");
        let root = tree(
            b"",
            gix_hash::Kind::Sha1.null(),
            None,
            vec![tree(
                b"child",
                child_id,
                Some(1),
                vec![tree(b"grandchild", grandchild_id, Some(1), Vec::new())],
            )],
        );

        let objects = OneTree::with_child(child_id, b"grandchild", grandchild_id);
        root.verify(true, &objects)
            .expect("the invalidated root is skipped while its valid child is verified");
        assert_eq!(objects.lookups.get(), 1, "only the valid child should be looked up");
    }

    #[test]
    fn valid_node_with_missing_object_still_fails() {
        let root = tree(
            b"",
            object_id(b"1111111111111111111111111111111111111111"),
            Some(1),
            vec![tree(
                b"child",
                object_id(b"2222222222222222222222222222222222222222"),
                Some(1),
                Vec::new(),
            )],
        );

        let err = root
            .verify(true, gix_object::find::Never)
            .expect_err("a valid node must resolve its object ID");
        assert_eq!(
            err.to_string(),
            "Tree node could not be found",
            "the failed valid-node lookup retains its verification context"
        );
        assert!(err.is_corrupted(), "a missing valid-node object is corruption");
    }

    #[test]
    fn valid_node_with_too_few_declared_entries_still_fails() {
        let root_id = object_id(b"1111111111111111111111111111111111111111");
        let child_id = object_id(b"2222222222222222222222222222222222222222");
        let root = tree(
            b"",
            root_id,
            Some(0),
            vec![tree(b"child", child_id, Some(1), Vec::new())],
        );

        let err = root
            .verify(true, OneTree::with_child(root_id, b"child", child_id))
            .expect_err("a valid node must cover all entries reachable through its children");
        assert_eq!(
            err.to_string(),
            "Expected not more than 0 entries to be reachable from the top-level, but actual count was 1",
            "valid nodes still enforce their declared entry count"
        );
        assert!(
            err.is_corrupted(),
            "an inconsistent valid-node entry count is corruption"
        );
    }

    struct MalformedTree;

    impl gix_object::Find for MalformedTree {
        fn try_find<'a>(&self, _id: &gix_hash::oid, _buffer: &'a mut Vec<u8>) -> Result<Option<gix_object::Data<'a>>> {
            Ok(Some(gix_object::Data::new(
                b"40000 child\0",
                gix_object::Kind::Tree,
                gix_hash::Kind::Sha1,
            )))
        }
    }

    #[test]
    fn malformed_object_tree_entries_are_not_ignored() {
        let root_id = gix_hash::Kind::Sha1.null();
        let tree = Tree {
            name: Default::default(),
            id: root_id,
            num_entries: Some(1),
            children: vec![Tree {
                name: b"child".as_slice().into(),
                id: root_id,
                num_entries: Some(0),
                children: Vec::new(),
            }],
        };

        let err = tree.verify(true, MalformedTree).expect_err("malformed entry must fail");
        insta::assert_debug_snapshot!(gix_testtools::redact_debug_snapshot(&(err), &[]), "malformed object tree entries are not ignored", @"
        Could not decode an entry in tree Oid(1)

        Caused by:
            0: object parsing failed
        ");
        assert!(err.is_validation());
    }
}
