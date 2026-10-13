#![forbid(unsafe_code)]
//! Pure todo validation and original-to-result mappings. Repository mutation,
//! editors, object creation and durable receipts belong to the caller.

use std::collections::{BTreeMap, BTreeSet};

/// One supported replay action.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Command {
    /// Apply a commit unchanged except for parent and committer.
    Pick,
    /// Apply a commit with an edited message.
    Reword,
    /// Stop for explicit amendment or splitting.
    Edit,
    /// Combine with the preceding retained group, editing its message.
    Squash,
    /// Combine with the preceding retained group, retaining its message.
    Fixup,
    /// Consume an original without producing a commit.
    Drop,
    /// Stop before the next action, without consuming an original.
    Break,
}

/// A validated command and its full pinned original OID, absent only for break.
#[derive(Clone, Debug, Eq, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(deny_unknown_fields))]
pub struct Entry {
    /// Replay action.
    pub command: Command,
    /// Full lowercase hexadecimal commit identity.
    pub commit: Option<String>,
}

/// A rejected input. Line zero denotes an invariant outside a single todo line.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Error {
    /// One-based input line, or zero for selection/mapping errors.
    pub line: usize,
    /// The violated contract, without reproducing arbitrary input bytes.
    pub message: &'static str,
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "sequencer line {}: {}", self.line, self.message)
    }
}
impl std::error::Error for Error {}
fn error(line: usize, message: &'static str) -> Error {
    Error { line, message }
}
fn valid_oid(id: &str) -> bool {
    matches!(id.len(), 40 | 64) && id.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
fn selection(ids: &[String]) -> Result<BTreeSet<&str>, Error> {
    let mut out = BTreeSet::new();
    for id in ids {
        if !valid_oid(id) || ids.first().is_some_and(|first| first.len() != id.len()) {
            return Err(error(0, "selection requires full OIDs of one hash algorithm"));
        }
        if !out.insert(id.as_str()) {
            return Err(error(0, "duplicate pinned original"));
        }
    }
    Ok(out)
}

/// Accepted todo bytes and parsed commands. Display subjects are never decoded.
#[derive(Clone, Debug, Eq, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(deny_unknown_fields))]
pub struct Todo {
    /// Commands in execution order, including breaks.
    pub entries: Vec<Entry>,
    /// Exact accepted bytes, including comments and arbitrary subject bytes.
    pub bytes: Vec<u8>,
}
impl Todo {
    /// Parse a full todo, accounting for every selected original exactly once.
    pub fn parse(bytes: &[u8], candidates: &[String]) -> Result<Self, Error> {
        Self::parse_suffix(bytes, candidates, false)
    }

    /// Parse the unconsumed suffix with an explicitly retained predecessor group.
    pub fn parse_suffix(bytes: &[u8], candidates: &[String], mut has_group: bool) -> Result<Self, Error> {
        let pinned = selection(candidates)?;
        let mut seen = BTreeSet::new();
        let mut entries = Vec::new();
        for (number, line) in bytes.split(|b| *b == b'\n').enumerate() {
            let line = line.trim_ascii();
            if line.is_empty() || line.starts_with(b"#") {
                continue;
            }
            let mut tokens = line.split(u8::is_ascii_whitespace).filter(|part| !part.is_empty());
            let command = match tokens.next().expect("nonempty line") {
                b"pick" | b"p" => Command::Pick,
                b"reword" | b"r" => Command::Reword,
                b"edit" | b"e" => Command::Edit,
                b"squash" | b"s" => Command::Squash,
                b"fixup" | b"f" => Command::Fixup,
                b"drop" | b"d" => Command::Drop,
                b"break" | b"b" => Command::Break,
                _ => return Err(error(number + 1, "unknown command or flags")),
            };
            if command == Command::Break {
                if tokens.next().is_some() {
                    return Err(error(number + 1, "break takes no OID or arguments"));
                }
                entries.push(Entry { command, commit: None });
                continue;
            }
            let id = tokens.next().and_then(|id| std::str::from_utf8(id).ok());
            let Some(id) = id.filter(|id| pinned.contains(id)) else {
                return Err(error(number + 1, "expected a full pinned original OID"));
            };
            if !seen.insert(id) {
                return Err(error(number + 1, "original occurs more than once"));
            }
            match command {
                Command::Pick | Command::Reword | Command::Edit => has_group = true,
                Command::Squash | Command::Fixup if !has_group => {
                    return Err(error(number + 1, "squash/fixup requires a retained preceding group"));
                }
                _ => {}
            }
            entries.push(Entry {
                command,
                commit: Some(id.into()),
            });
        }
        if seen != pinned {
            return Err(error(
                0,
                "every unconsumed original must occur exactly once, including drops",
            ));
        }
        Ok(Self {
            entries,
            bytes: bytes.to_vec(),
        })
    }

    /// Accept only a new suffix; completed command bytes and entries stay fixed.
    pub fn replace_suffix(&self, completed: usize, bytes: &[u8], has_group: bool) -> Result<Self, Error> {
        if completed > self.entries.len() {
            return Err(error(0, "completed cursor exceeds accepted todo"));
        }
        let remaining = self.entries[completed..]
            .iter()
            .filter_map(|e| e.commit.clone())
            .collect::<Vec<_>>();
        let suffix = Self::parse_suffix(bytes, &remaining, has_group)?;
        let mut prefix_end = 0;
        let mut consumed = 0;
        if completed != 0 {
            for line in self.bytes.split_inclusive(|b| *b == b'\n') {
                prefix_end += line.len();
                let trimmed = line.trim_ascii();
                if !trimmed.is_empty() && !trimmed.starts_with(b"#") {
                    consumed += 1;
                    if consumed == completed {
                        break;
                    }
                }
            }
            if consumed != completed {
                return Err(error(0, "accepted bytes disagree with completed cursor"));
            }
        }
        let mut accepted = self.bytes[..prefix_end].to_vec();
        if !accepted.is_empty() && !accepted.ends_with(b"\n") && !bytes.is_empty() {
            accepted.push(b'\n');
        }
        accepted.extend_from_slice(bytes);
        let mut entries = self.entries[..completed].to_vec();
        entries.extend(suffix.entries);
        Ok(Self {
            entries,
            bytes: accepted,
        })
    }
}

/// Rewrites in original selection order, independent of todo execution order.
#[derive(Clone, Debug, Eq, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(deny_unknown_fields))]
pub struct MappingLedger {
    originals: Vec<String>,
    mappings: BTreeMap<String, Vec<String>>,
    superseded: Vec<String>,
}
impl MappingLedger {
    /// Start with no consumed originals.
    pub fn new(originals: Vec<String>) -> Result<Self, Error> {
        selection(&originals)?;
        Ok(Self {
            originals,
            mappings: BTreeMap::new(),
            superseded: Vec::new(),
        })
    }
    fn validate_results(&self, results: &[String]) -> Result<(), Error> {
        selection(results)?;
        if results
            .iter()
            .any(|id| self.originals.first().is_none_or(|original| original.len() != id.len()))
        {
            return Err(error(0, "result uses a different hash algorithm"));
        }
        Ok(())
    }
    /// Consume one original, with zero results for drop/skip or an ordered split.
    pub fn record(&mut self, original: &str, results: &[String]) -> Result<(), Error> {
        if !self.originals.iter().any(|id| id == original) || self.mappings.contains_key(original) {
            return Err(error(0, "unknown or already consumed original"));
        }
        self.validate_results(results)?;
        self.mappings.insert(original.into(), results.to_vec());
        Ok(())
    }
    /// Replace a complete retained group, optionally consuming its next input.
    /// Empty results implement discarding an edit group; old outputs stay recorded.
    pub fn replace_group(&mut self, originals: &[String], results: &[String]) -> Result<(), Error> {
        let members = selection(originals)?;
        if members.is_empty() || originals.iter().any(|id| !self.originals.contains(id)) {
            return Err(error(0, "group contains no originals or an unknown original"));
        }
        self.validate_results(results)?;
        let existing = originals
            .iter()
            .filter_map(|id| self.mappings.get(id))
            .collect::<Vec<_>>();
        let Some(prior) = existing.first().copied().filter(|prior| !prior.is_empty()) else {
            return Err(error(0, "replacement requires a retained group"));
        };
        if originals.len() - existing.len() > 1
            || existing.iter().any(|mapping| *mapping != prior)
            || self
                .mappings
                .iter()
                .any(|(id, mapping)| mapping == prior && !members.contains(id.as_str()))
        {
            return Err(error(
                0,
                "replacement must include every constituent of one retained group",
            ));
        }
        for id in prior {
            if !results.contains(id) && !self.superseded.contains(id) {
                self.superseded.push(id.clone());
            }
        }
        for id in originals {
            self.mappings.insert(id.clone(), results.to_vec());
        }
        Ok(())
    }
    /// Emit post-rewrite edges in original order, split edges in chain order.
    pub fn edges(&self) -> Vec<(String, String)> {
        self.originals
            .iter()
            .flat_map(|old| {
                self.mappings
                    .get(old)
                    .into_iter()
                    .flatten()
                    .map(move |new| (old.clone(), new.clone()))
            })
            .collect()
    }
    /// Authored outputs superseded by an explicit group transition.
    pub fn superseded(&self) -> &[String] {
        &self.superseded
    }
}
