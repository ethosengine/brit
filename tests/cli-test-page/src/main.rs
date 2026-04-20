//! brit-test-page — runs the brit CLI test suite, produces baseline.md.
//!
//! Three modes:
//!   --check (default)           — diff candidate vs baseline.md; exit 1 on mismatch
//!   --update                    — copy candidate over baseline.md (after human review)
//!   --candidate <path>          — write candidate to arbitrary path (TDD redesign loop)

use std::collections::BTreeSet;
use std::fs;
use std::path::PathBuf;
use std::process::{Command, ExitCode};

use anyhow::{Context, Result};
use clap::Parser;

use cli_test_page::{coverage, diff, discover, format};

use coverage::compute_coverage;
use discover::{discover_subcommands, SubcommandPath};
use format::{format_test_page, BinarySection, SubcommandCapture};

const BINARIES: &[&str] = &["brit", "rakia", "brit-verify", "brit-build-ref"];

#[derive(Parser)]
#[command(name = "brit-test-page", version, about = "Run the brit CLI test suite and produce a markdown test page")]
struct Cli {
    /// Path to the brit workspace root (default: derived from CARGO_MANIFEST_DIR or cwd)
    #[arg(long)]
    workspace: Option<PathBuf>,

    /// Mode: check (default), update, or candidate
    #[command(flatten)]
    mode: Mode,
}

#[derive(clap::Args)]
#[group(multiple = false)]
struct Mode {
    /// Default mode: diff candidate vs baseline; exit 1 on mismatch
    #[arg(long, conflicts_with_all = ["update", "candidate"])]
    check: bool,
    /// Copy candidate over baseline.md (after human review of diff)
    #[arg(long, conflicts_with_all = ["check", "candidate"])]
    update: bool,
    /// Write candidate to arbitrary path (for the desired-then-iterate TDD loop)
    #[arg(long, value_name = "PATH", conflicts_with_all = ["check", "update"])]
    candidate: Option<PathBuf>,
}

fn main() -> ExitCode {
    match run() {
        Ok(code) => code,
        Err(e) => {
            eprintln!("error: {e}");
            for cause in e.chain().skip(1) {
                eprintln!("caused by: {cause}");
            }
            ExitCode::from(2)
        }
    }
}

fn run() -> Result<ExitCode> {
    let cli = Cli::parse();
    let workspace = cli.workspace.unwrap_or_else(|| {
        // CARGO_MANIFEST_DIR is set when running via `cargo run`; otherwise
        // assume the binary lives at <ws>/target/release/brit-test-page
        // and ws is two dirs up.
        std::env::var("CARGO_MANIFEST_DIR")
            .map(|s| {
                let p = PathBuf::from(s);
                p.parent()
                    .and_then(|p| p.parent())
                    .and_then(|p| p.parent())
                    .map(|p| p.to_path_buf())
                    .unwrap_or(p)
            })
            .unwrap_or_else(|_| std::env::current_dir().expect("cwd"))
    });

    let target_dir = workspace.join("target/release");
    let baseline_path = workspace.join("tests/baseline.md");

    // Step 1: invoke shell journey tests + Rust journey tests; both write to staging.
    invoke_test_layers(&workspace)?;

    // Step 2: discover the universe of subcommands.
    let mut all_coverage = Vec::new();
    let mut all_sections = Vec::new();
    let staging_dir = workspace.join("tests/.test-page-staging");

    for binary_name in BINARIES {
        let binary_path = target_dir.join(binary_name);
        if !binary_path.exists() {
            eprintln!("warning: {binary_name} not found at {}; skipping", binary_path.display());
            continue;
        }
        let universe = discover_subcommands(&binary_path, binary_name)
            .with_context(|| format!("discover {binary_name}"))?;
        let captured = collect_captured_paths(&staging_dir, binary_name)?;
        let cov = compute_coverage(binary_name, &universe, &captured);
        all_coverage.push(cov);

        let captures = read_captures(&staging_dir, binary_name)?;
        all_sections.push(BinarySection {
            binary: binary_name.to_string(),
            captures,
        });
    }

    // Step 3: format candidate.
    let candidate = format_test_page(&all_coverage, &all_sections);

    // Step 4: dispatch on mode.
    if let Some(out_path) = cli.mode.candidate {
        fs::write(&out_path, &candidate).with_context(|| format!("write {}", out_path.display()))?;
        println!("wrote candidate to {}", out_path.display());
        return Ok(ExitCode::SUCCESS);
    }

    if cli.mode.update {
        fs::write(&baseline_path, &candidate)
            .with_context(|| format!("write {}", baseline_path.display()))?;
        println!("baseline updated: {}", baseline_path.display());
        return Ok(ExitCode::SUCCESS);
    }

    // Default: --check mode.
    let baseline = fs::read_to_string(&baseline_path)
        .with_context(|| format!("read {}", baseline_path.display()))
        .unwrap_or_default();
    if diff::has_diff(&baseline, &candidate) {
        println!("--- baseline (current)");
        println!("+++ candidate (this run)");
        println!("{}", diff::render_unified_diff(&baseline, &candidate));
        eprintln!("\nbaseline differs from candidate. Run --update to accept changes.");
        Ok(ExitCode::from(1))
    } else {
        println!("OK — candidate matches baseline.");
        Ok(ExitCode::SUCCESS)
    }
}

fn invoke_test_layers(workspace: &PathBuf) -> Result<()> {
    let staging = workspace.join("tests/.test-page-staging");
    fs::remove_dir_all(&staging).ok();
    fs::create_dir_all(&staging).context("mkdir staging")?;

    // Shell layer: invoke journey.sh (it sources gix.sh, ein.sh, rakia.sh, etc.)
    // The shell tests dump captured outputs into tests/.test-page-staging/shell/<binary>/<subcmd>.txt
    // For now the shell layer doesn't write to staging — Tasks 17-18 add that wiring.
    // This block is a no-op until those tasks land; the runner just reads what's there (likely empty).

    // Rust layer: invoke `cargo test -p cli-journey` which writes to staging via runner helpers.
    // (Hooked in via test side effects when individual tests are filled in by Tasks 13-15.)
    let status = Command::new("cargo")
        .args(["test", "-p", "cli-journey", "--", "--nocapture"])
        .env("BRIT_TEST_PAGE_STAGING", &staging)
        .current_dir(workspace)
        .status()
        .context("run cargo test -p cli-journey")?;
    if !status.success() {
        // Don't bail outright — let the runner produce a candidate even when
        // tests fail, so the user can see what broke. Just warn.
        eprintln!("warning: cli-journey tests failed (exit {}); candidate may be incomplete",
            status.code().unwrap_or(-1));
    }
    Ok(())
}

fn collect_captured_paths(staging_dir: &PathBuf, binary: &str) -> Result<BTreeSet<SubcommandPath>> {
    // Look for files at <staging>/{shell,rust}/<binary>/.../*.txt
    let mut out: BTreeSet<SubcommandPath> = BTreeSet::new();
    for layer in ["shell", "rust"] {
        let bin_dir = staging_dir.join(layer).join(binary);
        if !bin_dir.exists() {
            continue;
        }
        walk_captures(&bin_dir, &[binary.to_string()], &mut out)?;
    }
    Ok(out)
}

fn walk_captures(
    dir: &std::path::Path,
    prefix: &[String],
    out: &mut BTreeSet<SubcommandPath>,
) -> Result<()> {
    for entry in fs::read_dir(dir).with_context(|| format!("read {}", dir.display()))? {
        let entry = entry?;
        let path = entry.path();
        let name = path.file_name().and_then(|s| s.to_str()).unwrap_or("").to_string();
        if path.is_dir() {
            let mut next = prefix.to_vec();
            next.push(name);
            walk_captures(&path, &next, out)?;
        } else if name.ends_with(".txt") {
            let stem = name.trim_end_matches(".txt").to_string();
            let mut full = prefix.to_vec();
            full.push(stem);
            out.insert(full);
        }
    }
    Ok(())
}

fn read_captures(staging_dir: &PathBuf, binary: &str) -> Result<Vec<SubcommandCapture>> {
    let mut captures = Vec::new();
    for layer in ["shell", "rust"] {
        let bin_dir = staging_dir.join(layer).join(binary);
        if !bin_dir.exists() {
            continue;
        }
        read_capture_dir(&bin_dir, &[binary.to_string()], &mut captures)?;
    }
    captures.sort_by(|a, b| a.subcommand_path.cmp(&b.subcommand_path));
    Ok(captures)
}

fn read_capture_dir(
    dir: &std::path::Path,
    prefix: &[String],
    captures: &mut Vec<SubcommandCapture>,
) -> Result<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        let name = path.file_name().and_then(|s| s.to_str()).unwrap_or("").to_string();
        if path.is_dir() {
            let mut next = prefix.to_vec();
            next.push(name);
            read_capture_dir(&path, &next, captures)?;
        } else if name.ends_with(".txt") {
            let stem = name.trim_end_matches(".txt").to_string();
            let mut subpath = prefix.to_vec();
            subpath.push(stem);
            let body = fs::read_to_string(&path)?;
            captures.push(SubcommandCapture {
                subcommand_path: subpath,
                help: String::from("(captured by test)"),
                invocation: format!("{}", path.display()),
                output: body,
            });
        }
    }
    Ok(())
}
