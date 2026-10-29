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
//!
//! Every write refuses what the verified read would refuse, before any git command runs. It then
//! publishes only against the prior it read (`git update-ref <ref> <new> <old>`), never over
//! whatever is there by then, and is `Ok` only once what it published reads back, verified and
//! equal to what it wrote: a write acknowledgement is not evidence.

use std::{
    path::{Path, PathBuf},
    process::Command,
};

use crate::{
    elohim::attestation::{
        build::BuildAttestationContentNode,
        covenant::{CovenantClauseContentNode, CovenantRecord},
        deploy::DeployAttestationContentNode,
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

/// Where a notes write builds its new notes commit before publishing it. git builds notes commits
/// only on refs under `refs/notes/`, so the private ref lives here — outside `refs/notes/brit/`,
/// where no brit listing and no `refs/notes/brit/*` refspec sees it.
const SCRATCH_PREFIX: &str = "refs/notes/brit-tmp/";
/// A scratch ref older than this was left by a writer that died; the next writer sweeps it.
const SCRATCH_TTL_SECS: u64 = 3600;
/// Publications a write attempts before it refuses.
const MAX_ATTEMPTS: u32 = 3;
/// The identity on every notes commit. Who signed a record is in the record, verified; a commit
/// field naming a signer would be an unverified identity claim beside it.
const NOTES_IDENT: (&str, &str) = ("brit", "notes@brit.invalid");

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

/// What a blob write expects to find before it publishes.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Prior {
    /// Whatever it observed there: replace it, unless it moved.
    Replace,
    /// Nothing: create only.
    Absent,
}

/// The bytes a record is stored as: its JSON value, compact (the form brit has always written).
fn record_bytes<T: serde::Serialize>(ref_name: &str, node: &T) -> Result<Vec<u8>, RefError> {
    let value = serde_json::to_value(node).map_err(RefError::Json)?;
    serde_json::to_vec(&value).map_err(|e| RefError::WouldNotVerify {
        ref_name: ref_name.to_string(),
        reason: format!("does not encode: {e}"),
    })
}

/// A write proceeds only if the verified read of its own bytes returns the node being written.
fn admissible<T: PartialEq>(ref_name: &str, node: &T, read: Result<T, String>) -> Result<(), RefError> {
    match read {
        Ok(back) if back == *node => Ok(()),
        Ok(_) => Err(RefError::WouldNotVerify {
            ref_name: ref_name.to_string(),
            reason: "does not decode back to the record given".into(),
        }),
        Err(reason) => Err(RefError::WouldNotVerify {
            ref_name: ref_name.to_string(),
            reason,
        }),
    }
}

fn unix_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

/// When a scratch ref named `<pid>-<unix secs>-<hex>` was created; `None` for any other name.
fn scratch_created(name: &str) -> Option<u64> {
    let mut parts = name.split('-');
    let (Some(pid), Some(secs), Some(nonce), None) = (parts.next(), parts.next(), parts.next(), parts.next()) else {
        return None;
    };
    let digits = |s: &str| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit());
    (digits(pid) && !nonce.is_empty() && nonce.bytes().all(|b| b.is_ascii_hexdigit()))
        .then(|| secs.parse().ok())
        .flatten()
}

/// A notes write's private ref, deleted when the write ends however it ends.
struct Scratch<'a> {
    refs: &'a BritRefManager,
    name: String,
}

impl<'a> Scratch<'a> {
    /// A fresh scratch ref holding `head` (create-only), or not yet existing when there is no
    /// head (`git notes add` then creates it with a root commit).
    fn create(refs: &'a BritRefManager, head: Option<&str>, zero: &str) -> Result<Self, RefError> {
        let scratch = Self {
            refs,
            name: format!(
                "{SCRATCH_PREFIX}{}-{}-{:08x}",
                std::process::id(),
                unix_now(),
                rand::random::<u32>()
            ),
        };
        if let Some(head) = head {
            refs.update_ref(&scratch.name, head, zero)?
                .map_err(|stderr| RefError::GitFailed(format!("update-ref {}: {stderr}", scratch.name)))?;
        }
        Ok(scratch)
    }
}

impl Drop for Scratch<'_> {
    fn drop(&mut self) {
        // It may not exist (no head, and `notes add` failed); either way it is gone after this.
        let _ = Command::new("git")
            .args(["update-ref", "-d", &self.name])
            .current_dir(&self.refs.repo_path)
            .output();
    }
}

/// A build note: verified, and bound to the step whose ref it was found under.
fn read_build(bytes: &[u8], step_name: &str) -> Result<BuildAttestationContentNode, String> {
    read_verified(bytes, |n: &BuildAttestationContentNode| {
        bound("stepName", &n.step_name, step_name)
    })
}

/// A deploy attestation: verified, and bound to the step and environment of its ref.
fn read_deploy(bytes: &[u8], step_name: &str, env: &str) -> Result<DeployAttestationContentNode, String> {
    read_verified(bytes, |n: &DeployAttestationContentNode| {
        bound("stepName", &n.step_name, step_name)?;
        bound("environmentLabel", &n.environment_label, env)
    })
}

/// A validation attestation: verified, and bound to the check of its ref. The node carries no
/// step, so the step segment of the ref is the ref's own claim, not the signer's.
fn read_validation(bytes: &[u8], check_name: &str) -> Result<ValidationAttestationContentNode, String> {
    read_verified(bytes, |n: &ValidationAttestationContentNode| {
        bound("checkName", &n.check_name, check_name)
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

/// Called as `hook(ref_name, attempt)` before a write publishes; see [`BritRefManager::before_publish`].
type PublishHook = std::sync::Arc<dyn Fn(&str, u32) + Send + Sync>;

/// Manages git refs under `refs/notes/brit/` for attestation indexing.
pub struct BritRefManager {
    repo_path: PathBuf,
    before_publish: Option<PublishHook>,
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
            before_publish: None,
        })
    }

    /// Run `hook(ref_name, attempt)` each time a write has read the prior it expects and built
    /// what it will publish, immediately before it publishes. Nothing in brit sets it: it is the
    /// seam a test uses to lose a race deterministically, by publishing underneath the writer.
    #[doc(hidden)] // test seam: not part of the supported API
    pub fn before_publish(mut self, hook: impl Fn(&str, u32) + Send + Sync + 'static) -> Self {
        self.before_publish = Some(std::sync::Arc::new(hook));
        self
    }

    fn publishing(&self, ref_name: &str, attempt: u32) {
        if let Some(hook) = &self.before_publish {
            hook(ref_name, attempt);
        }
    }

    // --- Build refs (per-commit via git notes) ---

    fn build_ref(step_name: &str) -> String {
        format!("{BUILD_PREFIX}{}", Self::encode_segment(step_name))
    }

    /// Write a build attestation as a git note on `commit_rev`.
    ///
    /// The node must pass the read this ref gets — strict decode, its signature, `stepName ==
    /// step_name` — or nothing is written and no git command runs. The note is published against
    /// the notes head it was built on. A race lost to a note on another commit is rebuilt on the
    /// new head (at most [`MAX_ATTEMPTS`] publications); a race lost to a change of this commit's
    /// own note is refused with [`RefError::Concurrent`], unless that change wrote these same bytes.
    pub fn put_build_ref(
        &self,
        step_name: &str,
        commit_rev: &str,
        node: &BuildAttestationContentNode,
    ) -> Result<(), RefError> {
        let ref_name = Self::build_ref(step_name);
        let read = |b: &[u8]| read_build(b, step_name);
        // A note's bytes as `git notes add -m` has always stored them: the JSON, then a newline.
        let mut bytes = record_bytes(&ref_name, node)?;
        bytes.push(b'\n');
        admissible(&ref_name, node, read(&bytes))?;
        let commit_sha = self.resolve_rev(commit_rev)?;
        self.write_note(&ref_name, &commit_sha, &bytes, read, node)
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

    /// Write a deploy attestation as a standalone blob ref, replacing the one it observed there.
    ///
    /// The node must bind to `step_name` and `env` and verify, or nothing is written and no git
    /// command runs. If the ref moves after it was observed, the write is refused, never retried
    /// against the new value.
    pub fn put_deploy_ref(
        &self,
        step_name: &str,
        env: &str,
        node: &DeployAttestationContentNode,
    ) -> Result<(), RefError> {
        let ref_name = Self::deploy_ref(step_name, env);
        let read = |b: &[u8]| read_deploy(b, step_name, env);
        let bytes = record_bytes(&ref_name, node)?;
        admissible(&ref_name, node, read(&bytes))?;
        let observed = self.ref_oid(&ref_name)?;
        self.write_ref_blob(&ref_name, &bytes, observed, Prior::Replace, read, node)
    }

    /// The deploy attestation for `step_name` at `env`; it binds to both.
    pub fn get_deploy_ref(&self, step_name: &str, env: &str) -> Result<Lookup<DeployAttestationContentNode>, RefError> {
        self.read_blob_ref(&Self::deploy_ref(step_name, env), |b| read_deploy(b, step_name, env))
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

    /// Write a validation attestation as a standalone blob ref, replacing the one it observed
    /// there; it must bind to `check_name` and verify. Races are refused as for
    /// [`Self::put_deploy_ref`].
    pub fn put_validate_ref(
        &self,
        step_name: &str,
        check_name: &str,
        node: &ValidationAttestationContentNode,
    ) -> Result<(), RefError> {
        let ref_name = Self::validate_ref(step_name, check_name);
        let read = |b: &[u8]| read_validation(b, check_name);
        let bytes = record_bytes(&ref_name, node)?;
        admissible(&ref_name, node, read(&bytes))?;
        let observed = self.ref_oid(&ref_name)?;
        self.write_ref_blob(&ref_name, &bytes, observed, Prior::Replace, read, node)
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
        self.read_blob_ref(&Self::validate_ref(step_name, check_name), |b| {
            read_validation(b, check_name)
        })
    }

    /// List all validate ref names under `refs/notes/brit/validate/`.
    pub fn list_validate_refs(&self, _pattern: Option<&str>) -> Result<Vec<String>, RefError> {
        self.list_refs_under(VALIDATE_PREFIX)
    }

    // --- Covenant refs (append-only, one blob ref per clause) ---

    /// Record a covenant clause at `refs/notes/brit/covenant/<session>/<clause-cid>`; its CID.
    ///
    /// The blob is `{"cid": "<clause cid>", "clause": <clause>}`, the CID computed here from the
    /// clause. The clause must verify, or nothing is written and no git command runs. Append-only:
    /// the ref is created against the absence this write observed, so a ref that holds these same
    /// bytes is a no-op, one that holds other bytes is refused with [`RefError::Exists`], and one
    /// created underneath this write is refused with [`RefError::Concurrent`]. Nothing is ever
    /// overwritten.
    pub fn put_covenant_clause(&self, clause: &CovenantClauseContentNode) -> Result<String, RefError> {
        let cid = clause
            .compute_cid()
            .map_err(|e| RefError::WouldNotVerify {
                ref_name: format!("{COVENANT_PREFIX}{}", Self::encode_segment(&clause.session)),
                reason: format!("clause does not encode: {e}"),
            })?
            .to_string();
        let ref_name = Self::covenant_ref(&clause.session, &cid);
        let (session_segment, cid_segment) = (Self::encode_segment(&clause.session), Self::encode_segment(&cid));
        let read = |b: &[u8]| read_covenant(b, &session_segment, &cid_segment);
        let record = CovenantRecord {
            cid: cid.clone(),
            clause: clause.clone(),
        };
        let bytes = record_bytes(&ref_name, &record)?;
        admissible(&ref_name, &record, read(&bytes))?;
        let observed = self.ref_oid(&ref_name)?;
        self.write_ref_blob(&ref_name, &bytes, observed, Prior::Absent, read, &record)?;
        Ok(cid)
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

    /// A standalone blob ref, through its verified `read`.
    fn read_blob_ref<T>(
        &self,
        ref_name: &str,
        read: impl FnOnce(&[u8]) -> Result<T, String>,
    ) -> Result<Lookup<T>, RefError> {
        let Some(oid) = self.ref_oid(ref_name)? else {
            return Ok(Lookup::Absent);
        };
        let body = self.cat_blobs(vec![oid])?.remove(0);
        Ok(match body.and_then(|b| read(&b)) {
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

    /// Note `bytes` on `commit_sha` under `ref_name`, published against the notes head it was
    /// built on, then read back from the published head through `read`.
    fn write_note<T: PartialEq>(
        &self,
        ref_name: &str,
        commit_sha: &str,
        bytes: &[u8],
        read: impl Fn(&[u8]) -> Result<T, String>,
        node: &T,
    ) -> Result<(), RefError> {
        let zero = self.zero_oid()?;
        let blob = self.hash_object(bytes)?;
        self.sweep_scratch()?;
        // This commit's note as the first read saw it: absent, or a blob. Any later read that
        // finds it otherwise means another writer changed this very note.
        let mut entry_seen: Option<Option<String>> = None;
        let mut published_against = zero.clone();
        let mut last_failure = String::new();
        let mut attempts = 0;
        loop {
            let head = self.ref_oid(ref_name)?;
            let entry = match &head {
                Some(head) => self.note_entry(head, commit_sha)?,
                None => None,
            };
            if let (Some(head), Some(entry)) = (&head, &entry) {
                if *entry == blob {
                    // These bytes are already the note: nothing to publish.
                    return self.read_back_note(ref_name, head, commit_sha, &blob, &read, node);
                }
            }
            let observed = head.clone().unwrap_or_else(|| zero.clone());
            let changed = entry_seen.as_ref().is_some_and(|seen| *seen != entry);
            if changed || attempts == MAX_ATTEMPTS {
                if !changed && observed == published_against {
                    return Err(RefError::GitFailed(format!(
                        "update-ref {ref_name} failed {attempts} times with the ref unmoved: {last_failure}"
                    )));
                }
                return Err(RefError::Concurrent {
                    ref_name: ref_name.to_string(),
                    expected: published_against,
                    observed,
                    attempts,
                });
            }
            entry_seen.get_or_insert(entry);
            attempts += 1;

            let scratch = Scratch::create(self, head.as_deref(), &zero)?;
            self.git_ok(
                &["notes", "--ref", &scratch.name, "add", "-f", "-C", &blob, commit_sha],
                &[
                    ("GIT_AUTHOR_NAME", NOTES_IDENT.0),
                    ("GIT_AUTHOR_EMAIL", NOTES_IDENT.1),
                    ("GIT_COMMITTER_NAME", NOTES_IDENT.0),
                    ("GIT_COMMITTER_EMAIL", NOTES_IDENT.1),
                ],
            )?;
            let new = self
                .git_text(&["rev-parse", "--verify", &scratch.name])?
                .trim()
                .to_string();
            self.publishing(ref_name, attempts);
            published_against = observed;
            match self.update_ref(ref_name, &new, &published_against)? {
                Ok(()) => {
                    drop(scratch);
                    return self.read_back_note(ref_name, &new, commit_sha, &blob, &read, node);
                }
                // Lost, or the ref was locked: the next pass re-reads the head and decides.
                Err(stderr) => last_failure = stderr,
            }
        }
    }

    /// The note blob on `annotated` in the notes commit `notes_commit`, or `None` when it has none.
    ///
    /// A notes tree may fan an object's path out into directories (`ab/cdef…`); the entry is the
    /// one whose path, slashes removed, is the annotated object's id.
    fn note_entry(&self, notes_commit: &str, annotated: &str) -> Result<Option<String>, RefError> {
        let listing = self.git_text(&["ls-tree", "-r", "-z", notes_commit])?;
        for record in listing.split('\0').filter(|r| !r.is_empty()) {
            let (meta, path) = record
                .split_once('\t')
                .ok_or_else(|| RefError::GitFailed(format!("ls-tree {notes_commit}: unexpected entry `{record}`")))?;
            let mut fields = meta.split(' ');
            let (Some(_mode), Some(kind), Some(oid)) = (fields.next(), fields.next(), fields.next()) else {
                return Err(RefError::GitFailed(format!(
                    "ls-tree {notes_commit}: unexpected entry `{record}`"
                )));
            };
            if kind == "blob" && path.replace('/', "") == annotated {
                return Ok(Some(oid.to_string()));
            }
        }
        Ok(None)
    }

    /// `Ok` only if the notes commit this write published carries `blob` on `commit_sha` and
    /// those bytes pass `read` as the node written.
    fn read_back_note<T: PartialEq>(
        &self,
        ref_name: &str,
        published: &str,
        commit_sha: &str,
        blob: &str,
        read: impl Fn(&[u8]) -> Result<T, String>,
        node: &T,
    ) -> Result<(), RefError> {
        match self.note_entry(published, commit_sha)? {
            Some(entry) if entry == blob => self.read_back_blob(ref_name, blob, read, node),
            other => Err(RefError::ReadBack {
                ref_name: ref_name.to_string(),
                reason: format!("notes commit {published} holds {other:?} for {commit_sha}, not {blob}"),
            }),
        }
    }

    /// `Ok` only if the object `blob` passes `read` as `node`.
    fn read_back_blob<T: PartialEq>(
        &self,
        ref_name: &str,
        blob: &str,
        read: impl Fn(&[u8]) -> Result<T, String>,
        node: &T,
    ) -> Result<(), RefError> {
        let read_back = |reason: String| RefError::ReadBack {
            ref_name: ref_name.to_string(),
            reason,
        };
        let bytes = self.cat_blobs(vec![blob.to_string()])?.remove(0).map_err(read_back)?;
        match read(&bytes) {
            Ok(back) if back == *node => Ok(()),
            Ok(_) => Err(read_back(format!("{blob} verifies but is not the record written"))),
            Err(reason) => Err(read_back(reason)),
        }
    }

    /// Point `ref_name` at a blob of `bytes`, against `observed` — what this write saw there. A ref
    /// that moved is refused, never retried against its new value; a ref that is already these
    /// bytes is a no-op. With [`Prior::Absent`] an existing ref of other bytes is
    /// [`RefError::Exists`].
    fn write_ref_blob<T: PartialEq>(
        &self,
        ref_name: &str,
        bytes: &[u8],
        observed: Option<String>,
        prior: Prior,
        read: impl Fn(&[u8]) -> Result<T, String>,
        node: &T,
    ) -> Result<(), RefError> {
        let zero = self.zero_oid()?;
        let blob = self.hash_object(bytes)?;
        if observed.as_deref() == Some(blob.as_str()) {
            return self.read_back_blob(ref_name, &blob, read, node);
        }
        if prior == Prior::Absent && observed.is_some() {
            return Err(RefError::Exists(ref_name.to_string()));
        }
        let expected = observed.clone().unwrap_or_else(|| zero.clone());
        let mut current = observed.clone();
        let mut last_failure = String::new();
        let mut attempts = 0;
        loop {
            if current.as_deref() == Some(blob.as_str()) {
                // Another writer published these same bytes.
                return self.read_back_blob(ref_name, &blob, read, node);
            }
            if current != observed {
                return Err(RefError::Concurrent {
                    ref_name: ref_name.to_string(),
                    expected,
                    observed: current.unwrap_or(zero),
                    attempts,
                });
            }
            if attempts == MAX_ATTEMPTS {
                return Err(RefError::GitFailed(format!(
                    "update-ref {ref_name} failed {attempts} times with the ref unmoved: {last_failure}"
                )));
            }
            attempts += 1;
            self.publishing(ref_name, attempts);
            match self.update_ref(ref_name, &blob, &expected)? {
                Ok(()) => return self.read_back_blob(ref_name, &blob, read, node),
                Err(stderr) => {
                    last_failure = stderr;
                    current = self.ref_oid(ref_name)?;
                }
            }
        }
    }

    /// `git update-ref <ref_name> <new> <expected>`: `Ok(Err(stderr))` when git declined it (the
    /// ref moved, or was locked), `Err` when git could not be run.
    fn update_ref(&self, ref_name: &str, new: &str, expected: &str) -> Result<Result<(), String>, RefError> {
        let output = Command::new("git")
            .args(["update-ref", ref_name, new, expected])
            .current_dir(&self.repo_path)
            .output()
            .map_err(RefError::GitCommand)?;
        Ok(if output.status.success() {
            Ok(())
        } else {
            Err(String::from_utf8_lossy(&output.stderr).trim().to_string())
        })
    }

    /// The all-zero object id in this repository's object format: what `update-ref` takes as
    /// "this ref must not exist".
    fn zero_oid(&self) -> Result<String, RefError> {
        match self.git_text(&["rev-parse", "--show-object-format"])?.trim() {
            "sha1" => Ok("0".repeat(40)),
            "sha256" => Ok("0".repeat(64)),
            other => Err(RefError::GitFailed(format!("unknown object format `{other}`"))),
        }
    }

    /// Store `bytes` as a blob; its object id.
    fn hash_object(&self, bytes: &[u8]) -> Result<String, RefError> {
        use std::{io::Write, process::Stdio};

        let mut child = Command::new("git")
            .args(["hash-object", "-w", "--stdin"])
            .current_dir(&self.repo_path)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(RefError::GitCommand)?;
        {
            let mut stdin = child
                .stdin
                .take()
                .ok_or_else(|| RefError::GitFailed("stdin pipe unavailable".into()))?;
            stdin.write_all(bytes).map_err(RefError::GitCommand)?;
        }
        let output = child.wait_with_output().map_err(RefError::GitCommand)?;
        if !output.status.success() {
            return Err(RefError::GitFailed(format!(
                "hash-object: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            )));
        }
        String::from_utf8(output.stdout)
            .map(|oid| oid.trim().to_string())
            .map_err(|_| RefError::GitFailed("hash-object: output is not UTF-8".into()))
    }

    /// Delete scratch refs that a writer created more than [`SCRATCH_TTL_SECS`] ago and never
    /// removed (it died mid-write). A ref here that is not named `<pid>-<unix secs>-<hex>` is not
    /// brit's and is left alone; a delete that loses to another sweeper is no loss.
    fn sweep_scratch(&self) -> Result<(), RefError> {
        let now = unix_now();
        for (name, oid) in self.for_each_ref(SCRATCH_PREFIX)? {
            let Some(created) = name.strip_prefix(SCRATCH_PREFIX).and_then(scratch_created) else {
                continue;
            };
            if now.saturating_sub(created) > SCRATCH_TTL_SECS {
                let _ = Command::new("git")
                    .args(["update-ref", "-d", &name, &oid])
                    .current_dir(&self.repo_path)
                    .output();
            }
        }
        Ok(())
    }

    /// Run git with `env` set; an error naming its stderr when it fails.
    fn git_ok(&self, args: &[&str], env: &[(&str, &str)]) -> Result<(), RefError> {
        let output = Command::new("git")
            .args(args)
            .envs(env.iter().copied())
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
    /// The record would not pass the verified read, so it is not written. No git command ran.
    #[error("refusing to write {ref_name}: {reason}")]
    WouldNotVerify {
        /// The ref the record was for.
        ref_name: String,
        /// Why the verified read would refuse it.
        reason: String,
    },
    /// Another writer moved the ref underneath this one and the write was not retried onto it.
    #[error("REFUSED concurrent update: ref={ref_name} expected={expected} observed={observed} attempts={attempts}")]
    Concurrent {
        /// The ref being published.
        ref_name: String,
        /// The prior this write read and published against (the zero oid when absent).
        expected: String,
        /// What the ref held when the write gave up (the zero oid when absent).
        observed: String,
        /// Publication attempts made.
        attempts: u32,
    },
    /// The write was published but does not read back, verified and equal, from what it published.
    #[error("wrote {ref_name} but it does not read back as written: {reason}")]
    ReadBack {
        /// The ref published.
        ref_name: String,
        /// What the read-back found.
        reason: String,
    },
}
