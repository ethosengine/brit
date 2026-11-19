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

/// A superproject whose `elohim/rakia` is a gitlink and never checked out:
/// the pinned commit object exists nowhere on disk.
fn gitlink_superproject(repo: &std::path::Path) -> Vec<String> {
    gix_testtools::git(repo, "init").expect("init");
    std::fs::create_dir_all(repo.join("src")).expect("mkdir");
    std::fs::write(repo.join("src/lib.txt"), "local source\n").expect("write");
    std::fs::write(
        repo.join(".gitmodules"),
        "[submodule \"elohim/rakia\"]\n\tpath = elohim/rakia\n\turl = https://example.invalid/rakia.git\n",
    )
    .expect("write");
    std::fs::write(
        repo.join("build-manifest.json"),
        r#"{
            "manifestVersion":"1.0",
            "pipeline":"gitlink-fixture",
            "description":"steps that do and do not read a submodule",
            "steps":{
                "schemas":{
                    "description":"reads files pinned by the rakia gitlink",
                    "inputs":{"sources":["src/**","elohim/rakia/schemas/**","elohim/**"],"buildProcess":[]},
                    "outputs":{"artifacts":[],"verify":null},
                    "depends":[],
                    "executor":{"stage":"test","function":null}
                },
                "local":{
                    "description":"reads only superproject files",
                    "inputs":{"sources":["src/**","elohim/other/**"],"buildProcess":[]},
                    "outputs":{"artifacts":[],"verify":null},
                    "depends":[],
                    "executor":{"stage":"test","function":null}
                }
            },
            "gate":{},
            "deployment":{}
        }"#,
    )
    .expect("write");
    gix_testtools::git(repo, "add build-manifest.json .gitmodules src/lib.txt").expect("add");
    let mut commits = Vec::new();
    for pin in [
        "1111111111111111111111111111111111111111",
        "2222222222222222222222222222222222222222",
    ] {
        gix_testtools::git(
            repo,
            &format!("update-index --add --cacheinfo 160000,{pin},elohim/rakia"),
        )
        .expect("pin");
        gix_testtools::git(repo, &format!("commit -m pin-{}", &pin[..4])).expect("commit");
        commits.push(
            gix_testtools::git(repo, "rev-parse HEAD")
                .expect("rev-parse")
                .trim()
                .to_string(),
        );
    }
    commits
}

fn fingerprint_steps(repo: &std::path::Path, commit: &str) -> serde_json::Value {
    let mut command = Command::new(brit_binary());
    gix_testtools::configure_git_environment(&mut command, repo);
    let out = command
        .args(["build", "fingerprint"])
        .arg(repo.join("build-manifest.json"))
        .args(["--commit", commit])
        .current_dir(repo)
        .output()
        .expect("invoke brit build fingerprint");
    assert!(
        out.status.success(),
        "exit {} stderr: {}",
        out.status,
        String::from_utf8_lossy(&out.stderr)
    );
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).expect("json");
    v["fingerprints"]
        .as_array()
        .expect("fingerprints")
        .iter()
        .map(|fp| (fp["step"].as_str().expect("step").to_string(), fp.clone()))
        .collect::<serde_json::Map<_, _>>()
        .into()
}

#[test]
fn fingerprint_keys_a_crossed_gitlink_pin_without_the_submodule() {
    let dir = gix_testtools::tempfile::tempdir().expect("temp");
    let repo = dir.path();
    let commits = gitlink_superproject(repo);
    assert!(
        !repo.join("elohim/rakia/schemas").exists(),
        "submodule is not checked out"
    );

    let first = fingerprint_steps(repo, &commits[0]);
    let second = fingerprint_steps(repo, &commits[1]);

    // The crossing step re-keys when only the pin moves, and lists the gitlink once.
    assert_ne!(first["schemas"]["fingerprint"], second["schemas"]["fingerprint"]);
    assert_eq!(first["schemas"]["input_count"], 2, "src/lib.txt plus the gitlink");
    assert_eq!(
        first["schemas"]["gitlinks"],
        serde_json::json!([{"path": "elohim/rakia", "commit": "1111111111111111111111111111111111111111"}])
    );
    assert_eq!(
        second["schemas"]["gitlinks"][0]["commit"],
        "2222222222222222222222222222222222222222"
    );

    // A step whose globs cannot reach beneath the gitlink is unchanged and lists none.
    assert_eq!(first["local"]["fingerprint"], second["local"]["fingerprint"]);
    assert_eq!(first["local"]["input_count"], 1);
    assert!(
        first["local"].get("gitlinks").is_none(),
        "no gitlink listing: {}",
        first["local"]
    );
}

/// Record a symlink in the index (mode 120000; its blob is the target path).
fn stage_symlink(repo: &std::path::Path, link: &str, target: &str) {
    let scratch = repo.join(".git/link-target");
    std::fs::write(&scratch, target).expect("write target");
    let blob = gix_testtools::git(repo, &format!("hash-object -w {}", scratch.display())).expect("hash");
    gix_testtools::git(
        repo,
        &format!("update-index --add --cacheinfo 120000,{},{link}", blob.trim()),
    )
    .expect("link");
}

/// A repository whose last commit edits `src/lib.txt` (so `plan --since
/// HEAD~1` selects the step) and sets the rakia pin and the config link.
fn plan_fixture(repo: &std::path::Path, pin: &str, config_target: &str) {
    gix_testtools::git(repo, "init").expect("init");
    std::fs::create_dir_all(repo.join("src")).expect("mkdir");
    std::fs::write(repo.join("src/lib.txt"), "v1\n").expect("write");
    std::fs::write(repo.join("src/prod.json"), "{}\n").expect("write");
    std::fs::write(repo.join("src/dev.json"), "[]\n").expect("write");
    std::fs::write(
        repo.join(".gitmodules"),
        "[submodule \"elohim/rakia\"]\n\tpath = elohim/rakia\n\turl = https://example.invalid/rakia.git\n",
    )
    .expect("write");
    std::fs::write(
        repo.join("build-manifest.json"),
        r#"{
            "manifestVersion":"1.0",
            "pipeline":"plan-fixture",
            "description":"plan fingerprints over a gitlink and a symlink",
            "steps":{
                "schemas":{
                    "description":"reads src and the rakia schemas",
                    "inputs":{"sources":["src/**","elohim/rakia/schemas/**"],"buildProcess":[]},
                    "outputs":{"artifacts":[],"verify":null},
                    "depends":[],
                    "executor":{"stage":"test","function":null}
                }
            },
            "gate":{},
            "deployment":{}
        }"#,
    )
    .expect("write");
    gix_testtools::git(
        repo,
        "add build-manifest.json .gitmodules src/lib.txt src/prod.json src/dev.json",
    )
    .expect("add");
    gix_testtools::git(
        repo,
        "update-index --add --cacheinfo 160000,3333333333333333333333333333333333333333,elohim/rakia",
    )
    .expect("pin");
    stage_symlink(repo, "src/config.json", "prod.json");
    gix_testtools::git(repo, "commit -m base").expect("commit");

    std::fs::write(repo.join("src/lib.txt"), "v2\n").expect("write");
    gix_testtools::git(repo, "add src/lib.txt").expect("add");
    gix_testtools::git(repo, &format!("update-index --cacheinfo 160000,{pin},elohim/rakia")).expect("pin");
    stage_symlink(repo, "src/config.json", config_target);
    gix_testtools::git(repo, "commit -m change").expect("commit");
}

fn plan_fingerprint(pin: &str, config_target: &str) -> String {
    let dir = gix_testtools::tempfile::tempdir().expect("temp");
    let repo = dir.path();
    plan_fixture(repo, pin, config_target);
    let mut command = Command::new(brit_binary());
    gix_testtools::configure_git_environment(&mut command, repo);
    let out = command
        .args(["build", "plan", "--since", "HEAD~1", "--repo"])
        .arg(repo)
        .current_dir(repo)
        .output()
        .expect("invoke brit build plan");
    assert!(
        out.status.success(),
        "exit {} stderr: {}",
        out.status,
        String::from_utf8_lossy(&out.stderr)
    );
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).expect("json");
    assert_eq!(v["levels"][0][0]["qualifiedName"], "plan-fixture:schemas", "plan: {v}");
    v["levels"][0][0]["fingerprint"]
        .as_str()
        .expect("fingerprint")
        .to_string()
}

#[test]
fn plan_fingerprint_keys_a_reached_gitlink_and_a_matched_symlink() {
    let base = plan_fingerprint("1111111111111111111111111111111111111111", "prod.json");
    assert!(base.starts_with("bafyrei"), "{base}");
    let moved_pin = plan_fingerprint("2222222222222222222222222222222222222222", "prod.json");
    let retargeted = plan_fingerprint("1111111111111111111111111111111111111111", "dev.json");
    assert_ne!(base, moved_pin, "a pin move must re-key the planned step");
    assert_ne!(base, retargeted, "a symlink retarget must re-key the planned step");
    assert_eq!(
        base,
        plan_fingerprint("1111111111111111111111111111111111111111", "prod.json"),
        "same tree, same planned fingerprint"
    );
}

#[test]
fn fingerprint_lists_a_matched_symlink_by_its_object() {
    let dir = gix_testtools::tempfile::tempdir().expect("temp");
    let repo = dir.path();
    plan_fixture(repo, "1111111111111111111111111111111111111111", "dev.json");
    let v = fingerprint_steps(repo, "HEAD");
    let symlinks = v["schemas"]["symlinks"].as_array().expect("symlinks listed");
    assert_eq!(symlinks.len(), 1);
    assert_eq!(symlinks[0]["path"], "src/config.json");
    let object = gix_testtools::git(repo, "rev-parse HEAD:src/config.json").expect("rev-parse");
    assert_eq!(symlinks[0]["object"], object.trim());
    // src/lib.txt, prod.json, dev.json, the gitlink and the symlink.
    assert_eq!(v["schemas"]["input_count"], 5);
}
