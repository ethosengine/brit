//! `reach` subcommand — compute/get reach level for a pipeline step.

use std::path::Path;

use brit_epr::elohim::attestation::reach::{compute_reach, ReachInput};
use brit_epr::elohim::refs::BritRefManager;

pub fn compute(repo: &Path, step: &str) -> anyhow::Result<()> {
    let refs = BritRefManager::new(repo)?;

    // Collect build attestations (step names from build refs).
    let build_attestations = refs.list_build_refs(None)?
        .into_iter()
        .filter(|name| name == step || name.starts_with(&format!("{step}/")))
        .collect::<Vec<_>>();

    // Collect deploy attestations (env labels from deploy refs for this step).
    let deploy_attestations = refs.list_deploy_refs(None)?
        .into_iter()
        .filter(|name| name.starts_with(&format!("{step}/")))
        .map(|name| name.trim_start_matches(&format!("{step}/")).to_string())
        .collect::<Vec<_>>();

    // Collect validation attestations (check names from validate refs for this step).
    let validation_attestations = refs.list_validate_refs(None)?
        .into_iter()
        .filter(|name| name.starts_with(&format!("{step}/")))
        .map(|name| name.trim_start_matches(&format!("{step}/")).to_string())
        .collect::<Vec<_>>();

    let input = ReachInput {
        build_attestations,
        deploy_attestations,
        validation_attestations,
    };

    let level = compute_reach(&input);

    let payload = serde_json::json!({
        "step": step,
        "reach": level,
        "computed_at": chrono::Utc::now().to_rfc3339(),
    });

    refs.put_reach_ref(step, &payload)?;

    println!("{}", serde_json::to_string_pretty(&payload)?);
    Ok(())
}

pub fn get(repo: &Path, step: &str) -> anyhow::Result<()> {
    let refs = BritRefManager::new(repo)?;
    match refs.get_reach_ref(step)? {
        Some(v) => println!("{}", serde_json::to_string_pretty(&v)?),
        None => eprintln!("no reach ref found for step={step}"),
    }
    Ok(())
}
