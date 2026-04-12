//! `parse_trailer_block` — extract a commit's RFC-822-style trailer block
//! into a `TrailerSet`. Wraps `gix_object::commit::message::BodyRef::trailers()`.

use gix_object::bstr::ByteSlice;
use gix_object::commit::message::BodyRef;

use crate::engine::TrailerSet;

/// Parse a commit body's bytes into a `TrailerSet`.
///
/// The body is the message *after* the commit headers (author, committer,
/// tree, parent lines) — i.e., what gitoxide calls "the body" of a commit.
/// This function extracts the final trailing block of `Key: value` lines
/// (if any) and records each as an entry in a `TrailerSet`, preserving
/// insertion order.
///
/// Returns an empty `TrailerSet` if the body has no trailer block.
pub fn parse_trailer_block(body: &[u8]) -> TrailerSet {
    let body_ref = BodyRef::from_bytes(body);
    let mut set = TrailerSet::new();

    for trailer in body_ref.trailers() {
        let key = String::from_utf8_lossy(trailer.token.as_bytes()).into_owned();
        let value = String::from_utf8_lossy(trailer.value.as_bytes()).into_owned();
        set.push(key, value);
    }

    set
}
