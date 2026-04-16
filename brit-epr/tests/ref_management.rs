use brit_epr::elohim::refs::BritRefManager;
use std::process::Command;
use tempfile::TempDir;

fn init_git_repo() -> TempDir {
    let tmp = TempDir::new().unwrap();
    Command::new("git").args(["init", "--initial-branch=main"]).current_dir(tmp.path()).output().unwrap();
    Command::new("git").args(["-c", "user.email=test@test.com", "-c", "user.name=test", "commit", "--allow-empty", "-m", "init"]).current_dir(tmp.path()).output().unwrap();
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
    mgr.put_validate_ref("elohim-edge:storage", "sonarqube-scan@v10", &payload).unwrap();
    let got = mgr.get_validate_ref("elohim-edge:storage", "sonarqube-scan@v10").unwrap();
    assert_eq!(got, Some(payload));
}

#[test]
fn list_build_refs() {
    let tmp = init_git_repo();
    let mgr = BritRefManager::new(tmp.path()).unwrap();
    mgr.put_build_ref("step-a", "HEAD", &serde_json::json!({"a": 1})).unwrap();
    mgr.put_build_ref("step-b", "HEAD", &serde_json::json!({"b": 2})).unwrap();
    let mut refs = mgr.list_build_refs(None).unwrap();
    refs.sort();
    assert_eq!(refs, vec!["step-a", "step-b"]);
}
