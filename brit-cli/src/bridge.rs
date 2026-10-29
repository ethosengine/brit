//! Named Git bridges: operations `brit` does not own and never performs.
//!
//! Brit coexists with stock Git. For a bridged name it prints the exact `git`
//! command to run and exits; there is no fallback, no alias and no attempt to
//! run Git on the caller's behalf. The table is consulted before clap parses.

use std::ffi::OsString;

/// Exit code of every bridge refusal; the same code clap and `CliError::Args` use for usage errors.
pub const EXIT_CODE: i32 = 2;

/// Top-level subcommands that Git owns and brit bridges.
pub const BRIDGED: &[&str] = &[
    "reset",
    "rm",
    "mv",
    "show",
    "stash",
    "cherry-pick",
    "revert",
    "checkout",
    "notes",
];

/// `submodule` is owned by brit (`submodule list`); only these verbs are bridged.
pub const BRIDGED_SUBMODULE: &[&str] = &["update", "init", "sync"];

/// Return the line to print if `args` (including the program name) invoke a bridged operation.
pub fn line(args: &[OsString]) -> Option<String> {
    let name = args.get(1)?.to_str()?;
    let (name, rest) = if name == "submodule" {
        let verb = args.get(2)?.to_str()?;
        if !BRIDGED_SUBMODULE.contains(&verb) {
            return None;
        }
        (format!("submodule {verb}"), &args[3..])
    } else if BRIDGED.contains(&name) {
        (name.to_owned(), &args[2..])
    } else {
        return None;
    };
    let mut line = format!("brit: {name} is a Git bridge operation; run: git {name}");
    for arg in rest {
        line.push(' ');
        line.push_str(&shell_quote(&arg.to_string_lossy()));
    }
    Some(line)
}

fn shell_quote(arg: &str) -> String {
    let plain = !arg.is_empty()
        && arg
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "-_./=:@%+,^~".contains(c));
    if plain {
        arg.to_owned()
    } else {
        format!("'{}'", arg.replace('\'', r"'\''"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(list: &[&str]) -> Vec<OsString> {
        list.iter().map(OsString::from).collect()
    }

    #[test]
    fn bridged_names_print_the_same_arguments_for_git() {
        assert_eq!(
            line(&args(&["brit", "reset", "--hard", "HEAD~1"])).as_deref(),
            Some("brit: reset is a Git bridge operation; run: git reset --hard HEAD~1")
        );
        assert_eq!(
            line(&args(&["brit", "stash"])).as_deref(),
            Some("brit: stash is a Git bridge operation; run: git stash")
        );
        assert_eq!(
            line(&args(&["brit", "submodule", "update", "--init", "--recursive"])).as_deref(),
            Some("brit: submodule update is a Git bridge operation; run: git submodule update --init --recursive")
        );
    }

    #[test]
    fn arguments_needing_quotes_are_quoted_for_a_shell() {
        assert_eq!(
            line(&args(&["brit", "show", "a b", "it's", ""])).as_deref(),
            Some(r"brit: show is a Git bridge operation; run: git show 'a b' 'it'\''s' ''")
        );
    }

    #[test]
    fn owned_and_unknown_names_are_not_bridged() {
        for owned in [
            &["brit"][..],
            &["brit", "status"],
            &["brit", "submodule"],
            &["brit", "submodule", "list"],
            &["brit", "submodule", "list", "update"],
            &["brit", "build", "reset"],
            &["brit", "--version"],
            &["brit", "resets"],
        ] {
            assert_eq!(line(&args(owned)), None, "{owned:?}");
        }
    }
}
