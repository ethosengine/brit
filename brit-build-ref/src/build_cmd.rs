//! `build` subcommand — put/get/list build attestation refs.

use std::path::Path;

use brit_epr::{
    elohim::{attestation::build::BuildAttestationContentNode, refs::BritRefManager},
    engine::{content_node::ContentNode, object_store::LocalObjectStore, signing::AgentKey},
};

/// Where the manifest CID comes from: given, or derived from the manifest file's bytes.
pub enum ManifestSource {
    Cid(String),
    File(std::path::PathBuf),
}

/// Where the output CID comes from: given, or the CID of a verdict node stored for a step
/// that produced no artifact (a gate records what it observed, addressed like anything else).
pub enum OutputSource {
    Cid(String),
    VerdictJson(String),
}

/// A step's observed verdict, stored so the output CID an attestation names resolves to the
/// bytes it describes. The value is whatever the gate chose to record (exit, seconds, jobs).
#[derive(serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct GateVerdictContentNode {
    step_name: String,
    verdict: serde_json::Value,
}

impl ContentNode for GateVerdictContentNode {
    fn content_type(&self) -> &'static str {
        "brit.gate-verdict"
    }
}

#[allow(clippy::too_many_arguments)]
pub fn put(
    repo: &Path,
    step: &str,
    manifest: ManifestSource,
    output: OutputSource,
    inputs_hash: &str,
    success: bool,
    hardware: &str,
    duration_ms: u64,
    commit: &str,
) -> anyhow::Result<()> {
    let git_dir = crate::git_dir::common_git_dir(repo)?;
    let key_path = crate::git_dir::agent_key_path(repo)?;
    let agent_key = AgentKey::load_or_generate(&key_path)?;
    let store = LocalObjectStore::for_git_dir(&git_dir);
    let refs = BritRefManager::new(repo)?;

    let hardware_profile: serde_json::Value =
        serde_json::from_str(hardware).map_err(|e| anyhow::anyhow!("invalid --hardware JSON: {e}"))?;

    let manifest_cid: brit_epr::engine::cid::BritCid = match manifest {
        ManifestSource::Cid(cid) => cid
            .parse()
            .map_err(|e| anyhow::anyhow!("invalid --manifest CID: {e:?}"))?,
        ManifestSource::File(path) => {
            let bytes = std::fs::read(&path)
                .map_err(|e| anyhow::anyhow!("cannot read --manifest-path {}: {e}", path.display()))?;
            brit_epr::engine::cid::BritCid::compute_raw(&bytes)
        }
    };
    let output_cid: brit_epr::engine::cid::BritCid = match output {
        OutputSource::Cid(cid) => cid
            .parse()
            .map_err(|e| anyhow::anyhow!("invalid --output CID: {e:?}"))?,
        OutputSource::VerdictJson(json) => {
            let verdict: serde_json::Value =
                serde_json::from_str(&json).map_err(|e| anyhow::anyhow!("invalid --output-json: {e}"))?;
            store.put(&GateVerdictContentNode {
                step_name: step.to_string(),
                verdict,
            })?
        }
    };

    let built_at = chrono::Utc::now().to_rfc3339();

    // Build with empty signature, compute canonical JSON, sign, then set signature.
    let mut node = BuildAttestationContentNode {
        manifest_cid,
        step_name: step.to_string(),
        inputs_hash: inputs_hash.to_string(),
        output_cid,
        agent_id: agent_key.agent_id(),
        hardware_profile,
        build_duration_ms: duration_ms,
        built_at,
        success,
        signature: String::new(),
    };

    let canonical = node.canonical_bytes()?;
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

/// Print every attestation for `step` whose inputs hash equals `inputs_hash`, across every
/// commit that carried one, as a JSON array of `{commit, attestation}` — `[]` when none.
/// Exit 0 either way: an absent attestation is an answer, not a failure.
pub fn find(repo: &Path, step: &str, inputs_hash: &str) -> anyhow::Result<()> {
    let refs = BritRefManager::new(repo)?;
    let rows: Vec<serde_json::Value> = refs
        .find_build_by_inputs(step, inputs_hash)?
        .into_iter()
        .map(|(commit, attestation)| serde_json::json!({ "commit": commit, "attestation": attestation }))
        .collect();
    println!("{}", serde_json::to_string(&rows)?);
    Ok(())
}

pub fn list(repo: &Path) -> anyhow::Result<()> {
    let refs = BritRefManager::new(repo)?;
    for name in refs.list_build_refs(None)? {
        println!("{name}");
    }
    Ok(())
}
