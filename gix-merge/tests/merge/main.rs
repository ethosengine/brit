use gix_hash::ObjectId;
extern crate core;
extern crate gix_imara_diff as imara_diff;

mod blob;
mod tree;

pub use gix_testtools::Result;

fn hex_to_id(hex: &str) -> ObjectId {
    ObjectId::from_hex(hex.as_bytes()).expect("40 bytes hex")
}

#[expect(
    clippy::too_many_arguments,
    reason = "Mirrors the public commit merge API for storage conformance"
)]
fn commit_with_storage_conformance<'a>(
    ours: ObjectId,
    theirs: ObjectId,
    labels: gix_merge::blob::builtin_driver::text::Labels<'_>,
    graph: &mut gix_revwalk::Graph<'_, '_, gix_revwalk::graph::Commit<gix_revision::merge_base::Flags>>,
    diff: &mut gix_diff::blob::Platform,
    blobs: &mut gix_merge::blob::Platform,
    objects: &'a (impl gix_object::FindObjectOrHeader + gix_object::FindStream + gix_object::Write),
    abbreviate: &mut dyn FnMut(&gix_hash::oid) -> String,
    options: gix_merge::commit::Options,
) -> gix_error::Result<gix_merge::commit::Outcome<'a>> {
    use gix_error::ResultExt;
    use std::sync::atomic::AtomicBool;
    let budget = gix_utils::stream::Budget::new(&std::env::temp_dir()).or_error()?;
    let storage = gix_merge::Bounded {
        budget: budget.clone(),
        buffer_limit: 0,
        interrupt: &AtomicBool::new(false),
    };
    let mut bounded_diff = diff.clone();
    bounded_diff.clear_resource_cache();
    let mut bounded_blobs = blobs.clone();
    let mut bounded_graph = gix_revwalk::Graph::new(objects, None);
    let bounded = gix_merge::commit_bounded(
        ours,
        theirs,
        labels,
        &mut bounded_graph,
        &mut bounded_diff,
        &mut bounded_blobs,
        objects,
        abbreviate,
        options.clone(),
        &storage,
    );
    let buffered = gix_merge::commit(ours, theirs, labels, graph, diff, blobs, objects, abbreviate, options);
    match (&buffered, bounded) {
        (Ok(expected), Ok(mut actual)) => {
            let actual_tree = actual.tree_merge.tree.write(|tree| objects.write(tree))?;
            let expected_tree = expected.tree_merge.tree.clone().write(|tree| objects.write(tree))?;
            assert_eq!(actual_tree, expected_tree);
            assert_eq!(
                format!("{:?}", actual.tree_merge.conflicts),
                format!("{:?}", expected.tree_merge.conflicts)
            );
            assert_eq!(
                actual.tree_merge.failed_on_first_unresolved_conflict,
                expected.tree_merge.failed_on_first_unresolved_conflict
            );
            assert_eq!(actual.merge_bases, expected.merge_bases);
            assert_eq!(actual.merge_base_tree_id, expected.merge_base_tree_id);
            assert_eq!(actual.virtual_merge_bases, expected.virtual_merge_bases);
        }
        (Err(expected), Err(actual)) => assert_eq!(expected.to_string(), actual.to_string()),
        (expected, actual) => panic!(
            "merge storage outcome differs: buffered success {}, bounded success {}",
            expected.is_ok(),
            actual.is_ok()
        ),
    }
    drop(bounded_diff);
    drop(bounded_blobs);
    assert_eq!(budget.usage().0, 0);
    buffered
}
