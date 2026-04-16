//! `deploy` subcommand — put/get/list deploy attestation refs.

use std::path::Path;

use brit_epr::engine::content_node::ContentNode;
use brit_epr::engine::object_store::LocalObjectStore;
use brit_epr::engine::signing::AgentKey;
use brit_epr::elohim::attestation::deploy::{DeployAttestationContentNode, HealthStatus};
use brit_epr::elohim::refs::BritRefManager;

pub fn put(
    repo: &Path,
    step: &str,
    env: &str,
    artifact_cid: &str,
    endpoint: &str,
    health: HealthStatus,
    ttl: u64,
) -> anyhow::Result<()> {
    let git_dir = repo.join(".git");
    let key_path = git_dir.join("brit").join("agent-key");
    let agent_key = AgentKey::load_or_generate(&key_path)?;
    let store = LocalObjectStore::for_git_dir(&git_dir);
    let refs = BritRefManager::new(repo)?;

    let artifact_cid: brit_epr::engine::cid::BritCid = artifact_cid
        .parse()
        .map_err(|e| anyhow::anyhow!("invalid --artifact CID: {e:?}"))?;

    let now = chrono::Utc::now().to_rfc3339();

    let mut node = DeployAttestationContentNode {
        artifact_cid,
        step_name: step.to_string(),
        environment_label: env.to_string(),
        endpoint: endpoint.to_string(),
        health_check_url: format!("{endpoint}/health"),
        health_status: health,
        deployed_at: now.clone(),
        attested_at: now,
        liveness_ttl_sec: ttl,
        agent_id: agent_key.agent_id(),
        signature: String::new(),
    };

    let canonical = node.canonical_json()?;
    node.signature = agent_key.sign(&canonical);

    store.put(&node)?;

    let payload = serde_json::to_value(&node)?;
    refs.put_deploy_ref(step, env, &payload)?;

    let cid = node.compute_cid()?;
    println!("{cid}");
    Ok(())
}

pub fn get(repo: &Path, step: &str, env: &str) -> anyhow::Result<()> {
    let refs = BritRefManager::new(repo)?;
    match refs.get_deploy_ref(step, env)? {
        Some(v) => println!("{}", serde_json::to_string_pretty(&v)?),
        None => eprintln!("no deploy ref found for step={step} env={env}"),
    }
    Ok(())
}

pub fn list(repo: &Path) -> anyhow::Result<()> {
    let refs = BritRefManager::new(repo)?;
    for name in refs.list_deploy_refs(None)? {
        println!("{name}");
    }
    Ok(())
}
