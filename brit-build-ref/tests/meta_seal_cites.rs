use std::process::Command;

#[test]
fn seal_projects_cites_into_eprmeta() {
    let tmp = tempfile::tempdir().unwrap();
    Command::new("git").arg("init").arg(tmp.path()).output().unwrap();
    let docs = tmp.path().join("docs");
    std::fs::create_dir_all(&docs).unwrap();
    std::fs::write(
        docs.join("a.md"),
        "---\nid: doc-a\ncites:\n  - doc-b | needs b | sha256:0011223344556677\n---\n# A\nbody\n",
    )
    .unwrap();

    let out = Command::new(env!("CARGO_BIN_EXE_brit-build-ref"))
        .arg("--repo")
        .arg(tmp.path())
        .arg("meta")
        .arg("seal")
        .arg("--dir")
        .arg(&docs)
        .output()
        .unwrap();
    assert!(out.status.success(), "stderr: {}", String::from_utf8_lossy(&out.stderr));
    let cid = String::from_utf8(out.stdout).unwrap();
    let node_path = tmp.path().join(".git/brit/objects").join(cid.trim());
    let bytes = std::fs::read(node_path).unwrap();
    let meta: brit_epr::EprMeta = serde_ipld_dagcbor::from_slice(&bytes).unwrap();
    assert!(meta.exports.iter().any(|e| e.ref_ == "doc-a"));
    assert!(meta.imports.iter().any(|e| e.ref_ == "doc-b"));
}
