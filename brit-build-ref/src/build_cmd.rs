//! `build` subcommand — put/get/list build attestation refs.

use std::path::Path;

use brit_epr::engine::content_node::ContentNode;
use brit_epr::engine::object_store::LocalObjectStore;
use brit_epr::engine::signing::AgentKey;
use brit_epr::elohim::attestation::build::BuildAttestationContentNode;
use brit_epr::elohim::refs::BritRefManager;

#[allow(clippy::too_many_arguments)]
pub fn put(
    repo: &Path,
    step: &str,
    manifest_cid: &str,
    output_cid: &str,
    success: bool,
    hardware: &str,
    duration_ms: u64,
    commit: &str,
) -> anyhow::Result<()> {
    let git_dir = repo.join(".git");
    let key_path = git_dir.join("brit").join("agent-key");
    let agent_key = AgentKey::load_or_generate(&key_path)?;
    let store = LocalObjectStore::for_git_dir(&git_dir);
    let refs = BritRefManager::new(repo)?;

    let hardware_profile: serde_json::Value = serde_json::from_str(hardware)
        .map_err(|e| anyhow::anyhow!("invalid --hardware JSON: {e}"))?;

    let manifest_cid: brit_epr::engine::cid::BritCid = manifest_cid
        .parse()
        .map_err(|e| anyhow::anyhow!("invalid --manifest CID: {e:?}"))?;
    let output_cid: brit_epr::engine::cid::BritCid = output_cid
        .parse()
        .map_err(|e| anyhow::anyhow!("invalid --output CID: {e:?}"))?;

    let built_at = chrono::Utc::now().to_rfc3339();

    // Build with empty signature, compute canonical JSON, sign, then set signature.
    let mut node = BuildAttestationContentNode {
        manifest_cid,
        step_name: step.to_string(),
        inputs_hash: String::new(),
        output_cid,
        agent_id: agent_key.agent_id(),
        hardware_profile,
        build_duration_ms: duration_ms,
        built_at,
        success,
        signature: String::new(),
    };

    let canonical = node.canonical_json()?;
    node.signature = agent_key.sign(&canonical);

    let cid = store.put(&node)?;

    let payload = serde_json::to_value(&node)?;
    refs.put_build_ref(step, commit, &payload)?;

    println!("{cid}");
    Ok(())
}

pub fn get(repo: &Path, step: &str, commit: &str) -> anyhow::Result<()> {
    let refs = BritRefManager::new(repo)?;
    match refs.get_build_ref(step, commit)? {
        Some(v) => println!("{}", serde_json::to_string_pretty(&v)?),
        None => eprintln!("no build ref found for step={step} commit={commit}"),
    }
    Ok(())
}

pub fn list(repo: &Path) -> anyhow::Result<()> {
    let refs = BritRefManager::new(repo)?;
    for name in refs.list_build_refs(None)? {
        println!("{name}");
    }
    Ok(())
}
