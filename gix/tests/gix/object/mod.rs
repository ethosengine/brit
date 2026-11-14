mod blob;
mod commit;
mod tree;

use gix_testtools::size_ok;

#[test]
fn small_delta_results_stream_when_their_base_exceeds_allocation_policy() -> gix_testtools::TestResult {
    use gix::objs::FindExt;
    use std::{io::Write, sync::Arc};

    for refs in [false, true] {
        let dir = gix_testtools::tempfile::tempdir()?;
        assert!(
            gix_testtools::git_command(dir.path())
                .args(["init", "-q"])
                .status()?
                .success()
        );
        let repo = gix::open_opts(
            dir.path(),
            gix::open::Options::isolated().cli_overrides(["gitoxide.objects.allocLimit=16"]),
        )?;
        let kind = repo.object_hash();
        let base = vec![b'x'; 2048];
        let expected = [b'x'; 16];
        let id = gix::objs::compute_hash(kind, gix::objs::Kind::Blob, &expected)?;
        let base_id = gix::objs::compute_hash(kind, gix::objs::Kind::Blob, &base)?;
        let mut pack = gix_pack::data::header::encode(gix_pack::data::Version::V2, 2).to_vec();
        let base_offset = pack.len();
        gix_pack::data::entry::Header::Blob.write_to(base.len() as u64, &mut pack)?;
        let compress = |data: &[u8]| -> std::io::Result<Vec<u8>> {
            let mut output = gix::zlib::stream::deflate::Write::new(Vec::new(), gix::zlib::Compression::BEST_SPEED);
            output.write_all(data)?;
            output.flush()?;
            Ok(output.into_inner())
        };
        pack.extend(compress(&base)?);
        let header = if refs {
            gix_pack::data::entry::Header::RefDelta { base_id }
        } else {
            gix_pack::data::entry::Header::OfsDelta {
                base_distance: (pack.len() - base_offset) as u64,
            }
        };
        // Base size 2048, result size 16, copy 16 bytes from offset zero.
        let delta = [0x80, 0x10, 0x10, 0x90, 0x10];
        header.write_to(delta.len() as u64, &mut pack)?;
        pack.extend(compress(&delta)?);
        let mut hash = gix::hash::hasher(kind);
        hash.update(&pack);
        let pack_id = hash.try_finalize()?;
        pack.extend_from_slice(pack_id.as_slice());
        let path = repo.git_dir().join(format!("objects/pack/pack-{pack_id}.pack"));
        std::fs::write(&path, pack)?;
        assert!(
            gix_testtools::git_command(dir.path())
                .arg("index-pack")
                .arg(&path)
                .output()?
                .status
                .success()
        );
        let error = repo
            .objects
            .find(&id, &mut Vec::new())
            .expect_err("ordinary buffering keeps its explicit limit");
        assert!(error.classify().any(|class| class.class()
            == gix::error::Class::ResourceExhaustion(gix::error::ResourceExhaustionKind::AllocationLimit)));
        let budget = gix::utils::stream::Budget::new(dir.path())?;
        let source = gix::object::bounded::Source::new(&repo, Arc::clone(&budget))?;
        let mut buffer = gix::objs::find::Buffer::default();
        assert_eq!(source.load(&id, &mut buffer)?.data, expected);
        assert_eq!(budget.usage().0, 16, "only the mapped result remains charged");
        drop(buffer);
        assert_eq!(budget.usage().0, 0);
    }
    Ok(())
}

#[test]
fn public_errors_use_the_crate_result_and_preserve_diagnostics() -> gix_testtools::TestResult {
    let repo = crate::basic_repo()?;
    let mut commit = repo.head_commit()?;
    commit.data = b"invalid commit".to_vec();
    let tag = gix::ObjectDetached {
        id: repo.object_hash().null(),
        kind: gix::objs::Kind::Tag,
        data: b"invalid tag".to_vec(),
    }
    .attach(&repo)
    .into_tag();
    let tree = gix::Tree::from_data(repo.object_hash().null(), b"invalid tree".to_vec(), &repo);
    let entry = tree.iter().next().expect("malformed entry is reported").map(|_| ());
    let results: [gix::Result<()>; 13] = [
        commit.message().map(|_| ()),
        commit.message_raw().map(|_| ()),
        commit.decode().map(|_| ()),
        commit.author().map(|_| ()),
        commit.committer().map(|_| ()),
        commit.tree_id().map(|_| ()),
        commit.signature().map(|_| ()),
        tag.decode().map(|_| ()),
        tag.target_id().map(|_| ()),
        tag.tagger().map(|_| ()),
        tree.decode().map(|_| ()),
        entry,
        gix::objs::Tree::try_from(tree).map(|_| ()),
    ];
    for result in results {
        let error = result.expect_err("malformed objects must fail to decode");
        assert!(
            error.downcast_any_ref::<gix_error::Message>().is_some(),
            "decoding errors retain their concrete plumbing diagnostic"
        );
    }

    let result: gix::Result<gix::Commit<'_>> = repo.head_tree_id()?.object()?.try_into_commit();
    assert!(
        result.expect_err("a tree cannot be a commit").is_validation(),
        "conversion errors retain their validation classification"
    );
    Ok(())
}

#[test]
fn failed_object_conversions_return_the_original_object() -> gix_testtools::TestResult {
    use gix::objs::Kind;

    let repo = crate::basic_repo()?;
    let object_id = repo.head_commit()?.id;
    for kind in [Kind::Blob, Kind::Commit, Kind::Tag, Kind::Tree] {
        let object = gix::ObjectDetached {
            id: object_id,
            kind,
            data: b"object bytes".to_vec(),
        }
        .attach(&repo);
        let data_ptr = object.data.as_ptr();
        let object: gix::Object<'_> = match kind {
            Kind::Blob => gix::Commit::try_from(object).expect_err("a blob is not a commit"),
            Kind::Commit => gix::Tag::try_from(object).err().expect("a commit is not a tag"),
            Kind::Tag => gix::Tree::try_from(object).expect_err("a tag is not a tree"),
            Kind::Tree => gix::Blob::try_from(object).err().expect("a tree is not a blob"),
        };
        assert_eq!(object.id, object_id, "the original object ID is returned");
        assert_eq!(object.kind, kind, "the original object kind is returned");
        assert_eq!(object.data, b"object bytes", "the original object data is returned");
        assert_eq!(object.data.as_ptr(), data_ptr, "the original allocation is returned");
        assert!(std::ptr::eq(object.repo, &repo), "the original repository is retained");
    }
    Ok(())
}

#[test]
fn object_ref_size_in_memory() {
    let actual = std::mem::size_of::<gix::Object<'_>>();
    let sha1 = 56;
    let sha256_extra = 16;
    let expected = sha1 + sha256_extra;
    assert!(
        size_ok(actual, expected),
        "the size of this structure should not change unexpectedly: {actual} <~ {expected}"
    );
}

#[test]
fn oid_size_in_memory() {
    let actual = std::mem::size_of::<gix::Id<'_>>();
    let sha1 = 32;
    let sha256_extra = 16;
    let expected = sha1 + sha256_extra;
    assert!(
        size_ok(actual, expected),
        "the size of this structure should not change unexpectedly: {actual} <~ {expected}"
    );
}
