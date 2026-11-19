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
    nfa::thompson::{self, State, WhichCaptures, NFA},
    util::{primitives::StateID, syntax},
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
    /// A symlink whose path a pattern matches is keyed by its own object id,
    /// which changes when it is retargeted; it is not followed, so the file
    /// it points to counts only if a pattern matches that file. Empty pattern
    /// set or no matching entries produces a stable empty-input fingerprint.
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
        // record each reached gitlink with its pinned commit and each matched
        // symlink with its own object id.
        let mut inputs = BTreeMap::new();
        let mut gitlinks = BTreeMap::new();
        let mut symlinks = BTreeMap::new();
        let mut walk_errors: Vec<String> = Vec::new();

        let mut visitor = TreeCollector {
            repo,
            globset: &globset,
            reach: GitlinkReach::new(&globset, &globs),
            inputs: &mut inputs,
            gitlinks: &mut gitlinks,
            symlinks: &mut symlinks,
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

        // Step D: the sorted path/NUL/CID/NUL contract, plus separate gitlink
        // and symlink sections only when one was reached or matched.
        Ok(Self::from_inputs(inputs, gitlinks, symlinks))
    }
}

/// Upper bound on the heap a pattern's Thompson NFA may use. A pattern over
/// it is answered "reaches" (over-keying), never "does not reach".
const NFA_SIZE_LIMIT: usize = 1 << 20;

/// Decides whether a step's patterns reach a gitlink: whether some pattern
/// matches the gitlink path itself, or could match at least one path beneath it.
///
/// "Beneath" is decided on the Thompson NFA of the same regex globset compiles
/// for each pattern, with globset's syntax options (so `*` crosses `/` exactly
/// as it does for blob matching, since patterns are built with globset's
/// defaults). The NFA is simulated over `<gitlink>/`; the pattern reaches the
/// gitlink iff, from the live states, some byte transition leads on to a match
/// state. That is a graph search, linear in the NFA's size: nothing is
/// determinized, so no pattern can make it exponential.
///
/// Every doubt answers "reaches": look-around states are passable whatever
/// the context (globset only anchors with `^`/`$`, so this can only add
/// matches), and a pattern whose NFA cannot be built within the size limit
/// reaches every gitlink. A wrong "reaches" over-keys a step; a wrong "does
/// not reach" would reuse evidence, so the decision never errs that way.
struct GitlinkReach<'a> {
    globset: &'a GlobSet,
    globs: &'a [Glob],
    size_limit: usize,
    /// One entry per pattern; `None` when its NFA could not be built.
    nfas: Option<Vec<Option<NFA>>>,
}

impl<'a> GitlinkReach<'a> {
    fn new(globset: &'a GlobSet, globs: &'a [Glob]) -> Self {
        Self::with_size_limit(globset, globs, NFA_SIZE_LIMIT)
    }

    fn with_size_limit(globset: &'a GlobSet, globs: &'a [Glob], size_limit: usize) -> Self {
        GitlinkReach {
            globset,
            globs,
            size_limit,
            nfas: None,
        }
    }

    fn reaches(&mut self, gitlink_path: &str) -> bool {
        if self.globset.is_match(gitlink_path) {
            return true;
        }
        let (globs, size_limit) = (self.globs, self.size_limit);
        let nfas = self
            .nfas
            .get_or_insert_with(|| globs.iter().map(|glob| beneath_nfa(glob.regex(), size_limit)).collect());
        let prefix = format!("{gitlink_path}/");
        nfas.iter().any(|nfa| match nfa {
            Some(nfa) => can_match_after(nfa, prefix.as_bytes()),
            None => true,
        })
    }
}

/// Compile a globset regex into a Thompson NFA with globset's own syntax
/// options (bytes, not UTF-8; `.` matches newline). `None` on any failure,
/// including the size limit.
fn beneath_nfa(regex: &str, size_limit: usize) -> Option<NFA> {
    thompson::Compiler::new()
        .syntax(syntax::Config::new().utf8(false).dot_matches_new_line(true))
        .configure(
            thompson::Config::new()
                .utf8(false)
                .nfa_size_limit(Some(size_limit))
                .which_captures(WhichCaptures::None),
        )
        .build(regex)
        .ok()
}

/// The state a byte transition of `state` leads to on `byte`, if any.
fn step(nfa: &NFA, state: StateID, byte: u8) -> Option<StateID> {
    match nfa.state(state) {
        State::ByteRange { trans } => trans.matches_byte(byte).then_some(trans.next),
        State::Sparse(sparse) => sparse.matches_byte(byte),
        State::Dense(dense) => dense.matches_byte(byte),
        _ => None,
    }
}

/// The states `state` reaches without consuming a byte. Look-around is
/// passable: that can only over-approximate the matches.
fn epsilon_successors(nfa: &NFA, state: StateID) -> Vec<StateID> {
    match nfa.state(state) {
        State::Look { next, .. } | State::Capture { next, .. } => vec![*next],
        State::Union { alternates } => alternates.to_vec(),
        State::BinaryUnion { alt1, alt2 } => vec![*alt1, *alt2],
        _ => Vec::new(),
    }
}

/// The states a byte transition of `state` can lead to, on any byte.
fn byte_successors(nfa: &NFA, state: StateID) -> Vec<StateID> {
    match nfa.state(state) {
        State::ByteRange { trans } => vec![trans.next],
        State::Sparse(sparse) => sparse.transitions.iter().map(|trans| trans.next).collect(),
        State::Dense(dense) => dense
            .transitions
            .iter()
            .copied()
            .filter(|next| *next != StateID::ZERO)
            .collect(),
        _ => Vec::new(),
    }
}

/// Every state reachable from `start` by epsilon transitions, `start` included.
fn epsilon_closure(nfa: &NFA, start: impl IntoIterator<Item = StateID>) -> Vec<StateID> {
    let mut seen = vec![false; nfa.states().len()];
    let mut stack: Vec<StateID> = start.into_iter().collect();
    let mut closure = Vec::new();
    while let Some(state) = stack.pop() {
        if std::mem::replace(&mut seen[state.as_usize()], true) {
            continue;
        }
        closure.push(state);
        stack.extend(epsilon_successors(nfa, state));
    }
    closure
}

/// Whether some non-empty suffix `s` can make `prefix + s` a full match.
fn can_match_after(nfa: &NFA, prefix: &[u8]) -> bool {
    let mut live = epsilon_closure(nfa, [nfa.start_anchored()]);
    for &byte in prefix {
        let next: Vec<StateID> = live.iter().filter_map(|&state| step(nfa, state, byte)).collect();
        live = epsilon_closure(nfa, next);
        if live.is_empty() {
            return false;
        }
    }
    // Consume at least one byte, then search for any path to a match.
    let mut seen = vec![false; nfa.states().len()];
    let mut stack: Vec<StateID> = live.iter().flat_map(|&state| byte_successors(nfa, state)).collect();
    while let Some(state) = stack.pop() {
        if std::mem::replace(&mut seen[state.as_usize()], true) {
            continue;
        }
        if matches!(nfa.state(state), State::Match { .. }) {
            return true;
        }
        stack.extend(epsilon_successors(nfa, state));
        stack.extend(byte_successors(nfa, state));
    }
    false
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
    GitlinkReach::new(&globset, &globs).reaches(gitlink_path)
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
    /// Matched symlinks: path -> the link's own object id (hex).
    symlinks: &'a mut BTreeMap<String, String>,
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
        // Blobs are hashed by content, gitlinks keyed by their pin and
        // symlinks by their own object id (their target path).
        let kind = entry.mode.kind();

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
            if self.reach.reaches(&path_str) {
                self.gitlinks.insert(path_str, entry.oid.to_hex().to_string());
            }
            return std::ops::ControlFlow::Continue(true);
        }

        // Glob match against the full path
        if !self.globset.is_match(&path_str) {
            return std::ops::ControlFlow::Continue(true);
        }

        if kind == gix::objs::tree::EntryKind::Link {
            // Not followed: the link's identity is its own blob, the target path.
            self.symlinks.insert(path_str, entry.oid.to_hex().to_string());
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
    use globset::{Glob, GlobSetBuilder};

    use super::{patterns_reach_gitlink, GitlinkReach};

    /// A pattern whose NFA exceeds the size limit reaches every gitlink: the
    /// decision may over-key a step, never under-key it.
    #[test]
    fn size_limit_hit_answers_reaches() {
        let globs = vec![Glob::new("elohim/other/**").expect("valid glob")];
        let mut builder = GlobSetBuilder::new();
        builder.add(globs[0].clone());
        let globset = builder.build().expect("valid set");
        assert!(!GitlinkReach::new(&globset, &globs).reaches("elohim/rakia"));
        assert!(GitlinkReach::with_size_limit(&globset, &globs, 1).reaches("elohim/rakia"));
    }

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

    /// Globs whose DFA is exponential in the number of `?` after a `*`
    /// (each was 15 s to minutes, unbounded memory, with a determinized DFA).
    #[test]
    fn pathological_globs_are_decided_quickly() {
        let reaching = [
            format!("*a{}", "?".repeat(14)),
            format!("*a{}", "?".repeat(16)),
            "**/deps/*-????????????????.rlib".to_string(),
            format!("*a{}", "?".repeat(64)),
        ];
        let not_reaching = [
            format!("elohim/other/*a{}", "?".repeat(16)),
            "elohim/other/**/deps/*-????????????????.rlib".to_string(),
        ];
        for (patterns, expected) in [(&reaching[..], true), (&not_reaching[..], false)] {
            for pattern in patterns {
                let started = std::time::Instant::now();
                let reached = patterns_reach_gitlink(&[pattern.as_str()], "elohim/rakia");
                let elapsed = started.elapsed();
                assert_eq!(reached, expected, "{pattern}");
                assert!(
                    elapsed < std::time::Duration::from_millis(500),
                    "{pattern} took {elapsed:?}"
                );
            }
        }
    }

    #[test]
    fn top_level_gitlink_is_reached_from_the_root() {
        assert!(patterns_reach_gitlink(&["sophia/packages/**"], "sophia"));
        assert!(patterns_reach_gitlink(&["**/*.ts"], "sophia"));
        assert!(!patterns_reach_gitlink(&["sophiax/**"], "sophia"));
    }
}
