//! Private retained-copy validation; no public extraction or replay-driver claim.
#![cfg(target_os = "linux")]

use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Result, ensure};
use gitoxide_core::repository::replay::retained::{self, Area, Kind, Member};
use gix::bstr::ByteSlice;

const NONCE: &str = "0123456789abcdef0123456789abcdef01234567";

fn git(root: &Path, args: &[&str]) -> Result<Vec<u8>> {
    let output = gix_testtools::git_command(root).args(args).output()?;
    ensure!(
        output.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    Ok(output.stdout)
}

fn fixture() -> Result<tempfile::TempDir> {
    let temp = tempfile::tempdir()?;
    let root = temp.path();
    git(root, &["init", "-q", "-b", "main"])?;
    git(root, &["config", "user.name", "Retained"])?;
    git(root, &["config", "user.email", "retained@example.invalid"])?;
    fs::write(root.join("file"), b"baseline\n")?;
    git(root, &["add", "file"])?;
    git(root, &["commit", "-qm", "baseline"])?;
    Ok(temp)
}

fn files(root: &Path) -> Result<BTreeMap<PathBuf, Vec<u8>>> {
    fn walk(root: &Path, dir: &Path, result: &mut BTreeMap<PathBuf, Vec<u8>>) -> Result<()> {
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            if entry.file_type()?.is_dir() {
                walk(root, &entry.path(), result)?;
            } else {
                result.insert(entry.path().strip_prefix(root)?.to_owned(), fs::read(entry.path())?);
            }
        }
        Ok(())
    }
    let mut result = BTreeMap::new();
    walk(root, root, &mut result)?;
    Ok(result)
}

fn sample_members(repo: &gix::Repository) -> Result<Vec<Member>> {
    let commit = repo.head_commit()?;
    Ok(vec![
        Member {
            area: Area::Worktree,
            path: b"raw-\xff/../literal".to_vec(),
            kind: Kind::File,
            mode: 0o100755,
            stage: None,
            data: b"private uncommitted \0 bytes".to_vec(),
        },
        Member {
            area: Area::Staged,
            path: b"link".to_vec(),
            kind: Kind::Symlink,
            mode: 0o120000,
            stage: None,
            data: b"../../outside-target".to_vec(),
        },
        Member {
            area: Area::After,
            path: b"deleted".to_vec(),
            kind: Kind::Deleted,
            mode: 0,
            stage: None,
            data: Vec::new(),
        },
        Member {
            area: Area::Indexes,
            path: b"index.original".to_vec(),
            kind: Kind::File,
            mode: 0o600,
            stage: None,
            data: fs::read(repo.index_path())?,
        },
        Member {
            area: Area::Objects,
            path: commit.id.to_string().into_bytes(),
            kind: Kind::Object {
                id: commit.id.to_string(),
                kind: "commit".into(),
            },
            mode: 0o600,
            stage: None,
            data: commit.data.clone(),
        },
    ])
}

#[test]
fn capture_roundtrips_raw_names_links_deletion_indexes_and_objects_without_odb_writes() -> Result<()> {
    if gix_testtools::run_in_isolated_process().map_err(anyhow::Error::from_boxed)? {
        return Ok(());
    }
    let temp = fixture()?;
    let repo = gix::open(temp.path())?;
    let commit = repo.head_commit()?.id;
    let members = sample_members(&repo)?;
    let odb = files(&repo.common_dir().join("objects"))?;
    let receipt = retained::capture(&repo, NONCE, 1, commit, commit, commit, &members)?;
    assert!(receipt.path.is_absolute());
    let copy = retained::inspect(&receipt.path)?;
    assert_eq!(copy.receipt.digest, receipt.digest);
    assert_eq!(copy.manifest.entries.len(), members.len());
    for (ordinal, member) in members.iter().enumerate() {
        let entry = &copy.manifest.entries[ordinal];
        assert_eq!(
            (&entry.path, &entry.kind, entry.mode),
            (&member.path, &member.kind, member.mode)
        );
        assert_eq!(copy.payload(ordinal)?, member.data);
        assert!(
            fs::symlink_metadata(receipt.path.join(format!("{ordinal:08}")))?.is_file(),
            "symlinks remain inert payload files"
        );
    }
    assert_eq!(
        files(&repo.common_dir().join("objects"))?,
        odb,
        "raw draft bytes never become Git objects"
    );
    assert!(
        !repo.git_dir().join("brit-rebase").exists(),
        "inspection needs no active journal"
    );
    let before = files(&receipt.path)?;
    let resumed = retained::capture(&repo, NONCE, 1, commit, commit, commit, &members)?;
    assert_eq!(resumed.digest, receipt.digest);
    assert_eq!(
        files(&receipt.path)?,
        before,
        "identical resume preserves every retained byte"
    );
    Ok(())
}

#[test]
fn completed_capture_refuses_changed_or_additional_sources_without_mutating_copy() -> Result<()> {
    if gix_testtools::run_in_isolated_process().map_err(anyhow::Error::from_boxed)? {
        return Ok(());
    }
    let temp = fixture()?;
    let repo = gix::open(temp.path())?;
    let commit = repo.head_commit()?.id;
    let original = sample_members(&repo)?;
    let receipt = retained::capture(&repo, NONCE, 1, commit, commit, commit, &original)?;
    let before = files(&receipt.path)?;
    let mut changed = original.clone();
    changed[0].data.push(42);
    assert!(retained::capture(&repo, NONCE, 1, commit, commit, commit, &changed).is_err());
    let mut appended = original;
    appended.push(Member {
        area: Area::Messages,
        path: b"extra".to_vec(),
        kind: Kind::File,
        mode: 0o600,
        stage: None,
        data: b"extra".to_vec(),
    });
    assert!(retained::capture(&repo, NONCE, 1, commit, commit, commit, &appended).is_err());
    assert_eq!(files(&receipt.path)?, before);
    retained::inspect(&receipt.path)?;
    Ok(())
}

#[test]
fn inspect_refuses_payload_and_directory_symlinks_without_following_them() -> Result<()> {
    if gix_testtools::run_in_isolated_process().map_err(anyhow::Error::from_boxed)? {
        return Ok(());
    }
    let temp = fixture()?;
    let repo = gix::open(temp.path())?;
    let commit = repo.head_commit()?.id;
    let members = sample_members(&repo)?;
    let receipt = retained::capture(&repo, NONCE, 1, commit, commit, commit, &members)?;
    let copy = retained::inspect(&receipt.path)?;
    let alias = temp.path().join("copy-alias");
    std::os::unix::fs::symlink(&receipt.path, &alias)?;
    assert!(retained::inspect(&alias).is_err());
    let outside = temp.path().join("outside");
    fs::write(&outside, &members[0].data)?;
    fs::remove_file(receipt.path.join("00000000"))?;
    std::os::unix::fs::symlink(&outside, receipt.path.join("00000000"))?;
    assert!(retained::inspect(&receipt.path).is_err());
    assert!(
        copy.payload(0).is_err(),
        "payload access revalidates after initial inspection"
    );
    assert_eq!(fs::read(&outside)?, members[0].data);
    Ok(())
}

#[test]
fn inspect_rejects_corruption_unknown_schema_and_incomplete_copy_receipts() -> Result<()> {
    if gix_testtools::run_in_isolated_process().map_err(anyhow::Error::from_boxed)? {
        return Ok(());
    }
    for shape in [
        "payload",
        "manifest",
        "schema",
        "missing-complete",
        "bad-complete",
        "unknown-member",
    ] {
        let temp = fixture()?;
        let repo = gix::open(temp.path())?;
        let commit = repo.head_commit()?.id;
        let receipt = retained::capture(&repo, NONCE, 1, commit, commit, commit, &sample_members(&repo)?)?;
        match shape {
            "payload" => fs::write(receipt.path.join("00000000"), b"corrupt")?,
            "manifest" => fs::write(receipt.path.join("manifest.json"), b"corrupt")?,
            "schema" => {
                let path = receipt.path.join("manifest.json");
                let mut envelope: serde_json::Value = serde_json::from_slice(&fs::read(&path)?)?;
                let payload: Vec<u8> = serde_json::from_value(envelope["payload"].clone())?;
                let mut manifest: serde_json::Value = serde_json::from_slice(&payload)?;
                manifest["version"] = 99.into();
                let payload = serde_json::to_vec(&manifest)?;
                let mut hash = gix::hash::hasher(gix::hash::Kind::Sha1);
                hash.update(&payload);
                let digest = hash.try_finalize()?.to_string();
                envelope["payload"] = serde_json::to_value(payload)?;
                envelope["checksum"] = digest.clone().into();
                fs::write(path, serde_json::to_vec(&envelope)?)?;
                fs::write(receipt.path.join("COPY_COMPLETE"), format!("{digest}\n"))?;
            }
            "missing-complete" => fs::remove_file(receipt.path.join("COPY_COMPLETE"))?,
            "bad-complete" => fs::write(receipt.path.join("COPY_COMPLETE"), b"wrong\n")?,
            "unknown-member" => fs::write(receipt.path.join("unexpected"), b"foreign")?,
            _ => unreachable!("bounded corruption cases"),
        }
        let before = files(&receipt.path)?;
        assert!(retained::inspect(&receipt.path).is_err(), "must refuse {shape}");
        assert_eq!(
            files(&receipt.path)?,
            before,
            "inspection never repairs or cleans up {shape}"
        );
    }
    Ok(())
}

#[test]
fn retained_authored_object_closure_survives_removal_from_refs_and_aggressive_gc() -> Result<()> {
    if gix_testtools::run_in_isolated_process().map_err(anyhow::Error::from_boxed)? {
        return Ok(());
    }
    let temp = fixture()?;
    let root = temp.path();
    let original = gix::open(root)?.head_commit()?.id;
    git(root, &["switch", "--detach", "-q"])?;
    fs::write(root.join("file"), b"authored retained content\n")?;
    git(root, &["add", "file"])?;
    git(root, &["commit", "-qm", "authored rewrite"])?;
    let repo = gix::open(root)?;
    let authored = repo.head_commit()?.id;
    let objects = git(root, &["rev-list", "--objects", "--no-object-names", "HEAD"])?;
    let members = objects
        .split(|byte| *byte == b'\n')
        .filter(|line| !line.is_empty())
        .map(|line| {
            let object = repo.find_object(gix::ObjectId::from_hex(line)?)?;
            Ok(Member {
                area: Area::Objects,
                path: line.to_vec(),
                kind: Kind::Object {
                    id: object.id.to_string(),
                    kind: object.kind.to_string(),
                },
                mode: 0o600,
                stage: None,
                data: object.data.clone(),
            })
        })
        .collect::<Result<Vec<_>>>()?;
    let receipt = retained::capture(&repo, NONCE, 1, original, authored, authored, &members)?;
    git(root, &["switch", "-q", "main"])?;
    git(root, &["reflog", "expire", "--expire=now", "--all"])?;
    git(root, &["gc", "--prune=now"])?;
    assert!(
        gix::open(root)?.find_object(authored).is_err(),
        "fixture proves authored commit is no longer in the live object database"
    );
    let copy = retained::inspect(&receipt.path)?;
    for (ordinal, member) in members.iter().enumerate() {
        assert_eq!(copy.payload(ordinal)?, member.data);
    }
    assert!(
        copy.manifest
            .entries
            .iter()
            .any(|entry| entry.path.as_bstr() == authored.to_string().as_bytes().as_bstr())
    );
    Ok(())
}

#[test]
fn conflict_stages_remain_distinct_and_invalid_or_duplicate_stages_refuse() -> Result<()> {
    if gix_testtools::run_in_isolated_process().map_err(anyhow::Error::from_boxed)? {
        return Ok(());
    }
    let temp = fixture()?;
    let repo = gix::open(temp.path())?;
    let commit = repo.head_commit()?.id;
    let members = (1..=3)
        .map(|stage| Member {
            area: Area::Staged,
            path: b"conflicted".to_vec(),
            kind: Kind::File,
            mode: 0o100644,
            stage: Some(stage),
            data: vec![stage],
        })
        .collect::<Vec<_>>();
    let receipt = retained::capture(&repo, NONCE, 1, commit, commit, commit, &members)?;
    let copy = retained::inspect(&receipt.path)?;
    for (ordinal, stage) in (1..=3).enumerate() {
        assert_eq!(copy.manifest.entries[ordinal].stage, Some(stage));
        assert_eq!(copy.payload(ordinal)?, vec![stage]);
    }
    for shape in ["zero", "four", "duplicate", "wrong-area"] {
        let mut invalid = members.clone();
        match shape {
            "zero" => invalid[0].stage = Some(0),
            "four" => invalid[0].stage = Some(4),
            "duplicate" => invalid.push(invalid[0].clone()),
            "wrong-area" => invalid[0].area = Area::Worktree,
            _ => unreachable!("bounded stage cases"),
        }
        let before = files(&repo.git_dir().join("brit-rebase-retained"))?;
        assert!(
            retained::capture(&repo, NONCE, 2, commit, commit, commit, &invalid).is_err(),
            "invalid retained stage: {shape}"
        );
        assert_eq!(
            files(&repo.git_dir().join("brit-rebase-retained"))?,
            before,
            "bad stage input must not mutate existing retained evidence"
        );
    }
    Ok(())
}

#[test]
fn incomplete_capture_pins_sources_that_have_not_yet_been_copied() -> Result<()> {
    if gix_testtools::run_in_isolated_process().map_err(anyhow::Error::from_boxed)? {
        return Ok(());
    }
    let temp = fixture()?;
    let repo = gix::open(temp.path())?;
    let commit = repo.head_commit()?.id;
    let members = sample_members(&repo)?;
    let receipt = retained::capture(&repo, NONCE, 1, commit, commit, commit, &members)?;
    // Reconstruct the valid interrupted prefix: full source manifest exists,
    // but one ordinary payload and the completion decision have not persisted.
    fs::remove_file(receipt.path.join("00000000"))?;
    fs::remove_file(receipt.path.join("COPY_COMPLETE"))?;
    assert!(retained::inspect(&receipt.path).is_err());
    let before = files(&receipt.path)?;
    let mut changed = members.clone();
    changed[0].data = b"changed live source while capture was interrupted".to_vec();
    assert!(
        retained::capture(&repo, NONCE, 1, commit, commit, commit, &changed).is_err(),
        "missing destination bytes must not permit replacing pinned source intent"
    );
    assert_eq!(files(&receipt.path)?, before);
    let resumed = retained::capture(&repo, NONCE, 1, commit, commit, commit, &members)?;
    assert_eq!(resumed.digest, receipt.digest);
    assert_eq!(retained::inspect(&resumed.path)?.payload(0)?, members[0].data);
    Ok(())
}

#[test]
fn retained_directory_identity_roundtrips_without_accepting_file_payload_bytes() -> Result<()> {
    use std::os::unix::fs::MetadataExt;
    if gix_testtools::run_in_isolated_process().map_err(anyhow::Error::from_boxed)? {
        return Ok(());
    }
    let temp = fixture()?;
    let repo = gix::open(temp.path())?;
    let commit = repo.head_commit()?.id;
    let metadata = fs::metadata(temp.path())?;
    let member = Member {
        area: Area::Before,
        path: b"original-directory".to_vec(),
        kind: Kind::Directory {
            device: metadata.dev(),
            inode: metadata.ino(),
        },
        mode: 0o700,
        stage: None,
        data: Vec::new(),
    };
    let receipt = retained::capture(&repo, NONCE, 1, commit, commit, commit, std::slice::from_ref(&member))?;
    let copy = retained::inspect(&receipt.path)?;
    assert_eq!(copy.manifest.entries[0].kind, member.kind);
    assert!(copy.payload(0)?.is_empty());
    let mut invalid = member;
    invalid.data = b"directory must not masquerade as file content".to_vec();
    assert!(retained::capture(&repo, NONCE, 2, commit, commit, commit, &[invalid]).is_err());
    retained::inspect(&receipt.path)?;
    Ok(())
}
