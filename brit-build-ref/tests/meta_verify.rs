// brit-build-ref/tests/meta_verify.rs
use std::process::Command;

fn seal(tmp: &std::path::Path) -> String {
    Command::new("git").arg("init").arg(tmp).output().unwrap();
    let sub = tmp.join("docs");
    std::fs::create_dir_all(&sub).unwrap();
    std::fs::write(sub.join("a.md"), b"alpha").unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_brit-build-ref"))
        .arg("--repo")
        .arg(tmp)
        .arg("meta")
        .arg("seal")
        .arg("--dir")
        .arg(&sub)
        .output()
        .unwrap();
    String::from_utf8(out.stdout).unwrap().trim().to_string()
}

#[test]
fn verify_passes_for_sealed_cid() {
    let tmp = tempfile::tempdir().unwrap();
    let cid = seal(tmp.path());
    let out = Command::new(env!("CARGO_BIN_EXE_brit-build-ref"))
        .arg("--repo")
        .arg(tmp.path())
        .arg("meta")
        .arg("verify")
        .arg("--cid")
        .arg(&cid)
        .output()
        .unwrap();
    assert!(out.status.success(), "stderr: {}", String::from_utf8_lossy(&out.stderr));
}

#[test]
fn verify_fails_for_unknown_cid() {
    let tmp = tempfile::tempdir().unwrap();
    Command::new("git").arg("init").arg(tmp.path()).output().unwrap();
    // A syntactically valid CID that was never stored (parses, lookup fails).
    let absent = brit_epr::BritCid::compute(&[0xa1]).to_string();
    let out = Command::new(env!("CARGO_BIN_EXE_brit-build-ref"))
        .arg("--repo")
        .arg(tmp.path())
        .arg("meta")
        .arg("verify")
        .arg("--cid")
        .arg(&absent)
        .output()
        .unwrap();
    assert!(!out.status.success());
}
