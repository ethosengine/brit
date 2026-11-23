//! Operation hooks share the authoring supervisor and its durable launch barrier.
use std::{ffi::OsString, path::Path};

use anyhow::{Result, ensure};

use crate::repository::authoring::{self, invocation::InvocationOutcome};

/// Configuration that names who is acting rather than how the operation
/// behaves. Each replayed commit records its own author and committer, so the
/// identity in force when a rebase resumes is evidence about that commit, not
/// a change of operation policy; gix also folds `GIT_AUTHOR_*`,
/// `GIT_COMMITTER_*` and `EMAIL` from the environment into these keys, which
/// made a resume under another shell's identity refuse. `user.name` and
/// `user.email` are left out for the same reason wherever they come from;
/// edits to configuration files are still pinned through their raw bytes
/// below. Every other key stays pinned: `user.signingKey`, and the
/// `GIT_AUTHOR_DATE`/`GIT_COMMITTER_DATE` overrides
/// (`gitoxide.commit.{author,committer}Date`), because an interrupted step
/// resumes the exact proposed commit bytes, timestamp included.
const IDENTITY_KEYS: &[(&str, Option<&str>, &str)] = &[
    ("author", None, "name"),
    ("author", None, "email"),
    ("committer", None, "name"),
    ("committer", None, "email"),
    ("user", None, "name"),
    ("user", None, "email"),
    ("gitoxide", Some("user"), "emailFallback"),
];

/// Names the digest form: a journal written before identity was left out
/// (Brit 0.3.2 and earlier) holds the untagged form, which still matches.
const POLICY_FORM: &str = "replay-policy-v2: identity excluded";

/// The effective configuration without [`IDENTITY_KEYS`]. A section left
/// empty by the removal is dropped, so whether an identity variable is set
/// at all does not change the digest either.
fn configuration_without_identity(config: &gix::config::File) -> Vec<u8> {
    let mut config = config.clone();
    let ids: Vec<_> = config.sections_and_ids().map(|(_, id)| id).collect();
    for id in ids {
        let Some(mut section) = config.section_mut_by_id(id) else {
            continue;
        };
        let name = section.header().name().to_string();
        let subsection = section.header().subsection_name().map(ToString::to_string);
        let mut identity = false;
        for (key_section, key_subsection, key) in IDENTITY_KEYS {
            if name.eq_ignore_ascii_case(key_section) && subsection.as_deref() == *key_subsection {
                identity = true;
                while section.remove(key).is_some() {}
            }
        }
        if identity && section.num_values() == 0 {
            config.remove_section_by_id(id);
        }
    }
    config.to_bstring().to_vec()
}

/// Pin effective configuration and executable operation hooks. Include source
/// files as well as the snapshot so edits made by a hook are visible immediately.
pub(super) fn policy(repo: &gix::Repository) -> Result<String> {
    let (config, sources, hooks) = policy_inputs(repo)?;
    crate::repository::switch::digest(&serde_json::to_vec(&(
        POLICY_FORM,
        configuration_without_identity(&config),
        sources,
        hooks,
    ))?)
}

/// Whether `pinned`, a digest a journal recorded, still holds. It accepts the
/// current form and the untagged form Brit 0.3.2 and earlier recorded (which
/// includes identity), so a rebase in progress across an upgrade resumes
/// under the identity it began with instead of refusing.
pub(super) fn policy_matches(repo: &gix::Repository, pinned: &str) -> Result<bool> {
    if policy(repo)? == pinned {
        return Ok(true);
    }
    let (config, sources, hooks) = policy_inputs(repo)?;
    Ok(
        crate::repository::switch::digest(&serde_json::to_vec(&(config.to_bstring().to_vec(), sources, hooks))?)?
            == pinned,
    )
}

type PolicySources = Vec<(Vec<u8>, Option<Vec<u8>>)>;
type PolicyHooks = Vec<(&'static str, Option<(Vec<u8>, Vec<u8>, u64, u64, u32, Vec<u8>)>)>;

fn policy_inputs(repo: &gix::Repository) -> Result<(gix::config::File, PolicySources, PolicyHooks)> {
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
    Ok((config.plumbing().clone(), sources, hooks))
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

    /// Sprint 4 item 5: identity is not policy, every other key is, and a
    /// digest in the untagged form Brit 0.3.2 recorded still matches.
    #[test]
    fn policy_ignores_identity_pins_other_configuration_and_accepts_the_untagged_form() -> Result<()> {
        if gix_testtools::run_in_isolated_process().map_err(anyhow::Error::from_boxed)? {
            return Ok(());
        }
        let temp = fixture()?;
        let mut repo = gix::open(temp.path())?;
        let untagged = {
            let (config, sources, hooks) = policy_inputs(&repo)?;
            crate::repository::switch::digest(&serde_json::to_vec(&(config.to_bstring().to_vec(), sources, hooks))?)?
        };
        let pinned = policy(&repo)?;
        assert_ne!(pinned, untagged, "the two forms never collide");
        assert!(policy_matches(&repo, &untagged)?, "a 0.3.2 journal still resumes");
        assert!(policy_matches(&repo, &pinned)?);
        {
            let mut config = repo.config_snapshot_mut();
            config.set_raw_value_by("committer", None, "name", "Another Committer")?;
            config.set_raw_value_by("author", None, "email", "another@example.com")?;
            config.set_raw_value_by("user", None, "name", "Another User")?;
            config.set_raw_value_by("gitoxide", Some("user".into()), "emailFallback", "fallback@example.com")?;
            config.commit()?;
        }
        assert_eq!(policy(&repo)?, pinned, "identity is not operation policy");
        assert!(
            !policy_matches(&repo, &untagged)?,
            "the untagged form keeps pinning identity"
        );
        {
            let mut config = repo.config_snapshot_mut();
            config.set_raw_value_by("user", None, "signingKey", "another-key")?;
            config.commit()?;
        }
        assert!(!policy_matches(&repo, &pinned)?, "the signing key stays pinned");
        let signed = policy(&repo)?;
        {
            let mut config = repo.config_snapshot_mut();
            config.set_raw_value_by("gitoxide", Some("commit".into()), "committerDate", "2001-01-01")?;
            config.commit()?;
        }
        assert!(
            !policy_matches(&repo, &signed)?,
            "a committer date override stays pinned: resumed proposals keep their timestamp"
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
