//! `show`: commits with their changes, annotated tags with what they point to, blobs and trees, in
//! Git's default medium format and with `--no-renames`.
//!
//! Known limits, each a refusal rather than different output: a merge whose result differs from
//! every parent at some path (Git's combined diff) in the patch, `--name-only` and `--name-status`
//! forms; a commit declaring an encoding Brit cannot decode exactly as Git's iconv would; an output
//! encoding (`i18n.logOutputEncoding`, else `i18n.commitEncoding`) other than UTF-8; and the diff
//! limits of [`super::diff`].

use std::borrow::Cow;

use gix::{
    ObjectId, Result,
    bstr::{BStr, BString, ByteSlice},
    error::{ResultExt, bail, unsupported},
    objs::Kind,
    prelude::ObjectIdExt,
    utils::stream::{self, Budget},
};

use super::diff::{
    Format,
    revisions::{Change, PathFilter, changes, render, split_positionals},
};

/// What `show` prints for each revision.
#[derive(Debug, Default, Clone)]
pub struct Options {
    /// The revisions to show, `HEAD` when empty.
    pub revisions: Vec<BString>,
    /// Literal paths restricting the changes of commits.
    pub paths: Vec<BString>,
    /// How to print a commit's changes, or `None` to print only its header (`--no-patch`).
    pub format: Option<Format>,
    /// Print each commit as its abbreviated id and title, like `--oneline`.
    pub oneline: bool,
    /// Whether `--` was given, which makes every name before it a revision even without paths.
    pub separator: bool,
}

/// Show each revision in `options` in order, like `git show --no-renames`.
///
/// A commit prints its header and its changes against its first parent (a root or shallow-boundary
/// commit against the empty tree). A merge prints its header, and with `--stat` or `--numstat` the
/// changes against its first parent, as Git does; where Git would print a combined diff (a path
/// that differs from every parent), it refuses rather than print a different patch. With paths, a
/// commit that does not change them is not shown. An annotated tag prints its header and message,
/// then the object it points to. A blob prints its content, and a tree `tree <name>`, a blank line
/// and its entry names, directories with a trailing slash, as in Git.
pub fn show(repo: gix::Repository, out: &mut dyn std::io::Write, options: Options) -> Result<()> {
    // Git re-encodes commit headers and messages to the log output encoding, which falls back to
    // the commit encoding. Text that is ASCII reads the same in any ASCII-compatible encoding.
    let mut output = None;
    let config = repo.config_snapshot();
    for key in ["i18n.logOutputEncoding", "i18n.commitEncoding"] {
        if let Some(encoding) = config.string(key) {
            if !is_utf8_label(encoding.as_ref()) {
                let refusal = format!("{key} asks for '{encoding}' output, but brit show prints UTF-8 only");
                if !is_ascii_compatible(encoding.as_ref()) {
                    bail!(unsupported(format!("{refusal}; use git show")));
                }
                output = Some(refusal);
            }
            break;
        }
    }
    let (revisions, positional_paths) = split_positionals(
        &repo,
        &options.revisions,
        options.separator || !options.paths.is_empty(),
    )?;
    let paths: Vec<BString> = positional_paths
        .into_iter()
        .chain(options.paths.iter().cloned())
        .collect();
    if let Some(range) = revisions.iter().find(|revision| revision.contains_str("..")) {
        bail!(unsupported(format!(
            "brit show takes single revisions, not the range '{range}'; use brit log or git show"
        )));
    }
    let budget = Budget::new(&std::env::temp_dir()).or_error()?;
    let mut rendered = std::io::BufWriter::new(budget.spool().or_error()?);
    let revisions = if revisions.is_empty() {
        vec![BString::from("HEAD")]
    } else {
        revisions
    };
    let mut state = State {
        filter: PathFilter::new(&repo, &paths)?,
        has_paths: !paths.is_empty(),
        output,
        shallow: repo.shallow_commits()?,
        mailmap: repo.open_mailmap(),
        shown_one: false,
        shown_commits: std::collections::HashSet::new(),
        options: &options,
    };
    for revision in &revisions {
        let id = repo.rev_parse_single(revision.as_bstr())?.detach();
        state.object(&repo, &mut rendered, revision.as_bstr(), id, &budget)?;
    }
    super::diff::write_diff_output(out, &mut rendered)
}

struct State<'a> {
    filter: PathFilter,
    /// Whether paths restrict the changes, which hides commits that do not touch them.
    has_paths: bool,
    /// The refusal to raise when text Git would re-encode to a non-UTF-8 output is not ASCII.
    output: Option<String>,
    shallow: Option<gix::shallow::Commits>,
    mailmap: gix::mailmap::Snapshot,
    /// Whether a commit, tag or tree was printed, after which a medium header, tag or tree is preceded
    /// by a blank line.
    shown_one: bool,
    /// Commits already printed, which Git never prints twice in one call (tags and trees it does).
    shown_commits: std::collections::HashSet<ObjectId>,
    options: &'a Options,
}

impl State<'_> {
    /// Show the object `name` resolved to, following tags; a tree reached through a tag keeps `name`.
    fn object(
        &mut self,
        repo: &gix::Repository,
        out: &mut dyn std::io::Write,
        name: &BStr,
        mut id: ObjectId,
        budget: &std::sync::Arc<Budget>,
    ) -> Result<()> {
        let mut seen = std::collections::HashSet::new();
        loop {
            match repo.find_header(id)?.kind() {
                Kind::Commit => return self.commit(repo, out, id),
                Kind::Tag => {
                    if !seen.insert(id) {
                        bail!(gix::error::corruption(format!("Tag cycle at {id}")));
                    }
                    id = self.tag(repo, out, id)?;
                }
                Kind::Blob => {
                    let (_, mut data) = gix::objs::FindStream::find_stream(
                        &repo.objects,
                        &id,
                        budget,
                        &gix::interrupt::IS_INTERRUPTED,
                    )?;
                    stream::copy(&mut data, out, &gix::interrupt::IS_INTERRUPTED).or_error()?;
                    return Ok(());
                }
                Kind::Tree => {
                    // Git's heading, then each entry's name, a directory with a trailing slash.
                    if self.shown_one {
                        out.write_all(b"\n").or_error()?;
                    }
                    out.write_all(b"tree ").or_error()?;
                    out.write_all(name).or_error()?;
                    out.write_all(b"\n\n").or_error()?;
                    for entry in id.attach(repo).object()?.into_tree().iter() {
                        let entry = entry?;
                        out.write_all(entry.filename()).or_error()?;
                        out.write_all(if entry.mode().is_tree() { b"/\n" } else { b"\n" })
                            .or_error()?;
                    }
                    self.shown_one = true;
                    return Ok(());
                }
            }
        }
    }

    /// Print an annotated tag like Git's `show` and return the object it points to.
    fn tag(&mut self, repo: &gix::Repository, out: &mut dyn std::io::Write, id: ObjectId) -> Result<ObjectId> {
        let object = repo.find_object(id)?;
        let tag = gix::objs::TagRef::from_bytes(&object.data, repo.object_hash()).or_error()?;
        if self.shown_one {
            out.write_all(b"\n").or_error()?;
        }
        out.write_all(b"tag ").or_error()?;
        out.write_all(tag.name).or_error()?;
        out.write_all(b"\n").or_error()?;
        // Like Git, the one-line format leaves out the tagger, the only part Git re-encodes.
        if let Some(tagger) = tag.tagger()?.filter(|_| !self.options.oneline) {
            let mut person = Vec::new();
            self.person(&mut person, b"Tagger: ", tagger)?;
            self.ascii_for_output(&person, "tag", id)?;
            out.write_all(&person).or_error()?;
        }
        // Git prints the object from its blank header-ending line on: the message and any signature.
        if let Some(end) = object.data.find(b"\n\n") {
            out.write_all(&object.data[end + 1..]).or_error()?;
        }
        self.shown_one = true;
        Ok(tag.target())
    }

    /// Refuse when a non-UTF-8 output encoding is configured and `text` is not ASCII.
    fn ascii_for_output(&self, text: &[u8], kind: &str, id: ObjectId) -> Result<()> {
        match &self.output {
            Some(refusal) if !text.is_ascii() => {
                bail!(unsupported(format!(
                    "{refusal} and {kind} {id} is not ASCII; use git show"
                )))
            }
            _ => Ok(()),
        }
    }

    fn person(&self, out: &mut dyn std::io::Write, label: &[u8], person: gix::actor::SignatureRef<'_>) -> Result<()> {
        let time = person.time()?;
        let resolved = self.mailmap.try_resolve(person);
        let (name, email) = match &resolved {
            Some(signature) => (signature.name.as_bstr(), signature.email.as_bstr()),
            None => (person.name, person.email),
        };
        out.write_all(label).or_error()?;
        out.write_all(name).or_error()?;
        out.write_all(b" <").or_error()?;
        out.write_all(email).or_error()?;
        out.write_all(b">\n").or_error()?;
        writeln!(out, "Date:   {}", time.format_or_unix(gix::date::time::format::DEFAULT)).or_error()?;
        Ok(())
    }

    fn commit(&mut self, repo: &gix::Repository, out: &mut dyn std::io::Write, id: ObjectId) -> Result<()> {
        if self.shown_commits.contains(&id) {
            return Ok(());
        }
        let object = repo.find_object(id)?;
        let declared = gix::objs::CommitRef::from_bytes(&object.data, repo.object_hash())
            .or_error()?
            .encoding;
        // Like Git, the whole commit (names and message) is re-encoded from its declared encoding.
        let data = to_utf8(&object.data, declared, id)?;
        let commit = gix::objs::CommitRef::from_bytes(&data, repo.object_hash()).or_error()?;
        let tree = commit.tree();
        // A shallow boundary commit has no parents, as in Git.
        let parents: Vec<ObjectId> = if self
            .shallow
            .as_ref()
            .is_some_and(|commits| commits.binary_search(&id).is_ok())
        {
            Vec::new()
        } else {
            commit.parents().collect()
        };
        let parent_trees = parents
            .iter()
            .map(|parent| Ok(parent.attach(repo).object()?.peel_to_commit()?.tree_id()?.detach()))
            .collect::<Result<Vec<_>>>()?;
        let against = |parent: Option<ObjectId>| changes(repo, parent, Some(tree), &self.filter);
        let first = against(parent_trees.first().copied())?;
        let merge = parent_trees.len() > 1;
        // With paths, a commit equal to a parent there is not shown at all.
        if self.has_paths {
            if !merge && first.is_empty() {
                return Ok(());
            }
            if merge {
                for parent in &parent_trees[1..] {
                    if against(Some(*parent))?.is_empty() {
                        return Ok(());
                    }
                }
                if first.is_empty() {
                    return Ok(());
                }
            }
        }
        let mut header = Vec::new();
        if self.options.oneline {
            header.extend_from_slice(id.attach(repo).shorten_or_id().to_string().as_bytes());
            header.push(b' ');
            header.extend_from_slice(&subject(commit.message));
        } else {
            if self.shown_one {
                out.write_all(b"\n").or_error()?;
            }
            header.extend_from_slice(format!("commit {id}\n").as_bytes());
            if merge {
                header.extend_from_slice(b"Merge:");
                for parent in &parents {
                    header.extend_from_slice(format!(" {}", parent.attach(repo).shorten_or_id()).as_bytes());
                }
                header.push(b'\n');
            }
            self.person(&mut header, b"Author: ", commit.author()?)?;
            header.push(b'\n');
            indent_message(&mut header, commit.message);
        }
        // Git trims trailing whitespace from the whole header, then ends it with one newline.
        while header.last().is_some_and(|byte| is_space(*byte)) {
            header.pop();
        }
        header.push(b'\n');
        self.ascii_for_output(&header, "commit", id)?;
        out.write_all(&header).or_error()?;
        self.shown_one = true;
        self.shown_commits.insert(id);
        let Some(format) = self.options.format else {
            return Ok(());
        };
        if merge {
            if !matches!(format, Format::Stat | Format::NumStat) {
                let combined = combined_paths(&first, &parent_trees[1..], &against)?;
                if let Some(path) = combined.first() {
                    bail!(unsupported(format!(
                        "merge {id} changes '{path}' against every parent, which needs a combined diff; use git show"
                    )));
                }
            }
            // Git separates a merge's header from its (here empty or first-parent) diff in every format.
            out.write_all(b"\n").or_error()?;
            if matches!(format, Format::Stat | Format::NumStat) {
                render(repo, out, &first, (parent_trees.first().copied(), Some(tree)), format)?;
            }
            return Ok(());
        }
        if first.is_empty() {
            return Ok(());
        }
        if !self.options.oneline {
            out.write_all(b"\n").or_error()?;
        }
        render(repo, out, &first, (parent_trees.first().copied(), Some(tree)), format)
    }
}

/// Whether `label` names an encoding in which ASCII text is the same bytes as in UTF-8.
fn is_ascii_compatible(label: &BStr) -> bool {
    is_latin1_or_ascii(&normalize(label))
        || gix::filter::plumbing::encoding::Encoding::for_label(label).is_some_and(ascii_compatible)
}

/// Whether ASCII text is the same bytes in `encoding` as in UTF-8: every encoding but UTF-16,
/// UTF-32 and the stateful ISO-2022-JP.
fn ascii_compatible(encoding: gix::filter::plumbing::encoding::Encoding) -> bool {
    let name = encoding.name();
    !(name.starts_with("UTF-16") || name.starts_with("UTF-32") || name == "ISO-2022-JP")
}

/// `label` lowercased without punctuation, so `ISO_8859-1` and `iso88591` compare equal.
fn normalize(label: &[u8]) -> Vec<u8> {
    label
        .iter()
        .filter(|byte| byte.is_ascii_alphanumeric())
        .map(u8::to_ascii_lowercase)
        .collect()
}

/// Whether a normalized label names Latin-1 or ASCII, which Git's iconv and WHATWG disagree on.
fn is_latin1_or_ascii(normalized: &[u8]) -> bool {
    LATIN1.contains(&normalized) || ASCII.contains(&normalized)
}

const LATIN1: &[&[u8]] = &[
    b"iso88591",
    b"iso885911987",
    b"latin1",
    b"l1",
    b"cp819",
    b"ibm819",
    b"isoir100",
    b"csisolatin1",
];
const ASCII: &[&[u8]] = &[b"usascii", b"ascii", b"ansix341968", b"csascii"];

/// Whether Git treats `label` as UTF-8 and so converts nothing.
fn is_utf8_label(label: &BStr) -> bool {
    label.eq_ignore_ascii_case(b"utf-8") || label.eq_ignore_ascii_case(b"utf8")
}

/// `data` (a commit object) re-encoded from the `encoding` it declares to UTF-8, as Git's
/// `logmsg_reencode()` does with iconv.
///
/// No header or a UTF-8 label converts nothing. Latin-1 maps each byte to the code point of the
/// same value, and ASCII converts nothing. Any other label must name an ASCII-compatible encoding
/// that `gix-encoding` knows by one of its iconv names; bytes that do not decode print unchanged,
/// as when iconv fails in Git. A label `gix-encoding` does not know is refused by name.
pub(crate) fn to_utf8<'a>(data: &'a [u8], encoding: Option<&BStr>, id: ObjectId) -> Result<Cow<'a, [u8]>> {
    use gix::filter::plumbing::encoding::Encoding;
    let Some(label) = encoding.filter(|label| !is_utf8_label(label)) else {
        return Ok(Cow::Borrowed(data));
    };
    let normalized = normalize(label);
    if LATIN1.contains(&normalized.as_slice()) && !data.is_ascii() {
        return Ok(Cow::Owned(
            data.iter()
                .map(|&byte| char::from(byte))
                .collect::<String>()
                .into_bytes(),
        ));
    }
    if is_latin1_or_ascii(&normalized) {
        return Ok(Cow::Borrowed(data));
    }
    let known = Encoding::for_label(label).filter(|encoding| ascii_compatible(*encoding));
    let Some(known) = known else {
        bail!(unsupported(format!(
            "commit {id} declares the encoding '{label}', which brit cannot decode as Git would; use git show"
        )));
    };
    Ok(
        match gix::filter::plumbing::encoding::convert(data, known, Encoding::Utf8) {
            Ok(text) => Cow::Owned(text),
            Err(_) => Cow::Borrowed(data),
        },
    )
}

/// The value of the `encoding` header of the commit object `data`, if it declares one.
pub(crate) fn declared_encoding(data: &[u8]) -> Option<&BStr> {
    data.lines()
        .take_while(|line| !line.is_empty())
        .find_map(|line| line.strip_prefix(b"encoding ").map(ByteSlice::as_bstr))
}

/// `data` (a commit object) in UTF-8 whichever `encoding` it declares, as [`to_utf8()`] converts it.
pub(crate) fn commit_to_utf8(data: &[u8], id: ObjectId) -> Result<Cow<'_, [u8]>> {
    to_utf8(data, declared_encoding(data), id)
}

/// The paths changed against the first parent (`first`) that also differ from every other parent.
fn combined_paths(
    first: &[Change],
    others: &[ObjectId],
    against: &dyn Fn(Option<ObjectId>) -> Result<Vec<Change>>,
) -> Result<Vec<BString>> {
    let mut paths: Vec<BString> = first.iter().map(|change| change.path.clone()).collect();
    for parent in others {
        let changed = against(Some(*parent))?;
        paths.retain(|path| changed.iter().any(|change| &change.path == path));
    }
    Ok(paths)
}

/// Git's `isspace()`, which also counts the vertical tab.
fn is_space(byte: u8) -> bool {
    byte.is_ascii_whitespace() || byte == 0x0b
}

fn is_blank(line: &[u8]) -> bool {
    line.iter().all(|byte| is_space(*byte))
}

/// The message lines after leading blank lines, without line terminators.
fn message_lines(message: &BStr) -> impl Iterator<Item = &[u8]> {
    let message = message.strip_suffix(b"\n").unwrap_or(message);
    message.split(|byte| *byte == b'\n').skip_while(|line| is_blank(line))
}

/// Git's medium body: every line indented by four columns, tabs expanded to eight-column stops.
fn indent_message(out: &mut Vec<u8>, message: &BStr) {
    use unicode_width::UnicodeWidthChar;
    for line in message_lines(message) {
        out.extend_from_slice(b"    ");
        if !is_blank(line) {
            let mut column = 0;
            for chunk in line.split_inclusive(|byte| *byte == b'\t') {
                let (text, tab) = match chunk.strip_suffix(b"\t") {
                    Some(text) => (text, true),
                    None => (chunk, false),
                };
                out.extend_from_slice(text);
                column += std::str::from_utf8(text)
                    .map_or(text.len(), |text| text.chars().map(|c| c.width().unwrap_or(0)).sum());
                if tab {
                    let spaces = 8 - column % 8;
                    out.extend(std::iter::repeat_n(b' ', spaces));
                    column += spaces;
                }
            }
        }
        out.push(b'\n');
    }
}

/// Git's one-line subject: the first paragraph with its lines joined by spaces.
fn subject(message: &BStr) -> Vec<u8> {
    let mut subject = Vec::new();
    for line in message_lines(message).take_while(|line| !is_blank(line)) {
        if !subject.is_empty() {
            subject.push(b' ');
        }
        subject.extend_from_slice(line);
    }
    subject
}

#[cfg(test)]
mod tests {
    use gix::bstr::ByteSlice;

    #[test]
    fn medium_bodies_indent_and_expand_tabs_like_git() {
        let mut out = Vec::new();
        super::indent_message(&mut out, b"\n\ntitle\n\n\tbody\tx\n".as_bstr());
        assert_eq!(out.as_bstr(), b"    title\n    \n            body    x\n".as_bstr());
    }

    #[test]
    fn declared_encodings_convert_like_iconv_or_refuse() {
        use std::borrow::Cow;
        let id = gix::ObjectId::null(gix::hash::Kind::Sha1);
        let utf8 = |data: &'static [u8], label: &'static str| {
            super::to_utf8(data, Some(label.as_bytes().as_bstr()), id).map(Cow::into_owned)
        };
        assert_eq!(
            utf8(b"caf\xe9 \x80", "ISO-8859-1").unwrap(),
            "caf\u{e9} \u{80}".as_bytes()
        );
        assert_eq!(utf8(b"caf\xe9", "UTF8").unwrap(), b"caf\xe9");
        assert_eq!(
            super::declared_encoding(b"tree t\nencoding ISO-8859-1\n\nencoding x\n"),
            Some(b"ISO-8859-1".as_bstr())
        );
        assert_eq!(super::declared_encoding(b"tree t\n\nencoding x\n"), None);
        // The Asian encodings come from the brit binary's `gix` features (`encoding-asian`), which a
        // build of this crate on its own, like `cargo test -p gitoxide-core`, does not enable.
        if gix::filter::plumbing::encoding::Encoding::for_label(b"Shift_JIS").is_none() {
            return;
        }
        assert_eq!(utf8(b"\x82\xa0", "Shift_JIS").unwrap(), "\u{3042}".as_bytes());
        assert_eq!(
            utf8(b"bad \x81", "Shift_JIS").unwrap(),
            b"bad \x81",
            "undecodable bytes stay"
        );
        assert!(utf8(b"x", "x-no-such-encoding").is_err());
        assert_eq!(
            utf8(b"\x82\xa0", "windows-31j").unwrap(),
            "\u{3042}".as_bytes(),
            "iconv's name for CP932"
        );
    }

    #[test]
    fn subjects_join_the_first_paragraph() {
        assert_eq!(super::subject(b"one\ntwo\n\nthree\n".as_bstr()), b"one two");
        assert_eq!(super::subject(b"".as_bstr()), b"");
    }
}
