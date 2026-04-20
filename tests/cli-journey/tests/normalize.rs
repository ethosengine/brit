//! Self-tests for the Normalizer.

use cli_journey::support::normalize::Normalizer;

#[test]
fn strips_ansi_escape_codes() {
    let n = Normalizer::new();
    let red_text = "\x1b[31mhello\x1b[0m world";
    assert_eq!(n.normalize(red_text), "hello world");
}

#[test]
fn redacts_tempdir_paths() {
    let n = Normalizer::new();
    // POSIX-style tempdirs
    let s = n.normalize("/tmp/brit-test-xyz123/foo");
    assert_eq!(s, "<TMPDIR>/foo");
    // macOS-style
    let s = n.normalize("/var/folders/ab/cd1234/T/brit-test-xyz/foo");
    assert!(s.contains("<TMPDIR>"), "got: {s}");
}

#[test]
fn redacts_rfc3339_timestamps() {
    let n = Normalizer::new();
    let s = n.normalize("generated_at: 2026-04-19T15:30:45.123456789+00:00");
    assert!(s.contains("<TIMESTAMP>"), "got: {s}");
    assert!(!s.contains("2026-04-19T15"), "got: {s}");
}

#[test]
fn redacts_variable_git_shas() {
    let n = Normalizer::new();
    // SHAs not declared as stable get redacted
    let sha = "a".repeat(40);
    let s = n.normalize(&sha);
    assert_eq!(s, "<SHA>");
}

#[test]
fn preserves_stable_git_shas() {
    let mut n = Normalizer::new();
    let stable = "b".repeat(40);
    n.add_stable_sha(&stable);
    let s = n.normalize(&stable);
    assert_eq!(s, stable);
}

#[test]
fn redacts_short_git_shas() {
    let n = Normalizer::new();
    // 7-char abbreviated SHAs are common in `git log` output
    let s = n.normalize("commit abc1234 by author");
    assert_eq!(s, "commit <SHA> by author");
}
