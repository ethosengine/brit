use crate::OutputFormat;
use gix::{
    Result,
    bstr::{BStr, BString, ByteSlice},
    error::{ResultExt, bail},
};

/// Create only a new local ref, leaving HEAD, index and worktree untouched.
pub fn create(repo: gix::Repository, name: &str, start_point: &str) -> Result<()> {
    let name = local_branch_name(name.as_bytes().as_bstr())?;
    let commit_id = repo.rev_parse_single(start_point)?.object()?.peel_to_commit()?.id;
    repo.reference(
        name,
        commit_id,
        gix::refs::transaction::PreviousValue::MustNotExist,
        format!("branch: Created from {start_point}"),
    )?;
    Ok(())
}

pub(crate) fn local_branch_name(name: &BStr) -> Result<gix::refs::FullName> {
    if name.starts_with(b"-") {
        bail!(gix::error::validation("Branch names must not start with '-'"));
    }
    let mut full_name: BString = "refs/heads/".into();
    full_name.extend_from_slice(name);
    gix::validate::reference::branch_name(full_name.as_bstr()).or_error()?;
    gix::refs::FullName::try_from(full_name).or_error()
}

pub mod list {
    pub enum Kind {
        Local,
        All,
    }

    pub struct Options {
        pub kind: Kind,
    }
}

pub fn list(
    repo: gix::Repository,
    out: &mut dyn std::io::Write,
    format: OutputFormat,
    options: list::Options,
) -> Result<()> {
    if format != OutputFormat::Human {
        bail!(gix::error::unsupported("JSON output isn't supported"));
    }

    let platform = repo.references()?;

    let (show_local, show_remotes) = match options.kind {
        list::Kind::Local => (true, false),
        list::Kind::All => (true, true),
    };

    if show_local {
        let mut branch_names: Vec<String> = platform
            .local_branches()?
            .flatten()
            .map(|branch| branch.name().shorten().to_string())
            .collect();

        branch_names.sort();

        for branch_name in branch_names {
            writeln!(out, "{branch_name}").or_error()?;
        }
    }

    if show_remotes {
        let mut branch_names: Vec<String> = platform
            .remote_branches()?
            .flatten()
            .map(|branch| branch.name().shorten().to_string())
            .collect();

        branch_names.sort();

        for branch_name in branch_names {
            writeln!(out, "{branch_name}").or_error()?;
        }
    }

    Ok(())
}
