//! Git's spellings of ref resolution and listing, each leading back to Brit's own verb.
//!
//! Agents guess Git's names first. These subcommands answer those names with Git's output
//! shapes for the forms in real use, and their help names the native Brit verb.

use clap::{Arg, ArgAction, ArgMatches, Command};
use gitoxide_core::repository::git_spellings::rev_parse;

/// `rev-parse`, parsed by hand so that path queries and revisions keep their command-line order.
#[derive(Debug)]
pub struct RevParse {
    pub args: Vec<rev_parse::Arg>,
    pub options: rev_parse::Options,
}

const REV: &str = "rev";
const PATHS: &str = "paths";
const SHOW_TOPLEVEL: &str = "show-toplevel";
const GIT_DIR: &str = "git-dir";
const ABSOLUTE_GIT_DIR: &str = "absolute-git-dir";
const GIT_COMMON_DIR: &str = "git-common-dir";
const PATH_FORMAT: &str = "path-format";
const SHORT: &str = "short";
const VERIFY: &str = "verify";
const QUIET: &str = "quiet";
const ABBREV_REF: &str = "abbrev-ref";

fn flag(id: &'static str, help: &'static str) -> Arg {
    Arg::new(id).long(id).action(ArgAction::Count).help(help)
}

impl clap::Args for RevParse {
    fn augment_args(cmd: Command) -> Command {
        cmd.arg(
            Arg::new(REV)
                .value_name("REV")
                .action(ArgAction::Append)
                .num_args(0..)
                .help("Revisions like `HEAD`, `origin/dev`, `HEAD^{tree}`, `HEAD:path` or `A..B`"),
        )
        .arg(
            Arg::new(PATHS)
                .value_name("PATH")
                .action(ArgAction::Append)
                .num_args(0..)
                .last(true)
                .help("Printed verbatim after `--`, as Git does"),
        )
        .arg(flag(SHOW_TOPLEVEL, "Print the absolute path of the worktree's top-level directory"))
        .arg(flag(
            GIT_DIR,
            "Print the repository's private Git directory: `.git` at a main worktree's top, else absolute",
        ))
        .arg(flag(ABSOLUTE_GIT_DIR, "Print the private Git directory as an absolute path"))
        .arg(flag(
            GIT_COMMON_DIR,
            "Print the Git directory shared by all worktrees; differs from --git-dir in a linked worktree",
        ))
        .arg(
            Arg::new(PATH_FORMAT)
                .long(PATH_FORMAT)
                .value_name("FORMAT")
                .value_parser(["absolute", "relative"])
                .action(ArgAction::Append)
                .help("Print the path queries that follow as absolute or cwd-relative paths"),
        )
        .arg(
            Arg::new(SHORT)
                .long(SHORT)
                .value_name("N")
                .num_args(0..=1)
                .require_equals(true)
                .default_missing_value("")
                .action(ArgAction::Set)
                .help("Print a unique abbreviation of at least N hex digits (core.abbrev when N is absent); implies --verify"),
        )
        .arg(flag(VERIFY, "Require exactly one revision and print its full object name"))
        .arg(
            Arg::new(QUIET)
                .long(QUIET)
                .short('q')
                .action(ArgAction::Count)
                .help("With --verify, print nothing on failure and exit with 1"),
        )
        .arg(flag(
            ABBREV_REF,
            "Print the shortest unambiguous ref name instead of the object name; non-refs print nothing",
        ))
    }

    fn augment_args_for_update(cmd: Command) -> Command {
        Self::augment_args(cmd)
    }
}

impl clap::FromArgMatches for RevParse {
    fn from_arg_matches(matches: &ArgMatches) -> Result<Self, clap::Error> {
        let mut ordered: Vec<(usize, rev_parse::Arg)> = Vec::new();
        let mut take = |id: &str, item: &dyn Fn(Option<&String>) -> rev_parse::Arg| {
            // Counted flags default to zero, and defaults have indices too: only typed arguments count.
            if matches.value_source(id) != Some(clap::parser::ValueSource::CommandLine) {
                return;
            }
            let values: Vec<Option<&String>> = matches
                .try_get_many::<String>(id)
                .ok()
                .flatten()
                .map(|values| values.map(Some).collect())
                .unwrap_or_default();
            for (position, index) in matches.indices_of(id).into_iter().flatten().enumerate() {
                ordered.push((index, item(values.get(position).copied().flatten())));
            }
        };
        take(REV, &|value| rev_parse::Arg::Rev(value.cloned().unwrap_or_default()));
        take(SHOW_TOPLEVEL, &|_| rev_parse::Arg::ShowToplevel);
        take(GIT_DIR, &|_| rev_parse::Arg::GitDir);
        take(ABSOLUTE_GIT_DIR, &|_| rev_parse::Arg::AbsoluteGitDir);
        take(GIT_COMMON_DIR, &|_| rev_parse::Arg::GitCommonDir);
        take(PATH_FORMAT, &|value| {
            rev_parse::Arg::PathFormat(if value.is_some_and(|v| v == "relative") {
                rev_parse::PathFormat::Relative
            } else {
                rev_parse::PathFormat::Absolute
            })
        });
        ordered.sort_by_key(|(index, _)| *index);
        let mut args: Vec<_> = ordered.into_iter().map(|(_, arg)| arg).collect();
        if let Some(paths) = matches.get_many::<String>(PATHS) {
            args.push(rev_parse::Arg::Paths(paths.cloned().collect()));
        }
        let short = match matches.get_one::<String>(SHORT) {
            None => None,
            Some(value) if value.is_empty() => Some(None),
            Some(value) => Some(Some(value.parse::<usize>().map_err(|_| {
                clap::Error::raw(
                    clap::error::ErrorKind::InvalidValue,
                    format!("--short expects a number of hex digits, got `{value}`\n"),
                )
            })?)),
        };
        Ok(RevParse {
            args,
            options: rev_parse::Options {
                verify: matches.get_count(VERIFY) > 0,
                quiet: matches.get_count(QUIET) > 0,
                abbrev_ref: matches.get_count(ABBREV_REF) > 0,
                short,
            },
        })
    }

    fn update_from_arg_matches(&mut self, matches: &ArgMatches) -> Result<(), clap::Error> {
        *self = Self::from_arg_matches(matches)?;
        Ok(())
    }
}

/// `for-each-ref` with Git's default format, `--format` atoms and `--sort` keys in real use.
#[derive(Debug, clap::Parser)]
pub struct ForEachRef {
    /// Interpolate `%(atom)` for each ref; implemented atoms: refname, refname:short, objectname,
    /// objectname:short, objecttype, subject, committerdate, committerdate:short, committerdate:iso.
    ///
    /// The default is Git's `%(objectname) %(objecttype)\t%(refname)`. An unimplemented atom is refused by name.
    #[clap(long)]
    pub format: Option<String>,
    /// Sort by `refname` (the default) or `committerdate`; prefix `-` for descending. Repeat to add keys, the last
    /// one primary, like Git.
    #[clap(long, value_name = "KEY", action = clap::ArgAction::Append)]
    pub sort: Vec<String>,
    /// Only refs matching these prefixes (at a `/` boundary) or `fnmatch` globs, like `refs/heads/` or
    /// `refs/heads/feat-*`.
    pub patterns: Vec<String>,
}

/// `ls-remote` printing Git's `<oid>\t<refname>` lines.
#[derive(Debug, clap::Parser)]
pub struct LsRemote {
    /// Only branches under `refs/heads/`.
    #[clap(long, visible_alias = "branches")]
    pub heads: bool,
    /// Only tags under `refs/tags/`.
    #[clap(long, short = 't')]
    pub tags: bool,
    /// Omit peeled tags (`^{}`) and pseudo-refs like `HEAD`.
    #[clap(long)]
    pub refs: bool,
    /// A configured remote, URL or path; the current branch's remote (else `origin`) when omitted.
    pub remote: Option<String>,
    /// Show only refs whose name ends with a pattern at a `/` boundary, like `dev`, `refs/heads/dev` or
    /// `refs/heads/feat/*`.
    pub patterns: Vec<String>,
}

/// `check-ref-format` answering through its exit code, like Git.
#[derive(Debug, clap::Parser)]
pub struct CheckRefFormat {
    /// Accept a name without a `/`, like `main` or `HEAD`.
    #[clap(long)]
    pub allow_onelevel: bool,
    /// The reference name to validate, like `refs/heads/main`.
    #[clap(value_parser = crate::shared::AsBString)]
    pub refname: gix::bstr::BString,
}
