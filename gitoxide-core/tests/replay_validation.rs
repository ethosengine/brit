//! Scoped validation with shared content and malformed type aliases: a selection
//! verifies and keeps what its chain holds above the merge base, never the history.
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
fn repeated_blob_histories_below_the_base_are_never_kept() -> Result<()> {
    if gix_testtools::run_in_isolated_process().map_err(anyhow::Error::from_boxed)? {
        return Ok(());
    }
    for revisions in [8, 32] {
        let (_temp, repo) = fixture()?;
        let blob_id = repo.write_blob(vec![b'x'; 256 * 1024])?.detach();
        let mut parent = None;
        let mut history = std::collections::BTreeSet::new();
        let mut entries = Vec::new();
        for revision in 0..revisions {
            entries = (0..128)
                .map(|alias| Entry {
                    mode: EntryKind::Blob.into(),
                    filename: format!("file-{revision:03}-{alias:03}").into(),
                    oid: blob_id,
                })
                .collect();
            let tree_id = repo
                .write_object(&Tree {
                    entries: entries.clone(),
                })?
                .detach();
            let commit_id = repo.commit("HEAD", "shared content", tree_id, parent)?.detach();
            history.extend([tree_id, commit_id]);
            parent = Some(commit_id);
        }
        let base = parent.expect("history");
        let base_tree = repo.find_commit(base)?.tree_id()?.detach();
        repo.reference(
            "refs/heads/base",
            base,
            gix::refs::transaction::PreviousValue::Any,
            "base",
        )?;
        entries.push(Entry {
            mode: EntryKind::Blob.into(),
            filename: "topic".into(),
            oid: blob_id,
        });
        entries.sort_by(|a, b| a.filename.cmp(&b.filename));
        let topic_tree = repo.write_object(&Tree { entries })?.detach();
        let topic = repo.commit("HEAD", "topic", topic_tree, [base])?.detach();
        let started = std::time::Instant::now();
        let selection = Selection::prepare(&repo, b"base".as_bstr())?;
        eprintln!(
            "revisions={revisions} aliases=128 blob_bytes=262144 history_objects={} kept={} selection_us={}",
            history.len(),
            selection.closure.len(),
            started.elapsed().as_micros()
        );
        assert_eq!(
            selection.closure,
            std::collections::BTreeSet::from([topic, topic_tree, base, base_tree, blob_id]),
            "the chain's commit, its delta over the base, and the base it compares against"
        );
        assert_eq!(selection.base, base);
        assert!(
            history
                .iter()
                .filter(|id| **id != base && **id != base_tree)
                .all(|id| !selection.closure.contains(id)),
            "no history below the base is kept"
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
    let base_tree = repo.write_object(&Tree { entries: Vec::new() })?.detach();
    let base = repo
        .commit("HEAD", "base", base_tree, gix::commit::NO_PARENT_IDS)?
        .detach();
    repo.reference(
        "refs/heads/base",
        base,
        gix::refs::transaction::PreviousValue::Any,
        "base",
    )?;
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
    repo.commit("HEAD", "malformed type alias", tree_id, [base])?;
    let error = Selection::prepare(&repo, b"base".as_bstr()).expect_err("cached identities must retain their type");
    let message = format!("{error:#}");
    assert!(
        message.contains("wrong type") || message.contains("Expected object of kind tree"),
        "{message}"
    );
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
