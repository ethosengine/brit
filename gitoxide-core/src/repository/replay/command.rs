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
    Start {
        upstream: BString,
        interactive: bool,
    },
    Status,
    Continue,
    Skip,
    Abort,
    /// Set aside a finished or undone rebase whose retained copy is all that remains.
    Retire,
    EditTodo,
    ShowCurrentPatch,
    Stage {
        paths: Vec<BString>,
        expand: bool,
    },
    Amend(Message),
    BeginSplit,
    Commit(Message),
}

#[cfg(not(target_os = "linux"))]
pub fn run(_repo: gix::Repository, _action: Action, _out: &mut dyn Write, _err: &mut dyn Write) -> Result<()> {
    anyhow::bail!("native rebase execution is qualified only on Linux")
}

#[cfg(target_os = "linux")]
pub fn run(repo: gix::Repository, action: Action, out: &mut dyn Write, err: &mut dyn Write) -> Result<()> {
    run_action(repo, action, out, err).map_err(|error| {
        let full = error
            .chain()
            .filter_map(|cause| cause.downcast_ref::<std::io::Error>())
            .any(|cause| cause.kind() == std::io::ErrorKind::StorageFull);
        if full {
            error.context(
                "the disk is full: nothing was lost, and a write that failed removed its own partial file; free \
                 space, then run the same brit rebase command again, which resumes where it stopped",
            )
        } else {
            error
        }
    })
}

#[cfg(target_os = "linux")]
fn run_action(mut repo: gix::Repository, action: Action, out: &mut dyn Write, err: &mut dyn Write) -> Result<()> {
    use super::Session;
    use anyhow::{Context, ensure};
    use gix::bstr::ByteSlice;
    repo.make_paths_absolute()?;
    let active = repo.git_dir().join("brit-rebase").try_exists()?;
    if let Action::Retire = action {
        match Session::retire(&repo)? {
            Some(retired) => report_retired(&retired, out)?,
            None => writeln!(out, "No rebase to retire.")?,
        }
        return Ok(());
    }
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
        let cancelled = cancelled_preparations(&repo)?;
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
                for path in cancelled {
                    writeln!(
                        out,
                        "Cancelled preparation not yet removed: {path:?}; brit rebase --abort finishes it"
                    )?;
                }
                for path in crate::repository::switch::scratch_leftovers(repo.git_dir())? {
                    writeln!(
                        out,
                        "Scratch left by a killed Brit process (blocks nothing): {path:?}; once no Brit command is running, remove it with rm -rf {}",
                        shell_quote(&path)
                    )?;
                }
                for path in kept_journals(&repo)? {
                    writeln!(
                        out,
                        "Kept by --retire (blocks nothing): {path:?}; remove it with rm -rf {}",
                        shell_quote(&path)
                    )?;
                }
                return Ok(());
            }
            Action::Abort if !cancelled.is_empty() => {
                for path in cancelled {
                    Session::cleanup_prepared(&repo, &path)?;
                    writeln!(out, "Finished removing cancelled preparation {path:?}")?;
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
            _ if !receipts.is_empty() => anyhow::bail!(
                "no rebase in progress, but a finished rebase's cleanup is pending; brit rebase --continue finishes it"
            ),
            _ => anyhow::bail!("no rebase in progress"),
        }
    }
    if let Err(error) = Session::check_own(&repo) {
        if matches!(action, Action::Status) {
            return report_unopenable(&repo, &error, out);
        }
        return Err(error.context(
            "the rebase journal does not open as this repository's own, so no brit command will change, rename or \
             remove it; nothing was changed; brit rebase --status says what failed and what to do by hand",
        ));
    }
    if !matches!(action, Action::Status) {
        Session::settle_pending(&repo)?;
    }
    if matches!(action, Action::Abort)
        && let Some(differences) = Session::cancel_preparation(&repo)?
    {
        writeln!(
            out,
            "Rebase preparation cancelled; it had changed nothing live, so nothing was restored."
        )?;
        for difference in differences {
            writeln!(err, "warning: {difference}; left as found")?;
        }
        return Ok(());
    }
    let journal = repo.git_dir().join("brit-rebase");
    if !matches!(action, Action::Status)
        && !["state.json", "state.pending"]
            .iter()
            .any(|name| journal.join(name).symlink_metadata().is_ok())
    {
        anyhow::bail!(
            "a rebase preparation stopped before it recorded anything and changed nothing live, so there is \
             nothing to continue; brit rebase --abort removes its directory {journal:?}, then the rebase can be \
             started again"
        );
    }
    let mut session = match Session::open(&repo) {
        Ok(session) => session,
        Err(error) if matches!(action, Action::Status) => {
            let journal = repo.git_dir().join("brit-rebase");
            if ["state.json", "state.pending"]
                .iter()
                .any(|name| journal.join(name).symlink_metadata().is_ok())
            {
                writeln!(
                    out,
                    "A rebase journal of this repository exists but could not be read in full: {error:#}. Nothing \
                     was changed; brit rebase --abort tries to restore the original state."
                )?;
            } else {
                writeln!(
                    out,
                    "A rebase preparation stopped before it recorded anything and changed nothing live; brit rebase \
                     --abort removes its directory {journal:?}."
                )?;
            }
            return Ok(());
        }
        Err(error) => return Err(error),
    };
    // A stop recorded before its authoring session was created (a kill between
    // the two): each authoring command first creates it, as --continue does.
    if matches!(
        action,
        Action::Stage { .. } | Action::Amend(_) | Action::Commit(_) | Action::BeginSplit
    ) && session.phase() == "authoring"
        && !session.has_pending_decision()
        && !repo.git_dir().join("brit-authoring").try_exists()?
    {
        session.recover_stop(&repo)?;
    }
    let result = match action {
        Action::Status => {
            session.status(out)?;
            session.child_status(&repo, out)
        }
        Action::ShowCurrentPatch => session.show_current_patch(&repo, out),
        Action::Continue => continue_operation(&repo, &mut session, out, err),
        Action::Abort => {
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
        Action::Start { .. } | Action::Retire => unreachable!("handled above"),
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
fn report_retired(retired: &super::Retired, out: &mut dyn Write) -> Result<()> {
    writeln!(
        out,
        "Retired the rebase; nothing was deleted and nothing live was changed."
    )?;
    if retired.published {
        writeln!(
            out,
            "The rebase is published at {} (it was {} before); only its cleanup copy was left to build.",
            retired.tip, retired.original
        )?;
    } else {
        writeln!(
            out,
            "The rebase did not publish: its branch names {}, and HEAD, the index and the worktree are as they were \
             before it; only its retained copy was left to build.",
            retired.original
        )?;
    }
    writeln!(
        out,
        "Kept journal: {:?} ({} bytes); remove it with: rm -rf {}",
        retired.journal,
        retired.journal_bytes,
        shell_quote(&retired.journal)
    )?;
    if let Some((path, bytes)) = &retired.copies {
        writeln!(
            out,
            "Kept retained copies: {path:?} ({bytes} bytes); remove them with: rm -rf {}",
            shell_quote(path)
        )?;
    }
    for (name, id) in &retired.refs {
        writeln!(
            out,
            "Kept retention ref {name} -> {id} (keeps those commits from garbage collection); remove it with: git update-ref -d {name}"
        )?;
    }
    Ok(())
}

/// `--status` for a journal that does not open as this repository's own: say what
/// failed, that no Brit command will touch it, and what a person can do by hand.
#[cfg(target_os = "linux")]
fn report_unopenable(repo: &gix::Repository, error: &anyhow::Error, out: &mut dyn Write) -> Result<()> {
    let directory = repo.git_dir().join("brit-rebase");
    writeln!(
        out,
        "A rebase journal exists at {directory:?}, but it does not open as this repository's own: {error:#}."
    )?;
    writeln!(
        out,
        "No brit command will change, rename or remove it, or anything it names, while that is so; every Brit \
         mutation refuses until it is resolved by hand."
    )?;
    writeln!(
        out,
        "If this repository was copied or moved while a rebase was in progress, the journal may describe ANOTHER \
         repository's live rebase, at the location it records: finish or abort that rebase there, not here. HEAD, the \
         index and the worktree of this copy may be part-way through it, and the journal's payload-* files hold the \
         uncommitted work it preserved."
    )?;
    writeln!(
        out,
        "If it is this repository's own journal and only its permissions changed (Brit writes the directory mode \
         0700 and its files mode 0600, owned by the repository's owner), restoring them lets --status, --abort and \
         --continue use it again."
    )?;
    if let Some(recorded) = super::Session::recorded_location(repo) {
        writeln!(
            out,
            "The journal records that its rebase ran in {} (no longer present), so this repository was most likely \
             moved from there part-way through it. Moving it back to {} and running brit rebase --abort there \
             restores HEAD, the index, the worktree and the uncommitted work exactly; brit rebase --continue there \
             carries the rebase on.",
            shell_quote(&recorded),
            shell_quote(&recorded)
        )?;
    }
    writeln!(
        out,
        "To set it aside by hand instead, once no Brit or Git command runs in this repository: move {} out of the \
         way, for example to {}. This restores nothing: if the rebase had begun changing the checkout, HEAD, the \
         index and the worktree stay part-way through it, and only the journal's payload-* files and the retained \
         copies hold what it changed.",
        shell_quote(&directory),
        shell_quote(&repo.git_dir().join("brit-rebase-kept-manual"))
    )?;
    let mut references = Vec::new();
    for reference in repo.references()?.prefixed("refs/brit-rebase-retain/")? {
        let reference = reference.map_err(|error| anyhow::anyhow!("{error}"))?;
        references.push(reference.name().as_bstr().to_string());
    }
    for name in references {
        writeln!(
            out,
            "Retention ref {name} keeps that rebase's commits from garbage collection; remove it, once the journal \
             is set aside, with: git update-ref -d {name}"
        )?;
    }
    for lock in git_locks(repo)? {
        writeln!(
            out,
            "Git lock {lock:?} exists; a rebase stopped part-way can leave it. Remove it by hand only once you are \
             sure no process in this repository holds it."
        )?;
    }
    let copies = repo.git_dir().join("brit-rebase-retained");
    if copies.try_exists()? {
        writeln!(
            out,
            "Retained copies under {copies:?} are independent of the journal; brit-rebase-retained inspects them."
        )?;
    }
    Ok(())
}

/// The ordinary Git lock files a stopped rebase transition can leave: `*.lock`
/// directly in the Git directory, and under `logs/`, `refs/` and beside
/// `packed-refs` in the common directory. Symbolic links are not followed.
#[cfg(target_os = "linux")]
fn git_locks(repo: &gix::Repository) -> Result<Vec<PathBuf>> {
    let mut locks = Vec::new();
    let mut pending = vec![
        (repo.git_dir().to_owned(), false),
        (repo.git_dir().join("logs"), true),
        (repo.common_dir().join("logs"), true),
        (repo.common_dir().join("refs"), true),
        (repo.common_dir().to_owned(), false),
    ];
    while let Some((directory, recurse)) = pending.pop() {
        let entries = match std::fs::read_dir(&directory) {
            Ok(entries) => entries,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => return Err(error.into()),
        };
        for entry in entries {
            let entry = entry?;
            let kind = entry.file_type()?;
            if kind.is_dir() && recurse {
                pending.push((entry.path(), true));
            } else if kind.is_file() && entry.file_name().as_encoded_bytes().ends_with(b".lock") {
                locks.push(entry.path());
            }
        }
    }
    locks.sort();
    locks.dedup();
    Ok(locks)
}

#[cfg(target_os = "linux")]
fn shell_quote(path: &std::path::Path) -> String {
    format!("'{}'", path.display().to_string().replace('\'', r"'\''"))
}

/// Journals set aside by `--retire`.
#[cfg(target_os = "linux")]
fn kept_journals(repo: &gix::Repository) -> Result<Vec<PathBuf>> {
    let mut paths = Vec::new();
    for entry in std::fs::read_dir(repo.git_dir())? {
        let entry = entry?;
        let name = entry.file_name();
        if let Some(nonce) = name.as_encoded_bytes().strip_prefix(b"brit-rebase-kept-")
            && nonce.len() == 40
            && nonce.iter().all(u8::is_ascii_hexdigit)
        {
            paths.push(entry.path());
        }
    }
    paths.sort();
    Ok(paths)
}

/// Tombstones of cancelled preparations whose removal was interrupted, named by
/// their directory, or by their receipt once the directory itself is gone.
#[cfg(target_os = "linux")]
fn cancelled_preparations(repo: &gix::Repository) -> Result<Vec<PathBuf>> {
    use gix::bstr::ByteSlice;
    let mut paths = std::collections::BTreeSet::new();
    for entry in std::fs::read_dir(repo.git_dir())? {
        let name = entry?.file_name();
        let Some(rest) = name.as_encoded_bytes().strip_prefix(b"brit-rebase-done-") else {
            continue;
        };
        let nonce = rest.strip_suffix(b".receipt").unwrap_or(rest);
        if nonce.len() == 40 && nonce.iter().all(u8::is_ascii_hexdigit) {
            paths.insert(repo.git_dir().join(format!("brit-rebase-done-{}", nonce.as_bstr())));
        }
    }
    Ok(paths.into_iter().collect())
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
