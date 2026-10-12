//! A transport for the installed native evaluator, with no parallel ledger or sibling lookup.

use std::{ffi::OsString, process::Command};

use crate::error::{CliError, Result};

pub(crate) fn run(args: Vec<OsString>) -> Result<()> {
    installed(&["flow", "context"], args)
}

pub(crate) fn integrate(args: Vec<OsString>) -> Result<()> {
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
