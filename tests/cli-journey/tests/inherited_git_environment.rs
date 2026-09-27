//! A journey fixture must stay inside its own disposable repository even when the
//! parent process supplies Git overrides pointing at another disposable repository.

#[cfg(unix)]
#[test]
fn support_helpers_ignore_hostile_inherited_git_paths_and_user_config() -> gix_testtools::Result {
    if gix_testtools::run_in_isolated_process()? {
        return Ok(());
    }

    use cli_journey::support::{mock_remote::MockRemote, runner::BritInvocation, test_repo::TestRepo};

    let decoy = TestRepo::new("hostile-decoy")?;
    let decoy_head = decoy.head_id()?;
    let home = tempfile::tempdir()?;
    let user_config = home.path().join(".gitconfig");
    std::fs::write(
        &user_config,
        "[user]\n\tname = Hostile User\n\temail = hostile@example.com\n",
    )?;

    // These process-wide mutations run only in the isolated child. Both repositories
    // are disposable, and no inherited override can point at the source checkout.
    std::env::set_var("GIT_DIR", decoy.path().join(".git"));
    std::env::set_var("GIT_WORK_TREE", decoy.path());
    std::env::set_var("GIT_COMMON_DIR", decoy.path().join(".git"));
    std::env::set_var("GIT_INDEX_FILE", decoy.path().join(".git/index"));
    std::env::set_var("GIT_OBJECT_DIRECTORY", decoy.path().join(".git/objects"));
    std::env::set_var("GIT_CONFIG_GLOBAL", &user_config);
    std::env::set_var("GIT_CONFIG_COUNT", "1");
    std::env::set_var("GIT_CONFIG_KEY_0", "user.name");
    std::env::set_var("GIT_CONFIG_VALUE_0", "Hostile Override");
    std::env::set_var("XDG_CONFIG_HOME", home.path());

    let repo = TestRepo::new("isolated-target")?;
    let target_head = repo.commit_file("own.txt", "target bytes\n")?;
    assert_eq!(
        repo.head_id()?,
        target_head,
        "target HEAD is read from target repository"
    );
    assert_eq!(decoy.head_id()?, decoy_head, "decoy HEAD must not move");
    assert!(
        !decoy.path().join("own.txt").exists(),
        "target file must not appear in decoy"
    );

    let author = gix_testtools::git_command(repo.path())
        .args(["log", "-1", "--format=%an <%ae>"])
        .output()?;
    assert!(author.status.success(), "read deterministic fixture author");
    assert_eq!(
        String::from_utf8(author.stdout)?.trim(),
        "Sebastian Thiel <git@example.com>",
        "static fixture author survives inherited user configuration"
    );

    let remote = MockRemote::new("hostile-remote")?;
    let bare = gix_testtools::git_command(remote.path())
        .args(["rev-parse", "--is-bare-repository"])
        .output()?;
    assert!(bare.status.success(), "mock remote is readable as bare repo");
    assert_eq!(String::from_utf8(bare.stdout)?.trim(), "true");
    assert_eq!(decoy.head_id()?, decoy_head, "mock remote creation did not move decoy");

    // BritInvocation can launch a program that invokes Git indirectly. Its child
    // must see the target working tree, while an explicit test override still wins.
    let capture = BritInvocation::new("sh")
        .args(["-c", "git rev-parse --show-toplevel; printf '%s' \"$JOURNEY_OVERRIDE\""])
        .current_dir(repo.path())
        .env("JOURNEY_OVERRIDE", "explicit")
        .run()?;
    assert!(
        capture.status.success(),
        "indirect Git invocation succeeds: {}",
        capture.stderr
    );
    let mut lines = capture.stdout.lines();
    assert_eq!(
        lines.next(),
        Some(repo.path().to_str().expect("temporary repository path is UTF-8")),
        "indirect Git resolves the target repository"
    );
    assert_eq!(lines.next(), Some("explicit"), "explicit invocation override wins");
    assert_eq!(decoy.head_id()?, decoy_head, "runner did not move decoy");
    Ok(())
}
