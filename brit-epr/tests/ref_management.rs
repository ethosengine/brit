use std::process::Command;

use brit_epr::{
    elohim::{
        attestation::{
            build::BuildAttestationContentNode,
            covenant::{CovenantClauseContentNode, CovenantClauseKind, CovenantRecord},
            deploy::{DeployAttestationContentNode, HealthStatus},
            validation::{ValidationAttestationContentNode, ValidationResult},
        },
        refs::{BritRefManager, Lookup},
    },
    engine::{cid::BritCid, content_node::ContentNode, signing::AgentKey},
};
use tempfile::TempDir;

fn git(dir: &std::path::Path, args: &[&str]) -> String {
    let out = Command::new("git")
        .args(["-c", "user.email=test@test.com", "-c", "user.name=test"])
        .args(args)
        .current_dir(dir)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

fn init_git_repo() -> TempDir {
    let tmp = TempDir::new().unwrap();
    git(tmp.path(), &["init", "--initial-branch=main"]);
    git(tmp.path(), &["commit", "--allow-empty", "-m", "init"]);
    tmp
}

fn commit_empty(dir: &std::path::Path, message: &str) -> String {
    git(dir, &["commit", "--allow-empty", "-m", message]);
    git(dir, &["rev-parse", "HEAD"])
}

fn key(dir: &TempDir, name: &str) -> AgentKey {
    AgentKey::generate(&dir.path().join("keys").join(name)).unwrap()
}

fn build(key: &AgentKey, step: &str, inputs: &str, success: bool) -> BuildAttestationContentNode {
    let mut node = BuildAttestationContentNode {
        manifest_cid: BritCid::compute_raw(b"manifest"),
        step_name: step.into(),
        inputs_hash: inputs.into(),
        output_cid: BritCid::compute_raw(b"output"),
        agent_id: key.agent_id(),
        hardware_profile: serde_json::json!({"arch": "x86_64", "cores": 12}),
        build_duration_ms: 1000,
        built_at: "2026-04-16T10:00:00Z".into(),
        success,
        signature: String::new(),
    };
    node.signature = key.sign(&node.canonical_bytes().unwrap());
    node
}

fn deploy(key: &AgentKey, step: &str, env: &str) -> DeployAttestationContentNode {
    let mut node = DeployAttestationContentNode {
        artifact_cid: BritCid::compute_raw(b"artifact"),
        step_name: step.into(),
        environment_label: env.into(),
        endpoint: "https://staging.example".into(),
        health_check_epr: "epr:x/health".into(),
        health_status: HealthStatus::Healthy,
        deployed_at: "2026-04-16T10:00:00Z".into(),
        attested_at: "2026-04-16T10:00:00Z".into(),
        liveness_ttl_sec: 300,
        agent_id: key.agent_id(),
        signature: String::new(),
    };
    node.signature = key.sign(&node.canonical_bytes().unwrap());
    node
}

fn validation(key: &AgentKey, check: &str, result: ValidationResult) -> ValidationAttestationContentNode {
    let mut node = ValidationAttestationContentNode {
        artifact_cid: BritCid::compute_raw(b"artifact"),
        check_name: check.into(),
        validator_id: key.agent_id(),
        validator_version: "1".into(),
        result,
        result_summary: String::new(),
        findings_cid: None,
        validated_at: "2026-04-16T10:00:00Z".into(),
        ttl_sec: None,
        signature: String::new(),
    };
    node.signature = key.sign(&node.canonical_bytes().unwrap());
    node
}

fn clause(key: &AgentKey, session: &str, valid_from: &str, quote: &str) -> CovenantClauseContentNode {
    let mut node = CovenantClauseContentNode {
        kind: CovenantClauseKind::DeclaresPurpose,
        session: session.into(),
        party: "human:t".into(),
        quote: quote.into(),
        payload: serde_json::json!({"purpose": "p", "writeSet": ["docs/**"]}),
        valid_from: valid_from.into(),
        valid_until: None,
        supersedes: None,
        agent_id: key.agent_id(),
        signature: String::new(),
    };
    node.signature = key.sign(&node.canonical_bytes().unwrap());
    node
}

fn json<T: serde::Serialize>(node: &T) -> serde_json::Value {
    serde_json::to_value(node).unwrap()
}

fn verified<T: std::fmt::Debug>(lookup: Lookup<T>) -> T {
    match lookup {
        Lookup::Verified(node) => node,
        other => panic!("expected a verified record, got {other:?}"),
    }
}

#[test]
fn put_and_get_build_ref() {
    let tmp = init_git_repo();
    let k = key(&tmp, "a");
    let mgr = BritRefManager::new(tmp.path()).unwrap();
    let node = build(&k, "elohim-edge:storage", "closure-1", true);
    mgr.put_build_ref("elohim-edge:storage", "HEAD", &json(&node)).unwrap();
    assert_eq!(
        verified(mgr.get_build_ref("elohim-edge:storage", "HEAD").unwrap()),
        node
    );
}

#[test]
fn get_missing_ref_returns_absent() {
    let tmp = init_git_repo();
    let mgr = BritRefManager::new(tmp.path()).unwrap();
    assert_eq!(mgr.get_build_ref("nonexistent", "HEAD").unwrap(), Lookup::Absent);
    assert_eq!(mgr.get_deploy_ref("nonexistent", "staging").unwrap(), Lookup::Absent);
    assert_eq!(mgr.get_validate_ref("nonexistent", "lint").unwrap(), Lookup::Absent);
    assert_eq!(mgr.get_covenant_clause("bafyreimissing").unwrap(), Lookup::Absent);
}

#[test]
fn get_build_ref_on_a_commit_without_a_note_is_absent() {
    let tmp = init_git_repo();
    let k = key(&tmp, "a");
    let mgr = BritRefManager::new(tmp.path()).unwrap();
    let first = git(tmp.path(), &["rev-parse", "HEAD"]);
    mgr.put_build_ref("s", &first, &json(&build(&k, "s", "c", true)))
        .unwrap();
    commit_empty(tmp.path(), "second");
    assert_eq!(mgr.get_build_ref("s", "HEAD").unwrap(), Lookup::Absent);
    verified(mgr.get_build_ref("s", &first).unwrap());
}

#[test]
fn unsigned_payloads_are_not_returned() {
    // The shapes earlier fixtures wrote as "attestations": no signature, missing fields.
    let tmp = init_git_repo();
    let mgr = BritRefManager::new(tmp.path()).unwrap();
    let head = git(tmp.path(), &["rev-parse", "HEAD"]);
    let unsigned = serde_json::json!({
        "attestationCid": "abc123", "outputCid": "def456", "agentId": "agent001", "builtAt": "2026-04-16T10:00:00Z"
    });
    mgr.put_build_ref("elohim-edge:storage", "HEAD", &unsigned).unwrap();
    assert!(matches!(
        mgr.get_build_ref("elohim-edge:storage", "HEAD").unwrap(),
        Lookup::Unverifiable { at, .. } if at == head
    ));
    mgr.put_deploy_ref(
        "elohim-edge:storage",
        "staging",
        &serde_json::json!({"artifactCid": "abc123", "healthStatus": "healthy"}),
    )
    .unwrap();
    assert!(matches!(
        mgr.get_deploy_ref("elohim-edge:storage", "staging").unwrap(),
        Lookup::Unverifiable { .. }
    ));
    // Every field present, but the signature is blank.
    let k = key(&tmp, "a");
    let mut blank = build(&k, "s", "c", true);
    blank.signature = String::new();
    mgr.put_build_ref("s", "HEAD", &json(&blank)).unwrap();
    assert!(matches!(
        mgr.get_build_ref("s", "HEAD").unwrap(),
        Lookup::Unverifiable { .. }
    ));
}

#[test]
fn put_and_get_deploy_ref_binds_step_and_env() {
    let tmp = init_git_repo();
    let k = key(&tmp, "a");
    let mgr = BritRefManager::new(tmp.path()).unwrap();
    let node = deploy(&k, "elohim-edge:storage", "staging");
    mgr.put_deploy_ref("elohim-edge:storage", "staging", &json(&node))
        .unwrap();
    assert_eq!(
        verified(mgr.get_deploy_ref("elohim-edge:storage", "staging").unwrap()),
        node
    );
    // The same signed node filed under another environment does not bind there.
    mgr.put_deploy_ref("elohim-edge:storage", "production", &json(&node))
        .unwrap();
    assert!(matches!(
        mgr.get_deploy_ref("elohim-edge:storage", "production").unwrap(),
        Lookup::Unverifiable { .. }
    ));
}

#[test]
fn put_and_get_validate_ref_binds_its_check() {
    let tmp = init_git_repo();
    let k = key(&tmp, "a");
    let mgr = BritRefManager::new(tmp.path()).unwrap();
    let node = validation(&k, "sonarqube-scan@v10", ValidationResult::Fail);
    mgr.put_validate_ref("elohim-edge:storage", "sonarqube-scan@v10", &json(&node))
        .unwrap();
    assert_eq!(
        verified(
            mgr.get_validate_ref("elohim-edge:storage", "sonarqube-scan@v10")
                .unwrap()
        ),
        node,
        "a valid failing check is returned"
    );
    mgr.put_validate_ref("elohim-edge:storage", "lint@v1", &json(&node))
        .unwrap();
    assert!(matches!(
        mgr.get_validate_ref("elohim-edge:storage", "lint@v1").unwrap(),
        Lookup::Unverifiable { .. }
    ));
}

#[test]
fn list_build_refs() {
    let tmp = init_git_repo();
    let k = key(&tmp, "a");
    let mgr = BritRefManager::new(tmp.path()).unwrap();
    mgr.put_build_ref("step-a", "HEAD", &json(&build(&k, "step-a", "c", true)))
        .unwrap();
    mgr.put_build_ref("step-b", "HEAD", &json(&build(&k, "step-b", "c", true)))
        .unwrap();
    let mut refs = mgr.list_build_refs(None).unwrap();
    refs.sort();
    assert_eq!(refs, vec!["step-a", "step-b"]);
}

#[test]
fn find_build_by_inputs_hits_across_commits_and_ignores_other_hashes() {
    // Notes are keyed by commit; the gate's question is keyed by input closure. Two commits
    // carrying the same closure must both answer, a different closure must not, and a step
    // with no notes ref yet must read as empty rather than fail.
    let tmp = init_git_repo();
    let a = key(&tmp, "a");
    let b = key(&tmp, "b");
    let mgr = BritRefManager::new(tmp.path()).unwrap();
    let step = "elohim-edge:storage";
    let first = commit_empty(tmp.path(), "first");
    mgr.put_build_ref(step, &first, &json(&build(&a, step, "closure-1", true)))
        .unwrap();
    let second = commit_empty(tmp.path(), "second");
    mgr.put_build_ref(step, &second, &json(&build(&a, step, "closure-2", false)))
        .unwrap();
    let third = commit_empty(tmp.path(), "third");
    mgr.put_build_ref(step, &third, &json(&build(&b, step, "closure-1", true)))
        .unwrap();

    let hits = mgr.find_build_by_inputs(step, "closure-1").unwrap();
    assert!(hits.unverifiable.is_empty());
    let mut commits: Vec<&str> = hits.verified.iter().map(|(c, _)| c.as_str()).collect();
    commits.sort_unstable();
    let mut expected = vec![first.as_str(), third.as_str()];
    expected.sort_unstable();
    assert_eq!(commits, expected);
    assert!(hits.verified.iter().all(|(_, n)| n.inputs_hash == "closure-1"));
    let by_b = hits.verified.iter().find(|(c, _)| *c == third).unwrap();
    assert_eq!(by_b.1.agent_id, b.agent_id(), "each note answers under its own signer");

    let red = mgr.find_build_by_inputs(step, "closure-2").unwrap();
    assert_eq!(red.verified.len(), 1);
    assert_eq!(red.verified[0].0, second);
    assert!(!red.verified[0].1.success, "a valid red attestation is returned");

    assert!(mgr.find_build_by_inputs(step, "closure-9").unwrap().verified.is_empty());
    let none = mgr.find_build_by_inputs("no-such-step", "closure-1").unwrap();
    assert!(none.verified.is_empty() && none.unverifiable.is_empty());
}

#[test]
fn find_drops_tampered_notes_and_names_them() {
    let tmp = init_git_repo();
    let k = key(&tmp, "a");
    let mgr = BritRefManager::new(tmp.path()).unwrap();
    let step = "gate:x";
    let good = commit_empty(tmp.path(), "good");
    mgr.put_build_ref(step, &good, &json(&build(&k, step, "c", true)))
        .unwrap();
    let flipped = commit_empty(tmp.path(), "flipped");
    let mut v = json(&build(&k, step, "c", false));
    v["success"] = serde_json::Value::Bool(true);
    mgr.put_build_ref(step, &flipped, &v).unwrap();
    let swapped = commit_empty(tmp.path(), "swapped");
    mgr.put_build_ref(step, &swapped, &json(&build(&k, "gate:y", "c", true)))
        .unwrap();
    let extended = commit_empty(tmp.path(), "extended");
    let mut v = json(&build(&k, step, "c", true));
    v["trusted"] = serde_json::Value::Bool(true);
    mgr.put_build_ref(step, &extended, &v).unwrap();

    let scan = mgr.find_build_by_inputs(step, "c").unwrap();
    assert_eq!(
        scan.verified.iter().map(|(c, _)| c.as_str()).collect::<Vec<_>>(),
        [good.as_str()]
    );
    let mut dropped: Vec<&str> = scan.unverifiable.iter().map(|(c, _)| c.as_str()).collect();
    dropped.sort_unstable();
    let mut expected = vec![flipped.as_str(), swapped.as_str(), extended.as_str()];
    expected.sort_unstable();
    assert_eq!(dropped, expected);
    let reason = |c: &str| &scan.unverifiable.iter().find(|(at, _)| at == c).unwrap().1;
    assert!(reason(&flipped).contains("signature"), "{}", reason(&flipped));
    assert!(reason(&swapped).contains("stepName"), "{}", reason(&swapped));
    assert!(reason(&extended).contains("decode"), "{}", reason(&extended));
}

#[test]
fn a_notes_ref_git_cannot_read_is_an_error_not_an_absence() {
    let tmp = init_git_repo();
    let mgr = BritRefManager::new(tmp.path()).unwrap();
    let blob = {
        use std::io::Write;
        let mut child = Command::new("git")
            .args(["hash-object", "-w", "--stdin"])
            .current_dir(tmp.path())
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .spawn()
            .unwrap();
        child.stdin.take().unwrap().write_all(b"not a notes commit").unwrap();
        String::from_utf8(child.wait_with_output().unwrap().stdout)
            .unwrap()
            .trim()
            .to_string()
    };
    git(tmp.path(), &["update-ref", "refs/notes/brit/build/s", &blob]);
    assert!(mgr.find_build_by_inputs("s", "c").is_err());
    assert!(mgr.get_build_ref("s", "HEAD").is_err());
    assert!(mgr.list_build_refs(None).is_ok(), "the ref itself still lists");
}

#[test]
fn a_blob_ref_to_a_missing_object_is_an_error_not_an_absence() {
    let tmp = init_git_repo();
    let mgr = BritRefManager::new(tmp.path()).unwrap();
    let dir = tmp.path().join(".git/refs/notes/brit/deploy/s");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("staging"), "1111111111111111111111111111111111111111\n").unwrap();
    assert!(mgr.get_deploy_ref("s", "staging").is_err());
}

#[test]
fn covenant_put_is_append_only_and_lists_verified_clauses_in_valid_from_order() {
    let tmp = init_git_repo();
    let k = key(&tmp, "a");
    let mgr = BritRefManager::new(tmp.path()).unwrap();
    // Put the later clause first: list order must follow validFrom, not insertion.
    let late = clause(&k, "s1", "2026-10-19T12:00:00Z", "late");
    let early = clause(&k, "s1", "2026-10-19T09:00:00Z", "early");
    let late_cid = late.compute_cid().unwrap().to_string();
    let early_cid = early.compute_cid().unwrap().to_string();
    mgr.put_covenant_clause("s1", &late_cid, &json(&late)).unwrap();
    mgr.put_covenant_clause("s1", &early_cid, &json(&early)).unwrap();

    let scan = mgr.list_covenant_clauses("s1").unwrap();
    assert!(scan.unverifiable.is_empty(), "{:?}", scan.unverifiable);
    let rows: Vec<&CovenantRecord> = scan.verified.iter().map(|(_, r)| r).collect();
    assert_eq!(rows.len(), 2, "two puts, two refs: {rows:?}");
    assert_eq!(rows[0].cid, early_cid);
    assert_eq!(rows[0].clause, early);
    assert_eq!(rows[1].cid, late_cid);
    assert_eq!(scan.verified[1].0, format!("refs/notes/brit/covenant/s1/{late_cid}"));

    // Same cid, same content: idempotent. Same cid, different content: refused, original kept.
    mgr.put_covenant_clause("s1", &late_cid, &json(&late)).unwrap();
    let other = clause(&k, "s1", "2026-10-19T12:00:00Z", "tampered");
    assert!(mgr.put_covenant_clause("s1", &late_cid, &json(&other)).is_err());
    assert_eq!(mgr.list_covenant_clauses("s1").unwrap().verified[1].1.clause, late);
}

#[test]
fn covenant_reads_check_the_cid_the_signature_and_the_session() {
    let tmp = init_git_repo();
    let k = key(&tmp, "a");
    let mgr = BritRefManager::new(tmp.path()).unwrap();
    assert!(mgr.list_covenant_clauses("nobody").unwrap().verified.is_empty());
    let one = clause(&k, "s:1", "2026-10-19T09:00:00Z", "q");
    let one_cid = one.compute_cid().unwrap().to_string();
    mgr.put_covenant_clause("s:1", &one_cid, &json(&one)).unwrap();
    assert!(mgr.list_covenant_clauses("other").unwrap().verified.is_empty());
    assert_eq!(mgr.list_covenant_clauses("s:1").unwrap().verified.len(), 1);
    let got = verified(mgr.get_covenant_clause(&one_cid).unwrap());
    assert_eq!(got.cid, one_cid);
    assert_eq!(got.clause, one);

    // A record filed under a CID that is not its clause's.
    let wrong = BritCid::compute(b"some other clause").to_string();
    mgr.put_covenant_clause("s:1", &wrong, &json(&one)).unwrap();
    // A record whose clause changed after signing, filed under the changed clause's CID.
    let mut tampered = one.clone();
    tampered.quote = "not what was said".into();
    let tampered_cid = tampered.compute_cid().unwrap().to_string();
    mgr.put_covenant_clause("s:1", &tampered_cid, &json(&tampered)).unwrap();
    // A valid record of session s:1 filed under session s:2.
    mgr.put_covenant_clause("s:2", &one_cid, &json(&one)).unwrap();

    let scan = mgr.list_covenant_clauses("s:1").unwrap();
    assert_eq!(scan.verified.len(), 1);
    assert_eq!(scan.unverifiable.len(), 2, "{:?}", scan.unverifiable);
    let s2 = mgr.list_covenant_clauses("s:2").unwrap();
    assert!(s2.verified.is_empty());
    assert_eq!(s2.unverifiable.len(), 1);
    assert!(s2.unverifiable[0].1.contains("session"), "{:?}", s2.unverifiable);

    assert!(matches!(
        mgr.get_covenant_clause(&wrong).unwrap(),
        Lookup::Unverifiable { .. }
    ));
    assert!(matches!(
        mgr.get_covenant_clause(&tampered_cid).unwrap(),
        Lookup::Unverifiable { .. }
    ));
    // The copy under s:2 does not hide the original under s:1.
    assert_eq!(verified(mgr.get_covenant_clause(&one_cid).unwrap()).clause, one);
}

#[test]
fn a_foreign_key_note_is_returned_under_its_own_agent_id() {
    let tmp = init_git_repo();
    let mine = key(&tmp, "mine");
    let foreign = key(&tmp, "foreign");
    let mgr = BritRefManager::new(tmp.path()).unwrap();
    let node = build(&foreign, "s", "c", true);
    mgr.put_build_ref("s", "HEAD", &json(&node)).unwrap();
    let got = verified(mgr.get_build_ref("s", "HEAD").unwrap());
    assert_eq!(got.agent_id, foreign.agent_id());
    assert_ne!(got.agent_id, mine.agent_id(), "brit does not judge whose key it is");
}
