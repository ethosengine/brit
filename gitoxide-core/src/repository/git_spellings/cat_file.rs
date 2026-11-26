//! Read one object's contents or metadata using Git's `cat-file` output and status.

use std::io::Write;

use gix::{Result, bstr::BStr, error::ResultExt, objs::Kind};

use super::exit;

/// The requested object operation.
#[derive(Debug, Clone, Copy)]
pub enum Mode {
    Pretty,
    Kind,
    Size,
    Exists,
}

pub fn run(repo: &gix::Repository, object: &BStr, mode: Mode, mut out: impl Write, mut err: impl Write) -> Result<()> {
    let invalid_name = || format!("fatal: Not a valid object name {object}");
    // A full hexadecimal object name is valid syntax even when the object is absent.
    let id = if let Ok(id) = gix::ObjectId::from_hex(object)
        && id.kind() == repo.object_hash()
    {
        id
    } else if let Ok(id) = repo.rev_parse_single(object) {
        id.detach()
    } else {
        let diagnostic = std::str::from_utf8(object)
            .ok()
            .and_then(|object| super::rev_parse::misspelt_object_path(repo, object))
            .unwrap_or_else(invalid_name);
        writeln!(err, "{diagnostic}").ok();
        return Err(exit(128));
    };
    if matches!(mode, Mode::Exists) {
        return if repo.has_object(id) { Ok(()) } else { Err(exit(1)) };
    }
    let Some(header) = repo.try_find_header(id)? else {
        let diagnostic = if matches!(mode, Mode::Pretty) {
            invalid_name()
        } else {
            "fatal: git cat-file: could not get object info".into()
        };
        writeln!(err, "{diagnostic}").ok();
        return Err(exit(128));
    };
    match mode {
        Mode::Kind => writeln!(out, "{}", header.kind()).or_error(),
        Mode::Size => writeln!(out, "{}", header.size()).or_error(),
        Mode::Pretty if header.kind() == Kind::Tree => crate::repository::tree::pretty_tree(repo, id, out),
        Mode::Pretty => crate::repository::cat::display_raw_object(repo, &id, out),
        Mode::Exists => unreachable!("existence was checked before reading the object header"),
    }
}
