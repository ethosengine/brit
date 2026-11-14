use gix_fsck::Connectivity;
use gix_hash::ObjectId;
use gix_hashtable::HashMap;
use gix_object::Kind;
use std::sync::LazyLock;

use crate::hex_to_id;

fn check_missing<'a>(repo_name: &str, commits: impl IntoIterator<Item = &'a ObjectId>) -> HashMap<ObjectId, Kind> {
    let db = {
        let fixture_path = gix_testtools::scripted_fixture_read_only("make_test_repos.sh")
            .expect("fixture path")
            .join(repo_name)
            .join(".git")
            .join("objects");
        let mut db = gix_odb::at(fixture_path, gix_testtools::object_hash()).expect("valid odb");
        db.refresh_never();
        db
    };

    let mut missing: HashMap<ObjectId, Kind> = HashMap::default();
    let record_missing_and_assert_no_duplicate = |oid: &ObjectId, kind: Kind| {
        missing.try_insert(*oid, kind).expect("no duplicate oid");
    };

    let mut check = Connectivity::new(db, record_missing_and_assert_no_duplicate);
    for commit in commits.into_iter() {
        check.check_commit(commit).expect("commit is present");
    }
    missing
}

fn hex_to_ids<'a>(hex_ids: impl IntoIterator<Item = &'a str>) -> Vec<ObjectId> {
    hex_ids.into_iter().map(hex_to_id).collect()
}

fn hex_to_objects<'a>(hex_ids: impl IntoIterator<Item = &'a str>, kind: Kind) -> HashMap<ObjectId, Kind> {
    hex_to_ids(hex_ids).into_iter().map(|id| (id, kind)).collect()
}

// Get a `&Vec<ObjectID` for each commit in the test fixture repository
fn all_commits() -> &'static [ObjectId] {
    static ALL_COMMITS: LazyLock<Vec<ObjectId>> = LazyLock::new(|| {
        hex_to_ids([
            "ebed23648b19484cb1f340c4ee04dda08479188a",
            "8ff6d0f8891c3cb22827be142cc64606121d47b3",
            "734c926856a328d1168ffd7088532e0d1ad19bbe",
        ])
    });
    &ALL_COMMITS
}

#[test]
fn no_missing() {
    // The "base" repo is the original, and has every object present
    assert_eq!(check_missing("base", all_commits()), HashMap::default());
}

#[test]
fn missing_blobs() {
    // The "blobless" repo is cloned with `--filter=blob:none`, and is missing one blob
    let expected = hex_to_objects(
        [
            "4cdeaab5b01f9a9fbbb2fb6c08404cf12b7bdab1",
            "c18147dc648481eeb65dc5e66628429a64843327",
        ],
        Kind::Blob,
    );
    assert_eq!(check_missing("blobless", all_commits()), expected);
}

#[test]
fn missing_trees() {
    // The "treeless" repo is cloned with `--filter=tree:0`, and is missing two trees
    // NOTE: This repo is also missing a blob, but we have no way of knowing that, as the tree referencing it is missing
    let expected = hex_to_objects(
        [
            "20317ffa7614f49b2702a057bf2833918ea9fd24",
            "fc264b3b6875a46e9031483aeb9994a1b897ffd3",
        ],
        Kind::Tree,
    );
    assert_eq!(check_missing("treeless", all_commits()), expected);
}

#[test]
fn tree_read_failures_are_not_missing_objects() -> gix_testtools::TestResult {
    use gix_object::{Exists, FindExt};
    struct FailingTree<'a> {
        db: &'a gix_odb::Handle,
        tree: ObjectId,
        class: gix_error::Class,
    }
    impl gix_object::Find for FailingTree<'_> {
        fn try_find<'a>(
            &self,
            id: &gix_hash::oid,
            buffer: &'a mut Vec<u8>,
        ) -> gix_error::Result<Option<gix_object::Data<'a>>> {
            use gix_error::ErrorExt;
            if id == self.tree {
                return Err(gix_error::message!("Failed tree read {id}")
                    .with_class(self.class)
                    .raise());
            }
            gix_object::Find::try_find(self.db, id, buffer)
        }
    }
    impl Exists for FailingTree<'_> {
        fn exists(&self, id: &gix_hash::oid) -> bool {
            self.db.exists(id)
        }
    }
    let fixture = gix_testtools::scripted_fixture_read_only("make_test_repos.sh")?;
    let db = gix_odb::at(fixture.join("base/.git/objects"), gix_testtools::object_hash())?;
    let commit = all_commits()[0];
    let tree = db.find_commit(&commit, &mut Vec::new())?.tree();
    for class in [
        gix_error::Class::ResourceExhaustion(gix_error::ResourceExhaustionKind::AllocationLimit),
        gix_error::Class::ResourceExhaustion(gix_error::ResourceExhaustionKind::AllocationFailure),
        gix_error::Class::ResourceExhaustion(gix_error::ResourceExhaustionKind::ScratchLimit),
        gix_error::Class::Cancelled,
        gix_error::Class::Corruption,
        gix_error::Class::NotFound,
        gix_error::Class::PermissionDenied,
    ] {
        let mut check = Connectivity::new(FailingTree { db: &db, tree, class }, |oid, kind| {
            panic!("a failed read must not be reported as missing {kind} {oid}")
        });
        for _ in 0..2 {
            let error = check
                .check_commit(&commit)
                .expect_err("a failed scan must not enter the visited cache");
            assert_eq!(error.dominant_class(), Some(class));
            assert!(format!("{error:#}").contains(&tree.to_string()));
        }
    }
    Ok(())
}

#[test]
fn malformed_objects_remain_corruption_and_do_not_poison_retries() -> gix_testtools::TestResult {
    struct Malformed {
        commit: ObjectId,
        tree: ObjectId,
        bad_commit: bool,
    }
    impl gix_object::Find for Malformed {
        fn try_find<'a>(
            &self,
            id: &gix_hash::oid,
            buffer: &'a mut Vec<u8>,
        ) -> gix_error::Result<Option<gix_object::Data<'a>>> {
            buffer.clear();
            let kind = if id == self.commit {
                buffer.extend_from_slice(format!("tree {}\n", self.tree).as_bytes());
                buffer.extend_from_slice(if self.bad_commit {
                    b"malformed author header\n"
                } else {
                    b"author Fixture <fixture@example.com> 1 +0000\ncommitter Fixture <fixture@example.com> 1 +0000\n\nmessage\n"
                });
                Kind::Commit
            } else {
                buffer.extend_from_slice(b"malformed tree entry");
                Kind::Tree
            };
            Ok(Some(gix_object::Data::new(buffer, kind, id.kind())))
        }
    }
    impl gix_object::Exists for Malformed {
        fn exists(&self, _: &gix_hash::oid) -> bool {
            true
        }
    }
    let commit = all_commits()[0];
    let tree = hex_to_id("20317ffa7614f49b2702a057bf2833918ea9fd24");
    for bad_commit in [false, true] {
        let mut check = Connectivity::new(
            Malformed {
                commit,
                tree,
                bad_commit,
            },
            |_, _| panic!("malformed data is not a missing object"),
        );
        for _ in 0..2 {
            let error = check.check_commit(&commit).expect_err("malformed object must refuse");
            assert!(error.is_corrupted(), "{error:#}");
            assert!(format!("{error:#}").contains(&if bad_commit { commit } else { tree }.to_string()));
        }
    }
    Ok(())
}

#[test]
fn blob_lookup_errors_survive_pointer_wrappers() -> gix_testtools::TestResult {
    use gix_object::Exists;
    #[derive(Clone)]
    struct FailingHeader(std::path::PathBuf);
    impl gix_object::Find for FailingHeader {
        fn try_find<'a>(
            &self,
            id: &gix_hash::oid,
            buffer: &'a mut Vec<u8>,
        ) -> gix_error::Result<Option<gix_object::Data<'a>>> {
            use gix_error::ResultExt;
            let db = gix_odb::at(&self.0, id.kind()).or_error()?;
            gix_object::Find::try_find(&db, id, buffer)
        }
    }
    impl Exists for FailingHeader {
        fn exists(&self, _: &gix_hash::oid) -> bool {
            panic!("a fallible store must not be reduced to a boolean")
        }
        fn try_exists(&self, id: &gix_hash::oid) -> gix_error::Result<bool> {
            use gix_error::ErrorExt;
            Err(gix_error::message!("Cannot inspect blob {id}")
                .with_class(gix_error::Class::PermissionDenied)
                .raise())
        }
    }
    fn assert_error<T: Exists>(db: T, blob: &gix_hash::oid) {
        let error = db.try_exists(blob).expect_err("metadata lookup failed");
        assert_eq!(error.dominant_class(), Some(gix_error::Class::PermissionDenied));
        assert!(format!("{error:#}").contains(&blob.to_string()));
    }
    let fixture = gix_testtools::scripted_fixture_read_only("make_test_repos.sh")?;
    let source = FailingHeader(fixture.join("base/.git/objects"));
    let blob = hex_to_id("4cdeaab5b01f9a9fbbb2fb6c08404cf12b7bdab1");
    assert_error::<&FailingHeader>(&source, &blob);
    assert_error(Box::new(source.clone()), &blob);
    assert_error(std::rc::Rc::new(source.clone()), &blob);
    assert_error(std::sync::Arc::new(source.clone()), &blob);
    let mut check = Connectivity::new(source, |_, _| panic!("lookup failure is not absence"));
    for _ in 0..2 {
        let error = check
            .check_commit(&all_commits()[0])
            .expect_err("blob metadata lookup failed");
        assert_eq!(error.dominant_class(), Some(gix_error::Class::PermissionDenied));
    }
    Ok(())
}
