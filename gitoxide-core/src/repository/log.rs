use anyhow::bail;
use gix::bstr::{BString, ByteSlice};

/// CLI-level options for `gix log`. Mirrors the user-visible flag surface
/// from `src/plumbing/options/mod.rs::log::Platform`.
#[derive(Debug, Default)]
pub struct Options {
    pub all: bool,
    pub branches: bool,
    pub tags: bool,
    pub remotes: bool,
    pub revspec: Option<BString>,
    pub path: Option<BString>,
}

pub fn log(mut repo: gix::Repository, out: &mut dyn std::io::Write, opts: Options) -> anyhow::Result<()> {
    repo.object_cache_size_if_unset(repo.compute_object_cache_size_for_tree_diffs(&**repo.index_or_empty()?));

    if let Some(path) = opts.path {
        log_file(repo, out, path)
    } else {
        log_all(repo, out, opts)
    }
}

fn log_all(repo: gix::Repository, out: &mut dyn std::io::Write, opts: Options) -> Result<(), anyhow::Error> {
    let mut tips: Vec<gix::ObjectId> = Vec::new();
    let mut ends: Vec<gix::ObjectId> = Vec::new();

    // --all / --branches / --tags / --remotes pseudo-ref selectors. Each
    // iterates a ref category and appends peeled commit ids as tips.
    // `git log --all` is equivalent to `--branches --tags --remotes` plus
    // HEAD; enabling all four category booleans matches that shape.
    let (want_branches, want_tags, want_remotes) = if opts.all {
        (true, true, true)
    } else {
        (opts.branches, opts.tags, opts.remotes)
    };
    // flatten() silently drops per-ref read errors — matches branch.rs's
    // list listing pattern and git's own behavior of skipping unreadable refs.
    // peel_to_id may still fail per-ref (e.g. corrupt packed-refs entry) and
    // is handled via `if let Ok(...)`.
    if want_branches {
        for mut r in repo.references()?.local_branches()?.flatten() {
            if let Ok(id) = r.peel_to_id() {
                tips.push(id.detach());
            }
        }
    }
    if want_tags {
        for mut r in repo.references()?.tags()?.flatten() {
            if let Ok(id) = r.peel_to_id() {
                tips.push(id.detach());
            }
        }
    }
    if want_remotes {
        for mut r in repo.references()?.remote_branches()?.flatten() {
            if let Ok(id) = r.peel_to_id() {
                tips.push(id.detach());
            }
        }
    }

    // Explicit revspec — additive with the pseudo-ref selectors above.
    match opts.revspec {
        Some(spec) => match repo.rev_parse(spec.as_bstr()) {
            Ok(parsed) => match parsed.detach() {
                gix::revision::plumbing::Spec::Include(id) | gix::revision::plumbing::Spec::ExcludeParents(id) => {
                    tips.push(id);
                }
                gix::revision::plumbing::Spec::Exclude(_) => {
                    // A bare `^rev` with no included side: git's setup_revisions
                    // emits "fatal: empty revision range" when no pseudo-ref was
                    // supplied either. For now accept the empty walk when the
                    // pseudo-refs supplied zero tips; wording parity is a later
                    // row.
                }
                gix::revision::plumbing::Spec::Range { from, to } => {
                    tips.push(to);
                    ends.push(from);
                }
                gix::revision::plumbing::Spec::Merge { theirs, ours } => {
                    let base = repo
                        .merge_base(theirs, ours)
                        .map(gix::Id::detach)
                        .map_err(|e| anyhow::anyhow!("failed to resolve merge-base for '{spec}': {e}"))?;
                    tips.push(theirs);
                    tips.push(ours);
                    ends.push(base);
                }
                gix::revision::plumbing::Spec::IncludeOnlyParents(id) => {
                    let commit = repo.find_commit(id)?;
                    tips.extend(commit.parent_ids().map(gix::Id::detach));
                }
            },
            Err(_) => {
                // Parity with git's setup_revisions: unknown revision/path dies 128
                // (vendor/git/revision.c::handle_revision_arg → die). Wording is
                // git's exact 3-line stanza.
                eprintln!(
                    "fatal: ambiguous argument '{spec}': unknown revision or path not in the working tree.\n\
                     Use '--' to separate paths from revisions, like this:\n\
                     'git <command> [<revision>...] -- [<file>...]'"
                );
                std::process::exit(128);
            }
        },
        None => {
            // No explicit revspec. If the user didn't pass any pseudo-ref
            // selector either, default to HEAD. If they did (e.g. `--tags`
            // in a repo with no tags), leave tips empty — git prints
            // nothing and exits 0 in that case.
            let pseudo_ref_requested = opts.all || opts.branches || opts.tags || opts.remotes;
            if !pseudo_ref_requested {
                let mut head = repo.head()?;
                // Parity with git: unborn HEAD (fresh `git init`, no commits)
                // dies 128 with "fatal: your current branch '<short>' does not
                // have any commits yet" (vendor/git/builtin/log.c →
                // cmd_log_walk → revision.c::die).
                if let gix::head::Kind::Unborn(name) = &head.kind {
                    eprintln!(
                        "fatal: your current branch '{}' does not have any commits yet",
                        name.shorten()
                    );
                    std::process::exit(128);
                }
                tips.push(head.peel_to_commit()?.id);
            }
        }
    }

    let topo = gix::traverse::commit::topo::Builder::from_iters(&repo.objects, tips, Some(ends)).build()?;

    for info in topo {
        let info = info?;

        write_info(&repo, &mut *out, &info)?;
    }

    Ok(())
}

fn log_file(_repo: gix::Repository, _out: &mut dyn std::io::Write, _path: BString) -> anyhow::Result<()> {
    bail!("File-based lookup isn't yet implemented in a way that is competitively fast");
}

fn write_info(
    repo: &gix::Repository,
    mut out: impl std::io::Write,
    info: &gix::traverse::commit::Info,
) -> Result<(), std::io::Error> {
    let commit = repo.find_commit(info.id).unwrap();

    let message = commit.message_raw_sloppy();
    let title = message.lines().next();

    writeln!(
        out,
        "{} {}",
        info.id.to_hex_with_len(8),
        title.map_or_else(|| "<no message>".into(), BString::from)
    )?;

    Ok(())
}
