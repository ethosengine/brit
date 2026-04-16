//! `validate` subcommand — put/get/list validation attestation refs.

use std::path::Path;

use brit_epr::engine::content_node::ContentNode;
use brit_epr::engine::object_store::LocalObjectStore;
use brit_epr::engine::signing::AgentKey;
use brit_epr::elohim::attestation::validation::{ValidationAttestationContentNode, ValidationResult};
use brit_epr::elohim::refs::BritRefManager;

pub fn put(
    repo: &Path,
    step: &str,
    check: &str,
    artifact_cid: &str,
    result: ValidationResult,
    summary: &str,
) -> anyhow::Result<()> {
    let git_dir = repo.join(".git");
    let key_path = git_dir.join("brit").join("agent-key");
    let agent_key = AgentKey::load_or_generate(&key_path)?;
    let store = LocalObjectStore::for_git_dir(&git_dir);
    let refs = BritRefManager::new(repo)?;

    let artifact_cid: brit_epr::engine::cid::BritCid = artifact_cid
        .parse()
        .map_err(|e| anyhow::anyhow!("invalid --artifact CID: {e:?}"))?;

    let validated_at = chrono::Utc::now().to_rfc3339();

    let mut node = ValidationAttestationContentNode {
        artifact_cid,
        check_name: check.to_string(),
        validator_id: agent_key.agent_id(),
        validator_version: "0.0.0".to_string(),
        result,
        result_summary: summary.to_string(),
        findings_cid: None,
        validated_at,
        ttl_sec: None,
        signature: String::new(),
    };

    let canonical = node.canonical_json()?;
    node.signature = agent_key.sign(&canonical);

    store.put(&node)?;

    let payload = serde_json::to_value(&node)?;
    refs.put_validate_ref(step, check, &payload)?;

    let cid = node.compute_cid()?;
    println!("{cid}");
    Ok(())
}

pub fn get(repo: &Path, step: &str, check: &str) -> anyhow::Result<()> {
    let refs = BritRefManager::new(repo)?;
    match refs.get_validate_ref(step, check)? {
        Some(v) => println!("{}", serde_json::to_string_pretty(&v)?),
        None => eprintln!("no validate ref found for step={step} check={check}"),
    }
    Ok(())
}

pub fn list(repo: &Path) -> anyhow::Result<()> {
    let refs = BritRefManager::new(repo)?;
    for name in refs.list_validate_refs(None)? {
        println!("{name}");
    }
    Ok(())
}
