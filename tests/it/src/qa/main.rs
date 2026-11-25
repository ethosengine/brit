use std::{fs::OpenOptions, io::Write, path::PathBuf, time::Instant};

use clap::{Parser, Subcommand};
use gix_error::{OptionExt, Result, ResultExt, bail, message};
use serde_json::{Value, json};

mod catalog;
mod coverage;
mod downstream;
mod features;
mod native_tests;
mod ops;
mod plan;
mod record;
mod requirements;
mod run;
mod scope;

#[derive(Parser)]
#[command(about = "Plan and run quality-assurance checks locally or in CI")]
struct Args {
    /// Show compact progress and retain detailed task logs (automatic under Codex).
    #[arg(long, global = true)]
    agent: bool,
    #[command(subcommand)]
    command: Action,
}

#[derive(clap::Args)]
struct Selection {
    /// Fast selects affected checks; exhaustive selects every distinct configuration.
    #[arg(long, default_value = "fast", value_parser = ["fast", "exhaustive"])]
    mode: String,
    /// Compare from the merge base with this revision, including local edits.
    #[arg(long, default_value = "origin/main")]
    base: String,
}

#[derive(Subcommand)]
enum Action {
    /// Show complete tasks and their commands for this host, or the CI assignment.
    Plan {
        #[command(flatten)]
        selection: Selection,
        /// Inspect another platform without running its commands.
        #[arg(long)]
        platform: Option<String>,
        #[arg(long)]
        all_platforms: bool,
        /// Restrict CI assignments to a comma-separated list of platform names.
        #[arg(long, requires = "all_platforms", value_delimiter = ',',
            value_parser = clap::builder::PossibleValuesParser::new(catalog::PLATFORMS.iter().copied()))]
        platforms: Vec<String>,
        /// Select a union of QA stages.
        #[arg(long, value_delimiter = ',', default_value = "all", value_parser = catalog::STAGES)]
        stage: Vec<String>,
        /// Run only Nextest commands, restricted to this filterset.
        #[arg(long, value_parser = clap::builder::NonEmptyStringValueParser::new())]
        tests: Option<String>,
        #[arg(long)]
        github_output: Option<PathBuf>,
    },
    /// Run all applicable tasks serially; independent tasks continue after a failure.
    Run {
        #[command(flatten)]
        selection: Selection,
        #[arg(long, conflicts_with = "from_env")]
        plan: Option<PathBuf>,
        /// Read QA_PLAN and optionally QA_JOB supplied by the CI scheduler.
        #[arg(long)]
        from_env: bool,
        /// Execute one job from a saved plan.
        #[arg(long)]
        job: Option<String>,
        /// Select a union of stages; overlapping selectors never repeat a task.
        #[arg(long, value_delimiter = ',', default_value = "all", value_parser = catalog::STAGES)]
        stage: Vec<String>,
        #[arg(long)]
        dry_run: bool,
        /// Automatically collect and reuse per-test coverage when LLVM is available; off disables it.
        #[arg(long, value_enum)]
        coverage: Option<coverage::Mode>,
    },
    #[command(hide = true)]
    Exec {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true, required = true)]
        args: Vec<String>,
    },
    /// Check a consumer checkout against this workspace (GitHub Actions only).
    #[command(hide = true)]
    Downstream {
        name: String,
        #[arg(long)]
        checkout: PathBuf,
    },
}

fn main() -> Result<()> {
    if std::env::var_os("GIX_QA_COVERAGE_RUSTC").as_deref() == Some(std::ffi::OsStr::new("1")) {
        return coverage::compiler(&std::env::args_os().skip(1).collect::<Vec<_>>());
    }
    let args = Args::parse();
    let started = Instant::now();
    let agent = args.agent || std::env::var_os("CODEX_THREAD_ID").is_some();
    let current_dir = std::env::current_dir().or_raise(|| message("read current directory"))?;
    if let Action::Downstream { name, checkout } = &args.command {
        if std::env::var("GITHUB_ACTIONS").as_deref() != Ok("true") {
            bail!("Downstream checks run only in GitHub Actions");
        }
        let result = downstream::execute(&current_dir, name, checkout);
        if result.is_err() {
            eprintln!(
                "::warning::The downstream compatibility check failed; see the errors below. This check is advisory while gix is pre-1.0."
            );
        }
        return result;
    }
    if let Action::Exec { args } = &args.command {
        if args[0] == "coverage-test" {
            return coverage::test(&args[1..]);
        }
        if args[0] == "native-tests" {
            return native_tests::execute(
                &current_dir,
                &args[1..],
                &mut native_tests::NativeSnapshots::default(),
                None,
                &mut None,
            );
        }
        return ops::execute(&current_dir, args);
    }
    let checkout = if record::eligible(&args.command)
        && std::env::var_os("CI").is_none()
        && std::env::var_os("GITHUB_ACTIONS").is_none()
    {
        match record::Checkout::capture(&current_dir) {
            Ok(checkout) => Some(checkout),
            Err(error) => {
                eprintln!("QA checks-pass recording is unavailable: {error}");
                None
            }
        }
    } else {
        None
    };
    let planning_started = Instant::now();
    let metadata = plan::metadata(&current_dir)?;
    let root = PathBuf::from(
        metadata["workspace_root"]
            .as_str()
            .ok_or_raise(|| message("missing workspace_root"))?,
    );
    match args.command {
        Action::Plan {
            selection,
            platform,
            all_platforms,
            platforms,
            stage,
            tests,
            github_output,
        } => {
            let pull_request = std::env::var("GITHUB_EVENT_NAME").as_deref() == Ok("pull_request");
            let stage_subset = !stage.iter().any(|stage| stage == "all");
            let narrowed = is_narrowed(
                &selection.mode,
                pull_request,
                !platforms.is_empty(),
                stage_subset,
                tests.is_some(),
            );
            let mut selected = if let Some(filter) = tests {
                plan::filtered(&metadata, &selection.mode, filter)?
            } else {
                let mut selected = plan::plan(&root, &metadata, &selection.mode, &selection.base)?;
                scope::prune(&root, &metadata, &mut selected)?;
                selected
            };
            let fast_dispatch = std::env::var("GITHUB_EVENT_NAME").as_deref() == Ok("workflow_dispatch");
            let platform = platform.unwrap_or_else(catalog::host);
            let mut plan = if all_platforms {
                catalog::ci_plan(&metadata, &selected, &platforms, &stage, fast_dispatch)?
            } else {
                catalog::local_plan(&metadata, &selected, &platform, &stage)?
            };
            plan["platform"] = platform.into();
            plan["narrowed"] = narrowed.into();
            if let Some(reasons) = selected["reasons"].as_array_mut() {
                if !platforms.is_empty() {
                    reasons.push(json!(format!("Restricted CI assignment to {}", platforms.join(", "))));
                    plan["platforms"] = json!(platforms);
                }
                if stage_subset {
                    reasons.push(json!(format!("Selected stages: {}", stage.join(", "))));
                    plan["stages"] = json!(stage);
                }
            }
            plan["selection"] = selected;
            if let Some(path) = github_output {
                let mut output = OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(&path)
                    .or_raise(|| message!("open GitHub output file {}", path.display()))?;
                writeln!(
                    output,
                    "matrix={}\nplan={}\ndownstream={}\nself_test={}",
                    json!({"include": plan["jobs"]}),
                    execution_plan(&plan),
                    if plan["downstream"]["include"]
                        .as_array()
                        .is_some_and(|rows| !rows.is_empty())
                    {
                        plan["downstream"].to_string()
                    } else {
                        String::new()
                    },
                    plan["tasks"]
                        .as_array()
                        .is_some_and(|tasks| tasks.iter().any(|row| row["task"]["id"] == "self-test")),
                )
                .or_raise(|| message("write GitHub job outputs"))?;
            }
            println!(
                "{}",
                serde_json::to_string_pretty(&plan).or_raise(|| message("serialize QA plan"))?
            );
        }
        Action::Run {
            selection,
            plan: saved,
            from_env,
            job,
            stage,
            dry_run,
            coverage,
        } => {
            let saved: Option<Value> = if let Some(path) = saved {
                Some(
                    serde_json::from_slice(
                        &std::fs::read(&path).or_raise(|| message!("read plan {}", path.display()))?,
                    )
                    .or_raise(|| message("parse saved QA plan"))?,
                )
            } else if from_env {
                Some(
                    serde_json::from_str(&std::env::var("QA_PLAN").or_raise(|| message("read QA_PLAN"))?)
                        .or_raise(|| message("parse QA_PLAN"))?,
                )
            } else {
                None
            };
            let job = job.or_else(|| from_env.then(|| std::env::var("QA_JOB").ok()).flatten());
            let ci_job = job.is_some();
            let (tasks, selected) = if let Some(job) = job {
                let saved = saved.as_ref().ok_or_raise(|| message("--job requires a saved plan"))?;
                (catalog::job_tasks(&metadata, saved, &job)?, None)
            } else {
                let mut selection = match &saved {
                    Some(plan) => plan["selection"].clone(),
                    None => plan::plan(&root, &metadata, &selection.mode, &selection.base)?,
                };
                if saved.is_none()
                    && stage
                        .iter()
                        .any(|stage| matches!(stage.as_str(), "all" | "check" | "tests" | "doctests" | "wasm"))
                {
                    scope::prune(&root, &metadata, &mut selection)?;
                }
                if agent {
                    eprintln!(
                        "QA selection: {} ({} packages)",
                        selection["mode"].as_str().unwrap_or_default(),
                        selection["packages"].as_array().map_or(0, Vec::len)
                    );
                    for reason in selection["reasons"].as_array().into_iter().flatten() {
                        if selection["mode"] == "exhaustive" {
                            eprintln!("{}", reason.as_str().unwrap_or_default());
                        }
                    }
                } else {
                    eprintln!("QA selection: {selection}");
                }
                let mut tasks = catalog::tasks(&metadata, &selection, &catalog::host())?;
                if let Some(stages) = saved.as_ref().and_then(|plan| plan.get("stages")) {
                    let stages = serde_json::from_value::<Vec<String>>(stages.clone())
                        .or_raise(|| message("read saved QA stages"))?;
                    tasks = catalog::select(tasks, &stages)?;
                }
                (catalog::select(tasks, &stage)?, Some(selection))
            };
            let planning = planning_started.elapsed();
            eprintln!("{} tasks selected for {}", tasks.len(), catalog::host());
            let logs = if agent && !dry_run {
                let logs = run::log_directory()?;
                eprintln!("QA logs: {}", logs.display());
                Some(logs)
            } else {
                None
            };
            let fast = selected
                .as_ref()
                .or_else(|| saved.as_ref().map(|saved| &saved["selection"]))
                .is_some_and(|selection| selection["mode"] == "fast");
            let mut coverage = if dry_run
                || saved
                    .as_ref()
                    .is_some_and(|plan| plan["selection"]["tests"].is_string())
                || !tasks
                    .iter()
                    .any(|task| matches!(task.id, "native-tests" | "feature-tests" | "isolated-tests"))
            {
                None
            } else {
                let base = selected
                    .as_ref()
                    .or_else(|| saved.as_ref().map(|saved| &saved["selection"]))
                    .and_then(|selection| selection["base"].as_str());
                coverage::Run::optional(&root, coverage, fast, base)
            };
            let timings = catalog::execute(&root, tasks, dry_run, ci_job, logs.as_deref(), &mut coverage)?;
            if let Some(coverage) = &mut coverage {
                coverage.finish(&root);
                if coverage.summary.omitted > 0 {
                    eprintln!("Coverage: {} test executions omitted.", coverage.summary.omitted);
                }
            }
            if let (Some(checkout), Some(selected)) = (checkout, selected) {
                record::finish(
                    &root,
                    checkout,
                    &selected,
                    &selection.base,
                    &record::Timings {
                        elapsed: started.elapsed(),
                        planning,
                        tasks: timings,
                    },
                    coverage.as_ref().map(|coverage| &coverage.summary),
                )?;
            }
        }
        Action::Exec { .. } => unreachable!("exec was handled before workspace discovery"),
        Action::Downstream { .. } => unreachable!("downstream was handled before workspace discovery"),
    }
    Ok(())
}

fn is_narrowed(mode: &str, pull_request: bool, platforms: bool, stages: bool, tests: bool) -> bool {
    platforms || stages || tests || (mode == "fast" && !pull_request)
}

// Keep CI's environment payload below Windows' 32,767-character limit.
fn execution_plan(plan: &Value) -> Value {
    let mut selection = json!({"mode": plan["selection"]["mode"], "packages": plan["selection"]["packages"]});
    if let Some(base) = plan["selection"].get("base") {
        selection["base"] = base.clone();
    }
    if let Some(commands) = plan["selection"].get("unaffected_dependencies") {
        selection["unaffected_dependencies"] = commands.clone();
    }
    if let Some(filter) = plan["selection"].get("tests") {
        selection["tests"] = filter.clone();
    }
    let mut compact = json!({"selection": selection, "jobs": plan["jobs"]});
    for key in ["narrowed", "platforms", "stages"] {
        if let Some(value) = plan.get(key) {
            compact[key] = value.clone();
        }
    }
    compact
}

#[cfg(test)]
mod tests {
    #[test]
    fn landing_name_is_reserved_for_default_ci_coverage() {
        for (mode, pull_request, platforms, stages, filter, expected) in [
            ("exhaustive", false, false, false, false, false),
            ("fast", true, false, false, false, false),
            ("fast", false, false, false, false, true),
            ("exhaustive", false, true, false, false, true),
            ("exhaustive", false, false, true, false, true),
            ("exhaustive", false, false, false, true, true),
        ] {
            assert_eq!(
                super::is_narrowed(mode, pull_request, platforms, stages, filter),
                expected,
                "the requested scope decides which check name the run can use"
            );
        }
    }

    #[test]
    fn ci_payload_omits_descriptions_and_unbounded_reasons() {
        let plan = serde_json::json!({
            "selection": {"mode": "fast", "packages": ["gix"], "reasons": ["x".repeat(40_000)]},
            "tasks": ["command descriptions"], "jobs": [{"id": "tests@linux-x86_64"}]
        });
        assert_eq!(
            super::execution_plan(&plan),
            serde_json::json!({
                "selection": {"mode": "fast", "packages": ["gix"]}, "jobs": plan["jobs"]
            }),
            "job execution needs only selection and assignment, even for very large diffs"
        );
    }

    #[test]
    fn ci_payload_keeps_dependency_configuration_exclusions() {
        let plan = serde_json::json!({
            "selection": {"mode": "fast", "packages": ["gix"], "changed_packages": ["gix-archive"], "base": "base-commit",
                "unaffected_dependencies": [{"args": ["-p", "gix"], "dev": false}]},
            "jobs": []
        });
        let compact = super::execution_plan(&plan);
        assert_eq!(
            compact["selection"]["base"], "base-commit",
            "workers compare the same base for automatic coverage selection"
        );
        assert_eq!(
            compact["selection"]["unaffected_dependencies"], plan["selection"]["unaffected_dependencies"],
            "CI executes the same feature configurations selected locally"
        );
        assert!(
            compact["selection"].get("changed_packages").is_none(),
            "dependency queries already ran in the planning gate"
        );
    }

    #[test]
    fn ci_payload_keeps_the_requested_filter_and_scope() {
        let plan = serde_json::json!({
            "selection": {"mode": "fast", "packages": ["gix"], "tests": "test(example)"},
            "jobs": [], "narrowed": true, "platforms": ["macos-x86_64"], "stages": ["tests"]
        });
        assert_eq!(
            super::execution_plan(&plan),
            plan,
            "a worker replays the planned filter and scope"
        );
    }

    #[test]
    fn clap() -> gix_error::TestResult {
        use clap::{CommandFactory, Parser};
        let mut command = super::Args::command();
        command.clone().debug_assert();
        command.build();
        let help = command
            .find_subcommand_mut("run")
            .expect("QA has a run subcommand")
            .render_help()
            .to_string();
        assert!(
            help.contains("[possible values: all, self-test, policy,"),
            "--help lists the available stage selectors"
        );
        assert!(help.contains("--agent"), "agent output can be explicitly requested");
        for args in [
            vec!["qa", "--agent", "run", "--dry-run"],
            vec!["qa", "run", "--agent", "--dry-run"],
        ] {
            assert!(
                super::Args::try_parse_from(args)?.agent,
                "--agent works before or after the subcommand"
            );
        }
        Ok(())
    }
}
