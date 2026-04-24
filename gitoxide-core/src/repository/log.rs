use anyhow::bail;
use gix::bstr::{BString, ByteSlice};

pub fn log(
    mut repo: gix::Repository,
    out: &mut dyn std::io::Write,
    revspec: Option<BString>,
    path: Option<BString>,
) -> anyhow::Result<()> {
    repo.object_cache_size_if_unset(repo.compute_object_cache_size_for_tree_diffs(&**repo.index_or_empty()?));

    if let Some(path) = path {
        log_file(repo, out, path)
    } else {
        log_all(repo, out, revspec)
    }
}

fn log_all(repo: gix::Repository, out: &mut dyn std::io::Write, revspec: Option<BString>) -> Result<(), anyhow::Error> {
    let (tips, ends): (Vec<gix::ObjectId>, Vec<gix::ObjectId>) = match revspec {
        Some(spec) => match repo.rev_parse(spec.as_bstr()) {
            Ok(parsed) => match parsed.detach() {
                gix::revision::plumbing::Spec::Include(id) | gix::revision::plumbing::Spec::ExcludeParents(id) => {
                    (vec![id], Vec::new())
                }
                gix::revision::plumbing::Spec::Exclude(_) => {
                    // A bare `^rev` with no included side: git's setup_revisions
                    // emits "fatal: empty revision range" (vendor/git/revision.c
                    // ::prepare_revision_walk). For now return an empty walk;
                    // a later TODO row closes the exact-wording parity.
                    (Vec::new(), Vec::new())
                }
                gix::revision::plumbing::Spec::Range { from, to } => (vec![to], vec![from]),
                gix::revision::plumbing::Spec::Merge { theirs, ours } => {
                    // Symmetric difference (`theirs...ours`): include commits
                    // reachable from either side but not from their merge-base.
                    let base = repo
                        .merge_base(theirs, ours)
                        .map(gix::Id::detach)
                        .map_err(|e| anyhow::anyhow!("failed to resolve merge-base for '{spec}': {e}"))?;
                    (vec![theirs, ours], vec![base])
                }
                gix::revision::plumbing::Spec::IncludeOnlyParents(id) => {
                    // `id^@`: start from the parents of id, not id itself.
                    let commit = repo.find_commit(id)?;
                    let parents: Vec<_> = commit.parent_ids().map(gix::Id::detach).collect();
                    (parents, Vec::new())
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
            let mut head = repo.head()?;
            // Parity with git: unborn HEAD (fresh `git init`, no commits) dies 128
            // with "fatal: your current branch '<short>' does not have any commits yet"
            // (vendor/git/builtin/log.c → cmd_log_walk → revision.c::die).
            if let gix::head::Kind::Unborn(name) = &head.kind {
                eprintln!(
                    "fatal: your current branch '{}' does not have any commits yet",
                    name.shorten()
                );
                std::process::exit(128);
            }
            (vec![head.peel_to_commit()?.id], Vec::new())
        }
    };
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
