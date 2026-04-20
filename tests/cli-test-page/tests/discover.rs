//! Self-tests for subcommand discovery.

use cli_test_page::discover::parse_subcommands_from_help;

#[test]
fn parses_top_level_subcommand_list() {
    let help_text = r#"
The git underworld

Usage: brit [OPTIONS] <COMMAND>

Commands:
  archive       Subcommands for creating worktree archives
  branch        Interact with branches [aliases: branches]
  clean         Remove untracked files from the working tree
  log           List all commits in a repository

Options:
  -h, --help     Print help
"#;
    let subs = parse_subcommands_from_help(help_text);
    assert_eq!(subs, vec!["archive", "branch", "clean", "log"]);
}

#[test]
fn returns_empty_for_leaf_subcommand_with_no_subcommands() {
    let help_text = r#"
Print all commits

Usage: brit log [OPTIONS]

Options:
  -h, --help     Print help
"#;
    let subs = parse_subcommands_from_help(help_text);
    assert!(subs.is_empty());
}

#[test]
fn ignores_alias_annotations_and_strips_whitespace() {
    let help_text = r#"
Usage: brit [OPTIONS] <COMMAND>

Commands:
  branch        Interact with branches [aliases: branches]
  remote        Interact with remotes [aliases: remotes]
"#;
    let subs = parse_subcommands_from_help(help_text);
    assert_eq!(subs, vec!["branch", "remote"]);
}
