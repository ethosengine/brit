use gix::{
    Result,
    bstr::{BStr, BString, ByteSlice},
    error::{OptionExt, ResultExt, bail, message},
    objs::FindStream,
    prelude::ObjectIdExt,
};

use crate::OutputFormat;

#[derive(Eq, PartialEq, PartialOrd, Ord)]
enum VersionPart {
    String(BString),
    Number(usize),
}

/// `Version` is used to store multi-part version numbers. It does so in a rather naive way,
/// only distinguishing between parts that can be parsed as an integer and those that cannot.
///
/// `Version` does not parse version numbers in any structure-aware way, so `v0.a` is parsed into
/// `v`, `0`, `.a`.
///
/// Comparing two `Version`s comes down to comparing their `parts`. `parts` are either compared
/// numerically or lexicographically, depending on whether they are an integer or not. That way,
/// `v0.9` sorts before `v0.10` as one would expect from a version number.
///
/// When comparing versions of different lengths, shorter versions sort before longer ones (e.g.,
/// `v1.0` < `v1.0.1`). String parts always sort before numeric parts when compared directly.
#[derive(Eq, PartialEq, Ord, PartialOrd)]
struct Version {
    parts: Vec<VersionPart>,
}

impl Version {
    fn parse(version: &BStr) -> Self {
        let parts = version
            .chunk_by(|a, b| a.is_ascii_digit() == b.is_ascii_digit())
            .map(|part| {
                if let Ok(part) = part.to_str() {
                    part.parse::<usize>()
                        .map_or_else(|_| VersionPart::String(part.into()), VersionPart::Number)
                } else {
                    VersionPart::String(part.into())
                }
            })
            .collect();

        Self { parts }
    }
}

pub fn list(repo: gix::Repository, out: &mut dyn std::io::Write, format: OutputFormat) -> Result<()> {
    if format != OutputFormat::Human {
        bail!(gix::error::unsupported("JSON output isn't supported"));
    }

    let platform = repo.references()?;
    let mut budget = None;

    let mut tags: Vec<_> = platform
        .tags()?
        .map(|reference| -> Result<_> {
            let mut reference = reference?;
            let id = reference.follow_to_object()?.detach();
            // Lightweight tags need only the object's kind, never its complete contents.
            let tag = if repo.find_header(id)?.kind() == gix::objs::Kind::Tag {
                if budget.is_none() {
                    budget = Some(gix::utils::stream::Budget::new(&std::env::temp_dir()).or_error()?);
                }
                let (_, spool) = repo
                    .find_stream(
                        &id,
                        budget.as_ref().expect("annotated tags have scratch"),
                        &gix::interrupt::IS_INTERRUPTED,
                    )
                    .or_raise(|| message!("Could not read tag {} (object {id})", reference.name()))?;
                Some(spool.into_mapped().or_error()?)
            } else {
                None
            };
            let tag_ref = tag
                .as_ref()
                .map(|data| gix::objs::TagRef::from_bytes(data.as_slice(), repo.object_hash()))
                .transpose()?;

            // `name` is the name of the file in `refs/tags/`.
            // This applies to both lightweight and annotated tags.
            let name = reference.name().shorten();
            let mut fields = Vec::new();
            let version = Version::parse(name);
            Ok(match tag_ref {
                Some(tag_ref) => {
                    // `tag_name` is the name provided by the user via `git tag -a/-s/-u`.
                    // It is only present for annotated tags.
                    fields.push(format!(
                        "tag name: {}",
                        if name == tag_ref.name { "*".into() } else { tag_ref.name }
                    ));
                    if tag_ref.signature.is_some() {
                        fields.push("signed".into());
                    }

                    (version, format!("{name} [{fields}]", fields = fields.join(", ")))
                }
                _ => (version, name.to_string()),
            })
        })
        .collect::<Result<_>>()?;

    tags.sort_by(|a, b| a.0.cmp(&b.0));

    for (_, tag) in tags {
        writeln!(out, "{tag}").or_error()?;
    }

    Ok(())
}

pub mod create {
    /// What `brit tag [-a] [-m <msg>] [-f] <name> [<commit-ish>]` creates.
    pub struct Options {
        /// The tag's name below `refs/tags/`.
        pub name: String,
        /// The revision naming the object to tag.
        pub target: String,
        /// Write an annotated tag object, which requires a `message`.
        pub annotate: bool,
        /// The annotated tag's message; implies `annotate`.
        pub message: Option<String>,
        /// Replace an existing tag of the same name.
        pub force: bool,
    }
}

/// Validate `name` as Git's `check_tag_ref()` does and return its full reference name.
fn tag_name(name: &str) -> Result<gix::refs::FullName> {
    if name.starts_with('-') || name == "HEAD" {
        bail!(gix::error::validation(format!("'{name}' is not a valid tag name")));
    }
    let full: BString = format!("refs/tags/{name}").into();
    gix::validate::reference::name(full.as_bstr()).or_raise(|| message!("'{name}' is not a valid tag name"))?;
    gix::refs::FullName::try_from(full).or_error()
}

/// Git's comment prefix for message cleanup: the last of `core.commentString` and `core.commentChar`.
fn comment_prefix(repo: &gix::Repository) -> Result<String> {
    let mut prefix = "#".to_owned();
    if let Some(sections) = repo.config_snapshot().plumbing().sections_by_name("core") {
        for section in sections.filter(|section| section.header().subsection_name().is_none()) {
            if let Some(name) = section
                .value_names()
                .filter(|name| name.eq_ignore_ascii_case("commentString") || name.eq_ignore_ascii_case("commentChar"))
                .last()
                && let Some(value) = section.value(&name)
            {
                prefix = value.to_string();
            }
        }
    }
    if prefix == "auto" || prefix.is_empty() {
        bail!(gix::error::unsupported(
            "tag message cleanup with an automatic or empty comment prefix is not yet supported; use git tag"
        ));
    }
    Ok(prefix)
}

/// Create a lightweight tag, or an annotated tag object tagged by the committer identity.
pub fn create(repo: gix::Repository, options: create::Options, out: &mut dyn std::io::Write) -> Result<()> {
    use gix::refs::transaction::PreviousValue;
    let create::Options {
        name,
        target,
        annotate,
        message,
        force,
    } = options;
    let full_name = tag_name(&name)?;
    let object = repo.rev_parse_single(target.as_str())?.detach();
    let existing = repo
        .try_find_reference(full_name.as_ref())?
        .map(|reference| reference.target().into_owned());
    let constraint = match existing.clone() {
        Some(_) if !force => bail!(gix::error::validation(format!("tag '{name}' already exists"))),
        Some(previous) => PreviousValue::MustExistAndMatch(previous),
        None => PreviousValue::MustNotExist,
    };
    if annotate || message.is_some() {
        let message = message.ok_or_raise(|| {
            gix::error::validation("annotated tags need a message given with -m <msg>; brit launches no editor")
        })?;
        let message = super::commit::clean_message(&message, Some(comment_prefix(&repo)?.as_str()));
        if message.is_empty() {
            // gix encodes an empty message without the blank separator line Git always writes.
            bail!(gix::error::unsupported(
                "an annotated tag needs a non-empty message after cleanup; use git tag for empty messages"
            ));
        }
        let tagger = repo
            .committer()
            .ok_or_raise(|| gix::error::validation("tagger identity not configured"))??;
        let kind = repo.find_header(object)?.kind();
        repo.tag(&name, object, kind, Some(tagger), message, constraint)?;
    } else {
        repo.tag_reference(&name, object, constraint)?;
    }
    if let Some(gix::refs::Target::Object(previous)) = existing {
        let current = repo.find_reference(full_name.as_ref())?.id().detach();
        if previous != current {
            let was = previous.attach(&repo).shorten_or_id();
            writeln!(out, "Updated tag '{name}' (was {was})").or_error()?;
        }
    }
    Ok(())
}

/// Delete tags as `git tag --delete` does, but all of them or none.
pub fn delete(repo: gix::Repository, names: &[String], out: &mut dyn std::io::Write) -> Result<()> {
    use gix::refs::transaction::{Change, PreviousValue, RefEdit, RefLog};
    let mut edits = Vec::new();
    let mut reports = Vec::new();
    let mut seen = std::collections::BTreeSet::new();
    for name in names {
        let full_name = tag_name(name)?;
        if !seen.insert(full_name.clone()) {
            continue;
        }
        let Some(reference) = repo.try_find_reference(full_name.as_ref())? else {
            bail!(gix::error::not_found(format!("tag '{name}' not found.")));
        };
        let target = reference.target().into_owned();
        let was = match &target {
            gix::refs::Target::Object(id) => (*id).attach(&repo).shorten_or_id().to_string(),
            gix::refs::Target::Symbolic(referent) => referent.as_bstr().to_string(),
        };
        reports.push(format!("Deleted tag '{name}' (was {was})"));
        edits.push(RefEdit {
            change: Change::Delete {
                expected: PreviousValue::MustExistAndMatch(target),
                log: RefLog::AndReference,
            },
            name: full_name,
            deref: false,
        });
    }
    repo.edit_references(edits)?;
    for report in reports {
        writeln!(out, "{report}").or_error()?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cmp::Ordering;

    #[test]
    fn sorts_versions_correctly() {
        let mut actual = vec![
            "v2.0.0",
            "v1.10.0",
            "v1.2.1",
            "v1.0.0-beta",
            "v1.2",
            "v0.10.0",
            "v0.9.0",
            "v1.2.0",
            "v0.1.a",
            "v0.1.0",
            "v10.0.0",
            "1.0.0",
            "v1.0.0-alpha",
            "v1.0.0",
        ];

        actual.sort_by(|&a, &b| Version::parse(a.into()).cmp(&Version::parse(b.into())));
        let expected = [
            "v0.1.0",
            "v0.1.a",
            "v0.9.0",
            "v0.10.0",
            "v1.0.0",
            "v1.0.0-alpha",
            "v1.0.0-beta",
            "v1.2",
            "v1.2.0",
            "v1.2.1",
            "v1.10.0",
            "v2.0.0",
            "v10.0.0",
            "1.0.0",
        ];

        assert_eq!(actual, expected);
    }

    #[test]
    fn sorts_versions_with_different_lengths_correctly() {
        let v1 = Version::parse("v1.0".into());
        let v2 = Version::parse("v1.0.1".into());

        assert_eq!(v1.cmp(&v2), Ordering::Less);
        assert_eq!(v2.cmp(&v1), Ordering::Greater);
    }
}
