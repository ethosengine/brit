use cli_test_page::diff::{render_unified_diff, has_diff};

#[test]
fn identical_strings_have_no_diff() {
    let a = "hello\nworld\n";
    let b = "hello\nworld\n";
    assert!(!has_diff(a, b));
}

#[test]
fn different_strings_have_diff() {
    let a = "hello\nworld\n";
    let b = "hello\nWORLD\n";
    assert!(has_diff(a, b));
}

#[test]
fn render_unified_diff_includes_changed_lines() {
    let a = "alpha\nbeta\n";
    let b = "alpha\nGAMMA\n";
    let d = render_unified_diff(a, b);
    assert!(d.contains("beta"), "diff: {d}");
    assert!(d.contains("GAMMA"), "diff: {d}");
}
