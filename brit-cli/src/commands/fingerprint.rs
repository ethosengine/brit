//! brit fingerprint — content-addressed hash of step inputs.
//!
//! Resolves each step's source + buildProcess globs against the git tree at
//! the given commit (default HEAD), reads matching blob contents, and computes
//! a deterministic ContentFingerprint per step.

use std::path::Path;

use serde::Serialize;

use crate::error::{CliError, Result};

#[derive(Serialize)]
struct FingerprintOutput {
    manifest: String,
    commit: String,
    fingerprints: Vec<StepFingerprint>,
}

#[derive(Serialize)]
struct StepFingerprint {
    pipeline: String,
    step: String,
    fingerprint: String,
    input_count: usize,
}

pub fn run(manifest_path: &Path, step_filter: Option<&str>, commit_ref: &str) -> Result<()> {
    let text = std::fs::read_to_string(manifest_path)?;
    let m: rakia_core::manifest::BuildManifest = serde_json::from_str(&text)?;

    // Discover the repo from the manifest's parent dir
    let manifest_dir = manifest_path
        .parent()
        .ok_or_else(|| CliError::Args(format!("manifest has no parent dir: {}", manifest_path.display())))?;
    let repo = gix::discover(manifest_dir).map_err(|e| {
        CliError::Args(format!("repo discovery failed for {}: {e}", manifest_dir.display()))
    })?;

    // Resolve the commit ref to an ObjectId
    let commit_id = repo
        .rev_parse_single(commit_ref)
        .map_err(|e| CliError::Args(format!("could not resolve commit '{commit_ref}': {e}")))?
        .detach();

    let mut out = Vec::new();
    for (name, step) in &m.steps {
        if let Some(filter) = step_filter {
            if name != filter {
                continue;
            }
        }
        let mut all_patterns: Vec<String> = step.inputs.sources.clone();
        all_patterns.extend(step.inputs.build_process.iter().cloned());

        let fp = brit_graph::fingerprint::ContentFingerprint::from_repo_globs(
            &repo,
            commit_id,
            &all_patterns,
        )
        .map_err(|e| CliError::Args(format!("fingerprint compute failed for step '{name}': {e}")))?;

        out.push(StepFingerprint {
            pipeline: m.pipeline.clone(),
            step: name.clone(),
            fingerprint: fp.cid.as_str().to_string(),
            input_count: fp.inputs.len(),
        });
    }

    crate::output::print_json(&FingerprintOutput {
        manifest: manifest_path.display().to_string(),
        commit: commit_id.to_hex().to_string(),
        fingerprints: out,
    })?;
    Ok(())
}
