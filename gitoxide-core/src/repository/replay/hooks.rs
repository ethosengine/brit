//! Operation hooks share the authoring supervisor and its durable launch barrier.
use std::{ffi::OsString, path::Path};

use anyhow::{Result, ensure};

use crate::repository::authoring::{self, invocation::InvocationOutcome};

/// Pin effective configuration and executable operation hooks. Include source
/// files as well as the snapshot so edits made by a hook are visible immediately.
pub(super) fn policy(repo: &gix::Repository) -> Result<String> {
    use std::{collections::BTreeSet, fs, os::unix::fs::MetadataExt};
    let config = repo.config_snapshot();
    let mut paths: BTreeSet<_> = config
        .sections()
        .filter_map(|section| section.meta().path.clone())
        .collect();
    paths.extend([repo.common_dir().join("config"), repo.git_dir().join("config.worktree")]);
    let mut sources = Vec::new();
    for path in paths {
        let value = match fs::read(&path) {
            Ok(bytes) => Some(bytes),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
            Err(e) => return Err(e.into()),
        };
        sources.push((gix::path::into_bstr(path)?.to_vec(), value));
    }
    let mut hooks = Vec::new();
    for name in ["pre-rebase", "post-rewrite"] {
        let image = match authoring::hook_path(repo, name)? {
            Some(path) => {
                let canonical = fs::canonicalize(&path)?;
                let metadata = fs::metadata(&canonical)?;
                Some((
                    gix::path::into_bstr(path)?.to_vec(),
                    gix::path::into_bstr(canonical.clone())?.to_vec(),
                    metadata.dev(),
                    metadata.ino(),
                    metadata.mode(),
                    fs::read(canonical)?,
                ))
            }
            None => None,
        };
        hooks.push((name, image));
    }
    crate::repository::switch::digest(&serde_json::to_vec(&(config.to_bstring().to_vec(), sources, hooks))?)
}

/// `directory` names one invocation, never a reusable operation-wide directory.
/// Persist the parent's pending invocation in `durable_start`; failure there
/// leaves a prepared receipt but cannot execute the hook. Unknown outcomes are
/// inspected by the caller, never passed back here for an automatic retry.
pub(super) fn run(
    repo: &gix::Repository,
    directory: &Path,
    name: &str,
    args: &[OsString],
    input: Option<&[u8]>,
    private_index: &Path,
    durable_start: impl FnOnce() -> Result<()>,
) -> Result<InvocationOutcome> {
    ensure!(
        matches!(name, "pre-rebase" | "post-rewrite"),
        "automatic replay only runs operation hooks"
    );
    if authoring::hook_path(repo, name)?.is_none() {
        return Ok(InvocationOutcome::Succeeded);
    }
    let prepared = authoring::invocation::prepare(directory)?;
    crate::repository::switch::checkpoint("replay-operation-invocation-prepared")?;
    durable_start()?;
    authoring::run_prepared_hook(repo, name, args, input, private_index, prepared)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        cell::Cell,
        fs,
        os::unix::{ffi::OsStringExt, fs::PermissionsExt},
    };

    fn fixture() -> Result<tempfile::TempDir> {
        let temp = tempfile::tempdir()?;
        let output = gix_testtools::git_command(temp.path())
            .args(["init", "-q", "-b", "main"])
            .output()?;
        ensure!(output.status.success(), "fixture initialization failed");
        Ok(temp)
    }

    fn hook(root: &Path, name: &str, body: &str) -> Result<()> {
        let path = root.join(".git/hooks").join(name);
        fs::write(&path, format!("#!/bin/sh\nset -eu\n{body}\n"))?;
        fs::set_permissions(path, fs::Permissions::from_mode(0o700))?;
        Ok(())
    }

    #[test]
    fn absent_operation_hook_never_records_an_invocation_start() -> Result<()> {
        if gix_testtools::run_in_isolated_process().map_err(anyhow::Error::from_boxed)? {
            return Ok(());
        }
        let temp = fixture()?;
        let repo = gix::open(temp.path())?;
        let directory = repo.git_dir().join("invocation");
        let called = Cell::new(false);
        assert_eq!(
            run(&repo, &directory, "pre-rebase", &[], None, &repo.index_path(), || {
                called.set(true);
                Ok(())
            })?,
            InvocationOutcome::Succeeded
        );
        assert!(!called.get(), "missing hooks do not acquire a pending invocation");
        assert!(!directory.exists());
        Ok(())
    }

    #[test]
    fn failed_durable_start_cannot_release_the_hook_sentinel() -> Result<()> {
        if gix_testtools::run_in_isolated_process().map_err(anyhow::Error::from_boxed)? {
            return Ok(());
        }
        let temp = fixture()?;
        hook(temp.path(), "pre-rebase", "printf executed > hook-sentinel")?;
        let repo = gix::open(temp.path())?;
        let directory = repo.git_dir().join("invocation");
        let result = run(&repo, &directory, "pre-rebase", &[], None, &repo.index_path(), || {
            anyhow::bail!("injected parent persistence failure")
        });
        assert!(result.is_err());
        assert!(
            !temp.path().join("hook-sentinel").exists(),
            "no child side effect may precede durable parent invocation state"
        );
        assert!(directory.exists(), "prepared invocation evidence remains available");
        Ok(())
    }

    #[test]
    fn automatic_replay_refuses_commit_and_checkout_hooks() -> Result<()> {
        if gix_testtools::run_in_isolated_process().map_err(anyhow::Error::from_boxed)? {
            return Ok(());
        }
        let temp = fixture()?;
        let repo = gix::open(temp.path())?;
        for name in ["pre-commit", "commit-msg", "post-commit", "post-checkout", "post-merge"] {
            hook(temp.path(), name, "printf executed > forbidden-hook")?;
            let directory = repo.git_dir().join(format!("invocation-{name}"));
            let called = Cell::new(false);
            assert!(
                run(&repo, &directory, name, &[], None, &repo.index_path(), || {
                    called.set(true);
                    Ok(())
                })
                .is_err()
            );
            assert!(!called.get());
            assert!(!directory.exists());
            assert!(!temp.path().join("forbidden-hook").exists());
        }
        Ok(())
    }

    #[test]
    fn supervised_operation_hook_receives_private_index_raw_argv_and_worktree_cwd() -> Result<()> {
        if gix_testtools::run_in_isolated_process().map_err(anyhow::Error::from_boxed)? {
            return Ok(());
        }
        let temp = fixture()?;
        let root = temp.path();
        hook(
            root,
            "pre-rebase",
            "test -f parent-started\nprintf '%s\\0' \"$@\" > hook-args\nprintf '%s' \"$GIT_INDEX_FILE\" > hook-index-path\ncat \"$GIT_INDEX_FILE\" > hook-index-bytes\npwd > hook-cwd",
        )?;
        let repo = gix::open(root)?;
        let private_index = repo.git_dir().join("private index");
        fs::write(&private_index, b"private replay index bytes")?;
        fs::write(repo.index_path(), b"public preserved staging bytes")?;
        let arguments = [
            OsString::from("upstream ' ; $(touch injected)"),
            OsString::from_vec(b"non-utf8-\xff".to_vec()),
        ];
        assert_eq!(
            run(
                &repo,
                &repo.git_dir().join("invocation"),
                "pre-rebase",
                &arguments,
                None,
                &private_index,
                || {
                    fs::write(root.join("parent-started"), b"durable test marker")?;
                    Ok(())
                }
            )?,
            InvocationOutcome::Succeeded
        );
        assert_eq!(
            fs::read(root.join("hook-args"))?,
            b"upstream ' ; $(touch injected)\0non-utf8-\xff\0"
        );
        assert_eq!(fs::read(root.join("hook-index-bytes"))?, b"private replay index bytes");
        assert_eq!(fs::read(repo.index_path())?, b"public preserved staging bytes");
        assert_eq!(
            fs::read(root.join("hook-index-path"))?,
            gix::path::into_bstr(&private_index)?.to_vec()
        );
        let mut expected_cwd = gix::path::into_bstr(fs::canonicalize(root)?)?.to_vec();
        expected_cwd.push(b'\n');
        assert_eq!(fs::read(root.join("hook-cwd"))?, expected_cwd);
        assert!(
            !root.join("injected").exists(),
            "repository arguments never become shell source"
        );
        Ok(())
    }

    #[test]
    fn policy_detects_hook_content_and_inode_replacement_on_same_repository_handle() -> Result<()> {
        if gix_testtools::run_in_isolated_process().map_err(anyhow::Error::from_boxed)? {
            return Ok(());
        }
        let temp = fixture()?;
        hook(temp.path(), "pre-rebase", "exit 0")?;
        let repo = gix::open(temp.path())?;
        let original = policy(&repo)?;
        hook(temp.path(), "pre-rebase", "exit 1")?;
        let changed = policy(&repo)?;
        assert_ne!(
            changed, original,
            "hook bytes must be pinned independently of cached Git configuration"
        );
        let path = repo.git_dir().join("hooks/pre-rebase");
        let replacement = repo.git_dir().join("hooks/replacement");
        fs::write(&replacement, fs::read(&path)?)?;
        fs::set_permissions(&replacement, fs::Permissions::from_mode(0o700))?;
        fs::rename(&replacement, &path)?;
        let replaced = policy(&repo)?;
        assert_ne!(
            replaced, changed,
            "byte-identical executable replacement is still a hook identity change"
        );
        fs::set_permissions(path, fs::Permissions::from_mode(0o600))?;
        assert_ne!(
            policy(&repo)?,
            replaced,
            "removing hook execution permission changes effective policy"
        );
        Ok(())
    }

    #[test]
    fn policy_reads_changed_include_sources_despite_a_stale_configuration_snapshot() -> Result<()> {
        if gix_testtools::run_in_isolated_process().map_err(anyhow::Error::from_boxed)? {
            return Ok(());
        }
        let temp = fixture()?;
        let included = temp.path().join(".git/included-policy");
        fs::write(&included, b"[core]\n\teditor = first-editor\n")?;
        let output = gix_testtools::git_command(temp.path())
            .args(["config", "include.path", "included-policy"])
            .output()?;
        ensure!(output.status.success(), "configure included source");
        let repo = gix::open(temp.path())?;
        let cached = repo.config_snapshot().to_bstring();
        let original = policy(&repo)?;
        fs::write(&included, b"[core]\n\teditor = second-editor\n")?;
        assert_eq!(
            repo.config_snapshot().to_bstring(),
            cached,
            "fixture deliberately keeps the opened repository snapshot stale"
        );
        assert_ne!(
            policy(&repo)?,
            original,
            "source-file edits must invalidate the operation policy without reopening Git"
        );
        Ok(())
    }
}
