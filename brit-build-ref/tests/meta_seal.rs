use std::process::Command;

#[test]
fn seal_prints_bafyrei_and_stores_the_node() {
    let tmp = tempfile::tempdir().unwrap();
    // Make it a git repo so .git/brit/objects/ has a home.
    Command::new("git").arg("init").arg(tmp.path()).output().unwrap();
    let sub = tmp.path().join("docs");
    std::fs::create_dir_all(&sub).unwrap();
    std::fs::write(sub.join("a.md"), b"alpha").unwrap();
    std::fs::write(sub.join("b.md"), b"beta").unwrap();

    let out = Command::new(env!("CARGO_BIN_EXE_brit-build-ref"))
        .arg("--repo")
        .arg(tmp.path())
        .arg("meta")
        .arg("seal")
        .arg("--dir")
        .arg(&sub)
        .output()
        .unwrap();

    assert!(out.status.success(), "stderr: {}", String::from_utf8_lossy(&out.stderr));
    let cid = String::from_utf8(out.stdout).unwrap();
    assert!(cid.trim().starts_with("bafyrei"), "got {cid}");
    // The node was stored.
    let obj = tmp.path().join(".git/brit/objects").join(cid.trim());
    assert!(obj.exists(), "object not stored at {obj:?}");
}
