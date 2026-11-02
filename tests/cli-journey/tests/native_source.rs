//! The public native capture is the same canonical tree as the Git bridge.
#![cfg(target_os = "linux")]

use cli_journey::support::{
    runner::{brit_bin, BritInvocation},
    test_repo::TestRepo,
};
use serde_json::Value;
use std::{
    fs,
    os::unix::{
        ffi::OsStringExt,
        fs::{symlink, PermissionsExt},
    },
    path::Path,
};

fn invoke(cwd: &Path, args: &[&str]) -> Value {
    let result = BritInvocation::new(brit_bin().expect("Brit executable"))
        .args(args.iter().copied())
        .current_dir(cwd)
        .run()
        .expect("invoke");
    assert!(result.status.success(), "{}", result.stderr);
    serde_json::from_str(&result.stdout).expect("JSON")
}

#[test]
fn native_capture_and_git_seal_have_one_root() {
    let repo = TestRepo::new("native-equivalence").expect("Git fixture");
    let private = tempfile::tempdir().expect("private files");
    let source = private.path().join("source");
    let store = private.path().join("store");
    fs::create_dir(&source).expect("source");
    fs::create_dir(source.join("nested")).expect("nested");
    fs::write(source.join("nested/value"), b"A\0binary\xff\n").expect("binary");
    fs::write(source.join("executable"), b"executable bytes").expect("executable");
    fs::set_permissions(source.join("executable"), fs::Permissions::from_mode(0o755)).expect("mode");
    symlink("nested/value", source.join("link")).expect("literal link");
    let byte_name = std::ffi::OsString::from_vec(vec![b'n', 0xff]);
    fs::write(source.join(&byte_name), b"byte filename").expect("byte name");
    fs::write(
        private.path().join(".git"),
        b"malformed Git metadata must not be discovered",
    )
    .expect("discovery trap");
    // Use OsString argv for the byte path, not a lossy string projection.
    let captured = BritInvocation::new(brit_bin().expect("Brit"))
        .args(["snapshot", "capture", "--source"])
        .arg(&source)
        .arg("--store")
        .arg(&store)
        .args(["--", "nested/value", "executable", "link"])
        .arg(&byte_name)
        .current_dir(private.path())
        .run()
        .expect("native capture");
    assert!(captured.status.success(), "{}", captured.stderr);
    let captured: Value = serde_json::from_str(&captured.stdout).expect("capture JSON");
    fs::remove_file(private.path().join(".git")).expect("remove discovery trap for existing Git-aware restore");
    assert_eq!(captured["published"], false);
    assert!(!source.join(".git").exists());
    fs::create_dir(repo.path().join("nested")).expect("Git nested");
    for path in [
        std::path::PathBuf::from("nested/value"),
        "executable".into(),
        byte_name.into(),
    ] {
        fs::copy(source.join(&path), repo.path().join(&path)).expect("identical Git bytes");
    }
    symlink("nested/value", repo.path().join("link")).expect("Git link");
    let add = gix_testtools::git_command(repo.path())
        .args(["add", "--", "nested/value", "executable", "link"])
        .output()
        .expect("Git stage");
    assert!(add.status.success());
    let add_byte = gix_testtools::git_command(repo.path())
        .arg("add")
        .arg("--")
        .arg(std::ffi::OsString::from_vec(vec![b'n', 0xff]))
        .output()
        .expect("byte stage");
    assert!(add_byte.status.success());
    let commit = gix_testtools::git_command(repo.path())
        .env("GIT_AUTHOR_NAME", "Fixture")
        .env("GIT_AUTHOR_EMAIL", "fixture@example.test")
        .env("GIT_COMMITTER_NAME", "Fixture")
        .env("GIT_COMMITTER_EMAIL", "fixture@example.test")
        .args(["commit", "-m", "identical native tree"])
        .output()
        .expect("Git fixture commit");
    assert!(commit.status.success(), "{}", String::from_utf8_lossy(&commit.stderr));
    let sealed = invoke(
        repo.path(),
        &["snapshot", "seal", "--store", store.to_str().expect("store")],
    );
    assert_eq!(captured["root"], sealed["root"]);
    fs::write(source.join("nested/value"), b"B independent edit").expect("edit B");
    let destination = private.path().join("restored");
    invoke(
        private.path(),
        &[
            "snapshot",
            "restore",
            "--root",
            captured["root"].as_str().expect("root"),
            "--store",
            store.to_str().expect("store"),
            "--destination",
            destination.to_str().expect("destination"),
        ],
    );
    assert_eq!(
        fs::read(destination.join("nested/value")).expect("A"),
        b"A\0binary\xff\n"
    );
    assert_eq!(fs::read(source.join("nested/value")).expect("B"), b"B independent edit");
}

#[test]
fn native_capture_refuses_selection_and_build_rejects_git_options() {
    let temp = tempfile::tempdir().expect("fixture");
    let source = temp.path().join("source");
    fs::create_dir(&source).expect("source");
    fs::write(source.join("value"), b"A").expect("value");
    for selection in [".", "../escape", "*.rs", ".git/config"] {
        let output = BritInvocation::new(brit_bin().expect("Brit"))
            .args([
                "snapshot", "capture", "--source", "source", "--store", "store", "--", selection,
            ])
            .current_dir(temp.path())
            .run()
            .expect("capture refusal");
        assert!(!output.status.success(), "{selection}");
        assert!(!temp.path().join("store").exists());
    }
    let output = BritInvocation::new(brit_bin().expect("Brit"))
        .args(["build", "run", "--repo", "."])
        .current_dir(temp.path())
        .run()
        .expect("native option refusal");
    assert!(!output.status.success());
    assert!(output.stderr.contains("--repo"));
}

#[test]
fn native_build_fixture() {
    if std::env::var_os("BRIT_NATIVE_FIXTURE").is_none() {
        return;
    }
    let input = std::path::PathBuf::from(std::env::var_os("INPUT").expect("input"));
    let output = std::path::PathBuf::from(std::env::var_os("OUTPUT").expect("output"));
    fs::write(output.join("ready"), b"ready").expect("ready");
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
    while !output.join("continue").exists() {
        assert!(std::time::Instant::now() < deadline, "continuation missing");
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    fs::write(output.join("answer"), fs::read(input.join("value")).expect("fixed A")).expect("answer");
}

/// Explicit integration lane: standalone Brit builds do not install an Ark runtime.
#[test]
#[ignore = "requires an explicitly supplied installed ARK_BIN; run this journey with --ignored"]
fn native_build_run_uses_ark_and_fixed_candidate() {
    use std::process::{Command, Stdio};
    let ark = std::path::PathBuf::from(std::env::var_os("ARK_BIN").expect("explicit Ark executable"));
    let temp = tempfile::tempdir().expect("fixture");
    let source = temp.path().join("source");
    let store = temp.path().join("store");
    let run = temp.path().join("run");
    fs::create_dir(&source).expect("source");
    fs::write(source.join("value"), b"A captured\n").expect("A");
    fs::write(
        temp.path().join(".git"),
        b"malformed Git metadata must not be discovered",
    )
    .expect("Git trap");
    let capture = invoke(
        temp.path(),
        &[
            "snapshot",
            "capture",
            "--source",
            source.to_str().expect("source"),
            "--store",
            store.to_str().expect("store"),
            "--",
            "value",
        ],
    );
    let artifact = std::env::current_exe().expect("native fixture");
    let digest = Command::new(&ark)
        .arg("hash")
        .arg(&artifact)
        .output()
        .expect("Ark artifact digest");
    assert!(digest.status.success(), "{}", String::from_utf8_lossy(&digest.stderr));
    let digest = String::from_utf8(digest.stdout).expect("digest").trim().to_owned();
    let request = serde_json::json!({
        "method":{"name":"brit-gate:public-native-journey", "step":"native", "deterministic":false},
        "child":{"artifact":{"pinned":{"sha256":digest}}, "argv":["{artifact}","--exact","native_build_fixture","--nocapture"],
            "env":{"PATH":"", "HOME":"{data_root}/home", "BRIT_NATIVE_FIXTURE":"1", "INPUT":"{input}", "OUTPUT":"{output}"}, "envScrub":true},
        "outputs":["answer"]
    });
    let request_path = temp.path().join("request.json");
    fs::write(&request_path, serde_json::to_vec(&request).expect("request")).expect("write request");
    let mut child = Command::new(brit_bin().expect("Brit"))
        .args(["build", "run", "--root", capture["root"].as_str().expect("root")])
        .arg("--store")
        .arg(&store)
        .arg("--request")
        .arg(request_path)
        .arg("--artifact")
        .arg(&artifact)
        .arg("--run-dir")
        .arg(&run)
        .env("ARK_BIN", &ark)
        .env("PATH", "")
        .current_dir(temp.path())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("public build run");
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
    while !run.join("output/ready").exists() {
        if child.try_wait().expect("poll").is_some() || std::time::Instant::now() >= deadline {
            let _ = child.kill();
            let failed = child.wait_with_output().expect("failed child");
            panic!("no synchronization: {}", String::from_utf8_lossy(&failed.stderr));
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    fs::write(source.join("value"), b"B independent edit\n").expect("B");
    fs::write(run.join("output/continue"), b"continue").expect("continue");
    let output = child.wait_with_output().expect("outcome");
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let outcome: Value = serde_json::from_slice(&output.stdout).expect("TaskOutcome");
    assert_eq!(outcome["key"]["input"]["cid"], capture["root"]);
    assert!(outcome["record"].is_null());
    assert_eq!(outcome["hit"], false);
    assert_eq!(fs::read(run.join("output/answer")).expect("answer"), b"A captured\n");
    assert_eq!(
        fs::read(source.join("value")).expect("B remains"),
        b"B independent edit\n"
    );
    assert!(outcome["output"].as_str().is_some());
}

#[test]
fn native_explicit_ark_selection_never_falls_back() {
    use std::process::Command;
    let temp = tempfile::tempdir().expect("fixture");
    // If fallback occurs this real executable would parse the forwarded arguments,
    // producing a different diagnostic. No shell or hook is involved.
    symlink(brit_bin().expect("Brit"), temp.path().join("ark")).expect("PATH decoy");
    for selection in ["", "missing-explicit-ark"] {
        let output = Command::new(brit_bin().expect("Brit"))
            .args([
                "build",
                "run",
                "--root",
                "unused",
                "--store",
                "store",
                "--request",
                "request",
                "--artifact",
                "artifact",
                "--run-dir",
                "run",
            ])
            .env("ARK_BIN", selection)
            .env("PATH", temp.path())
            .current_dir(temp.path())
            .output()
            .expect("selection refusal");
        assert!(!output.status.success());
        assert!(String::from_utf8_lossy(&output.stderr).contains("could not execute installed ark"));
        assert!(!temp.path().join("run").exists());
    }
}
