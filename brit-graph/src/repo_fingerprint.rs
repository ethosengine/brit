//! Repo-aware fingerprint constructor (feature: `repo`).
//!
//! Builds a `ContentFingerprint` from file contents resolved through glob
//! patterns against a git tree at a specific commit. Reads blobs from the
//! tree, NOT the working tree — fingerprints are reproducible across machines
//! given the same commit and patterns.

use std::collections::{BTreeMap, VecDeque};
use std::sync::Arc;

use brit_epr::BritCid;
use gix::bstr::{BStr, BString, ByteSlice, ByteVec};
use gix::{error::ResultExt, objs::FindStream, utils::stream::Budget};
use globset::{Glob, GlobSet, GlobSetBuilder};
use regex_automata::{
    dfa::{dense, Automaton, StartKind},
    nfa::thompson,
    util::syntax,
    Anchored, Input,
};

use crate::fingerprint::{ContentFingerprint, FingerprintError};

impl ContentFingerprint {
    /// Compute a fingerprint by reading files from a git tree at a specific
    /// commit, matching against the given glob patterns.
    ///
    /// Files are read from the git tree (not the working tree) for
    /// reproducibility. Same commit + same patterns = same fingerprint,
    /// regardless of working-tree state.
    ///
    /// Regular and executable blobs that match a pattern are hashed by
    /// content. A gitlink (submodule entry) is an input when some pattern can
    /// match its path or a path beneath it: it is keyed by its path and the
    /// commit it pins, once per gitlink however many patterns reach it, and
    /// without reading the submodule's objects (they are often absent).
    /// Symlinks are skipped. Empty pattern set or no matching entries
    /// produces a stable empty-input fingerprint.
    pub fn from_repo_globs(
        repo: &gix::Repository,
        commit_id: gix::ObjectId,
        patterns: &[String],
    ) -> Result<Self, FingerprintError> {
        // Step A: build the GlobSet
        let mut builder = GlobSetBuilder::new();
        let mut globs = Vec::with_capacity(patterns.len());
        for pattern in patterns {
            let glob = Glob::new(pattern).map_err(|e| FingerprintError::InvalidGlob {
                pattern: pattern.clone(),
                message: e.to_string(),
            })?;
            builder.add(glob.clone());
            globs.push(glob);
        }
        let globset = builder.build().map_err(|e| FingerprintError::InvalidGlob {
            pattern: patterns.join(", "),
            message: e.to_string(),
        })?;

        // Step B: open the tree at the commit
        let object = repo
            .find_object(commit_id)
            .map_err(|e| FingerprintError::CommitResolve {
                commit: commit_id.to_hex().to_string(),
                message: e.to_string(),
            })?;
        let commit = object.try_into_commit().map_err(|e| FingerprintError::CommitResolve {
            commit: commit_id.to_hex().to_string(),
            message: format!("not a commit: {e}"),
        })?;
        let tree = commit.tree().map_err(|e| FingerprintError::TreeWalk(e.to_string()))?;

        // Step C: hash each matching blob, retaining only its path and CID;
        // record each reached gitlink with its pinned commit.
        let mut inputs = BTreeMap::new();
        let mut gitlinks = BTreeMap::new();
        let mut walk_errors: Vec<String> = Vec::new();

        let mut visitor = TreeCollector {
            repo,
            globset: &globset,
            reach: GitlinkReach::new(&globset, &globs),
            inputs: &mut inputs,
            gitlinks: &mut gitlinks,
            errors: &mut walk_errors,
            path: BString::default(),
            path_deque: VecDeque::new(),
            budget: None,
        };

        tree.traverse()
            .breadthfirst(&mut visitor)
            .map_err(|e| FingerprintError::TreeWalk(e.to_string()))?;

        if !walk_errors.is_empty() {
            return Err(FingerprintError::TreeWalk(walk_errors.join("; ")));
        }

        // Step D: the sorted path/NUL/CID/NUL contract, plus a separate
        // gitlink section only when a gitlink was reached.
        Ok(Self::from_inputs(inputs, gitlinks))
    }
}

/// Decides whether a step's patterns reach a gitlink: whether some pattern
/// matches the gitlink path itself, or matches at least one path beneath it.
///
/// "Beneath" is decided against the same regex globset compiles for each
/// pattern (so `*` crosses `/` exactly as it does for blob matching, since
/// patterns are built with globset's defaults). Each pattern's regex is
/// compiled once, lazily, into a minimized anchored DFA. After feeding
/// `<gitlink>/`, a pattern can match beneath the gitlink iff some further
/// non-empty input still reaches a match: minimization folds every state
/// with no route to a match into the dead state, and globset's trailing `$`
/// means a match is only ever confirmed at end of input.
struct GitlinkReach<'a> {
    globset: &'a GlobSet,
    globs: &'a [Glob],
    dfas: Option<Vec<dense::DFA<Vec<u32>>>>,
}

impl<'a> GitlinkReach<'a> {
    fn new(globset: &'a GlobSet, globs: &'a [Glob]) -> Self {
        GitlinkReach {
            globset,
            globs,
            dfas: None,
        }
    }

    fn reaches(&mut self, gitlink_path: &str) -> Result<bool, String> {
        if self.globset.is_match(gitlink_path) {
            return Ok(true);
        }
        if self.dfas.is_none() {
            let dfas = self
                .globs
                .iter()
                .map(|glob| beneath_dfa(glob.regex()).map_err(|e| format!("pattern '{}': {e}", glob.glob())))
                .collect::<Result<Vec<_>, _>>()?;
            self.dfas = Some(dfas);
        }
        let prefix = format!("{gitlink_path}/");
        let dfas = self.dfas.as_ref().expect("just built");
        for dfa in dfas {
            if can_match_after(dfa, prefix.as_bytes())? {
                return Ok(true);
            }
        }
        Ok(false)
    }
}

/// Compile a globset regex into a minimized DFA with globset's own syntax
/// options (bytes, not UTF-8; `.` matches newline).
fn beneath_dfa(regex: &str) -> Result<dense::DFA<Vec<u32>>, String> {
    dense::Builder::new()
        .configure(dense::Config::new().minimize(true).start_kind(StartKind::Anchored))
        .syntax(syntax::Config::new().utf8(false).dot_matches_new_line(true))
        .thompson(thompson::Config::new().utf8(false))
        .build(regex)
        .map_err(|e| e.to_string())
}

/// Whether some non-empty suffix `s` makes `prefix + s` a full match.
fn can_match_after(dfa: &dense::DFA<Vec<u32>>, prefix: &[u8]) -> Result<bool, String> {
    let input = Input::new(prefix).anchored(Anchored::Yes);
    let mut state = dfa.start_state_forward(&input).map_err(|e| e.to_string())?;
    for &byte in prefix {
        state = dfa.next_state(state, byte);
        if dfa.is_dead_state(state) {
            return Ok(false);
        }
    }
    Ok((0..=u8::MAX).any(|byte| !dfa.is_dead_state(dfa.next_state(state, byte))))
}

/// Whether the step's patterns reach the gitlink at `gitlink_path`.
#[cfg(test)]
fn patterns_reach_gitlink(patterns: &[&str], gitlink_path: &str) -> bool {
    let globs: Vec<Glob> = patterns.iter().map(|p| Glob::new(p).expect("valid glob")).collect();
    let mut builder = GlobSetBuilder::new();
    for glob in &globs {
        builder.add(glob.clone());
    }
    let globset = builder.build().expect("valid set");
    GitlinkReach::new(&globset, &globs)
        .reaches(gitlink_path)
        .expect("decidable")
}

/// Visitor that walks a tree, matches paths against globs, and collects
/// content identities for matching files.
///
/// Path tracking mirrors `gix`'s own `Recorder` — a deque holds the full
/// path snapshot for each pending subtree so that breadth-first traversal
/// can restore the correct prefix when it descends into a subdirectory.
struct TreeCollector<'a> {
    repo: &'a gix::Repository,
    globset: &'a globset::GlobSet,
    reach: GitlinkReach<'a>,
    inputs: &'a mut BTreeMap<String, BritCid>,
    /// Reached gitlinks: path -> pinned commit id (hex).
    gitlinks: &'a mut BTreeMap<String, String>,
    errors: &'a mut Vec<String>,
    /// The current path prefix (built up as we traverse).
    path: BString,
    /// Snapshot queue: one entry per pending subtree (breadth-first).
    path_deque: VecDeque<BString>,
    budget: Option<Arc<Budget>>,
}

impl<'a> TreeCollector<'a> {
    fn push_element(&mut self, component: &BStr) {
        if !self.path.is_empty() {
            self.path.push(b'/');
        }
        self.path.push_str(component);
    }

    fn pop_element(&mut self) {
        if let Some(pos) = self.path.rfind_byte(b'/') {
            self.path.resize(pos, 0);
        } else {
            self.path.clear();
        }
    }
}

impl<'a> gix::traverse::tree::Visit for TreeCollector<'a> {
    /// Restore path from the back of the deque (depth-first; not used here).
    fn pop_back_tracked_path_and_set_current(&mut self) {
        self.path = self.path_deque.pop_back().unwrap_or_default();
    }

    /// Restore path from the front of the deque (breadth-first descent).
    fn pop_front_tracked_path_and_set_current(&mut self) {
        self.path = self
            .path_deque
            .pop_front()
            .expect("every push_back_tracked_path_component has a matching pop_front");
    }

    /// Called for tree entries that will be queued for later traversal.
    /// Save the current path (with the directory component) so it can be
    /// restored when we descend into this subtree.
    fn push_back_tracked_path_component(&mut self, component: &BStr) {
        self.push_element(component);
        self.path_deque.push_back(self.path.clone());
    }

    /// Called for every entry (tree or nontree) before visit_*.
    fn push_path_component(&mut self, component: &BStr) {
        self.push_element(component);
    }

    /// Called after every entry (tree or nontree).
    fn pop_path_component(&mut self) {
        self.pop_element();
    }

    fn visit_tree(&mut self, _entry: &gix::objs::tree::EntryRef<'_>) -> gix::traverse::tree::visit::Action {
        // Continue(true) = descend into this subtree
        std::ops::ControlFlow::Continue(true)
    }

    fn visit_nontree(&mut self, entry: &gix::objs::tree::EntryRef<'_>) -> gix::traverse::tree::visit::Action {
        // Blobs are hashed by content and gitlinks keyed by their pin; symlinks are skipped.
        let kind = entry.mode.kind();
        if !matches!(
            kind,
            gix::objs::tree::EntryKind::Blob
                | gix::objs::tree::EntryKind::BlobExecutable
                | gix::objs::tree::EntryKind::Commit
        ) {
            return std::ops::ControlFlow::Continue(true);
        }

        // At this point self.path already contains the full path (push_path_component
        // was called with entry.filename before visit_nontree).
        let path_str = match std::str::from_utf8(&self.path) {
            Ok(s) => s.to_string(),
            Err(_) => {
                self.errors.push(format!("non-utf8 path: {:?}", self.path.as_bstr()));
                return std::ops::ControlFlow::Continue(true);
            }
        };

        if kind == gix::objs::tree::EntryKind::Commit {
            // The submodule's objects are not read (they are often absent):
            // the gitlink's identity is the commit it pins.
            match self.reach.reaches(&path_str) {
                Ok(true) => {
                    self.gitlinks.insert(path_str, entry.oid.to_hex().to_string());
                }
                Ok(false) => {}
                Err(e) => self.errors.push(format!("gitlink {path_str}: {e}")),
            }
            return std::ops::ControlFlow::Continue(true);
        }

        // Glob match against the full path
        if !self.globset.is_match(&path_str) {
            return std::ops::ControlFlow::Continue(true);
        }

        let content_id = (|| -> gix::Result<BritCid> {
            if self.budget.is_none() {
                self.budget = Some(Budget::new(&std::env::temp_dir()).or_error()?);
            }
            let (_, spool) = self.repo.find_stream(
                entry.oid,
                self.budget.as_ref().expect("selected input has scratch"),
                &gix::interrupt::IS_INTERRUPTED,
            )?;
            BritCid::compute_reader(gix::utils::interrupt::Read {
                inner: spool,
                should_interrupt: &gix::interrupt::IS_INTERRUPTED,
            })
            .or_error()
        })();
        match content_id {
            Ok(cid) => {
                self.inputs.insert(path_str, cid);
            }
            Err(e) => {
                let cancelled = e.is_cancelled();
                self.errors
                    .push(format!("read {} (object {}): {e:?}", path_str, entry.oid));
                if cancelled {
                    return std::ops::ControlFlow::Break(());
                }
            }
        }

        std::ops::ControlFlow::Continue(true)
    }
}

#[cfg(test)]
mod tests {
    use super::patterns_reach_gitlink;

    /// Patterns are built with globset's defaults, as blob matching builds
    /// them: `*` and `?` cross `/` (no literal separator), `**` spans any
    /// number of components, and every pattern is anchored at the repo root.
    #[test]
    fn gitlink_reach_decision_table() {
        let gitlink = "elohim/rakia";
        let table: &[(&str, bool, &str)] = &[
            ("elohim/rakia/schemas/**", true, "declared beneath the gitlink"),
            ("elohim/rakia/**", true, "everything beneath"),
            ("elohim/rakia/schemas/*.json", true, "file pattern beneath"),
            ("elohim/**", true, "ancestor recursive"),
            ("**", true, "everything"),
            ("**/*.json", true, "any-depth extension"),
            ("**/schemas/**", true, "any-depth component that can sit beneath"),
            ("**/rakia/**", true, "any-depth gitlink name"),
            ("elohim/*/schemas/*.json", true, "single star in the gitlink's slot"),
            ("*.json", true, "default star crosses '/'"),
            ("elohim/*", true, "matches the gitlink path itself"),
            ("elohim/rakia", true, "the gitlink path itself"),
            ("elohim/rakia*", true, "prefix star reaches the gitlink and beneath"),
            ("?lohim/rakia/x", true, "question mark in a literal component"),
            (
                "elohim/{other,rakia}/schemas/*.json",
                true,
                "alternation with the gitlink",
            ),
            ("elohim/[r]akia/**", true, "class containing the gitlink's byte"),
            ("elohim/other/**", false, "sibling directory"),
            ("elohim/rakia.md", false, "sibling file sharing the prefix"),
            ("elohim/rakiax/**", false, "longer sibling name"),
            ("elohim/rakia-schemas/**", false, "dashed sibling name"),
            ("elohim/[!r]akia/**", false, "negated class excludes the gitlink"),
            ("elohim/{other,docs}/**", false, "alternation without the gitlink"),
            ("rakia/**", false, "anchored at the repo root, not any depth"),
            ("src/**", false, "unrelated tree"),
            (
                "elohim/rakia/",
                false,
                "only the bare directory string, no path beneath",
            ),
            ("elohim", false, "the parent path alone"),
        ];
        let mut wrong = Vec::new();
        for (pattern, expected, why) in table {
            if patterns_reach_gitlink(&[pattern], gitlink) != *expected {
                wrong.push(format!("{pattern} (expected {expected}: {why})"));
            }
        }
        assert!(wrong.is_empty(), "misdecided patterns: {wrong:#?}");
    }

    #[test]
    fn any_reaching_pattern_in_a_step_reaches_the_gitlink() {
        assert!(patterns_reach_gitlink(
            &["src/**", "elohim/rakia/schemas/**"],
            "elohim/rakia"
        ));
        assert!(!patterns_reach_gitlink(&["src/**", "elohim/other/**"], "elohim/rakia"));
        assert!(!patterns_reach_gitlink(&[], "elohim/rakia"));
    }

    #[test]
    fn top_level_gitlink_is_reached_from_the_root() {
        assert!(patterns_reach_gitlink(&["sophia/packages/**"], "sophia"));
        assert!(patterns_reach_gitlink(&["**/*.ts"], "sophia"));
        assert!(!patterns_reach_gitlink(&["sophiax/**"], "sophia"));
    }
}
