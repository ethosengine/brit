//! `safe.directory` grants ownership trust from git's protected configuration: the system and global
//! files plus the invoker's command scope (`-c` and `GIT_CONFIG_COUNT`), never the repository's own
//! configuration. `Options::with(Trust::Reduced)` stands in for a repository owned by another user,
//! so the ownership decision runs without needing root to chown a fixture.
use gix_sec::Trust;
use gix_testtools::TestResult;

fn fixture() -> TestResult<std::path::PathBuf> {
    Ok(gix::path::realpath(gix_testtools::scripted_fixture_read_only(
        "make_many_packs.sh",
    )?)?)
}

fn open_reduced(path: &std::path::Path, options: gix::open::Options) -> gix::Result<gix::Repository> {
    Ok(gix::open_opts(path, options.with(Trust::Reduced))?)
}

fn safe_directory(value: impl std::fmt::Display) -> String {
    format!("safe.directory={value}")
}

#[test]
#[serial_test::serial]
fn cli_entries_grant_trust_with_git_matching_rules() -> TestResult {
    let _environment = gix_testtools::isolate_git_environment()?;
    let fixture = fixture()?;
    let display = fixture.display().to_string();
    let parent = fixture.parent().expect("fixture has a parent").display().to_string();
    let prefix_sibling = display[..display.len() - 1].to_string();

    for (entries, expected, why) in [
        (vec![display.clone()], Trust::Full, "the exact worktree root"),
        (vec!["*".to_string()], Trust::Full, "the wildcard"),
        (
            vec![format!("{parent}/*")],
            Trust::Full,
            "a trailing /* covers paths below",
        ),
        (vec![parent.clone()], Trust::Reduced, "a parent path is not the root"),
        (
            vec![prefix_sibling],
            Trust::Reduced,
            "a string prefix is not a path match",
        ),
        (vec![format!("{display}-other")], Trust::Reduced, "a different path"),
        (
            vec!["relative/path".to_string()],
            Trust::Reduced,
            "relative entries are ignored",
        ),
        (
            vec![display.clone(), String::new()],
            Trust::Reduced,
            "an empty entry resets earlier entries",
        ),
        (
            vec![String::new(), display.clone()],
            Trust::Full,
            "entries after a reset count again",
        ),
        (Vec::new(), Trust::Reduced, "no entry keeps reduced trust"),
    ] {
        let repo = open_reduced(
            &fixture,
            gix::open::Options::default().cli_overrides(entries.iter().map(safe_directory)),
        )?;
        assert_eq!(repo.git_dir_trust(), expected, "-c {entries:?}: {why}");
    }
    Ok(())
}

#[test]
#[serial_test::serial]
fn environment_entries_grant_trust_for_the_exact_root_only() -> TestResult {
    let fixture = fixture()?;
    let display = fixture.display().to_string();
    let parent = fixture.parent().expect("fixture has a parent").display().to_string();
    for (value, expected) in [
        (display.clone(), Trust::Full),
        (parent, Trust::Reduced),
        (display[..display.len() - 1].to_string(), Trust::Reduced),
    ] {
        let _environment = gix_testtools::isolate_git_environment()?
            .set("GIT_CONFIG_COUNT", "1")
            .set("GIT_CONFIG_KEY_0", "safe.directory")
            .set("GIT_CONFIG_VALUE_0", value.clone());
        let repo = open_reduced(&fixture, gix::open::Options::default())?;
        assert_eq!(repo.git_dir_trust(), expected, "GIT_CONFIG_VALUE_0={value}");
        if expected == Trust::Full {
            assert_eq!(
                repo.allocation_limit(),
                None,
                "a promoted repository has no reduced-trust allocation limit"
            );
        } else {
            assert!(
                repo.allocation_limit().is_some(),
                "a repository left at reduced trust keeps its allocation limit"
            );
        }
    }

    let _environment = gix_testtools::isolate_git_environment()?
        .set("GIT_CONFIG_COUNT", "1")
        .set("GIT_CONFIG_KEY_0", "safe.directory")
        .set("GIT_CONFIG_VALUE_0", display);
    let isolated = open_reduced(&fixture, gix::open::Options::isolated())?;
    assert_eq!(
        isolated.git_dir_trust(),
        Trust::Reduced,
        "without permission to read environment configuration there is no environment grant"
    );
    Ok(())
}

#[test]
#[serial_test::serial]
fn neither_repository_nor_api_configuration_grants_trust() -> TestResult {
    let _environment = gix_testtools::isolate_git_environment()?;
    let fixture = gix_testtools::scripted_fixture_writable("make_many_packs.sh")?;
    let root = gix::path::realpath(fixture.path())?;
    let mut config = std::fs::OpenOptions::new()
        .append(true)
        .open(root.join(".git/config"))?;
    std::io::Write::write_all(
        &mut config,
        format!(
            "\n[safe]\n\tdirectory = {}\n\tdirectory = *\n[core]\n\thooksPath = repository-hooks\n",
            root.display()
        )
        .as_bytes(),
    )?;
    drop(config);

    let repo = open_reduced(&root, gix::open::Options::default())?;
    assert_eq!(
        repo.git_dir_trust(),
        Trust::Reduced,
        "a repository cannot vouch for itself through its own configuration"
    );
    assert!(
        repo.config_snapshot().trusted_path("core.hooksPath")?.is_none(),
        "repository sections stay untrusted at reduced trust"
    );

    let repo = open_reduced(
        &root,
        gix::open::Options::default().config_overrides([safe_directory(root.display())]),
    )?;
    assert_eq!(
        repo.git_dir_trust(),
        Trust::Reduced,
        "programmatic overrides are not git's command scope"
    );

    let repo = open_reduced(
        &root,
        gix::open::Options::default().cli_overrides([safe_directory(root.display())]),
    )?;
    assert_eq!(repo.git_dir_trust(), Trust::Full, "the invoker's -c grants trust");
    assert!(
        repo.config_snapshot().trusted_path("core.hooksPath")?.is_some(),
        "once the invoker trusts the checkout its own configuration is trusted, as with a global entry"
    );
    Ok(())
}
