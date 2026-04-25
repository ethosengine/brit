use std::collections::HashSet;

use gix::prelude::ObjectIdExt;

use crate::OutputFormat;

/// Compute the ancestor set of `rev` (inclusive — the rev itself is
/// in the set). Used by `--merged` / `--no-merged` to test whether a
/// branch tip is reachable from the needle commit.
fn ancestors_of(repo: &gix::Repository, rev: &std::ffi::OsStr) -> anyhow::Result<HashSet<gix::ObjectId>> {
    let rev_bstr = gix::path::os_str_into_bstr(rev)?;
    let oid = repo.rev_parse_single(rev_bstr)?.detach();
    let mut set = HashSet::new();
    set.insert(oid);
    for res in oid.attach(repo).ancestors().all()? {
        let info = res?;
        set.insert(info.id);
    }
    Ok(set)
}

/// Return `true` iff `haystack_commit` has `needle` anywhere in its
/// ancestry (walking parents). Inclusive — a commit contains itself.
/// Mirrors git's `commit_contains` semantic used for branch/tag
/// `--contains` filtering.
fn commit_contains(
    repo: &gix::Repository,
    haystack_commit: gix::ObjectId,
    needle: gix::ObjectId,
) -> anyhow::Result<bool> {
    if haystack_commit == needle {
        return Ok(true);
    }
    for res in haystack_commit.attach(repo).ancestors().all()? {
        if res?.id == needle {
            return Ok(true);
        }
    }
    Ok(false)
}

/// `git branch <name> [<start-point>]`: create `refs/heads/<name>`
/// pointing at the resolved `<start-point>` (or HEAD if absent).
/// Without `--force`, the ref must not already exist. Matches the
/// create path in builtin/branch.c's cmd_branch + create_branch.
///
/// `track` — when `Some(_)` and `no_track` is false, write
/// `branch.<name>.{remote,merge}` so `<name>` tracks `<start-point>`.
/// Only the "direct" mode is implemented (use start-point itself as
/// upstream). `inherit` mode (copy upstream from start-point's existing
/// upstream) is a TODO: it requires a separate upstream-lookup path and
/// is not exercised by the parity fixture.
///
/// `branch.autoSetupMerge` implicit tracking is OUT OF SCOPE for this
/// sprint — it is deferred as a future parity row.
#[allow(clippy::too_many_arguments)]
pub fn create(
    mut repo: gix::Repository,
    name: gix::bstr::BString,
    start_point: Option<gix::bstr::BString>,
    force: bool,
    recurse_submodules: bool,
    track: Option<gix::bstr::BString>,
    no_track: bool,
    out: &mut dyn std::io::Write,
    err: &mut dyn std::io::Write,
) -> anyhow::Result<i32> {
    use gix::bstr::ByteSlice as _;
    use gix::refs::transaction::{Change, LogChange, PreviousValue, RefEdit};

    // git rejects --recurse-submodules unless
    // submodule.propagateBranches=true is configured (the option is
    // experimental; without the gate-config it dies 128 with the
    // exact stanza below).
    if recurse_submodules {
        let propagate = repo
            .config_snapshot()
            .boolean("submodule.propagateBranches")
            .unwrap_or(false);
        if !propagate {
            writeln!(
                err,
                "fatal: branch with --recurse-submodules can only be used if submodule.propagateBranches is enabled"
            )?;
            return Ok(128);
        }
    }

    let full = format!("refs/heads/{name}");
    // Already-exists pre-check: gix-ref's MustNotExist treats
    // same-target updates as a silent no-op, so we have to detect the
    // existing ref ourselves to match git's
    // "fatal: a branch named '<name>' already exists" + exit 128.
    // --force bypasses the check.
    if !force && repo.try_find_reference(full.as_str())?.is_some() {
        writeln!(err, "fatal: a branch named '{name}' already exists")?;
        return Ok(128);
    }

    // Resolve start-point to OID. For the --track path we also try to
    // find a ref by that name so we can record its FullName as upstream.
    // DWIM order: bare name → refs/heads/<name> → refs/remotes/<name>,
    // matching git's lookup order in create_branch / setup_tracking.
    let (target, start_point_full_ref): (gix::ObjectId, Option<gix::refs::FullName>) = match start_point {
        Some(ref rev) => {
            let rev_bstr: &gix::bstr::BStr = rev.as_ref();
            let rev_str = rev_bstr.to_str_lossy();
            // Try ref lookup for the upstream-config side.
            let start_ref_full = if track.is_some() && !no_track {
                let refs_heads = format!("refs/heads/{rev_str}");
                let refs_remotes = format!("refs/remotes/{rev_str}");
                if let Some(r) = repo.try_find_reference(rev_str.as_ref())? {
                    Some(r.name().to_owned())
                } else if let Some(r) = repo.try_find_reference(refs_heads.as_str())? {
                    Some(r.name().to_owned())
                } else {
                    repo.try_find_reference(refs_remotes.as_str())?.map(|r| r.name().to_owned())
                }
            } else {
                None
            };
            let oid = repo.rev_parse_single(rev_bstr)?.detach();
            (oid, start_ref_full)
        }
        None => (repo.head_id()?.detach(), None),
    };

    let full_name: gix::refs::FullName = full
        .try_into()
        .map_err(|err| anyhow::anyhow!("invalid refname: {err:?}"))?;
    let expected = if force {
        PreviousValue::Any
    } else {
        PreviousValue::MustNotExist
    };
    let log = LogChange {
        message: format!("branch: Created from {target}").into(),
        ..Default::default()
    };
    repo.edit_reference(RefEdit {
        change: Change::Update {
            log,
            expected,
            new: gix::refs::Target::Object(target),
        },
        name: full_name,
        deref: false,
    })?;

    // --track config write: after the ref is created, record upstream.
    // --no-track wins over --track; they are mutually exclusive at the
    // CLI level but we guard defensively with the OR.
    // Mirrors git's setup_tracking() call in create_branch().
    if track.is_some() && !no_track {
        if let Some(upstream_full) = start_point_full_ref {
            repo.set_branch_upstream(name.as_ref(), upstream_full.as_ref())?;
            // git's exact stdout on success (from install_branch_config_multiple_remotes):
            //   branch '<name>' set up to track '<upstream_short>'.
            let upstream_short = upstream_full.shorten().to_str_lossy();
            writeln!(out, "branch '{name}' set up to track '{upstream_short}'.")?;
            out.flush().ok();
        }
        // If start-point is a bare hash (no ref found), silently skip —
        // git would not write tracking config for a detached commit either.
    }

    Ok(0)
}

/// `git branch -m/-M [<old>] <new>`: rename `refs/heads/<old>` to
/// `refs/heads/<new>`, preserving its target. With one positional, old
/// = current branch (HEAD). Without --force, fails if <new> already
/// exists. Matches the rename path in builtin/branch.c's
/// rename_branch helper. The reflog is renamed alongside as a
/// follow-up gix-ref refinement; this implementation only moves the
/// ref pointer and leaves reflog files in place.
pub fn rename(
    repo: gix::Repository,
    old: Option<gix::bstr::BString>,
    new: gix::bstr::BString,
    force: bool,
    err: &mut dyn std::io::Write,
) -> anyhow::Result<i32> {
    use gix::refs::transaction::{Change, LogChange, PreviousValue, RefEdit};

    let new_full = format!("refs/heads/{new}");
    let old_short = match old {
        Some(s) => s,
        None => match repo.head_name()? {
            Some(name) => name.shorten().to_owned(),
            None => {
                writeln!(err, "fatal: HEAD is detached; cannot rename without an explicit <old>")?;
                return Ok(128);
            }
        },
    };
    let old_full = format!("refs/heads/{old_short}");

    if !force && repo.try_find_reference(new_full.as_str())?.is_some() {
        writeln!(err, "fatal: a branch named '{new}' already exists")?;
        return Ok(128);
    }
    let Some(old_ref) = repo.try_find_reference(old_full.as_str())? else {
        writeln!(err, "error: refname {old_short} not found")?;
        return Ok(1);
    };
    let target_oid = old_ref.clone().into_fully_peeled_id()?.detach();

    let new_full_name: gix::refs::FullName = new_full
        .try_into()
        .map_err(|err| anyhow::anyhow!("invalid refname: {err:?}"))?;
    let old_full_name: gix::refs::FullName = old_full
        .try_into()
        .map_err(|err| anyhow::anyhow!("invalid refname: {err:?}"))?;
    let new_log = LogChange {
        message: format!("Branch: renamed {old_short} to {new}").into(),
        ..Default::default()
    };
    repo.edit_references([
        RefEdit {
            change: Change::Update {
                log: new_log,
                expected: if force {
                    PreviousValue::Any
                } else {
                    PreviousValue::MustNotExist
                },
                new: gix::refs::Target::Object(target_oid),
            },
            name: new_full_name,
            deref: false,
        },
        RefEdit {
            change: Change::Delete {
                expected: PreviousValue::Any,
                log: gix::refs::transaction::RefLog::AndReference,
            },
            name: old_full_name,
            deref: false,
        },
    ])?;
    Ok(0)
}

/// `git branch -c/-C [<old>] <new>`: copy `refs/heads/<old>` to a new
/// `refs/heads/<new>` ref pointing at the same target. `<old>`
/// defaults to the current branch (HEAD) when only one positional is
/// given. Without `--force` / `-C`, fails if `<new>` already exists.
/// Mirrors the copy path in builtin/branch.c's copy_branch helper.
/// The reflog + config-section duplication git's copy does (clones
/// `branch.<old>.*` config keys to `branch.<new>.*` and copies the
/// reflog file) is deferred — this implementation only writes the
/// new ref.
pub fn copy(
    repo: gix::Repository,
    old: Option<gix::bstr::BString>,
    new: gix::bstr::BString,
    force: bool,
    err: &mut dyn std::io::Write,
) -> anyhow::Result<i32> {
    use gix::refs::transaction::{Change, LogChange, PreviousValue, RefEdit};

    let new_full = format!("refs/heads/{new}");
    let old_short = match old {
        Some(s) => s,
        None => match repo.head_name()? {
            Some(name) => name.shorten().to_owned(),
            None => {
                writeln!(err, "fatal: HEAD is detached; cannot copy without an explicit <old>")?;
                return Ok(128);
            }
        },
    };
    let old_full = format!("refs/heads/{old_short}");

    if !force && repo.try_find_reference(new_full.as_str())?.is_some() {
        writeln!(err, "fatal: a branch named '{new}' already exists")?;
        return Ok(128);
    }
    // Refuse to force-update the branch that is checked out in this
    // worktree (or any linked worktree). Matches git's
    // "cannot force update the branch '<name>' used by worktree at
    // '<path>'" + 128 stanza in builtin/branch.c.
    if force {
        if let Some(head_short_name) = repo.head_name()? {
            let head_short_bytes: &[u8] = head_short_name.shorten().as_ref();
            let new_bytes: &[u8] = new.as_ref();
            if head_short_bytes == new_bytes {
                let path = match repo.workdir() {
                    Some(p) => gix::path::realpath(p)
                        .unwrap_or_else(|_| p.to_path_buf())
                        .display()
                        .to_string(),
                    None => String::new(),
                };
                writeln!(
                    err,
                    "fatal: cannot force update the branch '{new}' used by worktree at '{path}'"
                )?;
                return Ok(128);
            }
        }
    }
    let Some(old_ref) = repo.try_find_reference(old_full.as_str())? else {
        writeln!(err, "error: refname {old_short} not found")?;
        return Ok(1);
    };
    let target_oid = old_ref.clone().into_fully_peeled_id()?.detach();

    let new_full_name: gix::refs::FullName = new_full
        .try_into()
        .map_err(|err| anyhow::anyhow!("invalid refname: {err:?}"))?;
    let log = LogChange {
        message: format!("Branch: copied {old_short} to {new}").into(),
        ..Default::default()
    };
    repo.edit_reference(RefEdit {
        change: Change::Update {
            log,
            expected: if force {
                PreviousValue::Any
            } else {
                PreviousValue::MustNotExist
            },
            new: gix::refs::Target::Object(target_oid),
        },
        name: new_full_name,
        deref: false,
    })?;
    Ok(0)
}

/// `git branch -d/-D [-r] <name>...`: delete each named branch. Each
/// missing name prints "error: branch '<name>' not found." and
/// contributes to a non-zero final exit. Successfully deleted
/// branches print
///   "Deleted branch <name> (was <abbrev-sha>)."
/// to stdout. Without `-D`, git also enforces a merged-into-upstream
/// check before deleting; that check is currently deferred (gix
/// behaves as if -D were always passed when -d is given). With `-r`,
/// the prefix becomes `refs/remotes/` instead of `refs/heads/`.
pub fn delete(
    repo: gix::Repository,
    names: Vec<gix::bstr::BString>,
    remotes: bool,
    out: &mut dyn std::io::Write,
    err: &mut dyn std::io::Write,
) -> anyhow::Result<i32> {
    use gix::refs::transaction::{Change, PreviousValue, RefEdit, RefLog};

    let prefix = if remotes { "refs/remotes/" } else { "refs/heads/" };
    let label = if remotes { "remote-tracking branch" } else { "branch" };
    let mut errors = 0i32;
    for name in names {
        let full = format!("{prefix}{name}");
        let Some(reference) = repo.try_find_reference(full.as_str())? else {
            writeln!(err, "error: {label} '{name}' not found")?;
            errors += 1;
            continue;
        };
        let target_oid = reference.clone().into_fully_peeled_id()?.detach();
        let abbrev: String = target_oid.to_string().chars().take(7).collect();
        let full_name: gix::refs::FullName = full
            .clone()
            .try_into()
            .map_err(|err| anyhow::anyhow!("invalid refname: {err:?}"))?;
        repo.edit_reference(RefEdit {
            change: Change::Delete {
                expected: PreviousValue::Any,
                log: RefLog::AndReference,
            },
            name: full_name,
            deref: false,
        })?;
        writeln!(out, "Deleted {label} {name} (was {abbrev}).")?;
    }
    Ok(if errors > 0 { 1 } else { 0 })
}

/// `git branch -u <upstream> [<branch>]` / `git branch --set-upstream-to=<upstream> [<branch>]`:
/// write `branch.<name>.remote` + `branch.<name>.merge` into the repo config.
///
/// `branch_short` is the short branch name to configure (e.g. `dev`); when `None` the current
/// branch is resolved from HEAD (detached HEAD exits 128 as git does). `upstream` is the
/// user-supplied string — a full refname, a short local-branch name, or a
/// `<remote>/<branch>` remote-tracking shorthand. We DWIM: try the value literally as a
/// full ref, then under `refs/heads/`, then under `refs/remotes/`. On success writes:
///
///   branch '<name>' set up to track '<upstream_short>'.
///
/// This matches the exact bytes git emits for the local-branch case
/// (`install_branch_config_multiple_remotes`, builtin/branch.c).
pub fn set_upstream_to(
    mut repo: gix::Repository,
    branch_short: Option<&gix::bstr::BStr>,
    upstream: &gix::bstr::BStr,
    out: &mut dyn std::io::Write,
    err: &mut dyn std::io::Write,
) -> anyhow::Result<()> {
    use gix::bstr::ByteSlice as _;

    // Resolve target branch name.
    let target: gix::bstr::BString = match branch_short {
        Some(b) => b.to_owned(),
        None => match repo.head_name()? {
            Some(name) => name.shorten().to_owned(),
            None => {
                writeln!(err, "fatal: HEAD not pointing to a branch")?;
                err.flush().ok();
                std::process::exit(128);
            }
        },
    };

    // DWIM: try the value as a full ref, then refs/heads/<upstream>,
    // then refs/remotes/<upstream>. We use try_find_reference (returns
    // Option) to avoid anyhow backtraces on the miss path.
    let upstream_str = upstream.to_str_lossy();
    let upstream_full: gix::refs::FullName = {
        let refs_heads = format!("refs/heads/{upstream_str}");
        let refs_remotes = format!("refs/remotes/{upstream_str}");
        if let Some(r) = repo.try_find_reference(upstream_str.as_ref())? {
            r.name().to_owned()
        } else if let Some(r) = repo.try_find_reference(refs_heads.as_str())? {
            r.name().to_owned()
        } else if let Some(r) = repo.try_find_reference(refs_remotes.as_str())? {
            r.name().to_owned()
        } else {
            writeln!(
                err,
                "error: the requested upstream branch '{upstream_str}' does not exist"
            )?;
            err.flush().ok();
            std::process::exit(1);
        }
    };

    repo.set_branch_upstream(target.as_ref(), upstream_full.as_ref())?;

    // git's exact success message (local-branch upstream case):
    //   branch 'dev' set up to track 'main'.
    // Captured from vendor/git `install_branch_config_multiple_remotes`.
    let target_short = target.to_str_lossy();
    let upstream_short = upstream_full.shorten().to_str_lossy();
    writeln!(out, "branch '{target_short}' set up to track '{upstream_short}'.")?;
    out.flush().ok();
    Ok(())
}

/// `git branch --unset-upstream [<branch>]`. `<branch>` defaults to the
/// current branch when omitted.
///
/// Success: silent, exit 0 — matches git's behavior.
/// No-upstream error: emits git's exact wording then exits 128:
///   `fatal: branch '<name>' has no upstream information`
/// Wording verified against vendor/git and captured from a live run.
pub fn unset_upstream(
    mut repo: gix::Repository,
    branch_short: Option<&gix::bstr::BStr>,
    err: &mut dyn std::io::Write,
) -> anyhow::Result<()> {
    use gix::bstr::ByteSlice as _;

    let target: gix::bstr::BString = match branch_short {
        Some(b) => b.to_owned(),
        None => match repo.head_name()? {
            Some(name) => name.shorten().to_owned(),
            None => {
                writeln!(err, "fatal: HEAD not pointing to a branch")?;
                err.flush().ok();
                std::process::exit(128);
            }
        },
    };

    match repo.unset_branch_upstream(target.as_ref()) {
        Ok(()) => Ok(()),
        Err(gix::config::branch_write::UnsetUpstream::NoUpstream(name)) => {
            // git's exact wording (lowercase 'b'), verified against vendor/git:
            //   fatal: branch '<name>' has no upstream information
            writeln!(
                err,
                "fatal: branch '{}' has no upstream information",
                name.to_str_lossy()
            )?;
            err.flush().ok();
            std::process::exit(128);
        }
        Err(e) => Err(e.into()),
    }
}

/// `git branch --edit-description [<branch>]`. Spawns the user's editor on
/// a temp file seeded with the existing `branch.<n>.description` value (if
/// any), reads the post-edit bytes back, trims trailing whitespace, and
/// writes the result into the repo config. An empty result clears the key,
/// matching git's behaviour: `branch_edit_description()` in
/// `vendor/git/builtin/branch.c` calls `launch_editor()` then
/// `git_config_set_gently("branch.<name>.description", strbuf_length(&buf) ? buf.buf : NULL)`.
///
/// Success: silent, exit 0 — matches git's behaviour when EDITOR exits 0.
/// Detached HEAD (no positional arg): emits the same fatal message git prints
/// then exits 128.
pub fn edit_description(
    mut repo: gix::Repository,
    branch_short: Option<&gix::bstr::BStr>,
    err: &mut dyn std::io::Write,
) -> anyhow::Result<()> {
    use gix::bstr::ByteSlice as _;

    let target: gix::bstr::BString = match branch_short {
        Some(b) => b.to_owned(),
        None => match repo.head_name()? {
            Some(name) => name.shorten().to_owned(),
            None => {
                writeln!(err, "fatal: HEAD not pointing to a branch")?;
                err.flush().ok();
                std::process::exit(128);
            }
        },
    };

    // Read existing description (if any) to seed the editor temp file.
    // We access the underlying gix-config File directly via `.plumbing()`
    // so we can use `string_by()` with a runtime subsection (branch name).
    let initial: Vec<u8> = repo
        .config_snapshot()
        .plumbing()
        .string_by("branch", Some(target.as_ref()), "description")
        .map(|s| s.into_owned().into())
        .unwrap_or_default();

    let edited = crate::shared::editor::edit_file(&repo, &initial, "BRANCH_DESCRIPTION")?;

    // git strips trailing whitespace from both ends and treats all-whitespace
    // (including a bare newline from EDITOR=true with empty initial) as empty
    // — meaning "clear the key". Mirrors the strbuf_trim / empty-string check
    // in vendor/git/builtin/branch.c branch_edit_description().
    let trimmed: &[u8] = {
        let mut end = edited.len();
        while end > 0
            && matches!(edited[end - 1], b'\n' | b'\r' | b' ' | b'\t')
        {
            end -= 1;
        }
        &edited[..end]
    };

    repo.set_branch_description(target.as_ref(), trimmed.as_bstr())?;
    Ok(())
}

/// `git branch --show-current`: print the current branch name alone,
/// or nothing in detached `HEAD` state. Matches builtin/branch.c
/// show_current behavior — no asterisk, no indent, no newline on
/// detached. Mirrors repo.head_name(): `None` when detached / unborn
/// → no output; `Some` → shortened name followed by `\n`.
pub fn show_current(repo: gix::Repository, out: &mut dyn std::io::Write) -> anyhow::Result<()> {
    if let Some(name) = repo.head_name()? {
        writeln!(out, "{}", name.shorten())?;
    }
    Ok(())
}

pub mod list {
    use std::ffi::OsString;

    use gix::bstr::BString;

    pub enum Kind {
        Local,
        Remote,
        All,
    }

    pub struct Options {
        pub kind: Kind,
        /// Shell-glob patterns (fnmatch(3), OR'd). Empty = match
        /// everything. Matches `git branch [<pattern>...]`.
        pub patterns: Vec<BString>,
        /// If set, only list branches whose tip has `<commit>` in its
        /// ancestry (inclusive). Git's `--contains`.
        pub contains: Option<OsString>,
        /// If set, only list branches whose tip does NOT have `<commit>`
        /// in its ancestry. Git's `--no-contains`.
        pub no_contains: Option<OsString>,
        /// If set, only list branches whose tip is reachable from
        /// `<commit>` (i.e., merged into it). Git's `--merged`.
        pub merged: Option<OsString>,
        /// If set, only list branches whose tip is NOT reachable from
        /// `<commit>`. Git's `--no-merged`.
        pub no_merged: Option<OsString>,
        /// If set, only list branches whose ref directly points at
        /// `<object>` (no reachability walk; byte-exact oid match).
        /// Git's `--points-at`.
        pub points_at: Option<OsString>,
        /// Format string interpolating `%(<atom>)` fields (for-each-ref
        /// atom set). When set, replaces the default asterisk/indent
        /// listing with one format-expanded row per branch (no
        /// current-branch decoration). Git's `--format`.
        pub format_string: Option<String>,
        /// Sort keys (for-each-ref syntax). Later keys take precedence
        /// as primary; a leading `-` reverses direction. Git's
        /// `--sort=<key>` (repeatable).
        pub sort: Vec<String>,
        /// Do not emit a newline for rows where the `--format`
        /// expansion is empty. Git's `--omit-empty`.
        pub omit_empty: bool,
        /// Case-insensitive sort and pattern match. Git's
        /// `-i/--ignore-case`.
        pub ignore_case: bool,
        /// `-v` count: 0 = bare, 1 = `-v` (sha + subject), 2 = `-vv` (+ upstream tracking).
        pub verbose: u8,
        /// `--abbrev=<n>`. None = use `core.abbrev` / 7 default.
        pub abbrev: Option<usize>,
        /// `--no-abbrev` — render full hash regardless of `--abbrev`.
        pub no_abbrev: bool,
        /// `--column[=<opts>]`. None = honor `column.branch` config / off.
        pub column: Option<String>,
        /// `--no-column` — explicit off, beats config.
        pub no_column: bool,
    }
}

/// A single row collected during the branch listing pass, carrying
/// everything needed by the emit loop so that maxwidth can be computed
/// across both local and remote sections before any output is written.
/// Mirrors git's `ref_array_item` scan in
/// `vendor/git/builtin/branch.c:calc_maxwidth` which walks the entire
/// `ref_array` rather than computing per-section widths.
struct Row {
    /// Display name as written by git: short for locals,
    /// `remotes/<short>` if `--all` pairs locals with remotes (so the
    /// two namespaces are disambiguated), otherwise short for remotes
    /// when listed alone.
    display_name: String,
    /// Peeled tip ObjectId. `None` only on a peel failure — mirrors
    /// git's behaviour of treating a missing peel as an empty subject
    /// and an empty abbrev-SHA.
    tip: Option<gix::ObjectId>,
    /// `true` iff this row represents the current `HEAD` branch.
    /// Only ever set for local rows.
    is_head: bool,
    /// Full refname used for `--format` expansion:
    /// `refs/heads/<short>` or `refs/remotes/<short>`.
    full_name: String,
}

/// Extract the commit subject (first line of message, condensed).
/// Returns `""` for non-commit tips or any read/decode error, which
/// matches git's `%(contents:subject)` default for non-commit refs.
fn commit_subject(repo: &gix::Repository, tip: gix::ObjectId) -> String {
    let Ok(object) = repo.find_object(tip) else {
        return String::new();
    };
    let Ok(commit) = object.try_into_commit() else {
        return String::new();
    };
    match commit.message() {
        Ok(msg) => msg.summary().to_string(),
        Err(_) => String::new(),
    }
}

/// Minimal subset of git's `for-each-ref` atom interpreter, enough
/// for the atoms exercised by branch-list rows. Unknown atoms expand
/// to the empty string — git would die via `verify_ref_format`, but
/// bytes-mode rows only request supported atoms.
fn expand_format(fmt: &str, full: &str, short: &str) -> String {
    let mut out = String::with_capacity(fmt.len() + 32);
    let bytes = fmt.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 1 < bytes.len() && bytes[i + 1] == b'(' {
            let Some(close_rel) = bytes[i + 2..].iter().position(|&b| b == b')') else {
                out.push('%');
                i += 1;
                continue;
            };
            let close = i + 2 + close_rel;
            let atom = &fmt[i + 2..close];
            i = close + 1;
            match atom {
                "refname" => out.push_str(full),
                "refname:short" | "refname:strip=2" | "refname:lstrip=2" => out.push_str(short),
                a if a.starts_with("refname:strip=") || a.starts_with("refname:lstrip=") => {
                    let n_str = a.split('=').nth(1).unwrap_or("0");
                    let n: usize = n_str.parse().unwrap_or(0);
                    let stripped = full.split('/').skip(n).collect::<Vec<_>>().join("/");
                    out.push_str(&stripped);
                }
                _ => {
                    // Unknown atom → empty. Same deliberate no-match
                    // as tag's interpreter.
                }
            }
        } else if bytes[i] == b'%' && i + 2 < bytes.len() {
            // Hex-pair escape like %00 (NUL), %0a (LF).
            let hex = &fmt[i + 1..i + 3];
            if let Ok(n) = u8::from_str_radix(hex, 16) {
                out.push(n as char);
                i += 3;
                continue;
            }
            out.push('%');
            i += 1;
        } else {
            out.push(bytes[i] as char);
            i += 1;
        }
    }
    out
}

/// Resolve the `-vv` upstream annotation for a local branch.
/// Returns `Some("[origin/main]")` (no divergence),
/// `Some("[origin/main: ahead 1]")`, `Some("[origin/main: behind 2]")`,
/// or `Some("[origin/main: ahead 1, behind 2]")`.
/// Returns `None` if the ref is a remote branch (no upstream column) or has
/// no upstream configured.
fn tracking_annotation(repo: &gix::Repository, full_ref_name: &str, tip: gix::ObjectId) -> Option<String> {
    use gix::refs::{Category, FullName};
    let full: FullName = FullName::try_from(full_ref_name).ok()?;
    let category = full.as_ref().category()?;
    if category != Category::LocalBranch {
        return None;
    }
    // Resolve the remote-tracking ref name (e.g. refs/remotes/origin/main).
    let tracking_full = repo
        .branch_remote_tracking_ref_name(full.as_ref(), gix::remote::Direction::Fetch)?
        .ok()?;
    let tracking_short = tracking_full.shorten().to_string();
    // Try to resolve the tracking ref's tip OID. If the tracking ref doesn't
    // exist locally (never been fetched), git prints `[<short>: gone]` in -vv.
    // For now we return None if the ref isn't present — close parity when
    // the gone annotation is tackled separately.
    let tracking_oid = match repo.find_reference(tracking_full.as_ref()) {
        Ok(mut r) => r.peel_to_id().ok().map(gix::Id::detach),
        Err(_) => None,
    };
    let tracking_oid = tracking_oid?;
    let (ahead, behind) = ahead_behind(repo, tip, tracking_oid).unwrap_or((0, 0));
    Some(match (ahead, behind) {
        (0, 0) => format!("[{tracking_short}]"),
        (a, 0) => format!("[{tracking_short}: ahead {a}]"),
        (0, b) => format!("[{tracking_short}: behind {b}]"),
        (a, b) => format!("[{tracking_short}: ahead {a}, behind {b}]"),
    })
}

/// Compute `(ahead, behind)` for `tip` relative to `upstream` via symmetric
/// ancestor-set difference. `ahead` = commits reachable from `tip` but not
/// `upstream`; `behind` = commits reachable from `upstream` but not `tip`.
///
/// The ancestor sets are computed with inclusive walks (each start is in its
/// own set). This matches git's ahead/behind semantics.
///
/// For the parity fixture sizes this is fine. Larger repos would benefit from
/// a merge-base-bounded walk; that's a future optimization.
fn ahead_behind(repo: &gix::Repository, tip: gix::ObjectId, upstream: gix::ObjectId) -> Option<(usize, usize)> {
    let to_set = |start: gix::ObjectId| -> Option<HashSet<gix::ObjectId>> {
        let mut set = HashSet::new();
        set.insert(start);
        let walk = repo.rev_walk([start]).all().ok()?;
        for info in walk {
            let info = info.ok()?;
            set.insert(info.id);
        }
        Some(set)
    };
    let tip_set = to_set(tip)?;
    let up_set = to_set(upstream)?;
    let ahead = tip_set.difference(&up_set).count();
    let behind = up_set.difference(&tip_set).count();
    Some((ahead, behind))
}

pub fn list(
    repo: gix::Repository,
    out: &mut dyn std::io::Write,
    format: OutputFormat,
    options: list::Options,
) -> anyhow::Result<()> {
    if format != OutputFormat::Human {
        anyhow::bail!("JSON output isn't supported");
    }

    let platform = repo.references()?;

    let (show_local, show_remotes) = match options.kind {
        list::Kind::Local => (true, false),
        list::Kind::Remote => (false, true),
        list::Kind::All => (true, true),
    };

    // Current-branch marker. None when HEAD is detached or unborn; in
    // both cases no local row is decorated with `*`. Matches git's
    // print_ref_list() which compares refs against head_ref.
    let head_short = repo.head_name()?.map(|name| name.shorten().to_string());

    // Resolve the primary sort key. git's OPT_REF_SORT accumulates
    // keys; the last one wins as primary. A leading `-` reverses.
    // For this iteration only `refname` is recognized — other keys
    // (committerdate, authordate, etc.) fall back to refname order
    // since the list is already refname-sorted to begin with.
    let sort_descending = options.sort.last().is_some_and(|k| k.starts_with('-'));

    // Pattern filter: fnmatch(3)-style shell globs OR'd; empty =
    // match everything. Matches `for_each_fullref_in_pattern` in
    // vendor/git/refs.c called from builtin/branch.c's filter_refs.
    // --ignore-case flips on wildmatch IGNORE_CASE and also drives the
    // sort comparator below.
    let patterns = &options.patterns;
    let wildmatch_mode = if options.ignore_case {
        gix::glob::wildmatch::Mode::IGNORE_CASE
    } else {
        gix::glob::wildmatch::Mode::empty()
    };
    let match_name = |name: &str| -> bool {
        patterns.is_empty()
            || patterns
                .iter()
                .any(|pat| gix::glob::wildmatch(pat.as_ref(), name.into(), wildmatch_mode))
    };

    // --contains / --no-contains filters: resolve each needle once
    // up-front so every branch's ancestor walk compares against the
    // same ObjectId.
    let resolve = |rev: &std::ffi::OsStr| -> anyhow::Result<gix::ObjectId> {
        let rev_bstr = gix::path::os_str_into_bstr(rev)?;
        Ok(repo.rev_parse_single(rev_bstr)?.detach())
    };
    let contains_oid: Option<gix::ObjectId> = options.contains.as_deref().map(resolve).transpose()?;
    let no_contains_oid: Option<gix::ObjectId> = options.no_contains.as_deref().map(resolve).transpose()?;
    let points_at_oid: Option<gix::ObjectId> = options.points_at.as_deref().map(resolve).transpose()?;
    let merged_set: Option<HashSet<gix::ObjectId>> =
        options.merged.as_deref().map(|r| ancestors_of(&repo, r)).transpose()?;
    let no_merged_set: Option<HashSet<gix::ObjectId>> = options
        .no_merged
        .as_deref()
        .map(|r| ancestors_of(&repo, r))
        .transpose()?;

    let tip_of = |reference: &gix::Reference<'_>| -> anyhow::Result<gix::ObjectId> {
        Ok(reference.clone().into_fully_peeled_id()?.detach())
    };

    let ancestry_keep = |tip: gix::ObjectId| -> anyhow::Result<bool> {
        if let Some(needle) = contains_oid {
            if !commit_contains(&repo, tip, needle)? {
                return Ok(false);
            }
        }
        if let Some(needle) = no_contains_oid {
            if commit_contains(&repo, tip, needle)? {
                return Ok(false);
            }
        }
        if let Some(set) = merged_set.as_ref() {
            if !set.contains(&tip) {
                return Ok(false);
            }
        }
        if let Some(set) = no_merged_set.as_ref() {
            if set.contains(&tip) {
                return Ok(false);
            }
        }
        if let Some(needle) = points_at_oid {
            if tip != needle {
                return Ok(false);
            }
        }
        Ok(true)
    };

    let needs_ancestry_walk = contains_oid.is_some()
        || no_contains_oid.is_some()
        || merged_set.is_some()
        || no_merged_set.is_some()
        || points_at_oid.is_some();

    // --- collect phase ---------------------------------------------------
    // Gather all rows first so maxwidth can span both local and remote
    // sections. git's calc_maxwidth (vendor/git/builtin/branch.c:466)
    // scans the entire ref_array, not per-section, before any output.

    let mut rows: Vec<Row> = Vec::new();

    if show_local {
        for reference in platform.local_branches()?.flatten() {
            let name = reference.name().shorten().to_string();
            if !match_name(&name) {
                continue;
            }
            let tip = if needs_ancestry_walk {
                let t = tip_of(&reference)?;
                if !ancestry_keep(t)? {
                    continue;
                }
                Some(t)
            } else if options.verbose >= 1 {
                // Even without ancestry filters, -v needs the tip for
                // the SHA + subject columns. Skip the peel for the bare
                // listing to keep that path O(refs) without any object
                // reads.
                tip_of(&reference).ok()
            } else {
                None
            };
            let is_head = head_short.as_ref() == Some(&name);
            rows.push(Row {
                display_name: name.clone(),
                tip,
                is_head,
                full_name: format!("refs/heads/{name}"),
            });
        }
    }

    if show_remotes {
        // When --all pairs locals + remotes, git disambiguates the
        // remote rows with a `remotes/` prefix (see
        // vendor/git/builtin/branch.c: REF_REMOTE_BRANCH vs
        // REF_LOCAL_BRANCH filter.kind → ref_array_item's refname is
        // used verbatim after `refs/` strip, so `refs/remotes/x` =>
        // `remotes/x`). With --remotes alone git instead prints the
        // shortened form `origin/main` because there is no ambiguity
        // against locals.
        let include_prefix = show_local;
        for reference in platform.remote_branches()?.flatten() {
            let short = reference.name().shorten().to_string();
            if !match_name(&short) {
                continue;
            }
            let tip = if needs_ancestry_walk {
                let t = tip_of(&reference)?;
                if !ancestry_keep(t)? {
                    continue;
                }
                Some(t)
            } else if options.verbose >= 1 {
                tip_of(&reference).ok()
            } else {
                None
            };
            let display_name = if include_prefix {
                format!("remotes/{short}")
            } else {
                short.clone()
            };
            rows.push(Row {
                display_name,
                tip,
                is_head: false,
                full_name: format!("refs/remotes/{short}"),
            });
        }
    }

    // Sort by display_name, respecting --ignore-case and --sort direction.
    if options.ignore_case {
        rows.sort_by_key(|r| r.display_name.to_lowercase());
    } else {
        rows.sort_by(|a, b| a.display_name.cmp(&b.display_name));
    }
    if sort_descending {
        rows.reverse();
    }

    // --- emit phase ------------------------------------------------------

    // --column=always: only honors bare-list mode (no -v, no --format).
    let column_always = options
        .column
        .as_deref()
        .is_some_and(|s| s == "always" || s.starts_with("always,") || s.starts_with("always="));
    let column_off = options.no_column || options.column.as_deref().is_some_and(|s| s == "never");

    let use_columns = column_always && !column_off && options.verbose == 0 && options.format_string.is_none();

    if use_columns {
        // git pads each row with the marker prefix ("* " or "  ") just like the
        // bare list does — column packing operates on the marker-prefixed strings.
        let lines: Vec<String> = rows
            .iter()
            .map(|row| {
                let marker = if row.is_head { "* " } else { "  " };
                format!("{marker}{}", row.display_name)
            })
            .collect();
        let width = std::env::var("COLUMNS")
            .ok()
            .and_then(|s| s.parse::<usize>().ok())
            .unwrap_or(80);
        crate::shared::columns::write_columns(out, &lines, width)?;
        return Ok(());
    }

    // Compute maxwidth across all rows so the name column aligns
    // uniformly, matching git's calc_maxwidth pass.
    let maxwidth = if options.verbose >= 1 {
        rows.iter().map(|r| r.display_name.chars().count()).max().unwrap_or(0)
    } else {
        0
    };

    // Resolve the abbreviated-SHA length once. git's branch -v uses
    // core.abbrev (default 7) unless --abbrev=<n> or --no-abbrev are
    // given. The full SHA-1 hex length is 40; SHA-256 is 64.
    let hash_kind = repo.object_hash();
    let abbrev_len = if options.no_abbrev {
        hash_kind.len_in_hex()
    } else {
        options.abbrev.unwrap_or(7).min(hash_kind.len_in_hex())
    };

    for row in &rows {
        if let Some(fmt) = options.format_string.as_deref() {
            // %(refname:short) must yield the bare name without the
            // `remotes/` prefix we added for disambiguation in --all
            // mode. Strip it back here so callers see e.g. `origin/main`
            // rather than `remotes/origin/main`.
            let short = row.display_name.strip_prefix("remotes/").unwrap_or(&row.display_name);
            let line = expand_format(fmt, &row.full_name, short);
            if !(options.omit_empty && line.is_empty()) {
                writeln!(out, "{line}")?;
            }
            continue;
        }

        let marker = if row.is_head { "* " } else { "  " };

        if options.verbose == 0 {
            writeln!(out, "{marker}{name}", name = row.display_name)?;
        } else {
            // `-v`: column-aligned name, then abbreviated SHA, then
            // subject. `-vv`: additionally insert `[<upstream>: ahead N,
            // behind N]` between SHA and subject for local branches with a
            // configured upstream (git builtin/branch.c: print_ref_list,
            // append_info_to_buf).
            //
            // git's format (builtin/branch.c print_ref_list):
            //   <marker><name padded to maxwidth> <abbrev-sha> <subject>
            // No trailing space after the subject.
            let short_sha = row
                .tip
                .map(|oid| format!("{}", oid.to_hex_with_len(abbrev_len)))
                .unwrap_or_default();
            let tracking = if options.verbose >= 2 {
                row.tip.and_then(|oid| tracking_annotation(&repo, &row.full_name, oid))
            } else {
                None
            };
            let subject = row.tip.map(|oid| commit_subject(&repo, oid)).unwrap_or_default();
            match tracking {
                Some(ref t) => writeln!(
                    out,
                    "{marker}{name:<width$} {short_sha} {t} {subject}",
                    name = row.display_name,
                    width = maxwidth,
                )?,
                None => writeln!(
                    out,
                    "{marker}{name:<width$} {short_sha} {subject}",
                    name = row.display_name,
                    width = maxwidth,
                )?,
            }
        }
    }

    Ok(())
}
