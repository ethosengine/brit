//! A transport for the installed native evaluator, with no parallel ledger or sibling lookup.

use std::{ffi::OsString, process::Command};

use crate::error::{CliError, Result};

pub(crate) fn run(args: Vec<OsString>) -> Result<()> {
    let status = Command::new("epr")
        .args(["flow", "context"])
        .args(args)
        .status()
        .map_err(|err| {
            CliError::Git(anyhow::anyhow!(
                "could not run installed `epr flow context`: {err}; install epr and make it available on PATH"
            ))
        })?;
    if status.success() {
        Ok(())
    } else {
        Err(CliError::ContextExit(status.code().unwrap_or(1)))
    }
}
