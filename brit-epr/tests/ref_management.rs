use std::process::Command;

use brit_epr::elohim::refs::BritRefManager;
use tempfile::TempDir;

fn init_git_repo() -> TempDir {
    let tmp = TempDir::new().unwrap();
    Command::new("git")
        .args(["init", "--initial-branch=main"])
        .current_dir(tmp.path())
        .output()
        .unwrap();
    Command::new("git")
        .args([
            "-c",
            "user.email=test@test.com",
            "-c",
            "user.name=test",
            "commit",
            "--allow-empty",
            "-m",
            "init",
        ])
        .current_dir(tmp.path())
        .output()
        .unwrap();
    tmp
}

#[test]
fn put_and_get_build_ref() {
    let tmp = init_git_repo();
    let mgr = BritRefManager::new(tmp.path()).unwrap();
    let payload = serde_json::json!({"attestationCid": "abc123", "outputCid": "def456", "agentId": "agent001", "builtAt": "2026-04-16T10:00:00Z"});
    mgr.put_build_ref("elohim-edge:storage", "HEAD", &payload).unwrap();
    let got = mgr.get_build_ref("elohim-edge:storage", "HEAD").unwrap();
    assert_eq!(got, Some(payload));
}

#[test]
fn get_missing_ref_returns_none() {
    let tmp = init_git_repo();
    let mgr = BritRefManager::new(tmp.path()).unwrap();
    let got = mgr.get_build_ref("nonexistent", "HEAD").unwrap();
    assert_eq!(got, None);
}

#[test]
fn put_and_get_deploy_ref() {
    let tmp = init_git_repo();
    let mgr = BritRefManager::new(tmp.path()).unwrap();
    let payload = serde_json::json!({"artifactCid": "abc123", "healthStatus": "healthy"});
    mgr.put_deploy_ref("elohim-edge:storage", "staging", &payload).unwrap();
    let got = mgr.get_deploy_ref("elohim-edge:storage", "staging").unwrap();
    assert_eq!(got, Some(payload));
}

#[test]
fn put_and_get_validate_ref() {
    let tmp = init_git_repo();
    let mgr = BritRefManager::new(tmp.path()).unwrap();
    let payload = serde_json::json!({"artifactCid": "abc123", "result": "pass"});
    mgr.put_validate_ref("elohim-edge:storage", "sonarqube-scan@v10", &payload)
        .unwrap();
    let got = mgr
        .get_validate_ref("elohim-edge:storage", "sonarqube-scan@v10")
        .unwrap();
    assert_eq!(got, Some(payload));
}

#[test]
fn list_build_refs() {
    let tmp = init_git_repo();
    let mgr = BritRefManager::new(tmp.path()).unwrap();
    mgr.put_build_ref("step-a", "HEAD", &serde_json::json!({"a": 1}))
        .unwrap();
    mgr.put_build_ref("step-b", "HEAD", &serde_json::json!({"b": 2}))
        .unwrap();
    let mut refs = mgr.list_build_refs(None).unwrap();
    refs.sort();
    assert_eq!(refs, vec!["step-a", "step-b"]);
}

fn commit_empty(dir: &std::path::Path, message: &str) -> String {
    Command::new("git")
        .args([
            "-c",
            "user.email=test@test.com",
            "-c",
            "user.name=test",
            "commit",
            "--allow-empty",
            "-m",
            message,
        ])
        .current_dir(dir)
        .output()
        .unwrap();
    let out = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(dir)
        .output()
        .unwrap();
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

#[test]
fn find_build_by_inputs_hits_across_commits_and_ignores_other_hashes() {
    // Notes are keyed by commit; the gate's question is keyed by input closure. Two commits
    // carrying the same closure must both answer, a different closure must not, and a step
    // with no notes ref yet must read as empty rather than fail.
    let tmp = init_git_repo();
    let mgr = BritRefManager::new(tmp.path()).unwrap();
    let first = commit_empty(tmp.path(), "first");
    mgr.put_build_ref(
        "elohim-edge:storage",
        &first,
        &serde_json::json!({"inputsHash": "closure-1", "success": true, "agentId": "a"}),
    )
    .unwrap();
    let second = commit_empty(tmp.path(), "second");
    mgr.put_build_ref(
        "elohim-edge:storage",
        &second,
        &serde_json::json!({"inputsHash": "closure-2", "success": false, "agentId": "a"}),
    )
    .unwrap();
    let third = commit_empty(tmp.path(), "third");
    mgr.put_build_ref(
        "elohim-edge:storage",
        &third,
        &serde_json::json!({"inputsHash": "closure-1", "success": true, "agentId": "b"}),
    )
    .unwrap();

    let hits = mgr.find_build_by_inputs("elohim-edge:storage", "closure-1").unwrap();
    let mut commits: Vec<&str> = hits.iter().map(|(c, _)| c.as_str()).collect();
    commits.sort_unstable();
    let mut expected = vec![first.as_str(), third.as_str()];
    expected.sort_unstable();
    assert_eq!(commits, expected);
    assert!(hits.iter().all(|(_, v)| v["inputsHash"] == "closure-1"));

    let misses = mgr.find_build_by_inputs("elohim-edge:storage", "closure-2").unwrap();
    assert_eq!(misses.len(), 1);
    assert_eq!(misses[0].0, second);
    assert_eq!(misses[0].1["success"], false);

    assert!(mgr
        .find_build_by_inputs("elohim-edge:storage", "closure-9")
        .unwrap()
        .is_empty());
    assert!(mgr
        .find_build_by_inputs("no-such-step", "closure-1")
        .unwrap()
        .is_empty());
}

fn clause(valid_from: &str, quote: &str) -> serde_json::Value {
    serde_json::json!({"kind": "declaresPurpose", "session": "s1", "validFrom": valid_from, "quote": quote})
}

#[test]
fn covenant_put_is_append_only_and_lists_in_valid_from_order() {
    let tmp = init_git_repo();
    let mgr = BritRefManager::new(tmp.path()).unwrap();
    // Put the later clause first: list order must follow validFrom, not insertion.
    let late = clause("2026-10-19T12:00:00Z", "late");
    let early = clause("2026-10-19T09:00:00Z", "early");
    mgr.put_covenant_clause("s1", "bafyreilate", &late).unwrap();
    mgr.put_covenant_clause("s1", "bafyreiearly", &early).unwrap();

    let rows = mgr.list_covenant_clauses("s1").unwrap();
    assert_eq!(rows.len(), 2, "two puts, two refs: {rows:?}");
    assert_eq!(rows[0]["cid"], "bafyreiearly");
    assert_eq!(rows[0]["clause"], early);
    assert_eq!(rows[1]["cid"], "bafyreilate");

    let refs = Command::new("git")
        .args(["for-each-ref", "--format=%(refname)", "refs/notes/brit/covenant/"])
        .current_dir(tmp.path())
        .output()
        .unwrap();
    let refs = String::from_utf8_lossy(&refs.stdout).to_string();
    assert!(refs.contains("refs/notes/brit/covenant/s1/bafyreilate"), "{refs}");
    assert!(refs.contains("refs/notes/brit/covenant/s1/bafyreiearly"), "{refs}");

    // Same cid, same content: idempotent. Same cid, different content: refused, original kept.
    mgr.put_covenant_clause("s1", "bafyreilate", &late).unwrap();
    let other = clause("2026-10-19T12:00:00Z", "tampered");
    assert!(mgr.put_covenant_clause("s1", "bafyreilate", &other).is_err());
    assert_eq!(mgr.list_covenant_clauses("s1").unwrap()[1]["clause"], late);
}

#[test]
fn covenant_unknown_session_lists_empty_and_get_finds_by_cid() {
    let tmp = init_git_repo();
    let mgr = BritRefManager::new(tmp.path()).unwrap();
    assert!(mgr.list_covenant_clauses("nobody").unwrap().is_empty());
    mgr.put_covenant_clause("s:1", "bafyreione", &clause("2026-10-19T09:00:00Z", "q"))
        .unwrap();
    assert!(mgr.list_covenant_clauses("other").unwrap().is_empty());
    assert_eq!(mgr.list_covenant_clauses("s:1").unwrap().len(), 1);
    let got = mgr.get_covenant_clause("bafyreione").unwrap().unwrap();
    assert_eq!(got["cid"], "bafyreione");
    assert!(mgr.get_covenant_clause("bafyreimissing").unwrap().is_none());
}
