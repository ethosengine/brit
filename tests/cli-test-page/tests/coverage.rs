use cli_test_page::coverage::compute_coverage;
use cli_test_page::discover::SubcommandPath;
use std::collections::BTreeSet;

#[test]
fn full_coverage_when_all_paths_have_captures() {
    let universe: Vec<SubcommandPath> = vec![
        vec!["brit".into(), "log".into()],
        vec!["brit".into(), "status".into()],
    ];
    let captured: BTreeSet<SubcommandPath> = universe.iter().cloned().collect();
    let cov = compute_coverage("brit", &universe, &captured);
    assert_eq!(cov.covered, 2);
    assert_eq!(cov.total, 2);
    assert_eq!(cov.percent(), 100);
    assert!(cov.uncovered.is_empty());
}

#[test]
fn partial_coverage_lists_uncovered() {
    let universe: Vec<SubcommandPath> = vec![
        vec!["brit".into(), "log".into()],
        vec!["brit".into(), "status".into()],
        vec!["brit".into(), "blame".into()],
    ];
    let captured: BTreeSet<SubcommandPath> = vec![
        vec!["brit".into(), "log".into()],
    ]
    .into_iter()
    .collect();
    let cov = compute_coverage("brit", &universe, &captured);
    assert_eq!(cov.covered, 1);
    assert_eq!(cov.total, 3);
    assert_eq!(cov.percent(), 33);
    assert_eq!(cov.uncovered.len(), 2);
}

#[test]
fn zero_total_yields_100_percent_to_avoid_div_by_zero() {
    let universe: Vec<SubcommandPath> = vec![];
    let captured: BTreeSet<SubcommandPath> = BTreeSet::new();
    let cov = compute_coverage("empty-bin", &universe, &captured);
    assert_eq!(cov.percent(), 100);
}
