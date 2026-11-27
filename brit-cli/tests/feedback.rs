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
fn history_keeps_numeric_paths_after_the_option_terminator() {
    let repo = gix_testtools::tempfile::tempdir().expect("fixture");
    gix_testtools::git(repo.path(), "init").expect("init");
    for n in 0..3 {
        std::fs::write(repo.path().join("file"), n.to_string()).expect("write");
        gix_testtools::git(repo.path(), "add file").expect("add");
        gix_testtools::git(repo.path(), &format!("commit -m commit-{n}")).expect("commit");
    }
    // `-2` before `--` is a count; after it, `-2` and `-5` are paths no commit touched.
    for (args, expected) in [
        (&["log", "-2", "file"][..], 2),
        (&["log", "--", "-2"][..], 0),
        (&["log", "-n", "2", "--", "-5"][..], 0),
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
        assert_eq!(
            String::from_utf8_lossy(&out.stdout).lines().count(),
            expected,
            "{args:?}"
        );
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
    // The refusal is followed by 10,000 detail lines, more than a pipe holds. They are
    // written in 100 blocks: a recorder has two seconds, and 10,000 separate writes took
    // longer than that on a loaded four-core macOS runner while the command was correct.
    std::fs::write(
        &recorder,
        r#"#!/bin/sh
printf '%s\n' "$@" > "$FEEDBACK_TEST_ARGS"
printf '%s\n' 'epr: feedback policy refused' >&2
block='secondary recorder detail'
i=1
while [ "$i" -lt 100 ]; do
  block="$block
secondary recorder detail"
  i=$((i+1))
done
i=0
while [ "$i" -lt 100 ]; do
  printf '%s\n' "$block" >&2
  i=$((i+1))
done
exit 47
"#,
    )
    .expect("recorder");
    std::fs::set_permissions(&recorder, std::fs::Permissions::from_mode(0o700)).expect("executable");
    let captured = fixture.path().join("args");
    for (index, (args, operation, outcome, code)) in [
        (&["--version"][..], "other", "passed", 0),
        (&["stash", "SECRET_ARGUMENT"][..], "bridge", "bridge-declared", 2),
        (&["log", "--unknown-secret-option"][..], "log", "refused", 2),
        (&["-r", ".", "log", "--unknown-secret-option"][..], "log", "refused", 2),
        (
            &["-r", "log", "status", "--unknown-secret-option"][..],
            "status",
            "refused",
            2,
        ),
    ]
    .into_iter()
    .enumerate()
    {
        let mut command = Command::new(env!("CARGO_BIN_EXE_brit"));
        gix_testtools::configure_git_environment(&mut command, fixture.path());
        let out = command
            .current_dir(fixture.path())
            .args(args)
            .env("PATH", &bin)
            .env("EPR_FEEDBACK_ROOT", fixture.path())
            .env("ELOHIM_SESSION_ID", index.to_string())
            .env("EPR_FEEDBACK_INVOCATION", "same-invocation")
            .env("XDG_CACHE_HOME", fixture.path().join("cache"))
            .env("XDG_STATE_HOME", fixture.path().join("state"))
            .env_remove("BRIT_QUIET")
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
        let stderr = String::from_utf8_lossy(&out.stderr);
        assert!(
            stderr.contains(
                "feedback capture gap: refused; command result unchanged [fb epr/recorder-unavailable:refused]"
            ),
            "the gap has the typed recorder cause: {stderr}"
        );
        assert!(
            !stderr.contains("secondary recorder detail"),
            "only the first line is reported"
        );
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
                .env("ELOHIM_SESSION_ID", format!("{stalled}-{code}"))
                .env_remove("EPR_FEEDBACK_INVOCATION")
                .env("XDG_CACHE_HOME", fixture.path().join("cache"))
                .env("XDG_STATE_HOME", fixture.path().join("state"))
                .env_remove("BRIT_QUIET")
                .output()
                .expect("brit");
            assert_eq!(out.status.code(), Some(code));
            let cause = if stalled { "timeout" } else { "not-found" };
            let stderr = String::from_utf8_lossy(&out.stderr);
            assert!(
                stderr.contains(&format!(
                    "feedback capture gap: {cause}; command result unchanged [fb epr/recorder-unavailable:{cause}]"
                )),
                "the recorder failure names its cause: {stderr}"
            );
            assert!(start.elapsed() < Duration::from_secs(8), "bounded recorder wait");
        }
    }
}

#[cfg(unix)]
mod markers {
    use std::{
        fs,
        os::unix::fs::PermissionsExt,
        path::Path,
        process::{Command, Output},
        time::{Duration, Instant, UNIX_EPOCH},
    };

    use gix_testtools::{tempfile, TestResult};
    use serde_json::Value;

    struct Fixture {
        root: tempfile::TempDir,
    }

    impl Fixture {
        fn new() -> TestResult<Self> {
            let root = tempfile::tempdir()?;
            fs::create_dir(root.path().join("bin"))?;
            gix_testtools::git(root.path(), "init")?;
            gix_testtools::git(root.path(), "commit --allow-empty -m fixture")?;
            Ok(Self { root })
        }

        fn recorder(&self, body: &str) -> TestResult {
            let path = self.root.path().join("bin/epr");
            fs::write(
                &path,
                format!("#!/bin/sh\nprintf '%s\\n' called >> \"$FEEDBACK_TEST_CALLS\"\n{body}"),
            )?;
            fs::set_permissions(path, fs::Permissions::from_mode(0o700))?;
            Ok(())
        }

        fn command(&self, session: &str) -> Command {
            let mut command = Command::new(env!("CARGO_BIN_EXE_brit"));
            let root = self.root.path();
            gix_testtools::configure_git_environment(&mut command, root);
            command
                .current_dir(root)
                .env("PATH", root.join("bin"))
                .env("XDG_CACHE_HOME", root.join("cache"))
                .env("XDG_STATE_HOME", root.join("state"))
                .env("FEEDBACK_TEST_CALLS", root.join("calls"))
                .env("EPR_FEEDBACK_ROOT", root)
                .env("ELOHIM_SESSION_ID", session)
                .env_remove("EPR_FEEDBACK_INVOCATION")
                .env_remove("EPR_BIN_RECORD")
                .env_remove("BRIT_QUIET");
            command
        }

        fn run(&self, session: &str, args: &[&str], code: i32) -> TestResult<Output> {
            let out = self.command(session).args(args).output()?;
            assert_eq!(out.status.code(), Some(code), "{args:?}: {out:?}");
            Ok(out)
        }

        fn calls(&self) -> TestResult<usize> {
            Ok(fs::read_to_string(self.root.path().join("calls"))?.lines().count())
        }
    }

    #[test]
    fn full_feedback_spool_reports_once_and_skips_further_recorder_spawns() -> TestResult {
        let fixture = Fixture::new()?;
        // A recorder that never answers is unavailable for the session; a refusal is not.
        fixture.recorder("exec /bin/sleep 5\n")?;
        let spool = fixture.root.path().join("state/brit/feedback-spool");
        fs::create_dir_all(&spool)?;
        for index in 0..256 {
            fs::write(spool.join(format!("{index}.json")), b"{}")?;
        }

        let first = fixture.run("first", &["--version"], 0)?;
        assert_eq!(
            String::from_utf8(first.stderr)?,
            full_spool_notice("timeout"),
            "the initial full-spool notice carries the actual gap cause"
        );
        assert_eq!(
            fixture.calls()?,
            1,
            "the initial failure identifies the unavailable recorder"
        );
        let repeat = fixture.run("first", &["status", "--no-write"], 0)?;
        assert!(
            repeat.stderr.is_empty(),
            "the suppressed recorder stays quiet: {repeat:?}"
        );
        assert_eq!(fixture.calls()?, 1, "other claims also skip the unavailable recorder");
        assert_eq!(fs::read_dir(&spool)?.count(), 256, "no new gap enters the full spool");

        fixture.recorder("# a different recorder identity\nexit 0\n")?;
        fixture.run("first", &["--version"], 0)?;
        assert_eq!(fixture.calls()?, 2, "a replacement recorder can capture at capacity");
        Ok(())
    }

    #[test]
    fn quiet_full_feedback_spool_notices_wait_for_a_visible_observe() -> TestResult {
        let fixture = Fixture::new()?;
        fixture.recorder("exit 47\n")?;
        let spool = fixture.root.path().join("state/brit/feedback-spool");
        fs::create_dir_all(&spool)?;
        for index in 0..256 {
            fs::write(spool.join(format!("{index}.json")), b"{}")?;
        }
        let quiet = fixture
            .command("first")
            .env("BRIT_QUIET", "1")
            .arg("--version")
            .output()?;
        assert!(quiet.status.success(), "capture preserves the command result");
        assert!(quiet.stderr.is_empty(), "quiet mode suppresses the full-spool notice");

        let visible = fixture.run("first", &["--version"], 0)?;
        assert_eq!(
            String::from_utf8(visible.stderr)?,
            full_spool_notice("refused"),
            "a visible observation reports the cause saved by the quiet observation"
        );
        assert_eq!(fixture.calls()?, 1, "the full-spool marker suppresses a repeat spawn");
        let other_claim = fixture.run("first", &["status", "--no-write"], 0)?;
        assert!(other_claim.stderr.is_empty(), "another claim shares the session notice");
        assert_eq!(fixture.calls()?, 2, "a refusal only suppresses the original claim");
        Ok(())
    }

    fn full_spool_notice(cause: &str) -> String {
        format!("brit: feedback spool full (256 entries); recorder gaps are not kept until epr drains it; command result unchanged [fb epr/recorder-unavailable:{cause}]\n")
    }

    #[test]
    fn a_refusing_recorder_marks_only_the_refused_claim() -> TestResult {
        let fixture = Fixture::new()?;
        fixture.recorder("exit 47\n")?;
        let spool = fixture.root.path().join("state/brit/feedback-spool");

        fixture.run("first", &["--version"], 0)?;
        assert_eq!(fixture.calls()?, 1);
        assert_eq!(fs::read_dir(&spool)?.count(), 1, "the refusal is kept for epr");
        fixture.run("first", &["--version"], 0)?;
        assert_eq!(fixture.calls()?, 1, "the refused claim is not retried in the session");
        let other = fixture.run("first", &["status", "--no-write"], 0)?;
        assert_eq!(fixture.calls()?, 2, "a refusal does not silence other claims");
        assert!(
            other.stderr.is_empty(),
            "the gap line is still shown at most once per session: {other:?}"
        );
        assert_eq!(
            fs::read_dir(&spool)?.count(),
            2,
            "each refused claim keeps its own record"
        );
        Ok(())
    }

    #[test]
    fn successful_capture_is_once_per_claim_outcome_and_session() -> TestResult {
        let fixture = Fixture::new()?;
        fixture.recorder("exit 0\n")?;
        for (session, args, code, calls) in [
            ("first", &["log", "--invalid-private"][..], 2, 1),
            ("first", &["log", "--invalid-private"][..], 2, 1),
            ("first", &["log"][..], 0, 2),
            ("first", &["log"][..], 0, 2),
            ("first", &["status", "--no-write"][..], 0, 3),
            ("second", &["log"][..], 0, 4),
        ] {
            fixture.run(session, args, code)?;
            assert_eq!(fixture.calls()?, calls, "{session}: {args:?}");
        }
        for (emitter, calls) in [("new-emitter", 5), ("new-emitter", 5), ("replacement-emitter", 6)] {
            let out = fixture
                .command("first")
                .env("EPR_BIN_RECORD", emitter)
                .args(["log"])
                .output()?;
            assert!(out.status.success(), "{out:?}");
            assert_eq!(fixture.calls()?, calls, "success markers also include the emitter");
        }
        Ok(())
    }

    #[test]
    fn capture_sends_claim_and_emitter_in_environment_without_changing_argv() -> TestResult {
        let fixture = Fixture::new()?;
        fixture.recorder(
            r#"printf '%s\n' "$EPR_FEEDBACK_CLAIM" "$EPR_FEEDBACK_EMITTER" "$@" > "$FEEDBACK_TEST_ARGS"
"#,
        )?;
        let recorded_emitter = "bafkreiaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
        let captured = fixture.root.path().join("recorder-input");
        for (index, (args, code, claim, operation, outcome)) in [
            (
                &["status", "--no-write"][..],
                0,
                "brit/command:status",
                "status",
                "passed",
            ),
            (
                &["stash", "PRIVATE_ARG"][..],
                2,
                "brit/capability-absent:stash",
                "bridge",
                "bridge-declared",
            ),
            (
                &["not-a-brit-command"][..],
                2,
                "brit/capability-absent:unknown",
                "other",
                "refused",
            ),
            (&["tag", "-d", "missing"][..], 1, "brit/command:tag", "tag", "failed"),
            (
                &["tag", "delete", "missing"][..],
                1,
                "brit/command:tag",
                "tag",
                "failed",
            ),
            (
                &["-c", "commit.gpgSign=true", "commit", "-m", "example"][..],
                1,
                "brit/capability-absent:commit.signing",
                "commit",
                "failed",
            ),
            (
                &["push", "--force-if-includes"][..],
                1,
                "brit/capability-absent:push.force-if-includes",
                "push",
                "failed",
            ),
            (
                &["push", "--force", "origin", "refs/notes/test:refs/notes/test"][..],
                1,
                "brit/refused:push.force",
                "push",
                "failed",
            ),
            (
                &["push", "--delete", "origin", "refs/notes/test:refs/notes/test"][..],
                1,
                "brit/refused:push.deletion",
                "push",
                "failed",
            ),
        ]
        .into_iter()
        .enumerate()
        {
            let session = index.to_string();
            let invocation = format!("declared-{index}");
            let output = fixture
                .command(&session)
                .env("EPR_FEEDBACK_INVOCATION", &invocation)
                .env("EPR_BIN_RECORD", recorded_emitter)
                .env("FEEDBACK_TEST_ARGS", &captured)
                .args(args)
                .output()?;
            assert_eq!(output.status.code(), Some(code), "{args:?}: {output:?}");
            let capture = fs::read_to_string(&captured)?;
            let lines = capture.lines().collect::<Vec<_>>();
            assert_eq!(lines.len(), 16, "{args:?}: {capture:?}");
            assert!(lines[13].parse::<u128>().is_ok(), "elapsed time stays a number");
            assert_eq!(
                lines,
                [
                    claim,
                    recorded_emitter,
                    "feedback",
                    "capture",
                    "--producer",
                    "brit",
                    "--operation",
                    operation,
                    "--outcome",
                    outcome,
                    "--invocation",
                    &invocation,
                    "--elapsed-ms",
                    lines[13],
                    "--root",
                    fixture.root.path().to_str().expect("fixture root"),
                ],
                "{args:?}: recorder environment and complete argv"
            );
        }

        let output = fixture
            .command("unrecorded")
            .env("FEEDBACK_TEST_ARGS", &captured)
            .args(["--version"])
            .output()?;
        assert!(output.status.success());
        let version = String::from_utf8(output.stdout)?;
        let source = version
            .lines()
            .find_map(|line| line.strip_prefix("source HEAD: "))
            .expect("source SHA");
        let identity = if source == "unavailable" {
            let digest = gix::hash::bytes_of_file(
                Path::new(env!("CARGO_BIN_EXE_brit")),
                fs::metadata(env!("CARGO_BIN_EXE_brit"))?.len(),
                gix::hash::Kind::Sha256,
                &mut gix::progress::Discard,
                &std::sync::atomic::AtomicBool::new(false),
            )?
            .to_string();
            format!("exe.{}", &digest[..12])
        } else {
            source[..12].to_owned()
        };
        let fallback = format!("unrecorded:{}+{identity}", env!("CARGO_PKG_VERSION"));
        let capture = fs::read_to_string(&captured)?;
        assert_eq!(
            capture.lines().nth(1),
            Some(fallback.as_str()),
            "an unrecorded checkout builds its emitter from version and SHA"
        );
        Ok(())
    }

    #[test]
    fn timed_out_recorder_is_skipped_until_its_identity_or_session_changes() -> TestResult {
        let fixture = Fixture::new()?;
        fixture.recorder("exec /bin/sleep 30\n")?;
        fixture.run("first", &["--version"], 0)?;
        assert_eq!(fixture.calls()?, 1, "one timed out spawn");
        let spool = fixture.root.path().join("state/brit/feedback-spool");
        let baseline = Instant::now();
        fixture
            .command("first")
            .env_remove("EPR_FEEDBACK_ROOT")
            .args(["status", "--help"])
            .output()?;
        let baseline = baseline.elapsed();
        let start = Instant::now();
        let repeat = fixture.run("first", &["status", "--help"], 0)?;
        assert!(
            start.elapsed().saturating_sub(baseline) < Duration::from_millis(200),
            "a skipped recorder adds under 200 ms: {:?} including the command",
            start.elapsed()
        );
        assert!(repeat.stderr.is_empty(), "the session has already reported the gap");
        assert_eq!(fixture.calls()?, 1, "another claim cannot spawn the unchanged recorder");
        assert_eq!(fs::read_dir(&spool)?.count(), 1, "the same identity has one record");

        fixture.run("second", &["--version"], 0)?;
        assert_eq!(fixture.calls()?, 2, "a different session retries the same recorder");
        assert_eq!(fs::read_dir(&spool)?.count(), 2, "each session records its own timeout");

        let diagnostic = "é€".repeat(150);
        fixture.recorder(&format!("printf '%s\\n' '{diagnostic}' more >&2\nexit 47\n"))?;
        let recorder = fixture.root.path().join("bin/epr").canonicalize()?;
        let metadata = fs::metadata(&recorder)?;
        let changed = fixture.run("first", &["--version"], 0)?;
        assert!(
            changed.stderr.is_empty(),
            "changing recorder identity keeps the session notice spent"
        );
        fixture.run("first", &["--version"], 0)?;
        assert_eq!(fixture.calls()?, 3, "the changed identity retries once");

        let files = fs::read_dir(&spool)?.collect::<Result<Vec<_>, _>>()?;
        assert_eq!(files.len(), 3, "the changed identity adds one record");
        let mut refused = 0;
        for file in files {
            let name = file.file_name();
            let name = name.to_str().expect("ASCII spool filename");
            let digest = name.strip_suffix(".json").expect("JSON suffix");
            assert_eq!(digest.len(), 32, "name carries 16 bytes of SHA-256");
            assert!(digest
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase()));
            let bytes = fs::read(file.path())?;
            assert!(bytes.len() <= 1024, "the spool record fits its wire limit");
            let row: Value = serde_json::from_slice(&bytes)?;
            assert_eq!(
                row.as_object()
                    .expect("spool object")
                    .keys()
                    .map(String::as_str)
                    .collect::<Vec<_>>(),
                ["at", "cause", "claim", "emitter", "recorder", "session", "stderr", "v"],
                "exact wire keys"
            );
            let recorder_row = row["recorder"].as_object().expect("recorder object");
            assert_eq!(
                recorder_row.keys().map(String::as_str).collect::<Vec<_>>(),
                ["mtime", "path", "size"]
            );
            assert_eq!(row["v"], 1);
            assert_eq!(
                row["claim"],
                format!("epr/recorder-unavailable:{}", row["cause"].as_str().expect("cause"))
            );
            assert_eq!(row["session"].as_str().expect("hashed session").len(), 64);
            let at = row["at"].as_str().expect("UTC timestamp");
            assert!(
                at.ends_with('Z') && at.len() == 20,
                "{at:?} is RFC 3339 UTC at seconds precision"
            );
            assert_eq!(recorder_row["path"], recorder.to_str().expect("fixture path"));
            if row["cause"] == "refused" {
                refused += 1;
                assert_eq!(recorder_row["size"], metadata.len());
                assert_eq!(
                    recorder_row["mtime"],
                    metadata.modified()?.duration_since(UNIX_EPOCH)?.as_secs()
                );
                let stored = row["stderr"].as_str().expect("the first diagnostic line");
                assert_eq!(stored, &diagnostic[..diagnostic.floor_char_boundary(256)]);
            } else {
                assert_eq!(row["cause"], "timeout");
                assert!(row["stderr"].is_null(), "the sleeping recorder wrote nothing");
            }
        }
        assert_eq!(refused, 1, "only the replacement refused");
        Ok(())
    }
}

#[test]
fn bridge_parser_and_typed_refusals_put_one_claim_on_stderr() -> gix_testtools::TestResult {
    let repo = gix_testtools::tempfile::tempdir()?;
    gix_testtools::git(repo.path(), "init")?;
    for (args, code, claim, text) in [
        (
            &["stash", "list"][..],
            2,
            "brit/capability-absent:stash",
            "run: git stash list",
        ),
        (
            &["submodules", "sync"][..],
            2,
            "brit/capability-absent:submodule.sync",
            "run: git submodule sync",
        ),
        (
            &["not-a-brit-command"][..],
            2,
            "brit/capability-absent:unknown",
            "unrecognized subcommand",
        ),
        (
            &["-c", "commit.gpgSign=true", "commit", "-m", "example"][..],
            1,
            "brit/capability-absent:commit.signing",
            "run: git commit -S",
        ),
    ] {
        let mut command = Command::new(env!("CARGO_BIN_EXE_brit"));
        gix_testtools::configure_git_environment(&mut command, repo.path());
        let output = command
            .current_dir(repo.path())
            .env_remove("EPR_FEEDBACK_ROOT")
            .args(args)
            .output()?;
        assert_eq!(output.status.code(), Some(code), "{args:?}: {output:?}");
        assert!(output.stdout.is_empty(), "{args:?} reports the refusal on stderr");
        let stderr = String::from_utf8(output.stderr)?;
        let suffix = format!(" [fb {claim}]");
        assert_eq!(stderr.matches(" [fb ").count(), 1, "{stderr}");
        assert!(
            stderr
                .lines()
                .any(|line| line.contains(text) && line.ends_with(&suffix)),
            "{stderr}"
        );
    }
    Ok(())
}

#[test]
fn feedback_claims_lists_the_same_ids_in_plain_and_json_output() -> gix_testtools::TestResult {
    let fixture = gix_testtools::tempfile::tempdir()?;
    let run = |args: &[&str]| -> gix_testtools::TestResult<Vec<u8>> {
        let mut command = Command::new(env!("CARGO_BIN_EXE_brit"));
        gix_testtools::configure_git_environment(&mut command, fixture.path());
        let out = command
            .current_dir(fixture.path())
            .env("EPR_FEEDBACK_ROOT", fixture.path())
            .env("PATH", fixture.path())
            .env("XDG_CACHE_HOME", fixture.path().join("cache"))
            .env("XDG_STATE_HOME", fixture.path().join("state"))
            .args(args)
            .output()?;
        assert!(out.status.success(), "{args:?}: {out:?}");
        assert!(out.stderr.is_empty(), "{args:?} only writes the requested output");
        Ok(out.stdout)
    };
    let plain = String::from_utf8(run(&["feedback", "claims"])?)?;
    let json = run(&["feedback", "claims", "--json"])?;
    assert_eq!(
        json,
        run(&["feedback", "claims", "--json"])?,
        "the export is byte-stable"
    );
    let manifest: serde_json::Value = serde_json::from_slice(&json)?;
    assert_eq!(manifest["cures"], serde_json::json!([]), "this release ships no cures");
    let rows = manifest["claims"]
        .as_array()
        .expect("the manifest contains a claims array");
    assert_eq!(
        rows.first().expect("status declaration"),
        &serde_json::json!({
            "id": "brit/command:status",
            "asserts": "Brit handled a command recorded as status.",
            "contradictedBy": "The invocation was not handled as this command.",
            "validityHorizon": "P14D",
            "supersedes": [],
            "clears": [],
            "polarity": "positive",
            "severity": "info",
            "weight": 0,
            "ceiling": "device",
            "retireWhen": null
        }),
        "the row uses the release wire schema"
    );
    let ids = rows
        .iter()
        .map(|row| row["id"].as_str().expect("claim ID"))
        .collect::<Vec<_>>();
    assert_eq!(
        plain.lines().collect::<Vec<_>>(),
        ids,
        "both outputs enumerate the same table"
    );
    assert!(
        ids.contains(&"brit/command:status"),
        "the command denominator is listed"
    );
    let raw = String::from_utf8(json)?;
    assert!(
        raw.starts_with(r#"{"claims":[{"id":"brit/command:status","asserts":"#),
        "field order starts at id, then asserts"
    );
    Ok(())
}
