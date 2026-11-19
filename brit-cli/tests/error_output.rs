//! A failed brit command ends its stderr with the error, never a stack frame, whatever
//! `RUST_BACKTRACE` says; a backtrace is printed only on `BRIT_BACKTRACE`, and before the error.

use std::process::{Command, Output};

fn run(program: &str, dir: &std::path::Path, args: &[&str], brit_backtrace: bool) -> Output {
    let mut command = Command::new(program);
    gix_testtools::configure_git_environment(&mut command, dir);
    command
        .env("RUST_BACKTRACE", "1")
        .env("RUST_LIB_BACKTRACE", "1")
        .env_remove("BRIT_BACKTRACE");
    if brit_backtrace {
        command.env("BRIT_BACKTRACE", "1");
    }
    command.args(args).current_dir(dir).output().expect("binary runs")
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

fn last_line(output: &Output) -> String {
    stderr(output)
        .lines()
        .rev()
        .find(|line| !line.trim().is_empty())
        .unwrap_or_default()
        .to_string()
}

/// A brit command that fails in a directory that is not a repository.
fn failing_commands() -> Vec<(&'static str, Vec<&'static str>)> {
    vec![
        (env!("CARGO_BIN_EXE_brit"), vec!["log", "-n", "1"]),
        (
            env!("CARGO_BIN_EXE_brit-rebase-retained"),
            vec!["inspect", "--copy", "missing"],
        ),
    ]
}

#[test]
fn failures_end_with_the_error_and_print_no_backtrace_under_rust_backtrace() {
    let dir = gix_testtools::tempfile::TempDir::new().expect("tempdir");
    for (program, args) in failing_commands() {
        let output = run(program, dir.path(), &args, false);
        let text = stderr(&output);
        assert!(!output.status.success(), "{program} {args:?} fails: {text}");
        assert!(
            !text.contains("Stack backtrace") && !text.contains("stack backtrace"),
            "{program} {args:?}: no backtrace without BRIT_BACKTRACE: {text}"
        );
        assert!(
            text.starts_with("Error: "),
            "{program} {args:?}: the error leads: {text}"
        );
        assert!(
            !last_line(&output).contains("<unknown>") && !last_line(&output).contains("__libc"),
            "{program} {args:?}: the last line is part of the error: {text}"
        );
    }
}

/// `brit log` in `dir`, with discovery stopped at the temporary directory so the chain does not depend on what lies
/// above it.
fn brit_log(dir: &std::path::Path, args: &[&str]) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_brit"));
    gix_testtools::configure_git_environment(&mut command, dir);
    command
        .arg("log")
        .args(args)
        .env("GIT_CEILING_DIRECTORIES", dir.parent().expect("tempdir has a parent"))
        .env("RUST_BACKTRACE", "1")
        .env("RUST_LIB_BACKTRACE", "1")
        .env_remove("BRIT_BACKTRACE")
        .current_dir(dir)
        .output()
        .expect("brit runs")
}

#[test]
fn the_last_line_is_the_deepest_message_with_its_class_folded_in() {
    // gix classifies "not found" with a class-only marker as the deepest source: it is folded into the message it
    // classifies instead of ending the output as a bare `NotFound`.
    let outside = gix_testtools::tempfile::TempDir::new().expect("tempdir");
    let output = brit_log(outside.path(), &["-n", "1"]);
    let text = stderr(&output);
    assert!(
        text.starts_with("Error: Could not find a git repository") && text.trim_end().ends_with("(NotFound)"),
        "one line, classified: {text}"
    );
    assert!(!text.contains("Caused by:"), "the marker was the only cause: {text}");

    let repo = gix_testtools::tempfile::TempDir::new().expect("tempdir");
    gix_testtools::git(repo.path(), "init").expect("git init");
    gix_testtools::git(repo.path(), "commit --allow-empty -m one").expect("git commit");
    let output = brit_log(repo.path(), &["no-such-rev"]);
    let text = stderr(&output);
    assert_eq!(
        last_line(&output),
        "    Reference \"no-such-rev\" could not be found (NotFound)",
        "the last line is the deepest real message: {text}"
    );
}

#[test]
fn brit_backtrace_prints_the_backtrace_before_the_error() {
    let dir = gix_testtools::tempfile::TempDir::new().expect("tempdir");
    let without = run(env!("CARGO_BIN_EXE_brit"), dir.path(), &["log", "-n", "1"], false);
    let with = run(env!("CARGO_BIN_EXE_brit"), dir.path(), &["log", "-n", "1"], true);
    let text = stderr(&with);
    assert!(text.contains("Stack backtrace:"), "BRIT_BACKTRACE asks for it: {text}");
    assert_eq!(
        last_line(&with),
        last_line(&without),
        "the error stays last even with a backtrace: {text}"
    );
}
