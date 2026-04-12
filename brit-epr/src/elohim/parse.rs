//! `parse_pillar_trailers` — convenience function that projects a
//! `TrailerSet` into the strongly-typed `PillarTrailers` view.

use crate::elohim::pillar_trailers::{PillarTrailers, TrailerKey};
use crate::engine::parse_trailer_block;

/// Parse pillar trailers from a commit body.
///
/// Pure function: no I/O beyond reading the body slice. Unknown trailers
/// (anything outside the six reserved pillar keys) are silently skipped —
/// a commit may carry `Signed-off-by:`, `Co-Authored-By:`, etc., alongside
/// the pillar trailers.
///
/// Permissive: malformed values in `*_Node:` trailers are accepted as raw
/// strings. Strict validation is done by `validate_pillar_trailers`.
pub fn parse_pillar_trailers(body: &[u8]) -> PillarTrailers {
    let set = parse_trailer_block(body);
    let mut out = PillarTrailers::default();

    for (key, value) in set.iter() {
        for pillar in TrailerKey::all() {
            if key == pillar.summary_token() {
                match pillar {
                    TrailerKey::Lamad => out.lamad = Some(value.to_string()),
                    TrailerKey::Shefa => out.shefa = Some(value.to_string()),
                    TrailerKey::Qahal => out.qahal = Some(value.to_string()),
                }
            } else if key == pillar.node_token() {
                match pillar {
                    TrailerKey::Lamad => out.lamad_node = Some(value.to_string()),
                    TrailerKey::Shefa => out.shefa_node = Some(value.to_string()),
                    TrailerKey::Qahal => out.qahal_node = Some(value.to_string()),
                }
            }
        }
    }

    out
}
