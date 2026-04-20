//! brit affected — which steps are affected by changes, with provenance.

use std::path::Path;

use serde::Serialize;

use crate::error::{CliError, Result};

#[derive(Serialize)]
struct AffectedOutput {
    changed_paths: Vec<String>,
    affected: Vec<AffectedStep>,
}

#[derive(Serialize)]
struct AffectedStep {
    qualified_name: String,
    affected_by: Vec<rakia_core::generated_types::AffectedReason>,
}

pub fn run(repo: &Path, files: Option<&str>, since: Option<&str>) -> Result<()> {
    let repo = repo.canonicalize().map_err(|source| CliError::RepoNotFound {
        path: repo.display().to_string(),
        source,
    })?;

    let changed_paths: Vec<String> = if let Some(files) = files {
        files
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect()
    } else if let Some(since) = since {
        rakia_brit::changes::changed_paths_since(&repo, since, "HEAD")
            .map_err(|e| CliError::ChangeDetection(format!("{e}")))?
    } else {
        return Err(CliError::Args("need --files or --since".into()));
    };

    let manifests = rakia_core::discover::discover_manifests(&repo)
        .map_err(|e| CliError::ManifestDiscovery(format!("{e}")))?;
    let constellation = rakia_core::constellation::build_constellation(&manifests)?;
    let plan = rakia_core::constellation::plan_from_changes(&constellation, &changed_paths)?;

    // Flatten plan levels into a single affected list — `affected` doesn't care about ordering
    let mut affected: Vec<AffectedStep> = Vec::new();
    for level in &plan.levels {
        for (step, reasons) in level {
            affected.push(AffectedStep {
                qualified_name: step.qualified_name.clone(),
                affected_by: reasons.clone(),
            });
        }
    }

    crate::output::print_json(&AffectedOutput {
        changed_paths,
        affected,
    })?;
    Ok(())
}
