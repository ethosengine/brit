use std::ffi::OsString;

use gix::bstr::{BString, ByteSlice};

use crate::OutputFormat;

/// git-compat `tag` listing: one shortened refname per line, sorted
/// lexicographically by refname. Matches git's default format
/// `%(refname:strip=2)` from `git tag` / `git tag -l` with no
/// `--format` override (see vendor/git/builtin/tag.c list_tags and
/// vendor/git/Documentation/git-tag.adoc OPTIONS/`--format`).
///
/// `patterns` are fnmatch(3)-style shell globs; a ref is shown if its
/// shortened name matches any pattern (OR), or unconditionally when
/// `patterns` is empty. Matches the `[<pattern>...]` positional of
/// `git tag -l`.
pub fn list(
    repo: gix::Repository,
    out: &mut dyn std::io::Write,
    format: OutputFormat,
    patterns: Vec<OsString>,
) -> anyhow::Result<()> {
    if format != OutputFormat::Human {
        anyhow::bail!("JSON output isn't supported");
    }

    let platform = repo.references()?;

    let mut names: Vec<BString> = platform
        .tags()?
        .flatten()
        .map(|reference| reference.name().shorten().to_owned())
        .collect();
    names.sort();

    let patterns: Vec<BString> = patterns
        .iter()
        .map(|p| gix::path::os_str_into_bstr(p).map(BString::from))
        .collect::<Result<_, _>>()?;

    for name in &names {
        if !patterns.is_empty()
            && !patterns
                .iter()
                .any(|pat| gix::glob::wildmatch(pat.as_ref(), name.as_ref(), gix::glob::wildmatch::Mode::empty()))
        {
            continue;
        }
        writeln!(out, "{name}", name = name.to_str_lossy())?;
    }

    Ok(())
}
