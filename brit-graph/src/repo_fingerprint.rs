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
use globset::{Glob, GlobBuilder, GlobSet, GlobSetBuilder};
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
    /// A symlink is an input by the same rule as a gitlink (a pattern matches
    /// its path or could match beneath it, as a glob written through a
    /// directory link does) and is keyed by its own object id, which changes
    /// when it is retargeted; it is not followed, so the file it points to
    /// counts only if a pattern matches that file. Empty pattern
    /// set or no matching entries produces a stable empty-input fingerprint.
    pub fn from_repo_globs(
        repo: &gix::Repository,
        commit_id: gix::ObjectId,
        patterns: &[String],
    ) -> Result<Self, FingerprintError> {
        Self::from_repo_globs_on(repo, commit_id, patterns, CANDIDATES_NORMALISED)
    }

    /// Compute a fingerprint from paths matched by `include` and not matched
    /// by `exclude` in the git tree at `commit_id`.
    ///
    /// Matching and input identity otherwise follow [`Self::from_repo_globs`],
    /// including gitlink and symlink reach. An empty `exclude` is equivalent
    /// to [`Self::from_repo_globs`] with `include`.
    pub fn from_repo_globs_excluding(
        repo: &gix::Repository,
        commit_id: gix::ObjectId,
        include: &[String],
        exclude: &[String],
    ) -> Result<Self, FingerprintError> {
        Self::from_repo_globs_excluding_on(repo, commit_id, include, exclude, CANDIDATES_NORMALISED)
    }

    /// [`Self::from_repo_globs`] with the host's candidate normalisation made
    /// explicit, so both values are exercised on any host.
    fn from_repo_globs_on(
        repo: &gix::Repository,
        commit_id: gix::ObjectId,
        patterns: &[String],
        candidates_normalised: bool,
    ) -> Result<Self, FingerprintError> {
        Self::from_repo_globs_excluding_on(repo, commit_id, patterns, &[], candidates_normalised)
    }

    fn from_repo_globs_excluding_on(
        repo: &gix::Repository,
        commit_id: gix::ObjectId,
        include: &[String],
        exclude: &[String],
        candidates_normalised: bool,
    ) -> Result<Self, FingerprintError> {
        // Step A: build the GlobSet
        let mut builder = GlobSetBuilder::new();
        let mut globs = Vec::with_capacity(include.len());
        for pattern in include {
            let glob = compile_step_glob(pattern)?;
            builder.add(glob.clone());
            globs.push(glob);
        }
        let globset = builder.build().map_err(|e| FingerprintError::InvalidGlob {
            pattern: include.join(", "),
            message: e.to_string(),
        })?;
        let mut exclude_builder = GlobSetBuilder::new();
        for pattern in exclude {
            let glob = compile_step_glob(pattern)?;
            exclude_builder.add(glob);
        }
        let exclude_globset = exclude_builder.build().map_err(|e| FingerprintError::InvalidGlob {
            pattern: exclude.join(", "),
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
            exclude_globset: &exclude_globset,
            inputs: &mut inputs,
            gitlinks: &mut gitlinks,
            symlinks: &mut symlinks,
            errors: &mut walk_errors,
            candidates_normalised,
            refusal: None,
            path: BString::default(),
            path_deque: VecDeque::new(),
            budget: None,
        };

        let walked = tree.traverse().breadthfirst(&mut visitor);
        if let Some(refusal) = visitor.refusal.take() {
            return Err(refusal);
        }
        walked.map_err(|e| FingerprintError::TreeWalk(e.to_string()))?;

        if !walk_errors.is_empty() {
            return Err(FingerprintError::TreeWalk(walk_errors.join("; ")));
        }

        // Step D: the sorted path/NUL/CID/NUL contract, plus separate gitlink
        // and symlink sections only when one was reached or matched.
        Ok(Self::from_inputs(inputs, gitlinks, symlinks))
    }
}

/// Compile one step glob.
///
/// The one place step-glob syntax is fixed; blob matching and gitlink/symlink
/// reach both use the glob (and so the regex) this returns. The five options
/// are globset's Unix defaults, stated explicitly because globset turns
/// `backslash_escape` off on Windows (where `\` is a path separator):
///
/// - `backslash_escape(true)`: `\` escapes the next character (`\*` is a
///   literal `*`); repository paths are always `/`-separated.
/// - `literal_separator(false)`: `*` and `?` also match `/`.
/// - `case_insensitive(false)`, `empty_alternates(false)`,
///   `allow_unclosed_class(false)`.
///
/// Guarantee: with these options fixed and the refusal below, the compiled
/// regex is the same on every platform. Matching then differs only for a
/// candidate path containing a backslash on a non-Unix host (globset rewrites
/// it to `/` there), which the tree walk refuses
/// ([`FingerprintError::UnportableTreePath`]).
///
/// Refused on every platform: a backslash directly beside `**`. globset's
/// `**` parser is the one place it still consults the host's separator rule
/// with these options, so `src/**\[x\]` would mean `src/[x]` on Unix and
/// `src/x` on Windows. See [`backslash_beside_double_star`] for the rule.
pub(crate) fn compile_step_glob(pattern: &str) -> Result<Glob, FingerprintError> {
    let glob = GlobBuilder::new(pattern)
        .backslash_escape(true)
        .literal_separator(false)
        .case_insensitive(false)
        .empty_alternates(false)
        .allow_unclosed_class(false)
        .build()
        .map_err(|e| FingerprintError::InvalidGlob {
            pattern: pattern.to_string(),
            message: e.to_string(),
        })?;
    if let Some(position) = backslash_beside_double_star(pattern) {
        return Err(FingerprintError::InvalidGlob {
            pattern: pattern.to_string(),
            message: format!(
                "the backslash at character {position} is directly beside `**`, which cannot be \
                 given one meaning on every platform; escape the name in a separate path \
                 component or use a character class"
            ),
        });
    }
    Ok(glob)
}

/// The position (in characters) of a backslash that globset's `**` parser
/// would read with the host's path-separator rule, if any.
///
/// Mirrors globset 0.4.19's tokeniser for a pattern that already compiled
/// with [`compile_step_glob`]'s options: outside a class, `\` escapes the
/// next character; `[` opens a class (after an optional `!`/`^`, a first
/// `]` is literal) that ends at the next `]`, with no escapes inside; an
/// unescaped `*` followed by `*` is one `**`. For each `**`, `parse_star`
/// looks at the raw character before the first `*` and the raw character
/// after the second. With `backslash_escape(true)` a raw `\` can stand
/// before an unescaped `*` only as the second half of an escaped backslash,
/// so the rule is: refuse when the character before a `**` is `\` (an
/// escaped backslash) or the character after it is `\` (an escape).
fn backslash_beside_double_star(pattern: &str) -> Option<usize> {
    let chars: Vec<char> = pattern.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        match chars[i] {
            '\\' => i += 2,
            '[' => {
                let mut j = i + 1;
                if matches!(chars.get(j), Some('!' | '^')) {
                    j += 1;
                }
                if chars.get(j) == Some(&']') {
                    j += 1;
                }
                while j < chars.len() && chars[j] != ']' {
                    j += 1;
                }
                i = j + 1;
            }
            '*' if chars.get(i + 1) == Some(&'*') => {
                if i > 0 && chars[i - 1] == '\\' {
                    return Some(i - 1);
                }
                if chars.get(i + 2) == Some(&'\\') {
                    return Some(i + 2);
                }
                i += 2;
            }
            _ => i += 1,
        }
    }
    None
}

/// Whether this host's glob matcher rewrites `\` in candidate paths to `/`
/// (globset's `normalize_path` on non-Unix hosts).
const CANDIDATES_NORMALISED: bool = cfg!(not(unix));

/// Refuse a tree path that this host would match with a different meaning:
/// one containing a backslash where candidates are normalised.
fn refuse_unportable_tree_path(path: &str, candidates_normalised: bool) -> Result<(), FingerprintError> {
    if candidates_normalised && path.contains('\\') {
        return Err(FingerprintError::UnportableTreePath { path: path.to_string() });
    }
    Ok(())
}

/// Upper bound on the heap a pattern's Thompson NFA may use. A pattern over
/// it is answered "reaches" (over-keying), never "does not reach".
const NFA_SIZE_LIMIT: usize = 1 << 20;

/// Decides whether a step's patterns reach a gitlink or a symlink: whether some
/// pattern matches its path itself, or could match at least one path beneath it.
///
/// "Beneath" is decided on the Thompson NFA of the same regex globset compiles
/// for each pattern, with globset's syntax options (so `*` crosses `/` exactly
/// as it does for blob matching: both use the glob [`compile_step_glob`]
/// returns). The NFA is simulated over `<gitlink>/`; the pattern reaches the
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
        State::Look { .. }
        | State::Union { .. }
        | State::BinaryUnion { .. }
        | State::Capture { .. }
        | State::Fail
        | State::Match { .. } => None,
    }
}

/// The states `state` reaches without consuming a byte. Look-around is
/// passable: that can only over-approximate the matches.
fn epsilon_successors(nfa: &NFA, state: StateID) -> Vec<StateID> {
    match nfa.state(state) {
        State::Look { next, .. } | State::Capture { next, .. } => vec![*next],
        State::Union { alternates } => alternates.to_vec(),
        State::BinaryUnion { alt1, alt2 } => vec![*alt1, *alt2],
        State::ByteRange { .. } | State::Sparse(_) | State::Dense(_) | State::Fail | State::Match { .. } => Vec::new(),
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
        State::Look { .. }
        | State::Union { .. }
        | State::BinaryUnion { .. }
        | State::Capture { .. }
        | State::Fail
        | State::Match { .. } => Vec::new(),
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
    let globs: Vec<Glob> = patterns
        .iter()
        .map(|p| compile_step_glob(p).expect("valid glob"))
        .collect();
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
    exclude_globset: &'a globset::GlobSet,
    inputs: &'a mut BTreeMap<String, BritCid>,
    /// Reached gitlinks: path -> pinned commit id (hex).
    gitlinks: &'a mut BTreeMap<String, String>,
    /// Matched symlinks: path -> the link's own object id (hex).
    symlinks: &'a mut BTreeMap<String, String>,
    errors: &'a mut Vec<String>,
    /// Whether candidates are normalised on this host (see
    /// [`refuse_unportable_tree_path`]).
    candidates_normalised: bool,
    /// Set when the walk must stop with a named error.
    refusal: Option<FingerprintError>,
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

        if let Err(refusal) = refuse_unportable_tree_path(&path_str, self.candidates_normalised) {
            self.refusal = Some(refusal);
            return std::ops::ControlFlow::Break(());
        }

        if kind == gix::objs::tree::EntryKind::Commit {
            // The submodule's objects are not read (they are often absent):
            // the gitlink's identity is the commit it pins.
            if self.reach.reaches(&path_str) && !self.exclude_globset.is_match(&path_str) {
                self.gitlinks.insert(path_str, entry.oid.to_hex().to_string());
            }
            return std::ops::ControlFlow::Continue(true);
        }

        if kind == gix::objs::tree::EntryKind::Link {
            // Not followed: the link's identity is its own blob, the target
            // path. Reached like a gitlink, so a glob written through a
            // directory link (`current/**`) keys the link too.
            if self.reach.reaches(&path_str) && !self.exclude_globset.is_match(&path_str) {
                self.symlinks.insert(path_str, entry.oid.to_hex().to_string());
            }
            return std::ops::ControlFlow::Continue(true);
        }

        // Glob match against the full path
        if !self.globset.is_match(&path_str) || self.exclude_globset.is_match(&path_str) {
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
    use globset::GlobSetBuilder;

    use super::{compile_step_glob, patterns_reach_gitlink, refuse_unportable_tree_path, GitlinkReach};

    /// A backslash directly beside `**` is read by globset's `**` parser with
    /// the host's path-separator rule, so it is refused on every platform;
    /// near-misses that tokenise without that adjacency still compile.
    #[test]
    fn backslash_beside_double_star_is_refused_everywhere() {
        let refused = [
            "src/**\\[x\\]",
            "**\\*.rs",
            "a\\\\**",
            "\\\\**",
            "**\\x",
            "{a,**\\b}",
            "src/**\\",
        ];
        for pattern in refused {
            assert!(compile_step_glob(pattern).is_err(), "{pattern} must be refused");
        }
        let compiled = [
            "**/\\[x\\]",
            "a\\*b/**",
            "\\*\\*",
            "a/**/b",
            "a\\\\\\**",
            "\\**",
            "***\\x",
            "[\\]**",
            "[]\\]/**",
            "src/**",
            "a\\\\/**",
        ];
        for pattern in compiled {
            assert!(compile_step_glob(pattern).is_ok(), "{pattern} must still compile");
        }
    }

    #[test]
    fn refusal_names_the_pattern_and_the_position() {
        let err = compile_step_glob("src/**\\[x\\]").expect_err("refused");
        let message = err.to_string();
        assert!(message.contains("src/**\\[x\\]"), "{message}");
        assert!(message.contains("character 6"), "{message}");
        assert!(
            message.contains("cannot be given one meaning on every platform"),
            "{message}"
        );
    }

    #[test]
    fn backslash_tree_path_is_refused_only_where_candidates_are_normalised() {
        use crate::fingerprint::FingerprintError;
        assert!(matches!(
            refuse_unportable_tree_path("dir\\file", true),
            Err(FingerprintError::UnportableTreePath { path }) if path == "dir\\file"
        ));
        assert!(refuse_unportable_tree_path("dir\\file", false).is_ok());
        assert!(refuse_unportable_tree_path("dir/file", true).is_ok());
        assert!(refuse_unportable_tree_path("dir/file", false).is_ok());
    }

    /// The walk calls the check for every entry it visits, with either flag.
    ///
    /// Unix only: Git for Windows refuses to stage a path containing a backslash, so
    /// this fixture cannot be built there. The function-level test
    /// `backslash_tree_path_is_refused_only_where_candidates_are_normalised` covers
    /// both flag values on every platform.
    #[cfg(unix)]
    #[test]
    fn fingerprint_walk_refuses_a_backslash_path_only_where_candidates_are_normalised() {
        use std::io::Write as _;
        use std::process::{Command, Stdio};

        use crate::fingerprint::{ContentFingerprint, FingerprintError};

        let dir = tempfile::TempDir::new().expect("temp");
        let path = dir.path();
        let git = |args: &[&str]| {
            let output = Command::new("git").args(args).current_dir(path).output().expect("git");
            assert!(
                output.status.success(),
                "git {args:?}: {}",
                String::from_utf8_lossy(&output.stderr)
            );
            String::from_utf8(output.stdout).expect("utf8").trim().to_string()
        };
        git(&["init", "-q"]);
        git(&["config", "user.email", "t@t.t"]);
        git(&["config", "user.name", "t"]);
        for (rel, bytes) in [("src/a.txt", "a\n"), ("other/dir\\file", "b\n")] {
            let mut child = Command::new("git")
                .args(["hash-object", "-w", "--stdin"])
                .current_dir(path)
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .spawn()
                .expect("hash-object");
            child
                .stdin
                .take()
                .expect("stdin")
                .write_all(bytes.as_bytes())
                .expect("write");
            let oid = String::from_utf8(child.wait_with_output().expect("wait").stdout).expect("utf8");
            git(&[
                "update-index",
                "--add",
                "--cacheinfo",
                &format!("100644,{},{rel}", oid.trim()),
            ]);
        }
        git(&["commit", "-q", "-m", "c"]);
        let repo = gix::open(path).expect("open");
        let head = repo.head_id().expect("head").detach();
        let patterns = vec!["src/**".to_string()];

        let unix = ContentFingerprint::from_repo_globs_on(&repo, head, &patterns, false).expect("no check");
        assert_eq!(unix.inputs.keys().collect::<Vec<_>>(), ["src/a.txt"]);
        let elsewhere = ContentFingerprint::from_repo_globs_on(&repo, head, &patterns, true);
        assert!(
            matches!(&elsewhere, Err(FingerprintError::UnportableTreePath { path }) if path == "other/dir\\file"),
            "{elsewhere:?}"
        );
    }

    /// Pins what `compile_step_glob`'s five options do: a backslash escapes
    /// (globset's Windows default would not), `*` crosses `/`, matching is
    /// case-sensitive, and the regex of an escaped pattern is exactly this.
    /// Written to pass on Linux, macOS and Windows alike.
    #[test]
    fn step_glob_settings_pin_an_escaped_pattern() {
        let escaped = compile_step_glob("a\\*b/**").expect("valid glob");
        assert_eq!(escaped.regex(), "(?-u)^a\\*b/.*$");
        let escaped = escaped.compile_matcher();
        assert!(escaped.is_match("a*b/x"));
        assert!(!escaped.is_match("axb/x"));

        let star = compile_step_glob("a*b").expect("valid glob").compile_matcher();
        assert!(star.is_match("a/x/b"), "`*` crosses `/`");
        assert!(!compile_step_glob("A*")
            .expect("valid glob")
            .compile_matcher()
            .is_match("a"));

        // The reach decision compiles the same way.
        assert!(patterns_reach_gitlink(&["\\[\\]\\]/**"], "[]]"));
        assert!(patterns_reach_gitlink(&["\\[-\\]/**"], "[-]"));
    }

    /// A pattern whose NFA exceeds the size limit reaches every gitlink: the
    /// decision may over-key a step, never under-key it.
    #[test]
    fn size_limit_hit_answers_reaches() {
        let globs = vec![compile_step_glob("elohim/other/**").expect("valid glob")];
        let mut builder = GlobSetBuilder::new();
        builder.add(globs[0].clone());
        let globset = builder.build().expect("valid set");
        assert!(!GitlinkReach::new(&globset, &globs).reaches("elohim/rakia"));
        assert!(GitlinkReach::with_size_limit(&globset, &globs, 1).reaches("elohim/rakia"));
    }

    /// Patterns are built by `compile_step_glob`, as blob matching builds
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

    /// The same rule decides gitlinks and symlinks, at any path: the path's
    /// own bytes are literal, whatever regex metacharacters they contain.
    #[test]
    fn reach_decision_table_for_other_paths() {
        let table: &[(&str, &str, bool, &str)] = &[
            (
                "elohim/sdk/schemas/current/**",
                "elohim/sdk/schemas/current",
                true,
                "glob written through a directory link",
            ),
            (
                "elohim/sdk/schemas/current/*.json",
                "elohim/sdk/schemas/current",
                true,
                "file pattern through the link",
            ),
            (
                "elohim/sdk/schemas/*/view.json",
                "elohim/sdk/schemas/current",
                true,
                "star in the link's slot",
            ),
            (
                "elohim/sdk/schemas/v1/**",
                "elohim/sdk/schemas/current",
                false,
                "sibling target directory",
            ),
            (
                "elohim/sdk/schemas/current.md",
                "elohim/sdk/schemas/current",
                false,
                "sibling file",
            ),
            ("x.y/a+b/**", "x.y/a+b", true, "metacharacters in the path are literal"),
            ("x?y/a?b/*", "x.y/a+b", true, "question marks over metacharacters"),
            ("xzy/a+b/**", "x.y/a+b", false, "'.' in the path is not a wildcard"),
            ("x.y/aab/**", "x.y/a+b", false, "'+' in the path is not a repetition"),
            ("é/**", "é/sub", true, "non-ASCII ancestor"),
            ("??/sub/*", "é/sub", true, "two '?' cover the two bytes of 'é'"),
            (
                "?/sub/*",
                "é/sub",
                false,
                "one '?' is one byte, as blob matching treats it",
            ),
            ("e/sub/**", "é/sub", false, "ASCII look-alike"),
            ("\\[\\]\\]/**", "[]]", true, "escaped brackets"),
            ("[[]]]/**", "[]]", true, "classes spelling the brackets"),
            ("[!]]*/**", "[]]", true, "'[' is not ']'"),
            (
                "[-]/**",
                "[-]",
                false,
                "a class matching '-' is not the three-byte name",
            ),
            ("\\[-\\]/**", "[-]", true, "escaped name"),
        ];
        let mut wrong = Vec::new();
        for (pattern, path, expected, why) in table {
            if patterns_reach_gitlink(&[pattern], path) != *expected {
                wrong.push(format!("{pattern} at {path} (expected {expected}: {why})"));
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
