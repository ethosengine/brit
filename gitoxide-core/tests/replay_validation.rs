//! Complete-history validation with shared content and malformed type aliases.
use anyhow::{Result, ensure};
use gitoxide_core::repository::replay::Selection;
use gix::{
    bstr::ByteSlice,
    objs::{
        Tree,
        tree::{Entry, EntryKind},
    },
};

fn fixture() -> Result<(tempfile::TempDir, gix::Repository)> {
    let temp = tempfile::tempdir()?;
    let output = gix_testtools::git_command(temp.path())
        .args(["init", "-q", "-b", "main"])
        .output()?;
    ensure!(output.status.success(), "fixture init failed");
    for (name, value) in [
        ("user.name", "Validation Author"),
        ("user.email", "validation@example.invalid"),
    ] {
        let output = gix_testtools::git_command(temp.path())
            .args(["config", name, value])
            .output()?;
        ensure!(output.status.success(), "fixture configuration failed");
    }
    let repo = gix::open(temp.path())?;
    Ok((temp, repo))
}

#[test]
fn repeated_blob_histories_keep_every_unique_object() -> Result<()> {
    if gix_testtools::run_in_isolated_process().map_err(anyhow::Error::from_boxed)? {
        return Ok(());
    }
    for revisions in [8, 32] {
        let (_temp, repo) = fixture()?;
        let blob_id = repo.write_blob(vec![b'x'; 256 * 1024])?.detach();
        let mut parent = None;
        let mut expected = std::collections::BTreeSet::from([blob_id]);
        for revision in 0..revisions {
            let tree = Tree {
                entries: (0..128)
                    .map(|alias| Entry {
                        mode: EntryKind::Blob.into(),
                        filename: format!("file-{revision:03}-{alias:03}").into(),
                        oid: blob_id,
                    })
                    .collect(),
            };
            let tree_id = repo.write_object(&tree)?.detach();
            let commit_id = repo.commit("HEAD", "shared content", tree_id, parent)?.detach();
            expected.extend([tree_id, commit_id]);
            parent = Some(commit_id);
        }
        let started = std::time::Instant::now();
        let selection = Selection::prepare(&repo, b"main".as_bstr())?;
        eprintln!(
            "revisions={revisions} aliases=128 blob_bytes=262144 unique_objects={} selection_us={}",
            expected.len(),
            started.elapsed().as_micros()
        );
        assert_eq!(
            selection.closure, expected,
            "all historical objects remain verified and retained"
        );
        assert!(!repo.git_dir().join("brit-rebase").exists());
    }
    Ok(())
}

#[test]
fn a_verified_blob_cannot_be_reused_as_a_tree() -> Result<()> {
    if gix_testtools::run_in_isolated_process().map_err(anyhow::Error::from_boxed)? {
        return Ok(());
    }
    let (_temp, repo) = fixture()?;
    let blob_id = repo.write_blob(b"ordinary blob")?.detach();
    let tree_id = repo
        .write_object(&Tree {
            entries: vec![
                Entry {
                    mode: EntryKind::Blob.into(),
                    filename: "a-blob".into(),
                    oid: blob_id,
                },
                Entry {
                    mode: EntryKind::Tree.into(),
                    filename: "b-tree".into(),
                    oid: blob_id,
                },
            ],
        })?
        .detach();
    repo.commit("HEAD", "malformed type alias", tree_id, gix::commit::NO_PARENT_IDS)?;
    let error = Selection::prepare(&repo, b"main".as_bstr()).expect_err("cached identities must retain their type");
    assert!(error.to_string().contains("wrong type"), "{error:#}");
    assert!(!repo.git_dir().join("brit-rebase").exists());
    Ok(())
}

fn pinned_patch_metadata(size: usize) -> Result<()> {
    if gix_testtools::run_in_isolated_process().map_err(anyhow::Error::from_boxed)? {
        return Ok(());
    }
    let (temp, repo) = fixture()?;
    let message = format!("governance decision\n\n{}\n", "x".repeat(size));
    let mut commits = Vec::new();
    for content in [b"before\n".as_slice(), b"after\n"] {
        let tree_id = repo
            .write_object(&Tree {
                entries: vec![Entry {
                    mode: EntryKind::Blob.into(),
                    filename: "selected".into(),
                    oid: repo.write_blob(content)?.detach(),
                }],
            })?
            .detach();
        commits.push(
            repo.commit("HEAD", message.as_str(), tree_id, commits.last().copied())?
                .detach(),
        );
    }
    drop(message);
    let head = repo.head_id()?.detach();
    drop(repo);
    let expected = gix_testtools::git_command(temp.path())
        .args(["diff", "--no-ext-diff", "--no-textconv", "--no-renames", "--no-color"])
        .args(commits.iter().map(ToString::to_string))
        .output()?;
    ensure!(expected.status.success());
    ensure!(!expected.stdout.is_empty());
    for packed in [false, true] {
        if packed {
            ensure!(
                gix_testtools::git_command(temp.path())
                    .args(["-c", "pack.threads=1", "repack", "-ad"])
                    .status()?
                    .success()
            );
        }
        for limit in [None, Some(1024), Some(16 * 1024 * 1024), Some(1024 * 1024 * 1024)] {
            let repo = gix::open_opts(
                temp.path(),
                gix::open::Options::isolated()
                    .cli_overrides(limit.map(|size| format!("gitoxide.objects.allocLimit={size}"))),
            )?;
            let mut patch = Vec::new();
            gitoxide_core::repository::diff::pinned_commit(&repo, &mut patch, head)?;
            assert_eq!(patch, expected.stdout, "packed={packed}, limit={limit:?}");
            assert_eq!(repo.head_id()?.detach(), head);
            assert!(!repo.index_path().exists(), "inspection must not publish an index");
        }
    }
    Ok(())
}

#[test]
fn pinned_patch_preserves_large_commit_metadata() -> Result<()> {
    pinned_patch_metadata(128 * 1024)
}

#[test]
#[ignore = "large metadata: qualification runs this explicitly"]
fn pinned_patch_twenty_mib_commit_metadata() -> Result<()> {
    pinned_patch_metadata(20 * 1024 * 1024)
}
