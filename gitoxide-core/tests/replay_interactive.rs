//! Internal interactive replay contracts. Public activation belongs to Sprint 5g.
#![cfg(target_os = "linux")]

use std::{fs, path::Path};

use anyhow::{Result, ensure};
use gitoxide_core::repository::replay::{Advance, Session};
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

fn oid(root: &Path, revision: &str) -> Result<gix::ObjectId> {
    Ok(gix::ObjectId::from_hex(git(root, &["rev-parse", revision])?.trim())?)
}

fn commit(root: &Path, path: &str, bytes: &[u8], message: &str) -> Result<gix::ObjectId> {
    fs::write(root.join(path), bytes)?;
    git(root, &["add", "--", path])?;
    git(root, &["commit", "-qm", message])?;
    oid(root, "HEAD")
}

fn fixture() -> Result<(tempfile::TempDir, Vec<gix::ObjectId>)> {
    let temp = tempfile::tempdir()?;
    let root = temp.path();
    git(root, &["init", "-q", "-b", "main"])?;
    git(root, &["config", "user.name", "Interactive Author"])?;
    git(root, &["config", "user.email", "interactive@example.invalid"])?;
    commit(root, "neighbor", b"clean neighbor\n", "base")?;
    git(root, &["branch", "upstream"])?;
    let first = commit(root, "first", b"first content\n", "first message")?;
    let second = commit(root, "second", b"second content\n", "second message")?;
    fs::write(root.join("neighbor"), b"preserved staged\n")?;
    git(root, &["add", "neighbor"])?;
    fs::write(root.join("neighbor"), b"preserved unstaged\n")?;
    Ok((temp, vec![first, second]))
}

fn assert_preserved(root: &Path) -> Result<()> {
    assert_eq!(git(root, &["show", ":neighbor"])?, b"preserved staged\n");
    assert_eq!(fs::read(root.join("neighbor"))?, b"preserved unstaged\n");
    assert_eq!(git(root, &["show", "HEAD:neighbor"])?, b"clean neighbor\n");
    Ok(())
}

#[test]
fn interactive_ancestor_upstream_reorders_candidates_and_rejected_todo_keeps_last_accepted_bytes() -> Result<()> {
    if gix_testtools::run_in_isolated_process().map_err(anyhow::Error::from_boxed)? {
        return Ok(());
    }
    let _editor = gix_testtools::Env::new().set("GIT_SEQUENCE_EDITOR", ":");
    let (temp, commits) = fixture()?;
    let root = temp.path();
    let repo = gix::open(root)?;
    let mut session = Session::prepare_interactive(&repo, b"upstream".as_bstr())?
        .expect("interactive ancestor selection must replay");
    let accepted = format!("pick {} second\npick {} first\n", commits[1], commits[0]).into_bytes();
    session.accept_todo(&repo, &accepted)?;
    assert_eq!(session.todo()?, accepted);
    let invalid = format!("pick {}\n", commits[1]);
    assert!(
        session.accept_todo(&repo, invalid.as_bytes()).is_err(),
        "missing candidate cannot silently drop work"
    );
    assert_eq!(session.todo()?, accepted);
    assert_eq!(session.advance(&repo)?, Advance::Started);
    assert!(matches!(session.advance(&repo)?, Advance::Step { .. }));
    assert_eq!(git(root, &["show", "HEAD:second"])?, b"second content\n");
    assert!(!root.join("first").exists());
    assert_preserved(root)?;
    assert!(matches!(session.advance(&repo)?, Advance::Step { .. }));
    assert_eq!(session.advance(&repo)?, Advance::ReadyFinal);
    session.finish(&repo)?;
    assert_eq!(
        git(root, &["log", "-2", "--format=%s"])?,
        b"first message\nsecond message\n"
    );
    assert_preserved(root)?;
    Ok(())
}

#[test]
fn explicit_drop_of_every_candidate_finishes_at_upstream_with_preserved_dirty_index() -> Result<()> {
    if gix_testtools::run_in_isolated_process().map_err(anyhow::Error::from_boxed)? {
        return Ok(());
    }
    let _editor = gix_testtools::Env::new().set("GIT_SEQUENCE_EDITOR", ":");
    let (temp, commits) = fixture()?;
    let root = temp.path();
    let repo = gix::open(root)?;
    let mut session = Session::prepare_interactive(&repo, b"upstream".as_bstr())?.expect("interactive selection");
    session.accept_todo(&repo, format!("drop {}\ndrop {}\n", commits[0], commits[1]).as_bytes())?;
    assert_eq!(session.advance(&repo)?, Advance::Started);
    for _ in 0..4 {
        if session.advance(&repo)? == Advance::ReadyFinal {
            break;
        }
    }
    session.finish(&repo)?;
    assert_eq!(oid(root, "main")?, oid(root, "upstream")?);
    assert!(!root.join("first").exists());
    assert!(!root.join("second").exists());
    assert_preserved(root)?;
    Ok(())
}

fn hook(root: &Path, name: &str, body: &str) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let path = root.join(".git/hooks").join(name);
    fs::write(&path, format!("#!/bin/sh\n{body}\n"))?;
    fs::set_permissions(path, fs::Permissions::from_mode(0o700))?;
    Ok(())
}

fn finish_remaining(session: &mut Session, root: &Path) -> Result<()> {
    for _ in 0..12 {
        match session.advance(&gix::open(root)?)? {
            Advance::ReadyFinal => return session.finish(&gix::open(root)?),
            Advance::Started | Advance::Step { .. } | Advance::Dropped => {}
            state => anyhow::bail!("unexpected interactive stop {state:?}"),
        }
    }
    anyhow::bail!("interactive sequence did not finish")
}

#[test]
fn squash_and_fixup_share_final_mapping_and_preserve_first_author_and_parent() -> Result<()> {
    if gix_testtools::run_in_isolated_process().map_err(anyhow::Error::from_boxed)? {
        return Ok(());
    }
    let _editors = gix_testtools::Env::new()
        .set("GIT_SEQUENCE_EDITOR", ":")
        .set("GIT_EDITOR", ":");
    for command in ["squash", "fixup"] {
        let (temp, commits) = fixture()?;
        let root = temp.path();
        hook(root, "post-rewrite", "cat > \"$GIT_DIR/rewrite-pairs\"")?;
        for name in ["pre-commit", "prepare-commit-msg", "commit-msg", "post-commit"] {
            hook(
                root,
                name,
                "printf 'unexpected author hook\\n' >> \"$GIT_DIR/author-hooks\"",
            )?;
        }
        let repo = gix::open(root)?;
        let mut session = Session::prepare_interactive(&repo, b"upstream".as_bstr())?.expect("interactive selection");
        session.accept_todo(
            &repo,
            format!("pick {}\n{command} {}\n", commits[0], commits[1]).as_bytes(),
        )?;
        finish_remaining(&mut session, root)?;
        let result = oid(root, "main")?;
        assert_eq!(git(root, &["rev-list", "--count", "upstream..main"])?, b"1\n");
        assert_eq!(oid(root, "main^")?, oid(root, "upstream")?);
        assert_eq!(
            fs::read(root.join(".git/rewrite-pairs"))?,
            format!("{} {result}\n{} {result}\n", commits[0], commits[1]).into_bytes()
        );
        let message = git(root, &["log", "-1", "--format=%B"])?;
        assert!(message.starts_with(b"first message\n"));
        assert_eq!(
            message
                .windows(b"second message".len())
                .any(|window| window == b"second message"),
            command == "squash"
        );
        let author = git(
            root,
            &["show", "-s", "--format=%an <%ae> %at %ai", &commits[0].to_string()],
        )?;
        assert_eq!(
            git(root, &["show", "-s", "--format=%an <%ae> %at %ai", "main"])?,
            author
        );
        assert!(!root.join(".git/author-hooks").exists());
        assert_preserved(root)?;
    }
    Ok(())
}

#[test]
fn failed_sequence_editor_retains_output_and_accepts_saved_bytes_without_rerunning() -> Result<()> {
    if gix_testtools::run_in_isolated_process().map_err(anyhow::Error::from_boxed)? {
        return Ok(());
    }
    let _editor = gix_testtools::Env::new().set(
        "GIT_SEQUENCE_EDITOR",
        "printf 'ran\\n' >> \"$GIT_DIR/editor-count\"; printf 'invalid editor output\\n' > \"$1\"; exit 7",
    );
    let (temp, commits) = fixture()?;
    let root = temp.path();
    let repo = gix::open(root)?;
    let mut session = Session::prepare_interactive(&repo, b"upstream".as_bstr())?.expect("interactive selection");
    let initial = session.todo()?;
    assert!(session.edit_todo(&repo).is_err());
    let output = session.editor_output().expect("failed editor retains output path");
    assert_eq!(fs::read(&output)?, b"invalid editor output\n");
    assert_eq!(session.todo()?, initial);
    assert!(session.accept_editor(&repo).is_err());
    assert_eq!(session.todo()?, initial);
    let saved = format!("drop {}\npick {}\n", commits[0], commits[1]).into_bytes();
    fs::write(&output, &saved)?;
    session.accept_editor(&repo)?;
    assert_eq!(session.todo()?, saved);
    assert_eq!(fs::read(root.join(".git/editor-count"))?, b"ran\n");
    finish_remaining(&mut session, root)?;
    assert!(!root.join("first").exists());
    assert_preserved(root)?;
    Ok(())
}

#[test]
fn failed_message_editor_accepts_nonempty_saved_bytes_and_preserves_original_author() -> Result<()> {
    if gix_testtools::run_in_isolated_process().map_err(anyhow::Error::from_boxed)? {
        return Ok(());
    }
    let _editors = gix_testtools::Env::new().set("GIT_SEQUENCE_EDITOR", ":").set(
        "GIT_EDITOR",
        "printf 'ran\\n' >> \"$GIT_DIR/editor-count\"; printf '  \\n' > \"$1\"; exit 8",
    );
    let (temp, commits) = fixture()?;
    let root = temp.path();
    let repo = gix::open(root)?;
    let mut session = Session::prepare_interactive(&repo, b"upstream".as_bstr())?.expect("interactive selection");
    session.accept_todo(
        &repo,
        format!("reword {}\ndrop {}\n", commits[0], commits[1]).as_bytes(),
    )?;
    assert_eq!(session.advance(&repo)?, Advance::Started);
    assert!(session.advance(&repo).is_err());
    let output = session.editor_output().expect("failed message output");
    assert!(session.accept_editor(&repo).is_err(), "empty messages cannot publish");
    fs::write(output, b"reworded message\n\xffraw bytes\n")?;
    session.accept_editor(&repo)?;
    finish_remaining(&mut session, root)?;
    assert_eq!(
        git(root, &["show", "-s", "--format=%B", "main"])?,
        b"reworded message\n\xffraw bytes\n\n"
    );
    assert_eq!(fs::read(root.join(".git/editor-count"))?, b"ran\n");
    assert_preserved(root)?;
    Ok(())
}

#[test]
fn edit_amend_uses_private_index_and_explicit_authoring_hooks() -> Result<()> {
    if gix_testtools::run_in_isolated_process().map_err(anyhow::Error::from_boxed)? {
        return Ok(());
    }
    let _editor = gix_testtools::Env::new().set("GIT_SEQUENCE_EDITOR", ":");
    let (temp, commits) = fixture()?;
    let root = temp.path();
    for name in ["pre-commit", "prepare-commit-msg", "commit-msg", "post-commit"] {
        hook(root, name, &format!("printf '{name}\\n' >> \"$GIT_DIR/author-hooks\""))?;
    }
    let repo = gix::open(root)?;
    let mut session = Session::prepare_interactive(&repo, b"upstream".as_bstr())?.expect("interactive selection");
    session.accept_todo(&repo, format!("edit {}\npick {}\n", commits[0], commits[1]).as_bytes())?;
    assert_eq!(session.advance(&repo)?, Advance::Started);
    assert_eq!(session.advance(&repo)?, Advance::Edit);
    assert!(!root.join(".git/author-hooks").exists());
    fs::write(root.join("first"), b"amended scoped content\n")?;
    session.stage(&repo, &["first".into()], false)?;
    session.amend(&repo, b"amended message\n")?;
    session.continue_edit(&repo)?;
    finish_remaining(&mut session, root)?;
    assert_eq!(git(root, &["show", "main:first"])?, b"amended scoped content\n");
    assert_eq!(
        fs::read(root.join(".git/author-hooks"))?,
        b"pre-commit\nprepare-commit-msg\ncommit-msg\npost-commit\n"
    );
    assert_preserved(root)?;
    Ok(())
}

#[test]
fn edit_split_records_ordered_one_to_many_mapping_and_keeps_preserved_bytes_out_of_each_tree() -> Result<()> {
    if gix_testtools::run_in_isolated_process().map_err(anyhow::Error::from_boxed)? {
        return Ok(());
    }
    let _editor = gix_testtools::Env::new().set("GIT_SEQUENCE_EDITOR", ":");
    let (temp, commits) = fixture()?;
    let root = temp.path();
    hook(root, "post-rewrite", "cat > \"$GIT_DIR/rewrite-pairs\"")?;
    let repo = gix::open(root)?;
    let mut session = Session::prepare_interactive(&repo, b"upstream".as_bstr())?.expect("interactive selection");
    session.accept_todo(&repo, format!("edit {}\npick {}\n", commits[0], commits[1]).as_bytes())?;
    assert_eq!(session.advance(&repo)?, Advance::Started);
    assert_eq!(session.advance(&repo)?, Advance::Edit);
    session.begin_split(&repo)?;
    let mut chain = Vec::new();
    for (bytes, message) in [
        (b"partial first\n".as_slice(), b"split one\n".as_slice()),
        (b"first content\n".as_slice(), b"split two\n".as_slice()),
    ] {
        fs::write(root.join("first"), bytes)?;
        session.stage(&repo, &["first".into()], false)?;
        session.commit_split(&repo, message)?;
        chain.push(oid(root, "HEAD")?);
        assert_preserved(root)?;
    }
    session.continue_edit(&repo)?;
    finish_remaining(&mut session, root)?;
    assert_eq!(oid(root, &format!("{}^", chain[0]))?, oid(root, "upstream")?);
    assert_eq!(oid(root, &format!("{}^", chain[1]))?, chain[0]);
    let final_id = oid(root, "main")?;
    assert_eq!(
        fs::read(root.join(".git/rewrite-pairs"))?,
        format!(
            "{} {}\n{} {}\n{} {final_id}\n",
            commits[0], chain[0], commits[0], chain[1], commits[1]
        )
        .into_bytes()
    );
    assert_preserved(root)?;
    Ok(())
}

#[test]
fn break_allows_todo_suffix_edit_but_refuses_authoring_and_skip_moves_past_only_break() -> Result<()> {
    if gix_testtools::run_in_isolated_process().map_err(anyhow::Error::from_boxed)? {
        return Ok(());
    }
    let _editor = gix_testtools::Env::new().set("GIT_SEQUENCE_EDITOR", ":");
    let (temp, commits) = fixture()?;
    let root = temp.path();
    let repo = gix::open(root)?;
    let mut session = Session::prepare_interactive(&repo, b"upstream".as_bstr())?.expect("interactive selection");
    session.accept_todo(
        &repo,
        format!("pick {}\nbreak\npick {}\n", commits[0], commits[1]).as_bytes(),
    )?;
    assert_eq!(session.advance(&repo)?, Advance::Started);
    assert!(matches!(session.advance(&repo)?, Advance::Step { .. }));
    assert_eq!(session.advance(&repo)?, Advance::Break);
    assert!(session.stage(&repo, &["first".into()], true).is_err());
    assert!(session.amend(&repo, b"forbidden\n").is_err());
    assert!(session.begin_split(&repo).is_err());
    assert!(
        session
            .accept_todo(&repo, format!("drop {}\ndrop {}\n", commits[0], commits[1]).as_bytes())
            .is_err()
    );
    session.accept_todo(&repo, format!("drop {}\n", commits[1]).as_bytes())?;
    session.skip(&repo)?;
    finish_remaining(&mut session, root)?;
    assert_eq!(git(root, &["show", "main:first"])?, b"first content\n");
    assert!(!root.join("second").exists());
    assert_preserved(root)?;
    Ok(())
}

#[test]
fn editor_precedence_honors_explicit_empty_values_and_sequence_overrides_normal_editor() -> Result<()> {
    use gitoxide_core::repository::replay::editors::{self, Kind};
    if gix_testtools::run_in_isolated_process().map_err(anyhow::Error::from_boxed)? {
        return Ok(());
    }
    let (temp, _) = fixture()?;
    let root = temp.path();
    let output = root.join(".git/editor-buffer");
    fs::write(&output, b"buffer\n")?;
    let _clean = gix_testtools::Env::new()
        .unset("GIT_SEQUENCE_EDITOR")
        .unset("GIT_EDITOR")
        .unset("VISUAL")
        .unset("EDITOR");
    git(root, &["config", "core.editor", "configured-normal"])?;
    git(root, &["config", "sequence.editor", "configured-sequence"])?;
    let repo = gix::open(root)?;
    assert_eq!(
        editors::resolve(&repo, Kind::Sequence, &output)?.program,
        b"configured-sequence"
    );
    assert_eq!(
        editors::resolve(&repo, Kind::Message, &output)?.program,
        b"configured-normal"
    );
    {
        let _env = gix_testtools::Env::new()
            .set("GIT_EDITOR", "environment-normal")
            .set("GIT_SEQUENCE_EDITOR", "environment-sequence");
        assert_eq!(
            editors::resolve(&repo, Kind::Sequence, &output)?.program,
            b"environment-sequence"
        );
        assert_eq!(
            editors::resolve(&repo, Kind::Message, &output)?.program,
            b"environment-normal"
        );
    }
    {
        let _empty = gix_testtools::Env::new().set("GIT_SEQUENCE_EDITOR", "");
        assert!(editors::resolve(&repo, Kind::Sequence, &output).is_err());
    }
    git(root, &["config", "--unset", "core.editor"])?;
    git(root, &["config", "--unset", "sequence.editor"])?;
    let repo = gix::open(root)?;
    {
        let _env = gix_testtools::Env::new()
            .set("VISUAL", "visual-program")
            .set("EDITOR", "editor-program");
        assert_eq!(
            editors::resolve(&repo, Kind::Message, &output)?.program,
            b"visual-program"
        );
    }
    {
        let _env = gix_testtools::Env::new().set("EDITOR", "editor-program");
        assert_eq!(
            editors::resolve(&repo, Kind::Sequence, &output)?.program,
            b"editor-program"
        );
    }
    assert!(
        editors::resolve(&repo, Kind::Message, &output).is_err(),
        "nonterminal fallback cannot launch vi"
    );
    Ok(())
}

#[cfg(feature = "switch-test-faults")]
#[test]
fn interrupted_editor_child_helper() -> Result<()> {
    let Some(root) = std::env::var_os("BRIT_TEST_INTERACTIVE_CHILD") else {
        return Ok(());
    };
    let repo = gix::open(Path::new(&root))?;
    if std::env::var_os("BRIT_TEST_RESUME_EDIT").is_some() {
        Session::open(&repo)?.continue_edit(&repo)?;
        return Ok(());
    }
    let mut session = Session::prepare_interactive(&repo, b"upstream".as_bstr())?.expect("interactive selection");
    if std::env::var_os("BRIT_TEST_AUTHOR_EDITOR").is_some() {
        let todo = session.todo()?;
        let first_edit = String::from_utf8(todo)?.replacen("pick ", "edit ", 1);
        session.accept_todo(&repo, first_edit.as_bytes())?;
        assert_eq!(session.advance(&repo)?, Advance::Started);
        assert_eq!(session.advance(&repo)?, Advance::Edit);
        if std::env::var_os("BRIT_TEST_ACCEPT_EDIT").is_some() {
            session.amend(&repo, b"accepted child amendment\n")?;
            session.continue_edit(&repo)?;
        } else {
            session.amend_edit(&repo)?;
        }
        Ok(())
    } else {
        session.edit_todo(&repo)
    }
}

#[cfg(feature = "switch-test-faults")]
#[test]
fn unknown_sequence_editor_accepts_validated_saved_bytes_without_relaunch() -> Result<()> {
    use std::{
        process::{Command, Stdio},
        time::{Duration, Instant},
    };
    if gix_testtools::run_in_isolated_process().map_err(anyhow::Error::from_boxed)? {
        return Ok(());
    }
    let (temp, commits) = fixture()?;
    let root = temp.path();
    git(
        root,
        &[
            "config",
            "sequence.editor",
            "printf 'once\\n' >> \"$GIT_DIR/editor-count\"",
        ],
    )?;
    let mut command = Command::new(std::env::current_exe()?);
    gix_testtools::configure_git_environment(&mut command, root);
    let mut child = command
        .args(["--exact", "interrupted_editor_child_helper", "--nocapture"])
        .env("BRIT_TEST_INTERACTIVE_CHILD", root)
        .env("BRIT_AUTHORING_INVOCATION_PAUSE", "after-wait")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::inherit())
        .spawn()?;
    let checkpoint = root.join(".git/brit-rebase/editor-0/checkpoint");
    let deadline = Instant::now() + Duration::from_secs(20);
    while !checkpoint.exists() {
        ensure!(child.try_wait()?.is_none(), "editor helper exited before pause");
        if Instant::now() >= deadline {
            child.kill()?;
            child.wait()?;
            anyhow::bail!("editor interruption checkpoint timeout");
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    child.kill()?;
    child.wait()?;
    let repo = gix::open(root)?;
    let mut session = Session::open(&repo)?;
    let saved = format!("drop {}\npick {}\n", commits[0], commits[1]).into_bytes();
    fs::write(session.editor_output().expect("saved uncertain output"), &saved)?;
    session.accept_editor(&repo)?;
    assert_eq!(session.todo()?, saved);
    assert_eq!(fs::read(root.join(".git/editor-count"))?, b"once\n");
    finish_remaining(&mut session, root)?;
    assert_eq!(fs::read(root.join(".git/editor-count"))?, b"once\n");
    assert_preserved(root)?;
    Ok(())
}

#[test]
fn supervised_editor_passes_non_utf8_shell_metacharacter_path_as_one_literal_argument() -> Result<()> {
    use gitoxide_core::repository::authoring::invocation::InvocationOutcome;
    use gitoxide_core::repository::replay::editors::{self, Kind};
    use std::os::unix::ffi::{OsStrExt, OsStringExt};
    if gix_testtools::run_in_isolated_process().map_err(anyhow::Error::from_boxed)? {
        return Ok(());
    }
    let (temp, _) = fixture()?;
    let root = temp.path();
    let output = root.join(".git").join(std::ffi::OsString::from_vec(
        b"message ' $(touch INJECTED) ; \xff".to_vec(),
    ));
    fs::write(&output, b"seed\n")?;
    fs::write(root.join(".git/editor-script"), b"#!/bin/sh\ntest \"$#\" = 2 || exit 21\ntest \"$1\" = 'fixed token' || exit 22\nprintf '%s' \"$2\" > \"$GIT_DIR/editor-argv\"\nprintf 'edited\\n' > \"$2\"\npwd > \"$GIT_DIR/editor-cwd\"\n")?;
    let _editor = gix_testtools::Env::new().set("GIT_EDITOR", "sh .git/editor-script 'fixed token'");
    let repo = gix::open(root)?;
    let plan = editors::resolve(&repo, Kind::Message, &output)?;
    let index = root.join(".git/editor-private-index");
    fs::copy(root.join(".git/index"), &index)?;
    let outcome = editors::run(&repo, &root.join(".git/editor-invocation"), &plan, &index, || Ok(()))?;
    assert_eq!(outcome, InvocationOutcome::Succeeded);
    assert_eq!(fs::read(root.join(".git/editor-argv"))?, output.as_os_str().as_bytes());
    assert_eq!(fs::read(output)?, b"edited\n");
    assert!(!root.join("INJECTED").exists());
    assert_eq!(
        fs::read(root.join(".git/editor-cwd"))?.trim(),
        root.as_os_str().as_bytes()
    );
    Ok(())
}

#[test]
fn unsupplied_edit_messages_follow_hook_editor_order_and_failed_output_continues_without_rerun() -> Result<()> {
    if gix_testtools::run_in_isolated_process().map_err(anyhow::Error::from_boxed)? {
        return Ok(());
    }
    let _sequence = gix_testtools::Env::new().set("GIT_SEQUENCE_EDITOR", ":");
    for split in [false, true] {
        for fail in [false, true] {
            let (temp, commits) = fixture()?;
            let root = temp.path();
            fs::write(
                root.join(".git/message-editor"),
                format!(
                    "#!/bin/sh\nprintf 'editor\\n' >> \"$GIT_DIR/author-order\"\nprintf 'edited message\\n' > \"$1\"\nexit {}\n",
                    if fail { 7 } else { 0 }
                ),
            )?;
            let _editor = gix_testtools::Env::new().set("GIT_EDITOR", "sh .git/message-editor");
            for name in ["pre-commit", "prepare-commit-msg", "commit-msg", "post-commit"] {
                hook(root, name, &format!("printf '{name}\\n' >> \"$GIT_DIR/author-order\""))?;
            }
            hook(
                root,
                "prepare-commit-msg",
                "printf 'prepare-commit-msg\\n' >> \"$GIT_DIR/author-order\"; printf '%s\\n' \"$#\" \"${2-}\" > \"$GIT_DIR/prepare-args\"",
            )?;
            hook(
                root,
                "post-rewrite",
                "printf 'post-rewrite:%s\\n' \"$1\" >> \"$GIT_DIR/author-order\"; cat >/dev/null",
            )?;
            let repo = gix::open(root)?;
            let mut session =
                Session::prepare_interactive(&repo, b"upstream".as_bstr())?.expect("interactive selection");
            session.accept_todo(&repo, format!("edit {}\ndrop {}\n", commits[0], commits[1]).as_bytes())?;
            assert_eq!(session.advance(&repo)?, Advance::Started);
            assert_eq!(session.advance(&repo)?, Advance::Edit);
            if split {
                session.begin_split(&repo)?;
                session.stage(&repo, &["first".into()], false)?;
            }
            let before = oid(root, "HEAD")?;
            let result = if split {
                session.commit_split_edit(&repo)
            } else {
                session.amend_edit(&repo)
            };
            if fail {
                assert!(result.is_err());
                assert_eq!(oid(root, "HEAD")?, before);
                assert_eq!(
                    fs::read(root.join(".git/author-order"))?,
                    b"pre-commit\nprepare-commit-msg\neditor\n"
                );
                fs::write(root.join(".git/brit-authoring/message"), b"explicit saved message\n")?;
            } else {
                result?;
            }
            session.continue_edit(&repo)?;
            let expected = if split {
                b"pre-commit\nprepare-commit-msg\neditor\ncommit-msg\npost-commit\n".as_slice()
            } else {
                b"pre-commit\nprepare-commit-msg\neditor\ncommit-msg\npost-commit\npost-rewrite:amend\n".as_slice()
            };
            assert_eq!(fs::read(root.join(".git/author-order"))?, expected);
            assert_eq!(
                fs::read(root.join(".git/prepare-args"))?,
                if split {
                    b"1\n\n".as_slice()
                } else {
                    b"3\ncommit\n".as_slice()
                }
            );
            finish_remaining(&mut session, root)?;
            assert_preserved(root)?;
        }
    }
    Ok(())
}

#[test]
fn reduced_trust_refuses_editor_resolution_before_invocation() -> Result<()> {
    use gitoxide_core::repository::replay::editors::{self, Kind};
    if gix_testtools::run_in_isolated_process().map_err(anyhow::Error::from_boxed)? {
        return Ok(());
    }
    let (temp, _) = fixture()?;
    let root = temp.path();
    let output = root.join(".git/editor-buffer");
    fs::write(&output, b"seed\n")?;
    let _editor = gix_testtools::Env::new().set("GIT_EDITOR", "touch UNTRUSTED_EXECUTED");
    let repo = gix::open_opts(root, gix::open::Options::default().with(gix::sec::Trust::Reduced))?;
    assert!(editors::resolve(&repo, Kind::Message, &output).is_err());
    assert!(!root.join("UNTRUSTED_EXECUTED").exists());
    Ok(())
}

#[test]
fn terminal_editor_child_helper() -> Result<()> {
    use gitoxide_core::repository::{
        authoring::invocation::InvocationOutcome,
        replay::editors::{self, Kind},
    };
    let Some(root) = std::env::var_os("BRIT_TEST_TERMINAL_EDITOR") else {
        return Ok(());
    };
    let root = Path::new(&root);
    let repo = gix::open(root)?;
    let output = root.join(".git/terminal-message");
    fs::write(&output, b"seed\n")?;
    let plan = editors::resolve(&repo, Kind::Message, &output)?;
    assert!(plan.terminal, "child owns an interactive PTY");
    let result = editors::run(
        &repo,
        &root.join(".git/terminal-invocation"),
        &plan,
        &root.join(".git/index"),
        || Ok(()),
    )?;
    assert_ne!(result, InvocationOutcome::Succeeded, "Ctrl-C interrupts editor");
    println!("PTY_EDITOR_RECOVERED");
    Ok(())
}

#[test]
fn terminal_editor_ctrl_z_fg_and_ctrl_c_return_terminal_to_calling_shell() -> Result<()> {
    use std::process::Command;
    if gix_testtools::run_in_isolated_process().map_err(anyhow::Error::from_boxed)? {
        return Ok(());
    }
    let (temp, _) = fixture()?;
    let root = temp.path();
    fs::write(
        root.join(".git/terminal-editor"),
        b"#!/bin/sh\nprintf 'PTY_EDITOR_READY\\n'\nexec cat >/dev/null\n",
    )?;
    let script = r#"
import os, pty, select, shlex, signal, sys, time
pid, fd = pty.fork()
if pid == 0:
    os.environ['PS1'] = 'BRIT_PTY_PROMPT> '
    os.environ['PROMPT_COMMAND'] = ''
    os.execvp('bash', ['bash', '--noprofile', '--norc', '-i'])
data = b''
transcript = b''
def until(needle):
    global data, transcript
    end = time.monotonic() + 20
    while needle not in data:
        if time.monotonic() > end:
            raise RuntimeError('PTY timeout waiting for %r: %r' % (needle, data))
        if select.select([fd], [], [], 0.1)[0]:
            try:
                chunk = os.read(fd, 65536)
            except OSError as error:
                raise RuntimeError('PTY closed waiting for %r, transcript=%r' % (needle, transcript)) from error
            if not chunk:
                raise RuntimeError('PTY EOF waiting for %r, transcript=%r' % (needle, transcript))
            data += chunk
            transcript += chunk
    data = data.split(needle, 1)[1]
def wait_state(predicate, description):
    end = time.monotonic() + 20
    while not predicate():
        if time.monotonic() >= end:
            raise RuntimeError('PTY state timeout: %s, transcript=%r' % (description() if callable(description) else description, transcript))
        time.sleep(0.01)
def process_state(process):
    with open('/proc/%d/stat' % process) as file:
        return file.read().rsplit(')', 1)[1].split()[0]
try:
    # Wait until bash has finished restoring terminal modes before typing.
    # Its stop notification precedes that restoration and may flush early input.
    until(b'BRIT_PTY_PROMPT> ')
    command = shlex.quote(sys.argv[1]) + ' --exact terminal_editor_child_helper --nocapture\n'
    os.write(fd, command.encode())
    until(b'PTY_EDITOR_READY\r\n')
    with open('/proc/%d/task/%d/children' % (pid, pid)) as file:
        children = [int(child) for child in file.read().split()]
    assert len(children) == 1, children
    supervisor = children[0]
    editor_group = os.tcgetpgrp(fd)
    assert editor_group not in (pid, supervisor)
    os.write(fd, b'\x1a')
    until(b'Stopped')
    until(b'BRIT_PTY_PROMPT> ')
    for background_attempt in range(2):
        os.write(fd, b'bg; printf "PTY_BG_ACK=%s\\n" "$?"\n')
        until(b'PTY_BG_ACK=0\r\n')
        until(b'BRIT_PTY_PROMPT> ')
        bg_states = []
        def background_stopped():
            foreground = os.tcgetpgrp(fd)
            state = process_state(supervisor)
            observation = (foreground, state, process_state(editor_group))
            if not bg_states or bg_states[-1] != observation:
                bg_states.append(observation)
            assert foreground != editor_group, 'bg editor stole terminal: shell=%d shellpgid=%d foreground=%d supervisor=%d supervisorpgid=%d editor=%d states=%r transcript=%r' % (pid, os.getpgid(pid), foreground, supervisor, os.getpgid(supervisor), editor_group, bg_states, transcript)
            return state == 'T' and foreground == pid
        wait_state(background_stopped, lambda: 'bg must stop supervisor and leave shell foreground: shell=%d shellpgid=%d supervisor=%d supervisorpgid=%d editor=%d foreground=%d states=%r' % (pid, os.getpgid(pid), supervisor, os.getpgid(supervisor), editor_group, os.tcgetpgrp(fd), bg_states))
    os.write(fd, b'fg\n')
    wait_state(lambda: os.tcgetpgrp(fd) == editor_group and process_state(editor_group) != 'T', 'fg must return terminal to running editor')
    os.write(fd, b'\x03')
    until(b'PTY_EDITOR_RECOVERED\r\n')
    until(b'BRIT_PTY_PROMPT> ')
    os.write(fd, b'printf "PTY_FG_EXIT=%s\\n" "$?"\n')
    until(b'PTY_FG_EXIT=0\r\n')
    until(b'BRIT_PTY_PROMPT> ')
    os.write(fd, b'printf "PTY_SHELL_REGAINED\\n"\n')
    until(b'PTY_SHELL_REGAINED\r\n')
finally:
    for entry in os.listdir('/proc'):
        if entry.isdigit():
            try:
                child = int(entry)
                if child != pid and os.getsid(child) == pid:
                    os.kill(child, signal.SIGKILL)
            except ProcessLookupError:
                pass
    os.kill(pid, signal.SIGKILL)
    os.waitpid(pid, 0)
    os.close(fd)
"#;
    let mut command = Command::new("python3");
    gix_testtools::configure_git_environment(&mut command, root);
    let output = command
        .args(["-c", script])
        .arg(std::env::current_exe()?)
        .env("BRIT_TEST_TERMINAL_EDITOR", root)
        .env("GIT_EDITOR", "sh .git/terminal-editor")
        .env("LC_ALL", "C")
        .output()?;
    ensure!(
        output.status.success(),
        "PTY supervision: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    Ok(())
}

#[cfg(feature = "switch-test-faults")]
#[test]
fn unknown_edit_author_message_is_explicitly_accepted_without_second_editor() -> Result<()> {
    use std::{
        process::{Command, Stdio},
        time::{Duration, Instant},
    };
    if gix_testtools::run_in_isolated_process().map_err(anyhow::Error::from_boxed)? {
        return Ok(());
    }
    let (temp, _) = fixture()?;
    let root = temp.path();
    fs::write(
        root.join(".git/message-editor"),
        b"#!/bin/sh\nprintf 'once\\n' >> \"$GIT_DIR/editor-count\"\nprintf 'unknown saved message\\n' > \"$1\"\n",
    )?;
    git(root, &["config", "core.editor", "sh .git/message-editor"])?;
    let mut command = Command::new(std::env::current_exe()?);
    gix_testtools::configure_git_environment(&mut command, root);
    let mut child = command
        .args(["--exact", "interrupted_editor_child_helper", "--nocapture"])
        .env("BRIT_TEST_INTERACTIVE_CHILD", root)
        .env("BRIT_TEST_AUTHOR_EDITOR", "1")
        .env("BRIT_AUTHORING_INVOCATION_PAUSE", "after-wait")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::inherit())
        .spawn()?;
    let checkpoint = root.join(".git/brit-authoring/invocation-1/checkpoint");
    let deadline = Instant::now() + Duration::from_secs(20);
    while !checkpoint.exists() {
        ensure!(child.try_wait()?.is_none(), "author editor helper exited before pause");
        if Instant::now() >= deadline {
            child.kill()?;
            child.wait()?;
            anyhow::bail!("author editor checkpoint timeout");
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    child.kill()?;
    child.wait()?;
    let repo = gix::open(root)?;
    let mut session = Session::open(&repo)?;
    session.continue_edit(&repo)?;
    assert_eq!(
        git(root, &["show", "-s", "--format=%B", "HEAD"])?,
        b"unknown saved message\n\n"
    );
    finish_remaining(&mut session, root)?;
    assert_eq!(fs::read(root.join(".git/editor-count"))?, b"once\n");
    assert_preserved(root)?;
    Ok(())
}

#[cfg(feature = "switch-test-faults")]
#[test]
fn real_signals_at_editor_planning_acceptance_and_edit_receipt_do_not_rerun_or_lose_authoring() -> Result<()> {
    use std::{
        process::{Command, Stdio},
        time::{Duration, Instant},
    };
    if gix_testtools::run_in_isolated_process().map_err(anyhow::Error::from_boxed)? {
        return Ok(());
    }
    for checkpoint in [
        "replay-editor-planned",
        "replay-editor-accepted",
        "replay-edit-accepted",
    ] {
        for signal in ["INT", "KILL"] {
            let (temp, _) = fixture()?;
            let root = temp.path();
            git(
                root,
                &[
                    "config",
                    "sequence.editor",
                    "printf 'once\\n' >> \"$GIT_DIR/editor-count\"",
                ],
            )?;
            let before_head = fs::read(root.join(".git/HEAD"))?;
            let before_index = fs::read(root.join(".git/index"))?;
            let checkpoints = tempfile::tempdir()?;
            let mut command = Command::new(std::env::current_exe()?);
            gix_testtools::configure_git_environment(&mut command, root);
            command
                .args(["--exact", "interrupted_editor_child_helper", "--nocapture"])
                .env("BRIT_TEST_INTERACTIVE_CHILD", root)
                .env("BRIT_SWITCH_CHECKPOINT", checkpoint)
                .env("BRIT_SWITCH_CHECKPOINT_DIR", checkpoints.path())
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::inherit());
            if checkpoint == "replay-edit-accepted" {
                command
                    .env("BRIT_TEST_AUTHOR_EDITOR", "1")
                    .env("BRIT_TEST_ACCEPT_EDIT", "1");
            }
            let mut child = command.spawn()?;
            let reached = checkpoints.path().join(format!("{checkpoint}.reached"));
            let deadline = Instant::now() + Duration::from_secs(20);
            while !reached.exists() {
                ensure!(child.try_wait()?.is_none(), "helper exited before {checkpoint}");
                if Instant::now() >= deadline {
                    child.kill()?;
                    child.wait()?;
                    anyhow::bail!("checkpoint timeout: {checkpoint}");
                }
                std::thread::sleep(Duration::from_millis(10));
            }
            ensure!(
                Command::new("/bin/kill")
                    .args(["-s", signal, &child.id().to_string()])
                    .status()?
                    .success(),
                "signal helper"
            );
            assert!(!child.wait()?.success());
            if checkpoint != "replay-edit-accepted" {
                assert_eq!(fs::read(root.join(".git/HEAD"))?, before_head);
                assert_eq!(fs::read(root.join(".git/index"))?, before_index);
            }
            let repo = gix::open(root)?;
            let mut session = Session::open(&repo)?;
            finish_remaining(&mut session, root)?;
            if checkpoint == "replay-editor-accepted" {
                assert_eq!(fs::read(root.join(".git/editor-count"))?, b"once\n");
            } else {
                assert!(
                    !root.join(".git/editor-count").exists(),
                    "no new editor launch on recovery"
                );
            }
            if checkpoint == "replay-edit-accepted" {
                assert_eq!(
                    git(root, &["show", "-s", "--format=%s", "main^"])?,
                    b"accepted child amendment\n"
                );
            }
            assert_preserved(root)?;
        }
    }
    Ok(())
}

#[test]
fn sequence_editor_mutation_of_work_or_private_index_refuses_saved_acceptance() -> Result<()> {
    if gix_testtools::run_in_isolated_process().map_err(anyhow::Error::from_boxed)? {
        return Ok(());
    }
    for script in [
        "printf 'mutated\\n' > first",
        "printf 'mutated\\n' > neighbor",
        "printf 'mutated\\n' > \"$GIT_INDEX_FILE\"",
    ] {
        let (temp, _) = fixture()?;
        let root = temp.path();
        let _editor = gix_testtools::Env::new().set("GIT_SEQUENCE_EDITOR", script);
        let repo = gix::open(root)?;
        let mut session = Session::prepare_interactive(&repo, b"upstream".as_bstr())?.expect("interactive selection");
        let accepted = session.todo()?;
        let original_head = oid(root, "HEAD")?;
        assert!(session.edit_todo(&repo).is_err());
        session = Session::open(&repo)?;
        assert!(session.accept_editor(&repo).is_err());
        assert_eq!(session.todo()?, accepted);
        assert_eq!(oid(root, "HEAD")?, original_head);
    }
    Ok(())
}

#[test]
fn squash_conflict_skip_removes_only_current_input_and_retains_prior_group() -> Result<()> {
    if gix_testtools::run_in_isolated_process().map_err(anyhow::Error::from_boxed)? {
        return Ok(());
    }
    let _editors = gix_testtools::Env::new()
        .set("GIT_SEQUENCE_EDITOR", ":")
        .set("GIT_EDITOR", ":");
    let (temp, commits) = fixture()?;
    let root = temp.path();
    git(root, &["reset", "-q", "HEAD", "--", "neighbor"])?;
    fs::write(root.join("neighbor"), b"clean neighbor\n")?;
    git(root, &["switch", "-q", "upstream"])?;
    commit(root, "second", b"upstream conflicting addition\n", "upstream conflict")?;
    git(root, &["switch", "-q", "main"])?;
    fs::write(root.join("neighbor"), b"preserved staged\n")?;
    git(root, &["add", "neighbor"])?;
    fs::write(root.join("neighbor"), b"preserved unstaged\n")?;
    hook(root, "post-rewrite", "cat > \"$GIT_DIR/rewrite-pairs\"")?;
    let repo = gix::open(root)?;
    let mut session = Session::prepare_interactive(&repo, b"upstream".as_bstr())?.expect("interactive selection");
    session.accept_todo(
        &repo,
        format!("pick {}\nsquash {}\n", commits[0], commits[1]).as_bytes(),
    )?;
    assert_eq!(session.advance(&repo)?, Advance::Started);
    let Advance::Step { commit: group } = session.advance(&repo)? else {
        anyhow::bail!("first retained group");
    };
    assert_eq!(session.advance(&repo)?, Advance::Conflict);
    fs::write(root.join("second"), b"discarded conflict work\n")?;
    let receipt = session.skip(&repo)?;
    gitoxide_core::repository::replay::retained::inspect(&receipt.path)?;
    assert_eq!(oid(root, "HEAD")?, group);
    finish_remaining(&mut session, root)?;
    assert_eq!(
        fs::read(root.join(".git/rewrite-pairs"))?,
        format!("{} {group}\n", commits[0]).into_bytes()
    );
    assert_eq!(git(root, &["show", "main:second"])?, b"upstream conflicting addition\n");
    assert_preserved(root)?;
    Ok(())
}

#[test]
fn skipping_edit_split_removes_whole_authored_chain_but_retains_its_raw_objects() -> Result<()> {
    use gitoxide_core::repository::replay::retained::{self, Area};
    if gix_testtools::run_in_isolated_process().map_err(anyhow::Error::from_boxed)? {
        return Ok(());
    }
    let _editor = gix_testtools::Env::new().set("GIT_SEQUENCE_EDITOR", ":");
    let (temp, commits) = fixture()?;
    let root = temp.path();
    hook(root, "post-rewrite", "cat > \"$GIT_DIR/rewrite-pairs\"")?;
    let repo = gix::open(root)?;
    let mut session = Session::prepare_interactive(&repo, b"upstream".as_bstr())?.expect("interactive selection");
    session.accept_todo(&repo, format!("edit {}\npick {}\n", commits[0], commits[1]).as_bytes())?;
    assert_eq!(session.advance(&repo)?, Advance::Started);
    assert_eq!(session.advance(&repo)?, Advance::Edit);
    session.begin_split(&repo)?;
    fs::write(root.join("first"), b"authored split that will be skipped\n")?;
    session.stage(&repo, &["first".into()], false)?;
    session.commit_split(&repo, b"retained split\n")?;
    let authored = oid(root, "HEAD")?;
    let bytes = git(root, &["cat-file", "commit", &authored.to_string()])?;
    let receipt = session.skip(&repo)?;
    assert_eq!(oid(root, "HEAD")?, oid(root, "upstream")?);
    assert!(!root.join("first").exists());
    finish_remaining(&mut session, root)?;
    let final_id = oid(root, "main")?;
    assert_eq!(
        fs::read(root.join(".git/rewrite-pairs"))?,
        format!("{} {final_id}\n", commits[1]).into_bytes()
    );
    let copy = retained::inspect(&receipt.path)?;
    let ordinal = copy
        .manifest
        .entries
        .iter()
        .position(|entry| entry.area == Area::Objects && entry.path == authored.to_string().as_bytes())
        .expect("skipped authored commit retained");
    assert_eq!(copy.payload(ordinal)?, bytes);
    assert_preserved(root)?;
    Ok(())
}

#[test]
fn failed_commit_message_hook_scoped_mutations_allow_explicit_retry_or_preserving_abort() -> Result<()> {
    use gitoxide_core::repository::replay::retained;
    if gix_testtools::run_in_isolated_process().map_err(anyhow::Error::from_boxed)? {
        return Ok(());
    }
    let _editors = gix_testtools::Env::new()
        .set("GIT_SEQUENCE_EDITOR", ":")
        .set("GIT_EDITOR", "sh .git/message-editor");
    for abort in [false, true] {
        let (temp, commits) = fixture()?;
        let root = temp.path();
        let original_index = fs::read(root.join(".git/index"))?;
        let original_head = fs::read(root.join(".git/HEAD"))?;
        fs::write(
            root.join(".git/message-editor"),
            b"#!/bin/sh\nprintf 'editor\\n' >> \"$GIT_DIR/order\"\nprintf 'edited\\n' > \"$1\"\n",
        )?;
        for name in ["pre-commit", "prepare-commit-msg", "post-commit"] {
            hook(root, name, &format!("printf '{name}\\n' >> \"$GIT_DIR/order\""))?;
        }
        hook(
            root,
            "commit-msg",
            "printf 'commit-msg\\n' >> \"$GIT_DIR/order\"; if test ! -f \"$GIT_DIR/hook-ran\"; then printf 'hook changed active work\\n' > first; cp \"$GIT_DIR/replacement-index\" \"$GIT_INDEX_FILE\"; : > \"$GIT_DIR/hook-ran\"; exit 9; fi",
        )?;
        let repo = gix::open(root)?;
        let mut session = Session::prepare_interactive(&repo, b"upstream".as_bstr())?.expect("interactive selection");
        session.accept_todo(&repo, format!("edit {}\ndrop {}\n", commits[0], commits[1]).as_bytes())?;
        assert_eq!(session.advance(&repo)?, Advance::Started);
        assert_eq!(session.advance(&repo)?, Advance::Edit);
        let replacement = root.join(".git/replacement-index");
        for args in [vec!["read-tree", "HEAD"], vec!["add", "--", "first"]] {
            if args[0] == "add" {
                fs::write(root.join("first"), b"hook changed active work\n")?;
            }
            let output = gix_testtools::git_command(root)
                .env("GIT_INDEX_FILE", &replacement)
                .args(args)
                .output()?;
            ensure!(
                output.status.success(),
                "fixture private index: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }
        fs::write(root.join("first"), b"first content\n")?;
        let before_authoring = oid(root, "HEAD")?;
        assert!(session.amend_edit(&repo).is_err());
        assert_eq!(oid(root, "HEAD")?, before_authoring);
        assert_eq!(git(root, &["show", ":first"])?, b"hook changed active work\n");
        assert_eq!(fs::read(root.join("first"))?, b"hook changed active work\n");
        session = Session::open(&repo)?;
        if abort {
            let receipt = session.abort(&repo)?;
            assert_eq!(fs::read(root.join(".git/index"))?, original_index);
            assert_eq!(fs::read(root.join(".git/HEAD"))?, original_head);
            let copy = retained::inspect(&receipt.path)?;
            let mut found = false;
            for ordinal in 0..copy.manifest.entries.len() {
                found |= copy.payload(ordinal)? == b"hook changed active work\n";
            }
            assert!(found, "abort retains hook-authored active work");
            assert_eq!(
                fs::read(root.join(".git/order"))?,
                b"pre-commit\nprepare-commit-msg\neditor\ncommit-msg\n"
            );
        } else {
            session.continue_edit(&repo)?;
            finish_remaining(&mut session, root)?;
            assert_eq!(git(root, &["show", "main:first"])?, b"hook changed active work\n");
            assert_eq!(
                fs::read(root.join(".git/order"))?,
                b"pre-commit\nprepare-commit-msg\neditor\ncommit-msg\ncommit-msg\npost-commit\n"
            );
        }
        assert_preserved(root)?;
    }
    Ok(())
}

#[test]
fn abort_after_nested_edit_scope_expansion_retains_user_created_directory_identity() -> Result<()> {
    use std::os::unix::fs::MetadataExt;
    if gix_testtools::run_in_isolated_process().map_err(anyhow::Error::from_boxed)? {
        return Ok(());
    }
    let _editor = gix_testtools::Env::new().set("GIT_SEQUENCE_EDITOR", ":");
    let (temp, commits) = fixture()?;
    let root = temp.path();
    let original_index = fs::read(root.join(".git/index"))?;
    let repo = gix::open(root)?;
    let mut session = Session::prepare_interactive(&repo, b"upstream".as_bstr())?.expect("interactive selection");
    session.accept_todo(&repo, format!("edit {}\npick {}\n", commits[0], commits[1]).as_bytes())?;
    assert_eq!(session.advance(&repo)?, Advance::Started);
    assert_eq!(session.advance(&repo)?, Advance::Edit);
    fs::create_dir_all(root.join("user-created/nested"))?;
    fs::write(root.join("user-created/nested/authored"), b"expanded authored bytes\n")?;
    let identity = fs::metadata(root.join("user-created/nested"))?;
    session.stage(&repo, &["user-created/nested/authored".into()], true)?;
    session.amend(&repo, b"expanded amendment\n")?;
    session.continue_edit(&repo)?;
    fs::write(
        root.join("user-created/nested/unrelated"),
        b"unrelated neighbor bytes\n",
    )?;
    let receipt = session.abort(&repo)?;
    assert_eq!(fs::read(root.join(".git/index"))?, original_index);
    let after = fs::metadata(root.join("user-created/nested"))?;
    assert_eq!(
        (after.dev(), after.ino()),
        (identity.dev(), identity.ino()),
        "abort cannot delete a user-created directory it never allocated"
    );
    assert_eq!(
        fs::read(root.join("user-created/nested/unrelated"))?,
        b"unrelated neighbor bytes\n"
    );
    gitoxide_core::repository::replay::retained::inspect(&receipt.path)?;
    assert_preserved(root)?;
    Ok(())
}

#[test]
fn superseded_edit_amendments_have_owned_refs_and_survive_gc_in_independent_abort_copy() -> Result<()> {
    use gitoxide_core::repository::replay::retained::{self, Area};
    if gix_testtools::run_in_isolated_process().map_err(anyhow::Error::from_boxed)? {
        return Ok(());
    }
    let _editor = gix_testtools::Env::new().set("GIT_SEQUENCE_EDITOR", ":");
    let (temp, commits) = fixture()?;
    let root = temp.path();
    let repo = gix::open(root)?;
    let mut session = Session::prepare_interactive(&repo, b"upstream".as_bstr())?.expect("interactive selection");
    session.accept_todo(&repo, format!("edit {}\npick {}\n", commits[0], commits[1]).as_bytes())?;
    assert_eq!(session.advance(&repo)?, Advance::Started);
    assert_eq!(session.advance(&repo)?, Advance::Edit);
    let mut authored = Vec::new();
    for number in 1..=2 {
        fs::write(root.join("first"), format!("amendment {number}\n"))?;
        session.stage(&repo, &["first".into()], false)?;
        session.amend(&repo, format!("amendment {number}\n").as_bytes())?;
        let id = oid(root, "HEAD")?;
        authored.push((id, git(root, &["cat-file", "commit", &id.to_string()])?));
    }
    let refs = git(
        root,
        &["for-each-ref", "--format=%(objectname)", "refs/brit-rebase-retain/"],
    )?;
    for (id, _) in &authored {
        assert!(
            refs.split(|byte| *byte == b'\n')
                .any(|line| line == id.to_string().as_bytes()),
            "every superseded authored HEAD has owned retention"
        );
    }
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
    for (id, bytes) in &authored {
        assert_eq!(git(root, &["cat-file", "commit", &id.to_string()])?, *bytes);
    }
    session = Session::open(&gix::open(root)?)?;
    session.abort(&gix::open(root)?)?;
    let receipt = session.cleanup(&gix::open(root)?)?;
    git(root, &["reflog", "expire", "--expire=now", "--all"])?;
    git(root, &["gc", "--prune=now"])?;
    let copy = retained::inspect(&receipt.path)?;
    for (id, bytes) in authored {
        let ordinal = copy
            .manifest
            .entries
            .iter()
            .position(|entry| entry.area == Area::Objects && entry.path == id.to_string().as_bytes())
            .expect("superseded authored object remains after cleanup");
        assert_eq!(copy.payload(ordinal)?, bytes);
    }
    assert_preserved(root)?;
    Ok(())
}

#[cfg(feature = "switch-test-faults")]
fn interrupt_amend(root: &Path, checkpoint: &str, signal: &str) -> Result<()> {
    use std::{
        process::{Command, Stdio},
        time::{Duration, Instant},
    };
    let checkpoints = tempfile::tempdir()?;
    let mut command = Command::new(std::env::current_exe()?);
    gix_testtools::configure_git_environment(&mut command, root);
    let mut child = command
        .args(["--exact", "interrupted_editor_child_helper", "--nocapture"])
        .env("BRIT_TEST_INTERACTIVE_CHILD", root)
        .env("BRIT_TEST_AUTHOR_EDITOR", "1")
        .env("BRIT_TEST_ACCEPT_EDIT", "1")
        .env("BRIT_SWITCH_CHECKPOINT", checkpoint)
        .env("BRIT_SWITCH_CHECKPOINT_DIR", checkpoints.path())
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::inherit())
        .spawn()?;
    let reached = checkpoints.path().join(format!("{checkpoint}.reached"));
    let deadline = Instant::now() + Duration::from_secs(20);
    while !reached.exists() {
        ensure!(child.try_wait()?.is_none(), "amend helper exited before {checkpoint}");
        if Instant::now() >= deadline {
            child.kill()?;
            child.wait()?;
            anyhow::bail!("amend checkpoint timeout {checkpoint}");
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    ensure!(
        Command::new("/bin/kill")
            .args(["-s", signal, &child.id().to_string()])
            .status()?
            .success(),
        "signal amend helper"
    );
    assert!(!child.wait()?.success());
    Ok(())
}

#[cfg(feature = "switch-test-faults")]
#[test]
fn durable_child_proposal_recovers_fixed_commit_after_gc_without_repeating_hooks() -> Result<()> {
    if gix_testtools::run_in_isolated_process().map_err(anyhow::Error::from_boxed)? {
        return Ok(());
    }
    for signal in ["INT", "KILL"] {
        let (temp, _) = fixture()?;
        let root = temp.path();
        for name in ["pre-commit", "prepare-commit-msg", "commit-msg"] {
            hook(
                root,
                name,
                &format!("printf '{name}\\n' >> \"$GIT_DIR/proposal-hooks\""),
            )?;
        }
        hook(
            root,
            "post-commit",
            "printf 'post-commit\\n' >> \"$GIT_DIR/proposal-post-hooks\"",
        )?;
        hook(
            root,
            "post-rewrite",
            "printf 'post-rewrite:%s\\n' \"$1\" >> \"$GIT_DIR/proposal-post-hooks\"; cat >/dev/null",
        )?;
        interrupt_amend(root, "replay-child-proposal-durable", signal)?;
        let proposals = fs::read_dir(root.join(".git/brit-authoring"))?
            .filter_map(|entry| entry.ok())
            .filter(|entry| entry.file_name().to_string_lossy().ends_with("-proposed.json"))
            .map(|entry| entry.path())
            .collect::<Vec<_>>();
        assert_eq!(proposals.len(), 1);
        let envelope: serde_json::Value = serde_json::from_slice(&fs::read(&proposals[0])?)?;
        let payload: Vec<u8> = serde_json::from_value(envelope["payload"].clone())?;
        let decision: serde_json::Value = serde_json::from_slice(&payload)?;
        let proposed = decision["after"]["head"].as_str().expect("proposed child HEAD");
        let bytes = git(root, &["cat-file", "commit", proposed])?;
        let hooks = fs::read(root.join(".git/proposal-hooks"))?;
        assert!(!root.join(".git/proposal-post-hooks").exists());
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
        let present = gix_testtools::git_command(root)
            .args(["cat-file", "-e", proposed])
            .output()?;
        assert!(
            !present.status.success(),
            "unreferenced proposal is removed so recovery must use its raw receipt"
        );
        let repo = gix::open(root)?;
        let mut session = Session::open(&repo)?;
        session.recover_child(&repo)?;
        assert_eq!(oid(root, "HEAD")?.to_string(), proposed);
        assert_eq!(git(root, &["cat-file", "commit", proposed])?, bytes);
        assert_eq!(fs::read(root.join(".git/proposal-hooks"))?, hooks);
        assert!(
            !root.join(".git/proposal-post-hooks").exists(),
            "recovery publishes the decision but cannot imply notification delivery"
        );
        session.continue_edit(&repo)?;
        assert_eq!(
            fs::read(root.join(".git/proposal-post-hooks"))?,
            b"post-commit\npost-rewrite:amend\n"
        );
        finish_remaining(&mut session, root)?;
        assert_eq!(fs::read(root.join(".git/proposal-hooks"))?, hooks);
        assert_eq!(
            fs::read(root.join(".git/proposal-post-hooks"))?,
            b"post-commit\npost-rewrite:amend\npost-rewrite:rebase\n"
        );
        assert_preserved(root)?;
    }
    Ok(())
}

#[cfg(feature = "switch-test-faults")]
#[test]
fn interrupted_child_raw_object_temporary_is_retained_opaquely_on_abort() -> Result<()> {
    use gitoxide_core::repository::replay::retained;
    if gix_testtools::run_in_isolated_process().map_err(anyhow::Error::from_boxed)? {
        return Ok(());
    }
    for checkpoint in [
        "replay-child-object-temp-created",
        "replay-child-object-temp-written",
        "replay-child-object-temp-synced",
    ] {
        let (temp, _) = fixture()?;
        let root = temp.path();
        let original_index = fs::read(root.join(".git/index"))?;
        interrupt_amend(root, checkpoint, "KILL")?;
        let objects = root.join(".git/brit-authoring/objects");
        let partials = fs::read_dir(&objects)?
            .filter_map(|entry| entry.ok())
            .filter(|entry| entry.path().extension().is_some_and(|extension| extension == "new"))
            .map(|entry| (entry.path(), fs::read(entry.path())))
            .collect::<Vec<_>>();
        assert_eq!(partials.len(), 1);
        let raw = partials[0].1.as_ref().expect("read interrupted raw object").clone();
        let repo = gix::open(root)?;
        let mut session = Session::open(&repo)?;
        let receipt = session.abort(&repo)?;
        assert_eq!(fs::read(root.join(".git/index"))?, original_index);
        let copy = retained::inspect(&receipt.path)?;
        let mut preserved = false;
        for (ordinal, entry) in copy.manifest.entries.iter().enumerate() {
            if entry.path.ends_with(b".new") && copy.payload(ordinal)? == raw {
                preserved = true;
            }
        }
        assert!(
            preserved,
            "partial raw receipt is opaque evidence, not a published object"
        );
        assert!(
            session.cleanup(&repo).is_err(),
            "unproven raw receipt keeps cleanup pending"
        );
        assert_preserved(root)?;
    }
    Ok(())
}

#[cfg(feature = "switch-test-faults")]
#[test]
fn unknown_first_queued_post_hook_is_acknowledged_once_before_second_hook_runs() -> Result<()> {
    use std::{
        process::{Command, Stdio},
        time::{Duration, Instant},
    };
    if gix_testtools::run_in_isolated_process().map_err(anyhow::Error::from_boxed)? {
        return Ok(());
    }
    let (temp, _) = fixture()?;
    let root = temp.path();
    for name in ["pre-commit", "prepare-commit-msg", "commit-msg"] {
        hook(root, name, &format!("printf '{name}\\n' >> \"$GIT_DIR/pre-count\""))?;
    }
    hook(
        root,
        "post-commit",
        "printf 'post-commit\\n' >> \"$GIT_DIR/post-count\"",
    )?;
    hook(
        root,
        "post-rewrite",
        "printf 'post-rewrite:%s\\n' \"$1\" >> \"$GIT_DIR/post-count\"; cat >/dev/null",
    )?;
    interrupt_amend(root, "replay-child-proposal-durable", "KILL")?;
    let repo = gix::open(root)?;
    Session::open(&repo)?.recover_child(&repo)?;
    assert!(!root.join(".git/post-count").exists());
    let pre = fs::read(root.join(".git/pre-count"))?;
    let mut command = Command::new(std::env::current_exe()?);
    gix_testtools::configure_git_environment(&mut command, root);
    let mut child = command
        .args(["--exact", "interrupted_editor_child_helper", "--nocapture"])
        .env("BRIT_TEST_INTERACTIVE_CHILD", root)
        .env("BRIT_TEST_RESUME_EDIT", "1")
        .env("BRIT_AUTHORING_INVOCATION_PAUSE", "after-wait")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::inherit())
        .spawn()?;
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        let reached = fs::read_dir(root.join(".git/brit-authoring"))?
            .filter_map(|entry| entry.ok())
            .any(|entry| entry.path().join("checkpoint").is_file());
        if reached {
            break;
        }
        ensure!(
            child.try_wait()?.is_none(),
            "queued post-hook helper exited before pause"
        );
        if Instant::now() >= deadline {
            child.kill()?;
            child.wait()?;
            anyhow::bail!("queued post-hook pause timeout");
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    child.kill()?;
    child.wait()?;
    assert_eq!(fs::read(root.join(".git/post-count"))?, b"post-commit\n");
    let mut session = Session::open(&repo)?;
    session.continue_edit(&repo)?;
    assert_eq!(
        fs::read(root.join(".git/post-count"))?,
        b"post-commit\npost-rewrite:amend\n"
    );
    assert_eq!(fs::read(root.join(".git/pre-count"))?, pre);
    finish_remaining(&mut session, root)?;
    assert_eq!(
        fs::read(root.join(".git/post-count"))?,
        b"post-commit\npost-rewrite:amend\npost-rewrite:rebase\n"
    );
    assert_eq!(fs::read(root.join(".git/pre-count"))?, pre);
    assert_preserved(root)?;
    Ok(())
}

#[test]
fn todo_edit_at_break_and_edit_stop_keeps_unchanged_ancestor_out_of_preserved_sibling_scope() -> Result<()> {
    use std::os::unix::fs::MetadataExt;
    if gix_testtools::run_in_isolated_process().map_err(anyhow::Error::from_boxed)? {
        return Ok(());
    }
    let _editor = gix_testtools::Env::new().set("GIT_SEQUENCE_EDITOR", ":");
    for edit_stop in [false, true] {
        let temp = tempfile::tempdir()?;
        let root = temp.path();
        git(root, &["init", "-q", "-b", "main"])?;
        git(root, &["config", "user.name", "Directory Scope"])?;
        git(root, &["config", "user.email", "scope@example.invalid"])?;
        fs::create_dir(root.join("dir"))?;
        fs::write(root.join("dir/selected"), b"baseline selected\n")?;
        fs::write(root.join("dir/neighbor"), b"baseline neighbor\n")?;
        git(root, &["add", "dir"])?;
        git(root, &["commit", "-qm", "baseline"])?;
        git(root, &["branch", "upstream"])?;
        let first = commit(root, "dir/selected", b"first selected\n", "first")?;
        let second = commit(root, "dir/selected", b"second selected\n", "second")?;
        fs::write(root.join("dir/neighbor"), b"preserved staged\n")?;
        git(root, &["add", "dir/neighbor"])?;
        fs::write(root.join("dir/neighbor"), b"preserved working\n")?;
        let index_before = fs::read(root.join(".git/index"))?;
        let directory = fs::metadata(root.join("dir"))?;
        let identity = (directory.dev(), directory.ino());
        let repo = gix::open(root)?;
        let mut session = Session::prepare_interactive(&repo, b"upstream".as_bstr())?.expect("interactive selection");
        let todo = if edit_stop {
            format!("edit {first}\npick {second}\n")
        } else {
            format!("pick {first}\nbreak\npick {second}\n")
        };
        session.accept_todo(&repo, todo.as_bytes())?;
        assert_eq!(session.advance(&repo)?, Advance::Started);
        if edit_stop {
            assert_eq!(session.advance(&repo)?, Advance::Edit);
        } else {
            assert!(matches!(session.advance(&repo)?, Advance::Step { .. }));
            assert_eq!(session.advance(&repo)?, Advance::Break);
        }
        session.edit_todo(&repo)?;
        session.accept_todo(&repo, format!("drop {second}\n").as_bytes())?;
        if edit_stop {
            assert!(matches!(session.continue_edit(&repo)?, Advance::Step { .. }));
        }
        assert_eq!(session.advance(&repo)?, Advance::Dropped);
        assert_eq!(session.advance(&repo)?, Advance::ReadyFinal);
        assert_eq!(git(root, &["show", "HEAD:dir/neighbor"])?, b"baseline neighbor\n");
        assert_eq!(git(root, &["show", ":dir/neighbor"])?, b"preserved staged\n");
        assert_eq!(fs::read(root.join("dir/neighbor"))?, b"preserved working\n");
        assert_eq!(oid(root, "main")?, second, "original branch remains untouched");
        session.abort(&repo)?;
        assert_eq!(fs::read(root.join(".git/index"))?, index_before);
        assert_eq!(fs::read(root.join("dir/selected"))?, b"second selected\n");
        assert_eq!(fs::read(root.join("dir/neighbor"))?, b"preserved working\n");
        let directory = fs::metadata(root.join("dir"))?;
        assert_eq!(
            (directory.dev(), directory.ino()),
            identity,
            "unchanged ancestor inode remains owned by the user"
        );
    }
    Ok(())
}
