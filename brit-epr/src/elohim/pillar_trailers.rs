//! Pillar trailer types — the strongly-typed view the elohim app schema
//! uses to represent the three pillars plus their linked-node CID slots.

/// Which of the three pillars a trailer belongs to.
///
/// The elohim protocol pillars:
///
/// - **Lamad** (לָמַד, "to learn") — knowledge positioning.
/// - **Shefa** (שֶׁפַע, "abundance") — economic positioning.
/// - **Qahal** (קָהָל, "assembly") — governance positioning.
///
/// Each pillar has two trailer forms: a canonical summary (e.g., `Lamad:`)
/// and a linked-node CID reference (e.g., `Lamad-Node:`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TrailerKey {
    /// Knowledge-layer trailer.
    Lamad,
    /// Economic-layer trailer.
    Shefa,
    /// Governance-layer trailer.
    Qahal,
}

impl TrailerKey {
    /// The RFC-822 token name for the canonical-summary trailer.
    pub fn summary_token(self) -> &'static str {
        match self {
            TrailerKey::Lamad => "Lamad",
            TrailerKey::Shefa => "Shefa",
            TrailerKey::Qahal => "Qahal",
        }
    }

    /// The RFC-822 token name for the linked-node CID trailer.
    pub fn node_token(self) -> &'static str {
        match self {
            TrailerKey::Lamad => "Lamad-Node",
            TrailerKey::Shefa => "Shefa-Node",
            TrailerKey::Qahal => "Qahal-Node",
        }
    }

    /// All three pillars, in canonical order.
    pub fn all() -> [TrailerKey; 3] {
        [TrailerKey::Lamad, TrailerKey::Shefa, TrailerKey::Qahal]
    }
}

/// Pillar trailers extracted from a commit body and projected into the
/// typed view the elohim app schema uses.
///
/// Each `*_node` field holds the raw CID *string* — Phase 1 does not parse
/// the string into a typed `Cid`, does not resolve it, and does not check
/// the target's type. The parser is permissive; strict CID validation and
/// resolution arrive in Phase 2.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PillarTrailers {
    /// Canonical summary value of the `Lamad:` trailer, trimmed.
    pub lamad: Option<String>,
    /// Canonical summary value of the `Shefa:` trailer, trimmed.
    pub shefa: Option<String>,
    /// Canonical summary value of the `Qahal:` trailer, trimmed.
    pub qahal: Option<String>,

    /// Raw CID string from a `Lamad-Node:` trailer, if present. Phase 1
    /// does not parse or resolve this.
    pub lamad_node: Option<String>,
    /// Raw CID string from a `Shefa-Node:` trailer, if present.
    pub shefa_node: Option<String>,
    /// Raw CID string from a `Qahal-Node:` trailer, if present.
    pub qahal_node: Option<String>,
}
