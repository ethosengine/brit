//! brit plan — topologically grouped build plan, conforming to build-plan.schema.json.

use std::path::Path;

use crate::error::{CliError, Result};

const TOOL_VERSION: &str = env!("CARGO_PKG_VERSION");

pub fn run(
    repo: &Path,
    files: Option<&str>,
    since: Option<&str>,
    pipeline: Option<&str>,
) -> Result<()> {
    let repo = repo.canonicalize().map_err(|source| CliError::RepoNotFound {
        path: repo.display().to_string(),
        source,
    })?;

    let (changed_paths, baseline_ref, baseline_commit, head_commit) = if let Some(files) = files {
        let paths: Vec<String> = files
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
        // For --files mode, baseline + head are not git-derived; use placeholders
        // (40 zeros for both — schema accepts them as long as they're 40 hex chars)
        (paths, "(none)".to_string(), "0".repeat(40), "0".repeat(40))
    } else if let Some(since) = since {
        let head_commit_sha = rakia_brit::changes::head_commit(&repo)
            .map_err(|e| CliError::ChangeDetection(format!("{e}")))?;
        let baseline_commit_sha = rakia_brit::changes::resolve_ref(&repo, since)
            .map_err(|e| CliError::ChangeDetection(format!("{e}")))?;
        let paths = rakia_brit::changes::changed_paths_since(&repo, since, "HEAD")
            .map_err(|e| CliError::ChangeDetection(format!("{e}")))?;
        let ref_name = if let Some(p) = pipeline {
            format!("refs/notes/rakia/baselines/{p}")
        } else {
            since.to_string()
        };
        (paths, ref_name, baseline_commit_sha, head_commit_sha)
    } else {
        return Err(CliError::Args("need --files or --since".into()));
    };

    let manifests = rakia_core::discover::discover_manifests(&repo)
        .map_err(|e| CliError::ManifestDiscovery(format!("{e}")))?;
    let constellation = rakia_core::constellation::build_constellation(&manifests)?;
    let plan = rakia_core::constellation::plan_from_changes(&constellation, &changed_paths)?;

    let bp = rakia_core::build_plan::to_build_plan(
        &plan,
        &baseline_ref,
        &baseline_commit,
        &head_commit,
        &changed_paths,
        TOOL_VERSION,
    );

    crate::output::print_json(&bp)?;
    Ok(())
}
