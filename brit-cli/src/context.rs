//! A transport for the installed native evaluator, with no parallel ledger or sibling lookup.

use std::{ffi::OsString, process::Command};

use crate::error::{CliError, Result};

pub(crate) fn run(args: Vec<OsString>) -> Result<()> {
    installed(&["flow", "context"], args)
}

pub(crate) fn integrate(args: Vec<OsString>) -> Result<()> {
    let mut root = std::path::PathBuf::from(".");
    let mut cursor = args.iter();
    while let Some(arg) = cursor.next() {
        if arg == "--root" {
            if let Some(value) = cursor.next() {
                root = value.into();
            }
        } else if let Some(value) = arg.to_str().and_then(|arg| arg.strip_prefix("--root=")) {
            root = value.into();
        }
    }
    let mut guards = gitoxide::plumbing::MutationGuards::default();
    guards.path(&root).map_err(|err| CliError::Git(err.into()))?;
    installed(&["integrate"], args)
}

fn installed(prefix: &[&str], args: Vec<OsString>) -> Result<()> {
    let status = Command::new("epr").args(prefix).args(args).status().map_err(|err| {
        CliError::Git(anyhow::anyhow!(
            "could not run installed `epr {}`: {err}; install epr and make it available on PATH",
            prefix.join(" ")
        ))
    })?;
    if status.success() {
        Ok(())
    } else {
        Err(CliError::ContextExit(status.code().unwrap_or(1)))
    }
}
