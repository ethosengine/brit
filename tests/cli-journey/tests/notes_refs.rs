use std::{
    fs,
    path::Path,
    process::{Command, Output},
};

use anyhow::{ensure, Result};
use cli_journey::support::{
    runner::{brit_bin, BritInvocation, Capture},
    test_repo::TestRepo,
};

const DEVICE: &str = "refs/notes/brit/devices/device-a/build/x";
const FROZEN: &str = "refs/notes/brit/build/frozen";

fn git(repo: &Path, args: &[&str]) -> Result<Output> {
    Ok(gix_testtools::git_command(repo).args(args).output()?)
}

fn git_ok(repo: &Path, args: &[&str]) -> Result<Vec<u8>> {
    let output = git(repo, args)?;
    ensure!(
        output.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    Ok(output.stdout)
}

fn brit(repo: &Path, args: &[&str]) -> Result<Capture> {
    BritInvocation::new(brit_bin().expect("required binary"))
        .args(args.iter().copied())
        .current_dir(repo)
        .run()
}

fn bare_remote(repos: &[&TestRepo]) -> Result<tempfile::TempDir> {
    let remote = tempfile::tempdir()?;
    git_ok(remote.path(), &["init", "--bare", "-q"])?;
    for repo in repos {
        git_ok(
            repo.path(),
            &["remote", "add", "origin", remote.path().to_str().expect("temp path")],
        )?;
    }
    Ok(remote)
}

fn note(repo: &TestRepo, reference: &str, body: &str) -> Result<()> {
    git_ok(repo.path(), &["notes", "--ref", reference, "add", "-f", "-m", body])?;
    Ok(())
}

#[test]
fn notes_transfer_help_describes_the_single_refspec() -> gix_testtools::TestResult {
    let fixture = tempfile::tempdir()?;
    for command in ["fetch", "push"] {
        let help = brit(fixture.path(), &[command, "--help"])?;
        assert!(help.status.success(), "{command} help failed: {}", help.stderr);
        assert!(
            help.stdout.contains("refs/notes/X:refs/notes/X") && help.stdout.contains("fast-forward"),
            "{command} help must describe the notes refspec and its fast-forward limit\n{}",
            help.stdout
        );
    }
    Ok(())
}

#[test]
fn round_trips_one_notes_ref() -> Result<()> {
    let a = TestRepo::new("notes-writer")?;
    let b = TestRepo::new("notes-reader")?;
    let remote = bare_remote(&[&a, &b])?;
    note(&a, FROZEN, "frozen receipt")?;
    git_ok(a.path(), &["push", "origin", &format!("{FROZEN}:{FROZEN}")])?;
    let frozen_path = remote.path().join(FROZEN);
    let frozen = fs::read(&frozen_path)?;
    note(&a, FROZEN, "local change that must stay local")?;
    let spec = format!("{DEVICE}:{DEVICE}");
    for body in [
        "first receipt",
        "second receipt

with a second paragraph",
    ] {
        note(&a, DEVICE, body)?;
        let pushed = brit(a.path(), &["push", "origin", &spec])?;
        ensure!(pushed.status.success(), "notes push: {}", pushed.stderr);
        assert_eq!(fs::read(&frozen_path)?, frozen, "only the device ref was pushed");
        let fetched = brit(b.path(), &["fetch", "origin", &spec])?;
        ensure!(fetched.status.success(), "notes fetch: {}", fetched.stderr);
        assert_eq!(fs::read(&frozen_path)?, frozen, "fetch leaves the remote ref untouched");
        assert_eq!(
            git_ok(b.path(), &["for-each-ref", "--format=%(refname)", "refs/notes/"])?,
            format!("{DEVICE}\n").as_bytes()
        );
        let expected = git_ok(a.path(), &["notes", "--ref", DEVICE, "show", "HEAD"])?;
        let shown = brit(b.path(), &["notes", "show", "--ref", DEVICE])?;
        ensure!(shown.status.success(), "notes show: {}", shown.stderr);
        assert_eq!(
            shown.stdout.as_bytes(),
            expected,
            "the note body matches Git byte for byte"
        );
        let shown = brit(b.path(), &["notes", "--ref", DEVICE, "show", &b.head_id()?])?;
        ensure!(
            shown.status.success(),
            "notes show with an explicit object: {}",
            shown.stderr
        );
        assert_eq!(
            shown.stdout.as_bytes(),
            expected,
            "explicit objects select the same notes"
        );
    }
    Ok(())
}

#[test]
fn non_fast_forward_writes_nothing() -> Result<()> {
    let a = TestRepo::new("notes-diverged-a")?;
    let b = TestRepo::new("notes-diverged-b")?;
    let remote = bare_remote(&[&a, &b])?;
    let spec = format!("{DEVICE}:{DEVICE}");
    note(&a, DEVICE, "base")?;
    git_ok(a.path(), &["push", "origin", &spec])?;
    git_ok(b.path(), &["fetch", "origin", &spec])?;
    note(&a, DEVICE, "remote child")?;
    note(&b, DEVICE, "local child")?;
    git_ok(a.path(), &["push", "origin", &spec])?;

    let local_path = b.path().join(".git").join(DEVICE);
    let remote_path = remote.path().join(DEVICE);
    let local_before = fs::read(&local_path)?;
    let remote_before = fs::read(&remote_path)?;
    let fetch_head = fs::read(b.path().join(".git/FETCH_HEAD"))?;
    let local_objects = git_ok(b.path(), &["count-objects", "-v"])?;
    let remote_objects = git_ok(remote.path(), &["count-objects", "-v"])?;
    for verb in ["push", "fetch"] {
        let output = brit(b.path(), &[verb, "origin", &spec])?;
        ensure!(!output.status.success(), "{verb} must refuse divergent notes history");
        ensure!(output.stderr.contains("fast-forward"), "{verb}: {}", output.stderr);
        assert_eq!(
            fs::read(&local_path)?,
            local_before,
            "{verb} leaves the local ref intact"
        );
        assert_eq!(
            fs::read(&remote_path)?,
            remote_before,
            "{verb} leaves the remote ref intact"
        );
        assert_eq!(
            fs::read(b.path().join(".git/FETCH_HEAD"))?,
            fetch_head,
            "{verb} leaves FETCH_HEAD intact"
        );
        assert_eq!(
            git_ok(b.path(), &["count-objects", "-v"])?,
            local_objects,
            "{verb} leaves local objects intact"
        );
        assert_eq!(
            git_ok(remote.path(), &["count-objects", "-v"])?,
            remote_objects,
            "{verb} leaves remote objects intact"
        );
    }
    Ok(())
}

#[test]
fn wildcard_notes_refspecs_are_refused() -> Result<()> {
    let repo = TestRepo::new("notes-wildcard")?;
    let _remote = bare_remote(&[&repo])?;
    for verb in ["push", "fetch"] {
        for spec in ["refs/notes/*", "refs/notes/brit/build/*:refs/notes/brit/build/*"] {
            let output = brit(repo.path(), &[verb, "origin", spec])?;
            ensure!(!output.status.success(), "{verb} must refuse {spec}");
            ensure!(
                output.stderr.contains("refs/notes/X:refs/notes/X"),
                "{verb}: {}",
                output.stderr
            );
        }
    }
    Ok(())
}

#[test]
fn notes_transfers_refuse_force_and_multiple_refs() -> Result<()> {
    let repo = TestRepo::new("notes-single-ref")?;
    let _remote = bare_remote(&[&repo])?;
    let spec = format!("{DEVICE}:{DEVICE}");
    let forced = format!("+{spec}");
    let frozen = format!("{FROZEN}:{FROZEN}");
    for verb in ["push", "fetch"] {
        for args in [
            vec![verb, "origin", forced.as_str()],
            vec![verb, "origin", spec.as_str(), frozen.as_str()],
            vec![verb, "origin", spec.as_str(), "refs/heads/main:refs/heads/main"],
        ] {
            let output = brit(repo.path(), &args)?;
            ensure!(!output.status.success(), "{args:?} must be refused");
            ensure!(
                output.stderr.contains("refs/notes/X:refs/notes/X"),
                "{args:?}: {}",
                output.stderr
            );
        }
    }
    let output = brit(repo.path(), &["push", "--force", "origin", &spec])?;
    ensure!(!output.status.success(), "a notes push must refuse --force");
    ensure!(output.stderr.contains("fast-forward only"), "{}", output.stderr);
    Ok(())
}

#[test]
fn show_preserves_binary_notes_and_uses_the_configured_default() -> Result<()> {
    let repo = TestRepo::new("notes-bytes")?;
    let notes_bytes = b"build receipt\0\xff";
    fs::write(repo.path().join("body"), notes_bytes)?;
    let note_blob = git_ok(repo.path(), &["hash-object", "-w", "body"])?;
    let note_blob = std::str::from_utf8(&note_blob)?.trim();
    git_ok(repo.path(), &["config", "core.notesRef", DEVICE])?;
    git_ok(repo.path(), &["notes", "add", "-C", note_blob])?;
    let expected = git_ok(repo.path(), &["notes", "show", "HEAD"])?;
    assert_eq!(
        expected, notes_bytes,
        "the fixture includes binary bytes and no final newline"
    );

    // BritInvocation captures UTF-8 text. Use its Git isolation setup for raw stdout.
    let config = tempfile::tempdir()?;
    let mut command = Command::new(brit_bin().expect("required binary"));
    let shown = gix_testtools::configure_git_environment(&mut command, repo.path())
        .env("GIT_CONFIG_GLOBAL", config.path().join("missing.gitconfig"))
        .current_dir(repo.path())
        .args(["notes", "show"])
        .output()?;
    ensure!(
        shown.status.success(),
        "notes show: {}",
        String::from_utf8_lossy(&shown.stderr)
    );
    assert_eq!(shown.stdout, expected, "binary note bytes match Git");
    Ok(())
}

#[test]
fn missing_note_matches_git_and_other_verbs_stay_bridged() -> Result<()> {
    let repo = TestRepo::new("notes-bridge")?;
    let expected = git(repo.path(), &["notes", "--ref", DEVICE, "show", "HEAD"])?;
    ensure!(!expected.status.success(), "Git reports the missing note");
    let missing = brit(repo.path(), &["notes", "show", "--ref", DEVICE, "HEAD"])?;
    assert_eq!(missing.status.code(), expected.status.code(), "missing-note exit code");
    assert_eq!(missing.stderr.as_bytes(), expected.stderr, "missing-note diagnostic");
    assert!(missing.stdout.is_empty(), "missing note produces no body");
    for (args, command) in [
        (&["notes", "add", "-m", "a receipt"][..], "git notes add -m 'a receipt'"),
        (&["notes", "list"][..], "git notes list"),
        (
            &["notes", "--ref", DEVICE, "list"][..],
            "git notes --ref refs/notes/brit/devices/device-a/build/x list",
        ),
    ] {
        let output = brit(repo.path(), args)?;
        assert_eq!(output.status.code(), Some(2), "{command} remains bridged");
        let bridge = format!("brit: notes is a Git bridge operation; run: {command}\n");
        assert_eq!(output.stderr, bridge, "{command} prints the bridge line on stderr");
        assert!(output.stdout.is_empty(), "{command} leaves stdout empty");
    }
    Ok(())
}
