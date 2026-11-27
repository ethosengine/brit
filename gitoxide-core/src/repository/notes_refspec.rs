use gix::{
    Result,
    bstr::{BStr, BString, ByteSlice},
    error::{bail, validation},
    prelude::ObjectIdExt,
};

pub(super) const ONE_REF_FORM: &str = "notes transfers require one named ref in the form refs/notes/X:refs/notes/X, or refs/notes/X:refs/notes/Y for push (fast-forward only)";

pub(super) fn is_notes(name: &BStr) -> bool {
    name.starts_with(b"refs/notes/")
}

/// One explicit notes transfer: a named notes ref and the notes ref it lands as on the other side.
pub(super) struct Transfer {
    pub source: gix::refs::FullName,
    pub destination: gix::refs::FullName,
}

impl Transfer {
    /// The one name of a same-name transfer, or `None` when the two sides differ.
    pub(super) fn same_name(&self) -> Option<&gix::refs::FullName> {
        (self.source == self.destination).then_some(&self.source)
    }
}

/// Recognize explicit notes transfers before parsing generic Git refspecs.
///
/// Both sides must be full names under `refs/notes/`; they may differ. Callers that cannot map
/// a notes ref onto another name refuse a transfer whose [`Transfer::same_name`] is `None`.
pub(super) fn single(specs: &[BString]) -> Result<Option<Transfer>> {
    if !specs.iter().any(|spec| {
        spec.trim_start_with(|b| matches!(b, '+' | '^'))
            .split_str(":")
            .any(|side| is_notes(side.as_bstr()))
    }) {
        return Ok(None);
    }
    if let [spec] = specs
        && let Some((source, destination)) = spec.split_once_str(":")
        && is_notes(source.as_bstr())
        && is_notes(destination.as_bstr())
        && let Ok(source) = gix::refs::FullName::try_from(source.as_bstr().to_owned())
        && let Ok(destination) = gix::refs::FullName::try_from(destination.as_bstr().to_owned())
    {
        return Ok(Some(Transfer { source, destination }));
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
