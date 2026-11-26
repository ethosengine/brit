use std::{collections::HashMap, io::Write};

use gix::{
    ObjectId, Result,
    bstr::{BStr, BString, ByteSlice},
    error::{ResultExt, bail, message},
    object::bounded::Source,
    objs::{CommitRef, CommitRefIter, Kind, find::Buffer},
    prelude::ObjectIdExt,
};

use super::super::{diff, show};
use super::Options;

/// A custom format with Git's separator or terminator semantics.
#[derive(Clone)]
pub struct PrettyFormat {
    template: String,
    terminate: bool,
}

impl PrettyFormat {
    /// Use a newline terminator for each commit, as for `--format`.
    pub fn terminated(template: String) -> Self {
        Self {
            template,
            terminate: true,
        }
    }

    /// Parse the explicit `format:` or `tformat:` spelling of `--pretty`.
    pub fn parse(pretty: String) -> Result<Self> {
        if let Some(template) = pretty.strip_prefix("format:") {
            Ok(Self {
                template: template.to_owned(),
                terminate: false,
            })
        } else if let Some(template) = pretty.strip_prefix("tformat:") {
            Ok(Self::terminated(template.to_owned()))
        } else {
            bail!(message!("Unsupported pretty format '{pretty}'; use format:<fmt> or tformat:<fmt>").validation());
        }
    }
}

enum Token<'a> {
    Text(&'a [u8]),
    Field(&'a str),
}

pub(super) struct Template<'a> {
    tokens: Vec<Token<'a>>,
    terminate: bool,
}

impl<'a> Template<'a> {
    pub(super) fn new(format: &'a PrettyFormat) -> Result<Self> {
        let mut tokens = Vec::new();
        let mut rest = format.template.as_str();
        while let Some((literal, fields)) = rest.split_once('%') {
            if !literal.is_empty() {
                tokens.push(Token::Text(literal.as_bytes()));
            }
            let mut chars = fields.char_indices();
            let _ = chars.next();
            if fields.starts_with(['a', 'c']) {
                let _ = chars.next();
            }
            let end = chars.next().map_or(fields.len(), |(offset, _)| offset);
            let field = &fields[..end];
            match field {
                "H" | "h" | "s" | "b" | "B" | "an" | "ae" | "ad" | "aI" | "at" | "cn" | "ce" | "cI" | "ct" | "P"
                | "p" | "d" | "D" | "n" | "%" => tokens.push(Token::Field(field)),
                _ => bail!(message!("Unknown log placeholder '%{field}'").validation()),
            }
            rest = &fields[end..];
        }
        if !rest.is_empty() {
            tokens.push(Token::Text(rest.as_bytes()));
        }
        Ok(Self {
            tokens,
            terminate: format.terminate,
        })
    }

    fn render(
        &self,
        log: &Render<'_>,
        commit: &CommitRef<'_>,
        id: ObjectId,
        parents: &[ObjectId],
        out: &mut Vec<u8>,
    ) -> Result<()> {
        use gix::date::time::format::ISO8601_STRICT;
        // Signature parsing is needed only for formats that display it.
        for token in &self.tokens {
            let bytes: &[u8] = match token {
                Token::Text(bytes) => bytes,
                Token::Field(field) => match *field {
                    "H" => {
                        write!(out, "{id}").or_error()?;
                        continue;
                    }
                    "h" => {
                        write!(out, "{}", id.attach(log.repo).shorten_or_id()).or_error()?;
                        continue;
                    }
                    "s" => {
                        out.extend_from_slice(&show::subject(commit.message));
                        continue;
                    }
                    "b" => body(commit.message),
                    "B" => commit.message,
                    "an" => commit.author()?.name,
                    "ae" => commit.author()?.email,
                    "cn" => commit.committer()?.name,
                    "ce" => commit.committer()?.email,
                    "ad" => {
                        write!(out, "{}", format_time(commit.author()?.time()?, log.date)).or_error()?;
                        continue;
                    }
                    "aI" | "cI" | "at" | "ct" => {
                        let actor = if field.starts_with('a') {
                            commit.author()?
                        } else {
                            commit.committer()?
                        };
                        let format = if field.ends_with('I') {
                            ISO8601_STRICT.into()
                        } else {
                            gix::date::time::Format::Unix
                        };
                        write!(out, "{}", format_time(actor.time()?, format)).or_error()?;
                        continue;
                    }
                    "P" | "p" => {
                        for (index, parent) in parents.iter().enumerate() {
                            if index != 0 {
                                out.push(b' ');
                            }
                            if *field == "P" {
                                write!(out, "{parent}").or_error()?;
                            } else {
                                write!(out, "{}", parent.attach(log.repo).shorten_or_id()).or_error()?;
                            }
                        }
                        continue;
                    }
                    "d" | "D" => {
                        if let Some(refs) = log.decorations.get(&id) {
                            decoration(out, refs, *field == "d");
                        }
                        continue;
                    }
                    "n" => b"\n",
                    "%" => b"%",
                    _ => unreachable!("validated placeholder"),
                },
            };
            out.extend_from_slice(bytes);
        }
        Ok(())
    }
}

fn body(message: &BStr) -> &[u8] {
    let mut offset = 0;
    let mut lines = message.split_inclusive(|byte| *byte == b'\n').peekable();
    for blank in [true, false, true] {
        while lines.peek().is_some_and(|line| show::is_blank(line) == blank) {
            if let Some(line) = lines.next() {
                offset += line.len();
            }
        }
    }
    &message[offset..]
}

fn format_time(time: gix::date::Time, format: gix::date::time::Format) -> String {
    let mut date = time.format_or_unix(format);
    // Git's strict ISO format uses Z to represent UTC.
    if date.ends_with("+00:00") {
        date.truncate(date.len() - "+00:00".len());
        date.push('Z');
    }
    date
}

enum Format<'a> {
    Legacy { oneline: bool },
    Custom(Template<'a>),
    Oneline,
    Medium(gix::mailmap::Snapshot),
}

struct Decoration {
    prefix: &'static [u8],
    name: BString,
}

fn decorations(repo: &gix::Repository) -> Result<HashMap<ObjectId, Vec<Decoration>>> {
    let head = repo.head()?;
    let head_ref = head.referent_name();
    let platform = repo.references()?;
    let mut refs = platform.all()?.collect::<Result<Vec<_>>>()?;
    refs.sort_unstable_by(|left, right| right.name().as_bstr().cmp(left.name().as_bstr()));
    let mut by_id: HashMap<_, Vec<_>> = HashMap::new();
    for mut reference in refs {
        let name = reference.name();
        let full = name.as_bstr();
        let tag = full.starts_with(b"refs/tags/");
        if !tag && !full.starts_with(b"refs/heads/") && !full.starts_with(b"refs/remotes/") && full != b"refs/stash" {
            continue;
        }
        let is_head = Some(name) == head_ref;
        let shortened = name.shorten().into();
        let id = reference.peel_to_id()?.detach();
        let refs = by_id.entry(id).or_default();
        if is_head {
            refs.insert(
                0,
                Decoration {
                    prefix: b"HEAD -> ",
                    name: shortened,
                },
            );
        } else {
            refs.push(Decoration {
                prefix: if tag { b"tag: " } else { b"" },
                name: shortened,
            });
        }
    }
    if head.is_detached()
        && let Some(id) = head.id()
    {
        by_id.entry(id.detach()).or_default().insert(
            0,
            Decoration {
                prefix: b"",
                name: "HEAD".into(),
            },
        );
    }
    Ok(by_id)
}

fn decoration(out: &mut Vec<u8>, refs: &[Decoration], parens: bool) {
    if parens {
        out.extend_from_slice(b" (");
    }
    for (index, reference) in refs.iter().enumerate() {
        if index != 0 {
            out.extend_from_slice(b", ");
        }
        out.extend_from_slice(reference.prefix);
        out.extend_from_slice(&reference.name);
    }
    if parens {
        out.push(b')');
    }
}

pub(super) struct Render<'a> {
    repo: &'a gix::Repository,
    out: &'a mut dyn Write,
    format: Format<'a>,
    date: gix::date::time::Format,
    decorations: HashMap<ObjectId, Vec<Decoration>>,
    filter: Option<diff::revisions::PathFilter>,
    shallow: Option<gix::shallow::Commits>,
    first_parent: bool,
    printed_header: bool,
}

impl<'a> Render<'a> {
    pub(super) fn new(
        repo: &'a gix::Repository,
        out: &'a mut dyn Write,
        options: &Options,
        paths: &[BString],
        template: Option<Template<'a>>,
    ) -> Result<Self> {
        let decorate = template.as_ref().is_some_and(|template| {
            template
                .tokens
                .iter()
                .any(|token| matches!(token, Token::Field("d" | "D")))
        });
        let format = match template {
            Some(template) => Format::Custom(template),
            None if options.stat && options.oneline => Format::Oneline,
            None if options.stat => Format::Medium(repo.open_mailmap()),
            None => Format::Legacy {
                oneline: options.oneline,
            },
        };
        Ok(Self {
            repo,
            out,
            format,
            date: options.date.unwrap_or_else(|| gix::date::time::format::DEFAULT.into()),
            decorations: if decorate { decorations(repo)? } else { HashMap::new() },
            filter: options
                .stat
                .then(|| diff::revisions::PathFilter::new(repo, paths))
                .transpose()?,
            shallow: if options.stat || options.pretty.is_some() {
                repo.shallow_commits()?
            } else {
                None
            },
            first_parent: matches!(options.parents, gix::traverse::commit::Parents::First),
            printed_header: false,
        })
    }

    pub(super) fn commit(&mut self, source: &Source<'_>, buffer: &mut Buffer, id: ObjectId) -> Result<()> {
        let object = source.load(&id, buffer)?;
        if object.kind != Kind::Commit {
            bail!(message!("Expected commit {id}").validation());
        }
        let data = show::commit_to_utf8(object.data, id)?;
        if let Format::Legacy { oneline } = self.format {
            let message = data.find(b"\n\n").map(|pos| &data[pos + 2..]).unwrap_or_default();
            let mut title = message.lines().next().unwrap_or(b"<no message>");
            if oneline {
                write!(self.out, "{} ", id.attach(self.repo).shorten_or_id()).or_error()?;
            } else {
                write!(self.out, "{} ", id.to_hex_with_len(8)).or_error()?;
            }
            gix::utils::stream::copy(&mut title, self.out, &gix::interrupt::IS_INTERRUPTED).or_error()?;
            writeln!(self.out).or_error()?;
        } else {
            let commit = CommitRef::from_bytes(&data, self.repo.object_hash()).or_error()?;
            let parents: Vec<_> = if self
                .shallow
                .as_ref()
                .is_some_and(|commits| commits.binary_search(&id).is_ok())
            {
                Vec::new()
            } else {
                commit.parents().collect()
            };
            let (has_header, stat_newline) = self.header(&commit, id, &parents)?;
            if let Some(filter) = &self.filter
                && (parents.len() < 2 || self.first_parent)
            {
                let old = parents
                    .first()
                    .map(|parent| {
                        let mut parent_buffer = Buffer::default();
                        let parent = source.load(parent, &mut parent_buffer)?;
                        CommitRefIter::from_bytes(parent.data, self.repo.object_hash()).tree_id()
                    })
                    .transpose()?;
                let new = Some(commit.tree());
                let changes = diff::revisions::stat_changes(self.repo, old, new, filter)?;
                if !changes.is_empty() {
                    if has_header && stat_newline {
                        writeln!(self.out).or_error()?;
                    }
                    diff::revisions::render(self.repo, self.out, &changes, (old, new), diff::Format::Stat)?;
                }
            }
        }
        buffer.clear();
        Ok(())
    }

    /// Write the header and report whether a following diffstat needs an extra newline.
    fn header(&mut self, commit: &CommitRef<'_>, id: ObjectId, parents: &[ObjectId]) -> Result<(bool, bool)> {
        let mut header = Vec::new();
        let (separate, terminate, stat_newline) = match &self.format {
            Format::Custom(template) => {
                if template.tokens.is_empty() {
                    return Ok((false, false));
                }
                template.render(self, commit, id, parents, &mut header)?;
                (!template.terminate, template.terminate, true)
            }
            Format::Oneline => {
                write!(header, "{} ", id.attach(self.repo).shorten_or_id()).or_error()?;
                header.extend_from_slice(&show::subject(commit.message));
                (false, true, false)
            }
            Format::Medium(mailmap) => {
                self.medium(&mut header, commit, id, parents, mailmap)?;
                (true, true, true)
            }
            Format::Legacy { .. } => unreachable!("legacy header is streamed directly"),
        };
        if separate && self.printed_header {
            writeln!(self.out).or_error()?;
        }
        self.out.write_all(&header).or_error()?;
        if terminate {
            writeln!(self.out).or_error()?;
        }
        self.printed_header = true;
        Ok((true, stat_newline))
    }

    fn medium(
        &self,
        header: &mut Vec<u8>,
        commit: &CommitRef<'_>,
        id: ObjectId,
        parents: &[ObjectId],
        mailmap: &gix::mailmap::Snapshot,
    ) -> Result<()> {
        writeln!(header, "commit {id}").or_error()?;
        if parents.len() > 1 {
            header.extend_from_slice(b"Merge:");
            for parent in parents {
                write!(header, " {}", parent.attach(self.repo).shorten_or_id()).or_error()?;
            }
            header.push(b'\n');
        }
        let author = commit.author()?;
        let author = mailmap.resolve(author);
        header.extend_from_slice(b"Author: ");
        header.extend_from_slice(&author.name);
        header.extend_from_slice(b" <");
        header.extend_from_slice(&author.email);
        header.extend_from_slice(b">\n");
        writeln!(header, "Date:   {}\n", format_time(author.time, self.date)).or_error()?;
        show::indent_message(header, commit.message);
        while header.last().is_some_and(|byte| show::is_blank(&[*byte])) {
            header.pop();
        }
        Ok(())
    }
}
