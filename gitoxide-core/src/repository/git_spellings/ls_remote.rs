//! `brit ls-remote`: Git's spelling of `brit remote refs`, printing `<oid>\t<refname>` lines.
//!
//! Refs print in the server's order. An annotated tag prints its tag object and then its peeled
//! object as `<refname>^{}`, and patterns match the tail of a ref name at a `/` boundary, both
//! like Git. An unborn `HEAD` is not listed, as Git does not request it without `--symref`.

use gix::{Result, bstr::BString, error::ResultExt, protocol::handshake};

/// What `ls-remote` was asked for.
pub struct Options {
    /// A configured remote, URL or path; the default remote when `None`.
    pub remote: Option<String>,
    /// Tail patterns; all refs when empty.
    pub patterns: Vec<String>,
    /// `--heads`
    pub heads: bool,
    /// `--tags`
    pub tags: bool,
    /// `--refs`
    pub refs: bool,
}

/// Git's `tail_match()`: `pattern` matches `name` when `*/<pattern>` matches `/<name>`.
fn tail_matches(patterns: &[String], name: &str) -> bool {
    if patterns.is_empty() {
        return true;
    }
    let name: BString = format!("/{name}").into();
    patterns.iter().any(|pattern| {
        let pattern: BString = format!("*/{pattern}").into();
        gix::glob::wildmatch(pattern.as_ref(), name.as_ref(), gix::glob::wildmatch::Mode::empty())
    })
}

fn selected(options: &Options, name: &str) -> bool {
    let kind = (!options.heads && !options.tags)
        || (options.heads && name.starts_with("refs/heads/"))
        || (options.tags && name.starts_with("refs/tags/"));
    let refs_only = !options.refs || (name.starts_with("refs/") && !name.ends_with("^{}"));
    kind && refs_only && tail_matches(&options.patterns, name)
}

/// List the remote's refs like `git ls-remote`.
#[gix::protocol::bisync::bisync]
pub async fn ls_remote(
    repo: gix::Repository,
    mut progress: impl gix::Progress,
    mut out: impl std::io::Write,
    options: Options,
) -> Result<()> {
    let remote = crate::repository::remote::by_name_or_url(&repo, options.remote.as_deref())?;
    let (map, _handshake) = remote
        .connect(gix::remote::Direction::Fetch)
        .await?
        .ref_map(
            &mut progress,
            gix::remote::ref_map::Options {
                prefix_from_spec_as_filter_on_remote: false,
                ..Default::default()
            },
        )
        .await?;
    for remote_ref in &map.remote_refs {
        let (name, object, peeled) = match remote_ref {
            handshake::Ref::Direct { full_ref_name, object } => (full_ref_name, object, None),
            handshake::Ref::Peeled {
                full_ref_name,
                tag,
                object,
            } => (full_ref_name, tag, Some(object)),
            handshake::Ref::Symbolic {
                full_ref_name,
                tag,
                object,
                ..
            } => match tag {
                Some(tag) => (full_ref_name, tag, Some(object)),
                None => (full_ref_name, object, None),
            },
            handshake::Ref::Unborn { .. } => continue,
        };
        let name = name.to_string();
        if selected(&options, &name) {
            writeln!(out, "{object}\t{name}").or_error()?;
        }
        if let Some(peeled) = peeled {
            let peeled_name = format!("{name}^{{}}");
            if selected(&options, &peeled_name) {
                writeln!(out, "{peeled}\t{peeled_name}").or_error()?;
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::tail_matches;

    #[test]
    fn tail_patterns_match_like_git() {
        let p = |list: &[&str]| list.iter().map(|s| (*s).to_owned()).collect::<Vec<_>>();
        assert!(tail_matches(&p(&["dev"]), "refs/heads/dev"));
        assert!(tail_matches(&p(&["refs/heads/dev"]), "refs/heads/dev"));
        assert!(!tail_matches(&p(&["ev"]), "refs/heads/dev"));
        assert!(!tail_matches(&p(&["refs/heads/dev"]), "refs/heads/dev2"));
        assert!(tail_matches(&p(&["refs/heads/*"]), "refs/heads/feat/x"));
        assert!(tail_matches(
            &p(&["refs/heads/feat/steward-arrival*"]),
            "refs/heads/feat/steward-arrival-2"
        ));
        assert!(!tail_matches(&p(&["refs/tags/v1"]), "refs/tags/v1^{}"));
        assert!(tail_matches(&p(&["refs/tags/*"]), "refs/tags/v1^{}"));
    }
}
