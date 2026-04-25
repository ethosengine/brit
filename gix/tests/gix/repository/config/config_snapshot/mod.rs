use gix::config::tree::{gitoxide, Branch, Core, Key};

use crate::{named_repo, repo_rw};

#[cfg(feature = "credentials")]
mod credential_helpers;

#[test]
fn commit_auto_rollback() -> crate::Result {
    let mut repo: gix::Repository = named_repo("make_basic_repo.sh")?;
    assert_eq!(repo.head_id()?.shorten()?.to_string(), "3189cd3");

    {
        let mut config = repo.config_snapshot_mut();
        config.set_raw_value(Core::ABBREV, "4")?;
        let repo = config.commit_auto_rollback()?;
        assert_eq!(repo.head_id()?.shorten()?.to_string(), "3189");
    }

    assert_eq!(repo.head_id()?.shorten()?.to_string(), "3189cd3");

    let repo = {
        let mut config = repo.config_snapshot_mut();
        config.set_raw_value(Core::ABBREV, "4")?;
        let mut repo = config.commit_auto_rollback()?;
        assert_eq!(repo.head_id()?.shorten()?.to_string(), "3189");
        // access to the mutable repo underneath
        repo.object_cache_size_if_unset(16 * 1024);
        repo.rollback()?
    };
    assert_eq!(repo.head_id()?.shorten()?.to_string(), "3189cd3");

    Ok(())
}

mod trusted_path {
    use crate::util::named_repo;

    #[test]
    fn optional_is_respected() -> crate::Result {
        let mut repo: gix::Repository = named_repo("make_basic_repo.sh")?;
        repo.config_snapshot_mut().set_raw_value("my.path", "does-not-exist")?;

        let actual = repo
            .config_snapshot()
            .trusted_path("my.path")
            .transpose()?
            .expect("is set");
        assert_eq!(
            actual.as_ref(),
            "does-not-exist",
            "the path isn't evaluated by default, and may not exist"
        );

        repo.config_snapshot_mut()
            .set_raw_value("my.path", ":(optional)does-not-exist")?;
        let actual = repo.config_snapshot().trusted_path("my.path").transpose()?;
        assert_eq!(actual, None, "non-existing paths aren't returned to the caller");
        Ok(())
    }
}

#[test]
fn snapshot_mut_commit_and_forget() -> crate::Result {
    let mut repo: gix::Repository = named_repo("make_basic_repo.sh")?;
    let repo = {
        let mut repo = repo.config_snapshot_mut();
        repo.set_value(&Core::ABBREV, "4")?;
        repo.commit()?
    };
    assert_eq!(repo.config_snapshot().integer("core.abbrev").expect("set"), 4);
    {
        let mut repo = repo.config_snapshot_mut();
        repo.set_raw_value(Core::ABBREV, "8")?;
        repo.forget();
    }
    assert_eq!(repo.config_snapshot().integer("core.abbrev"), Some(4));
    Ok(())
}

#[test]
fn values_are_set_in_memory_only() {
    let mut repo = named_repo("make_config_repo.sh").unwrap();
    let repo_clone = repo.clone();
    let key = "hallo.welt";
    let key_subsection = "branch.main.merge";
    assert_eq!(repo.config_snapshot().boolean(key), None, "no value there just yet");
    assert_eq!(repo.config_snapshot().string(key_subsection), None);

    {
        let mut config = repo.config_snapshot_mut();
        config.set_raw_value("hallo.welt", "true").unwrap();
        config
            .set_subsection_value(&Branch::MERGE, "main", "refs/heads/foo")
            .unwrap();
    }

    assert_eq!(
        repo.config_snapshot().boolean(key),
        Some(true),
        "value was set and applied"
    );
    assert_eq!(
        repo.config_snapshot().string(key_subsection).as_deref(),
        Some("refs/heads/foo".into())
    );

    assert_eq!(
        repo_clone.config_snapshot().boolean(key),
        None,
        "values are not written back automatically nor are they shared between clones"
    );
    assert_eq!(repo_clone.config_snapshot().string(key_subsection), None);
}

#[test]
fn set_value_in_subsection() {
    let mut repo = named_repo("make_config_repo.sh").unwrap();
    {
        let mut config = repo.config_snapshot_mut();
        config
            .set_value(&gitoxide::Credentials::TERMINAL_PROMPT, "yes")
            .unwrap();
        assert_eq!(
            config
                .string(&*gitoxide::Credentials::TERMINAL_PROMPT.logical_name())
                .expect("just set")
                .as_ref(),
            "yes"
        );
    }
}

#[test]
fn apply_cli_overrides() -> crate::Result {
    let mut repo = named_repo("make_config_repo.sh").unwrap();
    repo.config_snapshot_mut().append_config(
        [
            "a.b=c",
            "remote.origin.url = url",
            "implicit.bool-true",
            "implicit.bool-false = ",
        ],
        gix_config::Source::Cli,
    )?;

    let config = repo.config_snapshot();
    assert_eq!(config.string("a.b").expect("present").as_ref(), "c");
    assert_eq!(config.string("remote.origin.url").expect("present").as_ref(), "url");
    assert_eq!(
        config.string("implicit.bool-true"),
        None,
        "no keysep is interpreted as 'not present' as we don't make up values"
    );
    assert_eq!(
        config.string("implicit.bool-false").expect("present").as_ref(),
        "",
        "empty values are fine"
    );
    assert_eq!(
        config.boolean("implicit.bool-false"),
        Some(false),
        "empty values are boolean true"
    );
    assert_eq!(
        config.boolean("implicit.bool-true"),
        Some(true),
        "values without key-sep are true"
    );

    Ok(())
}

#[test]
fn reload_reloads_on_disk_changes() -> crate::Result {
    use std::io::Write;

    let (mut repo, _tmp) = repo_rw("make_config_repo.sh")?;
    assert_eq!(repo.config_snapshot().integer("core.abbrev"), None);

    let config_path = repo.git_dir().join("config");
    let mut config = std::fs::OpenOptions::new().append(true).open(config_path)?;
    writeln!(config, "\n[core]\n\tabbrev = 4")?;

    assert_eq!(repo.config_snapshot().integer("core.abbrev"), None);
    repo.reload()?;
    assert_eq!(repo.config_snapshot().integer("core.abbrev"), Some(4));
    Ok(())
}

#[test]
fn reload_discards_in_memory_only_changes() -> crate::Result {
    let mut repo = named_repo("make_config_repo.sh")?;

    repo.config_snapshot_mut().set_raw_value(Core::ABBREV, "4")?;
    assert_eq!(repo.config_snapshot().integer("core.abbrev"), Some(4));

    repo.reload()?;
    assert_eq!(repo.config_snapshot().integer("core.abbrev"), None);
    Ok(())
}

mod branch_write {
    use crate::named_repo;

    fn full_ref(name: &str) -> gix_ref::FullName {
        gix_ref::FullName::try_from(name.to_owned()).expect("static test input is a valid full ref name")
    }

    #[test]
    fn set_branch_upstream_remote_tracking_branch() -> crate::Result {
        let mut repo = named_repo("make_config_repo.sh")?;

        // refs/remotes/<remote>/<branch> → remote=<remote>, merge=refs/heads/<branch>
        repo.set_branch_upstream(b"dev".into(), full_ref("refs/remotes/origin/main").as_ref())?;

        let snap = repo.config_snapshot();
        assert_eq!(
            snap.string("branch.dev.remote").as_deref(),
            Some("origin".into()),
            "remote name extracted from refs/remotes/<remote>/..."
        );
        assert_eq!(
            snap.string("branch.dev.merge").as_deref(),
            Some("refs/heads/main".into()),
            "merge ref reconstructed as refs/heads/<branch>"
        );
        Ok(())
    }

    #[test]
    fn set_branch_upstream_local_branch() -> crate::Result {
        let mut repo = named_repo("make_config_repo.sh")?;

        // refs/heads/<branch> → remote=., merge=refs/heads/<branch> (local tracking sentinel)
        repo.set_branch_upstream(b"feature".into(), full_ref("refs/heads/main").as_ref())?;

        let snap = repo.config_snapshot();
        assert_eq!(
            snap.string("branch.feature.remote").as_deref(),
            Some(".".into()),
            "local-tracking branches use '.' as remote sentinel"
        );
        assert_eq!(
            snap.string("branch.feature.merge").as_deref(),
            Some("refs/heads/main".into()),
            "full ref passed through unchanged for local tracking"
        );
        Ok(())
    }

    #[test]
    fn unset_branch_upstream_removes_keys() -> crate::Result {
        let mut repo = named_repo("make_config_repo.sh")?;

        // First, set an upstream so we have something to unset.
        repo.set_branch_upstream(b"dev".into(), full_ref("refs/remotes/origin/main").as_ref())?;
        assert!(
            repo.config_snapshot().string("branch.dev.remote").is_some(),
            "remote must be present before unset"
        );

        repo.unset_branch_upstream(b"dev".into())?;

        let snap = repo.config_snapshot();
        assert!(snap.string("branch.dev.remote").is_none(), "remote cleared after unset");
        assert!(snap.string("branch.dev.merge").is_none(), "merge cleared after unset");
        Ok(())
    }

    #[test]
    fn unset_branch_upstream_errors_when_no_upstream() -> crate::Result {
        let mut repo = named_repo("make_config_repo.sh")?;

        let result = repo.unset_branch_upstream(b"nonexistent".into());
        assert!(
            matches!(result, Err(gix::config::branch_write::UnsetUpstream::NoUpstream(_))),
            "must error with NoUpstream when the branch has no upstream config"
        );
        Ok(())
    }

    #[test]
    fn set_branch_description_sets_and_clears() -> crate::Result {
        let mut repo = named_repo("make_config_repo.sh")?;

        repo.set_branch_description(b"feature".into(), b"my feature branch description".into())?;
        assert_eq!(
            repo.config_snapshot().string("branch.feature.description").as_deref(),
            Some("my feature branch description".into()),
            "description written correctly"
        );

        // Clearing with empty value.
        repo.set_branch_description(b"feature".into(), b"".into())?;
        assert!(
            repo.config_snapshot().string("branch.feature.description").is_none(),
            "empty value clears the description key"
        );
        Ok(())
    }
}
