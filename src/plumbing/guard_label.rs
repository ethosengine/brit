//! Name another Brit process by its subcommand path, never by its option values or positionals.

/// The subcommand names in `args` (a Brit command line without its program name), walking
/// `command`'s tree. Options are skipped together with any separate value they take, and the
/// walk stops at the first token that is not a subcommand, so no option value, URL, path or
/// other positional is ever returned.
pub(crate) fn subcommand_path(command: &clap::Command, args: &[String]) -> Vec<String> {
    let root = command;
    let mut current = command;
    let mut path = Vec::new();
    let mut tokens = args.iter();
    while let Some(token) = tokens.next() {
        if token == "--" {
            break;
        }
        if let Some(long) = token.strip_prefix("--") {
            if !long.contains('=') && takes_separate_value(root, current, |arg| arg.get_long() == Some(long)) {
                tokens.next();
            }
            continue;
        }
        if let Some(shorts) = token.strip_prefix('-')
            && !shorts.is_empty()
        {
            // `-cvalue` carries its value; a lone `-c` takes the next token.
            let mut chars = shorts.chars();
            if let (Some(short), None) = (chars.next(), chars.next())
                && takes_separate_value(root, current, |arg| arg.get_short() == Some(short))
            {
                tokens.next();
            }
            continue;
        }
        let Some(subcommand) = current.find_subcommand(token) else {
            break;
        };
        path.push(subcommand.get_name().to_owned());
        current = subcommand;
    }
    path
}

fn takes_separate_value(root: &clap::Command, current: &clap::Command, matches: impl Fn(&clap::Arg) -> bool) -> bool {
    current
        .get_arguments()
        .chain(root.get_arguments().filter(|arg| arg.is_global_set()))
        .find(|arg| matches(arg))
        .is_some_and(|arg| arg.get_action().takes_values() && !arg.is_require_equals_set())
}

#[cfg(test)]
mod tests {
    use clap::CommandFactory;

    fn path(args: &[&str]) -> Vec<String> {
        let args: Vec<String> = args.iter().map(|arg| (*arg).to_owned()).collect();
        super::subcommand_path(&crate::plumbing::Args::command(), &args)
    }

    #[test]
    fn credentials_in_urls_and_config_values_never_appear() {
        for (args, expected) in [
            (
                &["-c", "http.extraHeader=Authorization: Bearer s3cr3t", "branch", "list"][..],
                &["branch", "list"][..],
            ),
            (
                &["-r", "https://user:TOKEN@host/repo.git", "commit", "-m", "TOKEN"][..],
                &["commit"][..],
            ),
            (
                &[
                    "--config=http.extraHeader=s3cr3t",
                    "worktree",
                    "add",
                    "-b",
                    "x",
                    "TOKEN",
                ],
                &["worktree", "add"],
            ),
            (&["--wait=5", "branch", "create", "TOKEN"], &["branch", "create"]),
            (&["-cuser.password=s3cr3t", "status"], &["status"]),
            (&["merge-base", "TOKEN", "s3cr3t"], &["merge-base"]),
        ] {
            let named = path(args);
            assert_eq!(named, expected, "{args:?}");
            assert!(
                named
                    .iter()
                    .all(|name| !name.contains("TOKEN") && !name.contains("s3cr3t")),
                "{args:?}"
            );
        }
    }
}
