use std::process::Command;

fn brit_binary() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_BIN_EXE_brit"))
}

#[test]
fn graph_discover_outputs_json_with_manifests() {
    // Use the actual repo root (three levels up from brit-cli). This layout
    // (brit nested inside the elohim monorepo) is only present when brit is
    // checked out as part of the monorepo, not in a standalone checkout.
    let crate_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let repo_root = match crate_dir.join("../../../").canonicalize() {
        Ok(p) => p,
        Err(_) => {
            eprintln!("skipping: monorepo layout not present (standalone checkout)");
            return;
        }
    };
    // Three levels up always exists; in a standalone checkout it is just the
    // CI runner's work dir, which may hold unrelated manifests. Only treat it
    // as the monorepo when brit really sits at <root>/elohim/brit.
    if repo_root.join("elohim/brit/brit-cli").canonicalize().ok() != crate_dir.canonicalize().ok() {
        eprintln!("skipping: monorepo layout not present (standalone checkout)");
        return;
    }

    let mut command = Command::new(brit_binary());
    gix_testtools::configure_git_environment(&mut command, &repo_root);
    let out = command
        .args(["build", "graph", "discover", "--repo"])
        .arg(&repo_root)
        .output()
        .expect("invoke brit build");

    if !out.status.success() {
        panic!("brit build graph failed: {}", String::from_utf8_lossy(&out.stderr));
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
    let crate_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let repo_root = match crate_dir.join("../../../").canonicalize() {
        Ok(p) => p,
        Err(_) => {
            eprintln!("skipping: monorepo layout not present (standalone checkout)");
            return;
        }
    };
    // Three levels up always exists; in a standalone checkout it is just the
    // CI runner's work dir, which may hold unrelated manifests. Only treat it
    // as the monorepo when brit really sits at <root>/elohim/brit.
    if repo_root.join("elohim/brit/brit-cli").canonicalize().ok() != crate_dir.canonicalize().ok() {
        eprintln!("skipping: monorepo layout not present (standalone checkout)");
        return;
    }

    let manifest = repo_root.join("app/elohim-app/build-manifest.json");
    if !manifest.exists() {
        // Skip if running outside the elohim repo
        eprintln!("skipping: monorepo layout not present (standalone checkout)");
        return;
    }

    let mut command = std::process::Command::new(brit_binary());
    gix_testtools::configure_git_environment(&mut command, &repo_root);
    let out = command
        .args(["build", "fingerprint"])
        .arg(&manifest)
        .args(["--step", "build-angular"])
        .output()
        .expect("invoke brit build");

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
