use crate::OutputFormat;
use gix::{
    Result,
    error::{ResultExt, bail},
};

pub fn merge_base(
    mut repo: gix::Repository,
    first: String,
    others: Vec<String>,
    mut out: impl std::io::Write,
    format: OutputFormat,
) -> Result<()> {
    if format != OutputFormat::Human {
        bail!(gix::error::unsupported("Only 'human' format is currently supported"));
    }
    repo.object_cache_size_if_unset(50 * 1024 * 1024);
    let first_id = commit_id(&repo, first.as_str())?;
    let other_ids: Vec<_> = others
        .iter()
        .map(|other| commit_id(&repo, other.as_str()))
        .collect::<std::result::Result<_, _>>()?;

    let cache = repo.commit_graph_if_enabled()?;
    let mut graph = repo.revision_graph(cache.as_ref());
    let bases = repo.merge_bases_many_with_graph(first_id, &other_ids, &mut graph)?;
    if bases.is_empty() {
        bail!(
            "No base found for {first} and {others}".not_found(),
            others = others.join(", ")
        )
    }
    for id in bases {
        writeln!(&mut out, "{id}").or_error()?;
    }
    Ok(())
}

/// `merge-base --is-ancestor <first> <second>`, answered through the exit code like Git: `Ok` when `first`
/// is an ancestor of `second` or the same commit, status 1 when it is not, 128 for a revision that names
/// no commit or for more than two revisions, and the usage status 129 for one. With no revision at all,
/// clap's required `first` answers with its usage status 2 where Git exits 129 (a known difference).
pub fn is_ancestor(
    mut repo: gix::Repository,
    first: &str,
    others: &[String],
    mut err: impl std::io::Write,
) -> Result<()> {
    use crate::repository::git_spellings::exit;
    let second = match others {
        [second] => second,
        [] => {
            writeln!(err, "usage: brit merge-base --is-ancestor <commit> <commit>").ok();
            return Err(exit(129));
        }
        _ => {
            writeln!(err, "fatal: --is-ancestor takes exactly two commits").ok();
            return Err(exit(128));
        }
    };
    repo.object_cache_size_if_unset(50 * 1024 * 1024);
    let mut commit = |name: &str| -> Result<gix::ObjectId> {
        let Ok(id) = repo.rev_parse_single(name).map(gix::Id::detach) else {
            writeln!(err, "fatal: Not a valid object name {name}").ok();
            return Err(exit(128));
        };
        match gix::object::bounded::Source::deferred(&repo).peel(id, gix::object::Kind::Commit) {
            Ok(commit) => Ok(commit),
            Err(_) => {
                let kind = repo.find_header(id).map(|header| header.kind().to_string());
                if let Ok(kind) = kind {
                    writeln!(err, "error: object {id} is a {kind}, not a commit").ok();
                }
                writeln!(err, "fatal: Not a valid commit name {name}").ok();
                Err(exit(128))
            }
        }
    };
    let ancestor = commit(first)?;
    let descendant = commit(second)?;
    if ancestor == descendant {
        return Ok(());
    }
    let cache = repo.commit_graph_if_enabled()?;
    let mut graph = repo.revision_graph(cache.as_ref());
    // The merge base of an ancestor and its descendant is the ancestor itself.
    let bases = repo.merge_bases_many_with_graph(ancestor, &[descendant], &mut graph)?;
    if bases.iter().any(|base| *base == ancestor) {
        Ok(())
    } else {
        Err(exit(1))
    }
}

fn commit_id(repo: &gix::Repository, revspec: &str) -> Result<gix::ObjectId> {
    gix::object::bounded::Source::deferred(repo)
        .peel(repo.rev_parse_single(revspec)?.detach(), gix::object::Kind::Commit)
}
