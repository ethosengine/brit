use cli_test_page::coverage::BinaryCoverage;
use cli_test_page::format::{format_test_page, BinarySection, SubcommandCapture};

#[test]
fn renders_coverage_summary_table() {
    let coverage = vec![BinaryCoverage {
        binary: "brit".into(),
        covered: 5,
        total: 5,
        uncovered: vec![],
    }];
    let sections: Vec<BinarySection> = vec![];
    let md = format_test_page(&coverage, &sections);
    assert!(md.contains("# Brit CLI Test Page"));
    assert!(md.contains("## Coverage"));
    assert!(md.contains("brit"));
    assert!(md.contains("5"));
    assert!(md.contains("100%"));
}

#[test]
fn renders_subcommand_capture_with_help_invocation_output() {
    let coverage = vec![];
    let sections = vec![BinarySection {
        binary: "brit".into(),
        captures: vec![SubcommandCapture {
            subcommand_path: vec!["brit".into(), "log".into()],
            help: "Print all commits".into(),
            invocation: "brit log".into(),
            output: "abc1234 init\n".into(),
        }],
    }];
    let md = format_test_page(&coverage, &sections);
    assert!(md.contains("### brit log"));
    assert!(md.contains("**Help:** Print all commits"));
    assert!(md.contains("```sh\nbrit log\n```"));
    assert!(md.contains("```\nabc1234 init\n"));
}
