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

use brit_epr::{
    diagnostic::{emit, render_context},
    parse_pillar_trailers, validate_pillar_trailers, PillarTrailers,
};

fn main() -> ExitCode {
    brit_epr::diagnostic::install_panic_hook("brit-verify", env!("CARGO_PKG_VERSION"));
    let args: Vec<String> = std::env::args().collect();

    let (rev, repo_path) = match parse_args(&args) {
        Ok(parsed) => parsed,
        Err(msg) => {
            emit(format_args!("{msg}\n\nUsage: brit-verify <commit-rev> [--repo <path>]"));
            return ExitCode::from(2);
        }
    };

    let repo = match gix::discover(&repo_path) {
        Ok(repo) => repo,
        Err(e) => {
            emit(render_context(format_args!("failed to open repo at {repo_path}"), &e));
            return ExitCode::from(3);
        }
    };

    let commit = match repo.rev_parse_single(rev.as_str()) {
        Ok(id) => match id.object() {
            Ok(obj) => match obj.try_into_commit() {
                Ok(c) => c,
                Err(_) => {
                    emit(format_args!("rev {rev} does not point at a commit"));
                    return ExitCode::from(3);
                }
            },
            Err(e) => {
                emit(render_context(format_args!("failed to load object for {rev}"), &e));
                return ExitCode::from(3);
            }
        },
        Err(e) => {
            emit(render_context(format_args!("failed to resolve rev {rev}"), &e));
            return ExitCode::from(3);
        }
    };

    let decoded = match commit.decode() {
        Ok(c) => c,
        Err(e) => {
            emit(render_context(format_args!("failed to decode commit {rev}"), &e));
            return ExitCode::from(3);
        }
    };

    // decoded.message is &BStr (the full message including trailing trailers).
    // parse_pillar_trailers takes &[u8]; BStr derefs to [u8].
    let trailers = parse_pillar_trailers(decoded.message.as_ref());

    match validate_pillar_trailers(&trailers) {
        Ok(()) => match print_valid(&rev, &trailers) {
            Ok(()) => ExitCode::SUCCESS,
            // The reader closed stdout early: end quietly like Git killed by `SIGPIPE`, never with a panic.
            Err(e) if e.kind() == std::io::ErrorKind::BrokenPipe => {
                ExitCode::from(brit_epr::diagnostic::CLOSED_READER_EXIT)
            }
            Err(e) => {
                emit(render_context("failed to write the result to stdout", &e));
                ExitCode::from(3)
            }
        },
        Err(e) => {
            emit(format_args!("✗ pillar validation failed for {rev}: {e}"));
            ExitCode::FAILURE
        }
    }
}

fn print_valid(rev: &str, trailers: &PillarTrailers) -> std::io::Result<()> {
    brit_epr::outln!("✓ pillar trailers valid for {rev}")?;
    brit_epr::outln!("  Lamad: {}", trailers.lamad.as_deref().unwrap_or("-"))?;
    brit_epr::outln!("  Shefa: {}", trailers.shefa.as_deref().unwrap_or("-"))?;
    brit_epr::outln!("  Qahal: {}", trailers.qahal.as_deref().unwrap_or("-"))?;
    if let Some(ref c) = trailers.lamad_node {
        brit_epr::outln!("  Lamad-Node: {c}")?;
    }
    if let Some(ref c) = trailers.shefa_node {
        brit_epr::outln!("  Shefa-Node: {c}")?;
    }
    if let Some(ref c) = trailers.qahal_node {
        brit_epr::outln!("  Qahal-Node: {c}")?;
    }
    Ok(())
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
                repo_path.clone_from(&args[i]);
                i += 1;
            }
            unknown => return Err(format!("unknown argument: {unknown}")),
        }
    }
    Ok((rev, repo_path))
}
