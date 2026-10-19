//! Git bridge authoring primitives; no covenant or participant authority.
use std::{
    ffi::OsString,
    path::{Path, PathBuf},
    process::Stdio,
};

#[cfg(target_os = "linux")]
pub mod conflict;
#[cfg(target_os = "linux")]
pub mod invocation;

use anyhow::{Context, Result, bail, ensure};

fn absolute(repo: &gix::Repository, path: &std::path::Path) -> PathBuf {
    repo.current_dir().join(path)
}

/// Locate executable hooks using the worktree root and shared Git directory.
pub(super) fn hook_path(repo: &gix::Repository, name: &str) -> Result<Option<PathBuf>> {
    let config = repo.config_snapshot();
    let configured = config.trusted_path("core.hooksPath")?;
    ensure!(
        configured.is_some() || config.string("core.hooksPath").is_none(),
        "refusing to ignore an untrusted core.hooksPath"
    );
    let directory = match configured {
        Some(path) => absolute(repo, repo.workdir().unwrap_or(repo.git_dir())).join(path),
        None => absolute(repo, repo.common_dir()).join("hooks"),
    };
    let path = directory.join(name);
    let metadata = match path.metadata() {
        Ok(metadata) => metadata,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound || err.kind() == std::io::ErrorKind::NotADirectory => {
            return Ok(None);
        }
        Err(err) => return Err(err).with_context(|| format!("inspect hook {}", path.display())),
    };
    if !metadata.is_file() {
        return Ok(None);
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if metadata.permissions().mode() & 0o111 == 0 {
            eprintln!("hint: ignoring non-executable hook {}", path.display());
            return Ok(None);
        }
    }
    ensure!(
        repo.git_dir_trust() == gix::sec::Trust::Full,
        "refusing to execute hooks in an untrusted repository"
    );
    Ok(Some(path))
}

pub(super) fn run_hook(repo: &gix::Repository, name: &str, args: &[OsString]) -> Result<()> {
    run_hook_with_input(repo, name, args, None)
}

pub(super) fn run_hook_with_input(
    repo: &gix::Repository,
    name: &str,
    args: &[OsString],
    input: Option<&[u8]>,
) -> Result<()> {
    let Some(path) = hook_path(repo, name)? else {
        return Ok(());
    };
    let mut command = hook_command(repo, &path, args, &repo.index_path(), input.is_some(), false)?;
    let mut child = command
        .spawn()
        .with_context(|| format!("could not execute {name} hook"))?;
    let write_result = if let Some(input) = input {
        use std::io::Write;
        let mut stdin = child.stdin.take().context("hook stdin was not piped")?;
        stdin.write_all(input)
    } else {
        Ok(())
    };
    // Always reap the child, even when it closes stdin before reading every ref.
    let status = child
        .wait()
        .with_context(|| format!("could not wait for {name} hook"))?;
    if let Err(err) = write_result
        && err.kind() != std::io::ErrorKind::BrokenPipe
    {
        return Err(err).with_context(|| format!("could not write {name} hook input"));
    }
    ensure!(status.success(), "{name} hook rejected the operation ({status})");
    Ok(())
}

/// An explicit index context prevents preserved public staging entering hook commits.
#[cfg(target_os = "linux")]
pub fn run_hook_in_context(
    repo: &gix::Repository,
    name: &str,
    args: &[OsString],
    input: Option<&[u8]>,
    index: &Path,
    invocation_dir: &Path,
) -> Result<invocation::InvocationOutcome> {
    let Some(path) = hook_path(repo, name)? else {
        return Ok(invocation::InvocationOutcome::Succeeded);
    };
    let command = hook_command(repo, &path, args, index, true, true)?;
    invocation::run(command, input, invocation::prepare(invocation_dir)?)
}

#[cfg(target_os = "linux")]
pub(super) fn run_prepared_hook(
    repo: &gix::Repository,
    name: &str,
    args: &[OsString],
    input: Option<&[u8]>,
    index: &Path,
    prepared: invocation::PreparedInvocation,
) -> Result<invocation::InvocationOutcome> {
    let path = hook_path(repo, name)?.context("prepared hook disappeared")?;
    invocation::run(hook_command(repo, &path, args, index, true, true)?, input, prepared)
}

fn hook_command(
    repo: &gix::Repository,
    path: &Path,
    args: &[OsString],
    index: &Path,
    piped: bool,
    gated: bool,
) -> Result<std::process::Command> {
    let mut context = repo.command_context()?;
    context.git_dir = Some(absolute(repo, repo.git_dir()));
    context.worktree_dir = repo.workdir().map(|path| absolute(repo, path));
    // Hook path and arguments remain argv, including shebang-less hook fallback.
    // The launch barrier is consumed before the hook can execute.
    let script = if gated {
        "IFS= read -r brit_gate && test \"$brit_gate\" = go && exec \"$@\""
    } else {
        "exec \"$@\""
    };
    let mut command: std::process::Command = gix::command::prepare(script)
        .with_shell()
        .with_context(context)
        .arg(path.as_os_str())
        .args(args.iter().cloned())
        .env("GIT_INDEX_FILE", absolute(repo, index).into_os_string())
        .env("GIT_EDITOR", ":")
        .stdin(if piped { Stdio::piped() } else { Stdio::null() })
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .try_into()?;
    command.current_dir(absolute(repo, repo.workdir().unwrap_or(repo.git_dir())));
    Ok(command)
}

/// Materialize only persisted index entries, preserving byte paths and modes.
pub(super) fn index_tree(repo: &gix::Repository) -> Result<gix::ObjectId> {
    let index = repo.index_or_empty()?;
    let mut tree = repo.empty_tree().edit()?;
    for entry in index.entries() {
        ensure!(
            entry.stage() == gix::index::entry::Stage::Unconflicted,
            "cannot commit an unmerged index"
        );
        if entry.flags.contains(gix::index::entry::Flags::INTENT_TO_ADD) {
            continue;
        }
        if entry.mode.is_sparse() {
            bail!("sparse index authoring is not supported; use git commit");
        }
        let mode = entry.mode.to_tree_entry_mode().context("invalid index entry mode")?;
        tree.upsert(entry.path(&index), mode.kind(), entry.id)?;
    }
    Ok(tree.write()?.detach())
}
