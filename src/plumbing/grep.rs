use std::io::Write;

use gitoxide_core::repository::{git_spellings, grep};
use gix::{
    Result,
    bstr::ByteSlice,
    error::{bail, message},
};

use super::options::grep::Platform;

pub(super) fn grep(repo: &gix::Repository, out: &mut dyn Write, args: Platform) -> Result<()> {
    let Platform {
        line_number,
        ignore_case,
        files_with_matches,
        fixed_strings,
        extended_regexp,
        mut patterns,
        positionals,
        mut paths,
    } = args;
    let revision = match positionals.as_slice() {
        [pattern, revision] if patterns.is_empty() => {
            patterns.push(pattern.clone());
            revision
        }
        [revision] if !patterns.is_empty() => revision,
        [revision] if !paths.is_empty() => {
            patterns.push(paths.remove(0));
            revision
        }
        _ => bail!(message("grep needs PATTERN REVISION or REVISION -- PATTERN").validation()),
    };
    let output = if files_with_matches {
        grep::Output::Names
    } else if line_number {
        grep::Output::NumberedLines
    } else {
        grep::Output::Lines
    };
    let _locale = posix::use_environment_locale();
    let patterns = patterns
        .iter()
        .flat_map(|pattern| pattern.split(|byte| *byte == b'\n'))
        .map(|pattern| Pattern::new(pattern, fixed_strings, extended_regexp, ignore_case))
        .collect::<Result<Vec<_>>>()?;
    let mut folded = Vec::new();
    let found = grep::grep(repo, out, revision.as_bstr(), &paths, output, |line| {
        let fixed_line = if fixed_strings && ignore_case {
            folded.clear();
            folded.extend(line.iter().map(u8::to_ascii_lowercase));
            &folded
        } else {
            line
        };
        for pattern in &patterns {
            let matches = match pattern {
                Pattern::Fixed(pattern) => pattern.find(fixed_line).is_some(),
                Pattern::Regex(pattern) => pattern.is_match(line)?,
            };
            if matches {
                return Ok(true);
            }
        }
        Ok(false)
    })?;
    if found { Ok(()) } else { Err(git_spellings::exit(1)) }
}

enum Pattern {
    Fixed(gix::bstr::Finder<'static>),
    Regex(posix::Regex),
}

impl Pattern {
    fn new(pattern: &[u8], fixed_strings: bool, extended_regexp: bool, ignore_case: bool) -> Result<Self> {
        if !fixed_strings {
            return posix::Regex::new(pattern, extended_regexp, ignore_case).map(Self::Regex);
        }
        if !ignore_case {
            return Ok(Self::Fixed(gix::bstr::Finder::new(pattern).into_owned()));
        }
        if pattern.is_ascii() {
            return Ok(Self::Fixed(
                gix::bstr::Finder::new(&pattern.to_ascii_lowercase()).into_owned(),
            ));
        }

        // Git uses regular expressions for non-ASCII case-insensitive literals, where
        // case folding depends on the locale. Escape metacharacters before compilation.
        let mut literal = Vec::with_capacity(pattern.len());
        for &byte in pattern {
            if b".^$[](){}*+?|\\".contains(&byte) {
                literal.push(b'\\');
            }
            literal.push(byte);
        }
        posix::Regex::new(&literal, true, true).map(Self::Regex)
    }
}

#[cfg(any(
    all(target_os = "linux", target_env = "gnu"),
    target_os = "android",
    target_vendor = "apple",
    target_os = "freebsd",
    target_os = "netbsd",
    target_os = "openbsd",
    target_os = "dragonfly"
))]
#[path = "grep_posix.rs"]
mod posix;

#[cfg(not(any(
    all(target_os = "linux", target_env = "gnu"),
    target_os = "android",
    target_vendor = "apple",
    target_os = "freebsd",
    target_os = "netbsd",
    target_os = "openbsd",
    target_os = "dragonfly"
)))]
#[path = "grep_portable.rs"]
mod posix;
