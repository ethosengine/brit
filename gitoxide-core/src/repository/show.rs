//! `show`: commits with their changes, annotated tags with what they point to, and blobs or trees
//! printed like `cat`, in Git's default medium format and with `--no-renames`.

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
    revisions::{Change, PathFilter, changes, render},
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
}

/// Show each revision in `options` in order, like `git show --no-renames`.
///
/// A commit prints its header and its changes against its first parent (a root or shallow-boundary
/// commit against the empty tree). A merge prints its header, and with `--stat` or `--numstat` the
/// changes against its first parent, as Git does; where Git would print a combined diff (a path
/// that differs from every parent), it refuses rather than print a different patch. With paths, a
/// commit that does not change them is not shown. An annotated tag prints its header and message,
/// then the object it points to. A blob or tree prints like `cat`.
pub fn show(repo: gix::Repository, out: &mut dyn std::io::Write, options: Options) -> Result<()> {
    let budget = Budget::new(&std::env::temp_dir()).or_error()?;
    let mut rendered = std::io::BufWriter::new(budget.spool().or_error()?);
    let revisions = if options.revisions.is_empty() {
        vec![BString::from("HEAD")]
    } else {
        options.revisions.clone()
    };
    let mut state = State {
        filter: PathFilter::new(&repo, &options.paths)?,
        shallow: repo.shallow_commits()?,
        mailmap: repo.open_mailmap(),
        shown_one: false,
        options: &options,
    };
    for revision in &revisions {
        let id = repo.rev_parse_single(revision.as_bstr())?.detach();
        state.object(&repo, &mut rendered, id, &budget)?;
    }
    super::diff::write_diff_output(out, &mut rendered)
}

struct State<'a> {
    filter: PathFilter,
    shallow: Option<gix::shallow::Commits>,
    mailmap: gix::mailmap::Snapshot,
    /// Whether a commit, tag or tree was printed, after which a medium header is preceded by a blank line.
    shown_one: bool,
    options: &'a Options,
}

impl State<'_> {
    fn object(
        &mut self,
        repo: &gix::Repository,
        out: &mut dyn std::io::Write,
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
                    // Listed like `brit cat`, not with Git's `tree <name>` heading.
                    for entry in id.attach(repo).object()?.into_tree().iter() {
                        writeln!(out, "{}", entry?).or_error()?;
                    }
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
        // Like Git, the one-line format leaves out the tagger.
        if let Some(tagger) = tag.tagger()?.filter(|_| !self.options.oneline) {
            self.person(out, b"Tagger: ", tagger)?;
        }
        // Git prints the object from its blank header-ending line on: the message and any signature.
        if let Some(end) = object.data.find(b"\n\n") {
            out.write_all(&object.data[end + 1..]).or_error()?;
        }
        self.shown_one = true;
        Ok(tag.target())
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
        let object = repo.find_object(id)?;
        let commit = gix::objs::CommitRef::from_bytes(&object.data, repo.object_hash()).or_error()?;
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
        if !self.options.paths.is_empty() {
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
        out.write_all(&header).or_error()?;
        self.shown_one = true;
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
    fn subjects_join_the_first_paragraph() {
        assert_eq!(super::subject(b"one\ntwo\n\nthree\n".as_bstr()), b"one two");
        assert_eq!(super::subject(b"".as_bstr()), b"");
    }
}
