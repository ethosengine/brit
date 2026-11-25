use std::io::Write;

use gix::{
    Result,
    bstr::{BStr, BString, ByteSlice},
    error::{ResultExt, bail, message},
    object::bounded::Source,
    objs::{Kind, find::Buffer},
    worktree::stack::state::attributes,
};

use super::diff::{quote_patch_path, quote_path};

/// How to display a matching blob.
#[derive(Debug, Clone, Copy)]
pub enum Output {
    /// Print matching lines.
    Lines,
    /// Print matching lines with one-based line numbers.
    NumberedLines,
    /// Print each matching name once.
    Names,
}

/// Search regular files in a tree-ish revision. Read attributes for pathspec matching from
/// that tree, and send each candidate line to `matches` without its terminating LF.
pub fn grep(
    repo: &gix::Repository,
    out: &mut dyn Write,
    revision: &BStr,
    paths: &[BString],
    output: Output,
    mut matches: impl FnMut(&[u8]) -> Result<bool>,
) -> Result<bool> {
    use gix::index::entry::Mode;

    let source = Source::deferred(repo);
    let tree_id = source.peel(repo.rev_parse_single(revision)?.detach(), Kind::Tree)?;
    let index = repo.index_from_tree(&tree_id)?;
    let mut pathspec = repo.pathspec(true, paths, false, &index, attributes::Source::IdMapping)?;
    let prefix = repo.prefix()?;
    let quote_high = quote_path(repo)?;
    let mut buffer = Buffer::default();
    let mut found = false;
    for entry in index.entries() {
        if gix::interrupt::is_triggered() {
            bail!(message("grep interrupted"));
        }
        let path = entry.path(&index);
        if !matches!(entry.mode, Mode::FILE | Mode::FILE_EXECUTABLE) || !pathspec.is_included(path, Some(false)) {
            continue;
        }
        let blob = source.load(&entry.id, &mut buffer)?;
        if blob.kind != Kind::Blob {
            bail!(message!("Expected blob {}", entry.id).corrupted());
        }
        let binary = blob.data[..blob.data.len().min(8000)].contains(&0);
        let mut name = None;
        for (number, line) in blob.data.lines_with_terminator().enumerate() {
            let line = line.strip_suffix(b"\n").unwrap_or(line);
            if !matches(line)? {
                continue;
            }
            found = true;
            if name.is_none() {
                name = Some(display_name(revision, path, prefix, quote_high)?);
            }
            let name = name.as_ref().expect("the matching path was formatted");
            if matches!(output, Output::Names) {
                out.write_all(name).or_error()?;
                writeln!(out).or_error()?;
                break;
            }
            if binary {
                out.write_all(b"Binary file ").or_error()?;
                out.write_all(name).or_error()?;
                out.write_all(b" matches\n").or_error()?;
                break;
            }
            out.write_all(name).or_error()?;
            out.write_all(b":").or_error()?;
            if matches!(output, Output::NumberedLines) {
                write!(out, "{}:", number + 1).or_error()?;
            }
            out.write_all(line).or_error()?;
            writeln!(out).or_error()?;
        }
        buffer.clear();
    }
    Ok(found)
}

fn display_name(revision: &BStr, path: &BStr, prefix: Option<&std::path::Path>, quote_high: bool) -> Result<BString> {
    let relative;
    let path = if let Some(prefix) = prefix {
        let path = gix::path::from_byte_slice(path)?;
        let path = gix::path::relativize_with_prefix(path, prefix);
        relative = gix::path::to_unix_separators_on_windows(gix::path::into_bstr(path)?);
        &relative
    } else {
        path
    };
    let mut label = revision.to_vec();
    label.push(b':');
    label.extend_from_slice(&quote_patch_path(path, quote_high));
    Ok(label.into())
}
