//! Writes publish only against the prior they read, and are Ok only once what they published
//! reads back verified.
//!
//! Every race here is forced through `BritRefManager::before_publish`, the seam that runs after a
//! writer has read its expected prior and built its new head and before it publishes. A second
//! writer publishes underneath it there, with barriers or inline, never with sleeps.

use std::{
    path::Path,
    process::Command,
    sync::{
        atomic::{AtomicU32, Ordering},
        Arc, Barrier,
    },
};

use brit_epr::{
    elohim::{
        attestation::{
            build::BuildAttestationContentNode,
            covenant::{CovenantClauseContentNode, CovenantClauseKind},
            deploy::{DeployAttestationContentNode, HealthStatus},
            validation::{ValidationAttestationContentNode, ValidationResult},
        },
        refs::{BritRefManager, Lookup, RefError},
    },
    engine::{cid::BritCid, content_node::ContentNode, signing::AgentKey},
};
use tempfile::TempDir;

const STEP: &str = "gate:x";
const NOTES: &str = "refs/notes/brit/build/gate%3Ax";
const SCRATCH: &str = "refs/notes/brit-tmp/";

fn git_status(dir: &Path, args: &[&str]) -> std::process::Output {
    Command::new("git")
        .args(["-c", "user.email=test@test.com", "-c", "user.name=test"])
        .args(args)
        .current_dir(dir)
        .output()
        .unwrap()
}

fn git(dir: &Path, args: &[&str]) -> String {
    let out = git_status(dir, args);
    assert!(
        out.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

fn repo_with(format: &str) -> TempDir {
    let tmp = TempDir::new().unwrap();
    git(
        tmp.path(),
        &["init", "--initial-branch=main", &format!("--object-format={format}")],
    );
    git(tmp.path(), &["commit", "--allow-empty", "-m", "init"]);
    tmp
}

fn repo() -> TempDir {
    repo_with("sha1")
}

fn commit(dir: &Path, message: &str) -> String {
    git(dir, &["commit", "--allow-empty", "-m", message]);
    git(dir, &["rev-parse", "HEAD"])
}

fn key(dir: &TempDir, name: &str) -> AgentKey {
    AgentKey::generate(&dir.path().join("keys").join(name)).unwrap()
}

fn build(key: &AgentKey, step: &str, inputs: &str) -> BuildAttestationContentNode {
    let mut node = BuildAttestationContentNode {
        manifest_cid: BritCid::compute_raw(b"manifest"),
        step_name: step.into(),
        inputs_hash: inputs.into(),
        output_cid: BritCid::compute_raw(b"output"),
        agent_id: key.agent_id(),
        hardware_profile: serde_json::json!({"arch": "x86_64"}),
        build_duration_ms: 1000,
        built_at: "2026-10-29T10:00:00Z".into(),
        success: true,
        signature: String::new(),
    };
    node.signature = key.sign(&node.canonical_bytes().unwrap());
    node
}

fn deploy(key: &AgentKey, at: &str) -> DeployAttestationContentNode {
    let mut node = DeployAttestationContentNode {
        artifact_cid: BritCid::compute_raw(b"artifact"),
        step_name: STEP.into(),
        environment_label: "staging".into(),
        endpoint: "https://staging.example".into(),
        health_check_epr: "epr:x/health".into(),
        health_status: HealthStatus::Healthy,
        deployed_at: at.into(),
        attested_at: at.into(),
        liveness_ttl_sec: 300,
        agent_id: key.agent_id(),
        signature: String::new(),
    };
    node.signature = key.sign(&node.canonical_bytes().unwrap());
    node
}

fn validation(key: &AgentKey, summary: &str) -> ValidationAttestationContentNode {
    let mut node = ValidationAttestationContentNode {
        artifact_cid: BritCid::compute_raw(b"artifact"),
        check_name: "lint".into(),
        validator_id: key.agent_id(),
        validator_version: "1".into(),
        result: ValidationResult::Pass,
        result_summary: summary.into(),
        findings_cid: None,
        validated_at: "2026-10-29T10:00:00Z".into(),
        ttl_sec: None,
        signature: String::new(),
    };
    node.signature = key.sign(&node.canonical_bytes().unwrap());
    node
}

fn clause(key: &AgentKey, quote: &str) -> CovenantClauseContentNode {
    let mut node = CovenantClauseContentNode {
        kind: CovenantClauseKind::DeclaresPurpose,
        session: "s1".into(),
        party: "human:t".into(),
        quote: quote.into(),
        payload: serde_json::json!({"purpose": "p", "writeSet": ["docs/**"]}),
        valid_from: "2026-10-29T10:00:00Z".into(),
        valid_until: None,
        supersedes: None,
        agent_id: key.agent_id(),
        signature: String::new(),
    };
    node.signature = key.sign(&node.canonical_bytes().unwrap());
    node
}

fn verified<T: std::fmt::Debug>(lookup: Lookup<T>) -> T {
    match lookup {
        Lookup::Verified(node) => node,
        other => panic!("expected a verified record, got {other:?}"),
    }
}

/// The oid `name` holds, or `None` when it does not exist.
fn ref_oid(dir: &Path, name: &str) -> Option<String> {
    let out = git_status(dir, &["rev-parse", "--verify", "-q", name]);
    out.status
        .success()
        .then(|| String::from_utf8_lossy(&out.stdout).trim().to_string())
}

fn scratch_refs(dir: &Path) -> Vec<String> {
    git(dir, &["for-each-ref", "--format=%(refname)", SCRATCH])
        .lines()
        .map(str::to_string)
        .collect()
}

fn raw_ref(dir: &Path, ref_name: &str, bytes: &[u8]) -> String {
    use std::io::Write;
    let mut child = Command::new("git")
        .args(["hash-object", "-w", "--stdin"])
        .current_dir(dir)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(bytes).unwrap();
    let blob = String::from_utf8(child.wait_with_output().unwrap().stdout)
        .unwrap()
        .trim()
        .to_string();
    git(dir, &["update-ref", ref_name, &blob]);
    blob
}

/// A writer whose first publication waits for `other` to run to completion underneath it.
fn loses_first_race_to(dir: &Path, other: impl FnOnce() + Send + 'static) -> (BritRefManager, Arc<AtomicU32>) {
    let barrier = Arc::new(Barrier::new(2));
    let calls = Arc::new(AtomicU32::new(0));
    let theirs = Arc::clone(&barrier);
    std::thread::spawn(move || {
        theirs.wait();
        other();
        theirs.wait();
    });
    let counted = Arc::clone(&calls);
    let writer = BritRefManager::new(dir).unwrap().before_publish(move |_, attempt| {
        counted.fetch_add(1, Ordering::SeqCst);
        if attempt == 1 {
            barrier.wait();
            barrier.wait();
        }
    });
    (writer, calls)
}

fn zero(format_len: usize) -> String {
    "0".repeat(format_len)
}

#[test]
fn notes_on_different_commits_both_survive_a_lost_race() {
    let tmp = repo();
    let k = key(&tmp, "k");
    let (c1, c2) = (commit(tmp.path(), "one"), commit(tmp.path(), "two"));
    let (a, b) = (build(&k, STEP, "a"), build(&k, STEP, "b"));
    let (path, theirs, c2_theirs) = (tmp.path().to_path_buf(), b.clone(), c2.clone());
    let (writer, calls) = loses_first_race_to(tmp.path(), move || {
        BritRefManager::new(&path)
            .unwrap()
            .put_build_ref(STEP, &c2_theirs, &theirs)
            .unwrap();
    });
    writer.put_build_ref(STEP, &c1, &a).unwrap();

    assert_eq!(
        calls.load(Ordering::SeqCst),
        2,
        "lost once, rebuilt on the new head once"
    );
    let mgr = BritRefManager::new(tmp.path()).unwrap();
    assert_eq!(verified(mgr.get_build_ref(STEP, &c1).unwrap()), a);
    assert_eq!(verified(mgr.get_build_ref(STEP, &c2).unwrap()), b);
    // The winner's head is an ancestor of the final head: A was rebuilt on it, not beside it.
    let head = ref_oid(tmp.path(), NOTES).unwrap();
    let theirs_head = git(tmp.path(), &["rev-parse", &format!("{head}^")]);
    assert!(
        git_status(tmp.path(), &["merge-base", "--is-ancestor", &theirs_head, &head])
            .status
            .success()
    );
    assert_eq!(git(tmp.path(), &["notes", "--ref", NOTES, "list"]).lines().count(), 2);
    assert!(scratch_refs(tmp.path()).is_empty());
}

#[test]
fn a_note_on_the_same_commit_changed_underneath_is_refused_and_kept() {
    let tmp = repo();
    let k = key(&tmp, "k");
    let c1 = commit(tmp.path(), "one");
    let (mine, theirs) = (build(&k, STEP, "mine"), build(&k, STEP, "theirs"));
    let (path, t, c) = (tmp.path().to_path_buf(), theirs.clone(), c1.clone());
    let (writer, calls) = loses_first_race_to(tmp.path(), move || {
        BritRefManager::new(&path).unwrap().put_build_ref(STEP, &c, &t).unwrap();
    });
    let err = writer.put_build_ref(STEP, &c1, &mine).unwrap_err();

    let observed = ref_oid(tmp.path(), NOTES).unwrap();
    match &err {
        RefError::Concurrent {
            ref_name,
            expected,
            observed: o,
            attempts,
        } => {
            assert_eq!(ref_name, NOTES);
            assert_eq!(expected, &zero(40), "the notes ref was absent when this write read it");
            assert_eq!(o, &observed);
            assert_eq!(*attempts, 1);
        }
        other => panic!("expected a concurrent-update refusal, got {other:?}"),
    }
    assert_eq!(
        err.to_string(),
        format!(
            "REFUSED concurrent update: ref={NOTES} expected={} observed={observed} attempts=1",
            zero(40)
        )
    );
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    let mgr = BritRefManager::new(tmp.path()).unwrap();
    assert_eq!(
        verified(mgr.get_build_ref(STEP, &c1).unwrap()),
        theirs,
        "the winner's note is intact"
    );
    assert!(scratch_refs(tmp.path()).is_empty());
}

#[test]
fn equal_bytes_written_underneath_are_idempotent() {
    let tmp = repo();
    let k = key(&tmp, "k");
    let c1 = commit(tmp.path(), "one");
    let node = build(&k, STEP, "same");
    let (path, t, c) = (tmp.path().to_path_buf(), node.clone(), c1.clone());
    let (writer, _) = loses_first_race_to(tmp.path(), move || {
        BritRefManager::new(&path).unwrap().put_build_ref(STEP, &c, &t).unwrap();
    });
    let theirs_head = {
        writer.put_build_ref(STEP, &c1, &node).unwrap();
        ref_oid(tmp.path(), NOTES).unwrap()
    };
    assert_eq!(
        git(tmp.path(), &["rev-list", "--count", &theirs_head]),
        "1",
        "no second notes commit for bytes already there"
    );
    // A repeat put of the same bytes publishes nothing either.
    BritRefManager::new(tmp.path())
        .unwrap()
        .put_build_ref(STEP, &c1, &node)
        .unwrap();
    assert_eq!(ref_oid(tmp.path(), NOTES).unwrap(), theirs_head);
}

#[test]
fn the_retry_bound_is_exact() {
    let tmp = repo();
    let k = key(&tmp, "k");
    let target = commit(tmp.path(), "target");
    let others: Vec<String> = (0..4).map(|i| commit(tmp.path(), &format!("other {i}"))).collect();
    let node = build(&k, STEP, "target");

    // Every publication loses: another note lands on another commit each time.
    let (path, kk, list) = (tmp.path().to_path_buf(), key(&tmp, "other"), others.clone());
    let calls = Arc::new(AtomicU32::new(0));
    let counted = Arc::clone(&calls);
    let always_loses = BritRefManager::new(tmp.path())
        .unwrap()
        .before_publish(move |_, attempt| {
            counted.fetch_add(1, Ordering::SeqCst);
            let c = &list[attempt as usize - 1];
            BritRefManager::new(&path)
                .unwrap()
                .put_build_ref(STEP, c, &build(&kk, STEP, c))
                .unwrap();
        });
    let err = always_loses.put_build_ref(STEP, &target, &node).unwrap_err();
    assert!(matches!(err, RefError::Concurrent { attempts: 3, .. }), "{err:?}");
    assert!(err.to_string().ends_with(" attempts=3"), "{err}");
    assert_eq!(calls.load(Ordering::SeqCst), 3, "exactly three publications, no fourth");
    let mgr = BritRefManager::new(tmp.path()).unwrap();
    assert_eq!(mgr.get_build_ref(STEP, &target).unwrap(), Lookup::Absent);
    for c in &others[..3] {
        verified(mgr.get_build_ref(STEP, c).unwrap());
    }
    assert!(scratch_refs(tmp.path()).is_empty());

    // Losing twice and winning the third publication is Ok.
    let tmp = repo();
    let k = key(&tmp, "k");
    let target = commit(tmp.path(), "target");
    let others: Vec<String> = (0..2).map(|i| commit(tmp.path(), &format!("other {i}"))).collect();
    let (path, kk, list) = (tmp.path().to_path_buf(), key(&tmp, "other"), others.clone());
    let calls = Arc::new(AtomicU32::new(0));
    let counted = Arc::clone(&calls);
    let wins_third = BritRefManager::new(tmp.path())
        .unwrap()
        .before_publish(move |_, attempt| {
            counted.fetch_add(1, Ordering::SeqCst);
            if let Some(c) = list.get(attempt as usize - 1) {
                BritRefManager::new(&path)
                    .unwrap()
                    .put_build_ref(STEP, c, &build(&kk, STEP, c))
                    .unwrap();
            }
        });
    let node = build(&k, STEP, "target");
    wins_third.put_build_ref(STEP, &target, &node).unwrap();
    assert_eq!(calls.load(Ordering::SeqCst), 3);
    let mgr = BritRefManager::new(tmp.path()).unwrap();
    assert_eq!(verified(mgr.get_build_ref(STEP, &target).unwrap()), node);
    for c in &others {
        verified(mgr.get_build_ref(STEP, c).unwrap());
    }
}

#[test]
fn a_blob_ref_moved_underneath_a_stale_prior_is_refused_and_kept() {
    let tmp = repo();
    let k = key(&tmp, "k");
    let refs = BritRefManager::new(tmp.path()).unwrap();
    let ref_name = "refs/notes/brit/deploy/gate%3Ax/staging";
    refs.put_deploy_ref(STEP, "staging", &deploy(&k, "2026-10-29T10:00:00Z"))
        .unwrap();
    let stale = ref_oid(tmp.path(), ref_name).unwrap();

    let theirs = deploy(&k, "2026-10-29T11:00:00Z");
    let (path, t) = (tmp.path().to_path_buf(), theirs.clone());
    let (writer, calls) = loses_first_race_to(tmp.path(), move || {
        BritRefManager::new(&path)
            .unwrap()
            .put_deploy_ref(STEP, "staging", &t)
            .unwrap();
    });
    let err = writer
        .put_deploy_ref(STEP, "staging", &deploy(&k, "2026-10-29T12:00:00Z"))
        .unwrap_err();
    let observed = ref_oid(tmp.path(), ref_name).unwrap();
    assert_ne!(observed, stale);
    assert_eq!(
        err.to_string(),
        format!("REFUSED concurrent update: ref={ref_name} expected={stale} observed={observed} attempts=1")
    );
    assert_eq!(
        calls.load(Ordering::SeqCst),
        1,
        "a moved blob ref is never retried onto"
    );
    assert_eq!(verified(refs.get_deploy_ref(STEP, "staging").unwrap()), theirs);
}

#[test]
fn a_blob_ref_created_underneath_an_absence_is_refused_and_kept() {
    for format in ["sha1", "sha256"] {
        let tmp = repo_with(format);
        let k = key(&tmp, "k");
        let ref_name = "refs/notes/brit/validate/gate%3Ax/lint";
        let theirs = validation(&k, "theirs");
        let (path, t) = (tmp.path().to_path_buf(), theirs.clone());
        let (writer, _) = loses_first_race_to(tmp.path(), move || {
            BritRefManager::new(&path)
                .unwrap()
                .put_validate_ref(STEP, "lint", &t)
                .unwrap();
        });
        let err = writer
            .put_validate_ref(STEP, "lint", &validation(&k, "mine"))
            .unwrap_err();
        let observed = ref_oid(tmp.path(), ref_name).unwrap();
        let expected = zero(observed.len());
        assert_eq!(expected.len(), if format == "sha1" { 40 } else { 64 });
        assert_eq!(
            err.to_string(),
            format!("REFUSED concurrent update: ref={ref_name} expected={expected} observed={observed} attempts=1"),
            "{format}"
        );
        let refs = BritRefManager::new(tmp.path()).unwrap();
        assert_eq!(verified(refs.get_validate_ref(STEP, "lint").unwrap()), theirs);
    }
}

#[test]
fn covenant_creation_is_create_only_against_the_absence_it_read() {
    let tmp = repo();
    let k = key(&tmp, "k");
    let node = clause(&k, "q");
    let cid = node.compute_cid().unwrap().to_string();
    let ref_name = format!("refs/notes/brit/covenant/s1/{cid}");

    // Other bytes appear at the clause's ref between the absence check and publication.
    let (path, r) = (tmp.path().to_path_buf(), ref_name.clone());
    let (writer, _) = loses_first_race_to(tmp.path(), move || {
        raw_ref(&path, &r, br#"{"squatter":true}"#);
    });
    let err = writer.put_covenant_clause(&node).unwrap_err();
    assert!(
        matches!(&err, RefError::Concurrent { expected, attempts: 1, .. } if *expected == zero(40)),
        "{err:?}"
    );
    assert_eq!(
        git(tmp.path(), &["cat-file", "-p", &ref_name]),
        r#"{"squatter":true}"#,
        "never overwritten"
    );

    // The same clause recorded underneath is the same bytes: Ok.
    let tmp = repo();
    let k = key(&tmp, "k");
    let node = clause(&k, "q");
    let cid = node.compute_cid().unwrap().to_string();
    let (path, t) = (tmp.path().to_path_buf(), node.clone());
    let (writer, calls) = loses_first_race_to(tmp.path(), move || {
        BritRefManager::new(&path).unwrap().put_covenant_clause(&t).unwrap();
    });
    assert_eq!(writer.put_covenant_clause(&node).unwrap(), cid);
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    let refs = BritRefManager::new(tmp.path()).unwrap();
    assert_eq!(verified(refs.get_covenant_clause(&cid).unwrap()).clause, node);
}

#[test]
fn a_node_that_would_not_verify_is_refused_before_git_runs() {
    // Not a repository, only shaped like one: any git command here fails. A refusal that names
    // verification therefore came before git ran.
    let fake = TempDir::new().unwrap();
    std::fs::write(fake.path().join("HEAD"), "").unwrap();
    let refs = BritRefManager::new(fake.path()).unwrap();
    let k = key(&fake, "k");
    let mut flipped = build(&k, STEP, "c");
    flipped.success = false;
    let refused = |r: Result<(), RefError>| matches!(r, Err(RefError::WouldNotVerify { .. }));
    assert!(refused(refs.put_build_ref(STEP, "HEAD", &flipped)));
    assert!(refused(refs.put_build_ref("gate:y", "HEAD", &build(&k, STEP, "c"))));
    let mut d = deploy(&k, "2026-10-29T10:00:00Z");
    d.endpoint.push('!');
    assert!(refused(refs.put_deploy_ref(STEP, "staging", &d)));
    assert!(refused(refs.put_deploy_ref(STEP, "production", &deploy(&k, "t"))));
    let mut v = validation(&k, "s");
    v.result = ValidationResult::Fail;
    assert!(refused(refs.put_validate_ref(STEP, "lint", &v)));
    assert!(refused(refs.put_validate_ref(STEP, "other", &validation(&k, "s"))));
    let mut c = clause(&k, "q");
    c.party = "human:someone-else".into();
    assert!(matches!(
        refs.put_covenant_clause(&c),
        Err(RefError::WouldNotVerify { .. })
    ));
    let mut blank = build(&k, STEP, "c");
    blank.signature = String::new();
    assert!(refused(refs.put_build_ref(STEP, "HEAD", &blank)));
    // A valid node does reach git, and git fails here.
    assert!(!refused(refs.put_build_ref(STEP, "HEAD", &build(&k, STEP, "c"))));
    let mut entries: Vec<String> = std::fs::read_dir(fake.path())
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    entries.sort();
    assert_eq!(entries, ["HEAD", "keys"]);

    // In a real repository a refused write leaves no ref and no object.
    let tmp = repo();
    let refs = BritRefManager::new(tmp.path()).unwrap();
    let objects = git(tmp.path(), &["count-objects", "-v"]);
    assert!(refused(refs.put_build_ref(STEP, "HEAD", &flipped)));
    assert!(refused(refs.put_deploy_ref(STEP, "staging", &d)));
    assert!(matches!(
        refs.put_covenant_clause(&c),
        Err(RefError::WouldNotVerify { .. })
    ));
    assert_eq!(git(tmp.path(), &["for-each-ref", "refs/notes/"]), "");
    assert_eq!(git(tmp.path(), &["count-objects", "-v"]), objects);
}

#[test]
fn notes_commits_carry_the_neutral_brit_ident() {
    let tmp = repo();
    git(tmp.path(), &["config", "user.name", "Some Person"]);
    git(tmp.path(), &["config", "user.email", "person@example.com"]);
    let k = key(&tmp, "k");
    BritRefManager::new(tmp.path())
        .unwrap()
        .put_build_ref(STEP, "HEAD", &build(&k, STEP, "c"))
        .unwrap();
    assert_eq!(
        git(tmp.path(), &["log", "-1", "--format=%an <%ae>|%cn <%ce>", NOTES]),
        "brit <notes@brit.invalid>|brit <notes@brit.invalid>"
    );
}

#[test]
fn scratch_refs_never_outlive_a_write_and_stale_ones_are_swept() {
    let tmp = repo();
    let k = key(&tmp, "k");
    let head = git(tmp.path(), &["rev-parse", "HEAD"]);
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let stale = format!("{SCRATCH}424242-{}-0123abcd", now - 2 * 3600);
    let fresh = format!("{SCRATCH}424242-{now}-4567cdef");
    let foreign = format!("{SCRATCH}not-ours");
    for r in [&stale, &fresh, &foreign] {
        git(tmp.path(), &["update-ref", r, &head]);
    }
    let c1 = commit(tmp.path(), "one");
    BritRefManager::new(tmp.path())
        .unwrap()
        .put_build_ref(STEP, &c1, &build(&k, STEP, "c"))
        .unwrap();
    let mut left = scratch_refs(tmp.path());
    left.sort();
    let mut want = vec![fresh.clone(), foreign.clone()];
    want.sort();
    assert_eq!(
        left, want,
        "the stale scratch ref is swept; a fresh or unnamed one is not ours to take"
    );

    // A refused write leaves no scratch ref behind either.
    let (path, t, c) = (tmp.path().to_path_buf(), build(&k, STEP, "theirs"), c1.clone());
    let (writer, _) = loses_first_race_to(tmp.path(), move || {
        BritRefManager::new(&path).unwrap().put_build_ref(STEP, &c, &t).unwrap();
    });
    assert!(writer.put_build_ref(STEP, &c1, &build(&k, STEP, "mine")).is_err());
    let mut left = scratch_refs(tmp.path());
    left.sort();
    assert_eq!(left, want);
}

#[test]
fn a_sha256_repository_writes_and_reads_back() {
    let tmp = repo_with("sha256");
    let k = key(&tmp, "k");
    let refs = BritRefManager::new(tmp.path()).unwrap();
    let node = build(&k, STEP, "c");
    refs.put_build_ref(STEP, "HEAD", &node).unwrap();
    let c2 = commit(tmp.path(), "two");
    refs.put_build_ref(STEP, &c2, &build(&k, STEP, "d")).unwrap();
    assert_eq!(verified(refs.get_build_ref(STEP, "HEAD~1").unwrap()), node);
    refs.put_deploy_ref(STEP, "staging", &deploy(&k, "t")).unwrap();
    let cid = refs.put_covenant_clause(&clause(&k, "q")).unwrap();
    verified(refs.get_covenant_clause(&cid).unwrap());
    assert_eq!(ref_oid(tmp.path(), NOTES).unwrap().len(), 64);
}

/// The oid git gives `bytes`, without storing them.
fn oid_of(dir: &Path, bytes: &[u8]) -> String {
    use std::io::Write;
    let mut child = Command::new("git")
        .args(["hash-object", "--stdin"])
        .current_dir(dir)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(bytes).unwrap();
    String::from_utf8(child.wait_with_output().unwrap().stdout)
        .unwrap()
        .trim()
        .to_string()
}

/// Make the object store answer `oid` with `bytes`: the loose file of `oid` is replaced by the
/// loose file of `bytes`. git does not re-hash a loose object it serves.
fn corrupt_store(dir: &Path, oid: &str, bytes: &[u8]) {
    let other = raw_ref(dir, "refs/scratch/corrupt", bytes);
    git(dir, &["update-ref", "-d", "refs/scratch/corrupt"]);
    let loose = |o: &str| dir.join(".git/objects").join(&o[..2]).join(&o[2..]);
    std::fs::remove_file(loose(oid)).unwrap();
    std::fs::copy(loose(&other), loose(oid)).unwrap();
}

fn stored_bytes<T: serde::Serialize>(node: &T) -> Vec<u8> {
    serde_json::to_vec(&serde_json::to_value(node).unwrap()).unwrap()
}

#[test]
fn a_published_write_is_not_ok_until_it_reads_back_as_written() {
    // The notes commit publishes (update-ref checks that commit, not the note blob inside it),
    // but the store answers the note with other, validly signed bytes. A write acknowledgement is
    // not evidence; only the read-back is.
    let tmp = repo();
    let k = key(&tmp, "k");
    let c1 = commit(tmp.path(), "one");
    let node = build(&k, STEP, "mine");
    let note = |n: &BuildAttestationContentNode| {
        let mut b = stored_bytes(n);
        b.push(b'\n');
        b
    };
    let blob = oid_of(tmp.path(), &note(&node));
    let (path, other) = (tmp.path().to_path_buf(), note(&build(&k, STEP, "other")));
    let refs = BritRefManager::new(tmp.path())
        .unwrap()
        .before_publish(move |_, _| corrupt_store(&path, &blob, &other));
    let err = refs.put_build_ref(STEP, &c1, &node).unwrap_err();
    assert!(
        matches!(&err, RefError::ReadBack { reason, .. } if reason.contains("not the record written")),
        "{err:?}"
    );
    assert!(
        ref_oid(tmp.path(), NOTES).is_some(),
        "it was published, and still is not Ok"
    );

    // A blob ref whose object the store answers wrongly is never published at all: update-ref
    // checks the object it points the ref at.
    let node = deploy(&k, "2026-10-29T10:00:00Z");
    let blob = oid_of(tmp.path(), &stored_bytes(&node));
    let (path, other) = (
        tmp.path().to_path_buf(),
        stored_bytes(&deploy(&k, "2026-10-29T11:00:00Z")),
    );
    let refs = BritRefManager::new(tmp.path())
        .unwrap()
        .before_publish(move |_, _| corrupt_store(&path, &blob, &other));
    assert!(refs.put_deploy_ref(STEP, "staging", &node).is_err());
    assert_eq!(
        BritRefManager::new(tmp.path())
            .unwrap()
            .get_deploy_ref(STEP, "staging")
            .unwrap(),
        Lookup::Absent
    );
}

#[test]
fn a_fanned_out_notes_tree_is_written_and_read_back() {
    // Past 256 notes git stores a note at `ab/cdef…`, not `abcdef…`. The write's record of the
    // commit's prior entry and its read-back must find the note either way.
    use std::{fmt::Write as _, io::Write};
    const N: usize = 300;
    let tmp = repo();
    let mut script = String::new();
    for i in 0..N {
        let mark = i + 1;
        write!(
            script,
            "commit refs/heads/many\nmark :{mark}\ncommitter t <t@t> 1700000000 +0000\ndata 5\nc{i:03}\n"
        )
        .unwrap();
    }
    write!(
        script,
        "commit {NOTES}\ncommitter t <t@t> 1700000000 +0000\ndata 5\nnotes\n"
    )
    .unwrap();
    for i in 0..N {
        let mark = i + 1;
        write!(script, "N inline :{mark}\ndata 4\njunk\n").unwrap();
    }
    let mut child = Command::new("git")
        .args(["fast-import", "--quiet"])
        .current_dir(tmp.path())
        .stdin(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(script.as_bytes()).unwrap();
    assert!(child.wait().unwrap().success());
    let tree = git(tmp.path(), &["ls-tree", "-r", "--name-only", NOTES]);
    assert!(tree.lines().all(|p| p.contains('/')), "fanned out: {tree:.200}");

    let k = key(&tmp, "k");
    let refs = BritRefManager::new(tmp.path()).unwrap();
    let noted = git(tmp.path(), &["rev-parse", "refs/heads/many~7"]);
    let node = build(&k, STEP, "over-junk");
    refs.put_build_ref(STEP, &noted, &node).unwrap();
    assert_eq!(verified(refs.get_build_ref(STEP, &noted).unwrap()), node);
    let fresh = commit(tmp.path(), "fresh");
    let node = build(&k, STEP, "fresh");
    refs.put_build_ref(STEP, &fresh, &node).unwrap();
    assert_eq!(verified(refs.get_build_ref(STEP, &fresh).unwrap()), node);
    // Repeating it is recognised as already there through the fanned path.
    let head = ref_oid(tmp.path(), NOTES).unwrap();
    refs.put_build_ref(STEP, &fresh, &node).unwrap();
    assert_eq!(ref_oid(tmp.path(), NOTES).unwrap(), head);
    assert_eq!(
        git(tmp.path(), &["notes", "--ref", NOTES, "list"]).lines().count(),
        N + 1
    );
}

#[test]
fn eight_writers_by_twenty_five_puts_every_ok_reads_back_verified() {
    const WRITERS: usize = 8;
    const PUTS: usize = 25;
    let tmp = repo();
    let commits: Vec<String> = (0..WRITERS * PUTS)
        .map(|i| commit(tmp.path(), &format!("c{i}")))
        .collect();
    let start = Arc::new(Barrier::new(WRITERS));
    let handles: Vec<_> = (0..WRITERS)
        .map(|w| {
            let path = tmp.path().to_path_buf();
            let mine: Vec<String> = commits[w * PUTS..(w + 1) * PUTS].to_vec();
            let start = Arc::clone(&start);
            let k = AgentKey::generate(&path.join("keys").join(format!("w{w}"))).unwrap();
            std::thread::spawn(move || {
                let refs = BritRefManager::new(&path).unwrap();
                start.wait();
                mine.into_iter()
                    .map(|c| {
                        let node = build(&k, STEP, &c);
                        let result = refs.put_build_ref(STEP, &c, &node);
                        (c, node, result)
                    })
                    .collect::<Vec<_>>()
            })
        })
        .collect();
    let results: Vec<_> = handles.into_iter().flat_map(|h| h.join().unwrap()).collect();
    assert_eq!(results.len(), WRITERS * PUTS);

    let refs = BritRefManager::new(tmp.path()).unwrap();
    let mut ok = 0;
    for (c, node, result) in &results {
        match result {
            Ok(()) => {
                ok += 1;
                assert_eq!(&verified(refs.get_build_ref(STEP, c).unwrap()), node, "{c}");
            }
            Err(RefError::Concurrent { attempts: 3, .. }) => {
                assert_eq!(refs.get_build_ref(STEP, c).unwrap(), Lookup::Absent, "{c}");
            }
            Err(other) => panic!("{c}: only a bounded concurrent refusal may fail a put: {other:?}"),
        }
    }
    eprintln!(
        "{ok} of {} puts Ok; the rest refused after three lost publications",
        WRITERS * PUTS
    );
    assert!(ok > 0);
    assert_eq!(
        git(tmp.path(), &["notes", "--ref", NOTES, "list"]).lines().count(),
        ok,
        "every Ok put is in the published notes, and nothing else is"
    );
    assert!(scratch_refs(tmp.path()).is_empty());
}
