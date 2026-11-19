//! `brit for-each-ref`: Git's ref listing, with the format atoms and sort keys agents use.
//!
//! The default format is Git's `%(objectname) %(objecttype)\t%(refname)`, sorted by refname.
//! Patterns match like Git's: a prefix ending at a `/` boundary, or an `fnmatch` glob where `*`
//! does not cross `/`. Atoms and sort keys that are not implemented are refused by name.

use std::{collections::HashSet, io::Write};

use gix::{
    Result,
    bstr::{BStr, BString, ByteSlice},
    error::{ResultExt, bail, message},
    glob::wildmatch,
    prelude::ObjectIdExt,
};

use super::shorten_unambiguous;

/// What `for-each-ref` was asked for.
pub struct Options {
    /// The `--format` string, or Git's default.
    pub format: Option<String>,
    /// `--sort` keys in command-line order; the last one is primary.
    pub sort: Vec<String>,
    /// Ref name patterns; all refs when empty.
    pub patterns: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Atom {
    RefName,
    RefNameShort,
    ObjectName,
    ObjectNameShort,
    ObjectType,
    Subject,
    CommitterDate,
    CommitterDateShort,
    CommitterDateIso,
}

impl Atom {
    const IMPLEMENTED: &'static str = "refname, refname:short, objectname, objectname:short, objecttype, subject, committerdate, committerdate:short, committerdate:iso";

    fn parse(name: &str) -> Result<Self> {
        Ok(match name {
            "refname" => Atom::RefName,
            "refname:short" => Atom::RefNameShort,
            "objectname" => Atom::ObjectName,
            "objectname:short" => Atom::ObjectNameShort,
            "objecttype" => Atom::ObjectType,
            "subject" => Atom::Subject,
            "committerdate" => Atom::CommitterDate,
            "committerdate:short" => Atom::CommitterDateShort,
            "committerdate:iso" | "committerdate:iso8601" => Atom::CommitterDateIso,
            other => bail!(message!(
                "brit for-each-ref does not implement the field `%({other})`; implemented: {}",
                Atom::IMPLEMENTED
            )),
        })
    }
}

enum Piece {
    Literal(BString),
    Atom(Atom),
}

fn parse_format(format: &str) -> Result<Vec<Piece>> {
    let mut pieces = Vec::new();
    let mut literal = BString::default();
    let bytes = format.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' {
            if bytes.get(i + 1) == Some(&b'(') {
                let Some(end) = bytes[i + 2..].find_byte(b')') else {
                    bail!(message!("malformed format string {format}"));
                };
                let name = &format[i + 2..i + 2 + end];
                if !literal.is_empty() {
                    pieces.push(Piece::Literal(std::mem::take(&mut literal)));
                }
                pieces.push(Piece::Atom(Atom::parse(name)?));
                i += end + 3;
                continue;
            }
            if bytes.get(i + 1) == Some(&b'%') {
                literal.push(b'%');
                i += 2;
                continue;
            }
            // Git's `hex2chr()`: exactly two hex digits; `from_str_radix` alone would accept `%+1`.
            if let Some(byte) = bytes
                .get(i + 1..i + 3)
                .filter(|hex| hex.iter().all(u8::is_ascii_hexdigit))
                .and_then(|hex| std::str::from_utf8(hex).ok())
                .and_then(|hex| u8::from_str_radix(hex, 16).ok())
            {
                literal.push(byte);
                i += 3;
                continue;
            }
        }
        literal.push(bytes[i]);
        i += 1;
    }
    if !literal.is_empty() {
        pieces.push(Piece::Literal(literal));
    }
    Ok(pieces)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SortKey {
    RefName,
    CommitterDate,
}

struct Entry {
    name: BString,
    id: gix::ObjectId,
    kind: gix::object::Kind,
    subject: BString,
    committer_time: Option<gix::date::Time>,
}

/// Git's `match_name_as_path()`.
fn matches(patterns: &[String], name: &BStr) -> bool {
    patterns.is_empty()
        || patterns.iter().any(|pattern| {
            let pattern = pattern.as_bytes();
            let boundary = name.starts_with(pattern)
                && (name.len() == pattern.len() || name[pattern.len()] == b'/' || pattern.last() == Some(&b'/'));
            boundary || wildmatch(pattern.as_bstr(), name, wildmatch::Mode::NO_MATCH_SLASH_LITERAL)
        })
}

/// List refs like `git for-each-ref`.
pub fn for_each_ref(repo: gix::Repository, mut out: impl Write, options: Options) -> Result<()> {
    let pieces = parse_format(
        options
            .format
            .as_deref()
            .unwrap_or("%(objectname) %(objecttype)\t%(refname)"),
    )?;
    let mut keys = Vec::new();
    for key in options.sort.iter().rev() {
        let (descending, name) = key.strip_prefix('-').map_or((false, key.as_str()), |name| (true, name));
        let key = match name {
            "refname" => SortKey::RefName,
            "committerdate" => SortKey::CommitterDate,
            other => bail!(message!(
                "brit for-each-ref does not implement the sort key `{other}`; implemented: refname, committerdate (prefix `-` to reverse)"
            )),
        };
        keys.push((key, descending));
    }

    let mut all_names = HashSet::new();
    let mut entries = Vec::new();
    for reference in repo.references()?.all()? {
        let Ok(mut reference) = reference else { continue };
        let name = reference.name().as_bstr().to_owned();
        all_names.insert(name.to_string());
        if !matches(&options.patterns, name.as_ref()) {
            continue;
        }
        let Ok(id) = reference.follow_to_object() else {
            writeln!(std::io::stderr(), "warning: ignoring broken ref {name}").ok();
            continue;
        };
        let object = id.object()?;
        let kind = object.kind;
        let (subject, committer_time) = match kind {
            gix::object::Kind::Commit => {
                let commit = object.into_commit();
                let subject = commit.message()?.summary().into_owned();
                (subject, Some(commit.committer()?.time()?))
            }
            gix::object::Kind::Tag => {
                let tag = object.into_tag();
                let decoded = tag.decode()?;
                let subject = gix::objs::commit::MessageRef::from_bytes(decoded.message)
                    .summary()
                    .into_owned();
                (subject, None)
            }
            _ => (BString::default(), None),
        };
        entries.push(Entry {
            name,
            id: id.detach(),
            kind,
            subject,
            committer_time,
        });
    }

    entries.sort_by(|a, b| {
        for (key, descending) in &keys {
            let ordering = match key {
                SortKey::RefName => a.name.cmp(&b.name),
                SortKey::CommitterDate => a
                    .committer_time
                    .map_or(0, |t| t.seconds)
                    .cmp(&b.committer_time.map_or(0, |t| t.seconds)),
            };
            let ordering = if *descending { ordering.reverse() } else { ordering };
            if ordering.is_ne() {
                return ordering;
            }
        }
        a.name.cmp(&b.name)
    });

    let exists = |candidate: &str| {
        if candidate.starts_with("refs/") {
            all_names.contains(candidate)
        } else {
            super::rev_parse::ref_exists(&repo, candidate)
        }
    };
    let mut line = BString::default();
    for entry in &entries {
        line.clear();
        for piece in &pieces {
            match piece {
                Piece::Literal(text) => line.extend_from_slice(text),
                Piece::Atom(atom) => match atom {
                    Atom::RefName => line.extend_from_slice(&entry.name),
                    Atom::RefNameShort => {
                        line.extend_from_slice(shorten_unambiguous(&entry.name.to_str_lossy(), exists).as_bytes());
                    }
                    Atom::ObjectName => line.extend_from_slice(entry.id.to_string().as_bytes()),
                    Atom::ObjectNameShort => {
                        line.extend_from_slice(entry.id.attach(&repo).shorten_or_id().to_string().as_bytes());
                    }
                    Atom::ObjectType => line.extend_from_slice(entry.kind.as_bytes()),
                    Atom::Subject => line.extend_from_slice(&entry.subject),
                    Atom::CommitterDate | Atom::CommitterDateShort | Atom::CommitterDateIso => {
                        if let Some(time) = entry.committer_time {
                            let format = match atom {
                                Atom::CommitterDateShort => gix::date::time::format::SHORT,
                                Atom::CommitterDateIso => gix::date::time::format::ISO8601,
                                _ => gix::date::time::format::DEFAULT,
                            };
                            line.extend_from_slice(time.format_or_unix(format).as_bytes());
                        }
                    }
                },
            }
        }
        line.push(b'\n');
        out.write_all(&line).or_error()?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{matches, parse_format};
    use gix::bstr::ByteSlice;

    #[test]
    fn patterns_match_like_git() {
        let p = |list: &[&str]| list.iter().map(|s| (*s).to_owned()).collect::<Vec<_>>();
        assert!(matches(&p(&["refs/heads"]), b"refs/heads/main".as_bstr()));
        assert!(matches(&p(&["refs/heads/"]), b"refs/heads/main".as_bstr()));
        assert!(!matches(&p(&["refs/hea"]), b"refs/heads/main".as_bstr()));
        assert!(matches(&p(&["refs/*/main"]), b"refs/heads/main".as_bstr()));
        assert!(!matches(&p(&["refs/*/main"]), b"refs/remotes/origin/main".as_bstr()));
        assert!(matches(
            &p(&["refs/heads/explore/0.7.1*"]),
            b"refs/heads/explore/0.7.1-a".as_bstr()
        ));
    }

    #[test]
    fn unknown_atoms_are_refused_by_name() {
        let error = parse_format("%(refname) %(authorname)")
            .err()
            .expect("unimplemented atom");
        assert!(error.to_string().contains("%(authorname)"), "{error}");
        assert!(parse_format("%% %41 %(refname:short)").is_ok());
        let literal = |format: &str| match parse_format(format).expect("valid format").as_slice() {
            [super::Piece::Literal(text)] => text.clone(),
            _ => panic!("{format} is one literal"),
        };
        assert_eq!(literal("%+1"), "%+1", "a sign is not a hex digit");
        assert_eq!(literal("%-1"), "%-1");
        assert_eq!(literal("%41"), "A");
    }
}
