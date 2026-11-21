//! Source and build-process inputs stay distinct across affected plans and fingerprints.

use std::{ffi::OsString, path::Path};

use anyhow::{ensure, Context, Result};
use cli_journey::support::{
    runner::{brit_bin, BritInvocation, Capture},
    test_repo::TestRepo,
};

fn step(sources: &[&str], build_process: &[&str], depends: &[&str]) -> serde_json::Value {
    serde_json::json!({
        "description": "input-class fixture",
        "inputs": {
            "sources": sources,
            "buildProcess": build_process,
        },
        "outputs": {"artifacts": [], "verify": null},
        "depends": depends,
        "executor": {"stage": "test", "function": null},
    })
}

fn manifest(pipeline: &str, steps: serde_json::Value) -> String {
    serde_json::to_string_pretty(&serde_json::json!({
        "manifestVersion": "1.0",
        "pipeline": pipeline,
        "description": "input-class fixture",
        "steps": steps,
    }))
    .expect("fixture manifest serializes")
}

fn brit<I, A>(root: &Path, args: I) -> Result<Capture>
where
    I: IntoIterator<Item = A>,
    A: Into<OsString>,
{
    BritInvocation::new(brit_bin().expect("built Brit"))
        .arg("build")
        .args(args)
        .current_dir(root)
        .run()
}

fn affected(root: &Path, path: &str, inputs: Option<&str>) -> Result<serde_json::Value> {
    let mut args = vec![
        OsString::from("affected"),
        OsString::from("--files"),
        OsString::from(path),
    ];
    if let Some(inputs) = inputs {
        args.extend([OsString::from("--inputs"), OsString::from(inputs)]);
    }
    let output = brit(root, args)?;
    ensure!(output.status.success(), "affected {path} {inputs:?}: {}", output.stderr);
    serde_json::from_str(&output.stdout).context("affected JSON")
}

fn fingerprint(root: &Path, commit: &str, step: &str, inputs: Option<&str>) -> Result<String> {
    let mut args = vec![
        OsString::from("fingerprint"),
        root.join("build-manifest.json").into_os_string(),
        OsString::from("--step"),
        OsString::from(step),
        OsString::from("--commit"),
        OsString::from(commit),
    ];
    if let Some(inputs) = inputs {
        args.extend([OsString::from("--inputs"), OsString::from(inputs)]);
    }
    let output = brit(root, args)?;
    ensure!(
        output.status.success(),
        "fingerprint {step} at {commit} {inputs:?}: {}",
        output.stderr
    );
    let value: serde_json::Value = serde_json::from_str(&output.stdout).context("fingerprint JSON")?;
    value["fingerprints"][0]["fingerprint"]
        .as_str()
        .map(str::to_owned)
        .context("fingerprint CID")
}

fn input_classes<'a>(value: &'a serde_json::Value, qualified_name: &str) -> Vec<&'a str> {
    value["affected"]
        .as_array()
        .expect("affected array")
        .iter()
        .find(|step| step["qualified_name"] == qualified_name)
        .unwrap_or_else(|| panic!("missing affected step {qualified_name}: {value}"))["affected_by"]
        .as_array()
        .expect("affected reasons")
        .iter()
        .map(|reason| reason["inputClass"].as_str().expect("classified reason"))
        .collect()
}

#[test]
fn pipeline_only_change_preserves_legacy_output_and_sources_fingerprint() -> Result<()> {
    let repo = TestRepo::new("input-classes-pipeline")?;
    repo.commit_file("src/input.txt", "source\n")?;
    repo.commit_file("ci/pipeline.yml", "pipeline version 1\n")?;
    repo.commit_file(
        "build-manifest.json",
        &manifest(
            "fixture",
            serde_json::json!({"compile": step(&["src/**"], &["ci/pipeline.yml"], &[])}),
        ),
    )?;
    let before = repo.head_id()?;
    let after = repo.commit_file("ci/pipeline.yml", "pipeline version 2\n")?;

    let legacy = brit(repo.path(), ["affected", "--files", "ci/pipeline.yml"])?;
    ensure!(legacy.status.success(), "legacy affected: {}", legacy.stderr);
    assert_eq!(
        legacy.stdout,
        r#"{
  "changed_paths": [
    "ci/pipeline.yml"
  ],
  "affected": [
    {
      "qualified_name": "fixture:compile",
      "affected_by": [
        {
          "kind": "changedFile",
          "path": "ci/pipeline.yml"
        }
      ]
    }
  ]
}
"#,
        "omitting --inputs preserves the legacy JSON bytes"
    );

    let all_affected = affected(repo.path(), "ci/pipeline.yml", Some("all"))?;
    assert_eq!(input_classes(&all_affected, "fixture:compile"), ["buildProcess"]);
    assert!(
        affected(repo.path(), "ci/pipeline.yml", Some("sources"))?["affected"]
            .as_array()
            .expect("affected array")
            .is_empty(),
        "a build-process-only change does not select source inputs"
    );

    let sources_before = fingerprint(repo.path(), &before, "compile", Some("sources"))?;
    let sources_after = fingerprint(repo.path(), &after, "compile", Some("sources"))?;
    assert_eq!(
        sources_before, sources_after,
        "pipeline bytes are absent from the sources CID"
    );

    let all_before = fingerprint(repo.path(), &before, "compile", Some("all"))?;
    let all_after = fingerprint(repo.path(), &after, "compile", Some("all"))?;
    assert_ne!(all_before, all_after, "pipeline bytes remain part of the all-input CID");
    assert_eq!(
        fingerprint(repo.path(), &before, "compile", None)?,
        all_before,
        "default and explicit all agree before the change"
    );
    assert_eq!(
        fingerprint(repo.path(), &after, "compile", None)?,
        all_after,
        "default and explicit all agree after the change"
    );
    Ok(())
}

#[test]
fn source_only_change_moves_both_fingerprints_and_selects_both_modes() -> Result<()> {
    let repo = TestRepo::new("input-classes-source")?;
    repo.commit_file("src/input.txt", "source version 1\n")?;
    repo.commit_file("ci/pipeline.yml", "pipeline\n")?;
    repo.commit_file(
        "build-manifest.json",
        &manifest(
            "fixture",
            serde_json::json!({"compile": step(&["src/**"], &["ci/pipeline.yml"], &[])}),
        ),
    )?;
    let before = repo.head_id()?;
    let after = repo.commit_file("src/input.txt", "source version 2\n")?;

    for inputs in ["sources", "all"] {
        let selected = affected(repo.path(), "src/input.txt", Some(inputs))?;
        assert_eq!(input_classes(&selected, "fixture:compile"), ["sources"]);
        assert_ne!(
            fingerprint(repo.path(), &before, "compile", Some(inputs))?,
            fingerprint(repo.path(), &after, "compile", Some(inputs))?,
            "a source change moves the {inputs} fingerprint"
        );
    }
    Ok(())
}

#[test]
fn one_path_is_classified_per_step() -> Result<()> {
    let repo = TestRepo::new("input-classes-per-step")?;
    repo.commit_file("shared.txt", "shared\n")?;
    repo.commit_file("other/input.txt", "other\n")?;
    repo.commit_file(
        "build-manifest.json",
        &manifest(
            "fixture",
            serde_json::json!({
                "consumer": step(&["shared.txt"], &[], &[]),
                "executor": step(&["other/**"], &["shared.txt"], &[]),
            }),
        ),
    )?;

    let all = affected(repo.path(), "shared.txt", Some("all"))?;
    assert_eq!(input_classes(&all, "fixture:consumer"), ["sources"]);
    assert_eq!(input_classes(&all, "fixture:executor"), ["buildProcess"]);

    let sources = affected(repo.path(), "shared.txt", Some("sources"))?;
    assert_eq!(input_classes(&sources, "fixture:consumer"), ["sources"]);
    assert_eq!(sources["affected"].as_array().expect("affected array").len(), 1);
    Ok(())
}

#[test]
fn broad_source_glob_subtracts_its_own_pipeline_file() -> Result<()> {
    let repo = TestRepo::new("input-classes-broad-source")?;
    repo.commit_file("project/src/lib.rs", "source\n")?;
    repo.commit_file("project/Jenkinsfile", "pipeline version 1\n")?;
    repo.commit_file(
        "build-manifest.json",
        &manifest(
            "eprfs",
            serde_json::json!({
                "rust-build-test": step(&["project/**"], &["project/Jenkinsfile"], &[]),
            }),
        ),
    )?;
    let before = repo.head_id()?;
    let after = repo.commit_file("project/Jenkinsfile", "pipeline version 2\n")?;

    assert!(
        affected(repo.path(), "project/Jenkinsfile", Some("sources"))?["affected"]
            .as_array()
            .expect("affected array")
            .is_empty(),
        "the step's own build process is removed from its broad source glob"
    );
    assert_eq!(
        fingerprint(repo.path(), &before, "rust-build-test", Some("sources"))?,
        fingerprint(repo.path(), &after, "rust-build-test", Some("sources"))?,
        "the eprfs-shaped source CID ignores its own Jenkinsfile"
    );
    Ok(())
}

#[test]
fn sources_mode_does_not_propagate_from_build_process_only_step() -> Result<()> {
    let repo = TestRepo::new("input-classes-upstream")?;
    repo.commit_file("ci/base.yml", "pipeline\n")?;
    repo.commit_file("src/base/input.txt", "base\n")?;
    repo.commit_file("src/dependent/input.txt", "dependent\n")?;
    repo.commit_file(
        "build-manifest.json",
        &manifest(
            "fixture",
            serde_json::json!({
                "base": step(&["src/base/**"], &["ci/base.yml"], &[]),
                "dependent": step(&["src/dependent/**"], &[], &["base"]),
            }),
        ),
    )?;

    let all = affected(repo.path(), "ci/base.yml", Some("all"))?;
    assert_eq!(input_classes(&all, "fixture:base"), ["buildProcess"]);
    assert_eq!(input_classes(&all, "fixture:dependent"), ["upstream"]);

    assert!(
        affected(repo.path(), "ci/base.yml", Some("sources"))?["affected"]
            .as_array()
            .expect("affected array")
            .is_empty(),
        "dropping the build-process seed also drops its dependents"
    );
    Ok(())
}

#[test]
fn function_suffixed_pipeline_file_moves_the_all_key_it_selects_under() -> Result<()> {
    let repo = TestRepo::new("input-classes-function-suffix")?;
    repo.commit_file("src/input.txt", "source\n")?;
    repo.commit_file("ci/Jenkinsfile", "pipeline version 1\n")?;
    repo.commit_file(
        "build-manifest.json",
        &manifest(
            "fixture",
            serde_json::json!({"compile": step(&["src/**"], &["ci/Jenkinsfile@buildApp"], &[])}),
        ),
    )?;
    let before = repo.head_id()?;
    let after = repo.commit_file("ci/Jenkinsfile", "pipeline version 2\n")?;

    let all_affected = affected(repo.path(), "ci/Jenkinsfile", Some("all"))?;
    assert_eq!(input_classes(&all_affected, "fixture:compile"), ["buildProcess"]);
    assert_ne!(
        fingerprint(repo.path(), &before, "compile", Some("all"))?,
        fingerprint(repo.path(), &after, "compile", Some("all"))?,
        "a change that selects the step under all moves its all key"
    );
    assert_eq!(
        fingerprint(repo.path(), &before, "compile", Some("sources"))?,
        fingerprint(repo.path(), &after, "compile", Some("sources"))?,
        "the pipeline file stays out of the sources key"
    );
    assert_eq!(
        fingerprint(repo.path(), &before, "compile", None)?,
        fingerprint(repo.path(), &after, "compile", None)?,
        "flag omitted keeps the legacy union, whose suffixed glob matches no file"
    );
    Ok(())
}
