//! Git bridge authoring primitives; no covenant or participant authority.
use std::{ffi::OsString, path::PathBuf, process::Stdio};

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
    let Some(path) = hook_path(repo, name)? else {
        return Ok(());
    };
    let mut context = repo.command_context()?;
    context.git_dir = Some(absolute(repo, repo.git_dir()));
    context.worktree_dir = repo.workdir().map(|path| absolute(repo, path));
    // Keep the path in argv, never in shell source (including hooks with no
    // arguments). The shell also provides Git's fallback for shebang-less hooks.
    let mut command: std::process::Command = gix::command::prepare("\"$@\"")
        .with_shell()
        .with_context(context)
        .arg(path.into_os_string())
        .args(args.iter().cloned())
        .env("GIT_INDEX_FILE", absolute(repo, &repo.index_path()).into_os_string())
        .env("GIT_EDITOR", ":")
        .stdin(Stdio::null())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .into();
    command.current_dir(absolute(repo, repo.workdir().unwrap_or(repo.git_dir())));
    let status = command
        .status()
        .with_context(|| format!("could not execute {name} hook"))?;
    ensure!(status.success(), "{name} hook rejected the operation ({status})");
    Ok(())
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
