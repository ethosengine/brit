//! Bounded CLI orchestration; all mutation remains in the durable replay driver.
use anyhow::Result;
use gix::bstr::BString;
use std::{io::Write, path::PathBuf};

#[derive(Debug)]
pub enum Message {
    Bytes(Vec<u8>),
    File(PathBuf),
    Unchanged,
    Editor,
}
#[derive(Debug)]
pub enum Action {
    Start { upstream: BString, interactive: bool },
    Status,
    Continue,
    Skip,
    Abort,
    EditTodo,
    ShowCurrentPatch,
    Stage { paths: Vec<BString>, expand: bool },
    Amend(Message),
    BeginSplit,
    Commit(Message),
}

#[cfg(not(target_os = "linux"))]
pub fn run(_repo: gix::Repository, _action: Action, _out: &mut dyn Write, _err: &mut dyn Write) -> Result<()> {
    anyhow::bail!("native rebase execution is qualified only on Linux")
}

#[cfg(target_os = "linux")]
pub fn run(mut repo: gix::Repository, action: Action, out: &mut dyn Write, err: &mut dyn Write) -> Result<()> {
    use super::Session;
    use anyhow::{Context, ensure};
    use gix::bstr::ByteSlice;
    repo.make_paths_absolute()?;
    let active = repo.git_dir().join("brit-rebase").try_exists()?;
    if let Action::Start { upstream, interactive } = action {
        let session = if interactive {
            Session::prepare_interactive(&repo, upstream.as_bstr())?
        } else {
            Session::prepare(&repo, upstream.as_bstr())?
        };
        if let Some(mut session) = session {
            return drive(&repo, &mut session, out, err);
        }
        writeln!(out, "Already up to date; no rebase operation created.")?;
        return Ok(());
    }
    if !active {
        let receipts = cleanup_receipts(&repo)?;
        match action {
            Action::Status => {
                writeln!(out, "No active rebase.")?;
                for path in receipts {
                    let (retired, copy) = super::cleanup::inspect(&repo, &path)?;
                    writeln!(
                        out,
                        "Cleanup pending (reservation retired: {retired}): {path:?}; retained copy: {copy:?}"
                    )?;
                }
                return Ok(());
            }
            Action::Continue if !receipts.is_empty() => {
                ensure!(
                    receipts.len() == 1,
                    "multiple cleanup receipts; inspect --status; no implicit sweep"
                );
                let (_, copy) = super::cleanup::inspect(&repo, &receipts[0])?;
                Session::recover_cleanup(&repo, &receipts[0])?;
                writeln!(out, "Cleanup completed; retained copy: {copy:?}")?;
                return Ok(());
            }
            _ => anyhow::bail!("no rebase in progress"),
        }
    }
    let mut session = Session::open(&repo)?;
    let result = match action {
        Action::Status => {
            session.status(out)?;
            session.child_status(&repo, out)
        }
        Action::ShowCurrentPatch => session.show_current_patch(&repo, out),
        Action::Continue => continue_operation(&repo, &mut session, out, err),
        Action::Abort => {
            if session.phase() == "preparing" {
                session = Session::recover_preparation(&repo)?;
            }
            if session.phase() == "aborted" {
                return cleanup(&repo, &mut session, out);
            }
            let copy = session.abort(&repo)?;
            writeln!(out, "Original state restored; retained copy: {:?}", copy.path)?;
            cleanup(&repo, &mut session, out)
        }
        Action::Skip => {
            ensure!(
                !session.has_pending_decision(),
                "recover the durable decision with --continue before --skip"
            );
            let copy = session.skip(&repo)?;
            writeln!(out, "Skipped input; retained copy: {:?}", copy.path)?;
            drive(&repo, &mut session, out, err)
        }
        Action::EditTodo => {
            session.edit_todo(&repo)?;
            session.status(out)
        }
        Action::Stage { paths, expand } => {
            session.stage(&repo, &paths, expand)?;
            session.status(out)
        }
        Action::BeginSplit => {
            session.begin_split(&repo)?;
            session.status(out)
        }
        Action::Amend(message) => author(&repo, &mut session, message, out, err, true),
        Action::Commit(message) => author(&repo, &mut session, message, out, err, false),
        Action::Start { .. } => unreachable!("start handled above"),
    };
    result.with_context(|| format!("rebase phase {}; inspect with --status", session.phase()))
}

#[cfg(target_os = "linux")]
fn author(
    repo: &gix::Repository,
    session: &mut super::Session,
    message: Message,
    out: &mut dyn Write,
    err: &mut dyn Write,
    amend: bool,
) -> Result<()> {
    let message = match message {
        Message::File(path) => Message::Bytes(std::fs::read(path)?),
        other => other,
    };
    let outcome = match (amend, message) {
        (true, Message::Bytes(bytes)) => session.amend(repo, &bytes)?,
        (false, Message::Bytes(bytes)) => session.commit_split(repo, &bytes)?,
        (true, Message::Unchanged) => session.amend_no_edit(repo)?,
        (true, Message::Editor) => session.amend_edit(repo)?,
        (false, Message::Editor) => session.commit_split_edit(repo)?,
        _ => anyhow::bail!("--no-edit is supported only with --amend"),
    };
    writeln!(out, "Authored commit: {}", outcome.commit)?;
    for warning in outcome.warnings {
        writeln!(err, "Committed with warning: {warning}")?;
    }
    session.status(out)
}

#[cfg(target_os = "linux")]
fn continue_operation(
    repo: &gix::Repository,
    session: &mut super::Session,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> Result<()> {
    use super::{Advance, Session};
    if session.has_pending_decision() {
        *session = Session::recover_decision(repo)?;
        if matches!(session.phase(), "authoring" | "empty" | "break") {
            if session.phase() == "authoring" && !repo.git_dir().join("brit-authoring").try_exists()? {
                session.recover_stop(repo)?;
            }
            return stopped(session, out);
        }
    }
    if session.phase() == "prepared" && session.directory().join("pre-rebase").try_exists()? {
        session.recover_pre_hook(repo)?;
    }
    if session.phase() == "final-published" && session.directory().join("post-rewrite").try_exists()? {
        session.acknowledge_post_hook(repo)?;
        return cleanup(repo, session, out);
    }
    match session.phase() {
        "preparing" => {
            *session = Session::recover_preparation(repo)?;
        }
        "invoking-pre" => {
            session.recover_pre_hook(repo)?;
        }
        "retaining-abort" | "aborted" => {
            if session.phase() != "aborted" {
                let copy = session.abort(repo)?;
                writeln!(out, "Original state restored; retained copy: {:?}", copy.path)?;
            }
            return cleanup(repo, session, out);
        }
        "retaining-skip" => {
            let copy = session.skip(repo)?;
            writeln!(out, "Skipped input; retained copy: {:?}", copy.path)?;
        }
        "invoking-post" | "post-unknown" => {
            session.acknowledge_post_hook(repo)?;
            return cleanup(repo, session, out);
        }
        "completed" => return cleanup(repo, session, out),
        "authoring" => {
            if session.has_pending_editor() {
                session.accept_editor(repo)?;
            }
            if !repo.git_dir().join("brit-authoring").try_exists()? {
                session.recover_stop(repo)?;
            }
            if session.has_pending_child_decision(repo)? {
                session.recover_child(repo)?;
            }
            let progress = if session.is_edit_stop() {
                session.continue_edit(repo)?
            } else {
                session.continue_conflict(repo)?
            };
            if matches!(
                progress,
                Advance::Conflict | Advance::Empty | Advance::Edit | Advance::Break | Advance::Message
            ) {
                return stopped(session, out);
            }
        }
        _ => {}
    }
    drive(repo, session, out, err)
}

#[cfg(target_os = "linux")]
fn drive(
    repo: &gix::Repository,
    session: &mut super::Session,
    out: &mut dyn Write,
    _err: &mut dyn Write,
) -> Result<()> {
    use super::Advance;
    loop {
        if matches!(session.phase(), "ready-final" | "final-published") {
            session.finish(repo)?;
            writeln!(out, "Rebase published at {}.", session.tip()?)?;
            return cleanup(repo, session, out);
        }
        match session.advance(repo)? {
            Advance::Started | Advance::Step { .. } | Advance::Dropped | Advance::ReadyFinal => {}
            Advance::Conflict | Advance::Empty | Advance::Edit | Advance::Break | Advance::Message => {
                return stopped(session, out);
            }
        }
    }
}

#[cfg(target_os = "linux")]
fn stopped(session: &super::Session, out: &mut dyn Write) -> Result<()> {
    session.status(out)?;
    if session.phase() == "empty" {
        anyhow::bail!("rebase stopped on an empty input; --continue keeps it, --skip drops it")
    }
    if session.phase() == "authoring" && !session.is_edit_stop() {
        anyhow::bail!("rebase stopped on conflicts; resolve and --stage before --continue, or --skip/--abort")
    }
    Ok(())
}

#[cfg(target_os = "linux")]
fn cleanup(repo: &gix::Repository, session: &mut super::Session, out: &mut dyn Write) -> Result<()> {
    let terminal = session.phase().to_owned();
    session.status(out)?;
    match session.cleanup(repo) {
        Ok(copy) => {
            writeln!(out, "Rebase {terminal}; retained copy: {:?}", copy.path)?;
            Ok(())
        }
        Err(error) => anyhow::bail!("rebase {terminal}; cleanup pending, retained data remains: {error:#}"),
    }
}

#[cfg(target_os = "linux")]
fn cleanup_receipts(repo: &gix::Repository) -> Result<Vec<PathBuf>> {
    let mut paths = Vec::new();
    for entry in std::fs::read_dir(repo.git_dir())? {
        let entry = entry?;
        let name = entry.file_name();
        let bytes = name.as_encoded_bytes();
        if bytes.starts_with(b"brit-rebase-cleanup-") && bytes.ends_with(b".json") {
            super::cleanup::inspect(repo, &entry.path())?;
            paths.push(entry.path());
        }
    }
    paths.sort();
    Ok(paths)
}
