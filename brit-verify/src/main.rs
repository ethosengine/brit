//! `brit-verify` — verify pillar trailers on a git commit.
//!
//! Usage: `brit-verify <commit-rev> [--repo <path>]`
//!
//! Opens the repository at `<path>` (current directory if omitted), resolves
//! `<commit-rev>` to a commit object, extracts the commit message body,
//! parses pillar trailers with brit-epr, runs structural validation, and
//! prints the result. Exits 0 on success, 1 on validation failure, 2 on
//! usage error, 3 on repo error.
//!
//! No clap, no tracing — smallest possible end-to-end proof that parser
//! and validator work against real git objects.

use std::process::ExitCode;

use brit_epr::{parse_pillar_trailers, validate_pillar_trailers};

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();

    let (rev, repo_path) = match parse_args(&args) {
        Ok(parsed) => parsed,
        Err(msg) => {
            eprintln!("{msg}\n\nUsage: brit-verify <commit-rev> [--repo <path>]");
            return ExitCode::from(2);
        }
    };

    let repo = match gix::discover(&repo_path) {
        Ok(repo) => repo,
        Err(e) => {
            eprintln!("failed to open repo at {repo_path}: {e}");
            return ExitCode::from(3);
        }
    };

    let commit = match repo.rev_parse_single(rev.as_str()) {
        Ok(id) => match id.object() {
            Ok(obj) => match obj.try_into_commit() {
                Ok(c) => c,
                Err(_) => {
                    eprintln!("rev {rev} does not point at a commit");
                    return ExitCode::from(3);
                }
            },
            Err(e) => {
                eprintln!("failed to load object for {rev}: {e}");
                return ExitCode::from(3);
            }
        },
        Err(e) => {
            eprintln!("failed to resolve rev {rev}: {e}");
            return ExitCode::from(3);
        }
    };

    let decoded = match commit.decode() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("failed to decode commit {rev}: {e}");
            return ExitCode::from(3);
        }
    };

    // decoded.message is &BStr (the full message including trailing trailers).
    // parse_pillar_trailers takes &[u8]; BStr derefs to [u8].
    let trailers = parse_pillar_trailers(decoded.message.as_ref());

    match validate_pillar_trailers(&trailers) {
        Ok(()) => {
            println!("✓ pillar trailers valid for {rev}");
            println!("  Lamad: {}", trailers.lamad.as_deref().unwrap_or("-"));
            println!("  Shefa: {}", trailers.shefa.as_deref().unwrap_or("-"));
            println!("  Qahal: {}", trailers.qahal.as_deref().unwrap_or("-"));
            if let Some(ref c) = trailers.lamad_node {
                println!("  Lamad-Node: {c}");
            }
            if let Some(ref c) = trailers.shefa_node {
                println!("  Shefa-Node: {c}");
            }
            if let Some(ref c) = trailers.qahal_node {
                println!("  Qahal-Node: {c}");
            }
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("✗ pillar validation failed for {rev}: {e}");
            ExitCode::FAILURE
        }
    }
}

fn parse_args(args: &[String]) -> Result<(String, String), String> {
    if args.len() < 2 {
        return Err("missing <commit-rev> argument".into());
    }
    let rev = args[1].clone();
    let mut repo_path = ".".to_string();

    let mut i = 2;
    while i < args.len() {
        match args[i].as_str() {
            "--repo" => {
                i += 1;
                if i >= args.len() {
                    return Err("--repo requires a path argument".into());
                }
                repo_path = args[i].clone();
                i += 1;
            }
            unknown => return Err(format!("unknown argument: {unknown}")),
        }
    }
    Ok((rev, repo_path))
}
