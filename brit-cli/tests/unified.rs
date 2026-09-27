//! The public package must supply one executable for Git and build operations.
use std::process::Command;

fn brit(args: &[&str]) -> std::process::Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_brit"));
    gix_testtools::configure_git_environment(&mut command, std::env::current_dir().expect("current directory"));
    command.args(args).output().expect("run freshly built brit")
}

#[test]
fn help_exposes_git_and_build_in_one_executable() {
    let output = brit(&["--help"]);
    assert!(output.status.success(), "help: {:?}", output);
    let help = String::from_utf8_lossy(&output.stdout);
    for command in ["status", "push", "build"] {
        assert!(help.contains(command), "missing {command} in {help}");
    }
}

#[test]
fn build_namespace_exposes_existing_operator_commands() {
    let output = brit(&["build", "--help"]);
    assert!(output.status.success(), "build help: {:?}", output);
    let help = String::from_utf8_lossy(&output.stdout);
    for command in ["graph", "affected", "plan", "fingerprint", "baseline"] {
        assert!(help.contains(command), "missing {command} in {help}");
    }
}

#[test]
fn build_graph_discovers_a_disposable_manifest() {
    let repo = gix_testtools::tempfile::tempdir().expect("temporary build graph");
    std::fs::write(
        repo.path().join("build-manifest.json"),
        r#"{"manifestVersion":"1.0","pipeline":"fixture","description":"fixture graph","steps":{},"gate":{},"deployment":{}}"#,
    )
    .expect("write disposable manifest");
    let mut command = Command::new(env!("CARGO_BIN_EXE_brit"));
    gix_testtools::configure_git_environment(&mut command, repo.path());
    let output = command
        .args(["build", "graph", "discover", "--repo"])
        .arg(repo.path())
        .output()
        .expect("discover fixture graph");
    assert!(output.status.success(), "discover: {:?}", output);
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).expect("valid graph JSON");
    assert_eq!(json["manifests"][0]["pipeline"], "fixture");
    assert_eq!(json["manifests"][0]["path"], "build-manifest.json");
}

#[test]
fn invalid_build_arguments_remain_usage_errors() {
    let output = brit(&["build", "plan"]);
    assert_eq!(output.status.code(), Some(2), "{:?}", output);
}

#[test]
fn git_repository_option_cannot_redirect_a_build_write() {
    let repo = gix_testtools::tempfile::tempdir().expect("temporary repository");
    gix_testtools::git(repo.path(), "init").expect("initialize fixture");
    let mut command = Command::new(env!("CARGO_BIN_EXE_brit"));
    gix_testtools::configure_git_environment(&mut command, repo.path());
    let output = command
        .arg("--repository")
        .arg(repo.path())
        .args([
            "build",
            "baseline",
            "write",
            "pipeline",
            "0000000000000000000000000000000000000000",
        ])
        .output()
        .expect("build usage refusal");
    assert_eq!(output.status.code(), Some(2), "{:?}", output);
    assert!(String::from_utf8_lossy(&output.stderr).contains("does not apply to `build`"));
    assert!(!repo.path().join(".git/refs/notes/rakia/baselines/pipeline").exists());

    let config = brit(&["-c", "core.abbrev=7", "build", "baseline", "read", "pipeline"]);
    assert_eq!(config.status.code(), Some(2), "{:?}", config);
    assert!(String::from_utf8_lossy(&config.stderr).contains("does not apply to `build`"));
}

#[test]
fn version_identifies_checkout_and_feature_family() {
    let output = brit(&["--version"]);
    assert!(output.status.success(), "version: {:?}", output);
    let version = String::from_utf8_lossy(&output.stdout);
    assert!(version.contains("source HEAD:"), "{version}");
    assert!(version.contains("source state:"), "{version}");
    assert!(version.contains("frontend presets:"), "{version}");
    if let Ok(head) = gix_testtools::git(env!("CARGO_MANIFEST_DIR"), "rev-parse HEAD") {
        assert!(
            version.contains(head.trim()),
            "build receipt must name current HEAD: {version}"
        );
    }
}

#[test]
fn completions_include_build_namespace() {
    let output = brit(&["completions", "--shell", "bash"]);
    assert!(output.status.success(), "completions: {:?}", output);
    let script = String::from_utf8_lossy(&output.stdout);
    assert!(script.contains("build"), "build is absent from generated completions");
}

#[test]
fn legacy_name_forwards_build_commands_with_a_warning() {
    let temp = gix_testtools::tempfile::tempdir().expect("temporary legacy executable");
    let legacy = temp.path().join(format!("rakia{}", std::env::consts::EXE_SUFFIX));
    // Test argv[0] dispatch without assuming Cargo's target and the fixture
    // directory share a filesystem (CI containers may mount them separately).
    std::fs::copy(env!("CARGO_BIN_EXE_brit"), &legacy).expect("copy same executable bytes");
    let mut command = Command::new(legacy);
    gix_testtools::configure_git_environment(&mut command, temp.path());
    let output = command.args(["graph", "--help"]).output().expect("legacy help");
    assert!(output.status.success(), "legacy help: {:?}", output);
    assert!(String::from_utf8_lossy(&output.stderr).contains("deprecated"));
    assert!(String::from_utf8_lossy(&output.stdout).contains("discover"));
}

#[test]
fn git_dispatch_operates_on_an_isolated_repository() {
    let repo = gix_testtools::tempfile::tempdir().expect("temporary repository");
    gix_testtools::git(repo.path(), "init").expect("initialize fixture");
    let mut command = Command::new(env!("CARGO_BIN_EXE_brit"));
    gix_testtools::configure_git_environment(&mut command, repo.path());
    let output = command
        .args(["status"])
        .current_dir(repo.path())
        .output()
        .expect("status");
    assert!(output.status.success(), "status: {:?}", output);
}

#[test]
fn git_push_dispatch_updates_a_disposable_remote() {
    let local = gix_testtools::tempfile::tempdir().expect("temporary local repository");
    let remote = gix_testtools::tempfile::tempdir().expect("temporary bare remote");
    gix_testtools::git(local.path(), "init").expect("initialize local repository");
    gix_testtools::git(local.path(), "commit --allow-empty -m initial").expect("commit local history");
    gix_testtools::git(remote.path(), "init --bare").expect("initialize remote repository");
    let head = gix_testtools::git(local.path(), "rev-parse HEAD").expect("local head");
    let branch = gix_testtools::git(local.path(), "symbolic-ref --short HEAD").expect("local branch");
    let refspec = format!("refs/heads/{}:refs/heads/main", branch.trim());

    let mut command = Command::new(env!("CARGO_BIN_EXE_brit"));
    gix_testtools::configure_git_environment(&mut command, local.path());
    let output = command
        .arg("push")
        .arg(remote.path())
        .arg(refspec)
        .current_dir(local.path())
        .output()
        .expect("push through unified brit");
    assert!(output.status.success(), "push: {:?}", output);
    let pushed = gix_testtools::git(remote.path(), "rev-parse refs/heads/main").expect("remote head");
    assert_eq!(head, pushed, "pushed ref must resolve to local head");
}
