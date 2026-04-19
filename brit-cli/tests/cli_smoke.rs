use std::process::Command;

fn brit_binary() -> std::path::PathBuf {
    let manifest_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    // workspace target/debug/brit (brit-cli is a workspace member)
    manifest_dir.join("../target/debug/brit")
}

#[test]
fn graph_discover_outputs_json_with_manifests() {
    // Use the actual repo root (three levels up from brit-cli)
    let repo_root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../").canonicalize().unwrap();

    let out = Command::new(brit_binary())
        .args(["graph", "discover", "--repo"])
        .arg(&repo_root)
        .output()
        .expect("invoke brit");

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
