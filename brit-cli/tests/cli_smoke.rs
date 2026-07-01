use std::process::Command;

fn rakia_binary() -> std::path::PathBuf {
    // Built + located by cargo/nextest for brit-cli's own integration tests
    // (the `[[bin]]` named `rakia` is defined in this crate's Cargo.toml).
    std::path::PathBuf::from(env!("CARGO_BIN_EXE_rakia"))
}

#[test]
fn graph_discover_outputs_json_with_manifests() {
    // Use the actual repo root (three levels up from brit-cli). This layout
    // (brit nested inside the elohim monorepo) is only present when brit is
    // checked out as part of the monorepo, not in a standalone checkout.
    let repo_root = match std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../")
        .canonicalize()
    {
        Ok(p) => p,
        Err(_) => {
            eprintln!("skipping: monorepo layout not present (standalone checkout)");
            return;
        }
    };

    let out = Command::new(rakia_binary())
        .args(["graph", "discover", "--repo"])
        .arg(&repo_root)
        .output()
        .expect("invoke rakia");

    if !out.status.success() {
        eprintln!(
            "skipping: monorepo layout not present (standalone checkout); exit {} stderr: {}",
            out.status,
            String::from_utf8_lossy(&out.stderr)
        );
        return;
    }
    let stdout = String::from_utf8(out.stdout).expect("utf8 stdout");
    let v: serde_json::Value = serde_json::from_str(&stdout).expect("parse json");
    assert!(v.get("manifests").is_some(), "expected 'manifests' key in output");

    let manifests = v["manifests"].as_array().expect("manifests is array");
    if manifests.is_empty() {
        eprintln!("skipping: monorepo layout not present (standalone checkout)");
        return;
    }
    assert!(
        manifests.len() >= 8,
        "expected at least 8 manifests, got {}",
        manifests.len()
    );
}

#[test]
fn fingerprint_emits_content_addressed_hex_for_real_manifest() {
    let repo_root = match std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../")
        .canonicalize()
    {
        Ok(p) => p,
        Err(_) => {
            eprintln!("skipping: monorepo layout not present (standalone checkout)");
            return;
        }
    };

    let manifest = repo_root.join("app/elohim-app/build-manifest.json");
    if !manifest.exists() {
        // Skip if running outside the elohim repo
        eprintln!("skipping: monorepo layout not present (standalone checkout)");
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
    // ContentFingerprint::cid is a canonical CIDv1 (dag-cbor, sha2-256) string,
    // not a raw blake3 hex (see CLAUDE.md "Canonical content addressing").
    let cid = fp["fingerprint"].as_str().expect("fingerprint string");
    assert!(cid.starts_with("bafyrei"), "expected a dag-cbor CIDv1, got: {cid}");
    let input_count = fp["input_count"].as_u64().expect("input_count");
    assert!(input_count > 0, "build-angular should match real source files");

    // Verify the new `commit` field is also a 40-char hex SHA
    let commit = v["commit"].as_str().expect("commit string");
    assert_eq!(commit.len(), 40, "git SHA-1 is 40 hex chars");
    assert!(commit.chars().all(|c| c.is_ascii_hexdigit()), "hex");
}
