//! Recursive --help parsing for subcommand discovery.
//!
//! For each binary in scope, invoke `<bin> --help`, parse the
//! `Commands:` block, recurse into each subcommand's `--help`,
//! return the full tree of leaf subcommand paths.

use std::path::Path;
use std::process::Command;

use anyhow::{Context, Result};

/// A path to a leaf subcommand, e.g. `["brit", "log"]` or `["brit", "branch", "list"]`.
pub type SubcommandPath = Vec<String>;

/// Parse the `Commands:` block of clap's --help output.
/// Returns the names of immediate subcommands (no recursion here).
///
/// Format expected (clap default):
/// ```text
/// Commands:
///   archive       Subcommands for creating worktree archives
///   branch        Interact with branches [aliases: branches]
///   help          Print this message or the help of the given subcommand(s)
/// ```
///
/// Returns an empty vec if no `Commands:` section exists (i.e., a leaf).
/// Filters out `help` (clap's auto-generated help command).
pub fn parse_subcommands_from_help(help_text: &str) -> Vec<String> {
    let mut in_commands = false;
    let mut subs: Vec<String> = Vec::new();

    for line in help_text.lines() {
        let trimmed = line.trim_end();
        if trimmed.starts_with("Commands:") {
            in_commands = true;
            continue;
        }
        if !in_commands {
            continue;
        }
        // Empty line OR section header at column 0 → end of Commands block
        if trimmed.is_empty() {
            break;
        }
        if !line.starts_with(' ') {
            // New section
            break;
        }
        // Subcommand line: "  <name>       <description>"
        let line = trimmed.trim_start();
        let name = line
            .split_whitespace()
            .next()
            .map(|s| s.to_string());
        if let Some(n) = name {
            if n != "help" {
                subs.push(n);
            }
        }
    }
    subs
}

/// Recursively discover all leaf subcommand paths for a binary.
/// Invokes `<binary> [path...] --help` for each branch.
pub fn discover_subcommands(binary: &Path, binary_name: &str) -> Result<Vec<SubcommandPath>> {
    let mut results: Vec<SubcommandPath> = Vec::new();
    let initial_path = vec![binary_name.to_string()];
    walk(binary, &initial_path, &mut results)?;
    Ok(results)
}

fn walk(binary: &Path, current: &[String], out: &mut Vec<SubcommandPath>) -> Result<()> {
    // Invoke `<binary> [args...] --help`
    let args: Vec<String> = current.iter().skip(1).cloned().collect();
    let output = Command::new(binary)
        .args(&args)
        .arg("--help")
        .output()
        .with_context(|| format!("invoke {} {:?} --help", binary.display(), args))?;
    // Combine stdout + stderr (clap may print to either)
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let subs = parse_subcommands_from_help(&combined);
    if subs.is_empty() {
        // Leaf — record it (unless it's the bare binary name with nothing else)
        if current.len() > 1 {
            out.push(current.to_vec());
        }
    } else {
        for sub in subs {
            let mut next = current.to_vec();
            next.push(sub);
            walk(binary, &next, out)?;
        }
    }
    Ok(())
}
