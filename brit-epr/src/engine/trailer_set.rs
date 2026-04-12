//! `TrailerSet` — ordered, duplicate-aware key/value pairs from a commit
//! trailer block. Preserves insertion order for roundtrip-compatible
//! rendering.

use std::fmt;

/// A commit trailer block, parsed into ordered key/value pairs.
///
/// Order is preserved because the engine must be able to re-render the
/// trailer block byte-identically for signing and round-trip use cases.
/// Duplicate keys are allowed (e.g., multiple `Signed-off-by:` or
/// repeatable app-schema keys like `Built-By:`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TrailerSet {
    entries: Vec<(String, String)>,
}

impl TrailerSet {
    /// Create an empty set.
    pub fn new() -> Self {
        Self { entries: Vec::new() }
    }

    /// Append a trailer entry, preserving insertion order.
    pub fn push(&mut self, key: impl Into<String>, value: impl Into<String>) {
        self.entries.push((key.into(), value.into()));
    }

    /// Return the first value for a given key, or `None` if absent.
    pub fn get(&self, key: &str) -> Option<&str> {
        self.entries
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.as_str())
    }

    /// Return all values for a given key (preserves order).
    pub fn get_all(&self, key: &str) -> Vec<&str> {
        self.entries
            .iter()
            .filter(|(k, _)| k == key)
            .map(|(_, v)| v.as_str())
            .collect()
    }

    /// Iterate over all `(key, value)` pairs in insertion order.
    pub fn iter(&self) -> impl Iterator<Item = (&str, &str)> {
        self.entries.iter().map(|(k, v)| (k.as_str(), v.as_str()))
    }

    /// Number of entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// True when there are no entries.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

impl fmt::Display for TrailerSet {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (k, v) in &self.entries {
            writeln!(f, "{k}: {v}")?;
        }
        Ok(())
    }
}
