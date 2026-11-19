use crate::OutputFormat;
use gix::{
    Result,
    bstr::{BStr, BString, ByteSlice},
    error::{OptionExt, ResultExt, bail},
    prelude::ObjectIdExt,
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

/// How a worktree uses a branch.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Use {
    /// The worktree's `HEAD` refers to the branch.
    CheckedOut,
    /// The worktree is rebasing or bisecting the branch.
    InProgress,
}

/// A worktree as seen from `repo`: the name addressing its `HEAD` in `repo`'s ref transactions,
/// its checkout (or Git directory), the branch `HEAD` refers to, and the worktree opened on its own.
struct Worktree {
    head: gix::refs::FullName,
    path: std::path::PathBuf,
    git_dir: std::path::PathBuf,
    head_name: Option<gix::refs::FullName>,
    repo: gix::Repository,
}

fn worktrees(repo: &gix::Repository) -> Result<Vec<Worktree>> {
    let current = gix::path::realpath(repo.git_dir()).or_error()?;
    let main = repo.main_repo()?;
    let mut worktrees = vec![(None, main)];
    for proxy in repo.worktrees()? {
        let id = proxy.id()?.to_owned();
        worktrees.push((Some(id), proxy.into_repo_with_possibly_inaccessible_worktree()?));
    }
    worktrees
        .into_iter()
        .map(|(id, worktree)| {
            let git_dir = worktree.git_dir().to_owned();
            let head = if gix::path::realpath(&git_dir).or_error()? == current {
                "HEAD".into()
            } else if let Some(id) = id {
                format!("worktrees/{id}/HEAD")
            } else {
                "main-worktree/HEAD".into()
            };
            Ok(Worktree {
                head: gix::refs::FullName::try_from(head).or_error()?,
                path: worktree.workdir().unwrap_or(&git_dir).to_owned(),
                head_name: worktree.head_name()?,
                git_dir,
                repo: worktree,
            })
        })
        .collect()
}

/// Read an in-progress marker; only a missing marker means "not in progress", so an unreadable one fails closed.
fn read_marker(worktree: &Worktree, file: &str) -> Result<Option<Vec<u8>>> {
    match std::fs::read(worktree.git_dir.join(file)) {
        Ok(bytes) => Ok(Some(bytes)),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(err) => Err(err).or_raise(|| {
            gix::error::message!(
                "cannot tell whether the worktree at '{}' is rebasing or bisecting: '{file}' is unreadable",
                worktree.path.display()
            )
        }),
    }
}

/// Where a branch is in use: a worktree's `HEAD`, or the branch a worktree is rebasing or bisecting.
fn worktree_using(worktrees: &[Worktree], name: &gix::refs::FullNameRef) -> Result<Option<(Use, std::path::PathBuf)>> {
    for worktree in worktrees {
        let mut in_progress = false;
        for file in ["rebase-merge/head-name", "rebase-apply/head-name"] {
            in_progress |= read_marker(worktree, file)?
                .is_some_and(|head_name| head_name.trim_ascii_end() == name.as_bstr().as_bytes());
        }
        in_progress |= read_marker(worktree, "BISECT_START")?
            .is_some_and(|start| start.trim_ascii_end() == name.shorten().as_bytes());
        if in_progress {
            return Ok(Some((Use::InProgress, worktree.path.clone())));
        }
        if worktree.head_name.as_ref().is_some_and(|head| head.as_ref() == name) {
            return Ok(Some((Use::CheckedOut, worktree.path.clone())));
        }
    }
    Ok(None)
}

/// The commit a branch tip must be contained in: its existing upstream, otherwise `HEAD`.
fn merge_reference(
    repo: &gix::Repository,
    name: &gix::refs::FullNameRef,
) -> Result<Option<(gix::ObjectId, Option<gix::refs::FullName>)>> {
    if let Some(upstream) = repo
        .branch_remote_tracking_ref_name(name, gix::remote::Direction::Fetch)
        .transpose()?
        && let Some(mut reference) = repo.try_find_reference(upstream.as_ref())?
        && let Ok(commit) = reference.peel_to_commit()
    {
        return Ok(Some((commit.id, Some(upstream))));
    }
    Ok(repo.head()?.id().map(|id| (id.detach(), None)))
}

fn contains(repo: &gix::Repository, descendant: gix::ObjectId, ancestor: gix::ObjectId) -> Result<bool> {
    Ok(descendant == ancestor
        || repo
            .merge_base(ancestor, descendant)?
            .is_some_and(|base| base == ancestor))
}

/// Delete local branches as `git branch --delete [--force]` does, but all of them or none.
pub fn delete(
    repo: gix::Repository,
    names: &[String],
    force: bool,
    out: &mut dyn std::io::Write,
    err: &mut dyn std::io::Write,
) -> Result<()> {
    use gix::refs::{
        Target,
        transaction::{Change, PreviousValue, RefEdit, RefLog},
    };
    let worktrees = worktrees(&repo)?;
    let mut edits = Vec::new();
    let mut reports = Vec::new();
    let mut seen = std::collections::BTreeSet::new();
    for short in names {
        let name = local_branch_name(short.as_bytes().as_bstr())?;
        if !seen.insert(name.clone()) {
            continue;
        }
        let Some(reference) = repo.try_find_reference(name.as_ref())? else {
            bail!(gix::error::not_found(format!("branch '{short}' not found")));
        };
        if let Some((_, path)) = worktree_using(&worktrees, name.as_ref())? {
            bail!(gix::error::validation(format!(
                "cannot delete branch '{short}' used by worktree at '{}'",
                path.display()
            )));
        }
        let target = reference.target().into_owned();
        let was = match &target {
            Target::Object(id) => {
                let id = *id;
                if !force {
                    let tip = repo
                        .find_object(id)?
                        .peel_to_commit()
                        .or_raise(|| gix::error::message!("branch '{short}' does not point at a commit"))?
                        .id;
                    let merged = match merge_reference(&repo, name.as_ref())? {
                        Some((reference, upstream)) => {
                            let merged = contains(&repo, reference, tip)?;
                            if merged && let Some(upstream) = upstream {
                                let head = repo.head()?.id().map(gix::Id::detach);
                                if !head
                                    .map(|head| contains(&repo, head, tip))
                                    .transpose()?
                                    .unwrap_or(false)
                                {
                                    writeln!(
                                        err,
                                        "warning: deleting branch '{short}' that has been merged to '{}', but not yet merged to HEAD",
                                        upstream.as_bstr()
                                    )
                                    .or_error()?;
                                }
                            }
                            merged
                        }
                        None => false,
                    };
                    if !merged {
                        bail!(gix::error::validation(format!(
                            "the branch '{short}' is not fully merged\n\
                             hint: If you are sure you want to delete it, run 'brit branch -D {short}'"
                        )));
                    }
                }
                id.attach(&repo).shorten_or_id().to_string()
            }
            Target::Symbolic(referent) => referent.shorten().to_string(),
        };
        reports.push(format!("Deleted branch {short} (was {was})."));
        edits.push(RefEdit {
            change: Change::Delete {
                expected: PreviousValue::MustExistAndMatch(target),
                log: RefLog::AndReference,
            },
            name,
            deref: false,
        });
    }
    repo.edit_references(edits)?;
    remove_branch_config(&repo, names)?;
    for report in reports {
        writeln!(out, "{report}").or_error()?;
    }
    Ok(())
}

/// Rewrite the repository-local configuration under its lock with `edit`, if `edit` changed anything.
fn edit_local_config(repo: &gix::Repository, edit: impl FnOnce(&mut gix::config::File) -> bool) -> Result<()> {
    use std::io::Write;
    let path = repo.common_dir().join("config");
    let mut lock = gix::lock::File::acquire_to_update_resource(&path, gix::lock::acquire::Fail::Immediately, None, 0)
        .or_raise(|| gix::error::message!("Could not lock configuration file at \"{}\"", path.display()))?;
    let input = std::fs::read(&path)
        .or_raise(|| gix::error::message!("Could not read configuration file at \"{}\"", path.display()))?;
    // Includes stay unresolved: only this file's own sections may be edited and written back.
    let mut config = gix::config::File::from_bytes_no_includes(
        &input,
        gix::config::file::Metadata::from(gix::config::Source::Local),
        Default::default(),
    )?;
    if !edit(&mut config) {
        return Ok(());
    }
    let mut bytes = Vec::with_capacity(input.len());
    config.write_to(&mut bytes).or_error()?;
    lock.write_all(&bytes).or_error()?;
    lock.commit()
        .map_err(|err| err.error)
        .or_raise(|| gix::error::message!("Could not commit configuration file at \"{}\"", path.display()))?;
    Ok(())
}

fn remove_branch_config(repo: &gix::Repository, names: &[String]) -> Result<()> {
    edit_local_config(repo, |config| {
        let mut changed = false;
        for name in names {
            while config
                .remove_section("branch", Some(name.as_bytes().as_bstr()))
                .is_some()
            {
                changed = true;
            }
        }
        changed
    })
    .or_raise(|| gix::error::message("branches were deleted, but their configuration could not be removed"))
}

/// Rename a local branch as `git branch --move` does: its reflog and configuration move with it,
/// and every worktree with it checked out follows, all in one reference transaction.
pub fn rename(repo: gix::Repository, old: &str, new: &str) -> Result<()> {
    use gix::refs::{
        Target,
        transaction::{Change, LogChange, PreviousValue, RefEdit, RefLog},
    };
    let old_name = local_branch_name(old.as_bytes().as_bstr())?;
    let new_name = local_branch_name(new.as_bytes().as_bstr())?;
    let Some(reference) = repo.try_find_reference(old_name.as_ref())? else {
        bail!(gix::error::not_found(format!("no branch named '{old}'")));
    };
    let renaming = old_name != new_name;
    if renaming && repo.try_find_reference(new_name.as_ref())?.is_some() {
        bail!(gix::error::validation(format!("a branch named '{new}' already exists")));
    }
    let Target::Object(tip) = reference.target().into_owned() else {
        bail!(gix::error::unsupported(format!(
            "branch '{old}' is a symbolic reference; renaming it is not yet supported"
        )));
    };
    let worktrees = worktrees(&repo)?;
    if let Some((Use::InProgress, path)) = worktree_using(&worktrees, old_name.as_ref())? {
        bail!(gix::error::validation(format!(
            "branch '{old}' is being rebased or bisected at '{}'",
            path.display()
        )));
    }
    // Every worktree whose HEAD follows the rename is rewritten, this one included. The
    // repository guard is held, so take each worktree's guard without waiting: a writer that
    // holds one (a push paused in its pre-push hook) refuses the rename before anything moves.
    let _worktree_guards = if renaming {
        worktrees
            .iter()
            .filter(|worktree| worktree.head_name.as_ref() == Some(&old_name))
            .map(|worktree| {
                crate::repository::switch::other_worktree_guard(&worktree.repo).map_err(|err| {
                    gix::error::Error::from_boxed(
                        err.context(format!(
                            "cannot rename branch '{old}': the worktree at '{}' that has it checked out is in use by another Brit writer",
                            worktree.path.display()
                        ))
                        .into_boxed_dyn_error(),
                    )
                })
            })
            .collect::<std::result::Result<Vec<_>, _>>()?
    } else {
        Vec::new()
    };
    if renaming {
        refuse_directory_conflicts(&repo, &old_name, &new_name)?;
    }
    let message: BString = format!("Branch: renamed {} to {}", old_name.as_bstr(), new_name.as_bstr()).into();
    let log_path = |name: &gix::refs::FullName| -> Result<std::path::PathBuf> {
        Ok(repo
            .common_dir()
            .join("logs")
            .join(gix::path::from_bstr(name.as_bstr())?))
    };
    let old_log = match std::fs::read(log_path(&old_name)?) {
        Ok(bytes) => Some(bytes),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => None,
        Err(err) => return Err(err).or_raise(|| gix::error::message!("Could not read the reflog of '{old}'")),
    };
    let committer = repo
        .committer()
        .ok_or_raise(|| gix::error::validation("committer identity required for the rename reflog entry"))??
        .to_owned()?;
    // Like Git, keep the old history and record the rename as a no-op move of the tip.
    let carried_log = |old_log: Vec<u8>| -> Result<()> {
        let mut bytes = old_log;
        gix::refs::log::Line {
            previous_oid: tip,
            new_oid: tip,
            signature: committer.clone(),
            message: message.clone(),
        }
        .write_to(&mut bytes)
        .or_error()?;
        let path = log_path(&new_name)?;
        let mut lock =
            gix::lock::File::acquire_to_update_resource(&path, gix::lock::acquire::Fail::Immediately, None, 0)
                .or_raise(|| gix::error::message!("branch was renamed, but its reflog could not be locked"))?;
        std::io::Write::write_all(&mut lock, &bytes).or_error()?;
        lock.commit()
            .map_err(|err| err.error)
            .or_raise(|| gix::error::message!("branch was renamed, but its reflog could not be carried over"))?;
        Ok(())
    };
    if !renaming {
        // Git treats renaming a branch to itself as a no-op that still records the rename.
        return old_log.map_or(Ok(()), carried_log);
    }
    let log = || LogChange {
        mode: RefLog::AndReference,
        force_create_reflog: false,
        message: message.clone(),
    };
    let mut edits = vec![
        RefEdit {
            change: Change::Update {
                log: LogChange {
                    force_create_reflog: old_log.is_some(),
                    ..log()
                },
                expected: PreviousValue::MustNotExist,
                new: Target::Object(tip),
            },
            name: new_name.clone(),
            deref: false,
        },
        RefEdit {
            change: Change::Delete {
                expected: PreviousValue::MustExistAndMatch(Target::Object(tip)),
                log: RefLog::AndReference,
            },
            name: old_name.clone(),
            deref: false,
        },
    ];
    edits.extend(
        worktrees
            .into_iter()
            .filter(|worktree| worktree.head_name.as_ref() == Some(&old_name))
            .map(|worktree| RefEdit {
                change: Change::Update {
                    log: log(),
                    expected: PreviousValue::MustExistAndMatch(Target::Symbolic(old_name.clone())),
                    new: Target::Symbolic(new_name.clone()),
                },
                name: worktree.head,
                deref: false,
            }),
    );
    // Worktree HEADs follow without a HEAD reflog entry: gix writes no reflog for symbolic updates.
    repo.edit_references(edits)?;
    if let Some(old_log) = old_log {
        carried_log(old_log)?;
    }
    edit_local_config(&repo, |config| {
        config
            .rename_section(
                "branch",
                Some(old.as_bytes().as_bstr()),
                "branch",
                Some(BString::from(new)),
            )
            .is_ok()
    })
    .or_raise(|| gix::error::message("branch was renamed, but its configuration could not be moved"))
}

/// Refuse a rename whose target would need a directory where a ref file is, or the reverse,
/// naming the conflicting ref as Git does. Loose and packed refs are both considered.
fn refuse_directory_conflicts(
    repo: &gix::Repository,
    old_name: &gix::refs::FullName,
    new_name: &gix::refs::FullName,
) -> Result<()> {
    let new = new_name.as_bstr();
    let conflicts = |existing: &BStr| {
        let nested = |outer: &BStr, inner: &BStr| {
            inner.len() > outer.len() && inner.starts_with(outer) && inner[outer.len()] == b'/'
        };
        nested(existing, new) || nested(new, existing)
    };
    let mut conflict_with_old = false;
    for reference in repo.references()?.all()? {
        let reference = reference?;
        let existing = reference.name().as_bstr();
        if !conflicts(existing) {
            continue;
        }
        if existing == old_name.as_bstr() {
            conflict_with_old = true;
        } else {
            bail!(gix::error::validation(format!(
                "'{existing}' exists; cannot create '{new}'"
            )));
        }
    }
    if conflict_with_old {
        // Git deletes the old ref before creating the new one; one gix transaction cannot.
        bail!(gix::error::unsupported(format!(
            "cannot rename '{old}' to '{new}' in one transaction while '{old}' blocks the new path; \
             rename it through an unrelated temporary name instead",
            old = old_name.as_bstr()
        )));
    }
    Ok(())
}
