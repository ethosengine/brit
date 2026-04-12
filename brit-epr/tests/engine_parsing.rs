//! Engine-level tests — trailer block extraction, no app-schema semantics.

use brit_epr::engine::{parse_trailer_block, TrailerSet};

#[test]
fn extracts_trailer_block_from_commit_body() {
    let body = b"\
Add pillar trailer parser

Wires gix-object into the covenant engine so trailer blocks can be
extracted into a schema-agnostic TrailerSet.

Signed-off-by: Matthew Dowell <matthew@ethosengine.com>
Lamad: introduces pillar trailer model
Shefa: stewardship by @matthew
Qahal: no governance review required
";

    let trailers: TrailerSet = parse_trailer_block(body);

    assert_eq!(trailers.len(), 4, "expected 4 trailers, got {}", trailers.len());
    assert_eq!(trailers.get("Signed-off-by"), Some("Matthew Dowell <matthew@ethosengine.com>"));
    assert_eq!(trailers.get("Lamad"), Some("introduces pillar trailer model"));
    assert_eq!(trailers.get("Shefa"), Some("stewardship by @matthew"));
    assert_eq!(trailers.get("Qahal"), Some("no governance review required"));
}

#[test]
fn empty_trailer_block_returns_empty_set() {
    let body = b"Commit with no trailers at all, just a body.";
    let trailers = parse_trailer_block(body);
    assert_eq!(trailers.len(), 0);
}
