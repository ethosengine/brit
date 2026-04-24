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
    let start_id = match revspec {
        Some(spec) => match repo.rev_parse_single(spec.as_bstr()) {
            Ok(id) => id.detach(),
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
            head.peel_to_commit()?.id
        }
    };
    let topo = gix::traverse::commit::topo::Builder::from_iters(&repo.objects, [start_id], None::<Vec<gix::ObjectId>>)
        .build()?;

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
