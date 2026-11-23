use gix::{
    Result,
    bstr::{BString, ByteSlice as _},
    config::AsKey,
    error::{OptionExt, ResultExt, bail, message, unsupported, validation},
};
use std::io::Write as _;

use crate::OutputFormat;

/// List all files which contributed sections to the resolved configuration, in precedence order.
pub fn list_files(
    mut repo: gix::Repository,
    overrides: Vec<BString>,
    format: OutputFormat,
    mut out: impl std::io::Write,
) -> Result<()> {
    if format != OutputFormat::Human {
        bail!(unsupported("Only human output format is supported at the moment"));
    }
    repo.reload_with(|options| options.cli_overrides(overrides))?;
    let config = repo.config_snapshot();
    let mut seen = std::collections::BTreeSet::new();
    for meta in config.sections().map(|section| section.meta()).chain([config.meta()]) {
        let Some(path) = meta.path.as_ref() else {
            continue;
        };
        if seen.insert(path) {
            if meta.level == 0 {
                writeln!(out, "{}\t{{ source={:?} }}", path.display(), meta.source).or_error()?;
            } else {
                writeln!(
                    out,
                    "{}\t{{ source={:?}, include-level={} }}",
                    path.display(),
                    meta.source,
                    meta.level
                )
                .or_error()?;
            }
        }
    }
    Ok(())
}

pub fn show(
    mut repo: gix::Repository,
    filters: Vec<BString>,
    overrides: Vec<BString>,
    format: OutputFormat,
    mut out: impl std::io::Write,
) -> Result<()> {
    if format != OutputFormat::Human {
        bail!(unsupported("Only human output format is supported at the moment"));
    }
    repo.reload_with(|options| options.cli_overrides(overrides))?;
    let config = repo.config_snapshot();
    if let Some(frontmatter) = config.frontmatter() {
        for event in frontmatter {
            event.write_to(&mut out).or_error()?;
        }
    }
    let filters: Vec<_> = filters.into_iter().map(Filter::new).collect();
    let mut last_meta = None;
    let mut it = config.sections_and_postmatter().peekable();
    while let Some((section, matter)) = it.next() {
        if !filters.is_empty() && !filters.iter().any(|filter| filter.matches_section(&section)) {
            continue;
        }

        let meta = section.meta();
        if last_meta != Some(meta) {
            write_meta(meta, &mut out).or_error()?;
        }
        last_meta = Some(meta);

        section.write_to(&mut out).or_error()?;
        for event in matter {
            event.write_to(&mut out).or_error()?;
        }
        if it
            .peek()
            .is_some_and(|(next_section, _)| next_section.header().name() != section.header().name())
        {
            writeln!(&mut out).or_error()?;
        }
    }
    Ok(())
}

/// Format the git configuration file at `in_file`, or the repository-local configuration if `in_file`
/// is `None`, writing the result back in place, to `out_file`, or to `out` (stdout) respectively.
pub fn fmt(
    repo: Option<gix::Repository>,
    in_file: Option<std::path::PathBuf>,
    out_file: Option<std::path::PathBuf>,
    in_place: bool,
    mut out: impl std::io::Write,
) -> Result<()> {
    if in_place && out_file.is_some() {
        bail!(validation("Cannot combine --in-place with an explicit output file"));
    }
    let source = match in_file {
        Some(path) => path,
        None => repo
            .ok_or_raise(|| validation("Formatting the repository-local configuration requires being in a repository"))?
            .common_dir()
            .join("config"),
    };
    let lock = in_place
        .then(|| {
            gix::lock::File::acquire_to_update_resource(&source, gix::lock::acquire::Fail::Immediately, None, 0)
                .or_raise(|| message!("Could not lock configuration file at \"{}\"", source.display()))
        })
        .transpose()?;
    let input = std::fs::read(&source)
        .or_raise(|| message!("Could not read configuration file at \"{}\"", source.display()))?;
    let formatted = gix::config::format::normalize(&input, Default::default()).or_error()?;
    match (lock, out_file) {
        (Some(mut lock), _) => {
            lock.write_all(&formatted).or_raise(|| {
                message!(
                    "Could not write formatted configuration to \"{}.lock\"",
                    source.display()
                )
            })?;
            lock.commit()
                .map_err(|err| err.error)
                .or_raise(|| message!("Could not commit formatted configuration to \"{}\"", source.display()))?;
        }
        (None, Some(path)) => std::fs::write(&path, &formatted)
            .or_raise(|| message!("Could not write formatted configuration to \"{}\"", path.display()))?,
        (None, None) => out.write_all(&formatted).or_error()?,
    }
    Ok(())
}

/// Rewrite the repository-local configuration under its lock with `edit`, if `edit` changed anything.
///
/// An error from `edit` leaves the file untouched: the lock is dropped without being committed.
pub(crate) fn edit_local_config(
    repo: &gix::Repository,
    edit: impl FnOnce(&mut gix::config::File) -> Result<bool>,
) -> Result<()> {
    let path = repo.common_dir().join("config");
    let mut lock = gix::lock::File::acquire_to_update_resource(&path, gix::lock::acquire::Fail::Immediately, None, 0)
        .or_raise(|| message!("Could not lock configuration file at \"{}\"", path.display()))?;
    let input =
        std::fs::read(&path).or_raise(|| message!("Could not read configuration file at \"{}\"", path.display()))?;
    // Includes stay unresolved: only this file's own sections may be edited and written back.
    let mut config = gix::config::File::from_bytes_no_includes(
        &input,
        gix::config::file::Metadata::from(gix::config::Source::Local),
        Default::default(),
    )?;
    if !edit(&mut config)? {
        return Ok(());
    }
    let mut bytes = Vec::with_capacity(input.len());
    config.write_to(&mut bytes).or_error()?;
    lock.write_all(&bytes).or_error()?;
    lock.commit()
        .map_err(|err| err.error)
        .or_raise(|| message!("Could not commit configuration file at \"{}\"", path.display()))?;
    Ok(())
}

/// A key split as Git splits it: the section before the first dot, the name after the last,
/// and everything between (dots and slashes included) the subsection.
fn parse_key<'a>(key: &'a str, err: &mut impl std::io::Write) -> Result<gix::config::KeyRef<'a>> {
    use crate::repository::git_spellings::exit;
    let Some(parsed) = gix::config::KeyRef::parse_unvalidated(key.into()) else {
        writeln!(err, "error: key does not contain a section: {key}").ok();
        return Err(exit(2));
    };
    if parsed.section_name.is_empty() || parsed.value_name.is_empty() {
        writeln!(err, "error: invalid key: {key}").ok();
        return Err(exit(1));
    }
    Ok(parsed)
}

/// The sections of `config` holding `key`, once per value they hold for it.
fn holders(config: &gix::config::File, key: &gix::config::KeyRef<'_>) -> Vec<gix::config::file::SectionId> {
    config
        .sections_and_ids_by_name(key.section_name)
        .into_iter()
        .flatten()
        .filter(|(section, _)| section.header().subsection_name() == key.subsection_name)
        .flat_map(|(section, id)| std::iter::repeat_n(id, section.values(key.value_name).len()))
        .collect()
}

/// Set `key` to `value` in the repository-local configuration, as `git config <key> <value>` does:
/// an existing single value is replaced where it is, a missing one is added to the last matching
/// section or a new one, and a key with several values is refused with Git's status 5.
pub fn set(repo: gix::Repository, key: &str, value: &gix::bstr::BStr, mut err: impl std::io::Write) -> Result<()> {
    use crate::repository::git_spellings::exit;
    let parsed = parse_key(key, &mut err)?;
    edit_local_config(&repo, |config| {
        let mut section = match holders(config, &parsed).as_slice() {
            [] => config.section_mut_or_create_new(parsed.section_name, parsed.subsection_name)?,
            [id] => config.section_mut_by_id(*id).expect("id was just looked up"),
            _ => {
                writeln!(err, "warning: {key} has multiple values").ok();
                writeln!(err, "error: cannot overwrite multiple values with a single value").ok();
                return Err(exit(5));
            }
        };
        section.set(parsed.value_name, value)?;
        Ok(true)
    })
}

/// Remove `key` from the repository-local configuration, as `git config --unset <key>` does,
/// dropping its section when nothing else is left in it. A missing key, or one with several values,
/// is refused with Git's status 5 and the file is left as it was.
pub fn unset(repo: gix::Repository, key: &str, mut err: impl std::io::Write) -> Result<()> {
    use crate::repository::git_spellings::exit;
    let parsed = parse_key(key, &mut err)?;
    edit_local_config(&repo, |config| {
        let id = match holders(config, &parsed).as_slice() {
            [id] => *id,
            [] => {
                writeln!(err, "error: {key} is not set in the repository-local configuration").ok();
                return Err(exit(5));
            }
            _ => {
                writeln!(err, "warning: {key} has multiple values").ok();
                return Err(exit(5));
            }
        };
        let mut section = config.section_mut_by_id(id).expect("id was just looked up");
        section.remove(parsed.value_name);
        let text = section.to_bstring();
        // Like Git, a section left without entries goes too, unless it still holds a comment.
        if section.num_values() == 0 && text.lines().skip(1).all(|line| line.trim().is_empty()) {
            config.remove_section_by_id(id);
        }
        Ok(true)
    })
}

struct Filter {
    name: String,
    subsection: Option<BString>,
}

impl Filter {
    fn new(input: BString) -> Self {
        match (&input).try_as_key() {
            Some(key) => Filter {
                name: key.section_name.into(),
                subsection: key.subsection_name.map(ToOwned::to_owned),
            },
            None => Filter {
                name: input.to_string(),
                subsection: None,
            },
        }
    }

    fn matches_section(&self, section: &gix::config::file::SectionRef<'_>) -> bool {
        let ignore_case = gix::glob::wildmatch::Mode::IGNORE_CASE;

        if !gix::glob::wildmatch(self.name.as_bytes().into(), section.header().name(), ignore_case) {
            return false;
        }
        match (self.subsection.as_deref(), section.header().subsection_name()) {
            (Some(filter), Some(name)) => {
                if !gix::glob::wildmatch(filter.as_slice().into(), name, ignore_case) {
                    return false;
                }
            }
            (None, _) => {}
            (Some(_), None) => return false,
        }
        true
    }
}

fn write_meta(meta: &gix::config::file::Metadata, out: &mut impl std::io::Write) -> std::io::Result<()> {
    writeln!(
        out,
        "# From \"{}\" ({:?}{}{})",
        meta.path
            .as_deref()
            .map_or_else(|| "memory".into(), |p| p.display().to_string()),
        meta.source,
        if meta.level != 0 {
            format!(", include level {}", meta.level)
        } else {
            Default::default()
        },
        if meta.trust != gix::sec::Trust::Full {
            ", untrusted"
        } else {
            Default::default()
        }
    )
}
