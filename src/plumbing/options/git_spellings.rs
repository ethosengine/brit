//! Git's spellings of ref resolution and listing, each leading back to Brit's own verb.
//!
//! Agents guess Git's names first. These subcommands answer those names with Git's output
//! shapes for the forms in real use, and their help names the native Brit verb.

use clap::{Arg, ArgAction, ArgMatches, Command};

/// `rev-parse`, parsed by hand so that flags, path queries and revisions keep their command-line order.
///
/// Flags before the first revision are parsed by clap; from the first revision on, every argument
/// (flags and `--` included) is taken verbatim, and all of them are read in order the way Git reads
/// its argv, so `HEAD --short HEAD` prints a full and then a short object name.
#[derive(Debug)]
pub struct RevParse {
    /// The arguments in command-line order, with each flag in its canonical spelling.
    pub args: Vec<String>,
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

/// A flag that records every occurrence with its position; a counted flag keeps only the last one.
fn flag(id: &'static str, help: &'static str) -> Arg {
    Arg::new(id)
        .long(id)
        .action(ArgAction::Append)
        .num_args(0)
        .default_missing_value("")
        .help(help)
}

impl clap::Args for RevParse {
    fn augment_args(cmd: Command) -> Command {
        cmd.arg(
            Arg::new(REV)
                .value_name("REV")
                .action(ArgAction::Append)
                .num_args(0..)
                .allow_hyphen_values(true)
                .help("Revisions like `HEAD`, `origin/dev`, `HEAD^{tree}`, `HEAD:path`, `A..B` or `A...B`; flags after the first one apply from their position, as in Git"),
        )
        .arg(
            Arg::new(PATHS)
                .value_name("PATH")
                .action(ArgAction::Append)
                .num_args(0..)
                .last(true)
                .help("Printed verbatim after `--`, as Git does, unless --verify or --short came first"),
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
                .allow_hyphen_values(true)
                // A NUL cannot arrive in argv, so it tells `--short` from an explicit empty `--short=`.
                .default_missing_value("\0")
                .action(ArgAction::Append)
                .help("Print a unique abbreviation of at least N hex digits (core.abbrev when N is absent, at least 4); implies --verify"),
        )
        .arg(flag(VERIFY, "Require exactly one revision and print only its object name"))
        .arg(
            flag(QUIET, "With --verify, print nothing on failure and exit with 1").short('q'),
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
        let mut ordered: Vec<(usize, String)> = Vec::new();
        let mut take = |id: &str, token: &dyn Fn(&str) -> String| {
            if matches.value_source(id) != Some(clap::parser::ValueSource::CommandLine) {
                return;
            }
            let values: Vec<&String> = matches.get_many::<String>(id).into_iter().flatten().collect();
            for (value, index) in values.into_iter().zip(matches.indices_of(id).into_iter().flatten()) {
                ordered.push((index, token(value)));
            }
        };
        take(REV, &str::to_owned);
        for id in [
            SHOW_TOPLEVEL,
            GIT_DIR,
            ABSOLUTE_GIT_DIR,
            GIT_COMMON_DIR,
            VERIFY,
            ABBREV_REF,
        ] {
            take(id, &|_| format!("--{id}"));
        }
        take(QUIET, &|_| "--quiet".to_owned());
        take(PATH_FORMAT, &|value| format!("--path-format={value}"));
        take(SHORT, &|value| {
            if value == "\0" {
                "--short".to_owned()
            } else {
                format!("--short={value}")
            }
        });
        ordered.sort_by_key(|(index, _)| *index);
        let mut args: Vec<_> = ordered.into_iter().map(|(_, arg)| arg).collect();
        if let Some(paths) = matches.get_many::<String>(PATHS) {
            args.push("--".to_owned());
            args.extend(paths.cloned());
        }
        Ok(RevParse { args })
    }

    fn update_from_arg_matches(&mut self, matches: &ArgMatches) -> Result<(), clap::Error> {
        *self = Self::from_arg_matches(matches)?;
        Ok(())
    }
}

/// `for-each-ref` with Git's default format, `--format` atoms and `--sort` keys in real use.
///
/// A ref name that is not UTF-8 is shortened lossily by `%(refname:short)`, and `refname:short`
/// always shortens strictly, as if `core.warnAmbiguousRefs` were true.
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
    #[clap(long, value_name = "KEY", action = clap::ArgAction::Append, allow_hyphen_values = true)]
    pub sort: Vec<String>,
    /// Only refs matching these prefixes (at a `/` boundary) or `fnmatch` globs, like `refs/heads/` or
    /// `refs/heads/feat-*`.
    pub patterns: Vec<String>,
}

/// `ls-remote` printing Git's `<oid>\t<refname>` lines.
///
/// `-h` means `--heads`, as in Git; help is `--help` only.
#[derive(Debug, clap::Parser)]
#[command(disable_help_flag = true)]
pub struct LsRemote {
    /// Print help.
    #[clap(long, action = clap::ArgAction::Help)]
    pub help: Option<bool>,
    /// Only branches under `refs/heads/`; `-h` like Git.
    #[clap(long, short = 'h', visible_alias = "branches", short_alias = 'b')]
    pub heads: bool,
    /// Only tags under `refs/tags/`.
    #[clap(long, short = 't')]
    pub tags: bool,
    /// Omit peeled tags (`^{}`) and pseudo-refs like `HEAD`.
    #[clap(long)]
    pub refs: bool,
    /// Exit with status 2 when no ref matched.
    #[clap(long)]
    pub exit_code: bool,
    /// Do not print `From <url>` on stderr when the remote is omitted.
    #[clap(long, short = 'q')]
    pub quiet: bool,
    /// A configured remote, URL or path; the current branch's remote (else `origin`) when omitted.
    pub remote: Option<String>,
    /// Show only refs whose name ends with a pattern at a `/` boundary, like `dev`, `refs/heads/dev` or
    /// `refs/heads/feat/*`.
    pub patterns: Vec<String>,
}

/// `check-ref-format` answering through its exit code, like Git.
///
/// A missing name, or an argument starting with `-` in its place, is a usage error with Git's status 129.
#[derive(Debug, clap::Parser)]
pub struct CheckRefFormat {
    /// Accept a name without a `/`, like `main` or `HEAD`.
    #[clap(long)]
    pub allow_onelevel: bool,
    /// The reference name to validate, like `refs/heads/main`.
    #[clap(value_parser = crate::shared::AsBString, allow_hyphen_values = true)]
    pub refname: Option<gix::bstr::BString>,
}
