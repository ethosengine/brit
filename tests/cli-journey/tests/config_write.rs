//! `brit config set` and `brit config unset` write the repository-local configuration as
//! `git config <key> <value>` and `git config --unset <key>` do, byte for byte, and refuse
//! without touching it when Git would, or when another writer holds `config.lock`.
use anyhow::Result;
use cli_journey::support::{
    parity::{brit, git, git_ok},
    test_repo::TestRepo,
};

fn local_config(repo: &TestRepo) -> Result<Vec<u8>> {
    Ok(std::fs::read(repo.path().join(".git/config"))?)
}

#[test]
fn set_then_unset_matches_git_and_shows_in_brit_config() -> Result<()> {
    let ours = TestRepo::new("config-set-brit")?;
    let theirs = TestRepo::new("config-set-git")?;
    assert_eq!(local_config(&ours)?, local_config(&theirs)?, "same starting point");

    // The section is before the first dot and the name after the last one: a subsection keeps
    // its slashes and dots, as a branch name does.
    let entries = [
        ("branch.a/b.britReachBase", "origin/main"),
        ("branch.sprint4/x.y.britReachBase", "origin/dev"),
        ("core.britProbe", "yes"),
    ];
    for (key, value) in entries {
        let out = brit(ours.path(), &["config", "set", key, value])?;
        assert_eq!(out.code, Some(0), "brit config set {key}: {out:?}");
        git_ok(theirs.path(), &["config", key, value])?;
        assert_eq!(
            String::from_utf8(local_config(&ours)?)?,
            String::from_utf8(local_config(&theirs)?)?,
            "after setting {key}"
        );
        // Git reads what Brit wrote.
        assert_eq!(git_ok(ours.path(), &["config", "--get", key])?, value);
    }
    // Setting again replaces the value in place.
    let out = brit(
        ours.path(),
        &["config", "set", "branch.a/b.britReachBase", "origin/next"],
    )?;
    assert_eq!(out.code, Some(0), "{out:?}");
    git_ok(theirs.path(), &["config", "branch.a/b.britReachBase", "origin/next"])?;
    assert_eq!(local_config(&ours)?, local_config(&theirs)?, "after replacing");

    let shown = brit(ours.path(), &["config", "branch"])?;
    assert_eq!(shown.code, Some(0), "{shown:?}");
    assert!(shown.stdout.contains("[branch \"a/b\"]"), "{}", shown.stdout);
    assert!(shown.stdout.contains("britReachBase = origin/next"), "{}", shown.stdout);
    assert!(shown.stdout.contains("[branch \"sprint4/x.y\"]"), "{}", shown.stdout);

    for (key, _) in entries {
        let out = brit(ours.path(), &["config", "unset", key])?;
        assert_eq!(out.code, Some(0), "brit config unset {key}: {out:?}");
        git_ok(theirs.path(), &["config", "--unset", key])?;
        assert_eq!(
            String::from_utf8(local_config(&ours)?)?,
            String::from_utf8(local_config(&theirs)?)?,
            "after unsetting {key}"
        );
    }
    let shown = brit(ours.path(), &["config", "show"])?;
    assert_eq!(shown.code, Some(0), "{shown:?}");
    assert!(!shown.stdout.contains("britReachBase"), "{}", shown.stdout);
    assert!(!shown.stdout.contains("britProbe"), "{}", shown.stdout);
    Ok(())
}

#[test]
fn refusals_exit_like_git_and_leave_the_config_unchanged() -> Result<()> {
    let ours = TestRepo::new("config-refuse-brit")?;
    let theirs = TestRepo::new("config-refuse-git")?;
    for repo in [&ours, &theirs] {
        git_ok(repo.path(), &["config", "--add", "multi.value", "1"])?;
        git_ok(repo.path(), &["config", "--add", "multi.value", "2"])?;
    }
    let cases: [(&[&str], &[&str]); 5] = [
        (
            &["config", "unset", "branch.a/b.missing"],
            &["config", "--unset", "branch.a/b.missing"],
        ),
        (&["config", "unset", "nosection"], &["config", "--unset", "nosection"]),
        (&["config", "set", "nosection", "x"], &["config", "nosection", "x"]),
        (&["config", "set", "multi.value", "3"], &["config", "multi.value", "3"]),
        (
            &["config", "unset", "multi.value"],
            &["config", "--unset", "multi.value"],
        ),
    ];
    for (brit_args, git_args) in cases {
        let before = local_config(&ours)?;
        let expected = git(theirs.path(), git_args)?;
        let actual = brit(ours.path(), brit_args)?;
        assert_ne!(expected.code, Some(0), "git {git_args:?} refuses: {expected:?}");
        assert_eq!(
            actual.code, expected.code,
            "brit {brit_args:?}: {actual:?}\ngit: {expected:?}"
        );
        assert!(!actual.stderr.trim().is_empty(), "brit {brit_args:?} says why");
        assert_eq!(local_config(&ours)?, before, "brit {brit_args:?} left the config alone");
    }
    Ok(())
}

#[test]
fn a_held_config_lock_refuses_and_leaves_the_config_unchanged() -> Result<()> {
    let repo = TestRepo::new("config-lock")?;
    git_ok(repo.path(), &["config", "branch.a/b.britReachBase", "origin/main"])?;
    let lock = repo.path().join(".git/config.lock");
    std::fs::write(&lock, b"another writer\n")?;
    let before = local_config(&repo)?;
    for args in [
        &["config", "set", "branch.a/b.britReachBase", "origin/dev"][..],
        &["config", "unset", "branch.a/b.britReachBase"],
    ] {
        let out = brit(repo.path(), args)?;
        assert_ne!(out.code, Some(0), "brit {args:?}: {out:?}");
        assert!(
            out.stderr.contains("lock"),
            "brit {args:?} names the lock: {}",
            out.stderr
        );
        assert_eq!(local_config(&repo)?, before, "brit {args:?} left the config alone");
        assert_eq!(
            std::fs::read(&lock)?,
            b"another writer\n",
            "the other writer's lock stays"
        );
    }
    Ok(())
}
