//! `validate` subcommand — put/get/list validation attestation refs.

use std::path::Path;

use brit_epr::{
    elohim::{
        attestation::validation::{ValidationAttestationContentNode, ValidationResult},
        refs::BritRefManager,
    },
    engine::{content_node::ContentNode, object_store::LocalObjectStore, signing::AgentKey},
};

pub fn put(
    repo: &Path,
    step: &str,
    check: &str,
    artifact_cid: &str,
    result: ValidationResult,
    summary: &str,
    validator_version: &str,
) -> anyhow::Result<()> {
    let git_dir = crate::git_dir::common_git_dir(repo)?;
    let key_path = crate::git_dir::agent_key_path(repo)?;
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
        validator_version: if validator_version.is_empty() {
            "unknown".to_string()
        } else {
            validator_version.to_string()
        },
        result,
        result_summary: summary.to_string(),
        findings_cid: None,
        validated_at,
        ttl_sec: None,
        signature: String::new(),
    };

    let canonical = node.canonical_bytes()?;
    node.signature = agent_key.sign(&canonical);

    store.put(&node)?;

    refs.put_validate_ref(step, check, &node)?;

    let cid = node.compute_cid()?;
    println!("{cid}");
    Ok(())
}

pub fn get(repo: &Path, step: &str, check: &str) -> anyhow::Result<()> {
    let refs = BritRefManager::new(repo)?;
    crate::lookup_out::print(
        refs.get_validate_ref(step, check)?,
        &format!("no validate ref found for step={step} check={check}"),
    )
}

pub fn list(repo: &Path) -> anyhow::Result<()> {
    let refs = BritRefManager::new(repo)?;
    for name in refs.list_validate_refs(None)? {
        println!("{name}");
    }
    Ok(())
}
