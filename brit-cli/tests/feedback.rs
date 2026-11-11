//! Real CLI results survive optional recording, including parser and intentional bridge exits.
use std::process::Command;

#[test]
fn bounded_history_supports_long_short_and_numeric_spellings() {
    let repo = gix_testtools::tempfile::tempdir().expect("fixture");
    gix_testtools::git(repo.path(), "init").expect("init");
    for n in 0..3 {
        std::fs::write(repo.path().join("file"), n.to_string()).expect("write");
        gix_testtools::git(repo.path(), "add file").expect("add");
        gix_testtools::git(repo.path(), &format!("commit -m commit-{n}")).expect("commit");
    }
    for (args, expected) in [
        (&["log", "-2"][..], 2),
        (&["log", "-n", "2"][..], 2),
        (&["log", "--max-count", "2"][..], 2),
        (&["log", "--max-count=1"][..], 1),
        (&["-r", ".", "log", "-2"][..], 2),
        (&["log", "-5"][..], 3),
        (&["log", "-0"][..], 0),
        (&["log", "-n", "0"][..], 0),
        (&["log", "--max-count", "0"][..], 0),
    ] {
        let mut command = Command::new(env!("CARGO_BIN_EXE_brit"));
        gix_testtools::configure_git_environment(&mut command, repo.path());
        let out = command
            .current_dir(repo.path())
            .env_remove("EPR_FEEDBACK_ROOT")
            .args(args)
            .output()
            .expect("brit");
        assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
        let stdout = String::from_utf8_lossy(&out.stdout);
        let titles: Vec<_> = stdout
            .lines()
            .map(|line| line.split_once(' ').expect("commit and title").1)
            .collect();
        assert_eq!(titles, ["commit-2", "commit-1", "commit-0"][..expected]);
    }
}

#[test]
fn history_preserves_path_refusals_and_option_terminators() {
    let repo = gix_testtools::tempfile::tempdir().expect("fixture");
    gix_testtools::git(repo.path(), "init").expect("init");
    for args in [
        &["log", "-2", "file"][..],
        &["log", "--", "-2"][..],
        &["log", "-n", "2", "--", "-5"][..],
    ] {
        let mut command = Command::new(env!("CARGO_BIN_EXE_brit"));
        gix_testtools::configure_git_environment(&mut command, repo.path());
        let out = command
            .current_dir(repo.path())
            .env_remove("EPR_FEEDBACK_ROOT")
            .args(args)
            .output()
            .expect("brit");
        assert!(!out.status.success());
        assert!(out.stdout.is_empty());
        assert!(String::from_utf8_lossy(&out.stderr).contains("File-based lookup isn't yet implemented"));
    }
    for args in [&["log", "--max-count", "-1"][..], &["log", "-n", "many"][..]] {
        let mut command = Command::new(env!("CARGO_BIN_EXE_brit"));
        gix_testtools::configure_git_environment(&mut command, repo.path());
        let out = command
            .current_dir(repo.path())
            .env_remove("EPR_FEEDBACK_ROOT")
            .args(args)
            .output()
            .expect("brit");
        assert_eq!(out.status.code(), Some(2));
        assert!(out.stdout.is_empty());
    }
}

#[cfg(unix)]
#[test]
fn capture_is_structured_correlated_and_subordinate_to_the_command() {
    use std::os::unix::fs::PermissionsExt;
    let fixture = gix_testtools::tempfile::tempdir().expect("fixture");
    let bin = fixture.path().join("bin");
    std::fs::create_dir(&bin).expect("bin");
    let recorder = bin.join("epr");
    std::fs::write(
        &recorder,
        "#!/bin/sh\nprintf '%s\\n' \"$@\" > \"$FEEDBACK_TEST_ARGS\"\nexit 47\n",
    )
    .expect("recorder");
    std::fs::set_permissions(&recorder, std::fs::Permissions::from_mode(0o700)).expect("executable");
    let captured = fixture.path().join("args");
    for (args, operation, outcome, code) in [
        (&["--version"][..], "other", "passed", 0),
        (&["show", "SECRET_ARGUMENT"][..], "bridge", "bridge-declared", 2),
        (&["log", "--unknown-secret-option"][..], "log", "refused", 2),
        (&["-r", ".", "log", "--unknown-secret-option"][..], "log", "refused", 2),
        (
            &["-r", "log", "status", "--unknown-secret-option"][..],
            "status",
            "refused",
            2,
        ),
    ] {
        let mut command = Command::new(env!("CARGO_BIN_EXE_brit"));
        gix_testtools::configure_git_environment(&mut command, fixture.path());
        let out = command
            .current_dir(fixture.path())
            .args(args)
            .env("PATH", &bin)
            .env("EPR_FEEDBACK_ROOT", fixture.path())
            .env("EPR_FEEDBACK_INVOCATION", "same-invocation")
            .env("FEEDBACK_TEST_ARGS", &captured)
            .output()
            .expect("brit");
        assert_eq!(out.status.code(), Some(code), "capture failure preserves exit");
        let captured = std::fs::read_to_string(&captured).expect("captured argv");
        assert!(captured.contains(&format!("--operation\n{operation}\n")), "{captured}");
        assert!(captured.contains(outcome));
        assert!(captured.contains("same-invocation"));
        assert!(!captured.contains("SECRET_ARGUMENT"));
        assert!(!captured.contains("unknown-secret-option"));
        assert!(String::from_utf8_lossy(&out.stderr).contains("feedback capture gap"));
    }
}

#[cfg(unix)]
#[test]
fn absent_or_stalled_recorder_cannot_change_command_results() {
    use std::{
        os::unix::fs::PermissionsExt,
        time::{Duration, Instant},
    };
    let fixture = gix_testtools::tempfile::tempdir().expect("fixture");
    let bin = fixture.path().join("bin");
    std::fs::create_dir(&bin).expect("bin");
    for stalled in [false, true] {
        if stalled {
            let recorder = bin.join("epr");
            std::fs::write(&recorder, "#!/bin/sh\nexec /bin/sleep 30\n").expect("stalled recorder");
            std::fs::set_permissions(&recorder, std::fs::Permissions::from_mode(0o700)).expect("executable");
        }
        for (args, code) in [(&["--version"][..], 0), (&["log", "--invalid-private"][..], 2)] {
            let mut command = Command::new(env!("CARGO_BIN_EXE_brit"));
            gix_testtools::configure_git_environment(&mut command, fixture.path());
            let start = Instant::now();
            let out = command
                .current_dir(fixture.path())
                .args(args)
                .env("PATH", &bin)
                .env("EPR_FEEDBACK_ROOT", fixture.path())
                .output()
                .expect("brit");
            assert_eq!(out.status.code(), Some(code));
            assert!(String::from_utf8_lossy(&out.stderr).contains("feedback capture gap"));
            assert!(start.elapsed() < Duration::from_secs(8), "bounded recorder wait");
        }
    }
}
