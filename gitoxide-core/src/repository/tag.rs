use gix::bstr::ByteSlice;

use crate::OutputFormat;

/// git-compat `tag` listing: one shortened refname per line, sorted
/// lexicographically by refname. Matches git's default format
/// `%(refname:strip=2)` from `git tag` / `git tag -l` with no
/// `--format` override (see vendor/git/builtin/tag.c list_tags and
/// vendor/git/Documentation/git-tag.adoc OPTIONS/`--format`).
pub fn list(repo: gix::Repository, out: &mut dyn std::io::Write, format: OutputFormat) -> anyhow::Result<()> {
    if format != OutputFormat::Human {
        anyhow::bail!("JSON output isn't supported");
    }

    let platform = repo.references()?;

    let mut names: Vec<gix::bstr::BString> = platform
        .tags()?
        .flatten()
        .map(|reference| reference.name().shorten().to_owned())
        .collect();
    names.sort();

    for name in &names {
        writeln!(out, "{name}", name = name.to_str_lossy())?;
    }

    Ok(())
}
