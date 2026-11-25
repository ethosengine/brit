use gix::{
    Result,
    bstr::{BStr, BString, ByteSlice},
    error::{bail, validation},
    prelude::ObjectIdExt,
};

pub(super) const ONE_REF_FORM: &str =
    "notes transfers require one named ref in the form refs/notes/X:refs/notes/X (fast-forward only)";

pub(super) fn is_notes(name: &BStr) -> bool {
    name.starts_with(b"refs/notes/")
}

/// Recognize explicit notes transfers before parsing generic Git refspecs.
pub(super) fn single(specs: &[BString]) -> Result<Option<gix::refs::FullName>> {
    if !specs.iter().any(|spec| {
        spec.trim_start_with(|b| matches!(b, '+' | '^'))
            .split_str(":")
            .any(|side| is_notes(side.as_bstr()))
    }) {
        return Ok(None);
    }
    if let [spec] = specs
        && let Some((source, destination)) = spec.split_once_str(":")
        && source == destination
        && is_notes(source.as_bstr())
        && let Ok(name) = gix::refs::FullName::try_from(source.as_bstr().to_owned())
    {
        return Ok(Some(name));
    }
    bail!(validation(ONE_REF_FORM))
}

/// Verify ancestry without commit timestamp cutoffs or a cached commit graph.
pub(super) fn is_fast_forward(
    repo: &gix::Repository,
    old_commit_id: gix::ObjectId,
    new_commit_id: gix::ObjectId,
) -> Result<bool> {
    Ok(new_commit_id
        .attach(repo)
        .ancestors()
        .use_commit_graph(false)
        .all()?
        .find(|item| item.as_ref().map_or(true, |commit| commit.id == old_commit_id))
        .transpose()?
        .is_some())
}
