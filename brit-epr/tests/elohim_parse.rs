//! Integration tests for elohim pillar trailer parsing.

use brit_epr::{parse_pillar_trailers, PillarTrailers};

fn fixture(name: &str) -> Vec<u8> {
    let path = format!("tests/fixtures/{}", name);
    std::fs::read(&path).unwrap_or_else(|e| panic!("failed to read fixture {path}: {e}"))
}

#[test]
fn happy_path_all_three_pillars_parse() {
    let body = fixture("happy_all_three_pillars.txt");
    let trailers: PillarTrailers = parse_pillar_trailers(&body);

    assert_eq!(
        trailers.lamad.as_deref(),
        Some("introduces pillar trailer model; first testable EPR primitive")
    );
    assert_eq!(
        trailers.shefa.as_deref(),
        Some("stewardship by @matthew; contributor credit via git author")
    );
    assert_eq!(
        trailers.qahal.as_deref(),
        Some("no governance review required for scaffolding")
    );
    assert_eq!(trailers.lamad_node, None);
    assert_eq!(trailers.shefa_node, None);
    assert_eq!(trailers.qahal_node, None);
}

#[test]
fn missing_qahal_parses_partially() {
    let body = fixture("missing_qahal.txt");
    let trailers = parse_pillar_trailers(&body);

    assert_eq!(trailers.lamad.as_deref(), Some("no knowledge change — pure refactor"));
    assert_eq!(trailers.shefa.as_deref(), Some("no value flow — maintenance work"));
    assert_eq!(trailers.qahal, None);
}

#[test]
fn malformed_shefa_node_stored_as_raw_string() {
    let body = fixture("malformed_shefa_node.txt");
    let trailers = parse_pillar_trailers(&body);

    assert_eq!(trailers.lamad.as_deref(), Some("teaches the permissive parser behavior"));
    assert_eq!(trailers.shefa.as_deref(), Some("value summary is fine"));
    assert_eq!(trailers.qahal.as_deref(), Some("governance review complete"));

    // Phase 1 is permissive — stores raw string without parsing.
    // Phase 2 will add typed CID parsing and reject malformed values.
    assert_eq!(trailers.shefa_node.as_deref(), Some("not-a-valid-cid-at-all"));
}
