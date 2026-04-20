use std::process::Command;

fn rakia_binary() -> std::path::PathBuf {
    let manifest_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    // workspace target/debug/rakia (brit-cli is a workspace member, binary renamed from `brit`)
    manifest_dir.join("../target/debug/rakia")
}

#[test]
fn graph_discover_outputs_json_with_manifests() {
    // Use the actual repo root (three levels up from brit-cli)
    let repo_root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../").canonicalize().unwrap();

    let out = Command::new(rakia_binary())
        .args(["graph", "discover", "--repo"])
        .arg(&repo_root)
        .output()
        .expect("invoke rakia");

    assert!(out.status.success(),
        "exit {} stderr: {}",
        out.status,
        String::from_utf8_lossy(&out.stderr));
    let stdout = String::from_utf8(out.stdout).expect("utf8 stdout");
    let v: serde_json::Value = serde_json::from_str(&stdout).expect("parse json");
    assert!(v.get("manifests").is_some(), "expected 'manifests' key in output");

    let manifests = v["manifests"].as_array().expect("manifests is array");
    assert!(manifests.len() >= 8,
        "expected at least 8 manifests, got {}", manifests.len());
}

#[test]
fn fingerprint_emits_content_addressed_hex_for_real_manifest() {
    let repo_root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../").canonicalize().unwrap();

    let manifest = repo_root.join("app/elohim-app/build-manifest.json");
    if !manifest.exists() {
        // Skip if running outside the elohim repo
        return;
    }

    let out = std::process::Command::new(rakia_binary())
        .args(["fingerprint"])
        .arg(&manifest)
        .args(["--step", "build-angular"])
        .output()
        .expect("invoke rakia");

    assert!(
        out.status.success(),
        "exit {} stderr: {}",
        out.status,
        String::from_utf8_lossy(&out.stderr)
    );

    let stdout = String::from_utf8(out.stdout).expect("utf8");
    let v: serde_json::Value = serde_json::from_str(&stdout).expect("parse json");
    let fps = v["fingerprints"].as_array().expect("fingerprints array");
    assert_eq!(fps.len(), 1, "filtered to one step");

    let fp = &fps[0];
    assert_eq!(fp["step"], "build-angular");
    let hex = fp["fingerprint"].as_str().expect("fingerprint string");
    assert_eq!(hex.len(), 64, "blake3 hex is 64 chars");
    assert!(hex.chars().all(|c| c.is_ascii_hexdigit()), "hex");
    let input_count = fp["input_count"].as_u64().expect("input_count");
    assert!(input_count > 0, "build-angular should match real source files");

    // Verify the new `commit` field is also a 40-char hex SHA
    let commit = v["commit"].as_str().expect("commit string");
    assert_eq!(commit.len(), 40, "git SHA-1 is 40 hex chars");
    assert!(commit.chars().all(|c| c.is_ascii_hexdigit()), "hex");
}
