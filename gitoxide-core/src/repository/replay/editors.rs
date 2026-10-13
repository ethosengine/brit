//! Byte-safe editor selection and the existing supervised invocation protocol.
use std::{ffi::OsString, fs, io::IsTerminal, os::unix::fs::MetadataExt, path::Path, process::Stdio};

use anyhow::{Context, Result, ensure};
use gix::bstr::ByteSlice;
use serde::{Deserialize, Serialize};

use crate::repository::{authoring::invocation, switch};

/// Sequence editing has an additional precedence level above normal editing.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum Kind {
    Sequence,
    Message,
}

/// Persist this with the parent's invocation identifier before releasing a child.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Plan {
    pub kind: Kind,
    pub program: Vec<u8>,
    pub output: Vec<u8>,
    pub input_digest: String,
    pub policy: String,
    pub terminal: bool,
}

fn configured(repo: &gix::Repository, kind: Kind, terminal: bool) -> Result<OsString> {
    let config = repo.config_snapshot();
    let env = |name| std::env::var_os(name);
    let value = |key| -> Result<Option<OsString>> {
        config
            .string(key)
            .map(|value| Ok(gix::path::from_bstr(value.as_bstr())?.into_owned().into_os_string()))
            .transpose()
    };
    let normal = || -> Result<OsString> {
        if let Some(editor) = env("GIT_EDITOR") {
            return Ok(editor);
        }
        if let Some(editor) = value("core.editor")? {
            return Ok(editor);
        }
        if let Some(editor) = env("VISUAL") {
            return Ok(editor);
        }
        if let Some(editor) = env("EDITOR") {
            return Ok(editor);
        }
        ensure!(
            terminal,
            "configure an editor; vi fallback requires an interactive terminal"
        );
        Ok("vi".into())
    };
    let program = if kind == Kind::Sequence {
        if let Some(editor) = env("GIT_SEQUENCE_EDITOR") {
            editor
        } else if let Some(editor) = value("sequence.editor")? {
            editor
        } else {
            normal()?
        }
    } else {
        normal()?
    };
    ensure!(
        !program.as_encoded_bytes().is_empty(),
        "explicitly empty editor setting"
    );
    ensure!(!program.as_encoded_bytes().contains(&0), "editor contains NUL");
    Ok(program)
}

/// Resolve exact program/configuration and input bytes without invoking anything.
pub fn resolve(repo: &gix::Repository, kind: Kind, file: &Path) -> Result<Plan> {
    let file = repo.current_dir().join(file);
    let metadata = fs::symlink_metadata(&file)?;
    ensure!(
        metadata.is_file() && metadata.uid() == fs::metadata(repo.git_dir())?.uid(),
        "unsafe editor input"
    );
    resolve_input(repo, kind, &file, &fs::read(&file)?)
}

/// Resolve before creating the output so its exact bytes can be owned durably.
pub fn resolve_input(repo: &gix::Repository, kind: Kind, file: &Path, input: &[u8]) -> Result<Plan> {
    ensure!(
        repo.git_dir_trust() == gix::sec::Trust::Full,
        "editor execution requires a trusted repository"
    );
    let file = repo.current_dir().join(file);
    let terminal = std::io::stdin().is_terminal() && std::io::stdout().is_terminal();
    let input_digest = switch::digest(input)?;
    Ok(Plan {
        kind,
        program: configured(repo, kind, terminal)?.as_encoded_bytes().to_vec(),
        output: gix::path::into_bstr(file)?.to_vec(),
        input_digest,
        policy: super::hooks::policy(repo)?,
        terminal,
    })
}

/// Run once. The callback durably binds `plan` and the invocation ID in the parent.
pub fn run(
    repo: &gix::Repository,
    directory: &Path,
    plan: &Plan,
    index: &Path,
    durable_start: impl FnOnce() -> Result<()>,
) -> Result<invocation::InvocationOutcome> {
    let file = gix::path::from_bstr(plan.output.as_bstr())?;
    ensure!(
        resolve(repo, plan.kind, &file)? == *plan,
        "editor plan/configuration/input changed"
    );
    let prepared = invocation::prepare(directory)?;
    durable_start()?;
    let program = gix::path::from_bstr(plan.program.as_bstr())?
        .into_owned()
        .into_os_string();
    // Trusted configured command parsing is delegated to gix. Repository path
    // bytes stay a separate argument, even when the editor itself needs a shell.
    let editor: std::process::Command = gix::command::prepare(program)
        .with_shell()
        .arg(file.as_os_str())
        .try_into()?;
    let mut context = repo.command_context()?;
    context.git_dir = Some(repo.current_dir().join(repo.git_dir()));
    context.worktree_dir = repo.workdir().map(|p| repo.current_dir().join(p));
    let barrier = if plan.terminal {
        "IFS= read -r brit_gate && test \"$brit_gate\" = go && exec \"$@\" </dev/tty"
    } else {
        "IFS= read -r brit_gate && test \"$brit_gate\" = go && exec \"$@\""
    };
    let mut command: std::process::Command = gix::command::prepare(barrier)
        .with_shell()
        .with_context(context)
        .arg(editor.get_program())
        .args(editor.get_args())
        .env("GIT_INDEX_FILE", repo.current_dir().join(index).into_os_string())
        .stdin(Stdio::piped())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .try_into()?;
    command.current_dir(repo.workdir().context("editor requires worktree")?);
    invocation::run_editor(command, prepared, plan.terminal)
}
