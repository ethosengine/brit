//! Native edit-stop primitives: no public rebase command or replay sequencer.
#![cfg(target_os = "linux")]

use std::{fs, path::Path};

use anyhow::{Result, ensure};
use gitoxide_core::repository::authoring::conflict::{Session, StopKind};
use gix::bstr::BString;

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
    git(root, &["config", "user.name", "Author"])?;
    git(root, &["config", "user.email", "author@example.invalid"])?;
    fs::write(root.join("selected"), b"base\n")?;
    fs::write(root.join("neighbor"), b"base neighbor\n")?;
    git(root, &["add", "."])?;
    git(root, &["commit", "-qm", "parent"])?;
    fs::write(root.join("selected"), b"original step\n")?;
    git(root, &["add", "selected"])?;
    git(root, &["commit", "-qm", "step"])?;
    git(root, &["switch", "--detach"])?;
    Ok(temp)
}

fn active() -> Vec<BString> {
    vec!["selected".into()]
}

fn dirty_neighbor(root: &Path) -> Result<Vec<u8>> {
    fs::write(root.join("neighbor"), b"staged private\n")?;
    git(root, &["add", "neighbor"])?;
    fs::write(root.join("neighbor"), b"unstaged private\n")?;
    git(root, &["ls-files", "--stage", "-z", "--", "neighbor"])
}

#[test]
fn amendment_excludes_simultaneous_staged_and_unstaged_preserved_edits() -> Result<()> {
    if gix_testtools::run_in_isolated_process().map_err(anyhow::Error::from_boxed)? {
        return Ok(());
    }
    let temp = fixture()?;
    let root = temp.path();
    let preserved = dirty_neighbor(root)?;
    let repo = gix::open(root)?;
    let mut session = Session::prepare(&repo, StopKind::Edit, &active())?;
    fs::write(root.join("selected"), b"amended\n")?;
    session.stage(&repo, &active(), false)?;
    let outcome = session.amend(&repo, "amended message")?;
    assert_eq!(
        git(root, &["rev-parse", "HEAD"])?,
        format!("{}\n", outcome.commit).as_bytes()
    );
    assert_eq!(git(root, &["show", "HEAD:neighbor"])?, b"base neighbor\n");
    assert_eq!(git(root, &["show", "HEAD:selected"])?, b"amended\n");
    assert_eq!(git(root, &["ls-files", "--stage", "-z", "--", "neighbor"])?, preserved);
    assert_eq!(fs::read(root.join("neighbor"))?, b"unstaged private\n");
    assert!(
        session.directory().is_dir(),
        "before-images remain available after child publication"
    );
    Ok(())
}

#[test]
fn external_staging_imports_only_the_allowed_resolution_set() -> Result<()> {
    if gix_testtools::run_in_isolated_process().map_err(anyhow::Error::from_boxed)? {
        return Ok(());
    }
    let temp = fixture()?;
    let root = temp.path();
    let preserved = dirty_neighbor(root)?;
    let repo = gix::open(root)?;
    let mut session = Session::prepare(&repo, StopKind::Edit, &active())?;
    fs::write(root.join("selected"), b"external resolution\n")?;
    git(root, &["add", "selected"])?;
    session.import_external(&repo)?;
    session.amend(&repo, "external resolution")?;
    assert_eq!(git(root, &["show", "HEAD:selected"])?, b"external resolution\n");
    assert_eq!(git(root, &["show", "HEAD:neighbor"])?, b"base neighbor\n");
    assert_eq!(git(root, &["ls-files", "--stage", "-z", "--", "neighbor"])?, preserved);
    Ok(())
}

#[test]
fn external_staging_outside_scope_refuses_without_publishing() -> Result<()> {
    if gix_testtools::run_in_isolated_process().map_err(anyhow::Error::from_boxed)? {
        return Ok(());
    }
    let temp = fixture()?;
    let root = temp.path();
    let repo = gix::open(root)?;
    let mut session = Session::prepare(&repo, StopKind::Edit, &active())?;
    let head = git(root, &["rev-parse", "HEAD"])?;
    fs::write(root.join("neighbor"), b"unauthorized\n")?;
    git(root, &["add", "neighbor"])?;
    let index = fs::read(repo.index_path())?;
    assert!(session.import_external(&repo).is_err());
    assert_eq!(git(root, &["rev-parse", "HEAD"])?, head);
    assert_eq!(
        fs::read(repo.index_path())?,
        index,
        "refusal retains the user's external staging"
    );
    assert_eq!(fs::read(root.join("neighbor"))?, b"unauthorized\n");
    Ok(())
}

#[test]
fn external_split_index_refuses_import_without_rewriting_user_state() -> Result<()> {
    if gix_testtools::run_in_isolated_process().map_err(anyhow::Error::from_boxed)? {
        return Ok(());
    }
    let temp = fixture()?;
    let root = temp.path();
    let repo = gix::open(root)?;
    let mut session = Session::prepare(&repo, StopKind::Edit, &active())?;
    git(root, &["update-index", "--split-index"])?;
    let index = fs::read(repo.index_path())?;
    let head = fs::read(repo.git_dir().join("HEAD"))?;
    assert!(
        index.windows(4).any(|bytes| bytes == b"link"),
        "fixture contains split-index extension"
    );
    assert!(session.import_external(&repo).is_err());
    assert_eq!(fs::read(repo.index_path())?, index);
    assert_eq!(fs::read(repo.git_dir().join("HEAD"))?, head);
    assert_eq!(fs::read(root.join("selected"))?, b"original step\n");
    Ok(())
}

#[test]
fn symlinked_reflog_directory_refuses_before_any_local_or_external_publication() -> Result<()> {
    use std::os::unix::fs::symlink;
    if gix_testtools::run_in_isolated_process().map_err(anyhow::Error::from_boxed)? {
        return Ok(());
    }
    for before_prepare in [false, true] {
        let temp = fixture()?;
        let root = temp.path();
        let external = tempfile::tempdir()?;
        let repo = gix::open(root)?;
        let mut session = if before_prepare {
            None
        } else {
            Some(Session::prepare(&repo, StopKind::Edit, &active())?)
        };
        let redirected = external.path().join("logs");
        fs::rename(repo.git_dir().join("logs"), &redirected)?;
        symlink(&redirected, repo.git_dir().join("logs"))?;
        let external_head_log = fs::read(redirected.join("HEAD"))?;
        let external_branch_log = fs::read(redirected.join("refs/heads/main"))?;
        let head = fs::read(repo.git_dir().join("HEAD"))?;
        let index = fs::read(repo.index_path())?;
        if !before_prepare {
            fs::write(root.join("selected"), b"draft protected from redirected logs\n")?;
        }
        let working = fs::read(root.join("selected"))?;
        if let Some(session) = session.as_mut() {
            assert!(session.amend(&repo, "must refuse redirected logs").is_err());
            assert!(session.abort(&repo).is_err());
        } else {
            assert!(Session::prepare(&repo, StopKind::Edit, &active()).is_err());
        }
        assert_eq!(fs::read(repo.git_dir().join("HEAD"))?, head);
        assert_eq!(fs::read(repo.index_path())?, index);
        assert_eq!(fs::read(root.join("selected"))?, working);
        assert_eq!(fs::read(redirected.join("HEAD"))?, external_head_log);
        assert_eq!(fs::read(redirected.join("refs/heads/main"))?, external_branch_log);
        assert_eq!(fs::read_link(repo.git_dir().join("logs"))?, redirected);
    }
    Ok(())
}

#[test]
fn overlapping_preservation_and_ancestor_paths_refuse_before_journal_creation() -> Result<()> {
    if gix_testtools::run_in_isolated_process().map_err(anyhow::Error::from_boxed)? {
        return Ok(());
    }
    for path in ["neighbor", "neighbor/child"] {
        let temp = fixture()?;
        dirty_neighbor(temp.path())?;
        let repo = gix::open(temp.path())?;
        let before = fs::read(repo.index_path())?;
        assert!(
            Session::prepare(&repo, StopKind::Edit, &[path.into()]).is_err(),
            "collision {path}"
        );
        assert_eq!(fs::read(repo.index_path())?, before);
        assert_eq!(fs::read(temp.path().join("neighbor"))?, b"unstaged private\n");
    }
    let temp = fixture()?;
    let root = temp.path();
    fs::create_dir(root.join("folder"))?;
    fs::write(root.join("folder/child"), b"tracked\n")?;
    git(root, &["add", "folder/child"])?;
    git(root, &["commit", "-qm", "nested baseline"])?;
    fs::write(root.join("folder/child"), b"private\n")?;
    let repo = gix::open(root)?;
    assert!(Session::prepare(&repo, StopKind::Edit, &["folder".into()]).is_err());
    Ok(())
}

#[test]
fn split_builds_a_linear_chain_without_preserved_staging() -> Result<()> {
    if gix_testtools::run_in_isolated_process().map_err(anyhow::Error::from_boxed)? {
        return Ok(());
    }
    let temp = fixture()?;
    let root = temp.path();
    let preserved = dirty_neighbor(root)?;
    let parent = git(root, &["rev-parse", "HEAD^"])?;
    let repo = gix::open(root)?;
    let mut session = Session::prepare(&repo, StopKind::Edit, &active())?;
    session.begin_split(&repo)?;
    assert_eq!(git(root, &["rev-parse", "HEAD"])?, parent);
    fs::write(root.join("selected"), b"first half\n")?;
    session.stage(&repo, &active(), false)?;
    assert!(
        session.validate_completion(&repo).is_err(),
        "clean staged bytes still require a child commit"
    );
    let first = session.commit_split(&repo, "first")?;
    session.validate_completion(&repo)?;
    fs::write(root.join("selected"), b"second half\n")?;
    assert!(
        session.validate_completion(&repo).is_err(),
        "unstaged remainder prevents edit-stop completion"
    );
    session.stage(&repo, &active(), false)?;
    let second = session.commit_split(&repo, "second")?;
    session.validate_completion(&repo)?;
    assert_eq!(
        git(root, &["rev-parse", "HEAD^"])?,
        format!("{}\n", first.commit).as_bytes()
    );
    assert_eq!(
        git(root, &["rev-parse", "HEAD"])?,
        format!("{}\n", second.commit).as_bytes()
    );
    assert_eq!(git(root, &["show", "HEAD:neighbor"])?, b"base neighbor\n");
    assert_eq!(git(root, &["ls-files", "--stage", "-z", "--", "neighbor"])?, preserved);
    assert_eq!(fs::read(root.join("neighbor"))?, b"unstaged private\n");
    Ok(())
}

#[test]
fn split_can_commit_one_selected_path_while_the_other_part_remains_unstaged() -> Result<()> {
    if gix_testtools::run_in_isolated_process().map_err(anyhow::Error::from_boxed)? {
        return Ok(());
    }
    let temp = fixture()?;
    let root = temp.path();
    fs::write(root.join("second-part"), b"second original part\n")?;
    git(root, &["add", "second-part"])?;
    git(root, &["commit", "--amend", "--no-edit"])?;
    dirty_neighbor(root)?;
    let repo = gix::open(root)?;
    let mut session = Session::prepare(&repo, StopKind::Edit, &["selected".into(), "second-part".into()])?;
    session.begin_split(&repo)?;
    session.stage(&repo, &active(), false)?;
    let first = session.commit_split(&repo, "first selected part")?;
    let absent = gix_testtools::git_command(root)
        .args(["cat-file", "-e", "HEAD:second-part"])
        .output()?;
    assert!(!absent.status.success(), "first child includes only its scoped staging");
    assert_eq!(fs::read(root.join("second-part"))?, b"second original part\n");
    assert!(session.validate_completion(&repo).is_err());
    session.stage(&repo, &["second-part".into()], false)?;
    session.commit_split(&repo, "remaining part")?;
    session.validate_completion(&repo)?;
    assert_eq!(
        git(root, &["rev-parse", "HEAD^"])?,
        format!("{}\n", first.commit).as_bytes()
    );
    assert_eq!(git(root, &["show", "HEAD:second-part"])?, b"second original part\n");
    assert_eq!(git(root, &["show", "HEAD:neighbor"])?, b"base neighbor\n");
    assert_eq!(fs::read(root.join("neighbor"))?, b"unstaged private\n");
    Ok(())
}

#[test]
fn external_head_movement_is_not_adopted_as_child_authoring() -> Result<()> {
    if gix_testtools::run_in_isolated_process().map_err(anyhow::Error::from_boxed)? {
        return Ok(());
    }
    let temp = fixture()?;
    let root = temp.path();
    let repo = gix::open(root)?;
    let mut session = Session::prepare(&repo, StopKind::Edit, &active())?;
    git(root, &["commit", "--allow-empty", "-qm", "foreign"])?;
    let foreign = git(root, &["rev-parse", "HEAD"])?;
    assert!(session.validate(&repo).is_err());
    assert!(session.amend(&repo, "must refuse").is_err());
    assert_eq!(git(root, &["rev-parse", "HEAD"])?, foreign);
    Ok(())
}

#[test]
fn preserved_deletion_modes_symlinks_and_non_utf8_paths_never_enter_amendment() -> Result<()> {
    use std::{
        ffi::OsString,
        os::unix::{
            ffi::OsStringExt,
            fs::{PermissionsExt, symlink},
        },
    };
    if gix_testtools::run_in_isolated_process().map_err(anyhow::Error::from_boxed)? {
        return Ok(());
    }
    let temp = fixture()?;
    let root = temp.path();
    let byte_path = OsString::from_vec(b"private-\xff\n".to_vec());
    fs::write(root.join(&byte_path), b"base bytes\n")?;
    fs::write(root.join("deleted"), b"base deleted\n")?;
    fs::write(root.join("mode"), b"mode bytes\n")?;
    symlink("original-target", root.join("link"))?;
    git(root, &["add", "."])?;
    git(root, &["commit", "-qm", "preserved shapes baseline"])?;
    let baseline_tree = git(root, &["rev-parse", "HEAD^{tree}"])?;
    fs::write(root.join(&byte_path), b"staged bytes\n")?;
    fs::remove_file(root.join("deleted"))?;
    fs::set_permissions(root.join("mode"), fs::Permissions::from_mode(0o755))?;
    fs::remove_file(root.join("link"))?;
    symlink("staged-target", root.join("link"))?;
    git(root, &["add", "-A"])?;
    fs::write(root.join(&byte_path), b"unstaged bytes\n")?;
    fs::remove_file(root.join("link"))?;
    symlink("unstaged-target", root.join("link"))?;
    let before_entries = git(root, &["ls-files", "--stage", "-z"])?;
    let repo = gix::open(root)?;
    let before_index = fs::read(repo.index_path())?;
    let mut session = Session::prepare(&repo, StopKind::Edit, &active())?;
    session.amend(&repo, "message only")?;
    assert_eq!(git(root, &["rev-parse", "HEAD^{tree}"])?, baseline_tree);
    assert_eq!(git(root, &["ls-files", "--stage", "-z"])?, before_entries);
    assert_eq!(fs::read(root.join(&byte_path))?, b"unstaged bytes\n");
    assert!(!root.join("deleted").exists());
    assert_ne!(fs::metadata(root.join("mode"))?.permissions().mode() & 0o111, 0);
    assert_eq!(fs::read_link(root.join("link"))?, Path::new("unstaged-target"));
    assert_eq!(fs::read(session.directory().join("index.original"))?, before_index);
    Ok(())
}

#[test]
fn explicit_scope_expansion_retains_before_image_and_refuses_preserved_paths() -> Result<()> {
    if gix_testtools::run_in_isolated_process().map_err(anyhow::Error::from_boxed)? {
        return Ok(());
    }
    let temp = fixture()?;
    let root = temp.path();
    dirty_neighbor(root)?;
    let repo = gix::open(root)?;
    let mut session = Session::prepare(&repo, StopKind::Edit, &active())?;
    fs::write(root.join("new"), b"new explicit content\n")?;
    assert!(session.stage(&repo, &["new".into()], false).is_err());
    assert!(session.stage(&repo, &["neighbor".into()], true).is_err());
    session.stage(&repo, &["new".into()], true)?;
    session.amend(&repo, "expanded")?;
    assert_eq!(git(root, &["show", "HEAD:new"])?, b"new explicit content\n");
    assert_eq!(git(root, &["show", "HEAD:neighbor"])?, b"base neighbor\n");
    assert!(session.directory().join("index.original").is_file());
    Ok(())
}

#[test]
fn linked_worktree_owns_its_index_and_rejects_foreign_session_identity() -> Result<()> {
    if gix_testtools::run_in_isolated_process().map_err(anyhow::Error::from_boxed)? {
        return Ok(());
    }
    let temp = fixture()?;
    let root = temp.path();
    dirty_neighbor(root)?;
    let main_repo = gix::open(root)?;
    let main_index = fs::read(main_repo.index_path())?;
    let linked_parent = tempfile::tempdir()?;
    let linked = linked_parent.path().join("linked");
    git(
        root,
        &["worktree", "add", "--detach", linked.to_str().expect("temporary path")],
    )?;
    let repo = gix::open(&linked)?;
    let mut session = Session::prepare(&repo, StopKind::Edit, &active())?;
    assert!(session.directory().starts_with(repo.git_dir()));
    assert!(
        session.validate(&main_repo).is_err(),
        "common object storage does not confer worktree ownership"
    );
    fs::write(linked.join("selected"), b"linked amendment\n")?;
    session.stage(&repo, &active(), false)?;
    session.amend(&repo, "linked")?;
    assert_eq!(fs::read(main_repo.index_path())?, main_index);
    assert_eq!(fs::read(root.join("selected"))?, b"original step\n");
    assert_eq!(fs::read(root.join("neighbor"))?, b"unstaged private\n");
    Ok(())
}

fn hook(root: &Path, name: &str, body: &str) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let path = root.join(".git/hooks").join(name);
    fs::write(&path, format!("#!/bin/sh\n{body}\n"))?;
    fs::set_permissions(path, fs::Permissions::from_mode(0o755))?;
    Ok(())
}

#[test]
fn pre_hook_failure_publishes_nothing_and_post_hook_failure_reports_committed_result() -> Result<()> {
    if gix_testtools::run_in_isolated_process().map_err(anyhow::Error::from_boxed)? {
        return Ok(());
    }
    for pre in [true, false] {
        let temp = fixture()?;
        let root = temp.path();
        dirty_neighbor(root)?;
        hook(root, if pre { "pre-commit" } else { "post-commit" }, "exit 17")?;
        let repo = gix::open(root)?;
        let mut session = Session::prepare(&repo, StopKind::Edit, &active())?;
        let before = git(root, &["rev-parse", "HEAD"])?;
        fs::write(root.join("selected"), b"hook candidate\n")?;
        session.stage(&repo, &active(), false)?;
        let result = session.amend(&repo, "hook test");
        if pre {
            assert!(result.is_err());
            assert_eq!(git(root, &["rev-parse", "HEAD"])?, before);
        } else {
            let outcome = result?;
            assert!(
                !outcome.warnings.is_empty(),
                "a committed failure must remain observable"
            );
            assert_ne!(git(root, &["rev-parse", "HEAD"])?, before);
            assert_eq!(git(root, &["show", "HEAD:selected"])?, b"hook candidate\n");
        }
        assert_eq!(fs::read(root.join("neighbor"))?, b"unstaged private\n");
    }
    Ok(())
}

#[test]
fn authoring_hooks_receive_private_index_and_out_of_scope_mutations_refuse() -> Result<()> {
    if gix_testtools::run_in_isolated_process().map_err(anyhow::Error::from_boxed)? {
        return Ok(());
    }
    for mutate in [false, true] {
        let temp = fixture()?;
        let root = temp.path();
        dirty_neighbor(root)?;
        hook(
            root,
            "pre-commit",
            if mutate {
                "printf 'hook changed neighbor\\n' > neighbor\ngit add neighbor"
            } else {
                "test \"$(git show :neighbor)\" = 'base neighbor'\ncase \"$GIT_INDEX_FILE\" in */brit-authoring/index.private) ;; *) exit 23;; esac"
            },
        )?;
        let repo = gix::open(root)?;
        let mut session = Session::prepare(&repo, StopKind::Edit, &active())?;
        let before = git(root, &["rev-parse", "HEAD"])?;
        let result = session.amend(&repo, "hook index");
        if mutate {
            assert!(result.is_err());
            assert_eq!(git(root, &["rev-parse", "HEAD"])?, before);
            assert_eq!(
                fs::read(root.join("neighbor"))?,
                b"hook changed neighbor\n",
                "rejected hook edits are retained"
            );
        } else {
            result?;
            assert_eq!(git(root, &["show", "HEAD:neighbor"])?, b"base neighbor\n");
        }
    }
    Ok(())
}

#[test]
fn conflict_stages_are_published_then_replaced_by_scoped_resolution() -> Result<()> {
    use gix::bstr::ByteSlice;
    if gix_testtools::run_in_isolated_process().map_err(anyhow::Error::from_boxed)? {
        return Ok(());
    }
    let temp = fixture()?;
    let root = temp.path();
    let preserved = dirty_neighbor(root)?;
    let repo = gix::open(root)?;
    let mut session = Session::prepare(&repo, StopKind::Conflict, &active())?;
    let private = gix::index::File::at(
        session.directory().join("index.private"),
        repo.object_hash(),
        false,
        Default::default(),
    )?;
    let mut step = (*private).clone();
    step.remove_entries(|_, path, _| path == b"selected".as_bstr());
    for (stage, content) in [
        (gix::index::entry::Stage::Base, b"base\n".as_slice()),
        (gix::index::entry::Stage::Ours, b"upstream\n".as_slice()),
        (gix::index::entry::Stage::Theirs, b"replayed\n".as_slice()),
    ] {
        let blob = repo.write_blob(content)?;
        step.dangerously_push_entry(
            Default::default(),
            blob.detach(),
            gix::index::entry::Flags::from_stage(stage),
            gix::index::entry::Mode::FILE,
            b"selected".as_bstr(),
        );
    }
    step.sort_entries();
    session.install_step(&repo, &step)?;
    let unmerged = git(root, &["ls-files", "--unmerged"])?;
    assert_eq!(unmerged.split(|b| *b == b'\n').filter(|row| !row.is_empty()).count(), 3);
    assert!(
        session.private_tree(&repo).is_err(),
        "unmerged stages cannot become a commit tree"
    );
    fs::write(root.join("selected"), b"chosen resolution\n")?;
    session.stage(&repo, &active(), false)?;
    assert!(git(root, &["ls-files", "--unmerged"])?.is_empty());
    let tree = session.private_tree(&repo)?;
    assert_eq!(
        git(root, &["show", &format!("{tree}:selected")])?,
        b"chosen resolution\n"
    );
    assert_eq!(git(root, &["show", &format!("{tree}:neighbor")])?, b"base neighbor\n");
    assert_eq!(git(root, &["ls-files", "--stage", "-z", "--", "neighbor"])?, preserved);
    assert!(session.amend(&repo, "unauthorized conflict amend").is_err());
    Ok(())
}

#[test]
fn unstaged_resolution_and_outside_worktree_changes_refuse_without_discarding_bytes() -> Result<()> {
    if gix_testtools::run_in_isolated_process().map_err(anyhow::Error::from_boxed)? {
        return Ok(());
    }
    for outside in [false, true] {
        let temp = fixture()?;
        let root = temp.path();
        let repo = gix::open(root)?;
        let mut session = Session::prepare(&repo, StopKind::Edit, &active())?;
        let before = git(root, &["rev-parse", "HEAD"])?;
        let changed = if outside { "neighbor" } else { "selected" };
        fs::write(root.join(changed), b"unstaged draft\n")?;
        assert!(session.amend(&repo, "must refuse").is_err());
        assert_eq!(git(root, &["rev-parse", "HEAD"])?, before);
        assert_eq!(fs::read(root.join(changed))?, b"unstaged draft\n");
    }
    Ok(())
}

#[test]
fn hook_staging_is_imported_when_scoped_and_rejected_when_outside_scope() -> Result<()> {
    if gix_testtools::run_in_isolated_process().map_err(anyhow::Error::from_boxed)? {
        return Ok(());
    }
    for scoped in [true, false] {
        let temp = fixture()?;
        let root = temp.path();
        hook(
            root,
            "pre-commit",
            if scoped {
                "printf 'hook resolution\\n' > selected\ngit add selected"
            } else {
                "git update-index --force-remove neighbor"
            },
        )?;
        let repo = gix::open(root)?;
        let mut session = Session::prepare(&repo, StopKind::Edit, &active())?;
        let before = git(root, &["rev-parse", "HEAD"])?;
        let result = session.amend(&repo, "hook staging");
        if scoped {
            result?;
            assert_eq!(git(root, &["show", "HEAD:selected"])?, b"hook resolution\n");
        } else {
            assert!(
                result.is_err(),
                "private-only mutation outside A cannot be silently forgotten"
            );
            assert_eq!(git(root, &["rev-parse", "HEAD"])?, before);
        }
    }
    Ok(())
}

#[test]
fn explicit_scope_expansion_can_accept_a_changed_previously_clean_tracked_path() -> Result<()> {
    if gix_testtools::run_in_isolated_process().map_err(anyhow::Error::from_boxed)? {
        return Ok(());
    }
    let temp = fixture()?;
    let root = temp.path();
    let repo = gix::open(root)?;
    let mut session = Session::prepare(&repo, StopKind::Edit, &active())?;
    fs::write(root.join("neighbor"), b"explicit expansion\n")?;
    session.stage(&repo, &["neighbor".into()], true)?;
    session.amend(&repo, "expanded tracked")?;
    assert_eq!(git(root, &["show", "HEAD:neighbor"])?, b"explicit expansion\n");
    Ok(())
}

#[test]
fn scope_expansion_refuses_a_preexisting_untracked_file() -> Result<()> {
    if gix_testtools::run_in_isolated_process().map_err(anyhow::Error::from_boxed)? {
        return Ok(());
    }
    let temp = fixture()?;
    let root = temp.path();
    fs::write(root.join("private-untracked"), b"preexisting private draft\n")?;
    let repo = gix::open(root)?;
    let mut session = Session::prepare(&repo, StopKind::Edit, &active())?;
    let index = fs::read(repo.index_path())?;
    assert!(session.stage(&repo, &["private-untracked".into()], true).is_err());
    assert_eq!(fs::read(repo.index_path())?, index);
    assert_eq!(
        fs::read(root.join("private-untracked"))?,
        b"preexisting private draft\n"
    );
    Ok(())
}

#[test]
fn amendment_hook_arguments_and_order_preserve_native_authoring_contract() -> Result<()> {
    if gix_testtools::run_in_isolated_process().map_err(anyhow::Error::from_boxed)? {
        return Ok(());
    }
    let temp = fixture()?;
    let root = temp.path();
    hook(
        root,
        "pre-commit",
        "test $# = 0 || exit 1\nprintf 'pre\\n' >> .git/hooks-order",
    )?;
    hook(
        root,
        "prepare-commit-msg",
        "test $# = 3 && test \"$2\" = commit && test \"$3\" = \"$(git rev-parse HEAD)\" || exit 2\ntest -f \"$1\" || exit 3\nprintf 'prepare\\n' >> .git/hooks-order",
    )?;
    hook(
        root,
        "commit-msg",
        "test $# = 1 && test -f \"$1\" || exit 4\nprintf 'message\\n' >> .git/hooks-order",
    )?;
    hook(
        root,
        "post-commit",
        "test $# = 0 || exit 5\nprintf 'post\\n' >> .git/hooks-order",
    )?;
    hook(
        root,
        "post-rewrite",
        "test $# = 1 && test \"$1\" = amend || exit 6\ncat > .git/rewrite-pairs\nprintf 'rewrite\\n' >> .git/hooks-order",
    )?;
    let repo = gix::open(root)?;
    let old = String::from_utf8(git(root, &["rev-parse", "HEAD"])?)?;
    let mut session = Session::prepare(&repo, StopKind::Edit, &active())?;
    let outcome = session.amend(&repo, "hook contract")?;
    assert!(outcome.warnings.is_empty());
    assert_eq!(
        fs::read(repo.git_dir().join("hooks-order"))?,
        b"pre\nprepare\nmessage\npost\nrewrite\n"
    );
    assert_eq!(
        fs::read(repo.git_dir().join("rewrite-pairs"))?,
        format!("{} {}\n", old.trim(), outcome.commit).as_bytes()
    );
    Ok(())
}

#[test]
fn interrupted_child_decision_recovers_old_or_desired_state_and_refuses_third_state() -> Result<()> {
    if gix_testtools::run_in_isolated_process().map_err(anyhow::Error::from_boxed)? {
        return Ok(());
    }
    for state in ["old", "desired", "third-index", "foreign-preserved"] {
        let temp = fixture()?;
        let root = temp.path();
        dirty_neighbor(root)?;
        let repo = gix::open(root)?;
        let mut session = Session::prepare(&repo, StopKind::Edit, &active())?;
        fs::write(root.join("selected"), b"recoverable child\n")?;
        session.stage(&repo, &active(), false)?;
        let old_head = fs::read(repo.git_dir().join("HEAD"))?;
        let old_log = fs::read(repo.git_dir().join("logs/HEAD"))?;
        let state_path = session.directory().join("state.json");
        let original = fs::read(session.directory().join("index.original"))?;
        let outcome = session.amend(&repo, "recoverable")?;
        // Model a crash after the durable child decision but before its state
        // checkpoint. Read the actual publication decision, after any staging
        // child decisions, and preserve its before-state and resource bytes.
        let mut decisions: Vec<_> = fs::read_dir(session.directory())?
            .filter_map(|entry| {
                let entry = entry.ok()?;
                let name = entry.file_name();
                let number = name
                    .to_str()?
                    .strip_prefix("child-")?
                    .strip_suffix(".json")?
                    .parse::<usize>()
                    .ok()?;
                Some((number, entry.path()))
            })
            .collect();
        decisions.sort();
        let envelope: serde_json::Value =
            serde_json::from_slice(&fs::read(&decisions.last().expect("authoring decision").1)?)?;
        let payload: Vec<u8> = serde_json::from_value(envelope["payload"].clone())?;
        let decision: serde_json::Value = serde_json::from_slice(&payload)?;
        let old_index: Vec<u8> = serde_json::from_value(decision["old_index"].clone())?;
        let payload = serde_json::to_vec(&decision["before"])?;
        let mut hash = gix::hash::hasher(gix::hash::Kind::Sha1);
        hash.update(&payload);
        let envelope = serde_json::json!({"payload": payload, "checksum": hash.try_finalize()?.to_string()});
        fs::write(&state_path, serde_json::to_vec(&envelope)?)?;
        if state == "old" {
            fs::write(repo.git_dir().join("HEAD"), old_head)?;
            fs::write(repo.git_dir().join("logs/HEAD"), old_log)?;
            fs::write(repo.index_path(), old_index)?;
        } else if state == "third-index" {
            git(root, &["update-index", "--force-remove", "selected"])?;
        } else if state == "foreign-preserved" {
            fs::write(root.join("neighbor"), b"foreign private draft\n")?;
        }
        let before_head = git(root, &["rev-parse", "HEAD"])?;
        let before_index = fs::read(repo.index_path())?;
        let recovered = Session::recover(&repo);
        if matches!(state, "old" | "desired") {
            recovered?.validate(&repo)?;
            assert_eq!(
                git(root, &["rev-parse", "HEAD"])?,
                format!("{}\n", outcome.commit).as_bytes()
            );
            assert_eq!(git(root, &["show", "HEAD:neighbor"])?, b"base neighbor\n");
        } else {
            assert!(recovered.is_err(), "recovery refuses {state}");
            assert_eq!(git(root, &["rev-parse", "HEAD"])?, before_head);
            assert_eq!(fs::read(repo.index_path())?, before_index);
        }
        assert_eq!(fs::read(session.directory().join("index.original"))?, original);
        assert_eq!(
            fs::read(root.join("neighbor"))?,
            if state == "foreign-preserved" {
                b"foreign private draft\n".as_slice()
            } else {
                b"unstaged private\n".as_slice()
            }
        );
    }
    Ok(())
}

#[test]
fn corrupt_original_index_or_state_checksum_refuses_without_destroying_evidence() -> Result<()> {
    if gix_testtools::run_in_isolated_process().map_err(anyhow::Error::from_boxed)? {
        return Ok(());
    }
    for member in ["index.original", "state.json"] {
        let temp = fixture()?;
        let repo = gix::open(temp.path())?;
        let session = Session::prepare(&repo, StopKind::Edit, &active())?;
        let index = fs::read(repo.index_path())?;
        let head = fs::read(repo.git_dir().join("HEAD"))?;
        let corrupt = session.directory().join(member);
        let mut bytes = fs::read(&corrupt)?;
        bytes[0] ^= 1;
        fs::write(&corrupt, &bytes)?;
        assert!(Session::recover(&repo).is_err());
        assert_eq!(fs::read(repo.index_path())?, index);
        assert_eq!(fs::read(repo.git_dir().join("HEAD"))?, head);
        assert_eq!(fs::read(corrupt)?, bytes, "corruption stays available for inspection");
    }
    Ok(())
}

#[test]
fn abort_retains_child_work_and_exact_index_before_restoring_active_files() -> Result<()> {
    if gix_testtools::run_in_isolated_process().map_err(anyhow::Error::from_boxed)? {
        return Ok(());
    }
    let temp = fixture()?;
    let root = temp.path();
    dirty_neighbor(root)?;
    let repo = gix::open(root)?;
    let index = fs::read(repo.index_path())?;
    let head = fs::read(repo.git_dir().join("HEAD"))?;
    let mut session = Session::prepare(&repo, StopKind::Edit, &active())?;
    fs::write(root.join("selected"), b"retained child draft\n")?;
    session.stage(&repo, &active(), false)?;
    let child = session.amend(&repo, "child to retain")?;
    let commit_bytes = git(root, &["cat-file", "commit", &child.commit.to_string()])?;
    let blob = String::from_utf8(git(root, &["rev-parse", "HEAD:selected"])?)?;
    fs::write(root.join("unrelated-new"), b"new unrelated work\n")?;
    let retained = session.abort(&repo)?;
    assert!(retained.is_dir());
    assert!(
        !repo.git_dir().join("brit-authoring").exists(),
        "retention is not an active blocker"
    );
    assert_eq!(fs::read(repo.index_path())?, index);
    assert_eq!(fs::read(repo.git_dir().join("HEAD"))?, head);
    assert_eq!(fs::read(root.join("selected"))?, b"original step\n");
    assert_eq!(fs::read(root.join("neighbor"))?, b"unstaged private\n");
    assert_eq!(fs::read(root.join("unrelated-new"))?, b"new unrelated work\n");
    assert_eq!(fs::read(retained.join("index.original"))?, index);
    let envelope: serde_json::Value = serde_json::from_slice(&fs::read(retained.join("abort.json"))?)?;
    let payload: Vec<u8> = serde_json::from_value(envelope["payload"].clone())?;
    let receipt: serde_json::Value = serde_json::from_slice(&payload)?;
    let expected = serde_json::to_value(b"retained child draft\n".as_slice())?;
    assert!(
        receipt["changes"]
            .as_array()
            .expect("abort changes")
            .iter()
            .any(|row| row[1]["File"][0] == expected),
        "retained receipt includes exact authored file bytes"
    );
    git(root, &["reflog", "expire", "--expire=now", "--all"])?;
    git(root, &["gc", "--prune=now"])?;
    let pruned = gix_testtools::git_command(root)
        .args(["cat-file", "-e", &child.commit.to_string()])
        .output()?;
    assert!(
        !pruned.status.success(),
        "fixture actually prunes the unreachable authored commit"
    );
    for (id, kind, expected) in [
        (child.commit.to_string(), "commit", commit_bytes),
        (blob.trim().to_owned(), "blob", b"retained child draft\n".to_vec()),
    ] {
        let envelope: serde_json::Value = serde_json::from_slice(&fs::read(retained.join("objects").join(&id))?)?;
        let payload: Vec<u8> = serde_json::from_value(envelope["payload"].clone())?;
        let mut hash = gix::hash::hasher(gix::hash::Kind::Sha1);
        hash.update(&payload);
        assert_eq!(envelope["checksum"], hash.try_finalize()?.to_string());
        let object: serde_json::Value = serde_json::from_slice(&payload)?;
        assert_eq!(object["id"], id);
        assert_eq!(object["kind"], kind);
        let bytes: Vec<u8> = serde_json::from_value(object["data"].clone())?;
        assert_eq!(bytes, expected, "retained object bytes survive real Git pruning");
        if kind == "commit" {
            gix::objs::CommitRef::from_bytes(&bytes, repo.object_hash())?;
        }
    }
    Ok(())
}

#[test]
fn abort_with_foreign_head_refuses_before_overwriting_authored_files() -> Result<()> {
    if gix_testtools::run_in_isolated_process().map_err(anyhow::Error::from_boxed)? {
        return Ok(());
    }
    let temp = fixture()?;
    let root = temp.path();
    let repo = gix::open(root)?;
    let mut session = Session::prepare(&repo, StopKind::Edit, &active())?;
    fs::write(root.join("selected"), b"authored draft retained\n")?;
    session.stage(&repo, &active(), false)?;
    git(root, &["commit", "--allow-empty", "-qm", "foreign commit"])?;
    let head = git(root, &["rev-parse", "HEAD"])?;
    let index = fs::read(repo.index_path())?;
    assert!(session.abort(&repo).is_err());
    assert_eq!(git(root, &["rev-parse", "HEAD"])?, head);
    assert_eq!(fs::read(repo.index_path())?, index);
    assert_eq!(fs::read(root.join("selected"))?, b"authored draft retained\n");
    Ok(())
}

#[test]
fn abort_scope_expansion_restores_original_tracked_file_and_removes_new_file() -> Result<()> {
    if gix_testtools::run_in_isolated_process().map_err(anyhow::Error::from_boxed)? {
        return Ok(());
    }
    let temp = fixture()?;
    let root = temp.path();
    let repo = gix::open(root)?;
    let mut session = Session::prepare(&repo, StopKind::Edit, &active())?;
    fs::write(root.join("neighbor"), b"expanded neighbor draft\n")?;
    fs::write(root.join("new-expanded"), b"expanded new draft\n")?;
    session.stage(&repo, &["neighbor".into(), "new-expanded".into()], true)?;
    let retained = session.abort(&repo)?;
    assert_eq!(fs::read(root.join("neighbor"))?, b"base neighbor\n");
    assert!(!root.join("new-expanded").exists());
    let envelope: serde_json::Value = serde_json::from_slice(&fs::read(retained.join("abort.json"))?)?;
    let payload: Vec<u8> = serde_json::from_value(envelope["payload"].clone())?;
    let receipt: serde_json::Value = serde_json::from_slice(&payload)?;
    let changes = receipt["changes"].as_array().expect("retained changes");
    for expected in [
        b"expanded neighbor draft\n".as_slice(),
        b"expanded new draft\n".as_slice(),
    ] {
        let expected = serde_json::to_value(expected)?;
        assert!(changes.iter().any(|row| row[1]["File"][0] == expected));
    }
    Ok(())
}

#[test]
fn preserved_intent_to_add_stays_private_and_abort_restores_exact_index() -> Result<()> {
    use gix::bstr::ByteSlice;
    if gix_testtools::run_in_isolated_process().map_err(anyhow::Error::from_boxed)? {
        return Ok(());
    }
    let temp = fixture()?;
    let root = temp.path();
    fs::write(root.join("intent-private"), b"intent private draft\n")?;
    git(root, &["add", "--intent-to-add", "intent-private"])?;
    let repo = gix::open(root)?;
    let original_index = fs::read(repo.index_path())?;
    let mut session = Session::prepare(&repo, StopKind::Edit, &active())?;
    session.amend(&repo, "ITA excluded")?;
    let excluded = gix_testtools::git_command(root)
        .args(["cat-file", "-e", "HEAD:intent-private"])
        .output()?;
    assert!(
        !excluded.status.success(),
        "intent-to-add draft never becomes a rewritten tree entry"
    );
    let index = repo.open_index()?;
    let entry = index
        .entry_by_path(b"intent-private".as_bstr())
        .expect("preserved intent entry");
    assert!(entry.flags.contains(gix::index::entry::Flags::INTENT_TO_ADD));
    assert_eq!(fs::read(root.join("intent-private"))?, b"intent private draft\n");
    session.abort(&repo)?;
    assert_eq!(fs::read(repo.index_path())?, original_index);
    assert_eq!(fs::read(root.join("intent-private"))?, b"intent private draft\n");
    Ok(())
}

#[test]
fn expansion_refuses_lossy_reconstruction_of_clean_crlf_before_image() -> Result<()> {
    if gix_testtools::run_in_isolated_process().map_err(anyhow::Error::from_boxed)? {
        return Ok(());
    }
    let temp = fixture()?;
    let root = temp.path();
    git(root, &["config", "core.autocrlf", "true"])?;
    fs::write(root.join("neighbor"), b"base neighbor\r\n")?;
    git(root, &["diff", "--exit-code", "--", "neighbor"])?;
    let repo = gix::open(root)?;
    let mut session = Session::prepare(&repo, StopKind::Edit, &active())?;
    let before = fs::read(repo.index_path())?;
    fs::write(root.join("neighbor"), b"expanded CRLF draft\r\n")?;
    assert!(
        session.stage(&repo, &["neighbor".into()], true).is_err(),
        "clean filtering cannot fabricate original raw bytes"
    );
    assert_eq!(fs::read(repo.index_path())?, before);
    assert_eq!(fs::read(root.join("neighbor"))?, b"expanded CRLF draft\r\n");
    Ok(())
}

#[cfg(feature = "switch-test-faults")]
#[test]
fn invocation_child_helper() -> Result<()> {
    let Some(root) = std::env::var_os("BRIT_TEST_INVOCATION_ROOT") else {
        return Ok(());
    };
    let repo = gix::open(Path::new(&root))?;
    if std::env::var_os("BRIT_TEST_SESSION_CHILD").is_some() {
        let mut session = Session::prepare(&repo, StopKind::Edit, &active())?;
        session.amend(&repo, "interrupted session child")?;
        return Ok(());
    }
    gitoxide_core::repository::authoring::run_hook_in_context(
        &repo,
        "pre-commit",
        &[],
        None,
        &repo.index_path(),
        &repo.git_dir().join("test-invocation"),
    )?;
    Ok(())
}

#[cfg(feature = "switch-test-faults")]
fn invocation_subprocess(root: &Path, checkpoint: &str) -> Result<std::process::Child> {
    let mut command = std::process::Command::new(std::env::current_exe()?);
    gix_testtools::configure_git_environment(&mut command, root);
    Ok(command
        .args(["--exact", "invocation_child_helper", "--nocapture"])
        .env("BRIT_TEST_INVOCATION_ROOT", root)
        .env("BRIT_AUTHORING_INVOCATION_PAUSE", checkpoint)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::inherit())
        .spawn()?)
}

#[cfg(feature = "switch-test-faults")]
fn wait_for(path: &Path, child: &mut std::process::Child) -> Result<()> {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    while !path.exists() {
        ensure!(child.try_wait()?.is_none(), "child exited before {}", path.display());
        if std::time::Instant::now() >= deadline {
            child.kill()?;
            child.wait()?;
            anyhow::bail!("checkpoint timeout: {}", path.display());
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    Ok(())
}

#[cfg(feature = "switch-test-faults")]
#[test]
fn killed_supervisors_never_retry_unknown_hooks_and_pre_release_barrier_prevents_execution() -> Result<()> {
    use gitoxide_core::repository::authoring::{
        invocation::{InvocationOutcome, inspect},
        run_hook_in_context,
    };
    if gix_testtools::run_in_isolated_process().map_err(anyhow::Error::from_boxed)? {
        return Ok(());
    }
    for checkpoint in ["after-start", "after-launch", "after-wait"] {
        let temp = fixture()?;
        let root = temp.path();
        hook(root, "pre-commit", "printf 'executed\\n' >> hook-count")?;
        let repo = gix::open(root)?;
        let directory = repo.git_dir().join("test-invocation");
        let mut child = invocation_subprocess(root, checkpoint)?;
        wait_for(&directory.join("checkpoint"), &mut child)?;
        assert!(inspect(&directory).is_err(), "live supervisor owns invocation lease");
        child.kill()?;
        child.wait()?;
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        let outcome = loop {
            match inspect(&directory) {
                Ok(value) => break value,
                Err(error) if std::time::Instant::now() >= deadline => return Err(error),
                Err(_) => std::thread::sleep(std::time::Duration::from_millis(10)),
            }
        };
        assert_eq!(outcome, InvocationOutcome::Unknown);
        assert!(
            run_hook_in_context(&repo, "pre-commit", &[], None, &repo.index_path(), &directory).is_err(),
            "same invocation ID never retries"
        );
        if checkpoint == "after-wait" {
            assert_eq!(fs::read(root.join("hook-count"))?, b"executed\n");
        } else {
            assert!(
                !root.join("hook-count").exists(),
                "gate EOF prevents an unrecorded execution"
            );
        }
    }
    Ok(())
}

#[cfg(feature = "switch-test-faults")]
#[test]
fn live_orphan_hook_blocks_recovery_until_its_group_stops_and_swapped_lease_refuses() -> Result<()> {
    use gitoxide_core::repository::authoring::invocation::{InvocationOutcome, inspect};
    use std::os::unix::fs::PermissionsExt;
    if gix_testtools::run_in_isolated_process().map_err(anyhow::Error::from_boxed)? {
        return Ok(());
    }
    let temp = fixture()?;
    let root = temp.path();
    hook(
        root,
        "pre-commit",
        "printf 'started\\n' > hook-started\nwhile test ! -f hook-release; do sleep 0.05; done\nprintf 'done\\n' > hook-done",
    )?;
    let repo = gix::open(root)?;
    let directory = repo.git_dir().join("test-invocation");
    let mut child = invocation_subprocess(root, "after-release")?;
    wait_for(&directory.join("checkpoint"), &mut child)?;
    wait_for(&root.join("hook-started"), &mut child)?;
    child.kill()?;
    child.wait()?;
    let blocked = inspect(&directory);
    fs::write(root.join("hook-release"), b"release")?;
    let blocked = blocked.expect_err("orphan remains live after supervisor death");
    assert!(blocked.to_string().contains("live"), "{blocked:#}");
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    loop {
        match inspect(&directory) {
            Ok(value) => {
                assert_eq!(value, InvocationOutcome::Unknown);
                break;
            }
            Err(error) if std::time::Instant::now() >= deadline => return Err(error),
            Err(_) => std::thread::sleep(std::time::Duration::from_millis(10)),
        }
    }
    assert_eq!(fs::read(root.join("hook-done"))?, b"done\n");
    fs::rename(directory.join("lease"), repo.git_dir().join("lease.original"))?;
    fs::write(directory.join("lease"), b"foreign lease")?;
    fs::set_permissions(directory.join("lease"), fs::Permissions::from_mode(0o600))?;
    assert!(
        inspect(&directory).is_err(),
        "matching name and permissions cannot substitute lease identity"
    );
    assert_eq!(fs::read(directory.join("lease"))?, b"foreign lease");
    Ok(())
}

#[cfg(feature = "switch-test-faults")]
#[test]
fn unknown_pre_hook_is_abort_only_and_unknown_post_hook_acknowledges_without_retry() -> Result<()> {
    if gix_testtools::run_in_isolated_process().map_err(anyhow::Error::from_boxed)? {
        return Ok(());
    }
    for (pre, pause) in [(true, "after-start"), (true, "after-wait"), (false, "after-wait")] {
        let temp = fixture()?;
        let root = temp.path();
        hook(
            root,
            if pre { "pre-commit" } else { "post-commit" },
            "printf 'once\\n' >> .git/session-count",
        )?;
        let repo = gix::open(root)?;
        let before = git(root, &["rev-parse", "HEAD"])?;
        let mut command = std::process::Command::new(std::env::current_exe()?);
        gix_testtools::configure_git_environment(&mut command, root);
        let mut child = command
            .args(["--exact", "invocation_child_helper", "--nocapture"])
            .env("BRIT_TEST_INVOCATION_ROOT", root)
            .env("BRIT_TEST_SESSION_CHILD", "1")
            .env("BRIT_AUTHORING_INVOCATION_PAUSE", pause)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::inherit())
            .spawn()?;
        wait_for(
            &repo.git_dir().join("brit-authoring/invocation-1/checkpoint"),
            &mut child,
        )?;
        child.kill()?;
        child.wait()?;
        let mut session = Session::open(&repo)?;
        assert!(
            Session::recover(&repo).is_err(),
            "unknown invocation is never a retry instruction"
        );
        assert!(session.amend(&repo, "must not retry").is_err());
        if pre {
            assert_eq!(git(root, &["rev-parse", "HEAD"])?, before);
            assert!(session.acknowledge_post_hook(&repo).is_err());
            assert!(session.abort(&repo)?.is_dir());
        } else {
            assert_ne!(git(root, &["rev-parse", "HEAD"])?, before);
            session.acknowledge_post_hook(&repo)?;
            session.validate(&repo)?;
        }
        if pause == "after-start" {
            assert!(!repo.git_dir().join("session-count").exists());
        } else {
            assert_eq!(fs::read(repo.git_dir().join("session-count"))?, b"once\n");
        }
    }
    Ok(())
}

#[cfg(feature = "switch-test-faults")]
#[test]
fn failure_after_head_publication_reports_committed_partial_outcome_then_recovers() -> Result<()> {
    if gix_testtools::run_in_isolated_process().map_err(anyhow::Error::from_boxed)? {
        return Ok(());
    }
    let temp = fixture()?;
    let root = temp.path();
    let preserved = dirty_neighbor(root)?;
    let repo = gix::open(root)?;
    let mut session = Session::prepare(&repo, StopKind::Edit, &active())?;
    fs::write(root.join("selected"), b"published child\n")?;
    session.stage(&repo, &active(), false)?;
    let old = git(root, &["rev-parse", "HEAD"])?;
    let fault = gix_testtools::Env::new().set("BRIT_AUTHORING_FAIL_AFTER", "head-publication");
    let outcome = session.amend(&repo, "partial publication")?;
    drop(fault);
    assert!(
        !outcome.warnings.is_empty(),
        "publication partial outcome remains visible"
    );
    assert_ne!(git(root, &["rev-parse", "HEAD"])?, old);
    assert_eq!(
        git(root, &["rev-parse", "HEAD"])?,
        format!("{}\n", outcome.commit).as_bytes()
    );
    Session::recover(&repo)?.validate(&repo)?;
    assert_eq!(git(root, &["show", "HEAD:selected"])?, b"published child\n");
    assert_eq!(git(root, &["ls-files", "--stage", "-z", "--", "neighbor"])?, preserved);
    assert_eq!(fs::read(root.join("neighbor"))?, b"unstaged private\n");
    Ok(())
}
