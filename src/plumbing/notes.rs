use std::io::Write;

use gix::{
    Result,
    bstr::{BStr, BString, ByteVec},
    error::{ResultExt, bail, validation},
    object::{Kind, bounded::Source},
    objs::find::Buffer,
};

pub(super) fn show(
    repo: &gix::Repository,
    reference: Option<BString>,
    object: &BStr,
    mut out: impl Write,
    mut err: impl Write,
) -> Result<()> {
    let reference = reference
        .or_else(|| repo.config_snapshot().string("core.notesRef"))
        .unwrap_or_else(|| "refs/notes/commits".into());
    let mut name = BString::from(if reference.starts_with(b"refs/notes/") {
        ""
    } else if reference.starts_with(b"notes/") {
        "refs/"
    } else {
        "refs/notes/"
    });
    name.push_str(reference);
    let name = gix::refs::FullName::try_from(name).or_error()?;
    let object_id = repo.rev_parse_single(object)?;
    let source = Source::deferred(repo);
    if let Some(mut reference) = repo
        .try_find_reference(name.as_bstr())?
        .filter(|reference| reference.name().as_bstr() == name.as_bstr())
    {
        let tree_id = source.peel(reference.follow_to_object()?.detach(), Kind::Tree)?;
        let mut state = gix::note::plumbing::State::new(tree_id, &source)?;
        if let Some(note_blob_id) = state.get(&object_id, &source)? {
            let mut buffer = Buffer::default();
            let note = source.load(&note_blob_id, &mut buffer)?;
            if note.kind != Kind::Blob {
                bail!(validation(format!("note {note_blob_id} is not a blob")));
            }
            return out.write_all(note.data).or_error();
        }
    }
    writeln!(err, "error: no note found for object {object_id}.").or_error()?;
    Err(gitoxide_core::repository::git_spellings::exit(1))
}
