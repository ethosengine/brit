//! brit fingerprint — content-addressed hash of step inputs.
//!
//! Resolves each step's selected input globs against the git tree at the given
//! commit (default HEAD), reads matching blob contents, and computes a
//! deterministic ContentFingerprint per step. A gitlink a glob can reach is an
//! input keyed by its pinned commit, listed under `gitlinks`; a symlink a glob
//! matches or reaches through is keyed by its own object id (its target path,
//! not the file it points to), listed under `symlinks`.

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
    /// Blob, gitlink and symlink inputs.
    input_count: usize,
    /// Gitlinks the step's globs reach, keyed by their pinned commit. Omitted
    /// when there are none, so gitlink-free output is unchanged.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    gitlinks: Vec<GitlinkInput>,
    /// Symlinks the step's globs match or reach through, keyed by their own
    /// object id (the target path); the link is not followed. Omitted when
    /// there are none.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    symlinks: Vec<SymlinkInput>,
}

#[derive(Serialize)]
struct SymlinkInput {
    path: String,
    object: String,
}

#[derive(Serialize)]
struct GitlinkInput {
    path: String,
    commit: String,
}

pub fn run(
    manifest_path: &Path,
    step_filter: Option<&str>,
    commit_ref: &str,
    selection: rakia_core::inputs::InputSelection,
) -> Result<()> {
    let text = std::fs::read_to_string(manifest_path)?;
    let m: rakia_core::manifest::BuildManifest = serde_json::from_str(&text)?;

    // Discover the repo from the manifest's parent dir
    let manifest_dir = manifest_path
        .parent()
        .ok_or_else(|| CliError::Args(format!("manifest has no parent dir: {}", manifest_path.display())))?;
    let repo = crate::repository::discover(manifest_dir).map_err(|e| {
        CliError::Args(format!(
            "repo discovery failed for {}: {}",
            manifest_dir.display(),
            crate::error::format_error(&e)
        ))
    })?;

    // Resolve the commit ref to an ObjectId
    let commit_id = repo
        .rev_parse_single(commit_ref)
        .map_err(|e| {
            CliError::Args(format!(
                "could not resolve commit '{commit_ref}': {}",
                crate::error::format_error(&e)
            ))
        })?
        .detach();

    let mut out = Vec::new();
    for (name, step) in &m.steps {
        if let Some(filter) = step_filter {
            if name != filter {
                continue;
            }
        }
        let fp = match selection {
            rakia_core::inputs::InputSelection::All => {
                let mut all_patterns: Vec<String> = step.inputs.sources.clone();
                all_patterns.extend(step.inputs.build_process.iter().cloned());
                brit_graph::fingerprint::ContentFingerprint::from_repo_globs(&repo, commit_id, &all_patterns)
            }
            rakia_core::inputs::InputSelection::Sources => {
                let patterns = rakia_core::inputs::step_input_patterns(
                    &step.inputs.sources,
                    &step.inputs.build_process,
                    rakia_core::inputs::InputSelection::Sources,
                );
                brit_graph::fingerprint::ContentFingerprint::from_repo_globs_excluding(
                    &repo,
                    commit_id,
                    &patterns.include,
                    &patterns.exclude,
                )
            }
        }
        .map_err(|e| CliError::Args(format!("fingerprint compute failed for step '{name}': {e}")))?;

        out.push(StepFingerprint {
            pipeline: m.pipeline.clone(),
            step: name.clone(),
            fingerprint: fp.cid.to_string(),
            input_count: fp.input_count(),
            gitlinks: fp
                .gitlinks
                .iter()
                .map(|(path, commit)| GitlinkInput {
                    path: path.clone(),
                    commit: commit.clone(),
                })
                .collect(),
            symlinks: fp
                .symlinks
                .iter()
                .map(|(path, object)| SymlinkInput {
                    path: path.clone(),
                    object: object.clone(),
                })
                .collect(),
        });
    }

    crate::output::print_json(&FingerprintOutput {
        manifest: manifest_path.display().to_string(),
        commit: commit_id.to_hex().to_string(),
        fingerprints: out,
    })?;
    Ok(())
}
