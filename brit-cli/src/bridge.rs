//! Named Git bridges: operations `brit` does not own and never performs.
//!
//! Brit coexists with stock Git. For a bridged name it prints the exact `git`
//! command to run and exits; there is no fallback, no alias and no attempt to
//! run Git on the caller's behalf. The bridge runs before the CLI dispatches a command.

use std::ffi::OsString;

/// Exit code of every bridge refusal; the same code clap and `CliError::Args` use for usage errors.
pub const EXIT_CODE: i32 = 2;

/// Top-level subcommands that Git owns and brit bridges.
pub const BRIDGED: &[&str] = &[
    "reset",
    "rm",
    "mv",
    "stash",
    "cherry-pick",
    "revert",
    "checkout",
    "notes",
];

/// Submodule verbs handled entirely by Git.
pub const BRIDGED_SUBMODULE: &[&str] = &["init", "sync"];

/// Return the line to print if `args` (including the program name) invoke a bridged operation.
pub fn line(args: &[OsString], command_index: usize, native_update: bool) -> Option<String> {
    let name = args.get(command_index)?.to_str()?;
    let (name, rest) = if matches!(name, "submodule" | "submodules") {
        let verb = args.get(command_index + 1)?.to_str()?;
        let rest = &args[command_index + 2..];
        if !(BRIDGED_SUBMODULE.contains(&verb) || verb == "update" && (!native_update || bridge_update(rest))) {
            return None;
        }
        (format!("submodule {verb}"), rest)
    } else if BRIDGED.contains(&name) {
        (name.to_owned(), &args[command_index + 1..])
    } else {
        return None;
    };
    let mut line = format!("brit: {name} is a Git bridge operation; run: git");
    for arg in &args[1..command_index] {
        line.push(' ');
        line.push_str(&shell_quote(&arg.to_string_lossy()));
    }
    line.push(' ');
    line.push_str(&name);
    for arg in rest {
        line.push(' ');
        line.push_str(&shell_quote(&arg.to_string_lossy()));
    }
    Some(line)
}

fn bridge_update(args: &[OsString]) -> bool {
    let mut options = true;
    let mut has_path = false;
    let mut unsupported_form = false;
    for arg in args {
        if options {
            match arg.to_str() {
                Some("--") => {
                    options = false;
                    continue;
                }
                Some("--help" | "-h" | "--recursive" | "--remote") => return false,
                Some("--init") => continue,
                _ if arg.as_encoded_bytes().starts_with(b"-") => {
                    unsupported_form = true;
                    continue;
                }
                _ => {}
            }
        }
        if arg.as_encoded_bytes().starts_with(b":")
            || arg.as_encoded_bytes() == b"."
            || arg.as_encoded_bytes().iter().any(|byte| b"*?[".contains(byte))
        {
            unsupported_form = true;
        }
        has_path = true;
    }
    unsupported_form || !has_path
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

    fn bridge_line(list: &[&str]) -> Option<String> {
        line(&args(list), 1, true)
    }

    #[test]
    fn bridged_names_print_the_same_arguments_for_git() {
        assert_eq!(
            bridge_line(&["brit", "reset", "--hard", "HEAD~1"]).as_deref(),
            Some("brit: reset is a Git bridge operation; run: git reset --hard HEAD~1")
        );
        assert_eq!(
            bridge_line(&["brit", "stash"]).as_deref(),
            Some("brit: stash is a Git bridge operation; run: git stash")
        );
        assert_eq!(
            bridge_line(&["brit", "submodule", "update", "--merge", "child"]).as_deref(),
            Some("brit: submodule update is a Git bridge operation; run: git submodule update --merge child")
        );
    }

    #[test]
    fn arguments_needing_quotes_are_quoted_for_a_shell() {
        assert_eq!(
            bridge_line(&["brit", "stash", "a b", "it's", ""]).as_deref(),
            Some(r"brit: stash is a Git bridge operation; run: git stash 'a b' 'it'\''s' ''")
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
            &["brit", "submodule", "update", "--init", "child"],
            &["brit", "submodule", "update", "--init", "--recursive"],
            &["brit", "submodule", "update", "--remote", "child"],
            &["brit", "build", "reset"],
            &["brit", "--version"],
            &["brit", "resets"],
        ] {
            assert_eq!(bridge_line(owned), None, "{owned:?}");
        }
    }
}
