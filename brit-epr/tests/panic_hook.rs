//! The panic hook prints a short, honest report and leaves the panic's exit status alone.

use std::process::Command;

const CHILD: &str = "BRIT_EPR_PANIC_HOOK_CHILD";

/// The panicking half: only does anything when the parent set [`CHILD`].
#[test]
fn panics_with_the_hook_installed() {
    if std::env::var_os(CHILD).is_none() {
        return;
    }
    brit_epr::diagnostic::install_panic_hook("brit-test", "9.9.9");
    panic!("deliberate test panic");
}

#[test]
fn the_hook_names_the_binary_the_location_and_the_bug_and_keeps_status_101() {
    let output = Command::new(std::env::current_exe().expect("test binary"))
        .args([
            "--exact",
            "panics_with_the_hook_installed",
            "--nocapture",
            "--test-threads=1",
        ])
        .env(CHILD, "1")
        .env_remove("RUST_BACKTRACE")
        .output()
        .expect("child runs");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(output.status.code(), Some(101), "a panic keeps its status: {stderr}");
    assert!(
        stderr.contains("brit-test 9.9.9: internal error: panicked at "),
        "{stderr}"
    );
    assert!(stderr.contains("panic_hook.rs"), "names the panic location: {stderr}");
    assert!(stderr.contains("deliberate test panic"), "{stderr}");
    assert!(stderr.contains("This is a brit bug"), "{stderr}");
    assert!(
        !stderr.contains("thread '"),
        "the raw default report is replaced: {stderr}"
    );
}

#[test]
fn a_broken_pipe_is_recognised_by_type_through_the_cause_chain() {
    use brit_epr::diagnostic::{exit_for, is_broken_pipe};
    let pipe = std::io::Error::from(std::io::ErrorKind::BrokenPipe);
    let wrapped = Wrapper(pipe);
    assert!(is_broken_pipe(&wrapped));
    assert!(exit_for(&wrapped, true).is_some());
    let other = std::io::Error::other("BrokenPipe in the text only");
    assert!(!is_broken_pipe(&other));
    assert!(exit_for(&other, true).is_none());
}

#[derive(Debug)]
struct Wrapper(std::io::Error);

impl std::fmt::Display for Wrapper {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("could not write")
    }
}

impl std::error::Error for Wrapper {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.0)
    }
}
