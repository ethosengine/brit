//! brit baseline — read, write, and migrate baseline refs.

use std::path::Path;

use serde::Serialize;

use crate::error::{CliError, Result};

#[derive(Serialize)]
struct BaselineRead {
    pipeline: String,
    r#ref: String,
    commit: Option<String>,
}

#[derive(Serialize)]
struct BaselineWrite {
    pipeline: String,
    r#ref: String,
    commit: String,
    written: bool,
}

#[derive(Serialize)]
struct BaselineMigrate {
    source: String,
    migrated: Vec<String>,
    count: usize,
}

pub fn read(repo: &Path, pipeline: &str) -> Result<()> {
    let repo = repo.canonicalize().map_err(|source| CliError::RepoNotFound {
        path: repo.display().to_string(),
        source,
    })?;
    let commit = rakia_brit::baselines::read_baseline(&repo, pipeline)
        .map_err(|e| CliError::Baseline(format!("{e}")))?;
    crate::output::print_json(&BaselineRead {
        pipeline: pipeline.to_string(),
        r#ref: format!("refs/notes/rakia/baselines/{pipeline}"),
        commit,
    })?;
    Ok(())
}

pub fn write(repo: &Path, pipeline: &str, commit: &str) -> Result<()> {
    let repo = repo.canonicalize().map_err(|source| CliError::RepoNotFound {
        path: repo.display().to_string(),
        source,
    })?;
    rakia_brit::baselines::write_baseline(&repo, pipeline, commit)
        .map_err(|e| CliError::Baseline(format!("{e}")))?;
    crate::output::print_json(&BaselineWrite {
        pipeline: pipeline.to_string(),
        r#ref: format!("refs/notes/rakia/baselines/{pipeline}"),
        commit: commit.to_string(),
        written: true,
    })?;
    Ok(())
}

pub fn migrate(repo: &Path, json_path: &Path) -> Result<()> {
    let repo = repo.canonicalize().map_err(|source| CliError::RepoNotFound {
        path: repo.display().to_string(),
        source,
    })?;
    let migrated = rakia_brit::baselines::migrate_baselines(&repo, json_path)
        .map_err(|e| CliError::Baseline(format!("{e}")))?;
    let count = migrated.len();
    crate::output::print_json(&BaselineMigrate {
        source: json_path.display().to_string(),
        migrated,
        count,
    })?;
    Ok(())
}
