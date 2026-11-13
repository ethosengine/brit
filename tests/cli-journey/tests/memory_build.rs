//! Build fingerprints must release each selected blob without changing their existing CIDs.
use anyhow::{ensure, Result};
use cli_journey::support::{runner::brit_bin, test_repo::TestRepo};
use std::{fs, io::Write, path::Path, process::Command};

fn git(root: &Path, args: &[&str]) -> Result<Vec<u8>> {
    let output = gix_testtools::git_command(root)
        .args(["-c", &format!("safe.directory={}", root.display())])
        .args(args)
        .output()?;
    ensure!(
        output.status.success(),
        "Git {args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    Ok(output.stdout)
}

fn brit(root: &Path, args: &[&str]) -> Result<serde_json::Value> {
    eprintln!("qualifying build {args:?}");
    let output = brit_output(root, args)?;
    ensure!(
        output.status.success(),
        "Brit build {args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    Ok(serde_json::from_slice(&output.stdout)?)
}

fn brit_output(root: &Path, args: &[&str]) -> Result<std::process::Output> {
    let mut command = Command::new(brit_bin().expect("built Brit"));
    gix_testtools::configure_git_environment(&mut command, root);
    Ok(command.current_dir(root).arg("build").args(args).output()?)
}

fn unsupported_sha256_revision_build(root: &Path, command: &str, since: &str) -> Result<()> {
    // Published rakia-brit still uses gix 0.81, which cannot open SHA-256 repositories.
    // This refusal is independently reproduced with the pre-repair executable.
    let output = brit_output(root, &[command, "--since", since])?;
    ensure!(
        !output.status.success()
            && output.stdout.is_empty()
            && String::from_utf8_lossy(&output.stderr).contains("Failed to load the git configuration"),
        "existing SHA-256 revision build limitation: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    Ok(())
}

fn fingerprint(size: usize, count: usize, expected: &str, reduced: bool) -> Result<()> {
    let repo = TestRepo::new("memory-build")?;
    let root = repo.path();
    fs::create_dir(root.join("src"))?;
    let chunk = [b'x'; 65536];
    for n in 0..count {
        let path = format!("src/file{n:06}");
        let mut file = fs::File::create(root.join(&path))?;
        file.write_all(&[b'A' + (n % 26) as u8])?;
        let mut remaining = size - 1;
        while remaining != 0 {
            let amount = remaining.min(chunk.len());
            file.write_all(&chunk[..amount])?;
            remaining -= amount;
        }
        drop(file);
        git(root, &["add", &path])?;
    }
    git(root, &["update-index", "--chmod=+x", "src/file000000"])?;
    let link_input = root.join(".git/link-input");
    fs::write(&link_input, b"file000000")?;
    let link = String::from_utf8(git(root, &["hash-object", "-w", link_input.to_str().expect("path")])?)?;
    git(
        root,
        &[
            "update-index",
            "--add",
            "--cacheinfo",
            &format!("120000,{},src/ignored-link", link.trim()),
        ],
    )?;
    git(
        root,
        &[
            "update-index",
            "--add",
            "--cacheinfo",
            &format!("160000,{},src/ignored-submodule", repo.head_id()?),
        ],
    )?;
    let manifest = root.join("build-manifest.json");
    fs::write(
        &manifest,
        serde_json::to_vec(&serde_json::json!({
            "manifestVersion": "1.0", "pipeline": "memory", "description": "resource fixture",
            "steps": {"inspect": {
                "description": "fingerprint source", "inputs": {"sources": ["src/**"], "buildProcess": []},
                "outputs": {"artifacts": [], "verify": null}, "depends": [],
                "executor": {"stage": "inspect", "function": null}
            }}
        }))?,
    )?;
    git(root, &["add", "build-manifest.json"])?;
    git(root, &["commit", "-qm", "build inputs"])?;
    let head = repo.head_id()?;
    let index = fs::read(root.join(".git/index"))?;
    if reduced {
        #[cfg(target_os = "linux")]
        {
            use std::os::unix::fs::{chown, MetadataExt};
            ensure!(fs::metadata(root)?.uid() == 0, "qualification requires chown authority");
            chown(root, Some(65534), None)?;
            chown(root.join(".git"), Some(65534), None)?;
            let mut command = Command::new(brit_bin().expect("built Brit"));
            gix_testtools::configure_git_environment(&mut command, root);
            let output = command
                .current_dir(root)
                .args(["free", "trust", ".", ".git"])
                .output()?;
            ensure!(
                output.status.success()
                    && String::from_utf8_lossy(&output.stdout)
                        .lines()
                        .filter(|line| line.starts_with("Reduced"))
                        .count()
                        == 2,
                "ownership must produce actual reduced trust"
            );
        }
        #[cfg(not(target_os = "linux"))]
        anyhow::bail!("ownership qualification runs on Linux");
    }
    for packed in [false, true] {
        if packed {
            git(
                root,
                &["-c", "pack.threads=1", "-c", "pack.windowMemory=64m", "repack", "-ad"],
            )?;
            git(root, &["config", "--unset", "gitoxide.objects.allocLimit"])?;
        }
        for limit in [None, Some("16777216")] {
            if let Some(limit) = limit {
                git(root, &["config", "gitoxide.objects.allocLimit", limit])?;
            }
            eprintln!("fingerprint size={size}, count={count}, packed={packed}, limit={limit:?}");
            let result = brit(root, &["fingerprint", manifest.to_str().expect("path")])?;
            assert_eq!(result["fingerprints"][0]["fingerprint"], expected);
            assert_eq!(result["fingerprints"][0]["input_count"], count);
            if gix_testtools::object_hash().to_string() == "sha256" {
                unsupported_sha256_revision_build(root, "plan", "HEAD~1")?;
            } else {
                let plan = brit(root, &["plan", "--since", "HEAD~1"])?;
                assert_eq!(plan["levels"][0][0]["fingerprint"], expected);
                assert_eq!(plan["levels"][0][0]["qualifiedName"], "memory:inspect");
            }
        }
    }
    if !reduced {
        assert_eq!(
            brit(root, &["graph", "show"])?["nodes"]
                .as_array()
                .expect("nodes")
                .len(),
            1
        );
        if gix_testtools::object_hash().to_string() == "sha256" {
            unsupported_sha256_revision_build(root, "affected", "HEAD~1")?;
        } else {
            assert_eq!(
                brit(root, &["affected", "--since", "HEAD~1"])?["affected"][0]["qualified_name"],
                "memory:inspect"
            );
        }
        assert_eq!(
            brit(root, &["affected", "--files", "src/file000000"])?["affected"][0]["qualified_name"],
            "memory:inspect"
        );
    }
    ensure!(
        String::from_utf8(git(root, &["rev-parse", "HEAD"])?)?.trim() == head
            && fs::read(root.join(".git/index"))? == index,
        "build inspection preserves repository state"
    );
    Ok(())
}

#[test]
fn revision_build_hash_support() -> Result<()> {
    let repo = TestRepo::new("build-hash-support")?;
    for command in ["plan", "affected"] {
        if gix_testtools::object_hash().to_string() == "sha256" {
            unsupported_sha256_revision_build(repo.path(), command, "HEAD")?;
        } else {
            brit(repo.path(), &[command, "--since", "HEAD"])?;
        }
    }
    Ok(())
}

// Independent goldens: SHA-256 each fixed input; encode CIDv1/0x71/0x12/32 in lowercase
// base32; SHA-256 the sorted path + NUL + input CID + NUL sequence and encode identically.
// This preserves the existing fingerprint byte contract, including its historical codec.
#[test]
fn twenty_mib_fingerprint_and_plan() -> Result<()> {
    fingerprint(
        20 * 1024 * 1024,
        1,
        "bafyreibey7lihqqyhyhcx7crjhpn25lupwrb6otcjbndjh5drdhd4a6ygq",
        false,
    )
}

#[test]
fn three_256_mib_inputs_are_not_retained_together() -> Result<()> {
    fingerprint(
        256 * 1024 * 1024,
        3,
        "bafyreidys2k7jfknt2dtdsezpyucgijrblqwgp5l6id4o3pu5qzyyfsiwa",
        false,
    )
}

#[test]
fn many_small_inputs_exceed_the_former_boundary_in_aggregate() -> Result<()> {
    fingerprint(
        64 * 1024,
        512,
        "bafyreid3gtpmrf5y7l3bfma7xqbp42lqndh66naswz6ntg74psqg3vum4y",
        false,
    )
}

#[cfg(target_os = "linux")]
#[test]
#[ignore = "requires chown authority: qualification runs this explicitly with --ignored"]
fn actual_reduced_trust_fingerprints_stream_large_objects() -> Result<()> {
    fingerprint(
        20 * 1024 * 1024,
        1,
        "bafyreibey7lihqqyhyhcx7crjhpn25lupwrb6otcjbndjh5drdhd4a6ygq",
        true,
    )
}
