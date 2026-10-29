//! `BritRefManager` — read/write/list git refs under `refs/notes/brit/`.
//!
//! Every read is verified. A stored record is returned only when its original bytes decode
//! strictly into the typed node (an unknown field is a decode failure), its signature verifies
//! against the node's own `agentId`, and it binds to the ref it was found under. That establishes
//! provenance — which key produced these bytes — and nothing more: brit returns no trusted,
//! admitted or standing flag. Whether that key is a recognized device, and whether its holder had
//! standing, is the reader's to decide.
//!
//! Absence is established, never inferred: a record is absent only when git answered the lookup
//! and nothing is stored there. A git subprocess that fails is an error, not a miss.

use std::{
    path::{Path, PathBuf},
    process::Command,
};

use crate::{
    elohim::attestation::{
        build::BuildAttestationContentNode, covenant::CovenantRecord, deploy::DeployAttestationContentNode,
        validation::ValidationAttestationContentNode,
    },
    engine::{
        content_node::ContentNode,
        signing::{verify_signed_node, Signed},
    },
};

const BUILD_PREFIX: &str = "refs/notes/brit/build/";
const DEPLOY_PREFIX: &str = "refs/notes/brit/deploy/";
const VALIDATE_PREFIX: &str = "refs/notes/brit/validate/";
const COVENANT_PREFIX: &str = "refs/notes/brit/covenant/";

/// The answer to a lookup at one known place.
#[derive(Debug, Clone, PartialEq)]
pub enum Lookup<T> {
    /// Git answered the lookup and no record is stored there.
    Absent,
    /// A record is stored there and failed verification; it is not returned.
    Unverifiable {
        /// Where it was found: the annotated commit for a note, the ref name for a blob ref.
        at: String,
        /// Why it was not returned.
        reason: String,
    },
    /// The record decodes strictly, verifies against its own signer and binds to where it was found.
    Verified(T),
}

/// The records a scan found.
#[derive(Debug, Clone, PartialEq)]
pub struct Scan<T> {
    /// `(where, node)` for each verified record the scan selected.
    pub verified: Vec<(String, T)>,
    /// `(where, reason)` for each stored record that failed verification.
    pub unverifiable: Vec<(String, String)>,
}

impl<T> Default for Scan<T> {
    fn default() -> Self {
        Self {
            verified: Vec::new(),
            unverifiable: Vec::new(),
        }
    }
}

/// Decode `bytes` strictly into `T`, verify its signature against its own `agentId`, then check
/// `bind` — that it belongs where it was found. `Err` is the reason it is unverifiable.
///
/// The bytes are parsed as stored: never repaired, never read through a lossy UTF-8 conversion.
pub fn read_verified<T: Signed>(bytes: &[u8], bind: impl FnOnce(&T) -> Result<(), String>) -> Result<T, String> {
    let node: T = serde_json::from_slice(bytes).map_err(|e| format!("does not decode: {e}"))?;
    admit(node, bind)
}

/// Verify a decoded node's signature against its own signer, then `bind` it.
fn admit<T: Signed>(node: T, bind: impl FnOnce(&T) -> Result<(), String>) -> Result<T, String> {
    match verify_signed_node(&node) {
        Ok(true) => {}
        Ok(false) => {
            return Err(format!(
                "signature does not verify against agentId {:?}",
                node.agent_id()
            ))
        }
        Err(e) => return Err(format!("signature does not verify: {e}")),
    }
    bind(&node)?;
    Ok(node)
}

fn bound(field: &str, stored: &str, expected: &str) -> Result<(), String> {
    if stored == expected {
        Ok(())
    } else {
        Err(format!("{field} {stored:?} does not match this ref's {expected:?}"))
    }
}

/// A build note: verified, and bound to the step whose ref it was found under.
fn read_build(bytes: &[u8], step_name: &str) -> Result<BuildAttestationContentNode, String> {
    read_verified(bytes, |n: &BuildAttestationContentNode| {
        bound("stepName", &n.step_name, step_name)
    })
}

/// A covenant record: strict decode, the clause's signature, the stored CID recomputed from the
/// clause, and the ref's session and CID segments.
fn read_covenant(bytes: &[u8], session_segment: &str, cid_segment: &str) -> Result<CovenantRecord, String> {
    let CovenantRecord { cid, clause } = serde_json::from_slice(bytes).map_err(|e| format!("does not decode: {e}"))?;
    let clause = admit(clause, |c| {
        let computed = c
            .compute_cid()
            .map_err(|e| format!("clause does not encode: {e}"))?
            .to_string();
        bound("cid", &cid, &computed)?;
        bound("cid", &BritRefManager::encode_segment(&cid), cid_segment)?;
        bound("session", &BritRefManager::encode_segment(&c.session), session_segment)
    })?;
    Ok(CovenantRecord { cid, clause })
}

/// Manages git refs under `refs/notes/brit/` for attestation indexing.
pub struct BritRefManager {
    repo_path: PathBuf,
}

impl BritRefManager {
    /// Create a new `BritRefManager` for the given repository root.
    ///
    /// Returns [`RefError::NotARepo`] if `repo_path` is not a git repository.
    pub fn new(repo_path: &Path) -> Result<Self, RefError> {
        if !repo_path.join(".git").exists() && !repo_path.join("HEAD").exists() {
            return Err(RefError::NotARepo(repo_path.display().to_string()));
        }
        Ok(Self {
            repo_path: repo_path.to_path_buf(),
        })
    }

    // --- Build refs (per-commit via git notes) ---

    fn build_ref(step_name: &str) -> String {
        format!("{BUILD_PREFIX}{}", Self::encode_segment(step_name))
    }

    /// Write a build attestation payload as a git note on `commit_rev`.
    pub fn put_build_ref(
        &self,
        step_name: &str,
        commit_rev: &str,
        payload: &serde_json::Value,
    ) -> Result<(), RefError> {
        let commit_sha = self.resolve_rev(commit_rev)?;
        self.write_note(&Self::build_ref(step_name), &commit_sha, payload)
    }

    /// The build attestation noted on `commit_rev` for `step_name`.
    ///
    /// A note binds to its step: its `stepName` must be `step_name`. The note does not sign the
    /// commit it annotates — that association is the notes tree's, not the signer's.
    pub fn get_build_ref(
        &self,
        step_name: &str,
        commit_rev: &str,
    ) -> Result<Lookup<BuildAttestationContentNode>, RefError> {
        let commit_sha = self.resolve_rev(commit_rev)?;
        let Some((note, _)) = self
            .notes(&Self::build_ref(step_name))?
            .into_iter()
            .find(|(_, annotated)| *annotated == commit_sha)
        else {
            return Ok(Lookup::Absent);
        };
        let body = self.cat_blobs(vec![note])?.remove(0);
        Ok(match body.and_then(|b| read_build(&b, step_name)) {
            Ok(node) => Lookup::Verified(node),
            Err(reason) => Lookup::Unverifiable { at: commit_sha, reason },
        })
    }

    /// List all build ref step names under `refs/notes/brit/build/`.
    pub fn list_build_refs(&self, _pattern: Option<&str>) -> Result<Vec<String>, RefError> {
        self.list_refs_under(BUILD_PREFIX)
    }

    /// Every verified build attestation under this step's notes ref whose `inputsHash` equals
    /// `inputs_hash`, as `(commit_sha, node)` in `git notes list` order, plus every note under
    /// the ref that failed verification, whatever closure it claims.
    ///
    /// The notes are keyed by commit, but the question a gate asks is "has this exact input
    /// closure already been built?" — and that answer must not depend on which commit carried
    /// the closure. One `git notes list` plus one `git cat-file --batch` over the note objects;
    /// a step with no notes ref yet is an empty scan.
    pub fn find_build_by_inputs(
        &self,
        step_name: &str,
        inputs_hash: &str,
    ) -> Result<Scan<BuildAttestationContentNode>, RefError> {
        let pairs = self.notes(&Self::build_ref(step_name))?;
        let bodies = self.cat_blobs(pairs.iter().map(|(note, _)| note.clone()).collect())?;
        let mut scan = Scan::default();
        for ((_, commit), body) in pairs.into_iter().zip(bodies) {
            match body.and_then(|b| read_build(&b, step_name)) {
                Ok(node) if node.inputs_hash == inputs_hash => scan.verified.push((commit, node)),
                Ok(_) => {}
                Err(reason) => scan.unverifiable.push((commit, reason)),
            }
        }
        Ok(scan)
    }

    // --- Deploy refs (standalone blob refs) ---

    fn deploy_ref(step_name: &str, env: &str) -> String {
        format!(
            "{DEPLOY_PREFIX}{}/{}",
            Self::encode_segment(step_name),
            Self::encode_segment(env)
        )
    }

    /// Write a deploy attestation payload as a standalone blob ref.
    pub fn put_deploy_ref(&self, step_name: &str, env: &str, payload: &serde_json::Value) -> Result<(), RefError> {
        self.write_ref_blob(&Self::deploy_ref(step_name, env), payload)
    }

    /// The deploy attestation for `step_name` at `env`; it binds to both.
    pub fn get_deploy_ref(&self, step_name: &str, env: &str) -> Result<Lookup<DeployAttestationContentNode>, RefError> {
        self.read_blob_ref(&Self::deploy_ref(step_name, env), |n: &DeployAttestationContentNode| {
            bound("stepName", &n.step_name, step_name)?;
            bound("environmentLabel", &n.environment_label, env)
        })
    }

    /// List all deploy ref names under `refs/notes/brit/deploy/`.
    pub fn list_deploy_refs(&self, _pattern: Option<&str>) -> Result<Vec<String>, RefError> {
        self.list_refs_under(DEPLOY_PREFIX)
    }

    // --- Validate refs ---

    fn validate_ref(step_name: &str, check_name: &str) -> String {
        format!(
            "{VALIDATE_PREFIX}{}/{}",
            Self::encode_segment(step_name),
            Self::encode_segment(check_name)
        )
    }

    /// Write a validation attestation payload as a standalone blob ref.
    pub fn put_validate_ref(
        &self,
        step_name: &str,
        check_name: &str,
        payload: &serde_json::Value,
    ) -> Result<(), RefError> {
        self.write_ref_blob(&Self::validate_ref(step_name, check_name), payload)
    }

    /// The validation attestation for `check_name` under `step_name`.
    ///
    /// It binds to its check. The node carries no step, so the step segment of the ref is the
    /// ref's own claim, not the signer's.
    pub fn get_validate_ref(
        &self,
        step_name: &str,
        check_name: &str,
    ) -> Result<Lookup<ValidationAttestationContentNode>, RefError> {
        self.read_blob_ref(
            &Self::validate_ref(step_name, check_name),
            |n: &ValidationAttestationContentNode| bound("checkName", &n.check_name, check_name),
        )
    }

    /// List all validate ref names under `refs/notes/brit/validate/`.
    pub fn list_validate_refs(&self, _pattern: Option<&str>) -> Result<Vec<String>, RefError> {
        self.list_refs_under(VALIDATE_PREFIX)
    }

    // --- Covenant refs (append-only, one blob ref per clause) ---

    /// Record a covenant clause at `refs/notes/brit/covenant/<session>/<clause-cid>`.
    ///
    /// The blob is `{"cid": "<clause_cid_str>", "clause": <clause_json>}`. Append-only: a clause
    /// is addressed by its CID, so an existing ref with identical content is a no-op and one with
    /// different content is refused with [`RefError::Exists`]; nothing is ever overwritten.
    pub fn put_covenant_clause(
        &self,
        session: &str,
        clause_cid_str: &str,
        clause_json: &serde_json::Value,
    ) -> Result<(), RefError> {
        let ref_name = Self::covenant_ref(session, clause_cid_str);
        let record = serde_json::json!({ "cid": clause_cid_str, "clause": clause_json });
        if let Some(oid) = self.ref_oid(&ref_name)? {
            let existing = self
                .cat_blobs(vec![oid])?
                .remove(0)
                .map_err(|reason| RefError::GitFailed(format!("{ref_name}: {reason}")))?;
            let existing: serde_json::Value = serde_json::from_slice(&existing).map_err(RefError::Json)?;
            return if existing == record {
                Ok(())
            } else {
                Err(RefError::Exists(ref_name))
            };
        }
        self.write_ref_blob(&ref_name, &record)
    }

    /// Every verified clause recorded for `session`, ordered by the clause's `validFrom` (ties
    /// broken by CID), plus every record under it that failed verification. An unknown session
    /// is an empty scan.
    pub fn list_covenant_clauses(&self, session: &str) -> Result<Scan<CovenantRecord>, RefError> {
        let prefix = format!("{COVENANT_PREFIX}{}/", Self::encode_segment(session));
        let refs: Vec<(String, String)> = self
            .for_each_ref(&prefix)?
            .into_iter()
            .filter(|(name, _)| name.strip_prefix(&prefix).is_some_and(|rest| !rest.contains('/')))
            .collect();
        let mut scan = self.covenant_scan(refs)?;
        scan.verified
            .sort_by(|(_, a), (_, b)| (&a.clause.valid_from, &a.cid).cmp(&(&b.clause.valid_from, &b.cid)));
        Ok(scan)
    }

    /// The record for clause `cid` in any session. Several refs can end in the same CID (a record
    /// copied under another session); the first that verifies answers.
    pub fn get_covenant_clause(&self, cid: &str) -> Result<Lookup<CovenantRecord>, RefError> {
        let suffix = format!("/{}", Self::encode_segment(cid));
        let refs: Vec<(String, String)> = self
            .for_each_ref(COVENANT_PREFIX)?
            .into_iter()
            .filter(|(name, _)| name.ends_with(&suffix))
            .collect();
        let mut scan = self.covenant_scan(refs)?;
        if !scan.verified.is_empty() {
            return Ok(Lookup::Verified(scan.verified.remove(0).1));
        }
        Ok(match scan.unverifiable.into_iter().next() {
            Some((at, reason)) => Lookup::Unverifiable { at, reason },
            None => Lookup::Absent,
        })
    }

    fn covenant_ref(session: &str, cid: &str) -> String {
        format!(
            "{COVENANT_PREFIX}{}/{}",
            Self::encode_segment(session),
            Self::encode_segment(cid)
        )
    }

    fn covenant_scan(&self, refs: Vec<(String, String)>) -> Result<Scan<CovenantRecord>, RefError> {
        let bodies = self.cat_blobs(refs.iter().map(|(_, oid)| oid.clone()).collect())?;
        let mut scan = Scan::default();
        for ((name, _), body) in refs.into_iter().zip(bodies) {
            let verified = body.and_then(|b| {
                let (session_segment, cid_segment) = name
                    .strip_prefix(COVENANT_PREFIX)
                    .and_then(|rest| rest.rsplit_once('/'))
                    .ok_or_else(|| "not a <session>/<cid> covenant ref".to_string())?;
                read_covenant(&b, session_segment, cid_segment)
            });
            match verified {
                Ok(record) => scan.verified.push((name, record)),
                Err(reason) => scan.unverifiable.push((name, reason)),
            }
        }
        Ok(scan)
    }

    // --- Internal helpers ---

    /// A standalone blob ref, verified and bound.
    fn read_blob_ref<T: Signed>(
        &self,
        ref_name: &str,
        bind: impl FnOnce(&T) -> Result<(), String>,
    ) -> Result<Lookup<T>, RefError> {
        let Some(oid) = self.ref_oid(ref_name)? else {
            return Ok(Lookup::Absent);
        };
        let body = self.cat_blobs(vec![oid])?.remove(0);
        Ok(match body.and_then(|b| read_verified(&b, bind)) {
            Ok(node) => Lookup::Verified(node),
            Err(reason) => Lookup::Unverifiable {
                at: ref_name.to_string(),
                reason,
            },
        })
    }

    /// The object `ref_name` points at, or `None` when git lists no such ref.
    fn ref_oid(&self, ref_name: &str) -> Result<Option<String>, RefError> {
        Ok(self
            .for_each_ref(ref_name)?
            .into_iter()
            .find(|(name, _)| name == ref_name)
            .map(|(_, oid)| oid))
    }

    /// `(note object, annotated object)` for every note under `ref_name`. A notes ref that does
    /// not exist lists empty (git answers that itself); a notes ref git cannot read is an error.
    fn notes(&self, ref_name: &str) -> Result<Vec<(String, String)>, RefError> {
        let text = self.git_text(&["notes", "--ref", ref_name, "list"])?;
        text.lines()
            .filter(|line| !line.is_empty())
            .map(|line| match line.split_once(' ') {
                Some((note, annotated)) if !note.is_empty() && !annotated.is_empty() && !annotated.contains(' ') => {
                    Ok((note.to_string(), annotated.to_string()))
                }
                _ => Err(RefError::GitFailed(format!(
                    "notes list {ref_name}: unexpected line `{line}`"
                ))),
            })
            .collect()
    }

    /// Encode a ref path segment, replacing characters git rejects in ref names.
    ///
    /// Per `git check-ref-format`, ref names cannot contain: `:`, `@{`, `?`,
    /// `*`, `[`, `\`, `^`, `~`, space, DEL, or control characters. They also
    /// cannot contain `..` or end with `.lock`. We percent-encode a superset
    /// of forbidden characters to be safe.
    fn encode_segment(s: &str) -> String {
        let mut out = String::with_capacity(s.len());
        for c in s.chars() {
            match c {
                ':' => out.push_str("%3A"),
                '@' => out.push_str("%40"),
                '?' => out.push_str("%3F"),
                '*' => out.push_str("%2A"),
                '[' => out.push_str("%5B"),
                '\\' => out.push_str("%5C"),
                '^' => out.push_str("%5E"),
                '~' => out.push_str("%7E"),
                ' ' => out.push_str("%20"),
                c if c.is_ascii_control() => {
                    use std::fmt::Write;
                    let _ = write!(out, "%{:02X}", c as u8);
                }
                _ => out.push(c),
            }
        }
        // Also handle ".." which git forbids in ref names
        out.replace("..", "%2E%2E")
    }

    /// Decode a percent-encoded ref path segment back to its original form.
    fn decode_segment(s: &str) -> String {
        s.replace("%3A", ":")
            .replace("%40", "@")
            .replace("%3F", "?")
            .replace("%2A", "*")
            .replace("%5B", "[")
            .replace("%5C", "\\")
            .replace("%5E", "^")
            .replace("%7E", "~")
            .replace("%20", " ")
            .replace("%2E%2E", "..")
    }

    fn resolve_rev(&self, rev: &str) -> Result<String, RefError> {
        let output = Command::new("git")
            .args(["rev-parse", rev])
            .current_dir(&self.repo_path)
            .output()
            .map_err(RefError::GitCommand)?;
        if !output.status.success() {
            return Err(RefError::RevNotFound(rev.to_string()));
        }
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    }

    fn write_note(&self, ref_name: &str, commit_sha: &str, payload: &serde_json::Value) -> Result<(), RefError> {
        let json = serde_json::to_string(payload).map_err(RefError::Json)?;
        // `git notes add` creates a notes commit, which needs a committer identity.
        // brit is designed to run in CI (Jenkins) where ambient git identity is
        // often unset, so supply a deterministic one via `-c` rather than relying
        // on the environment.
        let output = Command::new("git")
            .args([
                "-c",
                "user.name=brit",
                "-c",
                "user.email=brit@ethosengine.com",
                "notes",
                "--ref",
                ref_name,
                "add",
                "-f",
                "-m",
                &json,
                commit_sha,
            ])
            .current_dir(&self.repo_path)
            .output()
            .map_err(RefError::GitCommand)?;
        if !output.status.success() {
            return Err(RefError::GitFailed(format!(
                "notes add to {ref_name}: {}",
                String::from_utf8_lossy(&output.stderr)
            )));
        }
        Ok(())
    }

    fn write_ref_blob(&self, ref_name: &str, payload: &serde_json::Value) -> Result<(), RefError> {
        let json = serde_json::to_string(payload).map_err(RefError::Json)?;
        let mut child = Command::new("git")
            .args(["hash-object", "-w", "--stdin"])
            .current_dir(&self.repo_path)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .spawn()
            .map_err(RefError::GitCommand)?;
        {
            use std::io::Write;
            let mut stdin = child
                .stdin
                .take()
                .ok_or_else(|| RefError::GitFailed("stdin pipe unavailable".into()))?;
            stdin.write_all(json.as_bytes()).map_err(RefError::GitCommand)?;
        }
        let hash_output = child.wait_with_output().map_err(RefError::GitCommand)?;
        if !hash_output.status.success() {
            return Err(RefError::GitFailed("hash-object failed".into()));
        }
        let blob_sha = String::from_utf8_lossy(&hash_output.stdout).trim().to_string();

        let update_output = Command::new("git")
            .args(["update-ref", ref_name, &blob_sha])
            .current_dir(&self.repo_path)
            .output()
            .map_err(RefError::GitCommand)?;
        if !update_output.status.success() {
            return Err(RefError::GitFailed(format!(
                "update-ref {ref_name}: {}",
                String::from_utf8_lossy(&update_output.stderr)
            )));
        }
        Ok(())
    }

    /// Run git in the repository; its stdout as UTF-8, or an error naming its stderr.
    fn git_text(&self, args: &[&str]) -> Result<String, RefError> {
        let output = Command::new("git")
            .args(args)
            .current_dir(&self.repo_path)
            .output()
            .map_err(RefError::GitCommand)?;
        if !output.status.success() {
            return Err(RefError::GitFailed(format!(
                "git {}: {}",
                args.join(" "),
                String::from_utf8_lossy(&output.stderr).trim()
            )));
        }
        String::from_utf8(output.stdout)
            .map_err(|_| RefError::GitFailed(format!("git {}: output is not UTF-8", args.join(" "))))
    }

    /// The bytes of each object, in order, through one `git cat-file --batch`. An object that is
    /// not a blob is `Err(reason)` in its slot; an object git cannot find fails the whole read.
    /// The oids are written from a separate thread so a long listing cannot deadlock against a
    /// full stdout pipe.
    fn cat_blobs(&self, oids: Vec<String>) -> Result<Vec<Result<Vec<u8>, String>>, RefError> {
        use std::{io::Write, process::Stdio};

        if oids.is_empty() {
            return Ok(Vec::new());
        }
        let expected = oids.len();
        let mut child = Command::new("git")
            .args(["cat-file", "--batch"])
            .current_dir(&self.repo_path)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(RefError::GitCommand)?;
        let mut stdin = child
            .stdin
            .take()
            .ok_or_else(|| RefError::GitFailed("stdin pipe unavailable".into()))?;
        let writer = std::thread::spawn(move || -> std::io::Result<()> {
            for oid in oids {
                writeln!(stdin, "{oid}")?;
            }
            Ok(())
        });
        let output = child.wait_with_output().map_err(RefError::GitCommand)?;
        writer
            .join()
            .map_err(|_| RefError::GitFailed("cat-file --batch: writer thread panicked".into()))?
            .map_err(RefError::GitCommand)?;
        if !output.status.success() {
            return Err(RefError::GitFailed(format!(
                "cat-file --batch: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            )));
        }
        // Each entry is `<oid> <type> <size>\n`, `size` bytes, then one `\n`; an object git
        // cannot find prints `<oid> missing\n` (or `ambiguous`) and nothing else.
        let bytes = output.stdout;
        let mut pos = 0;
        let mut out = Vec::with_capacity(expected);
        while pos < bytes.len() {
            let nl = bytes[pos..]
                .iter()
                .position(|&b| b == b'\n')
                .ok_or_else(|| RefError::GitFailed("cat-file --batch: truncated header".into()))?;
            let header = std::str::from_utf8(&bytes[pos..pos + nl])
                .map_err(|_| RefError::GitFailed("cat-file --batch: header is not UTF-8".into()))?;
            pos += nl + 1;
            let fields: Vec<&str> = header.split(' ').collect();
            let [oid, kind, size] = fields[..] else {
                return Err(RefError::GitFailed(format!("cat-file --batch: cannot read `{header}`")));
            };
            let size: usize = size
                .parse()
                .map_err(|_| RefError::GitFailed(format!("cat-file --batch: bad size in `{header}`")))?;
            let end = pos + size;
            if end >= bytes.len() || bytes[end] != b'\n' {
                return Err(RefError::GitFailed(format!("cat-file --batch: truncated object {oid}")));
            }
            out.push(if kind == "blob" {
                Ok(bytes[pos..end].to_vec())
            } else {
                Err(format!("object {oid} is a {kind}, not a blob"))
            });
            pos = end + 1;
        }
        if out.len() != expected {
            return Err(RefError::GitFailed(format!(
                "cat-file --batch: {expected} objects asked, {} read",
                out.len()
            )));
        }
        Ok(out)
    }

    /// `(refname, object id)` for every ref under `prefix`, refnames undecoded.
    fn for_each_ref(&self, prefix: &str) -> Result<Vec<(String, String)>, RefError> {
        let text = self.git_text(&["for-each-ref", "--format=%(refname) %(objectname)", prefix])?;
        text.lines()
            .filter(|line| !line.is_empty())
            .map(|line| {
                line.split_once(' ')
                    .map(|(n, o)| (n.to_string(), o.to_string()))
                    .ok_or_else(|| RefError::GitFailed(format!("for-each-ref: unexpected line `{line}`")))
            })
            .collect()
    }

    fn list_refs_under(&self, prefix: &str) -> Result<Vec<String>, RefError> {
        Ok(self
            .for_each_ref(prefix)?
            .into_iter()
            .filter_map(|(name, _)| name.strip_prefix(prefix).map(Self::decode_segment))
            .collect())
    }
}

/// Errors returned by [`BritRefManager`] operations.
#[derive(Debug, thiserror::Error)]
pub enum RefError {
    /// The given path is not a git repository.
    #[error("not a git repository: {0}")]
    NotARepo(String),
    /// A git revision could not be resolved.
    #[error("rev not found: {0}")]
    RevNotFound(String),
    /// Failed to spawn or communicate with a git subprocess.
    #[error("git command error: {0}")]
    GitCommand(std::io::Error),
    /// A git subprocess exited with a non-zero status, or its output could not be read.
    #[error("git command failed: {0}")]
    GitFailed(String),
    /// An append-only ref already exists with different content.
    #[error("ref already exists with different content: {0}")]
    Exists(String),
    /// JSON serialization or deserialization failed.
    #[error("JSON error: {0}")]
    Json(serde_json::Error),
}
