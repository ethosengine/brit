//! Check the raw bytes a script receives, without journey output normalization.

use cli_journey::support::{runner::brit_bin, test_repo::TestRepo};
use gix_testtools::TestResult;

#[test]
fn piped_status_contains_only_the_status_answer() -> TestResult {
    let repo = TestRepo::new("status-diagnostics")?;
    std::fs::write(repo.path().join("untracked.txt"), "untracked\n")?;
    let mut command = std::process::Command::new(brit_bin().expect("built Brit"));
    let out = gix_testtools::configure_git_environment(&mut command, repo.path())
        .current_dir(repo.path())
        .env("GIT_CONFIG_GLOBAL", repo.path().join("missing.gitconfig"))
        .env_remove("EPR_FEEDBACK_ROOT")
        .args(["status", "--no-write"])
        .output()?;
    assert!(out.status.success(), "status succeeds: {out:?}");
    assert_eq!(out.stdout, b"  ? untracked.txt\n", "status answer bytes");
    assert!(!out.stderr.contains(&0x1b), "piped stderr has no line clears: {out:?}");
    Ok(())
}

#[cfg(unix)]
mod feedback {
    use std::{os::unix::fs::PermissionsExt, path::PathBuf};

    use cli_journey::support::{
        parity::git,
        runner::{brit_bin, BritInvocation},
        test_repo::TestRepo,
    };
    use gix_testtools::TestResult;

    const GAP: &str = "brit: feedback capture gap: epr: test policy refused";

    struct Fixture {
        repo: TestRepo,
        state: tempfile::TempDir,
        path: std::ffi::OsString,
        binary: PathBuf,
    }

    impl Fixture {
        fn new() -> TestResult<Self> {
            let repo = TestRepo::new("feedback-diagnostics")?;
            let state = tempfile::tempdir()?;
            let bin = state.path().join("bin");
            std::fs::create_dir(&bin)?;
            let recorder = bin.join("epr");
            std::fs::write(
                &recorder,
                r#"#!/bin/sh
test "$1 $2" = 'feedback capture' || exit 99
printf '%s\n' called >> "$FEEDBACK_TEST_CALLS"
printf '%s\n' 'recorder stdout must stay private'
printf '%s\n' 'epr: test policy refused' >&2
exit 47
"#,
            )?;
            std::fs::set_permissions(&recorder, std::fs::Permissions::from_mode(0o700))?;
            let inherited_path = std::env::var_os("PATH").unwrap_or_default();
            let path = std::env::join_paths(std::iter::once(bin).chain(std::env::split_paths(&inherited_path)))?;
            Ok(Self {
                repo,
                state,
                path,
                binary: brit_bin().expect("built Brit"),
            })
        }

        fn brit(&self, session: &str) -> BritInvocation {
            BritInvocation::new(&self.binary)
                .current_dir(self.repo.path())
                .env("PATH", &self.path)
                .env("EPR_FEEDBACK_ROOT", self.state.path())
                .env("ELOHIM_SESSION_ID", session)
                .env("XDG_CACHE_HOME", self.state.path().join("cache"))
                .env("FEEDBACK_TEST_CALLS", self.state.path().join("calls"))
                .env("BRIT_QUIET", "0")
        }
    }

    #[test]
    fn piped_ref_queries_keep_recorder_diagnostics_on_stderr() -> TestResult {
        let fixture = Fixture::new()?;
        let out = fixture.brit("rev-parse").args(["rev-parse", "HEAD"]).run()?;
        assert!(out.status.success(), "rev-parse succeeds: {}", out.stderr);
        assert_eq!(out.stdout, format!("{}\n", fixture.repo.head_id()?), "only the hash");
        assert!(out.stderr.contains(GAP), "stderr identifies the cause: {}", out.stderr);

        let args = ["ls-remote", "--quiet", ".", "refs/heads/main"];
        let expected = git(fixture.repo.path(), &args)?;
        assert_eq!(expected.code, Some(0), "Git reads the local remote: {expected:?}");
        let out = fixture.brit("ls-remote").args(args).run()?;
        assert!(out.status.success(), "ls-remote succeeds: {}", out.stderr);
        assert_eq!(out.stdout, expected.stdout, "only the ref advertisement");
        assert!(
            out.stderr.contains(GAP),
            "ls-remote's own --quiet still allows the gap: {}",
            out.stderr
        );
        Ok(())
    }

    #[test]
    fn a_capture_gap_is_reported_once_per_session() -> TestResult {
        let fixture = Fixture::new()?;
        for (session, count) in [("first-session", 1), ("first-session", 0), ("next-session", 1)] {
            let out = fixture.brit(session).args(["rev-parse", "HEAD"]).run()?;
            assert!(out.status.success(), "rev-parse succeeds: {}", out.stderr);
            assert_eq!(
                out.stderr.matches(GAP).count(),
                count,
                "capture gaps in {session}: {}",
                out.stderr
            );
        }
        let calls = std::fs::read_to_string(fixture.state.path().join("calls"))?;
        assert_eq!(calls.lines().count(), 3, "the recorder is called on every invocation");
        Ok(())
    }

    #[test]
    fn quiet_switches_suppress_the_line_without_disabling_capture() -> TestResult {
        let fixture = Fixture::new()?;
        for cmd in [
            fixture.brit("quiet").env("BRIT_QUIET", "1"),
            fixture.brit("quiet").arg("--quiet"),
        ] {
            let out = cmd.args(["rev-parse", "HEAD"]).run()?;
            assert!(out.status.success(), "rev-parse succeeds: {}", out.stderr);
            assert!(out.stderr.is_empty(), "the first gap is quiet: {}", out.stderr);
        }
        let help = fixture.brit("quiet").args(["--quiet", "--help"]).run()?;
        assert!(help.status.success(), "help succeeds: {}", help.stderr);
        assert!(help.stderr.is_empty(), "help also honors root --quiet: {}", help.stderr);

        let bridge = fixture.brit("quiet").args(["--quiet", "stash", "list"]).run()?;
        assert_eq!(bridge.status.code(), Some(2), "the bridge still refuses");
        assert_eq!(
            bridge.stderr, "brit: stash is a Git bridge operation; run: git stash list\n",
            "root --quiet leaves the bridge diagnostic on stderr"
        );
        assert!(bridge.stdout.is_empty(), "the bridge writes its diagnostic to stderr");

        let out = fixture.brit("quiet").args(["rev-parse", "HEAD"]).run()?;
        assert!(out.status.success(), "rev-parse succeeds: {}", out.stderr);
        assert!(
            out.stderr.contains(GAP),
            "quiet invocations leave the warning available"
        );
        let calls = std::fs::read_to_string(fixture.state.path().join("calls"))?;
        assert_eq!(calls.lines().count(), 5, "quiet only affects reporting");
        Ok(())
    }
}
