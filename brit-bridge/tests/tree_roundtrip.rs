use std::{collections::HashMap, fs, path::Path, sync::Mutex};

use async_trait::async_trait;
use brit_bridge::{resolve_tree_revision, restore_git_tree, seal_git_tree};
use bytes::Bytes;
use eprfs_core::{
    AttestationDraft, BlobCid, BlobHandle, BlobPresence, EprRecord, EprRef, EprfsError, EprfsStorage, FetchPolicy,
    TreeLimits,
};
use gix_testtools::{git, git_command, Result};

#[derive(Default)]
struct MemoryStorage(Mutex<HashMap<BlobCid, Bytes>>);

impl MemoryStorage {
    fn corrupt(&self, cid: &BlobCid) {
        self.0
            .lock()
            .expect("storage lock")
            .insert(cid.clone(), Bytes::from_static(b"corrupt"));
    }
}

#[async_trait]
impl EprfsStorage for MemoryStorage {
    async fn resolve_epr(&self, reference: &EprRef) -> eprfs_core::Result<EprRecord> {
        Err(EprfsError::EprNotFound(reference.clone()))
    }

    async fn has_blob(&self, cid: &BlobCid) -> eprfs_core::Result<BlobPresence> {
        Ok(if self.0.lock().expect("storage lock").contains_key(cid) {
            BlobPresence::Local
        } else {
            BlobPresence::Missing
        })
    }

    async fn fetch_blob(&self, cid: &BlobCid, _policy: FetchPolicy) -> eprfs_core::Result<BlobHandle> {
        let bytes = self
            .0
            .lock()
            .expect("storage lock")
            .get(cid)
            .cloned()
            .ok_or_else(|| EprfsError::BlobNotFound(cid.clone()))?;
        Ok(BlobHandle {
            cid: cid.clone(),
            bytes,
        })
    }

    async fn put_blob(&self, bytes: Bytes) -> eprfs_core::Result<BlobCid> {
        let cid = BlobCid::compute_raw(&bytes);
        self.put_blob_verified(&cid, bytes).await?;
        Ok(cid)
    }

    async fn put_blob_verified(&self, cid: &BlobCid, bytes: Bytes) -> eprfs_core::Result<()> {
        if !cid.verifies(&bytes) {
            return Err(EprfsError::Storage("CID mismatch".into()));
        }
        self.0.lock().expect("storage lock").insert(cid.clone(), bytes);
        Ok(())
    }

    async fn publish_attestation(&self, _draft: AttestationDraft) -> eprfs_core::Result<EprRef> {
        Err(EprfsError::Storage("not used by tree bridge".into()))
    }
}

fn open(path: &Path) -> Result<gix::Repository> {
    Ok(gix::open_opts(path, gix::open::Options::isolated())?)
}

fn fixture() -> Result<(tempfile::TempDir, gix::ObjectId)> {
    fixture_with_format("sha1")
}

fn fixture_with_format(object_format: &str) -> Result<(tempfile::TempDir, gix::ObjectId)> {
    let dir = tempfile::tempdir()?;
    git(dir.path(), &format!("init --object-format={object_format}"))?;
    fs::write(dir.path().join("base"), b"base\n")?;
    git(dir.path(), "add base")?;
    git(
        dir.path(),
        "-c user.name=Test -c user.email=test@example.test commit -m base",
    )?;
    let commit_id = git(dir.path(), "rev-parse HEAD")?;

    fs::create_dir_all(dir.path().join("nested"))?;
    fs::write(dir.path().join("nested/binary"), [0, 0xff, 0x80, b'\n'])?;
    fs::write(dir.path().join("run.sh"), b"#!/bin/sh\nexit 0\n")?;
    #[cfg(unix)]
    {
        use std::{
            ffi::OsString,
            os::unix::{ffi::OsStringExt, fs::PermissionsExt},
        };
        fs::set_permissions(dir.path().join("run.sh"), fs::Permissions::from_mode(0o755))?;
        std::os::unix::fs::symlink("nested/binary", dir.path().join("shortcut"))?;
        let name = OsString::from_vec(vec![b'n', 0xff, b'm']);
        fs::write(dir.path().join(&name), b"byte name\n")?;
        let output = git_command(dir.path()).arg("add").arg(name).output()?;
        assert!(
            output.status.success(),
            "stage byte name: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        git(dir.path(), "add shortcut")?;
    }
    git(dir.path(), "add nested/binary run.sh")?;
    git(
        dir.path(),
        &format!("update-index --add --cacheinfo 160000,{},submodule", commit_id.trim()),
    )?;
    let tree_id = gix::ObjectId::from_hex(git(dir.path(), "write-tree")?.trim().as_bytes())?;
    Ok((dir, tree_id))
}

#[test]
fn revision_resolution_peels_head_ref_oid_and_annotated_tag_without_loading_tree() -> Result {
    let (source, tree_id) = fixture()?;
    git(
        source.path(),
        "-c user.name=Test -c user.email=test@example.test commit -m snapshot",
    )?;
    git(
        source.path(),
        "-c user.name=Test -c user.email=test@example.test tag -a snapshot -m snapshot",
    )?;
    git(source.path(), "branch feature@foo")?;
    let branch = git(source.path(), "symbolic-ref HEAD")?;
    let commit = git(source.path(), "rev-parse HEAD")?;
    let repo = open(source.path())?;
    let limits = TreeLimits::default();
    for revision in [
        "HEAD".to_string(),
        branch.trim().to_string(),
        commit.trim().to_string(),
        tree_id.to_string(),
        "refs/tags/snapshot".to_string(),
        "refs/heads/feature@foo".to_string(),
    ] {
        assert_eq!(resolve_tree_revision(&repo, &revision, &limits)?, tree_id);
    }
    let error = resolve_tree_revision(&repo, "HEAD^{tree}", &limits).unwrap_err();
    assert!(error.to_string().contains("simple ref"));
    Ok(())
}

#[test]
fn sha256_git_tree_restores_exact_oid_in_fresh_sha256_repository() -> Result {
    let (source, tree_id) = fixture_with_format("sha256")?;
    assert_eq!(tree_id.kind(), gix::hash::Kind::Sha256);
    let source_repo = open(source.path())?;
    let storage = MemoryStorage::default();
    let limits = TreeLimits::default();
    let sealed = futures_lite::future::block_on(seal_git_tree(&source_repo, tree_id, &storage, &limits))?;
    assert_eq!(sealed.git_tree_id, tree_id);

    let destination = tempfile::tempdir()?;
    git(destination.path(), "init --bare --object-format=sha256")?;
    let destination_repo = open(destination.path())?;
    assert_eq!(destination_repo.object_hash(), gix::hash::Kind::Sha256);
    let restored = futures_lite::future::block_on(restore_git_tree(
        &destination_repo,
        &sealed.root,
        tree_id,
        &storage,
        &limits,
    ))?;
    assert_eq!(restored, tree_id);
    assert_eq!(
        source_repo.find_tree(tree_id)?.data,
        destination_repo.find_tree(restored)?.data
    );
    assert_eq!(
        git(destination.path(), &format!("cat-file -t {restored}"))?.trim(),
        "tree"
    );
    let listing = git_command(destination.path())
        .args(["ls-tree", "-z", &restored.to_string()])
        .output()?;
    assert!(listing.status.success(), "stock Git could not read SHA-256 tree");
    assert!(listing
        .stdout
        .windows(b"160000 commit".len())
        .any(|w| w == b"160000 commit"));
    Ok(())
}

#[test]
fn committed_tree_restores_exact_git_oid_with_binary_modes_and_gitlink() -> Result {
    let (source, tree_id) = fixture()?;
    let source_repo = open(source.path())?;
    let storage = MemoryStorage::default();
    let limits = TreeLimits::default();
    let sealed = futures_lite::future::block_on(seal_git_tree(&source_repo, tree_id, &storage, &limits))?;
    assert_eq!(sealed.git_tree_id, tree_id);

    let destination = tempfile::tempdir()?;
    git(destination.path(), "init --bare")?;
    let destination_repo = open(destination.path())?;
    let restored = futures_lite::future::block_on(restore_git_tree(
        &destination_repo,
        &sealed.root,
        tree_id,
        &storage,
        &limits,
    ))?;
    assert_eq!(restored, tree_id);
    let source_tree = source_repo.find_tree(tree_id)?;
    let restored_tree = destination_repo.find_tree(restored)?;
    assert_eq!(
        source_tree.data, restored_tree.data,
        "Git tree bytes must match, including names and modes"
    );
    assert_eq!(
        git(destination.path(), &format!("cat-file -t {restored}"))?.trim(),
        "tree"
    );
    let listing = git_command(destination.path())
        .args(["ls-tree", "-z", &restored.to_string()])
        .output()?;
    assert!(listing.status.success(), "stock Git could not read restored tree");
    #[cfg(unix)]
    assert!(listing
        .stdout
        .windows(b"100755 blob".len())
        .any(|w| w == b"100755 blob"));
    #[cfg(unix)]
    assert!(listing
        .stdout
        .windows(b"120000 blob".len())
        .any(|w| w == b"120000 blob"));
    assert!(listing
        .stdout
        .windows(b"160000 commit".len())
        .any(|w| w == b"160000 commit"));
    #[cfg(unix)]
    assert!(listing.stdout.windows(b"n\xffm".len()).any(|w| w == b"n\xffm"));
    Ok(())
}

#[test]
fn corrupted_closure_is_rejected_before_ref_or_index_changes() -> Result {
    let (source, tree_id) = fixture()?;
    let source_repo = open(source.path())?;
    let storage = MemoryStorage::default();
    let limits = TreeLimits::default();
    let sealed = futures_lite::future::block_on(seal_git_tree(&source_repo, tree_id, &storage, &limits))?;
    let leaf = BlobCid::compute_raw(b"base\n");
    storage.corrupt(&leaf);

    let destination = tempfile::tempdir()?;
    git(destination.path(), "init --bare")?;
    let head_before = fs::read(destination.path().join("HEAD"))?;
    let destination_repo = open(destination.path())?;
    let result = futures_lite::future::block_on(restore_git_tree(
        &destination_repo,
        &sealed.root,
        tree_id,
        &storage,
        &limits,
    ));
    assert!(result.is_err(), "corrupt leaf must fail the closure load");
    assert_eq!(head_before, fs::read(destination.path().join("HEAD"))?);
    assert!(!destination.path().join("index").exists());
    assert!(
        destination_repo.find_tree(tree_id).is_err(),
        "no Git tree was written before verification"
    );
    Ok(())
}

#[test]
fn linked_worktree_seals_the_committed_tree_not_its_dirty_files() -> Result {
    let (source, _) = fixture()?;
    git(
        source.path(),
        "-c user.name=Test -c user.email=test@example.test commit -m snapshot",
    )?;
    let worktree_parent = tempfile::tempdir()?;
    let worktree = worktree_parent.path().join("linked");
    let output = git_command(source.path())
        .arg("worktree")
        .arg("add")
        .arg("--detach")
        .arg(&worktree)
        .arg("HEAD")
        .output()?;
    assert!(
        output.status.success(),
        "create worktree: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    fs::write(worktree.join("base"), b"dirty but uncommitted\n")?;
    let linked_repo = open(&worktree)?;
    let tree_id = linked_repo.head_tree_id()?.detach();
    let storage = MemoryStorage::default();
    let sealed =
        futures_lite::future::block_on(seal_git_tree(&linked_repo, tree_id, &storage, &TreeLimits::default()))?;
    assert_eq!(sealed.git_tree_id, tree_id);
    assert!(storage
        .0
        .lock()
        .expect("storage lock")
        .contains_key(&BlobCid::compute_raw(b"base\n")));
    assert!(!storage
        .0
        .lock()
        .expect("storage lock")
        .contains_key(&BlobCid::compute_raw(b"dirty but uncommitted\n")));
    Ok(())
}

#[test]
fn empty_git_subtree_survives_the_roundtrip() -> Result {
    let source = tempfile::tempdir()?;
    git(source.path(), "init")?;
    let source_repo = open(source.path())?;
    let empty_id = source_repo.write_object(gix::objs::Tree::empty())?.detach();
    let tree_id = source_repo
        .write_object(gix::objs::Tree {
            entries: vec![gix::objs::tree::Entry {
                mode: gix::objs::tree::EntryMode::try_from(0o040000u32).expect("tree mode"),
                filename: b"empty".as_slice().into(),
                oid: empty_id,
            }],
        })?
        .detach();
    assert_eq!(git(source.path(), &format!("cat-file -t {empty_id}"))?.trim(), "tree");

    let storage = MemoryStorage::default();
    let limits = TreeLimits::default();
    let sealed = futures_lite::future::block_on(seal_git_tree(&source_repo, tree_id, &storage, &limits))?;
    let destination = tempfile::tempdir()?;
    git(destination.path(), "init --bare")?;
    let destination_repo = open(destination.path())?;
    futures_lite::future::block_on(restore_git_tree(
        &destination_repo,
        &sealed.root,
        tree_id,
        &storage,
        &limits,
    ))?;
    assert!(destination_repo.find_tree(empty_id).is_ok());
    Ok(())
}

#[test]
fn unusual_git_file_mode_is_refused_instead_of_normalized() -> Result {
    let source = tempfile::tempdir()?;
    git(source.path(), "init")?;
    let repo = open(source.path())?;
    let blob_id = repo.write_blob(b"x")?.detach();
    let tree_id = repo
        .write_object(gix::objs::Tree {
            entries: vec![gix::objs::tree::Entry {
                mode: gix::objs::tree::EntryMode::try_from(0o100664u32).expect("representable unusual mode"),
                filename: b"odd".as_slice().into(),
                oid: blob_id,
            }],
        })?
        .detach();
    let storage = MemoryStorage::default();
    let result = futures_lite::future::block_on(seal_git_tree(&repo, tree_id, &storage, &TreeLimits::default()));
    assert!(result.is_err(), "unsupported mode must not become 100644 or 100755");
    Ok(())
}

#[test]
fn byte_budget_refuses_before_writing_canonical_storage() -> Result {
    let source = tempfile::tempdir()?;
    git(source.path(), "init")?;
    let repo = open(source.path())?;
    let blob_id = repo.write_blob(b"exceeds tiny budget")?.detach();
    let tree_id = repo
        .write_object(gix::objs::Tree {
            entries: vec![gix::objs::tree::Entry {
                mode: gix::objs::tree::EntryMode::try_from(0o100644u32).expect("file mode"),
                filename: b"file".as_slice().into(),
                oid: blob_id,
            }],
        })?
        .detach();
    let storage = MemoryStorage::default();
    let limits = TreeLimits {
        max_total_bytes: 1,
        ..TreeLimits::default()
    };
    let error = futures_lite::future::block_on(seal_git_tree(&repo, tree_id, &storage, &limits)).unwrap_err();
    assert!(error.to_string().contains("aggregate byte budget before body load"));
    assert!(
        storage.0.lock().expect("storage lock").is_empty(),
        "budget failure must precede content writes"
    );
    Ok(())
}

#[test]
fn oversized_git_blob_header_refuses_before_body_load_or_storage_write() -> Result {
    let source = tempfile::tempdir()?;
    git(source.path(), "init")?;
    let repo = open(source.path())?;
    let blob_id = repo.write_blob(vec![b'x'; 256 * 1024])?.detach();
    let tree_id = repo
        .write_object(gix::objs::Tree {
            entries: vec![gix::objs::tree::Entry {
                mode: gix::objs::tree::EntryMode::try_from(0o100644u32).expect("file mode"),
                filename: b"large".as_slice().into(),
                oid: blob_id,
            }],
        })?
        .detach();
    let storage = MemoryStorage::default();
    let limits = TreeLimits {
        max_total_bytes: 64 * 1024,
        ..TreeLimits::default()
    };
    let error = futures_lite::future::block_on(seal_git_tree(&repo, tree_id, &storage, &limits)).unwrap_err();
    assert!(
        error.to_string().contains("aggregate byte budget before body load"),
        "unexpected refusal: {error:#}"
    );
    assert!(storage.0.lock().expect("storage lock").is_empty());
    Ok(())
}

#[test]
fn wrong_expected_git_tree_refuses_before_object_writes() -> Result {
    let (source, tree_id) = fixture()?;
    let source_repo = open(source.path())?;
    let storage = MemoryStorage::default();
    let limits = TreeLimits::default();
    let sealed = futures_lite::future::block_on(seal_git_tree(&source_repo, tree_id, &storage, &limits))?;
    let destination = tempfile::tempdir()?;
    git(destination.path(), "init --bare")?;
    let destination_repo = open(destination.path())?;
    let wrong = destination_repo.write_object(gix::objs::Tree::empty())?.detach();
    assert_ne!(wrong, tree_id);
    let result = futures_lite::future::block_on(restore_git_tree(
        &destination_repo,
        &sealed.root,
        wrong,
        &storage,
        &limits,
    ));
    assert!(result.is_err());
    assert!(destination_repo.find_tree(tree_id).is_err());
    Ok(())
}
