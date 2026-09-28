//! Native tree proof through the public executable, using isolated Git fixtures.

use cli_journey::support::{
    runner::{brit_bin, BritInvocation, Capture},
    test_repo::TestRepo,
};
use serde_json::Value;
use std::path::Path;

fn invoke(repo: &Path, args: &[&str]) -> Capture {
    BritInvocation::new(brit_bin().expect("required Brit executable"))
        .args(args.iter().copied())
        .current_dir(repo)
        .run()
        .expect("invoke brit")
}

fn json_ok(capture: Capture) -> Value {
    assert!(capture.status.success(), "{}", capture.stderr);
    serde_json::from_str(&capture.stdout).expect("machine-readable result")
}

#[test]
fn committed_tree_survives_source_loss_and_git_roundtrip() {
    let source = TestRepo::new("shared-tree-source").unwrap();
    source.commit_file("nested/file.txt", "canonical bytes\n").unwrap();
    // Deliberately leave unrelated uncommitted bytes: sealing must use the chosen revision.
    std::fs::write(source.path().join("nested/file.txt"), "dirty local edit").unwrap();
    std::fs::write(source.path().join("unrelated.txt"), "not selected").unwrap();
    let artifacts = tempfile::tempdir().unwrap();
    let store = artifacts.path().join("store");
    let store_arg = store.to_str().unwrap();
    let sealed = json_ok(invoke(source.path(), &["snapshot", "seal", "--store", store_arg]));
    let root = sealed["root"].as_str().unwrap();
    let oid = sealed["gitTree"].as_str().unwrap();
    assert_eq!(sealed["published"], false);
    assert_eq!(
        std::fs::read(source.path().join("nested/file.txt")).unwrap(),
        b"dirty local edit"
    );
    drop(source);

    let verified = json_ok(invoke(
        artifacts.path(),
        &["snapshot", "verify", "--root", root, "--store", store_arg],
    ));
    assert_eq!(verified["externalBoundaries"], 0);
    assert_eq!(verified["authorityVerified"], false);

    let destination = TestRepo::new("shared-tree-target").unwrap();
    let original_head = destination.head_id().unwrap();
    let exported = json_ok(invoke(
        destination.path(),
        &[
            "snapshot",
            "export-git",
            "--root",
            root,
            "--store",
            store_arg,
            "--repo",
            ".",
            "--expected-tree",
            oid,
        ],
    ));
    assert_eq!(exported["gitTree"], oid);
    assert_eq!(
        destination.head_id().unwrap(),
        original_head,
        "export must not move refs"
    );
    let oracle = gix_testtools::git_command(destination.path())
        .args(["show", &format!("{oid}:nested/file.txt")])
        .output()
        .unwrap();
    assert!(oracle.status.success());
    assert_eq!(oracle.stdout, b"canonical bytes\n");

    #[cfg(target_os = "linux")]
    {
        let restored = artifacts.path().join("restored");
        json_ok(invoke(
            artifacts.path(),
            &[
                "snapshot",
                "restore",
                "--root",
                root,
                "--store",
                store_arg,
                "--destination",
                restored.to_str().unwrap(),
            ],
        ));
        assert_eq!(
            std::fs::read(restored.join("nested/file.txt")).unwrap(),
            b"canonical bytes\n"
        );
        assert!(!restored.join("unrelated.txt").exists());
        let refused = invoke(
            artifacts.path(),
            &[
                "snapshot",
                "restore",
                "--root",
                root,
                "--store",
                store_arg,
                "--destination",
                restored.to_str().unwrap(),
            ],
        );
        assert!(!refused.status.success(), "never replace an existing destination");
        assert_eq!(
            std::fs::read(restored.join("nested/file.txt")).unwrap(),
            b"canonical bytes\n"
        );
    }
}

#[test]
fn missing_store_and_invalid_cid_fail_without_creating_destination() {
    let temp = tempfile::tempdir().unwrap();
    let store = temp.path().join("missing");
    let destination = temp.path().join("destination");
    let refused = invoke(
        temp.path(),
        &[
            "snapshot",
            "restore",
            "--root",
            "not-a-cid",
            "--store",
            store.to_str().unwrap(),
            "--destination",
            destination.to_str().unwrap(),
        ],
    );
    assert!(!refused.status.success());
    assert!(!store.exists());
    assert!(!destination.exists());
}
