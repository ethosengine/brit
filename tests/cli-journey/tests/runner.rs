//! Self-tests for the BritInvocation runner.

use cli_journey::support::runner::BritInvocation;

#[test]
fn invokes_echo_and_captures_stdout() {
    // Use `echo` as a stand-in for any binary — it's universally present
    let cap = BritInvocation::new("echo")
        .arg("hello world")
        .run()
        .expect("run");
    assert!(cap.status.success());
    assert_eq!(cap.stdout.trim(), "hello world");
}

#[test]
fn captures_stderr_and_exit_code() {
    // `false` returns exit 1 with no output
    let cap = BritInvocation::new("false").run().expect("run");
    assert!(!cap.status.success());
    assert_eq!(cap.status.code(), Some(1));
}

#[test]
fn applies_normalizer_to_output() {
    // Echo a tempdir-like path; expect normalization
    let cap = BritInvocation::new("echo")
        .arg("/tmp/brit-test-xyz/foo")
        .normalize(true)
        .run()
        .expect("run");
    assert_eq!(cap.stdout.trim(), "<TMPDIR>/foo");
}
