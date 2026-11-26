use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Component, Path, PathBuf},
    process::Command,
};

use gix_error::{OptionExt, Result, ResultExt, bail, message};
use serde_json::{Value, json};

use super::{
    TestScope,
    ops::{git, output},
};

struct Package {
    name: String,
    directory: PathBuf,
    dependencies: BTreeSet<PathBuf>,
    dev_dependencies: BTreeSet<PathBuf>,
    extra_inputs: Vec<PathBuf>,
}

pub fn metadata(root: &Path) -> Result<Value> {
    let output = output(Command::new("cargo").current_dir(root).args([
        "metadata",
        "--no-deps",
        "--format-version",
        "1",
        "--locked",
    ]))?;
    serde_json::from_slice(&output).or_raise(|| message("Could not parse Cargo workspace metadata"))
}

pub fn plan(root: &Path, metadata: &Value, mode: &str, base: &str) -> Result<Value> {
    let packages = packages(metadata)?;
    if mode == "exhaustive" {
        return Ok(exhaustive(&packages, "Exhaustive mode was requested"));
    }
    if mode != "fast" {
        bail!(message!("Unknown QA mode: {mode}"));
    }

    let merge_base = git(root)?
        .args(["merge-base", "--", base, "HEAD"])
        .output()
        .or_raise(|| message("Could not find the Git merge base"))?;
    if !merge_base.status.success() {
        return Ok(exhaustive(
            &packages,
            &format!("Cannot compare with {base}; fetch the base ref to enable fast selection"),
        ));
    }
    let merge_base = std::str::from_utf8(&merge_base.stdout)
        .or_raise(|| message("Git returned an invalid merge base"))?
        .trim();
    let mut paths = BTreeSet::new();
    // Separate index and worktree diffs retain staged edits even if the worktree restores HEAD.
    for revisions in [vec![merge_base, "HEAD"], vec!["--cached", "HEAD"], vec![]] {
        let bytes = output(
            git(root)?
                .args([
                    "diff",
                    "--no-ext-diff",
                    "--no-textconv",
                    "--name-only",
                    "-z",
                    "--no-renames",
                ])
                .args(revisions)
                .arg("--"),
        )?;
        if !add_paths(&bytes, &mut paths) {
            return Ok(exhaustive(&packages, "A changed path is not valid UTF-8"));
        }
    }
    let untracked = output(git(root)?.args(["ls-files", "--others", "--exclude-standard", "-z"]))?;
    if !add_paths(&untracked, &mut paths) {
        return Ok(exhaustive(&packages, "An untracked path is not valid UTF-8"));
    }
    let mut plan = select(&packages, &paths);
    plan["base"] = merge_base.into();
    Ok(plan)
}

fn add_paths(bytes: &[u8], paths: &mut BTreeSet<PathBuf>) -> bool {
    for path in bytes.split(|byte| *byte == 0).filter(|path| !path.is_empty()) {
        let Ok(path) = std::str::from_utf8(path) else {
            return false;
        };
        paths.insert(path.into());
    }
    true
}

fn packages(metadata: &Value) -> Result<Vec<Package>> {
    let root = Path::new(
        metadata["workspace_root"]
            .as_str()
            .ok_or_raise(|| message("Cargo metadata is missing workspace_root"))?,
    );
    metadata["packages"]
        .as_array()
        .ok_or_raise(|| message("Cargo metadata is missing packages"))?
        .iter()
        .map(|package| {
            let name = package["name"]
                .as_str()
                .ok_or_raise(|| message("A Cargo package is missing its name"))?
                .to_owned();
            let manifest = Path::new(
                package["manifest_path"]
                    .as_str()
                    .ok_or_raise(|| message!("{name} is missing manifest_path"))?,
            );
            let directory = manifest
                .parent()
                .ok_or_raise(|| message!("{name} has no manifest directory"))?
                .strip_prefix(root)
                .or_raise(|| message!("{name} is outside the workspace root"))?
                .to_owned();
            let dependencies = package["dependencies"]
                .as_array()
                .ok_or_raise(|| message!("{name} is missing dependencies"))?;
            let workspace_dependencies = |dev| {
                dependencies
                    .iter()
                    .filter(|dependency| (dependency["kind"] == "dev") == dev)
                    .filter_map(|dependency| dependency["path"].as_str())
                    .filter_map(|path| Path::new(path).strip_prefix(root).ok())
                    .map(Path::to_owned)
                    .collect()
            };
            let mut extra_inputs = Vec::new();
            if let Some(inputs) = package["metadata"]["qa"].get("extra-inputs") {
                for input in inputs
                    .as_array()
                    .ok_or_raise(|| message!("{name}: qa.extra-inputs must be an array"))?
                {
                    let input = Path::new(
                        input
                            .as_str()
                            .ok_or_raise(|| message!("{name}: qa.extra-inputs entries must be paths"))?,
                    );
                    if input.as_os_str().is_empty()
                        || input.components().any(|part| !matches!(part, Component::Normal(_)))
                    {
                        bail!(message!(
                            "{name}: qa.extra-inputs must contain paths relative to the repository root"
                        ));
                    }
                    extra_inputs.push(input.to_owned());
                }
            }
            Ok(Package {
                name,
                directory,
                dependencies: workspace_dependencies(false),
                dev_dependencies: workspace_dependencies(true),
                extra_inputs,
            })
        })
        .collect()
}

pub(super) fn filtered(root: &Path, metadata: &Value, mode: &str, scope: TestScope) -> Result<Value> {
    let workspace = packages(metadata)?;
    let known: BTreeSet<_> = workspace.iter().map(|package| package.name.as_str()).collect();
    let validate = |name: &str| -> Result<()> {
        if !known.contains(name) {
            bail!(
                "Unknown workspace package '{name}'; valid packages: {}",
                known.iter().copied().collect::<Vec<_>>().join(", ")
            );
        }
        Ok(())
    };
    let mut selection = exhaustive(
        &workspace,
        "Test selection omits non-Nextest tasks and downstream checks",
    );
    selection["mode"] = mode.into();
    let mut reasons = Vec::new();
    let names = if scope.packages.is_empty() {
        let derived = if let Some(filter) = scope.tests.as_deref() {
            derive_packages(filter, &validate, || reverse_dependencies(root, metadata))?
        } else {
            None
        };
        if derived.is_some() {
            reasons.push("Derived package scope from the Nextest filterset".into());
        } else {
            reasons.push("The Nextest filterset has no safely derived package scope; searching the workspace".into());
        }
        derived
    } else {
        for name in &scope.packages {
            validate(name)?;
        }
        reasons.push("Restricted package scope to the requested workspace packages".into());
        Some(scope.packages.into_iter().collect())
    };
    if let Some(mut names) = names {
        if names.contains("cli-journey") && names.insert("brit-cli".into()) {
            reasons.push("Added brit-cli to build the brit executable required by cli-journey".into());
        }
        selection["package_scope"] = true.into();
        selection["packages"] = json!(names);
    }
    if let Some(filter) = scope.tests {
        selection["tests"] = filter.into();
    }
    if let Some(repeat) = scope.repeat {
        reasons.push(json!(format!(
            "Repeat each selected test {repeat} times with Nextest stress testing"
        )));
        selection["repeat"] = repeat.into();
    }
    selection["reasons"]
        .as_array_mut()
        .expect("selection reasons")
        .extend(reasons);
    Ok(selection)
}

// Recognize flat expressions with literal matchers. Any unrecognized syntax keeps
// workspace selection, so quoting, regexes and grouping remain Nextest's responsibility.
fn derive_packages(
    filter: &str,
    validate: &impl Fn(&str) -> Result<()>,
    reverse_dependencies: impl FnOnce() -> Option<BTreeMap<String, BTreeSet<String>>>,
) -> Result<Option<BTreeSet<String>>> {
    let mut rest = filter.trim();
    let mut groups = vec![Vec::new()];
    loop {
        let Some((atom, tail)) = rest.split_once(')') else {
            return Ok(None);
        };
        let Some((predicate, matcher)) = atom.split_once('(') else {
            return Ok(None);
        };
        let matcher = matcher.trim();
        if !matcher
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"_-=:#".contains(&byte))
        {
            return Ok(None);
        }
        match predicate.trim_end() {
            predicate @ ("package" | "rdeps") => {
                let name = matcher.strip_prefix(['=', '#']).unwrap_or(matcher);
                if name.is_empty()
                    || !name
                        .bytes()
                        .all(|byte| byte.is_ascii_alphanumeric() || b"-_".contains(&byte))
                {
                    return Ok(None);
                }
                groups.last_mut().expect("at least one group").push((predicate, name));
            }
            "test" | "binary" | "binary_id" | "platform" | "kind" | "group" | "all" | "none" | "default" | "deps" => {}
            _ => return Ok(None),
        }
        rest = tail.trim_start();
        if rest.is_empty() {
            break;
        }
        if let Some(tail) = rest.strip_prefix(['|', '+']).or_else(|| rest.strip_prefix("or ")) {
            groups.push(Vec::new());
            rest = tail.trim_start();
        } else if let Some(tail) = rest.strip_prefix('&').or_else(|| rest.strip_prefix("and ")) {
            rest = tail.trim_start();
        } else {
            return Ok(None);
        }
    }
    for (_, name) in groups.iter().flatten() {
        validate(name)?;
    }
    if groups.iter().any(Vec::is_empty) {
        return Ok(None);
    }
    let reverse = if groups.iter().flatten().any(|(predicate, _)| *predicate == "rdeps") {
        reverse_dependencies()
    } else {
        None
    };
    let mut selected = BTreeSet::new();
    for group in groups {
        // Any single positive bound is sufficient for an intersection.
        if let Some((_, name)) = group.iter().find(|(predicate, _)| *predicate == "package") {
            selected.insert((*name).into());
        } else if let Some(names) = reverse.as_ref().and_then(|graph| graph.get(group[0].1)) {
            selected.extend(names.iter().cloned());
        } else {
            return Ok(None);
        }
    }
    Ok(Some(selected))
}

fn reverse_dependencies(root: &Path, metadata: &Value) -> Option<BTreeMap<String, BTreeSet<String>>> {
    // All features and targets form a superset of the graph on each runner.
    let graph: Value = serde_json::from_slice(
        &output(Command::new("cargo").current_dir(root).args([
            "metadata",
            "--format-version",
            "1",
            "--locked",
            "--offline",
            "--all-features",
        ]))
        .ok()?,
    )
    .ok()?;
    let nodes = graph["resolve"]["nodes"].as_array()?;
    let members = metadata["packages"].as_array()?;
    let mut reverse = BTreeMap::<&str, Vec<&str>>::new();
    for node in nodes {
        for dependency in node["dependencies"].as_array()? {
            reverse
                .entry(dependency.as_str()?)
                .or_default()
                .push(node["id"].as_str()?);
        }
    }
    let mut packages = BTreeMap::new();
    for package in members {
        let id = package["id"].as_str()?;
        let mut visited = BTreeSet::from([id]);
        let mut pending = vec![id];
        while let Some(id) = pending.pop() {
            for &consumer in reverse.get(id).into_iter().flatten() {
                if visited.insert(consumer) {
                    pending.push(consumer);
                }
            }
        }
        let names = members
            .iter()
            .filter(|package| package["id"].as_str().is_some_and(|id| visited.contains(id)))
            .map(|package| package["name"].as_str().map(str::to_owned))
            .collect::<Option<_>>()?;
        packages.insert(package["name"].as_str()?.into(), names);
    }
    Some(packages)
}

fn exhaustive(packages: &[Package], reason: &str) -> Value {
    json!({
        "mode": "exhaustive",
        "packages": packages.iter().map(|package| package.name.as_str()).collect::<BTreeSet<_>>(),
        "reasons": [reason],
    })
}

fn select(packages: &[Package], paths: &BTreeSet<PathBuf>) -> Value {
    let mut selected = BTreeSet::new();
    let mut reasons = Vec::new();
    for path in paths {
        if global_input(path) {
            return exhaustive(
                packages,
                &format!("Global build or QA input changed: {}", path.display()),
            );
        }
        let mut known = false;
        for package in packages {
            if package.extra_inputs.iter().any(|input| path.starts_with(input)) {
                selected.insert(package.directory.clone());
                known = true;
            }
        }
        let owner = packages
            .iter()
            .filter(|package| {
                if package.directory.as_os_str().is_empty() {
                    path.starts_with("src") || path == Path::new("build.rs")
                } else {
                    path.starts_with(&package.directory)
                }
            })
            .max_by_key(|package| package.directory.components().count());
        if let Some(package) = owner {
            selected.insert(package.directory.clone());
            known = true;
        }
        if known {
            reasons.push(format!("Changed input: {}", path.display()));
        } else if !prose_only(path) {
            return exhaustive(packages, &format!("Unclassified input changed: {}", path.display()));
        }
    }
    let changed = selected.clone();
    loop {
        let previous_count = selected.len();
        for package in packages {
            if !selected.is_disjoint(&package.dependencies) {
                selected.insert(package.directory.clone());
            }
        }
        if selected.len() == previous_count {
            break;
        }
    }
    // Dev dependencies affect a consumer's tests, not the library used by its dependents.
    let affected = selected.clone();
    for package in packages {
        if !affected.is_disjoint(&package.dev_dependencies) {
            selected.insert(package.directory.clone());
        }
    }
    if selected.is_empty() {
        reasons.push("No build or test inputs changed".into());
    } else {
        reasons.push(
            "Includes reverse normal, build, optional, and target-specific dependencies, plus their dev consumers"
                .into(),
        );
    }
    json!({
        "mode": "fast",
        "changed_packages": packages.iter().filter(|package| changed.contains(&package.directory))
            .map(|package| package.name.as_str()).collect::<BTreeSet<_>>(),
        "packages": packages.iter().filter(|package| selected.contains(&package.directory))
            .map(|package| package.name.as_str()).collect::<BTreeSet<_>>(),
        "reasons": reasons,
    })
}

fn global_input(path: &Path) -> bool {
    matches!(
        path.file_name().and_then(|name| name.to_str()),
        Some("Cargo.toml" | "Cargo.lock")
    ) || path.components().any(|part| part.as_os_str() == "fuzz")
        || [
            ".github",
            ".config",
            ".cargo",
            "etc/scripts",
            "tests/it/src/qa",
            "tests/it/qa",
        ]
        .iter()
        .any(|prefix| path.starts_with(prefix))
        || matches!(path.to_str(), Some("justfile" | "tests/it/src/qa.rs"))
}

fn prose_only(path: &Path) -> bool {
    matches!(
        path.to_str(),
        Some(
            "AGENTS.md"
                | "CHANGELOG.md"
                | "CODE_OF_CONDUCT.md"
                | "COLLABORATING.md"
                | "CONTRIBUTING.md"
                | "DEVELOPMENT.md"
                | "README.md"
                | "SECURITY.md"
                | "SHORTCOMINGS.md"
                | "STABILITY.md"
                | "crate-status.md"
                | "general-tasks.md"
                | "tasks.md"
        )
    ) || (path.starts_with("etc/plan") && path.extension().is_some_and(|ext| ext == "md"))
}

#[cfg(test)]
mod tests {
    #[test]
    fn narrow_journey_selection_includes_brit_cli_once() -> gix_error::TestResult {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let metadata = super::metadata(&root)?;
        for (packages, tests, added) in [
            (vec!["cli-journey"], None, true),
            (vec!["cli-journey", "brit-cli"], None, false),
            (vec![], Some("package(=cli-journey)"), true),
        ] {
            let selection = super::filtered(
                &root,
                &metadata,
                "fast",
                super::TestScope {
                    packages: packages.into_iter().map(String::from).collect(),
                    tests: tests.map(String::from),
                    ..Default::default()
                },
            )?;
            assert_eq!(selection["packages"], json!(["brit-cli", "cli-journey"]));
            assert_eq!(
                selection["reasons"].to_string().contains("Added brit-cli"),
                added,
                "the plan explains when it adds the binary package"
            );
            let native = super::super::catalog::tasks(&metadata, &selection, "linux-x86_64")?
                .into_iter()
                .find(|task| task.id == "native-tests")
                .expect("the journey selection has a native test task");
            let packages = metadata["packages"].as_array().expect("workspace packages");
            for name in ["brit-cli", "cli-journey"] {
                let id = packages
                    .iter()
                    .find(|package| package["name"] == name)
                    .expect("workspace package")["id"]
                    .as_str()
                    .expect("workspace package ID");
                assert!(
                    native.commands[0].args.windows(2).any(|pair| pair == ["-p", id]),
                    "the native Nextest build includes {name}"
                );
            }
        }
        Ok(())
    }

    #[test]
    fn test_filter_derives_workspace_packages_only_when_safe() -> gix_error::TestResult {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let metadata = super::metadata(&root)?;
        let plan = |filter: &str| {
            super::filtered(
                &root,
                &metadata,
                "exhaustive",
                super::TestScope {
                    tests: Some(filter.into()),
                    ..Default::default()
                },
            )
        };
        for filter in ["package(brit-epr)", "test(example) & package(=brit-epr)"] {
            let plan = plan(filter)?;
            assert_eq!(plan["packages"], json!(["brit-epr"]), "{filter}");
            assert!(
                plan["reasons"].to_string().contains("Derived"),
                "the plan explains the derived scope"
            );
        }
        for filter in [
            "package(brit-epr) | test(example)",
            "not package(brit-epr)",
            "package(brit-*)",
        ] {
            let plan = plan(filter)?;
            assert!(
                plan["packages"].as_array().expect("workspace packages").len() > 1,
                "{filter}"
            );
        }
        let error = plan("package(unknown-plan-test)").expect_err("an unknown literal package must be refused");
        assert!(error.to_string().contains("unknown-plan-test"), "{error}");
        assert!(error.to_string().contains("brit-epr"), "{error}");
        let error = super::filtered(
            &root,
            &metadata,
            "exhaustive",
            super::TestScope {
                packages: vec!["unknown-plan-test".into()],
                ..Default::default()
            },
        )
        .expect_err("explicit package lists validate all names");
        assert!(error.to_string().contains("unknown-plan-test"), "{error}");
        assert!(error.to_string().contains("brit-epr"), "{error}");
        Ok(())
    }

    #[test]
    fn rdeps_uses_a_resolved_graph_or_keeps_workspace_selection() -> gix_error::TestResult {
        let filter = "rdeps(=api) & test(example) | package(other)";
        let validate = |_: &str| Ok(());
        let graph = Some([("api".into(), ["api".into(), "consumer".into()].into())].into());
        assert_eq!(
            super::derive_packages(filter, &validate, || graph)?,
            Some(["api".into(), "consumer".into(), "other".into()].into()),
            "reverse dependencies and the independent package arm form a safe bound"
        );
        assert_eq!(
            super::derive_packages(filter, &validate, || None)?,
            None,
            "missing dependency metadata must keep the workspace"
        );
        Ok(())
    }

    use super::*;
    use gix_error::TestResult;

    fn workspace() -> Result<Vec<Package>> {
        let root = Path::new("workspace");
        let mut entries = Vec::new();
        for (name, kind, optional, target) in [
            ("leaf", Value::Null, false, Value::Null),
            ("normal", Value::Null, false, Value::Null),
            ("build", json!("build"), false, Value::Null),
            ("dev", json!("dev"), false, Value::Null),
            ("optional", Value::Null, true, Value::Null),
            ("platform", Value::Null, false, json!("cfg(windows)")),
            ("unrelated", Value::Null, false, Value::Null),
        ] {
            entries.push(json!({
                "name": name,
                "manifest_path": root.join(name).join("Cargo.toml"),
                "dependencies": if matches!(name, "leaf" | "unrelated") { vec![] } else {
                    vec![json!({"path": root.join("leaf"), "kind": kind, "optional": optional, "target": target})]
                },
            }));
        }
        packages(&json!({"workspace_root": root, "packages": entries}))
    }

    #[test]
    fn all_dependency_kinds_are_reverse_dependencies() -> TestResult {
        let packages = workspace()?;
        let plan = select(&packages, &BTreeSet::from(["leaf/src/lib.rs".into()]));
        assert_eq!(plan["mode"], "fast", "A crate edit has a bounded dependency selection");
        assert_eq!(
            plan["packages"],
            json!(["build", "dev", "leaf", "normal", "optional", "platform"]),
            "Inactive features, dev dependencies, and other platforms remain relevant"
        );
        Ok(())
    }

    #[test]
    fn dev_consumers_do_not_propagate_but_normal_paths_still_do() -> TestResult {
        let mut packages = workspace()?;
        packages.push(Package {
            name: "dev-consumer".into(),
            directory: "dev-consumer".into(),
            dependencies: BTreeSet::from(["dev".into()]),
            dev_dependencies: BTreeSet::new(),
            extra_inputs: Vec::new(),
        });
        let changed = BTreeSet::from(["leaf/src/lib.rs".into()]);
        let plan = select(&packages, &changed);
        assert_eq!(
            plan["packages"],
            json!(["build", "dev", "leaf", "normal", "optional", "platform"]),
            "A changed dev dependency does not change the consumer's library"
        );
        assert_eq!(plan["changed_packages"], json!(["leaf"]), "Only input owners changed");

        packages[3].dependencies.insert("normal".into());
        let plan = select(&packages, &changed);
        assert_eq!(
            plan["packages"],
            json!(["build", "dev", "dev-consumer", "leaf", "normal", "optional", "platform"]),
            "A normal path still propagates when the same package also has a dev path"
        );
        let plan = select(&packages, &BTreeSet::from(["dev/src/lib.rs".into()]));
        assert_eq!(
            plan["packages"],
            json!(["dev", "dev-consumer"]),
            "Editing a dev consumer directly changes its library too"
        );
        Ok(())
    }

    #[test]
    fn deleted_paths_and_both_rename_ends_are_retained() -> TestResult {
        let mut paths = BTreeSet::new();
        assert!(
            add_paths(
                b"leaf/tests/deleted.rs\0unrelated/src/old name.rs\0normal/src/new\nname.rs\0",
                &mut paths
            ),
            "NUL-delimited names preserve spaces and newlines"
        );
        let plan = select(&workspace()?, &paths);
        assert_eq!(
            plan["packages"].as_array().map(Vec::len),
            Some(7),
            "Selection uses Git paths without requiring deleted or renamed files to exist"
        );
        assert!(
            !add_paths(b"leaf/\xff\0", &mut paths),
            "Non-UTF-8 input requires exhaustive mode"
        );
        Ok(())
    }

    #[test]
    fn global_and_unknown_paths_require_exhaustive_checks() -> TestResult {
        let mut packages = workspace()?;
        packages.push(Package {
            name: "internal-tools".into(),
            directory: "tests/it".into(),
            dependencies: BTreeSet::new(),
            dev_dependencies: BTreeSet::new(),
            extra_inputs: Vec::new(),
        });
        for path in [
            "Cargo.lock",
            "leaf/Cargo.toml",
            "justfile",
            ".config/nextest.toml",
            "leaf/fuzz/fuzz_targets/parse.rs",
            "tests/it/src/qa/plan.rs",
            "tests/it/qa/check-features.sh",
            "tests/helpers.sh",
            "unknown-crate/src/lib.rs",
            "etc/plan/reproduce.sh",
            "etc/plan/Cargo.toml",
            "etc/planning/design.md",
        ] {
            let plan = select(&packages, &BTreeSet::from([path.into()]));
            assert_eq!(
                plan["mode"], "exhaustive",
                "Unclassified or global path {path} cannot skip checks"
            );
        }
        for path in [
            "DEVELOPMENT.md",
            "etc/plan/qa-test-selection.md",
            "etc/plan/performance.md",
            "etc/plan/nested/design.md",
        ] {
            let plan = select(&packages, &BTreeSet::from([path.into()]));
            assert_eq!(plan["mode"], "fast", "Known prose {path} keeps fast selection");
            assert_eq!(plan["packages"], json!([]), "Known prose {path} needs no Rust builds");
        }
        Ok(())
    }

    #[test]
    fn extra_inputs_select_consumers_and_their_dependents() -> TestResult {
        let mut packages = workspace()?;
        packages[0].extra_inputs.push("unrelated/tests/fixtures".into());
        let plan = select(&packages, &BTreeSet::from(["unrelated/tests/fixtures/input".into()]));
        assert_eq!(
            plan["changed_packages"],
            json!(["leaf", "unrelated"]),
            "Shared fixture consumers are direct inputs alongside the owning crate"
        );
        assert_eq!(
            plan["packages"].as_array().map(Vec::len),
            Some(7),
            "Shared fixtures propagate to consumers"
        );
        let plan = select(
            &packages,
            &BTreeSet::from(["unrelated/tests/fixtures-other/input".into()]),
        );
        assert_eq!(
            plan["packages"],
            json!(["unrelated"]),
            "Input prefixes match whole path components"
        );
        packages[0].extra_inputs.push("etc/plan/input.md".into());
        let plan = select(&packages, &BTreeSet::from(["etc/plan/input.md".into()]));
        assert_eq!(
            plan["changed_packages"],
            json!(["leaf"]),
            "Explicit build or test inputs take precedence over prose classification"
        );
        Ok(())
    }

    #[test]
    fn fallback_selects_every_package_and_explains_why() -> TestResult {
        let plan = exhaustive(&workspace()?, "Missing comparison base");
        assert_eq!(
            plan["mode"], "exhaustive",
            "Missing history must not silently skip work"
        );
        assert_eq!(
            plan["packages"].as_array().map(Vec::len),
            Some(7),
            "Fallback includes every crate"
        );
        assert_eq!(
            plan["reasons"],
            json!(["Missing comparison base"]),
            "Fallback is reviewable"
        );
        Ok(())
    }
}
