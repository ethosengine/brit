use gix::{
    ObjectId, Result,
    error::{ResultExt, bail, message},
    objs::Kind,
    prelude::ObjectIdExt,
};

pub fn function(mut repo: gix::Repository, spec: Option<String>, mut out: impl std::io::Write) -> Result<()> {
    let spec = spec.unwrap_or("HEAD".into());

    repo.object_cache_size_if_unset(4 * 1024 * 1024);
    // We expect to be finding a bunch of non-existent objects here - never refresh the ODB
    repo.objects.refresh_never();

    let id = repo
        .rev_parse_single(spec.as_str())
        .or_raise(|| message("Only single revisions are supported"))?;
    let source = gix::object::bounded::Source::deferred(&repo).stored_only();
    let commits: gix::revision::Walk<'_> = source
        .peel(id.detach(), gix::object::Kind::Commit)
        .or_raise(|| message("Need committish as starting point"))?
        .attach(&repo)
        .ancestors()
        .all()?;

    let output_error = std::cell::RefCell::new(None);
    let on_missing = |oid: &ObjectId, kind: Kind| {
        let mut error = output_error.borrow_mut();
        if error.is_none() {
            *error = writeln!(out, "{oid}: {kind}").err();
        }
    };

    let mut check = gix_fsck::Connectivity::new(source, on_missing);
    // Walk all commits, checking each one for connectivity
    for commit in commits {
        if gix::interrupt::is_triggered() {
            bail!(gix::error::cancelled("interrupted by user"));
        }
        let commit = commit?;
        check.check_commit(&commit.id)?;
        if let Some(error) = output_error.borrow_mut().take() {
            return Err(error).or_error();
        }
        // Note that we leave parent-iteration to the commits iterator, as it will
        // correctly handle shallow repositories which are expected to have the commits
        // along the shallow boundary missing.
    }
    Ok(())
}
