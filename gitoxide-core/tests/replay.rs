//! Internal replay selection contracts; public rebase remains unavailable.
#![cfg(target_os = "linux")]

use std::{collections::BTreeMap, fs, path::Path};

use anyhow::{Result, ensure};
use gitoxide_core::repository::authoring::conflict::Preservation;
use gitoxide_core::repository::replay::{Advance, Selection, SelectionKind, Session, Step};
use gix::bstr::ByteSlice;

fn git(root: &Path, args: &[&str]) -> Result<Vec<u8>> {
    let output = gix_testtools::git_command(root).args(args).output()?;
    ensure!(
        output.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    Ok(output.stdout)
}

fn commit(root: &Path, path: &str, bytes: &[u8], message: &str) -> Result<gix::ObjectId> {
    fs::write(root.join(path), bytes)?;
    git(root, &["add", "--", path])?;
    git(root, &["commit", "-qm", message])?;
    oid(root, "HEAD")
}

fn oid(root: &Path, revision: &str) -> Result<gix::ObjectId> {
    Ok(gix::ObjectId::from_hex(git(root, &["rev-parse", revision])?.trim())?)
}

fn fixture() -> Result<tempfile::TempDir> {
    let temp = tempfile::tempdir()?;
    let root = temp.path();
    git(root, &["init", "-q", "-b", "main"])?;
    git(root, &["config", "user.name", "Replay Author"])?;
    git(root, &["config", "user.email", "replay@example.invalid"])?;
    commit(root, "selected", b"base\n", "base")?;
    git(root, &["branch", "base"])?;
    Ok(temp)
}

fn snapshot(root: &Path) -> Result<BTreeMap<std::path::PathBuf, Vec<u8>>> {
    fn walk(root: &Path, path: &Path, out: &mut BTreeMap<std::path::PathBuf, Vec<u8>>) -> Result<()> {
        for entry in fs::read_dir(path)? {
            let entry = entry?;
            if entry.file_type()?.is_dir() {
                walk(root, &entry.path(), out)?;
            } else {
                out.insert(entry.path().strip_prefix(root)?.to_owned(), fs::read(entry.path())?);
            }
        }
        Ok(())
    }
    let mut out = BTreeMap::new();
    walk(root, root, &mut out)?;
    Ok(out)
}

#[test]
fn equal_and_ancestor_upstreams_are_metadata_identical_noops() -> Result<()> {
    if gix_testtools::run_in_isolated_process().map_err(anyhow::Error::from_boxed)? {
        return Ok(());
    }
    let temp = fixture()?;
    let root = temp.path();
    commit(root, "selected", b"topic\n", "topic")?;
    fs::write(root.join("selected"), b"private unstaged\n")?;
    let before = snapshot(root)?;
    for upstream in ["main", "base"] {
        let repo = gix::open(root)?;
        let selection = Selection::prepare(&repo, upstream.as_bytes().as_bstr())?;
        assert_eq!(selection.kind, SelectionKind::Noop);
        assert_eq!(selection.original, oid(root, "HEAD")?);
        assert_eq!(selection.upstream, oid(root, upstream)?);
        assert_eq!(
            snapshot(root)?,
            before,
            "no-op must not write hooks, journals, refs, logs or index"
        );
    }
    Ok(())
}

#[test]
fn fast_forward_selection_pins_target_without_publishing_it() -> Result<()> {
    if gix_testtools::run_in_isolated_process().map_err(anyhow::Error::from_boxed)? {
        return Ok(());
    }
    let temp = fixture()?;
    let root = temp.path();
    let original = oid(root, "HEAD")?;
    git(root, &["switch", "-qc", "upstream"])?;
    let upstream = commit(root, "upstream", b"upstream\n", "upstream")?;
    git(root, &["switch", "-q", "main"])?;
    let before = snapshot(root)?;
    let selection = Selection::prepare(&gix::open(root)?, b"upstream".as_bstr())?;
    assert_eq!(selection.kind, SelectionKind::FastForward);
    assert_eq!(selection.original, original);
    assert_eq!(selection.upstream, upstream);
    assert!(selection.candidates.is_empty(), "fast-forward rewrites no commits");
    assert_eq!(snapshot(root)?, before, "selection does not publish a fast-forward");
    Ok(())
}

#[test]
fn divergent_selection_is_parent_first_and_keeps_full_pinned_oids() -> Result<()> {
    if gix_testtools::run_in_isolated_process().map_err(anyhow::Error::from_boxed)? {
        return Ok(());
    }
    let temp = fixture()?;
    let root = temp.path();
    let base = oid(root, "HEAD")?;
    git(root, &["switch", "-qc", "upstream"])?;
    let upstream = commit(root, "upstream", b"upstream\n", "upstream")?;
    git(root, &["switch", "-q", "main"])?;
    let first = commit(root, "selected", b"first\n", "first")?;
    let second = commit(root, "selected", b"second\n", "second")?;
    let before = snapshot(root)?;
    let selection = Selection::prepare(&gix::open(root)?, b"upstream".as_bstr())?;
    assert_eq!(selection.kind, SelectionKind::Replay);
    assert_eq!(selection.original, second);
    assert_eq!(selection.upstream, upstream);
    assert_eq!(
        selection.attachment.as_ref().map(|name| name.as_bstr()),
        Some(b"refs/heads/main".as_bstr())
    );
    assert_eq!(
        selection
            .candidates
            .iter()
            .map(|c| (c.commit, c.parent))
            .collect::<Vec<_>>(),
        vec![(first, base), (second, first)]
    );
    assert_eq!(snapshot(root)?, before, "planning cannot mutate integration state");
    git(root, &["update-ref", "refs/heads/upstream", &base.to_string()])?;
    assert_eq!(
        selection.upstream, upstream,
        "selection retains the resolved target even when its name moves"
    );
    Ok(())
}

#[test]
fn detached_selection_preserves_absence_of_attachment() -> Result<()> {
    if gix_testtools::run_in_isolated_process().map_err(anyhow::Error::from_boxed)? {
        return Ok(());
    }
    let temp = fixture()?;
    let root = temp.path();
    git(root, &["switch", "--detach", "-q"])?;
    let selection = Selection::prepare(&gix::open(root)?, b"base".as_bstr())?;
    assert_eq!(selection.attachment, None);
    assert_eq!(selection.kind, SelectionKind::Noop);
    Ok(())
}

#[test]
fn selected_merge_refuses_without_creating_operation_state() -> Result<()> {
    if gix_testtools::run_in_isolated_process().map_err(anyhow::Error::from_boxed)? {
        return Ok(());
    }
    let temp = fixture()?;
    let root = temp.path();
    git(root, &["switch", "-qc", "side"])?;
    commit(root, "side", b"side\n", "side")?;
    git(root, &["switch", "-q", "main"])?;
    commit(root, "selected", b"main\n", "main")?;
    git(root, &["merge", "--no-ff", "-qm", "selected merge", "side"])?;
    git(root, &["switch", "-qc", "upstream", "base"])?;
    commit(root, "upstream", b"upstream\n", "upstream")?;
    git(root, &["switch", "-q", "main"])?;
    let before = snapshot(root)?;
    assert!(
        Selection::prepare(&gix::open(root)?, b"upstream".as_bstr()).is_err(),
        "selected merges must never be silently flattened"
    );
    assert_eq!(snapshot(root)?, before);
    Ok(())
}

#[test]
fn initially_empty_commits_remain_candidates() -> Result<()> {
    if gix_testtools::run_in_isolated_process().map_err(anyhow::Error::from_boxed)? {
        return Ok(());
    }
    let temp = fixture()?;
    let root = temp.path();
    let base = oid(root, "HEAD")?;
    git(root, &["switch", "-qc", "upstream"])?;
    commit(root, "upstream", b"upstream\n", "upstream")?;
    git(root, &["switch", "-q", "main"])?;
    git(root, &["commit", "--allow-empty", "-qm", "intentionally empty"])?;
    let empty = oid(root, "HEAD")?;
    let selection = Selection::prepare(&gix::open(root)?, b"upstream".as_bstr())?;
    assert_eq!(selection.kind, SelectionKind::Replay);
    assert_eq!(selection.candidates.len(), 1);
    assert_eq!(selection.candidates[0].commit, empty);
    assert_eq!(selection.candidates[0].parent, base);
    Ok(())
}

#[test]
fn invalid_upstreams_and_unprovable_shallow_history_leave_every_byte_unchanged() -> Result<()> {
    if gix_testtools::run_in_isolated_process().map_err(anyhow::Error::from_boxed)? {
        return Ok(());
    }
    let temp = fixture()?;
    let root = temp.path();
    let blob = oid(root, "HEAD:selected")?;
    for upstream in ["missing".to_owned(), blob.to_string()] {
        let before = snapshot(root)?;
        assert!(Selection::prepare(&gix::open(root)?, upstream.as_bytes().as_bstr()).is_err());
        assert_eq!(snapshot(root)?, before);
    }
    git(root, &["switch", "-qc", "upstream"])?;
    commit(root, "upstream", b"upstream\n", "upstream")?;
    git(root, &["switch", "-q", "main"])?;
    let shallow = commit(root, "selected", b"shallow\n", "shallow")?;
    fs::write(root.join(".git/shallow"), format!("{shallow}\n"))?;
    let before = snapshot(root)?;
    assert!(
        Selection::prepare(&gix::open(root)?, b"upstream".as_bstr()).is_err(),
        "a shallow boundary cannot prove the selected chain"
    );
    assert_eq!(snapshot(root)?, before);
    Ok(())
}

#[test]
fn pending_replay_in_either_worktree_reserves_the_shared_repository() -> Result<()> {
    if gix_testtools::run_in_isolated_process().map_err(anyhow::Error::from_boxed)? {
        return Ok(());
    }
    for in_linked in [false, true] {
        for dangling in [false, true] {
            let temp = fixture()?;
            let root = temp.path();
            let linked_parent = tempfile::tempdir()?;
            let linked = linked_parent.path().join("linked");
            git(
                root,
                &["worktree", "add", "--detach", linked.to_str().expect("temporary path")],
            )?;
            let main_repo = gix::open(root)?;
            let linked_repo = gix::open(&linked)?;
            // Establish the common advisory lock before comparing resources.
            drop(gitoxide_core::repository::switch::mutation_guard(&main_repo)?);
            let owner = if in_linked { &linked_repo } else { &main_repo };
            let journal = owner.git_dir().join("brit-rebase");
            if dangling {
                std::os::unix::fs::symlink("missing-replay-state", &journal)?;
            } else {
                fs::create_dir(&journal)?;
            }
            let main_head = fs::read(main_repo.git_dir().join("HEAD"))?;
            let linked_head = fs::read(linked_repo.git_dir().join("HEAD"))?;
            let main_index = fs::read(main_repo.index_path())?;
            let linked_index = fs::read(linked_repo.index_path())?;
            for repo in [&main_repo, &linked_repo] {
                assert!(gitoxide_core::repository::switch::mutation_guard(repo).is_err());
                assert!(
                    Selection::prepare(repo, b"base".as_bstr()).is_err(),
                    "even a no-op must not bypass a competing replay reservation"
                );
            }
            assert_eq!(fs::read(main_repo.git_dir().join("HEAD"))?, main_head);
            assert_eq!(fs::read(linked_repo.git_dir().join("HEAD"))?, linked_head);
            assert_eq!(fs::read(main_repo.index_path())?, main_index);
            assert_eq!(fs::read(linked_repo.index_path())?, linked_index);
            assert!(
                fs::symlink_metadata(&journal).is_ok(),
                "a pending or damaged journal is never removed by refusal"
            );
        }
    }
    Ok(())
}

#[test]
fn legacy_switch_pull_and_authoring_journals_still_block_replay_selection() -> Result<()> {
    if gix_testtools::run_in_isolated_process().map_err(anyhow::Error::from_boxed)? {
        return Ok(());
    }
    for journal in ["brit-switch", "brit-authoring"] {
        let temp = fixture()?;
        let root = temp.path();
        let repo = gix::open(root)?;
        drop(gitoxide_core::repository::switch::mutation_guard(&repo)?);
        fs::create_dir(repo.git_dir().join(journal))?;
        let before = snapshot(root)?;
        assert!(gitoxide_core::repository::switch::mutation_guard(&repo).is_err());
        assert!(Selection::prepare(&repo, b"base".as_bstr()).is_err());
        assert_eq!(
            snapshot(root)?,
            before,
            "existing operation namespaces retain their refusal behavior"
        );
    }
    Ok(())
}

#[test]
fn same_oid_attachment_changes_are_foreign_head_movement() -> Result<()> {
    if gix_testtools::run_in_isolated_process().map_err(anyhow::Error::from_boxed)? {
        return Ok(());
    }
    let temp = fixture()?;
    let root = temp.path();
    let selection = Selection::prepare(&gix::open(root)?, b"base".as_bstr())?;
    git(root, &["switch", "-qc", "same-tip"])?;
    assert_eq!(oid(root, "HEAD")?, selection.original);
    assert!(selection.validate_head(&gix::open(root)?).is_err());
    git(root, &["switch", "-q", "main"])?;
    selection.validate_head(&gix::open(root)?)?;
    git(root, &["switch", "--detach", "-q"])?;
    assert!(selection.validate_head(&gix::open(root)?).is_err());
    Ok(())
}

#[test]
fn missing_blob_in_pinned_history_refuses_before_selection_is_accepted() -> Result<()> {
    if gix_testtools::run_in_isolated_process().map_err(anyhow::Error::from_boxed)? {
        return Ok(());
    }
    let temp = fixture()?;
    let root = temp.path();
    let blob = oid(root, "base:selected")?.to_string();
    commit(root, "selected", b"new tip\n", "new tip")?;
    fs::remove_file(root.join(".git/objects").join(&blob[..2]).join(&blob[2..]))?;
    let before = snapshot(root)?;
    assert!(
        Selection::prepare(&gix::open(root)?, b"base".as_bstr()).is_err(),
        "valid commit objects alone do not prove the required history closure"
    );
    assert_eq!(snapshot(root)?, before);
    Ok(())
}

fn divergent_fixture(conflict: bool) -> Result<tempfile::TempDir> {
    let temp = fixture()?;
    let root = temp.path();
    git(root, &["switch", "-qc", "upstream"])?;
    commit(
        root,
        if conflict { "selected" } else { "upstream" },
        b"upstream\n",
        "upstream",
    )?;
    git(root, &["switch", "-q", "main"])?;
    commit(root, "selected", b"replayed\n", "replayed")?;
    Ok(temp)
}

#[test]
fn clean_native_merge_changes_only_objects_and_proposes_original_author_bytes() -> Result<()> {
    if gix_testtools::run_in_isolated_process().map_err(anyhow::Error::from_boxed)? {
        return Ok(());
    }
    let temp = divergent_fixture(false)?;
    let root = temp.path();
    let output = gix_testtools::git_command(root)
        .args(["commit", "--amend", "--no-edit", "--reset-author"])
        .env("GIT_AUTHOR_NAME", "Original Author")
        .env("GIT_AUTHOR_EMAIL", "original@example.invalid")
        .env("GIT_AUTHOR_DATE", "1234567890 -0730")
        .output()?;
    ensure!(
        output.status.success(),
        "author fixture: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let original_commit = git(root, &["cat-file", "commit", "HEAD"])?;
    let _committer = gix_testtools::Env::new()
        .set("GIT_COMMITTER_NAME", "Replay Committer")
        .set("GIT_COMMITTER_EMAIL", "committer@example.invalid")
        .set("GIT_COMMITTER_DATE", "1700000000 +0530");
    let repo = gix::open(root)?;
    let selection = Selection::prepare(&repo, b"upstream".as_bstr())?;
    let head = fs::read(repo.git_dir().join("HEAD"))?;
    let index = fs::read(repo.index_path())?;
    let selected = fs::read(root.join("selected"))?;
    let step = Step::merge(&repo, &selection.candidates[0], selection.upstream)?;
    assert!(step.conflicts.is_empty());
    assert!(!step.initially_empty);
    assert!(!step.newly_empty);
    assert_eq!(git(root, &["show", &format!("{}:selected", step.tree)])?, b"replayed\n");
    assert_eq!(git(root, &["show", &format!("{}:upstream", step.tree)])?, b"upstream\n");
    let proposal = step.propose(&repo)?;
    let author = |bytes: &[u8]| {
        bytes
            .split(|b| *b == b'\n')
            .find(|line| line.starts_with(b"author "))
            .map(<[u8]>::to_vec)
    };
    assert_eq!(
        author(&proposal.bytes),
        author(&original_commit),
        "raw author identity, timestamp and timezone survive replay"
    );
    assert!(
        proposal
            .bytes
            .windows(b"1234567890 -0730".len())
            .any(|window| window == b"1234567890 -0730")
    );
    assert_eq!(
        proposal
            .bytes
            .split(|byte| *byte == b'\n')
            .find(|line| line.starts_with(b"committer ")),
        Some(b"committer Replay Committer <committer@example.invalid> 1700000000 +0530".as_slice()),
        "rewrites use the current explicitly configured committer"
    );
    let parents: Vec<_> = proposal
        .bytes
        .split(|b| *b == b'\n')
        .filter(|line| line.starts_with(b"parent "))
        .collect();
    assert_eq!(parents, vec![format!("parent {}", selection.upstream).as_bytes()]);
    assert!(proposal.bytes.ends_with(b"\n\nreplayed\n"));
    let check = gix_testtools::git_command(root)
        .args(["cat-file", "-e", &proposal.commit.to_string()])
        .output()?;
    assert!(
        !check.status.success(),
        "proposal bytes are not yet a published commit object"
    );
    assert_eq!(fs::read(repo.git_dir().join("HEAD"))?, head);
    assert_eq!(fs::read(repo.index_path())?, index);
    assert_eq!(fs::read(root.join("selected"))?, selected);
    assert!(
        !root.join("upstream").exists(),
        "native merge does not check out its result"
    );
    Ok(())
}

#[test]
fn native_conflict_stages_are_old_parent_upstream_and_original_input() -> Result<()> {
    if gix_testtools::run_in_isolated_process().map_err(anyhow::Error::from_boxed)? {
        return Ok(());
    }
    let temp = divergent_fixture(true)?;
    let root = temp.path();
    let repo = gix::open(root)?;
    let selection = Selection::prepare(&repo, b"upstream".as_bstr())?;
    let before_index = fs::read(repo.index_path())?;
    let before_work = fs::read(root.join("selected"))?;
    let step = Step::merge(&repo, &selection.candidates[0], selection.upstream)?;
    assert!(step.conflicts.iter().any(|path| path.as_slice() == b"selected"));
    let stages: Vec<_> = step
        .index
        .entries()
        .iter()
        .filter(|entry| entry.path(&step.index) == b"selected".as_bstr())
        .map(|entry| (entry.stage_raw(), entry.id))
        .collect();
    assert_eq!(
        stages,
        vec![
            (1, oid(root, "base:selected")?),
            (2, oid(root, "upstream:selected")?),
            (3, oid(root, "main:selected")?)
        ]
    );
    assert!(
        step.propose(&repo).is_err(),
        "unresolved conflict stages cannot enter a proposal"
    );
    assert_eq!(fs::read(repo.index_path())?, before_index);
    assert_eq!(fs::read(root.join("selected"))?, before_work);
    Ok(())
}

#[test]
fn native_replay_distinguishes_initially_empty_from_newly_empty_inputs() -> Result<()> {
    if gix_testtools::run_in_isolated_process().map_err(anyhow::Error::from_boxed)? {
        return Ok(());
    }
    for initially_empty in [false, true] {
        let temp = fixture()?;
        let root = temp.path();
        git(root, &["switch", "-qc", "upstream"])?;
        commit(root, "selected", b"equivalent\n", "upstream implementation")?;
        git(root, &["switch", "-q", "main"])?;
        if initially_empty {
            git(root, &["commit", "--allow-empty", "-qm", "empty input"])?;
        } else {
            commit(root, "selected", b"equivalent\n", "independent equivalent")?;
        }
        let repo = gix::open(root)?;
        let selection = Selection::prepare(&repo, b"upstream".as_bstr())?;
        assert_eq!(selection.candidates.len(), 1, "no patch-id based predropping");
        let step = Step::merge(&repo, &selection.candidates[0], selection.upstream)?;
        assert!(step.conflicts.is_empty());
        assert_eq!(step.initially_empty, initially_empty);
        assert_eq!(step.newly_empty, !initially_empty);
        assert_eq!(step.tree, oid(root, "upstream^{tree}")?);
    }
    Ok(())
}

#[test]
fn captured_preservation_is_reused_across_later_replay_checkouts() -> Result<()> {
    if gix_testtools::run_in_isolated_process().map_err(anyhow::Error::from_boxed)? {
        return Ok(());
    }
    let temp = fixture()?;
    let root = temp.path();
    commit(root, "neighbor", b"clean neighbor\n", "neighbor baseline")?;
    git(root, &["switch", "-qc", "replayed"])?;
    commit(root, "selected", b"later replay\n", "later replay")?;
    git(root, &["switch", "-q", "main"])?;
    fs::write(root.join("neighbor"), b"staged neighbor\n")?;
    git(root, &["add", "neighbor"])?;
    fs::write(root.join("neighbor"), b"unstaged neighbor\n")?;
    let repo = gix::open(root)?;
    let guard = gitoxide_core::repository::switch::mutation_guard(&repo)?;
    let before = snapshot(root)?;
    let preservation = Preservation::capture(&repo)?;
    assert_eq!(
        snapshot(root)?,
        before,
        "capture creates no draft blobs or journal writes"
    );
    let frozen = serde_json::to_vec(&preservation)?;
    let original_index = fs::read(repo.index_path())?;
    assert_eq!(preservation.original_index(), original_index);
    assert_eq!(preservation.preserved_paths(), &[b"neighbor".to_vec()]);
    drop(guard);
    // This changes clean HEAD/index/worktree entries while keeping both dirty
    // neighbor versions. The original capture must remain the only authority.
    git(root, &["switch", "-q", "replayed"])?;
    let repo = gix::open(root)?;
    let private = repo.index_from_tree(&repo.head_commit()?.tree_id()?)?;
    let public = preservation.public_index(&repo, &private)?;
    let public_neighbor = public
        .entries()
        .iter()
        .find(|entry| entry.path(&public) == b"neighbor".as_bstr())
        .expect("neighbor exists");
    assert_eq!(repo.find_blob(public_neighbor.id)?.data, b"staged neighbor\n");
    let private_neighbor = private
        .entries()
        .iter()
        .find(|entry| entry.path(&private) == b"neighbor".as_bstr())
        .expect("neighbor exists");
    assert_eq!(repo.find_blob(private_neighbor.id)?.data, b"clean neighbor\n");
    assert_eq!(fs::read(root.join("neighbor"))?, b"unstaged neighbor\n");
    assert_eq!(fs::read(root.join("selected"))?, b"later replay\n");
    assert_eq!(
        serde_json::to_vec(&preservation)?,
        frozen,
        "later checkouts never redefine preserved paths or original images"
    );
    assert_eq!(preservation.original_index(), original_index);
    Ok(())
}

#[test]
fn public_overlay_refuses_a_private_index_containing_preserved_staging() -> Result<()> {
    if gix_testtools::run_in_isolated_process().map_err(anyhow::Error::from_boxed)? {
        return Ok(());
    }
    let temp = fixture()?;
    let root = temp.path();
    commit(root, "neighbor", b"clean neighbor\n", "neighbor baseline")?;
    fs::write(root.join("neighbor"), b"staged private bytes\n")?;
    git(root, &["add", "neighbor"])?;
    fs::write(root.join("neighbor"), b"unstaged private bytes\n")?;
    let repo = gix::open(root)?;
    let _guard = gitoxide_core::repository::switch::mutation_guard(&repo)?;
    let preservation = Preservation::capture(&repo)?;
    let private = repo.index_from_tree(&repo.head_commit()?.tree_id()?)?;
    let public = preservation.public_index(&repo, &private)?;
    let before = snapshot(root)?;
    assert!(
        preservation.public_index(&repo, &public).is_err(),
        "using public staging as a private replay index must refuse before a rewritten tree is produced"
    );
    assert_eq!(snapshot(root)?, before, "refusal leaves both preserved versions intact");
    Ok(())
}

#[test]
fn frozen_preservation_rejects_equal_ancestor_and_descendant_scope_expansion() -> Result<()> {
    if gix_testtools::run_in_isolated_process().map_err(anyhow::Error::from_boxed)? {
        return Ok(());
    }
    let temp = fixture()?;
    let root = temp.path();
    fs::create_dir(root.join("nested"))?;
    commit(root, "nested/neighbor", b"clean\n", "nested baseline")?;
    fs::write(root.join("nested/neighbor"), b"private\n")?;
    let repo = gix::open(root)?;
    let _guard = gitoxide_core::repository::switch::mutation_guard(&repo)?;
    let preservation = Preservation::capture(&repo)?;
    let before = snapshot(root)?;
    for path in ["nested", "nested/neighbor", "nested/neighbor/child"] {
        assert!(
            preservation.validate_scope(&[path.as_bytes().to_vec()]).is_err(),
            "preserved scope collision: {path}"
        );
    }
    preservation.validate_scope(&[b"nested/sibling".to_vec(), b"nested-neighbor".to_vec()])?;
    assert_eq!(snapshot(root)?, before);
    Ok(())
}

#[test]
fn public_overlay_rejects_private_descendants_of_preserved_additions_and_deletions() -> Result<()> {
    if gix_testtools::run_in_isolated_process().map_err(anyhow::Error::from_boxed)? {
        return Ok(());
    }
    for deletion in [false, true] {
        let temp = fixture()?;
        let root = temp.path();
        if deletion {
            commit(root, "a", b"original a\n", "baseline a")?;
            fs::remove_file(root.join("a"))?;
            git(root, &["add", "-u", "--", "a"])?;
        } else {
            fs::write(root.join("a"), b"preserved addition\n")?;
            git(root, &["add", "--", "a"])?;
        }
        let repo = gix::open(root)?;
        let _guard = gitoxide_core::repository::switch::mutation_guard(&repo)?;
        let preservation = Preservation::capture(&repo)?;
        let mut private = repo.index_from_tree(&repo.head_commit()?.tree_id()?)?;
        let blob = repo.write_blob(b"replay descendant\n")?;
        private.dangerously_push_entry(
            Default::default(),
            blob.detach(),
            gix::index::entry::Flags::empty(),
            gix::index::entry::Mode::FILE,
            b"a/b".as_bstr(),
        );
        private.sort_entries();
        let before = snapshot(root)?;
        assert!(
            preservation.public_index(&repo, &private).is_err(),
            "private descendant must not collide with preserved {}",
            if deletion { "deletion" } else { "addition" }
        );
        assert_eq!(snapshot(root)?, before);
    }
    Ok(())
}

#[test]
fn upstream_names_are_unambiguous_refs_or_full_commit_oids_never_revspecs() -> Result<()> {
    if gix_testtools::run_in_isolated_process().map_err(anyhow::Error::from_boxed)? {
        return Ok(());
    }
    let temp = fixture()?;
    let root = temp.path();
    let base = oid(root, "base")?;
    commit(root, "selected", b"tip\n", "tip")?;
    git(root, &["tag", "base", "main"])?;
    git(root, &["tag", "-am", "annotated", "annotated", "main"])?;
    git(root, &["update-ref", "refs/remotes/origin/local", &base.to_string()])?;
    let before = snapshot(root)?;
    let repo = gix::open(root)?;
    for argument in [
        "base".to_owned(),
        "HEAD".to_owned(),
        "main~1".to_owned(),
        "main^".to_owned(),
        "main^{commit}".to_owned(),
        "main@{0}".to_owned(),
        "main:selected".to_owned(),
        base.to_string()[..8].to_owned(),
    ] {
        assert!(
            Selection::prepare(&repo, argument.as_bytes().as_bstr()).is_err(),
            "unsupported or ambiguous upstream must refuse: {argument}"
        );
    }
    for (argument, expected) in [
        ("refs/heads/base".to_owned(), base),
        ("refs/tags/base".to_owned(), oid(root, "main")?),
        ("annotated".to_owned(), oid(root, "main")?),
        ("origin/local".to_owned(), base),
        (base.to_string(), base),
    ] {
        assert_eq!(
            Selection::prepare(&repo, argument.as_bytes().as_bstr())?.upstream,
            expected
        );
    }
    assert_eq!(snapshot(root)?, before);
    Ok(())
}

#[test]
fn valid_loose_object_payload_under_wrong_hash_refuses_history_closure() -> Result<()> {
    if gix_testtools::run_in_isolated_process().map_err(anyhow::Error::from_boxed)? {
        return Ok(());
    }
    let temp = fixture()?;
    let root = temp.path();
    let original = oid(root, "base:selected")?.to_string();
    let repo = gix::open(root)?;
    let substituted = repo
        .write_blob(b"a valid blob stored under a foreign object ID\n")?
        .to_string();
    let path = |object: &str| root.join(".git/objects").join(&object[..2]).join(&object[2..]);
    fs::remove_file(path(&original))?;
    fs::copy(path(&substituted), path(&original))?;
    drop(repo);
    let repo = gix::open(root)?;
    assert_eq!(
        repo.find_blob(gix::ObjectId::from_hex(original.as_bytes())?)?.data,
        b"a valid blob stored under a foreign object ID\n",
        "fixture remains decodable; only the content hash is corrupt"
    );
    let before = snapshot(root)?;
    let error = Selection::prepare(&repo, b"base".as_bstr()).expect_err("corrupt closure must refuse");
    assert!(
        error.to_string().contains("checksum"),
        "refusal must identify object hash verification: {error:#}"
    );
    assert_eq!(snapshot(root)?, before);
    Ok(())
}

#[test]
fn external_merge_driver_is_refused_before_its_sentinel_can_execute() -> Result<()> {
    if gix_testtools::run_in_isolated_process().map_err(anyhow::Error::from_boxed)? {
        return Ok(());
    }
    let temp = divergent_fixture(true)?;
    let root = temp.path();
    fs::write(root.join(".gitattributes"), b"selected merge=sentinel\n")?;
    git(
        root,
        &[
            "config",
            "merge.sentinel.driver",
            "printf invoked > merge-driver-sentinel",
        ],
    )?;
    let repo = gix::open(root)?;
    let before = snapshot(root)?;
    let error = Selection::prepare(&repo, b"upstream".as_bstr())
        .expect_err("configured external merge driver must refuse during preflight");
    assert!(
        error.to_string().contains("external merge"),
        "refusal identifies the unsupported driver: {error:#}"
    );
    assert!(
        !root.join("merge-driver-sentinel").exists(),
        "replay must not run external merge drivers"
    );
    assert_eq!(
        snapshot(root)?,
        before,
        "driver refusal occurs before object/index/worktree mutation"
    );
    Ok(())
}

fn integration_snapshot(root: &Path) -> Result<BTreeMap<String, Option<Vec<u8>>>> {
    [
        ".git/HEAD",
        ".git/index",
        ".git/ORIG_HEAD",
        ".git/refs/heads/main",
        ".git/logs/HEAD",
        ".git/logs/refs/heads/main",
        "selected",
        "neighbor",
    ]
    .into_iter()
    .map(|path| {
        let bytes = match fs::read(root.join(path)) {
            Ok(bytes) => Some(bytes),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
            Err(error) => return Err(error.into()),
        };
        Ok((path.to_owned(), bytes))
    })
    .collect()
}

#[test]
fn prepared_session_preserves_integration_bytes_and_cancels_without_restoration() -> Result<()> {
    if gix_testtools::run_in_isolated_process().map_err(anyhow::Error::from_boxed)? {
        return Ok(());
    }
    let temp = fixture()?;
    let root = temp.path();
    commit(root, "neighbor", b"original neighbor\n", "neighbor")?;
    git(root, &["switch", "-qc", "new-upstream"])?;
    commit(root, "remote", b"remote\n", "remote")?;
    git(root, &["switch", "-q", "main"])?;
    commit(root, "selected", b"final topic\n", "final topic")?;
    fs::write(root.join("neighbor"), b"staged neighbor\n")?;
    git(root, &["add", "neighbor"])?;
    fs::write(root.join("neighbor"), b"unstaged neighbor\n")?;
    let before = integration_snapshot(root)?;
    let repo = gix::open(root)?;
    let session = Session::prepare(&repo, b"new-upstream".as_bstr())?.expect("divergence needs a session");
    assert_eq!(
        integration_snapshot(root)?,
        before,
        "preparation must not detach HEAD, publish or change staging"
    );
    assert!(gitoxide_core::repository::switch::mutation_guard(&repo).is_err());
    assert!(Session::prepare(&repo, b"new-upstream".as_bstr()).is_err());
    let reopened = Session::open(&repo)?;
    assert_eq!(reopened.directory(), session.directory());
    reopened.abort_prepared(&repo)?;
    assert_eq!(
        integration_snapshot(root)?,
        before,
        "unpublished cancellation keeps exact original staged/unstaged distinction"
    );
    assert!(!session.directory().exists());
    assert!(
        git(
            root,
            &["for-each-ref", "--format=%(refname)", "refs/brit-rebase-retain/"]
        )?
        .is_empty()
    );
    drop(gitoxide_core::repository::switch::mutation_guard(&repo)?);
    Ok(())
}

#[test]
fn prepared_retention_keeps_unnamed_upstream_objects_alive_under_aggressive_gc() -> Result<()> {
    if gix_testtools::run_in_isolated_process().map_err(anyhow::Error::from_boxed)? {
        return Ok(());
    }
    let temp = divergent_fixture(false)?;
    let root = temp.path();
    let upstream = oid(root, "upstream")?;
    let original = oid(root, "main")?;
    let repo = gix::open(root)?;
    let session = Session::prepare(&repo, b"upstream".as_bstr())?.expect("divergence needs a session");
    let refs = git(
        root,
        &["for-each-ref", "--format=%(objectname)", "refs/brit-rebase-retain/"],
    )?;
    let refs: Vec<_> = refs
        .split(|byte| *byte == b'\n')
        .filter(|line| !line.is_empty())
        .collect();
    assert_eq!(
        refs.len(),
        2,
        "both original and upstream have operation-owned retention refs"
    );
    assert!(refs.contains(&original.to_string().as_bytes()));
    assert!(refs.contains(&upstream.to_string().as_bytes()));
    git(root, &["update-ref", "-d", "refs/heads/upstream"])?;
    git(root, &["reflog", "expire", "--expire=now", "--all"])?;
    git(root, &["gc", "--prune=now"])?;
    assert_eq!(git(root, &["show", &format!("{upstream}:upstream")])?, b"upstream\n");
    Session::open(&gix::open(root)?)?;
    session.abort_prepared(&gix::open(root)?)?;
    assert!(!session.directory().exists());
    assert_eq!(oid(root, "main")?, original);
    Ok(())
}

#[test]
fn noop_session_preparation_creates_no_journal_retention_ref_or_lock() -> Result<()> {
    if gix_testtools::run_in_isolated_process().map_err(anyhow::Error::from_boxed)? {
        return Ok(());
    }
    let temp = fixture()?;
    let root = temp.path();
    let before = snapshot(root)?;
    assert!(Session::prepare(&gix::open(root)?, b"base".as_bstr())?.is_none());
    assert_eq!(snapshot(root)?, before);
    Ok(())
}

#[test]
fn corrupt_prepared_journal_refuses_without_cleaning_up_evidence() -> Result<()> {
    if gix_testtools::run_in_isolated_process().map_err(anyhow::Error::from_boxed)? {
        return Ok(());
    }
    let temp = divergent_fixture(false)?;
    let root = temp.path();
    let repo = gix::open(root)?;
    let session = Session::prepare(&repo, b"upstream".as_bstr())?.expect("divergence needs a session");
    let path = session.directory().join("state.json");
    let mut envelope: serde_json::Value = serde_json::from_slice(&fs::read(&path)?)?;
    envelope["checksum"] = serde_json::Value::String("0".repeat(40));
    fs::write(&path, serde_json::to_vec(&envelope)?)?;
    let before = snapshot(root)?;
    assert!(Session::open(&repo).is_err());
    assert!(session.abort_prepared(&repo).is_err());
    assert_eq!(snapshot(root)?, before, "corrupt ownership state is inspection-only");
    Ok(())
}

#[test]
fn copied_prepared_journal_cannot_claim_a_linked_worktree() -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    if gix_testtools::run_in_isolated_process().map_err(anyhow::Error::from_boxed)? {
        return Ok(());
    }
    let temp = divergent_fixture(false)?;
    let root = temp.path();
    let linked_parent = tempfile::tempdir()?;
    let linked = linked_parent.path().join("linked");
    git(
        root,
        &["worktree", "add", "--detach", linked.to_str().expect("temporary path")],
    )?;
    let repo = gix::open(root)?;
    let session = Session::prepare(&repo, b"upstream".as_bstr())?.expect("divergence needs a session");
    let linked_repo = gix::open(&linked)?;
    let foreign = linked_repo.git_dir().join("brit-rebase");
    fs::create_dir(&foreign)?;
    fs::set_permissions(&foreign, fs::Permissions::from_mode(0o700))?;
    fs::copy(session.directory().join("state.json"), foreign.join("state.json"))?;
    fs::set_permissions(foreign.join("state.json"), fs::Permissions::from_mode(0o600))?;
    let before = fs::read(foreign.join("state.json"))?;
    assert!(
        Session::open(&linked_repo).is_err(),
        "valid checksum cannot confer ownership of a different worktree"
    );
    assert!(session.abort_prepared(&linked_repo).is_err());
    assert_eq!(fs::read(foreign.join("state.json"))?, before);
    assert!(session.directory().is_dir());
    Ok(())
}

#[test]
fn abort_prepared_replay_keeps_an_independent_checked_copy_and_original_integration() -> Result<()> {
    use gitoxide_core::repository::replay::retained::{self, Area};
    if gix_testtools::run_in_isolated_process().map_err(anyhow::Error::from_boxed)? {
        return Ok(());
    }
    let temp = divergent_fixture(false)?;
    let root = temp.path();
    let before = integration_snapshot(root)?;
    let repo = gix::open(root)?;
    let original_index = fs::read(repo.index_path())?;
    let mut session = Session::prepare(&repo, b"upstream".as_bstr())?.expect("divergence");
    let receipt = session.abort(&repo)?;
    assert_eq!(integration_snapshot(root)?, before);
    let copy = retained::inspect(&receipt.path)?;
    let index = copy
        .manifest
        .entries
        .iter()
        .position(|entry| entry.area == Area::Indexes && entry.path == b"original")
        .expect("exact original index retained");
    assert_eq!(copy.payload(index)?, original_index);
    assert_eq!(copy.receipt.digest, receipt.digest);
    assert!(
        !receipt.path.starts_with(session.directory()),
        "independent copy cannot be removed with the active journal"
    );
    let copy_before = snapshot(&receipt.path)?;
    Session::open(&repo)?;
    assert_eq!(snapshot(&receipt.path)?, copy_before);
    Ok(())
}

#[test]
fn abort_refuses_foreign_same_oid_attachment_without_overwriting_it() -> Result<()> {
    if gix_testtools::run_in_isolated_process().map_err(anyhow::Error::from_boxed)? {
        return Ok(());
    }
    let temp = divergent_fixture(false)?;
    let root = temp.path();
    let repo = gix::open(root)?;
    let mut session = Session::prepare(&repo, b"upstream".as_bstr())?.expect("divergence");
    let original = oid(root, "HEAD")?;
    git(root, &["switch", "-qc", "foreign-attachment"])?;
    assert_eq!(oid(root, "HEAD")?, original);
    let before = snapshot(root)?;
    assert!(session.abort(&gix::open(root)?).is_err());
    assert_eq!(
        snapshot(root)?,
        before,
        "same-OID attachment change cannot be rolled back as owned replay state"
    );
    Ok(())
}

fn preserved_linear_fixture(conflict: bool) -> Result<tempfile::TempDir> {
    let temp = fixture()?;
    let root = temp.path();
    commit(root, "neighbor", b"clean neighbor\n", "shared neighbor")?;
    git(root, &["switch", "-qc", "upstream"])?;
    commit(
        root,
        if conflict { "selected" } else { "upstream" },
        b"upstream\n",
        "upstream",
    )?;
    git(root, &["switch", "-q", "main"])?;
    commit(root, "selected", b"first replay\n", "first replay")?;
    commit(root, "selected", b"second replay\n", "second replay")?;
    fs::write(root.join("neighbor"), b"staged private neighbor\n")?;
    git(root, &["add", "neighbor"])?;
    fs::write(root.join("neighbor"), b"unstaged private neighbor\n")?;
    Ok(temp)
}

#[test]
fn abort_after_each_completed_step_restores_exact_index_and_retains_rewritten_objects() -> Result<()> {
    use gitoxide_core::repository::replay::retained::{self, Area};
    if gix_testtools::run_in_isolated_process().map_err(anyhow::Error::from_boxed)? {
        return Ok(());
    }
    for steps in 0..=2 {
        let temp = preserved_linear_fixture(false)?;
        let root = temp.path();
        let before = integration_snapshot(root)?;
        let original = oid(root, "main")?;
        let mut session = Session::prepare(&gix::open(root)?, b"upstream".as_bstr())?.expect("divergence");
        assert_eq!(session.advance(&gix::open(root)?)?, Advance::Started);
        assert!(
            gix::open(root)?.head_name()?.is_none(),
            "replay detaches without publishing the original branch"
        );
        let mut rewritten = Vec::new();
        for ordinal in 0..steps {
            let Advance::Step { commit } = session.advance(&gix::open(root)?)? else {
                anyhow::bail!("clean step must complete");
            };
            rewritten.push(commit);
            assert_eq!(session.completed_steps(), ordinal + 1);
            assert_eq!(oid(root, "HEAD")?, commit);
            assert_eq!(
                oid(root, "main")?,
                original,
                "completed step still leaves original branch unpublished"
            );
            assert_eq!(
                git(root, &["show", &format!("{commit}:neighbor")])?,
                b"clean neighbor\n",
                "rewritten trees exclude both preserved dirty versions"
            );
            assert_eq!(git(root, &["show", ":neighbor"])?, b"staged private neighbor\n");
            assert_eq!(fs::read(root.join("neighbor"))?, b"unstaged private neighbor\n");
            session = Session::open(&gix::open(root)?)?;
        }
        if steps == 2 {
            assert_eq!(session.advance(&gix::open(root)?)?, Advance::ReadyFinal);
            assert_eq!(session.phase(), "ready-final");
        }
        let receipt = session.abort(&gix::open(root)?)?;
        assert_eq!(
            integration_snapshot(root)?,
            before,
            "abort after {steps} steps restores original HEAD/branch/log/index/work bytes"
        );
        let copy = retained::inspect(&receipt.path)?;
        for commit in rewritten {
            let ordinal = copy
                .manifest
                .entries
                .iter()
                .position(|entry| entry.area == Area::Objects && entry.path == commit.to_string().as_bytes())
                .expect("every rewritten commit remains independently retained");
            assert_eq!(
                copy.payload(ordinal)?,
                git(root, &["cat-file", "commit", &commit.to_string()])?
            );
        }
    }
    Ok(())
}

#[test]
fn linear_steps_replay_oldest_first_with_original_authorship_and_upstream_parentage() -> Result<()> {
    if gix_testtools::run_in_isolated_process().map_err(anyhow::Error::from_boxed)? {
        return Ok(());
    }
    let temp = preserved_linear_fixture(false)?;
    let root = temp.path();
    let original = oid(root, "main")?;
    let upstream = oid(root, "upstream")?;
    let originals = [oid(root, "main^")?, original];
    let mut session = Session::prepare(&gix::open(root)?, b"upstream".as_bstr())?.expect("divergence");
    assert_eq!(session.advance(&gix::open(root)?)?, Advance::Started);
    let mut parent = upstream;
    for original in originals {
        let Advance::Step { commit } = session.advance(&gix::open(root)?)? else {
            anyhow::bail!("linear step expected");
        };
        assert_eq!(oid(root, &format!("{commit}^"))?, parent);
        let old = git(root, &["cat-file", "commit", &original.to_string()])?;
        let new = git(root, &["cat-file", "commit", &commit.to_string()])?;
        let author = |bytes: &[u8]| {
            bytes
                .split(|byte| *byte == b'\n')
                .find(|line| line.starts_with(b"author "))
                .map(<[u8]>::to_vec)
        };
        assert_eq!(author(&new), author(&old));
        assert_eq!(
            git(root, &["show", "-s", "--format=%B", &commit.to_string()])?,
            git(root, &["show", "-s", "--format=%B", &original.to_string()])?
        );
        parent = commit;
    }
    assert_eq!(session.tip()?, parent);
    assert_eq!(session.advance(&gix::open(root)?)?, Advance::ReadyFinal);
    assert_eq!(oid(root, "main")?, original);
    Ok(())
}

#[test]
fn conflict_stop_keeps_original_preservation_and_correct_public_stages() -> Result<()> {
    if gix_testtools::run_in_isolated_process().map_err(anyhow::Error::from_boxed)? {
        return Ok(());
    }
    let temp = preserved_linear_fixture(true)?;
    let root = temp.path();
    let original = oid(root, "main")?;
    let expected = [
        oid(root, "main^^:selected")?,
        oid(root, "upstream:selected")?,
        oid(root, "main^:selected")?,
    ];
    let mut session = Session::prepare(&gix::open(root)?, b"upstream".as_bstr())?.expect("divergence");
    assert_eq!(session.advance(&gix::open(root)?)?, Advance::Started);
    assert_eq!(session.advance(&gix::open(root)?)?, Advance::Conflict);
    assert_eq!(session.completed_steps(), 0);
    assert_eq!(oid(root, "main")?, original);
    for (position, object) in expected.into_iter().enumerate() {
        assert_eq!(oid(root, &format!(":{}:selected", position + 1))?, object);
    }
    assert_eq!(git(root, &["show", ":neighbor"])?, b"staged private neighbor\n");
    assert_eq!(fs::read(root.join("neighbor"))?, b"unstaged private neighbor\n");
    assert!(
        gix::open(root)?.git_dir().join("brit-authoring").is_dir(),
        "conflict stop has an explicitly owned child session"
    );
    assert!(
        gitoxide_core::repository::authoring::conflict::Session::open(&gix::open(root)?).is_err(),
        "standalone authoring cannot bypass replay-parent ownership"
    );
    Ok(())
}

#[test]
fn live_empty_stop_requires_explicit_continue_while_initially_empty_commit_is_kept() -> Result<()> {
    if gix_testtools::run_in_isolated_process().map_err(anyhow::Error::from_boxed)? {
        return Ok(());
    }
    for initially_empty in [false, true] {
        let temp = fixture()?;
        let root = temp.path();
        git(root, &["switch", "-qc", "upstream"])?;
        commit(root, "selected", b"same change\n", "upstream")?;
        git(root, &["switch", "-q", "main"])?;
        if initially_empty {
            git(root, &["commit", "--allow-empty", "-qm", "empty original"])?;
        } else {
            commit(root, "selected", b"same change\n", "replayed same change")?;
        }
        let mut session = Session::prepare(&gix::open(root)?, b"upstream".as_bstr())?.expect("divergence");
        assert_eq!(session.advance(&gix::open(root)?)?, Advance::Started);
        if !initially_empty {
            assert_eq!(session.advance(&gix::open(root)?)?, Advance::Empty);
            assert_eq!(session.completed_steps(), 0);
            git(
                root,
                &[
                    "-c",
                    "gc.reflogExpire=never",
                    "-c",
                    "gc.reflogExpireUnreachable=never",
                    "gc",
                    "--prune=now",
                ],
            )?;
            session = Session::open(&gix::open(root)?)?;
            assert_eq!(
                session.phase(),
                "empty",
                "opening preserves the stop rather than continuing"
            );
        }
        let Advance::Step { commit } = session.advance(&gix::open(root)?)? else {
            anyhow::bail!("explicit empty keep must produce a commit");
        };
        assert_eq!(oid(root, &format!("{commit}^"))?, oid(root, "upstream")?);
        assert_eq!(oid(root, &format!("{commit}^{{tree}}"))?, oid(root, "upstream^{tree}")?);
        assert_eq!(session.completed_steps(), 1);
    }
    Ok(())
}

#[test]
fn conflict_resolution_imports_only_scoped_staging_through_the_owned_parent() -> Result<()> {
    if gix_testtools::run_in_isolated_process().map_err(anyhow::Error::from_boxed)? {
        return Ok(());
    }
    for external in [false, true] {
        let temp = preserved_linear_fixture(true)?;
        let root = temp.path();
        let original = oid(root, "main")?;
        let upstream = oid(root, "upstream")?;
        let mut session = Session::prepare(&gix::open(root)?, b"upstream".as_bstr())?.expect("divergence");
        assert_eq!(session.advance(&gix::open(root)?)?, Advance::Started);
        assert_eq!(session.advance(&gix::open(root)?)?, Advance::Conflict);
        git(
            root,
            &[
                "-c",
                "gc.reflogExpire=never",
                "-c",
                "gc.reflogExpireUnreachable=never",
                "gc",
                "--prune=now",
            ],
        )?;
        session = Session::open(&gix::open(root)?)?;
        session.recover_stop(&gix::open(root)?)?;
        fs::write(root.join("selected"), b"accepted resolution\n")?;
        if external {
            git(root, &["add", "selected"])?;
        } else {
            session.stage(&gix::open(root)?, &["selected".into()], false)?;
        }
        let Advance::Step { commit } = session.continue_conflict(&gix::open(root)?)? else {
            anyhow::bail!("resolved conflict must author one step");
        };
        assert_eq!(oid(root, &format!("{commit}^"))?, upstream);
        assert_eq!(
            git(root, &["show", &format!("{commit}:selected")])?,
            b"accepted resolution\n"
        );
        assert_eq!(
            git(root, &["show", &format!("{commit}:neighbor")])?,
            b"clean neighbor\n"
        );
        assert_eq!(git(root, &["show", ":neighbor"])?, b"staged private neighbor\n");
        assert_eq!(fs::read(root.join("neighbor"))?, b"unstaged private neighbor\n");
        assert_eq!(oid(root, "main")?, original);
        assert_eq!(session.completed_steps(), 1);
        assert!(
            !gix::open(root)?.git_dir().join("brit-authoring").exists(),
            "accepted child is retired before another step can begin"
        );
    }
    Ok(())
}

#[test]
fn final_publication_reattaches_original_branch_once_and_preserves_dirty_staging() -> Result<()> {
    if gix_testtools::run_in_isolated_process().map_err(anyhow::Error::from_boxed)? {
        return Ok(());
    }
    let temp = preserved_linear_fixture(false)?;
    let root = temp.path();
    let original = oid(root, "main")?;
    let original_log = fs::read(root.join(".git/logs/refs/heads/main"))?;
    let mut session = Session::prepare(&gix::open(root)?, b"upstream".as_bstr())?.expect("divergence");
    assert_eq!(session.advance(&gix::open(root)?)?, Advance::Started);
    for _ in 0..2 {
        assert!(matches!(session.advance(&gix::open(root)?)?, Advance::Step { .. }));
    }
    assert_eq!(session.advance(&gix::open(root)?)?, Advance::ReadyFinal);
    let final_tip = session.tip()?;
    assert_eq!(oid(root, "main")?, original);
    session.finish(&gix::open(root)?)?;
    assert_eq!(oid(root, "main")?, final_tip);
    assert_eq!(oid(root, "HEAD")?, final_tip);
    assert_eq!(fs::read(root.join(".git/HEAD"))?, b"ref: refs/heads/main\n");
    assert_eq!(oid(root, "ORIG_HEAD")?, original);
    assert_eq!(git(root, &["show", ":neighbor"])?, b"staged private neighbor\n");
    assert_eq!(fs::read(root.join("neighbor"))?, b"unstaged private neighbor\n");
    assert_eq!(git(root, &["show", "HEAD:neighbor"])?, b"clean neighbor\n");
    let final_log = fs::read(root.join(".git/logs/refs/heads/main"))?;
    assert!(
        final_log.starts_with(&original_log),
        "publication preserves every existing reflog byte"
    );
    let appended = &final_log[original_log.len()..];
    assert_eq!(
        appended.iter().filter(|byte| **byte == b'\n').count(),
        1,
        "final branch publication records exactly one reflog entry"
    );
    assert!(appended.starts_with(format!("{original} {final_tip} ").as_bytes()));
    let before_retry = integration_snapshot(root)?;
    assert!(
        session.abort(&gix::open(root)?).is_err(),
        "final decision is irrevocable"
    );
    let _ = session.finish(&gix::open(root)?);
    assert_eq!(
        integration_snapshot(root)?,
        before_retry,
        "repeated finish cannot append duplicate logs or undo final publication"
    );
    Ok(())
}

#[test]
fn automatic_replay_runs_only_operation_hooks_with_exact_rewrite_mapping() -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    if gix_testtools::run_in_isolated_process().map_err(anyhow::Error::from_boxed)? {
        return Ok(());
    }
    let temp = preserved_linear_fixture(false)?;
    let root = temp.path();
    let originals = [oid(root, "main^")?, oid(root, "main")?];
    for (name, body) in [
        (
            "pre-rebase",
            "printf '%s\\n' \"$@\" > \"$GIT_DIR/pre-args\"\nprintf 'pre\\n' >> \"$GIT_DIR/operation-hooks\"",
        ),
        (
            "post-rewrite",
            "printf '%s\\n' \"$@\" > \"$GIT_DIR/post-args\"\ncat > \"$GIT_DIR/post-mapping\"\ncat \"$GIT_DIR/HEAD\" > \"$GIT_DIR/post-head\"\nprintf 'post\\n' >> \"$GIT_DIR/operation-hooks\"",
        ),
        ("pre-commit", "printf forbidden > \"$GIT_DIR/forbidden-hook\"; exit 99"),
        (
            "prepare-commit-msg",
            "printf forbidden > \"$GIT_DIR/forbidden-hook\"; exit 99",
        ),
        ("commit-msg", "printf forbidden > \"$GIT_DIR/forbidden-hook\"; exit 99"),
        ("post-commit", "printf forbidden > \"$GIT_DIR/forbidden-hook\"; exit 99"),
    ] {
        let path = root.join(".git/hooks").join(name);
        fs::write(&path, format!("#!/bin/sh\nset -eu\n{body}\n"))?;
        fs::set_permissions(path, fs::Permissions::from_mode(0o700))?;
    }
    let mut session = Session::prepare(&gix::open(root)?, b"upstream".as_bstr())?.expect("divergence");
    assert_eq!(session.advance(&gix::open(root)?)?, Advance::Started);
    let mut mapping = Vec::new();
    for old in originals {
        let Advance::Step { commit } = session.advance(&gix::open(root)?)? else {
            anyhow::bail!("clean step");
        };
        mapping.extend_from_slice(format!("{old} {commit}\n").as_bytes());
    }
    assert_eq!(session.advance(&gix::open(root)?)?, Advance::ReadyFinal);
    session.finish(&gix::open(root)?)?;
    assert_eq!(fs::read(root.join(".git/pre-args"))?, b"upstream\n");
    assert_eq!(fs::read(root.join(".git/post-args"))?, b"rebase\n");
    assert_eq!(fs::read(root.join(".git/post-mapping"))?, mapping);
    assert_eq!(fs::read(root.join(".git/operation-hooks"))?, b"pre\npost\n");
    assert_eq!(
        fs::read(root.join(".git/post-head"))?,
        b"ref: refs/heads/main\n",
        "post hook sees already-published final attachment"
    );
    assert!(
        !root.join(".git/forbidden-hook").exists(),
        "automatic replay does not inherit explicit authoring hook policy"
    );
    Ok(())
}

#[test]
fn fast_forward_execution_has_zero_rewrites_and_keeps_attachment_or_aborts_exactly() -> Result<()> {
    if gix_testtools::run_in_isolated_process().map_err(anyhow::Error::from_boxed)? {
        return Ok(());
    }
    for detached in [false, true] {
        for abort in [false, true] {
            let temp = fixture()?;
            let root = temp.path();
            let original = oid(root, "main")?;
            git(root, &["switch", "-qc", "upstream"])?;
            let upstream = commit(root, "new", b"upstream file\n", "upstream")?;
            git(root, &["switch", "-q", "main"])?;
            if detached {
                git(root, &["switch", "--detach", "-q"])?;
            }
            let before = integration_snapshot(root)?;
            let mut session =
                Session::prepare(&gix::open(root)?, b"upstream".as_bstr())?.expect("fast-forward session");
            assert_eq!(session.advance(&gix::open(root)?)?, Advance::Started);
            assert_eq!(session.advance(&gix::open(root)?)?, Advance::ReadyFinal);
            assert_eq!(session.completed_steps(), 0);
            assert_eq!(
                oid(root, "main")?,
                original,
                "fast-forward is unpublished until final decision"
            );
            if abort {
                session.abort(&gix::open(root)?)?;
                assert_eq!(integration_snapshot(root)?, before);
                assert!(!root.join("new").exists());
            } else {
                session.finish(&gix::open(root)?)?;
                assert_eq!(oid(root, "HEAD")?, upstream);
                assert_eq!(oid(root, "main")?, if detached { original } else { upstream });
                assert_eq!(gix::open(root)?.head_name()?.is_none(), detached);
                assert_eq!(fs::read(root.join("new"))?, b"upstream file\n");
                assert_eq!(oid(root, "ORIG_HEAD")?, original);
            }
        }
    }
    Ok(())
}

#[test]
fn terminal_cleanup_keeps_independent_copies_after_journal_release_and_gc() -> Result<()> {
    use gitoxide_core::repository::replay::retained::{self, Area};
    if gix_testtools::run_in_isolated_process().map_err(anyhow::Error::from_boxed)? {
        return Ok(());
    }
    for abort in [false, true] {
        let temp = preserved_linear_fixture(false)?;
        let root = temp.path();
        let original_index = fs::read(root.join(".git/index"))?;
        let mut session = Session::prepare(&gix::open(root)?, b"upstream".as_bstr())?.expect("divergence");
        let mut rewritten = Vec::new();
        loop {
            match session.advance(&gix::open(root)?)? {
                Advance::ReadyFinal => break,
                Advance::Step { commit } => rewritten.push(commit),
                Advance::Started => {}
                state => anyhow::bail!("unexpected clean replay stop: {state:?}"),
            }
        }
        if abort {
            session.abort(&gix::open(root)?)?;
        } else {
            session.finish(&gix::open(root)?)?;
        }
        let before = integration_snapshot(root)?;
        let receipt = session.cleanup(&gix::open(root)?)?;
        assert_eq!(integration_snapshot(root)?, before);
        assert!(!session.directory().exists());
        assert!(
            git(
                root,
                &["for-each-ref", "--format=%(refname)", "refs/brit-rebase-retain/"]
            )?
            .is_empty()
        );
        drop(gitoxide_core::repository::switch::mutation_guard(&gix::open(root)?)?);
        git(root, &["reflog", "expire", "--expire=now", "--all"])?;
        git(root, &["gc", "--prune=now"])?;
        let copy = retained::inspect(&receipt.path)?;
        for commit in rewritten {
            let ordinal = copy
                .manifest
                .entries
                .iter()
                .position(|entry| entry.area == Area::Objects && entry.path == commit.to_string().as_bytes())
                .expect("released rewritten object independently retained");
            let bytes = copy.payload(ordinal)?;
            assert_eq!(
                gix::objs::compute_hash(commit.kind(), gix::objs::Kind::Commit, &bytes)?,
                commit
            );
        }
        assert_eq!(git(root, &["show", ":neighbor"])?, b"staged private neighbor\n");
        assert_eq!(fs::read(root.join("neighbor"))?, b"unstaged private neighbor\n");
        if abort {
            assert_eq!(fs::read(root.join(".git/index"))?, original_index);
        }
    }
    Ok(())
}

#[test]
fn unknown_terminal_journal_member_refuses_cleanup_before_releasing_refs() -> Result<()> {
    if gix_testtools::run_in_isolated_process().map_err(anyhow::Error::from_boxed)? {
        return Ok(());
    }
    let temp = divergent_fixture(false)?;
    let root = temp.path();
    let repo = gix::open(root)?;
    let mut session = Session::prepare(&repo, b"upstream".as_bstr())?.expect("divergence");
    session.abort(&repo)?;
    fs::write(session.directory().join("foreign-evidence"), b"do not delete\n")?;
    let refs = git(
        root,
        &[
            "for-each-ref",
            "--format=%(refname) %(objectname)",
            "refs/brit-rebase-retain/",
        ],
    )?;
    assert!(session.cleanup(&repo).is_err());
    assert_eq!(
        git(
            root,
            &[
                "for-each-ref",
                "--format=%(refname) %(objectname)",
                "refs/brit-rebase-retain/"
            ]
        )?,
        refs
    );
    assert_eq!(
        fs::read(session.directory().join("foreign-evidence"))?,
        b"do not delete\n"
    );
    Ok(())
}

fn directory_replay_fixture(shape: &str) -> Result<tempfile::TempDir> {
    let temp = fixture()?;
    let root = temp.path();
    if shape == "file-to-directory" {
        commit(root, "slot", b"old file\n", "file baseline")?;
    }
    if shape == "directory-to-file" {
        fs::create_dir(root.join("slot"))?;
        commit(root, "slot/child", b"old child\n", "directory baseline")?;
    }
    git(root, &["switch", "-qc", "upstream"])?;
    commit(root, "upstream", b"upstream\n", "upstream")?;
    git(root, &["switch", "-q", "main"])?;
    match shape {
        "nested" => {
            fs::create_dir_all(root.join("new/deep"))?;
            fs::write(root.join("new/deep/file"), b"nested replay\n")?;
        }
        "file-to-directory" => {
            fs::remove_file(root.join("slot"))?;
            fs::create_dir(root.join("slot"))?;
            fs::write(root.join("slot/child"), b"new child\n")?;
        }
        "directory-to-file" => {
            fs::remove_file(root.join("slot/child"))?;
            fs::remove_dir(root.join("slot"))?;
            fs::write(root.join("slot"), b"new file\n")?;
        }
        _ => anyhow::bail!("unknown directory fixture"),
    }
    git(root, &["add", "--", if shape == "nested" { "new" } else { "slot" }])?;
    git(root, &["commit", "-qm", "directory replay"])?;
    fs::write(root.join("untracked-neighbor"), b"untracked private\n")?;
    fs::write(root.join(".git/info/exclude"), b"ignored-neighbor\n")?;
    fs::write(root.join("ignored-neighbor"), b"ignored private\n")?;
    Ok(temp)
}

#[test]
fn directory_transitions_replay_then_abort_with_exact_original_directory_inodes() -> Result<()> {
    use std::os::unix::fs::MetadataExt;
    if gix_testtools::run_in_isolated_process().map_err(anyhow::Error::from_boxed)? {
        return Ok(());
    }
    for shape in ["nested", "file-to-directory", "directory-to-file"] {
        let temp = directory_replay_fixture(shape)?;
        let root = temp.path();
        let original_tree = oid(root, "main^{tree}")?;
        let before = integration_snapshot(root)?;
        let directory_inodes = ["new", "new/deep", "slot"]
            .into_iter()
            .filter(|path| root.join(path).is_dir())
            .map(|path| {
                let metadata = fs::metadata(root.join(path))?;
                Ok((path, (metadata.dev(), metadata.ino())))
            })
            .collect::<Result<Vec<_>>>()?;
        let mut session = Session::prepare(&gix::open(root)?, b"upstream".as_bstr())?.expect("divergence");
        assert_eq!(session.advance(&gix::open(root)?)?, Advance::Started);
        let Advance::Step { commit } = session.advance(&gix::open(root)?)? else {
            anyhow::bail!("clean directory step expected: {shape}");
        };
        let selected = match shape {
            "nested" => "new/deep/file",
            "file-to-directory" => "slot/child",
            _ => "slot",
        };
        assert_eq!(
            git(root, &["show", &format!("{commit}:{selected}")])?,
            git(root, &["show", &format!("{original_tree}:{selected}")])?
        );
        assert_eq!(fs::read(root.join("untracked-neighbor"))?, b"untracked private\n");
        assert_eq!(fs::read(root.join("ignored-neighbor"))?, b"ignored private\n");
        session.abort(&gix::open(root)?)?;
        assert_eq!(integration_snapshot(root)?, before);
        for (path, expected) in directory_inodes {
            let metadata = fs::metadata(root.join(path))?;
            assert_eq!(
                (metadata.dev(), metadata.ino()),
                expected,
                "abort restores original directory identity for {shape}:{path}"
            );
        }
        assert_eq!(
            fs::read(root.join(selected))?,
            git(root, &["show", &format!("main:{selected}")])?
        );
    }
    Ok(())
}

#[test]
fn directory_to_file_checkout_refuses_untracked_and_ignored_descendant_collisions() -> Result<()> {
    use std::os::unix::fs::MetadataExt;
    if gix_testtools::run_in_isolated_process().map_err(anyhow::Error::from_boxed)? {
        return Ok(());
    }
    for ignored in [false, true] {
        let temp = directory_replay_fixture("file-to-directory")?;
        let root = temp.path();
        fs::write(root.join("slot/private"), b"must survive directory replacement\n")?;
        if ignored {
            fs::write(root.join(".git/info/exclude"), b"slot/private\nignored-neighbor\n")?;
        }
        let repo = gix::open(root)?;
        drop(gitoxide_core::repository::switch::mutation_guard(&repo)?);
        let before = integration_snapshot(root)?;
        let descendants = snapshot(&root.join("slot"))?;
        let directory = fs::metadata(root.join("slot"))?;
        match Session::prepare(&repo, b"upstream".as_bstr()) {
            Ok(Some(mut session)) => assert!(
                session.advance(&repo).is_err(),
                "untracked/ignored descendants cannot be treated as owned directory contents during checkout"
            ),
            Err(_) => {}
            Ok(None) => anyhow::bail!("divergent collision fixture cannot be a no-op"),
        }
        assert_eq!(integration_snapshot(root)?, before);
        assert_eq!(
            snapshot(&root.join("slot"))?,
            descendants,
            "all tracked and private descendant bytes survive refusal"
        );
        let after = fs::metadata(root.join("slot"))?;
        assert_eq!((after.dev(), after.ino()), (directory.dev(), directory.ino()));
    }
    Ok(())
}

#[test]
fn virtualized_or_partial_object_histories_refuse_before_preparation() -> Result<()> {
    if gix_testtools::run_in_isolated_process().map_err(anyhow::Error::from_boxed)? {
        return Ok(());
    }
    for shape in ["alternate", "replacement", "graft", "promisor", "partial-clone"] {
        let temp = divergent_fixture(false)?;
        let root = temp.path();
        match shape {
            "alternate" => fs::write(
                root.join(".git/objects/info/alternates"),
                b"/nonexistent/local-alternate\n",
            )?,
            "replacement" => {
                let original = oid(root, "main")?;
                let upstream = oid(root, "upstream")?;
                git(
                    root,
                    &["update-ref", &format!("refs/replace/{original}"), &upstream.to_string()],
                )?;
            }
            "graft" => fs::write(root.join(".git/info/grafts"), format!("{}\n", oid(root, "main")?))?,
            "promisor" => fs::write(root.join(".git/objects/pack/pack-unavailable.promisor"), b"")?,
            "partial-clone" => {
                git(root, &["config", "extensions.partialClone", "origin"])?;
            }
            _ => unreachable!("bounded fixture shapes"),
        }
        let before = snapshot(root)?;
        let repo = gix::open(root)?;
        assert!(
            Selection::prepare(&repo, b"upstream".as_bstr()).is_err(),
            "history not provably local: {shape}"
        );
        assert!(
            Session::prepare(&repo, b"upstream".as_bstr()).is_err(),
            "no parent journal may claim virtualized history: {shape}"
        );
        assert_eq!(snapshot(root)?, before, "refusal is read-only for {shape}");
        assert!(!repo.git_dir().join("brit-rebase").exists());
    }
    Ok(())
}

#[test]
fn object_and_index_environment_redirections_refuse_without_writing_a_journal() -> Result<()> {
    if gix_testtools::run_in_isolated_process().map_err(anyhow::Error::from_boxed)? {
        return Ok(());
    }
    let temp = divergent_fixture(false)?;
    let root = temp.path();
    let repo = gix::open(root)?;
    let before = snapshot(root)?;
    for variable in [
        "GIT_INDEX_FILE",
        "GIT_OBJECT_DIRECTORY",
        "GIT_ALTERNATE_OBJECT_DIRECTORIES",
        "GIT_REPLACE_REF_BASE",
    ] {
        let _override = gix_testtools::Env::new().set(variable, "unsupported-redirection");
        assert!(
            Selection::prepare(&repo, b"upstream".as_bstr()).is_err(),
            "redirection must be rejected even on an already opened handle: {variable}"
        );
        assert!(Session::prepare(&repo, b"upstream".as_bstr()).is_err());
        assert_eq!(snapshot(root)?, before);
    }
    Ok(())
}

#[cfg(feature = "switch-test-faults")]
#[test]
fn preparation_interruption_child_helper() -> Result<()> {
    let Some(root) = std::env::var_os("BRIT_TEST_REPLAY_PREPARATION_ROOT") else {
        return Ok(());
    };
    let repo = gix::open(Path::new(&root))?;
    if std::env::var_os("BRIT_TEST_REPLAY_LIFECYCLE").is_some_and(|mode| mode == "retry-abort") {
        Session::open(&repo)?.abort(&repo)?;
        return Ok(());
    }
    let mut session = Session::prepare(&repo, b"upstream".as_bstr())?.expect("divergent fixture");
    if let Some(mode) = std::env::var_os("BRIT_TEST_REPLAY_LIFECYCLE") {
        if mode == "after-first-step" || mode == "abort-after-first-step" {
            {
                let _unarmed = gix_testtools::Env::new().unset("BRIT_SWITCH_CHECKPOINT");
                assert_eq!(session.advance(&repo)?, Advance::Started);
                assert!(matches!(session.advance(&repo)?, Advance::Step { .. }));
                assert_eq!(session.completed_steps(), 1);
            }
            if mode == "abort-after-first-step" {
                session.abort(&repo)?;
            } else {
                session.advance(&repo)?;
            }
        } else if mode == "conflict" {
            assert_eq!(session.advance(&repo)?, Advance::Started);
            assert_eq!(session.advance(&repo)?, Advance::Conflict);
            fs::write(Path::new(&root).join("selected"), b"resolved after interruption\n")?;
            session.stage(&repo, &["selected".into()], false)?;
            session.continue_conflict(&repo)?;
        } else {
            loop {
                match session.advance(&repo)? {
                    Advance::ReadyFinal => break,
                    Advance::Started | Advance::Step { .. } => {}
                    state => anyhow::bail!("unexpected helper replay stop: {state:?}"),
                }
            }
            if mode == "abort-cleanup" {
                session.abort(&repo)?;
            } else {
                session.finish(&repo)?;
            }
            session.cleanup(&repo)?;
        }
        return Ok(());
    }
    if std::env::var_os("BRIT_TEST_REPLAY_ABORT").is_some() {
        session.abort(&repo)?;
        return Ok(());
    }
    if std::env::var_os("BRIT_TEST_REPLAY_CANCEL").is_some() {
        session.abort_prepared(&repo)?;
    }
    Ok(())
}

#[cfg(feature = "switch-test-faults")]
#[test]
fn real_signals_during_preparation_recover_without_touching_original_integration() -> Result<()> {
    use std::{
        os::unix::process::ExitStatusExt,
        process::{Command, Stdio},
        time::{Duration, Instant},
    };
    if gix_testtools::run_in_isolated_process().map_err(anyhow::Error::from_boxed)? {
        return Ok(());
    }
    for signal in ["INT", "KILL"] {
        for checkpoint in [
            "replay-state-file-synced",
            "replay-state-directory-synced",
            "replay-retained",
            "replay-preparation-tombstoned",
            "replay-preparation-last-member-removed",
        ] {
            let temp = divergent_fixture(false)?;
            let root = temp.path();
            let checkpoints = tempfile::tempdir()?;
            let before = integration_snapshot(root)?;
            let mut command = Command::new(std::env::current_exe()?);
            gix_testtools::configure_git_environment(&mut command, root);
            command
                .args(["--exact", "preparation_interruption_child_helper", "--nocapture"])
                .env("BRIT_TEST_REPLAY_PREPARATION_ROOT", root)
                .env("BRIT_SWITCH_CHECKPOINT", checkpoint)
                .env("BRIT_SWITCH_CHECKPOINT_DIR", checkpoints.path())
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::inherit());
            if checkpoint.starts_with("replay-preparation-") {
                command.env("BRIT_TEST_REPLAY_CANCEL", "1");
            }
            let mut child = command.spawn()?;
            let reached = checkpoints.path().join(format!("{checkpoint}.reached"));
            let deadline = Instant::now() + Duration::from_secs(15);
            while !reached.exists() {
                ensure!(child.try_wait()?.is_none(), "helper exited before {checkpoint}");
                if Instant::now() >= deadline {
                    child.kill()?;
                    child.wait()?;
                    anyhow::bail!("checkpoint timeout: {checkpoint}");
                }
                std::thread::sleep(Duration::from_millis(10));
            }
            let sent = Command::new("/bin/kill")
                .args([&format!("-{signal}"), &child.id().to_string()])
                .status()?;
            ensure!(sent.success(), "send SIG{signal}");
            let status = child.wait()?;
            assert_eq!(
                status.signal(),
                Some(if signal == "INT" { 2 } else { 9 }),
                "actual SIG{signal} interrupted {checkpoint}"
            );
            assert_eq!(
                integration_snapshot(root)?,
                before,
                "SIG{signal} at {checkpoint} must not publish live changes"
            );
            let repo = gix::open(root)?;
            if checkpoint.starts_with("replay-preparation-") {
                assert!(!repo.git_dir().join("brit-rebase").exists());
                let tombstones = fs::read_dir(repo.git_dir())?
                    .filter_map(|entry| entry.ok())
                    .filter(|entry| {
                        entry.path().is_dir() && entry.file_name().to_string_lossy().starts_with("brit-rebase-done-")
                    })
                    .map(|entry| entry.path())
                    .collect::<Vec<_>>();
                assert_eq!(tombstones.len(), 1, "exact cancelled preparation remains for cleanup");
                Session::cleanup_prepared(&repo, &tombstones[0])?;
                assert!(!tombstones[0].exists());
            } else {
                assert!(
                    gitoxide_core::repository::switch::mutation_guard(&repo).is_err(),
                    "interruption keeps operation reservation"
                );
                let recovered = Session::recover_preparation(&repo)?;
                assert_eq!(
                    integration_snapshot(root)?,
                    before,
                    "recovery completes only private preparation"
                );
                recovered.abort_prepared(&repo)?;
                assert!(!recovered.directory().exists());
            }
            assert_eq!(integration_snapshot(root)?, before);
            assert!(
                git(
                    root,
                    &["for-each-ref", "--format=%(refname)", "refs/brit-rebase-retain/"]
                )?
                .is_empty()
            );
        }
    }
    Ok(())
}

#[cfg(feature = "switch-test-faults")]
#[test]
fn interrupted_abort_retains_copy_before_restoration_and_recovers_decision_once() -> Result<()> {
    use gitoxide_core::repository::replay::retained;
    use std::{
        process::{Command, Stdio},
        time::{Duration, Instant},
    };
    if gix_testtools::run_in_isolated_process().map_err(anyhow::Error::from_boxed)? {
        return Ok(());
    }
    for checkpoint in ["replay-abort-copy-complete", "replay-decision-durable"] {
        let temp = divergent_fixture(false)?;
        let root = temp.path();
        let checkpoints = tempfile::tempdir()?;
        let before = integration_snapshot(root)?;
        let mut command = Command::new(std::env::current_exe()?);
        gix_testtools::configure_git_environment(&mut command, root);
        let mut child = command
            .args(["--exact", "preparation_interruption_child_helper", "--nocapture"])
            .env("BRIT_TEST_REPLAY_PREPARATION_ROOT", root)
            .env("BRIT_TEST_REPLAY_ABORT", "1")
            .env("BRIT_SWITCH_CHECKPOINT", checkpoint)
            .env("BRIT_SWITCH_CHECKPOINT_DIR", checkpoints.path())
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::inherit())
            .spawn()?;
        let reached = checkpoints.path().join(format!("{checkpoint}.reached"));
        let deadline = Instant::now() + Duration::from_secs(15);
        while !reached.exists() {
            ensure!(child.try_wait()?.is_none(), "abort helper exited before {checkpoint}");
            if Instant::now() >= deadline {
                child.kill()?;
                child.wait()?;
                anyhow::bail!("abort checkpoint timeout: {checkpoint}");
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        child.kill()?;
        child.wait()?;
        assert_eq!(integration_snapshot(root)?, before);
        let repo = gix::open(root)?;
        let mut leaves = Vec::new();
        for operation in fs::read_dir(repo.git_dir().join("brit-rebase-retained"))? {
            for leaf in fs::read_dir(operation?.path())? {
                leaves.push(leaf?.path());
            }
        }
        assert_eq!(
            leaves.len(),
            1,
            "abort has one durable retained copy before restoration"
        );
        let receipt = retained::inspect(&leaves[0])?.receipt;
        let retained_before = snapshot(&receipt.path)?;
        if checkpoint == "replay-abort-copy-complete" {
            assert_eq!(
                Session::open(&repo)?.abort(&repo)?.digest,
                receipt.digest,
                "interrupted retention resumes the same copy"
            );
        } else {
            Session::open(&repo)?.abort(&repo)?;
        }
        assert_eq!(integration_snapshot(root)?, before);
        assert_eq!(snapshot(&receipt.path)?, retained_before);
        let _ = Session::recover_decision(&repo);
        assert_eq!(
            integration_snapshot(root)?,
            before,
            "repeated recovery never publishes a second logical decision"
        );
        assert_eq!(snapshot(&receipt.path)?, retained_before);
    }
    Ok(())
}

#[cfg(feature = "switch-test-faults")]
fn kill_at_lifecycle_checkpoint(root: &Path, checkpoint: &str, mode: &str) -> Result<()> {
    use std::{
        process::{Command, Stdio},
        time::{Duration, Instant},
    };
    let checkpoints = tempfile::tempdir()?;
    let mut command = Command::new(std::env::current_exe()?);
    gix_testtools::configure_git_environment(&mut command, root);
    let mut child = command
        .args(["--exact", "preparation_interruption_child_helper", "--nocapture"])
        .env("BRIT_TEST_REPLAY_PREPARATION_ROOT", root)
        .env("BRIT_TEST_REPLAY_LIFECYCLE", mode)
        .env("BRIT_SWITCH_CHECKPOINT", checkpoint)
        .env("BRIT_SWITCH_CHECKPOINT_DIR", checkpoints.path())
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::inherit())
        .spawn()?;
    let reached = checkpoints.path().join(format!("{checkpoint}.reached"));
    let deadline = Instant::now() + Duration::from_secs(20);
    while !reached.exists() {
        ensure!(
            child.try_wait()?.is_none(),
            "lifecycle helper exited before {checkpoint}"
        );
        if Instant::now() >= deadline {
            child.kill()?;
            child.wait()?;
            anyhow::bail!("lifecycle checkpoint timeout {checkpoint}");
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    child.kill()?;
    child.wait()?;
    Ok(())
}

#[cfg(feature = "switch-test-faults")]
fn cleanup_receipt(repo: &gix::Repository) -> Result<std::path::PathBuf> {
    let receipts = fs::read_dir(repo.git_dir())?
        .filter_map(|entry| entry.ok())
        .filter(|entry| {
            entry.path().is_file()
                && entry.file_name().to_string_lossy().starts_with("brit-rebase-cleanup-")
                && entry.path().extension().is_some_and(|extension| extension == "json")
        })
        .map(|entry| entry.path())
        .collect::<Vec<_>>();
    ensure!(receipts.len() == 1, "one exact cleanup receipt expected");
    Ok(receipts[0].clone())
}

#[cfg(feature = "switch-test-faults")]
#[test]
fn interrupted_terminal_cleanup_resumes_from_its_receipt_through_last_member_removal() -> Result<()> {
    use gitoxide_core::repository::replay::retained;
    if gix_testtools::run_in_isolated_process().map_err(anyhow::Error::from_boxed)? {
        return Ok(());
    }
    for checkpoint in [
        "replay-cleanup-receipt-durable",
        "replay-cleanup-retention-released",
        "replay-cleanup-directory-renamed",
        "replay-cleanup-tombstoned",
        "replay-cleanup-member-removed",
        "replay-cleanup-last-member-removed",
    ] {
        let temp = preserved_linear_fixture(false)?;
        let root = temp.path();
        let original = integration_snapshot(root)?;
        kill_at_lifecycle_checkpoint(root, checkpoint, "abort-cleanup")?;
        let repo = gix::open(root)?;
        let receipt = cleanup_receipt(&repo)?;
        assert_eq!(integration_snapshot(root)?, original);
        if receipt.to_string_lossy().ends_with(".retired.json") {
            drop(gitoxide_core::repository::switch::mutation_guard(&repo)?);
        } else {
            assert!(gitoxide_core::repository::switch::mutation_guard(&repo).is_err());
        }
        Session::recover_cleanup(&repo, &receipt)?;
        assert!(!receipt.exists());
        assert!(!repo.git_dir().join("brit-rebase").exists());
        assert_eq!(integration_snapshot(root)?, original);
        assert!(
            git(
                root,
                &["for-each-ref", "--format=%(refname)", "refs/brit-rebase-retain/"]
            )?
            .is_empty()
        );
        for operation in fs::read_dir(repo.git_dir().join("brit-rebase-retained"))? {
            for leaf in fs::read_dir(operation?.path())? {
                retained::inspect(&leaf?.path())?;
            }
        }
    }
    Ok(())
}

#[cfg(feature = "switch-test-faults")]
#[test]
fn cleanup_recovery_refuses_corrupt_or_missing_release_copy_before_deleting_refs() -> Result<()> {
    if gix_testtools::run_in_isolated_process().map_err(anyhow::Error::from_boxed)? {
        return Ok(());
    }
    for remove in [false, true] {
        let temp = divergent_fixture(false)?;
        let root = temp.path();
        kill_at_lifecycle_checkpoint(root, "replay-cleanup-receipt-durable", "abort-cleanup")?;
        let repo = gix::open(root)?;
        let receipt = cleanup_receipt(&repo)?;
        let envelope: serde_json::Value = serde_json::from_slice(&fs::read(&receipt)?)?;
        let payload: Vec<u8> = serde_json::from_value(envelope["payload"].clone())?;
        let ledger: serde_json::Value = serde_json::from_slice(&payload)?;
        let path: Vec<u8> = serde_json::from_value(ledger["retained_path"].clone())?;
        let leaf = gix::path::from_bstr(path.as_bstr())?;
        if remove {
            fs::remove_file(leaf.join("COPY_COMPLETE"))?;
        } else {
            fs::write(leaf.join("manifest.json"), b"corrupt release manifest")?;
        }
        let before = snapshot(root)?;
        assert!(Session::recover_cleanup(&repo, &receipt).is_err());
        assert_eq!(
            snapshot(root)?,
            before,
            "no retention release or cleanup after losing independent recovery proof"
        );
    }
    Ok(())
}

#[cfg(feature = "switch-test-faults")]
#[test]
fn interrupted_owned_child_preparation_and_archive_resume_without_recapturing_preservation() -> Result<()> {
    if gix_testtools::run_in_isolated_process().map_err(anyhow::Error::from_boxed)? {
        return Ok(());
    }
    for checkpoint in [
        "replay-child-recipe-durable",
        "replay-child-published",
        "replay-child-archived",
    ] {
        let temp = preserved_linear_fixture(true)?;
        let root = temp.path();
        let original = oid(root, "main")?;
        kill_at_lifecycle_checkpoint(root, checkpoint, "conflict")?;
        let repo = gix::open(root)?;
        let mut session = Session::open(&repo)?;
        let result = if checkpoint == "replay-child-archived" {
            session.advance(&repo)?
        } else {
            session.recover_stop(&repo)?;
            fs::write(root.join("selected"), b"resolved after interruption\n")?;
            session.stage(&repo, &["selected".into()], false)?;
            session.continue_conflict(&repo)?
        };
        let Advance::Step { commit } = result else {
            anyhow::bail!("recovered child must produce one step");
        };
        assert_eq!(session.completed_steps(), 1);
        assert_eq!(oid(root, "main")?, original);
        assert_eq!(
            git(root, &["show", &format!("{commit}:neighbor")])?,
            b"clean neighbor\n"
        );
        assert_eq!(git(root, &["show", ":neighbor"])?, b"staged private neighbor\n");
        assert_eq!(fs::read(root.join("neighbor"))?, b"unstaged private neighbor\n");
    }
    Ok(())
}

#[cfg(feature = "switch-test-faults")]
#[test]
fn interrupted_directory_allocation_resumes_the_pinned_transition_and_preserves_original_inode() -> Result<()> {
    use std::os::unix::fs::MetadataExt;
    if gix_testtools::run_in_isolated_process().map_err(anyhow::Error::from_boxed)? {
        return Ok(());
    }
    for shape in ["nested", "file-to-directory", "directory-to-file"] {
        let temp = directory_replay_fixture(shape)?;
        let root = temp.path();
        let original = integration_snapshot(root)?;
        let directories = ["new", "new/deep", "slot"]
            .into_iter()
            .filter(|path| root.join(path).is_dir())
            .map(|path| {
                let metadata = fs::metadata(root.join(path))?;
                Ok((path, (metadata.dev(), metadata.ino())))
            })
            .collect::<Result<Vec<_>>>()?;
        kill_at_lifecycle_checkpoint(root, "replay-directories-prepared", "abort-cleanup")?;
        assert_eq!(
            integration_snapshot(root)?,
            original,
            "directory preparation cannot publish checkout before decision"
        );
        let repo = gix::open(root)?;
        let mut session = Session::open(&repo)?;
        loop {
            match session.advance(&repo)? {
                Advance::ReadyFinal => break,
                Advance::Started | Advance::Step { .. } => {}
                state => anyhow::bail!("unexpected recovered directory state: {state:?}"),
            }
        }
        session.abort(&repo)?;
        assert_eq!(integration_snapshot(root)?, original);
        for (path, expected) in directories {
            let metadata = fs::metadata(root.join(path))?;
            assert_eq!(
                (metadata.dev(), metadata.ino()),
                expected,
                "original directory survives interrupted allocation: {shape}:{path}"
            );
        }
    }
    Ok(())
}

#[cfg(feature = "switch-test-faults")]
#[test]
fn unknown_operation_hooks_are_never_rerun_and_have_distinct_parent_recovery_actions() -> Result<()> {
    use gitoxide_core::repository::authoring::invocation::InvocationOutcome;
    use std::{
        os::unix::fs::PermissionsExt,
        process::{Command, Stdio},
        time::{Duration, Instant},
    };
    if gix_testtools::run_in_isolated_process().map_err(anyhow::Error::from_boxed)? {
        return Ok(());
    }
    for (post, foreign) in [(false, false), (true, false), (true, true)] {
        let temp = divergent_fixture(false)?;
        let root = temp.path();
        let original = integration_snapshot(root)?;
        let name = if post { "post-rewrite" } else { "pre-rebase" };
        let path = root.join(".git/hooks").join(name);
        fs::write(&path, b"#!/bin/sh\nprintf 'executed\\n' >> \"$GIT_DIR/hook-count\"\n")?;
        fs::set_permissions(path, fs::Permissions::from_mode(0o700))?;
        let mut command = Command::new(std::env::current_exe()?);
        gix_testtools::configure_git_environment(&mut command, root);
        let mut child = command
            .args(["--exact", "preparation_interruption_child_helper", "--nocapture"])
            .env("BRIT_TEST_REPLAY_PREPARATION_ROOT", root)
            .env("BRIT_TEST_REPLAY_LIFECYCLE", "finish-cleanup")
            .env("BRIT_AUTHORING_INVOCATION_PAUSE", "after-wait")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::inherit())
            .spawn()?;
        let checkpoint = root.join(".git/brit-rebase").join(name).join("checkpoint");
        let deadline = Instant::now() + Duration::from_secs(20);
        while !checkpoint.exists() {
            ensure!(
                child.try_wait()?.is_none(),
                "hook helper exited before supervised after-wait"
            );
            if Instant::now() >= deadline {
                child.kill()?;
                child.wait()?;
                anyhow::bail!("hook checkpoint timeout");
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        child.kill()?;
        child.wait()?;
        let repo = gix::open(root)?;
        let mut session = Session::open(&repo)?;
        assert_eq!(fs::read(root.join(".git/hook-count"))?, b"executed\n");
        if post {
            let published = integration_snapshot(root)?;
            assert_ne!(oid(root, "main")?, oid(root, "ORIG_HEAD")?);
            assert!(
                session.abort(&repo).is_err(),
                "unknown post hook cannot undo final decision"
            );
            assert!(
                session.finish(&repo).is_err(),
                "unknown post hook is not a retry instruction"
            );
            if foreign {
                fs::write(root.join("selected"), b"foreign work during unknown post hook\n")?;
                assert!(session.acknowledge_post_hook(&repo).is_err());
                assert_eq!(
                    fs::read(root.join("selected"))?,
                    b"foreign work during unknown post hook\n"
                );
                assert_ne!(session.phase(), "completed");
                assert!(session.cleanup(&repo).is_err());
                assert_eq!(Session::open(&repo)?.phase(), session.phase());
            } else {
                session.acknowledge_post_hook(&repo)?;
                assert_eq!(session.post_hook_outcome(), Some(InvocationOutcome::Unknown));
                assert_eq!(integration_snapshot(root)?, published);
                session.cleanup(&repo)?;
            }
        } else {
            session.recover_pre_hook(&repo)?;
            assert_eq!(session.phase(), "pre-unknown");
            assert!(session.advance(&repo).is_err());
            assert!(session.skip(&repo).is_err());
            session.abort(&repo)?;
            assert_eq!(
                integration_snapshot(root)?,
                original,
                "unknown pre hook leaves original integration and permits only abort"
            );
            session.cleanup(&repo)?;
        }
        assert_eq!(
            fs::read(root.join(".git/hook-count"))?,
            b"executed\n",
            "recovery must never run an uncertain hook a second time"
        );
    }
    Ok(())
}

#[cfg(feature = "switch-test-faults")]
#[test]
fn interrupted_proposal_retention_aborts_without_leaking_refs_or_losing_authored_objects() -> Result<()> {
    use gitoxide_core::repository::replay::retained::{self, Area};
    if gix_testtools::run_in_isolated_process().map_err(anyhow::Error::from_boxed)? {
        return Ok(());
    }
    for checkpoint in ["replay-retention-recipe", "replay-intermediate-retained"] {
        let temp = preserved_linear_fixture(false)?;
        let root = temp.path();
        let original = integration_snapshot(root)?;
        kill_at_lifecycle_checkpoint(root, checkpoint, "finish-cleanup")?;
        let repo = gix::open(root)?;
        let mut session = Session::open(&repo)?;
        assert_eq!(
            session.completed_steps(),
            0,
            "retention interruption precedes the first step decision"
        );
        let envelope: serde_json::Value =
            serde_json::from_slice(&fs::read(session.directory().join("proposal-0.json"))?)?;
        let payload: Vec<u8> = serde_json::from_value(envelope["payload"].clone())?;
        let (proposed, bytes): (String, Vec<u8>) = serde_json::from_slice(&payload)?;
        session.abort(&repo)?;
        assert_eq!(integration_snapshot(root)?, original);
        let receipt = session.cleanup(&repo)?;
        assert!(!session.directory().exists());
        assert!(
            git(
                root,
                &["for-each-ref", "--format=%(refname)", "refs/brit-rebase-retain/"]
            )?
            .is_empty(),
            "even a ref created before the step decision is released from its owned recipe"
        );
        git(root, &["reflog", "expire", "--expire=now", "--all"])?;
        git(root, &["gc", "--prune=now"])?;
        let copy = retained::inspect(&receipt.path)?;
        let ordinal = copy
            .manifest
            .entries
            .iter()
            .position(|entry| entry.area == Area::Objects && entry.path == proposed.as_bytes())
            .expect("aborted proposal remains independent of ODB retention");
        assert_eq!(copy.payload(ordinal)?, bytes);
    }
    Ok(())
}

fn advance_to_ready(session: &mut Session, root: &Path) -> Result<()> {
    loop {
        match session.advance(&gix::open(root)?)? {
            Advance::ReadyFinal => return Ok(()),
            Advance::Started | Advance::Step { .. } => {}
            state => anyhow::bail!("unexpected replay state: {state:?}"),
        }
    }
}

#[test]
fn ready_final_refuses_edited_replay_work_without_publishing() -> Result<()> {
    if gix_testtools::run_in_isolated_process().map_err(anyhow::Error::from_boxed)? {
        return Ok(());
    }
    let temp = divergent_fixture(false)?;
    let root = temp.path();
    let mut session = Session::prepare(&gix::open(root)?, b"upstream".as_bstr())?.unwrap();
    advance_to_ready(&mut session, root)?;
    fs::write(root.join("selected"), b"new work after READY_FINAL\n")?;
    let before = integration_snapshot(root)?;
    assert!(session.finish(&gix::open(root)?).is_err());
    assert_eq!(
        integration_snapshot(root)?,
        before,
        "refusal must preserve unpublished branch and new work"
    );
    assert_ne!(session.phase(), "completed");
    Ok(())
}

#[test]
fn final_publication_refuses_orig_head_replaced_since_preparation() -> Result<()> {
    if gix_testtools::run_in_isolated_process().map_err(anyhow::Error::from_boxed)? {
        return Ok(());
    }
    for initially_present in [false, true] {
        let temp = divergent_fixture(false)?;
        let root = temp.path();
        if initially_present {
            fs::write(root.join(".git/ORIG_HEAD"), format!("{}\n", oid(root, "base")?))?;
        }
        let mut session = Session::prepare(&gix::open(root)?, b"upstream".as_bstr())?.unwrap();
        advance_to_ready(&mut session, root)?;
        fs::write(root.join(".git/ORIG_HEAD"), format!("{}\n", oid(root, "upstream")?))?;
        let before = integration_snapshot(root)?;
        assert!(
            session.finish(&gix::open(root)?).is_err(),
            "ORIG_HEAD belongs to the preparation snapshot"
        );
        assert_eq!(integration_snapshot(root)?, before);
    }
    Ok(())
}

#[test]
fn successful_post_hook_that_changes_observed_state_does_not_report_completed() -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    if gix_testtools::run_in_isolated_process().map_err(anyhow::Error::from_boxed)? {
        return Ok(());
    }
    for (target, script) in [
        ("selected", "printf 'hook changed replay work\\n' > selected"),
        ("neighbor", "printf 'hook changed preserved work\\n' > neighbor"),
        (
            ".git/config",
            "printf '\\n[replay-test]\\n changed = true\\n' >> \"$GIT_DIR/config\"",
        ),
        (".git/HEAD", "printf 'ref: refs/heads/upstream\\n' > \"$GIT_DIR/HEAD\""),
    ] {
        let temp = preserved_linear_fixture(false)?;
        let root = temp.path();
        let hook = root.join(".git/hooks/post-rewrite");
        fs::write(&hook, format!("#!/bin/sh\n{script}\n"))?;
        fs::set_permissions(hook, fs::Permissions::from_mode(0o700))?;
        // Retain this repository handle: policy validation must also detect changed on-disk config.
        let repo = gix::open(root)?;
        let mut session = Session::prepare(&repo, b"upstream".as_bstr())?.unwrap();
        advance_to_ready(&mut session, root)?;
        let tip = session.tip()?;
        let before = fs::read(root.join(target))?;
        assert!(session.finish(&repo).is_err(), "successful hook modified {target}");
        assert_eq!(oid(root, "main")?, tip, "durable final publication remains committed");
        let changed = fs::read(root.join(target))?;
        assert_ne!(changed, before, "fixture hook must actually change {target}");
        assert_ne!(
            session.phase(),
            "completed",
            "changed state needs explicit recovery diagnostics"
        );
        assert!(session.cleanup(&gix::open(root)?).is_err());
        assert!(session.abort(&gix::open(root)?).is_err());
        assert_eq!(
            fs::read(root.join(target))?,
            changed,
            "refusal cannot roll back hook effects"
        );
    }
    Ok(())
}

#[test]
fn abort_preserves_unrelated_work_modified_after_replay_started() -> Result<()> {
    if gix_testtools::run_in_isolated_process().map_err(anyhow::Error::from_boxed)? {
        return Ok(());
    }
    let temp = fixture()?;
    let root = temp.path();
    commit(root, "outside", b"common clean outside\n", "common outside")?;
    git(root, &["switch", "-qc", "upstream"])?;
    commit(root, "upstream", b"upstream\n", "upstream")?;
    git(root, &["switch", "-q", "main"])?;
    commit(root, "selected", b"topic work\n", "topic")?;
    let original = integration_snapshot(root)?;
    let mut session = Session::prepare(&gix::open(root)?, b"upstream".as_bstr())?.unwrap();
    advance_to_ready(&mut session, root)?;
    fs::write(root.join("outside"), b"new unrelated work during replay\n")?;
    session.abort(&gix::open(root)?)?;
    assert_eq!(
        integration_snapshot(root)?,
        original,
        "owned integration restores exactly"
    );
    assert_eq!(fs::read(root.join("outside"))?, b"new unrelated work during replay\n");
    assert_eq!(git(root, &["show", ":outside"])?, b"common clean outside\n");
    Ok(())
}

#[cfg(feature = "switch-test-faults")]
#[test]
fn interrupted_step_reuses_exact_proposed_commit_bytes_and_committer_time() -> Result<()> {
    if gix_testtools::run_in_isolated_process().map_err(anyhow::Error::from_boxed)? {
        return Ok(());
    }
    for checkpoint in ["replay-retention-recipe", "replay-intermediate-retained"] {
        let temp = divergent_fixture(false)?;
        let root = temp.path();
        kill_at_lifecycle_checkpoint(root, checkpoint, "finish-cleanup")?;
        let repo = gix::open(root)?;
        let mut session = Session::open(&repo)?;
        let envelope: serde_json::Value =
            serde_json::from_slice(&fs::read(session.directory().join("proposal-0.json"))?)?;
        let payload: Vec<u8> = serde_json::from_value(envelope["payload"].clone())?;
        let (proposed, bytes): (String, Vec<u8>) = serde_json::from_slice(&payload)?;
        {
            let _time = gix_testtools::Env::new().set("GIT_COMMITTER_DATE", "1987654321 +0930");
            assert!(
                session.advance(&gix::open(root)?).is_err(),
                "changed date invalidates pinned policy"
            );
        }
        // Restoring the pinned policy resumes the exact saved bytes and timestamp.
        let Advance::Step { commit } = session.advance(&gix::open(root)?)? else {
            anyhow::bail!("interrupted proposal should resume its step");
        };
        assert_eq!(commit.to_string(), proposed);
        assert_eq!(git(root, &["cat-file", "commit", &proposed])?, bytes);
    }
    Ok(())
}

#[cfg(feature = "switch-test-faults")]
#[test]
fn durable_decision_refuses_third_state_work_index_head_and_replaced_lock() -> Result<()> {
    if gix_testtools::run_in_isolated_process().map_err(anyhow::Error::from_boxed)? {
        return Ok(());
    }
    for resource in ["selected", ".git/index", ".git/HEAD", ".git/index.lock"] {
        let temp = divergent_fixture(false)?;
        let root = temp.path();
        kill_at_lifecycle_checkpoint(root, "replay-decision-durable", "finish-cleanup")?;
        let repo = gix::open(root)?;
        let path = root.join(resource);
        if resource.ends_with(".lock") && path.exists() {
            fs::remove_file(&path)?;
        }
        fs::write(&path, b"foreign third state\n")?;
        let before = integration_snapshot(root)?;
        assert!(Session::recover_decision(&repo).is_err(), "refuse {resource}");
        assert_eq!(
            integration_snapshot(root)?,
            before,
            "no publication on {resource} refusal"
        );
        assert_eq!(fs::read(&path)?, b"foreign third state\n");
    }
    Ok(())
}

#[test]
fn accepted_child_archive_requires_every_decision_and_canonical_member_names() -> Result<()> {
    if gix_testtools::run_in_isolated_process().map_err(anyhow::Error::from_boxed)? {
        return Ok(());
    }
    for damage in ["missing-first", "noncanonical-child", "noncanonical-invocation"] {
        let temp = preserved_linear_fixture(true)?;
        let root = temp.path();
        let repo = gix::open(root)?;
        let mut session = Session::prepare(&repo, b"upstream".as_bstr())?.unwrap();
        assert_eq!(session.advance(&repo)?, Advance::Started);
        assert_eq!(session.advance(&repo)?, Advance::Conflict);
        for bytes in [b"first resolution\n".as_slice(), b"second resolution\n".as_slice()] {
            fs::write(root.join("selected"), bytes)?;
            session.stage(&repo, &["selected".into()], false)?;
        }
        assert!(matches!(session.continue_conflict(&repo)?, Advance::Step { .. }));
        session.abort(&repo)?;
        let archives = fs::read_dir(session.directory())?
            .filter_map(|entry| entry.ok())
            .filter(|entry| {
                entry.path().is_dir()
                    && entry.file_name().to_string_lossy().starts_with("child-")
                    && entry.file_name().to_string_lossy().ends_with("-accepted")
            })
            .map(|entry| entry.path())
            .collect::<Vec<_>>();
        assert_eq!(archives.len(), 1, "one accepted child archive");
        let archive = archives[0].clone();
        assert!(archive.join("child-1.json").is_file());
        assert!(archive.join("child-2.json").is_file());
        match damage {
            "missing-first" => fs::remove_file(archive.join("child-1.json"))?,
            "noncanonical-child" => fs::rename(archive.join("child-1.json"), archive.join("child-01.json"))?,
            "noncanonical-invocation" => fs::create_dir(archive.join("invocation-01"))?,
            _ => unreachable!(),
        }
        let refs = git(
            root,
            &[
                "for-each-ref",
                "--format=%(refname) %(objectname)",
                "refs/brit-rebase-retain/",
            ],
        )?;
        let before = snapshot(&archive)?;
        assert!(
            session.cleanup(&repo).is_err(),
            "refuse damaged accepted archive: {damage}"
        );
        assert_eq!(snapshot(&archive)?, before);
        assert_eq!(
            git(
                root,
                &[
                    "for-each-ref",
                    "--format=%(refname) %(objectname)",
                    "refs/brit-rebase-retain/"
                ]
            )?,
            refs
        );
    }
    Ok(())
}

#[cfg(feature = "switch-test-faults")]
#[test]
fn child_directory_before_recipe_can_abort_with_orphan_evidence_left_for_cleanup() -> Result<()> {
    if gix_testtools::run_in_isolated_process().map_err(anyhow::Error::from_boxed)? {
        return Ok(());
    }
    let temp = preserved_linear_fixture(true)?;
    let root = temp.path();
    let original = integration_snapshot(root)?;
    kill_at_lifecycle_checkpoint(root, "replay-child-preparation-directory-created", "conflict")?;
    let repo = gix::open(root)?;
    let mut session = Session::open(&repo)?;
    let orphans = fs::read_dir(session.directory())?
        .filter_map(|entry| entry.ok())
        .filter(|entry| {
            entry.path().is_dir()
                && entry.file_name().to_string_lossy().starts_with("child-")
                && entry.file_name().to_string_lossy().ends_with("-preparing")
        })
        .map(|entry| entry.path())
        .collect::<Vec<_>>();
    assert_eq!(orphans.len(), 1, "one unaccepted child directory");
    let orphan = orphans[0].clone();
    let name = orphan.file_name().expect("child directory name").to_string_lossy();
    let recipe = format!(
        "{}-prepare.json",
        name.strip_suffix("-preparing").expect("child preparation suffix")
    );
    assert!(!session.directory().join(recipe).exists());
    fs::write(orphan.join("interrupted-evidence"), b"unaccepted child evidence\n")?;
    let before = snapshot(&orphan)?;
    let receipt = session.abort(&repo)?;
    assert_eq!(integration_snapshot(root)?, original);
    gitoxide_core::repository::replay::retained::inspect(&receipt.path)?;
    assert_eq!(snapshot(&orphan)?, before);
    assert!(
        session.cleanup(&repo).is_err(),
        "unknown orphan inventory requires inspection"
    );
    assert_eq!(snapshot(&orphan)?, before);
    Ok(())
}

#[cfg(feature = "switch-test-faults")]
#[test]
fn retired_cleanup_can_finish_without_touching_a_new_active_replay() -> Result<()> {
    use std::os::unix::fs::MetadataExt;
    if gix_testtools::run_in_isolated_process().map_err(anyhow::Error::from_boxed)? {
        return Ok(());
    }
    let temp = preserved_linear_fixture(false)?;
    let root = temp.path();
    kill_at_lifecycle_checkpoint(root, "replay-cleanup-tombstoned", "abort-cleanup")?;
    let repo = gix::open(root)?;
    let receipt = cleanup_receipt(&repo)?;
    assert!(receipt.to_string_lossy().ends_with(".retired.json"));
    drop(gitoxide_core::repository::switch::mutation_guard(&repo)?);
    let new_session = Session::prepare(&repo, b"upstream".as_bstr())?.expect("new replay after retirement");
    let active_before = snapshot(new_session.directory())?;
    let active_metadata = fs::metadata(new_session.directory())?;
    let active_identity = (active_metadata.dev(), active_metadata.ino());
    let integration_before = integration_snapshot(root)?;
    let refs_before = git(
        root,
        &[
            "for-each-ref",
            "--format=%(refname) %(objectname)",
            "refs/brit-rebase-retain/",
        ],
    )?;
    Session::recover_cleanup(&repo, &receipt)?;
    assert!(!receipt.exists());
    assert_eq!(snapshot(new_session.directory())?, active_before);
    let active_metadata = fs::metadata(new_session.directory())?;
    assert_eq!((active_metadata.dev(), active_metadata.ino()), active_identity);
    assert_eq!(integration_snapshot(root)?, integration_before);
    assert_eq!(
        git(
            root,
            &[
                "for-each-ref",
                "--format=%(refname) %(objectname)",
                "refs/brit-rebase-retain/"
            ]
        )?,
        refs_before
    );
    Session::open(&repo)?.abort(&repo)?;
    Ok(())
}

#[cfg(feature = "switch-test-faults")]
#[test]
fn abort_completes_pending_nonfinal_decision_then_restores_original_integration() -> Result<()> {
    if gix_testtools::run_in_isolated_process().map_err(anyhow::Error::from_boxed)? {
        return Ok(());
    }
    for checkpoint in ["replay-decision-durable", "replay-worktree-publication"] {
        let temp = preserved_linear_fixture(false)?;
        let root = temp.path();
        let original = integration_snapshot(root)?;
        kill_at_lifecycle_checkpoint(root, checkpoint, "finish-cleanup")?;
        let repo = gix::open(root)?;
        let receipt = Session::open(&repo)?.abort(&repo)?;
        assert_eq!(integration_snapshot(root)?, original);
        gitoxide_core::repository::replay::retained::inspect(&receipt.path)?;
        assert_eq!(Session::open(&repo)?.phase(), "aborted");
    }
    Ok(())
}

#[cfg(feature = "switch-test-faults")]
#[test]
fn unowned_transition_preparation_after_completed_step_preserves_abort_and_orphan_evidence() -> Result<()> {
    use std::os::unix::fs::MetadataExt;
    if gix_testtools::run_in_isolated_process().map_err(anyhow::Error::from_boxed)? {
        return Ok(());
    }
    for checkpoint in [
        "replay-transition-directory-created",
        "replay-transition-directory-allocated",
    ] {
        let temp = divergent_fixture(false)?;
        let root = temp.path();
        fs::create_dir_all(root.join("new/deep"))?;
        commit(
            root,
            "new/deep/file",
            b"second replay nested file\n",
            "second nested replay",
        )?;
        git(root, &["rm", "-qr", "new"])?;
        git(root, &["commit", "-qm", "third replay removes intermediate directory"])?;
        let original = integration_snapshot(root)?;
        assert!(!root.join("new").exists());
        kill_at_lifecycle_checkpoint(root, checkpoint, "after-first-step")?;
        let repo = gix::open(root)?;
        let mut session = Session::open(&repo)?;
        assert_eq!(session.completed_steps(), 1, "fault occurs after a completed replay");
        let orphans = fs::read_dir(session.directory())?
            .filter_map(|entry| entry.ok())
            .filter(|entry| entry.file_name().to_string_lossy().starts_with("transition-3-") && entry.path().is_dir())
            .map(|entry| entry.path())
            .collect::<Vec<_>>();
        assert_eq!(orphans.len(), 1);
        let orphan = &orphans[0];
        let metadata = fs::metadata(orphan)?;
        let identity = (metadata.dev(), metadata.ino());
        let members = snapshot(orphan)?;
        let allocated = fs::read_dir(orphan)?
            .filter_map(|entry| entry.ok())
            .filter(|entry| entry.path().is_dir())
            .map(|entry| {
                let metadata = fs::metadata(entry.path())?;
                Ok((entry.path(), (metadata.dev(), metadata.ino())))
            })
            .collect::<Result<Vec<_>>>()?;
        if checkpoint.ends_with("created") {
            assert!(!orphan.join("owner").exists());
        } else {
            assert!(
                fs::read_dir(orphan)?
                    .filter_map(|entry| entry.ok())
                    .any(|entry| entry.file_name().to_string_lossy().starts_with("new-dir-"))
            );
        }
        let receipt = session.abort(&repo)?;
        assert_eq!(integration_snapshot(root)?, original);
        assert!(
            !root.join("new").exists(),
            "abort restores original tree without the intermediate directory"
        );
        gitoxide_core::repository::replay::retained::inspect(&receipt.path)?;
        assert_eq!(snapshot(orphan)?, members, "unproven orphan is not adopted or removed");
        let metadata = fs::metadata(orphan)?;
        assert_eq!((metadata.dev(), metadata.ino()), identity);
        for (path, expected) in allocated {
            let metadata = fs::metadata(path)?;
            assert_eq!((metadata.dev(), metadata.ino()), expected);
        }
        assert!(
            session.cleanup(&repo).is_err(),
            "unproven orphan leaves cleanup pending"
        );
        assert_eq!(snapshot(orphan)?, members);
    }
    Ok(())
}

#[cfg(feature = "switch-test-faults")]
#[test]
fn abort_retry_after_its_own_unowned_staging_retains_orphan_without_replacing_prior_copy() -> Result<()> {
    use gitoxide_core::repository::replay::retained;
    use std::os::unix::fs::MetadataExt;
    if gix_testtools::run_in_isolated_process().map_err(anyhow::Error::from_boxed)? {
        return Ok(());
    }
    for interrupt_sequence_save in [false, true] {
        let temp = preserved_linear_fixture(false)?;
        let root = temp.path();
        let original = integration_snapshot(root)?;
        kill_at_lifecycle_checkpoint(root, "replay-transition-directory-created", "abort-after-first-step")?;
        let repo = gix::open(root)?;
        let mut session = Session::open(&repo)?;
        assert_eq!(session.completed_steps(), 1);
        let orphan = fs::read_dir(session.directory())?
            .filter_map(|entry| entry.ok())
            .find(|entry| {
                entry.path().is_dir()
                    && entry.file_name().to_string_lossy().starts_with("transition-3-")
                    && !entry.path().join("owner").exists()
            })
            .expect("abort staging directory before its ownership record")
            .path();
        let metadata = fs::metadata(&orphan)?;
        let identity = (metadata.dev(), metadata.ino());
        let mut prior_copies = Vec::new();
        for operation in fs::read_dir(repo.git_dir().join("brit-rebase-retained"))? {
            for copy in fs::read_dir(operation?.path())? {
                let path = copy?.path();
                retained::inspect(&path)?;
                prior_copies.push((path.clone(), snapshot(&path)?));
            }
        }
        assert!(
            !prior_copies.is_empty(),
            "abort retained work before staging allocation"
        );
        let expected_work = fs::read(root.join("selected"))?;
        fs::write(root.join("selected"), b"foreign work during abort preparation\n")?;
        assert!(
            session.abort(&repo).is_err(),
            "abort preparation is not an authoring stop"
        );
        assert_eq!(
            fs::read(root.join("selected"))?,
            b"foreign work during abort preparation\n"
        );
        fs::write(root.join("selected"), expected_work)?;
        session = Session::open(&repo)?;
        fs::write(
            orphan.join("unproven-evidence"),
            b"new orphan evidence after interrupted abort\n",
        )?;
        let orphan_before = snapshot(&orphan)?;
        if interrupt_sequence_save {
            kill_at_lifecycle_checkpoint(root, "replay-state-file-synced", "retry-abort")?;
            session = Session::open(&repo)?;
        }
        let receipt = session.abort(&repo)?;
        assert_eq!(integration_snapshot(root)?, original);
        for (path, bytes) in prior_copies {
            assert_ne!(path, receipt.path, "new observation requires a fresh retained leaf");
            assert_eq!(snapshot(&path)?, bytes, "prior completed copies are immutable");
        }
        let copy = retained::inspect(&receipt.path)?;
        let mut covered = false;
        for ordinal in 0..copy.manifest.entries.len() {
            covered |= copy.payload(ordinal)? == b"new orphan evidence after interrupted abort\n";
        }
        assert!(
            covered,
            "retry copy covers newly observed orphan evidence before restoration"
        );
        assert_eq!(snapshot(&orphan)?, orphan_before);
        let metadata = fs::metadata(&orphan)?;
        assert_eq!((metadata.dev(), metadata.ino()), identity);
        assert!(session.cleanup(&repo).is_err());
    }
    Ok(())
}
