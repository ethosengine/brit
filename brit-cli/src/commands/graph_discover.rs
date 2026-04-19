//! brit graph discover — list all build-manifest.json files + summary JSON.

use std::path::Path;

use serde::Serialize;

use crate::error::{CliError, Result};

#[derive(Serialize)]
struct DiscoverOutput {
    manifests: Vec<ManifestSummary>,
}

#[derive(Serialize)]
struct ManifestSummary {
    path: String,
    pipeline: String,
    description: String,
    step_count: usize,
    steps: Vec<String>,
}

pub fn run(repo: &Path) -> Result<()> {
    let repo = repo.canonicalize().map_err(|source| CliError::RepoNotFound {
        path: repo.display().to_string(),
        source,
    })?;

    let manifests = rakia_core::discover::discover_manifests(&repo)
        .map_err(|e| CliError::ManifestDiscovery(format!("{e}")))?;

    let summaries: Vec<ManifestSummary> = manifests
        .into_iter()
        .map(|(path, m)| {
            let mut steps: Vec<String> = m.steps.keys().cloned().collect();
            steps.sort();
            ManifestSummary {
                path: path
                    .strip_prefix(&repo)
                    .unwrap_or(&path)
                    .display()
                    .to_string(),
                pipeline: m.pipeline,
                description: m.description,
                step_count: steps.len(),
                steps,
            }
        })
        .collect();

    crate::output::print_json(&DiscoverOutput { manifests: summaries })?;
    Ok(())
}
