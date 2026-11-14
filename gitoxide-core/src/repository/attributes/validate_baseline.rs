use crate::OutputFormat;

pub struct Options {
    pub format: OutputFormat,
    pub statistics: bool,
    pub ignore: bool,
}

pub(crate) mod function {
    use std::{
        collections::BTreeSet,
        io,
        io::{BufRead, Seek, Write},
        ops::Sub,
        path::PathBuf,
        sync::atomic::Ordering,
    };

    use gix::{
        Count, Progress, Result,
        attrs::{Assignment, NameRef},
        bstr::{BString, ByteSlice},
        error::{OptionExt, ResultExt, bail, message},
    };

    use crate::{
        OutputFormat,
        repository::attributes::{query::attributes_cache, validate_baseline::Options},
    };

    pub fn validate_baseline(
        repo: gix::Repository,
        paths: Option<impl Iterator<Item = io::Result<BString>> + Send + 'static>,
        mut progress: impl gix::NestedProgress + 'static,
        mut out: impl io::Write,
        mut err: impl io::Write,
        Options {
            format,
            statistics,
            mut ignore,
        }: Options,
    ) -> Result<()> {
        if format != OutputFormat::Human {
            bail!(gix::error::unsupported("JSON output isn't implemented yet"));
        }

        if repo.is_bare() {
            writeln!(
                err,
                "Repo at \"{repo}\" is bare - disabling git-ignore baseline as `git check-ignore` needs a worktree",
                repo = repo.path().display()
            )
            .or_error()?;
            ignore = false;
        }
        let paths = paths.map_or_else(
            {
                let repo = repo.clone();
                move || -> Result<_> {
                    let index = repo.index_or_load_from_head()?.into_owned();
                    let (entries, path_backing) = index.into_parts().0.into_entries();
                    let iter = Box::new(
                        entries
                            .into_iter()
                            .map(move |e| Ok(e.path_in(&path_backing).to_owned())),
                    );
                    Ok(iter as Box<dyn Iterator<Item = io::Result<BString>> + Send + 'static>)
                }
            },
            |paths| Result::Ok(Box::new(paths)),
        )?;
        let interrupt = &gix::interrupt::IS_INTERRUPTED;
        let budget = gix::utils::stream::Budget::new(&std::env::temp_dir()).or_error()?;
        let mut input = budget.spool().or_error()?;
        let mut num_entries = 0;
        let mut previous = None;
        for path in paths {
            gix::utils::stream::check(interrupt).or_error()?;
            let path = path.or_raise(|| message("Could not read baseline input path"))?;
            // Repeated adjacent queries have the same answer. Coalescing them also
            // bounds the helper's attribute group to one path's assignments.
            if previous.as_ref() == Some(&path) {
                continue;
            }
            input.write_all(&path).or_error()?;
            input.write_all(b"\0").or_error()?;
            previous = Some(path);
            num_entries += 1;
        }
        drop(previous);
        let input = input.into_named_input();
        let work_dir = if ignore {
            repo.workdir().ok_or_raise(|| {
                message!("repository at {:?} must have a worktree checkout", repo.path()).unsupported()
            })?
        } else {
            repo.git_dir()
        };
        let (mut cache, _index) = attributes_cache(&repo)?;
        let mut matches = cache.attribute_matches();
        let mut progress = progress.add_child("validate");
        let mut mismatches = budget.spool().or_error()?;
        let mut num_mismatches = 0;
        let start = std::time::Instant::now();
        progress.init(
            Some(num_entries * if ignore { 2 } else { 1 }),
            gix::progress::count("paths"),
        );

        let helpers: [(&str, &[&str], &std::path::Path); 2] = [
            (
                "attributes",
                &["check-attr", "--stdin", "-z", "-a"],
                repo.workdir().unwrap_or(repo.path()),
            ),
            (
                "excludes",
                &["check-ignore", "--stdin", "-z", "-nv", "--no-index"],
                work_dir,
            ),
        ];
        for (name, args, cwd) in helpers.into_iter().take(if ignore { 2 } else { 1 }) {
            let mut command = std::process::Command::try_from(gix::command::prepare(gix::path::env::exe_invocation()))?;
            command
                .args(args)
                .current_dir(cwd)
                .stdin(std::process::Stdio::from(std::fs::File::open(input.path()).or_error()?));
            let mut output = gix::utils::stream::command_output(&mut command, &budget, interrupt)
                .or_raise(|| message!("Could not run {name} baseline helper"))?;
            // check-ignore uses exit 1 for a successful query with no matching paths.
            if !(output.status.success() || name == "excludes" && output.status.code() == Some(1)) {
                gix::utils::stream::copy(&mut output.stderr, &mut err, interrupt).or_error()?;
                bail!("{name} baseline helper failed with {}", output.status);
            }
            drop(output.stderr);
            let mut reader = io::BufReader::new(output.stdout);
            let mut records = std::iter::from_fn(|| {
                if name == "attributes" {
                    parse_attribute(&mut reader).transpose()
                } else {
                    parse_exclude(&mut reader).transpose()
                }
            })
            .peekable();
            let mut paths = io::BufReader::new(std::fs::File::open(input.path()).or_error()?);
            while let Some(rela_path) = read_field(&mut paths)? {
                gix::utils::stream::check(interrupt).or_error()?;
                let baseline = if name == "attributes" {
                    let mut assignments = Vec::new();
                    loop {
                        match records.peek() {
                            Some(Err(_)) => {
                                return Err(records.next().expect("peeked record").err().expect("peeked error"));
                            }
                            Some(Ok((path, _))) if path.as_slice() == rela_path => {
                                let (_, Baseline::Attribute { assignments: next }) =
                                    records.next().expect("peeked record")?
                                else {
                                    unreachable!("attribute parser produces attribute records")
                                };
                                assignments.extend(next);
                            }
                            _ => break,
                        }
                    }
                    Baseline::Attribute { assignments }
                } else {
                    let (path, baseline) = records
                        .next()
                        .ok_or_raise(|| message("Missing exclude baseline record").corrupted())??;
                    if path.as_slice() != rela_path {
                        bail!(message("Unexpected path in exclude baseline record").corrupted());
                    }
                    baseline
                };
                let entry = cache.at_entry(rela_path.as_bstr(), None)?;
                let mismatch = match baseline {
                    Baseline::Attribute { assignments: expected } => {
                        entry.matching_attributes(&mut matches);
                        let fast_path_mismatch = matches.iter().count() != expected.len()
                            || matches
                                .iter()
                                .map(|m| m.assignment)
                                .zip(expected.iter().map(ThreadSafeAssignment::as_ref))
                                .any(|(a, b)| a != b);
                        if fast_path_mismatch {
                            let actual_set = matches.iter().map(|m| m.assignment).collect::<BTreeSet<_>>();
                            let expected_set = expected
                                .iter()
                                .map(ThreadSafeAssignment::as_ref)
                                .collect::<BTreeSet<_>>();
                            let different =
                                !(expected_set.sub(&actual_set).is_empty() && actual_set.sub(&expected_set).is_empty());
                            different.then(|| Mismatch::Attributes {
                                actual: matches.iter().map(|m| m.assignment.to_owned()).collect(),
                                expected: expected.into_iter().map(Into::into).collect(),
                            })
                        } else {
                            None
                        }
                    }
                    Baseline::Exclude { location } => {
                        let match_ = entry.matching_exclude_pattern();
                        (match_.is_some() != location.is_some()).then(|| Mismatch::Exclude {
                            actual: match_.map(Into::into),
                            expected: location,
                        })
                    }
                };
                if let Some(mismatch) = mismatch {
                    writeln!(mismatches, "{}: {mismatch:#?}", rela_path.as_bstr()).or_error()?;
                    num_mismatches += 1;
                }
                progress.inc();
            }
            if let Some(record) = records.next() {
                let (path, _) = record?;
                bail!(message!("Unexpected {name} baseline record for {path}").corrupted());
            }
        }
        if let Some(stats) = statistics.then(|| cache.take_statistics()) {
            out.flush().or_error()?;
            writeln!(err, "{stats:#?}").or_error()?;
        }
        progress.show_throughput(start);
        if num_mismatches == 0 {
            Ok(())
        } else {
            mismatches.rewind().or_error()?;
            gix::utils::stream::copy(&mut mismatches, &mut err, interrupt).or_error()?;
            bail!(
                "{}: Validation failed with {} mismatches out of {}",
                gix::path::realpath(repo.workdir().unwrap_or(repo.git_dir()))?.display(),
                num_mismatches,
                progress.counter().load(Ordering::Relaxed)
            );
        }
    }

    enum Baseline {
        Attribute { assignments: Vec<ThreadSafeAssignment> },
        Exclude { location: Option<ExcludeLocation> },
    }

    struct ThreadSafeAssignment {
        name: String,
        state: gix::attrs::State,
    }

    impl ThreadSafeAssignment {
        fn as_ref(&self) -> gix::attrs::AssignmentRef<'_> {
            gix::attrs::AssignmentRef {
                name: NameRef::try_from(self.name.as_bytes().as_bstr())
                    .expect("names from git check-attr were validated while parsing"),
                state: self.state.as_ref(),
            }
        }
    }

    impl From<ThreadSafeAssignment> for Assignment {
        fn from(value: ThreadSafeAssignment) -> Self {
            Assignment {
                name: NameRef::try_from(value.name.as_bytes().as_bstr())
                    .expect("names from git check-attr were validated while parsing")
                    .to_owned(),
                state: value.state,
            }
        }
    }

    #[derive(Debug)]
    // See note on `Mismatch`
    #[expect(dead_code)]
    pub struct ExcludeLocation {
        pub line: usize,
        pub rela_source_file: BString,
        pub pattern: BString,
    }

    #[derive(Debug)]
    // We debug-print this structure, which makes all fields 'used', but it doesn't count.
    // TODO: find a way to not have to do more work, but make the warning go away.
    #[expect(dead_code)]
    pub enum Mismatch {
        Attributes {
            actual: Vec<gix::attrs::Assignment>,
            expected: Vec<gix::attrs::Assignment>,
        },
        Exclude {
            actual: Option<ExcludeMatch>,
            expected: Option<ExcludeLocation>,
        },
    }

    #[derive(Debug)]
    // See note on `Mismatch`
    #[expect(dead_code)]
    pub struct ExcludeMatch {
        pub pattern: gix::glob::Pattern,
        pub source: Option<PathBuf>,
        pub sequence_number: usize,
    }

    impl From<gix::ignore::search::Match<'_>> for ExcludeMatch {
        fn from(value: gix::ignore::search::Match<'_>) -> Self {
            ExcludeMatch {
                pattern: value.pattern.clone(),
                source: value.source.map(ToOwned::to_owned),
                sequence_number: value.sequence_number,
            }
        }
    }

    fn read_field(reader: &mut impl BufRead) -> Result<Option<Vec<u8>>> {
        gix::utils::stream::check(&gix::interrupt::IS_INTERRUPTED).or_error()?;
        let mut field = Vec::new();
        if reader.read_until(0, &mut field).or_error()? == 0 {
            return Ok(None);
        }
        if field.pop() != Some(0) {
            bail!(message("Unterminated baseline field").corrupted());
        }
        Ok(Some(field))
    }

    fn required_field(reader: &mut impl BufRead) -> Result<Vec<u8>> {
        read_field(reader)?.ok_or_raise(|| message("Missing baseline field").corrupted())
    }

    fn parse_exclude(reader: &mut impl BufRead) -> Result<Option<(BString, Baseline)>> {
        let Some(source) = read_field(reader)? else {
            return Ok(None);
        };
        let line = required_field(reader)?;
        let pattern = required_field(reader)?;
        let path = required_field(reader)?;
        let location = if source.is_empty() && line.is_empty() && pattern.is_empty() {
            None
        } else {
            Some(ExcludeLocation {
                line: std::str::from_utf8(&line).or_error()?.parse::<usize>().or_error()?,
                rela_source_file: source.into(),
                pattern: pattern.into(),
            })
        };
        Ok(Some((path.into(), Baseline::Exclude { location })))
    }

    fn parse_attribute(reader: &mut impl BufRead) -> Result<Option<(BString, Baseline)>> {
        use gix::attrs::StateRef;

        let Some(path) = read_field(reader)? else {
            return Ok(None);
        };
        let name = required_field(reader)?;
        let info = required_field(reader)?;
        let state = match info.as_slice() {
            b"set" => StateRef::Set,
            b"unset" => StateRef::Unset,
            b"unspecified" => StateRef::Unspecified,
            _ => StateRef::from_bytes(&info),
        };
        let assignment = ThreadSafeAssignment {
            name: NameRef::try_from(name.as_bstr())?.as_str().to_owned(),
            state: state.to_owned(),
        };
        Ok(Some((
            path.into(),
            Baseline::Attribute {
                assignments: vec![assignment],
            },
        )))
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use std::io::Read;

        struct ReadFailure;
        impl Read for ReadFailure {
            fn read(&mut self, _: &mut [u8]) -> io::Result<usize> {
                Err(io::Error::other("synthetic read failure"))
            }
        }

        #[test]
        fn records_preserve_read_errors_and_reject_malformed_input() -> Result<()> {
            let mut reader = io::BufReader::new(io::Cursor::new(b"selected\0custom\0value\0").chain(ReadFailure));
            assert!(parse_attribute(&mut reader)?.is_some());
            assert!(parse_attribute(&mut reader).is_err());
            for mut malformed in [b"malformed".as_slice(), b"path\0name\0", b"path\0name\0value"] {
                assert!(parse_attribute(&mut malformed).is_err());
            }
            let mut input = b"a: path\xff\0custom\0value\0b\0extra\0set\0".as_slice();
            let (path, Baseline::Attribute { assignments }) = parse_attribute(&mut input)?.expect("first record")
            else {
                panic!("attribute record")
            };
            assert_eq!(path, b"a: path\xff".as_bstr());
            assert_eq!(assignments.len(), 1);
            assert_eq!(parse_attribute(&mut input)?.expect("second record").0, "b");
            assert!(parse_attribute(&mut input)?.is_none());
            Ok(())
        }
    }
}
