//! Linux stop/preservation qualification; each row checks observed staging,
//! working bytes and rewritten trees independently.
#![cfg(target_os = "linux")]

use anyhow::{Context, Result, ensure};
use gitoxide_core::repository::replay::{Advance, Session};
use gix::bstr::ByteSlice;
use std::{
    ffi::OsString,
    fs,
    os::unix::{
        ffi::{OsStrExt, OsStringExt},
        fs::{MetadataExt, PermissionsExt, symlink},
    },
    path::{Path, PathBuf},
};

#[derive(Clone, Copy, Debug)]
enum Stop {
    Conflict,
    Empty,
    Edit,
    Break,
    Reword,
    Squash,
}
#[derive(Clone, Copy, Debug)]
enum Shape {
    StagedUnstaged,
    StagedDeletion,
    UnstagedDeletion,
    Executable,
    Symlink,
    NonUtf8,
    Untracked,
    Ignored,
}
const SHAPES: [Shape; 8] = [
    Shape::StagedUnstaged,
    Shape::StagedDeletion,
    Shape::UnstagedDeletion,
    Shape::Executable,
    Shape::Symlink,
    Shape::NonUtf8,
    Shape::Untracked,
    Shape::Ignored,
];

fn git(root: &Path, args: &[&str]) -> Result<Vec<u8>> {
    let output = gix_testtools::git_command(root).args(args).output()?;
    ensure!(
        output.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    Ok(output.stdout)
}
fn path_git(root: &Path, args: &[&str], path: &Path) -> Result<Vec<u8>> {
    let output = gix_testtools::git_command(root)
        .args(args)
        .arg("--")
        .arg(path)
        .output()?;
    ensure!(
        output.status.success(),
        "git {args:?} {path:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    Ok(output.stdout)
}
fn commit(root: &Path, path: &str, data: &[u8], message: &str) -> Result<String> {
    fs::write(root.join(path), data)?;
    git(root, &["add", "--", path])?;
    git(root, &["commit", "-qm", message])?;
    Ok(String::from_utf8(git(root, &["rev-parse", "HEAD"])?)?.trim().to_owned())
}
#[derive(Debug, PartialEq)]
struct FileState {
    mode: u32,
    device: u64,
    inode: u64,
    symlink: bool,
    bytes: Vec<u8>,
}
fn file_state(path: &Path) -> Result<Option<FileState>> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(value) => value,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.into()),
    };
    let link = metadata.file_type().is_symlink();
    Ok(Some(FileState {
        mode: metadata.mode() & 0o777,
        device: metadata.dev(),
        inode: metadata.ino(),
        symlink: link,
        bytes: if link {
            fs::read_link(path)?.as_os_str().as_bytes().to_vec()
        } else {
            fs::read(path)?
        },
    }))
}
struct Fixture {
    temp: tempfile::TempDir,
    preserved: PathBuf,
    candidates: [String; 2],
}
fn fixture(stop: Stop, shape: Shape) -> Result<Fixture> {
    let temp = tempfile::tempdir()?;
    let root = temp.path();
    git(root, &["init", "-q", "-b", "main"])?;
    git(root, &["config", "user.name", "Qualification Author"])?;
    git(root, &["config", "user.email", "qualification@example.invalid"])?;
    fs::write(root.join("selected"), b"base\n")?;
    fs::write(root.join("second"), b"base second\n")?;
    fs::write(root.join(".gitignore"), b"ignored-neighbor\n")?;
    let preserved = match shape {
        Shape::NonUtf8 => PathBuf::from(OsString::from_vec(b"neighbor-\xff".to_vec())),
        Shape::Untracked => "untracked-neighbor".into(),
        Shape::Ignored => "ignored-neighbor".into(),
        _ => "neighbor".into(),
    };
    if matches!(shape, Shape::Symlink) {
        symlink("clean-target", root.join(&preserved))?;
    } else if !matches!(shape, Shape::Untracked | Shape::Ignored) {
        fs::write(root.join(&preserved), b"clean preserved\n")?;
    }
    git(root, &["add", "."])?;
    git(root, &["commit", "-qm", "base"])?;
    git(root, &["branch", "base"])?;
    git(root, &["switch", "-qc", "upstream"])?;
    match stop {
        Stop::Conflict => {
            commit(root, "selected", b"upstream conflict\n", "upstream")?;
        }
        Stop::Empty => {
            commit(root, "selected", b"topic\n", "upstream same patch")?;
        }
        _ => {
            commit(root, "upstream", b"upstream\n", "upstream")?;
        }
    }
    git(root, &["switch", "-q", "main"])?;
    let first = commit(root, "selected", b"topic\n", "first")?;
    let second = commit(root, "second", b"topic second\n", "second")?;
    let path = root.join(&preserved);
    match shape {
        Shape::StagedUnstaged | Shape::NonUtf8 => {
            fs::write(&path, b"staged preserved\n")?;
            path_git(root, &["add"], &preserved)?;
            fs::write(&path, b"unstaged preserved\n")?;
        }
        Shape::StagedDeletion => {
            fs::remove_file(&path)?;
            path_git(root, &["add"], &preserved)?;
        }
        Shape::UnstagedDeletion => fs::remove_file(&path)?,
        Shape::Executable => {
            fs::set_permissions(&path, fs::Permissions::from_mode(0o755))?;
            path_git(root, &["add"], &preserved)?;
            fs::set_permissions(&path, fs::Permissions::from_mode(0o644))?;
        }
        Shape::Symlink => {
            fs::remove_file(&path)?;
            symlink("staged-target", &path)?;
            path_git(root, &["add"], &preserved)?;
            fs::remove_file(&path)?;
            symlink(OsString::from_vec(b"unstaged-target-\xff".to_vec()), &path)?;
        }
        Shape::Untracked | Shape::Ignored => fs::write(&path, b"private neighbor\n")?,
    }
    Ok(Fixture {
        temp,
        preserved,
        candidates: [first, second],
    })
}

fn case(stop: Stop, shape: Shape, abort: bool) -> Result<()> {
    let fixture = fixture(stop, shape)?;
    let root = fixture.temp.path();
    let original_index = fs::read(root.join(".git/index"))?;
    let metadata_paths = ["HEAD", "logs/HEAD", "refs/heads/main", "logs/refs/heads/main"];
    let original_metadata = metadata_paths
        .iter()
        .map(|path| fs::read(root.join(".git").join(path)))
        .collect::<std::io::Result<Vec<_>>>()?;
    let original_selected = fs::read(root.join("selected"))?;
    let original_second = fs::read(root.join("second"))?;
    let work = file_state(&root.join(&fixture.preserved))?;
    let index = path_git(root, &["ls-files", "--stage", "-z"], &fixture.preserved)?;
    let clean_tree = path_git(root, &["ls-tree", "-z", "base"], &fixture.preserved)?;
    let [first, second] = &fixture.candidates;
    let todo = match stop {
        Stop::Edit => format!("edit {first}\ndrop {second}\n"),
        Stop::Break => format!("pick {first}\nbreak\ndrop {second}\n"),
        Stop::Reword => format!("reword {first}\ndrop {second}\n"),
        Stop::Squash => format!("pick {first}\nsquash {second}\n"),
        _ => format!("pick {first}\ndrop {second}\n"),
    };
    let repo = gix::open(root)?;
    let mut session =
        Session::prepare_interactive(&repo, b"upstream".as_bstr())?.expect("nonempty qualification selection");
    session.accept_todo(&repo, todo.as_bytes())?;
    assert_eq!(session.advance(&repo)?, Advance::Started);
    if matches!(stop, Stop::Break | Stop::Squash) {
        assert!(matches!(session.advance(&repo)?, Advance::Step { .. }));
    }
    match stop {
        Stop::Conflict => assert_eq!(session.advance(&repo)?, Advance::Conflict),
        Stop::Empty => assert_eq!(session.advance(&repo)?, Advance::Empty),
        Stop::Edit => assert_eq!(session.advance(&repo)?, Advance::Edit),
        Stop::Break => assert_eq!(session.advance(&repo)?, Advance::Break),
        Stop::Reword | Stop::Squash => {
            assert!(session.advance(&repo).is_err());
            assert_eq!(session.phase(), "message");
        }
    }
    assert_eq!(file_state(&root.join(&fixture.preserved))?, work);
    assert_eq!(
        path_git(root, &["ls-files", "--stage", "-z"], &fixture.preserved)?,
        index
    );
    if abort {
        let receipt = session.abort(&repo)?;
        gitoxide_core::repository::replay::retained::inspect(&receipt.path)?;
        assert_eq!(fs::read(root.join(".git/index"))?, original_index);
        for (path, expected) in metadata_paths.iter().zip(&original_metadata) {
            assert_eq!(fs::read(root.join(".git").join(path))?, *expected);
        }
        assert_eq!(fs::read(root.join("selected"))?, original_selected);
        assert_eq!(fs::read(root.join("second"))?, original_second);
    } else {
        match stop {
            Stop::Conflict => {
                fs::write(root.join("selected"), b"accepted conflict\n")?;
                session.stage(&repo, &["selected".into()], false)?;
                session.continue_conflict(&repo)?;
            }
            Stop::Edit => {
                session.amend(&repo, b"qualified amendment\n")?;
                session.continue_edit(&repo)?;
            }
            Stop::Reword | Stop::Squash => {
                fs::write(
                    session.editor_output().expect("saved failed editor output"),
                    b"qualified message\n",
                )?;
                session.accept_editor(&repo)?;
            }
            _ => {}
        }
        let mut ready = false;
        for _ in 0..8 {
            match session.advance(&repo)? {
                Advance::ReadyFinal => {
                    ready = true;
                    break;
                }
                Advance::Step { .. } | Advance::Dropped => {}
                state => anyhow::bail!("unexpected completion stop {state:?}"),
            }
        }
        ensure!(ready, "sequence reaches final boundary");
        session.finish(&repo)?;
        assert_eq!(git(root, &["symbolic-ref", "HEAD"])?, b"refs/heads/main\n");
        // Includes superseded and stopped trees protected by operation retention,
        // not just the final ancestry, so hidden dirty-content leakage is visible.
        for id in git(
            root,
            &["for-each-ref", "--format=%(objectname)", "refs/brit-rebase-retain/"],
        )?
        .split(|byte| *byte == b'\n')
        .filter(|line| !line.is_empty())
        {
            let id = std::str::from_utf8(id)?;
            assert_eq!(
                path_git(root, &["ls-tree", "-z", id], &fixture.preserved)?,
                clean_tree,
                "preserved changes leaked into retained rewritten tree {id}"
            );
        }
        assert_eq!(
            path_git(root, &["ls-files", "--stage", "-z"], &fixture.preserved)?,
            index
        );
    }
    assert_eq!(file_state(&root.join(&fixture.preserved))?, work);
    Ok(())
}
fn matrix(stop: Stop) -> Result<()> {
    if gix_testtools::run_in_isolated_process().map_err(anyhow::Error::from_boxed)? {
        return Ok(());
    }
    let _editors = gix_testtools::Env::new()
        .set("GIT_SEQUENCE_EDITOR", ":")
        .set("GIT_EDITOR", "exit 7");
    for shape in SHAPES {
        for abort in [false, true] {
            case(stop, shape, abort)
                .with_context(|| format!("stop={stop:?}, preservation={shape:?}, abort={abort}"))?;
        }
    }
    Ok(())
}
#[test]
fn conflict_preservation_matrix() -> Result<()> {
    matrix(Stop::Conflict)
}
#[test]
fn empty_preservation_matrix() -> Result<()> {
    matrix(Stop::Empty)
}
#[test]
fn edit_preservation_matrix() -> Result<()> {
    matrix(Stop::Edit)
}
#[test]
fn break_preservation_matrix() -> Result<()> {
    matrix(Stop::Break)
}
#[test]
fn reword_message_preservation_matrix() -> Result<()> {
    matrix(Stop::Reword)
}
#[test]
fn squash_message_preservation_matrix() -> Result<()> {
    matrix(Stop::Squash)
}

#[test]
fn ancestor_descendant_collisions_refuse_before_any_stop_can_mutate_live_state() -> Result<()> {
    if gix_testtools::run_in_isolated_process().map_err(anyhow::Error::from_boxed)? {
        return Ok(());
    }
    for stop in [
        Stop::Conflict,
        Stop::Empty,
        Stop::Edit,
        Stop::Break,
        Stop::Reword,
        Stop::Squash,
    ] {
        for ancestor in [false, true] {
            let fixture = fixture(stop, Shape::Untracked)?;
            let root = fixture.temp.path();
            git(root, &["switch", "-q", "upstream"])?;
            let target = if ancestor {
                fs::create_dir(root.join("collision"))?;
                "collision/leaf"
            } else {
                "collision"
            };
            commit(root, target, b"upstream path\n", "collision footprint")?;
            git(root, &["switch", "-q", "main"])?;
            let private = if ancestor {
                root.join("collision")
            } else {
                fs::create_dir(root.join("collision"))?;
                root.join("collision/leaf")
            };
            fs::write(&private, b"private colliding neighbor\n")?;
            let before = file_state(&private)?;
            let index = fs::read(root.join(".git/index"))?;
            let head = fs::read(root.join(".git/HEAD"))?;
            let branch = fs::read(root.join(".git/refs/heads/main"))?;
            let repo = gix::open(root)?;
            assert!(
                Session::prepare_interactive(&repo, b"upstream".as_bstr()).is_err(),
                "stop={stop:?}, ancestor={ancestor}"
            );
            assert_eq!(file_state(&private)?, before);
            assert_eq!(fs::read(root.join(".git/index"))?, index);
            assert_eq!(fs::read(root.join(".git/HEAD"))?, head);
            assert_eq!(fs::read(root.join(".git/refs/heads/main"))?, branch);
            assert!(!root.join(".git/brit-rebase").exists());
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
fn final_publication_fault_helper() -> Result<()> {
    let Some(root) = std::env::var_os("BRIT_QUALIFICATION_FINAL_ROOT") else {
        return Ok(());
    };
    let root = Path::new(&root);
    let repo = gix::open(root)?;
    let mut session;
    {
        let _unarmed = gix_testtools::Env::new().unset("BRIT_SWITCH_CHECKPOINT");
        session = Session::prepare(&repo, b"upstream".as_bstr())?.expect("divergent replay");
        loop {
            match session.advance(&repo)? {
                Advance::ReadyFinal => break,
                Advance::Started | Advance::Step { .. } => {}
                state => anyhow::bail!("unexpected final fixture stop {state:?}"),
            }
        }
    }
    session.finish(&repo)
}

#[cfg(feature = "switch-test-faults")]
fn kill_final_decision(root: &Path) -> Result<()> {
    use std::{
        process::{Command, Stdio},
        time::{Duration, Instant},
    };
    let checkpoints = tempfile::tempdir()?;
    let mut command = Command::new(std::env::current_exe()?);
    gix_testtools::configure_git_environment(&mut command, root);
    let mut child = command
        .args(["--exact", "final_publication_fault_helper", "--nocapture"])
        .env("BRIT_QUALIFICATION_FINAL_ROOT", root)
        .env("BRIT_SWITCH_CHECKPOINT", "replay-decision-durable")
        .env("BRIT_SWITCH_CHECKPOINT_DIR", checkpoints.path())
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::inherit())
        .spawn()?;
    let reached = checkpoints.path().join("replay-decision-durable.reached");
    let deadline = Instant::now() + Duration::from_secs(20);
    while !reached.exists() {
        ensure!(child.try_wait()?.is_none(), "final helper exited before decision");
        if Instant::now() >= deadline {
            child.kill()?;
            child.wait()?;
            anyhow::bail!("final decision checkpoint timeout");
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    child.kill()?;
    child.wait()?;
    Ok(())
}

#[cfg(feature = "switch-test-faults")]
#[test]
fn final_decision_refuses_replaced_head_branch_orig_head_locks_and_foreign_branch() -> Result<()> {
    if gix_testtools::run_in_isolated_process().map_err(anyhow::Error::from_boxed)? {
        return Ok(());
    }
    for resource in ["HEAD.lock", "refs/heads/main.lock", "ORIG_HEAD.lock", "refs/heads/main"] {
        let fixture = fixture(Stop::Edit, Shape::StagedUnstaged)?;
        let root = fixture.temp.path();
        kill_final_decision(root)?;
        let repo = gix::open(root)?;
        let path = root.join(".git").join(resource);
        if path.exists() {
            fs::remove_file(&path)?;
        }
        fs::write(
            &path,
            if resource.ends_with(".lock") {
                b"foreign lock bytes\n".to_vec()
            } else {
                git(root, &["rev-parse", "base"])?
            },
        )?;
        let observed = fs::read(&path)?;
        let metadata_paths = ["HEAD", "index", "refs/heads/main", "logs/HEAD", "logs/refs/heads/main"];
        let before = metadata_paths
            .iter()
            .map(|path| fs::read(root.join(".git").join(path)))
            .collect::<std::io::Result<Vec<_>>>()?;
        assert!(
            Session::recover_decision(&repo).is_err(),
            "recovery refuses replaced {resource}"
        );
        for (path, expected) in metadata_paths.iter().zip(before) {
            assert_eq!(fs::read(root.join(".git").join(path))?, expected);
        }
        assert_eq!(fs::read(path)?, observed);
    }
    Ok(())
}

fn reseal(path: &Path, mutate: impl FnOnce(&mut serde_json::Value)) -> Result<()> {
    let envelope: serde_json::Value = serde_json::from_slice(&fs::read(path)?)?;
    let payload: Vec<u8> = serde_json::from_value(envelope["payload"].clone())?;
    let mut value: serde_json::Value = serde_json::from_slice(&payload)?;
    mutate(&mut value);
    let payload = serde_json::to_vec(&value)?;
    let mut hash = gix::hash::hasher(gix::hash::Kind::Sha1);
    hash.update(&payload);
    let checksum = hash.try_finalize()?.to_string();
    fs::write(
        path,
        serde_json::to_vec(&serde_json::json!({"payload": payload, "checksum": checksum}))?,
    )?;
    Ok(())
}

#[test]
fn invocation_corruption_and_valid_checksum_pid_identity_ambiguity_refuse_read_only() -> Result<()> {
    use gitoxide_core::repository::{
        authoring::invocation,
        replay::editors::{self, Kind},
    };
    if gix_testtools::run_in_isolated_process().map_err(anyhow::Error::from_boxed)? {
        return Ok(());
    }
    let _editor = gix_testtools::Env::new().set("GIT_EDITOR", ":");
    for damage in ["start.json", "launch.json", "exit.json", "reused-pid"] {
        let fixture = fixture(Stop::Edit, Shape::StagedUnstaged)?;
        let root = fixture.temp.path();
        let repo = gix::open(root)?;
        let output = root.join(".git/editor-message");
        fs::write(&output, b"editor seed\n")?;
        let plan = editors::resolve(&repo, Kind::Message, &output)?;
        let directory = root.join(".git/qualification-invocation");
        assert!(editors::run(&repo, &directory, &plan, &repo.index_path(), || Ok(()))?.success());
        let target = directory.join(if damage == "reused-pid" { "launch.json" } else { damage });
        if damage == "reused-pid" {
            reseal(&target, |value| {
                value["pid"] = std::process::id().into();
                value["start"] = "impossible-reused-start".into();
            })?;
        } else {
            fs::write(&target, b"corrupt invocation receipt\n")?;
        }
        let before = fs::read(&target)?;
        let index = fs::read(repo.index_path())?;
        let head = fs::read(root.join(".git/HEAD"))?;
        assert!(invocation::inspect(&directory).is_err(), "refuse {damage}");
        assert_eq!(fs::read(target)?, before);
        assert_eq!(fs::read(repo.index_path())?, index);
        assert_eq!(fs::read(root.join(".git/HEAD"))?, head);
    }
    Ok(())
}

#[test]
fn owned_edit_child_refuses_forged_parent_binding_and_same_oid_attachment_change() -> Result<()> {
    if gix_testtools::run_in_isolated_process().map_err(anyhow::Error::from_boxed)? {
        return Ok(());
    }
    for damage in ["binding", "attachment"] {
        let fixture = fixture(Stop::Edit, Shape::StagedUnstaged)?;
        let root = fixture.temp.path();
        let repo = gix::open(root)?;
        let mut session = Session::prepare_interactive(&repo, b"upstream".as_bstr())?.expect("interactive selection");
        session.accept_todo(
            &repo,
            format!("edit {}\ndrop {}\n", fixture.candidates[0], fixture.candidates[1]).as_bytes(),
        )?;
        assert_eq!(session.advance(&repo)?, Advance::Started);
        assert_eq!(session.advance(&repo)?, Advance::Edit);
        let child = root.join(".git/brit-authoring/state.json");
        if damage == "binding" {
            reseal(&child, |value| {
                value["replay_parent"]["nonce"] = "foreign-parent-nonce".into();
            })?;
        } else {
            let id = git(root, &["rev-parse", "HEAD"])?;
            fs::write(root.join(".git/refs/heads/foreign-same-oid"), id)?;
            fs::write(root.join(".git/HEAD"), b"ref: refs/heads/foreign-same-oid\n")?;
        }
        let before_child = fs::read(&child)?;
        let before_head = fs::read(root.join(".git/HEAD"))?;
        let before_index = fs::read(repo.index_path())?;
        assert!(session.continue_edit(&repo).is_err(), "refuse {damage}");
        assert_eq!(fs::read(&child)?, before_child);
        assert_eq!(fs::read(root.join(".git/HEAD"))?, before_head);
        assert_eq!(fs::read(repo.index_path())?, before_index);
    }
    Ok(())
}
