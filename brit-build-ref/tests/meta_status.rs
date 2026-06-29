use std::process::Command;

#[test]
fn status_reports_dead_and_ok() {
    let tmp = tempfile::tempdir().unwrap();
    Command::new("git").arg("init").arg(tmp.path()).output().unwrap();
    let d = tmp.path().join("docs");
    std::fs::create_dir_all(&d).unwrap();
    std::fs::write(d.join("b.md"), "---\nid: doc-b\n---\nB body\n").unwrap();
    let bfp = {
        let c = std::fs::read_to_string(d.join("b.md")).unwrap();
        brit_epr::engine::frontmatter::drift_fingerprint(&c)
    };
    std::fs::write(d.join("a.md"), format!(
        "---\nid: doc-a\ncites:\n  - doc-b | b | {bfp}\n  - ghost | g | sha256:0000000000000000\n---\nA\n")).unwrap();

    let out = Command::new(env!("CARGO_BIN_EXE_brit-build-ref"))
        .arg("--repo").arg(tmp.path())
        .arg("meta").arg("status").arg("--dir").arg(&d)
        .output().unwrap();
    let s = String::from_utf8(out.stdout).unwrap();
    assert!(s.contains("dead") && s.contains("ghost"), "got: {s}");
    assert!(s.contains("ok") && s.contains("doc-b"), "got: {s}");
}
