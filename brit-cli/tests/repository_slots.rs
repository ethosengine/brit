//! Regression coverage for build-command repository object-store sizing.

use std::{io::Write, path::Path, process::Stdio};

#[cfg(unix)]
use std::{os::unix::fs::MetadataExt, process::Command};

use gix_testtools::TestResult;

#[path = "../src/repository.rs"]
mod build_repository;

fn git_with_input(repo: &Path, args: &[&str], input: &[u8]) -> TestResult<String> {
    let mut child = gix_testtools::git_command(repo)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    child
        .stdin
        .take()
        .expect("stdin was configured as piped")
        .write_all(input)?;
    let output = child.wait_with_output()?;
    if !output.status.success() {
        return Err(format!(
            "git {args:?} failed with {}: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr)
        )
        .into());
    }
    Ok(String::from_utf8(output.stdout)?)
}

fn create_many_pack_indexes(repo: &Path, head: &str) -> TestResult {
    let mut paths = String::new();
    for index in 0..64 {
        let name = format!("filler-{index}");
        std::fs::write(repo.join(&name), format!("filler object {index}\n"))?;
        paths.push_str(&name);
        paths.push('\n');
    }
    let ids = git_with_input(repo, &["hash-object", "-w", "--stdin-paths"], paths.as_bytes())?;
    let ids: Vec<_> = ids.lines().collect();
    assert_eq!(ids.len(), 64, "every filler file produces one loose object");

    for [first, second] in ids.as_chunks::<2>().0 {
        let input = format!("{first}\n{second}\n");
        git_with_input(repo, &["pack-objects", ".git/objects/pack/pack"], input.as_bytes())?;
    }
    git_with_input(
        repo,
        &["pack-objects", ".git/objects/pack/pack"],
        format!("{head}\n").as_bytes(),
    )?;
    gix_testtools::git(repo, "prune-packed")?;

    let index_count = std::fs::read_dir(repo.join(".git/objects/pack"))?
        .filter_map(Result::ok)
        .filter(|entry| entry.path().extension().is_some_and(|ext| ext == "idx"))
        .count();
    assert_eq!(index_count, 33, "fixture has more indexes than the reduced-trust cap");
    assert!(
        !repo.join(".git/objects/pack/multi-pack-index").exists(),
        "fixture must exercise individual index slots"
    );
    Ok(())
}

fn assert_slot_exhaustion(error: &gix::Error) {
    let capacity = error
        .metadata()
        .find(|values| values.contains_key("current") && values.contains_key("needed"))
        .unwrap_or_else(|| panic!("object lookup reports the gix-odb slot-capacity diagnostic; error: {error:?}"));
    assert_eq!(
        (capacity.get("current"), capacity.get("needed")),
        (
            Some(&gix::error::MetadataValue::from(32_usize)),
            Some(&gix::error::MetadataValue::from(1_usize))
        ),
        "a 32-slot store needs exactly one more slot for the 33rd pack index"
    );
}

fn create_fingerprint_repository(repo: &Path) -> TestResult<String> {
    gix_testtools::git(repo, "init")?;
    std::fs::write(repo.join("tracked.txt"), "fingerprinted content\n")?;
    std::fs::write(
        repo.join("build-manifest.json"),
        r#"{
            "manifestVersion":"1.0",
            "pipeline":"slot-regression",
            "description":"repository slot regression fixture",
            "steps":{
                "fingerprint":{
                    "description":"read one committed file",
                    "inputs":{"sources":["tracked.txt"],"buildProcess":[]},
                    "outputs":{"artifacts":[],"verify":null},
                    "depends":[],
                    "executor":{"stage":"test","function":null}
                }
            },
            "gate":{},
            "deployment":{}
        }"#,
    )?;
    gix_testtools::git(repo, "add build-manifest.json tracked.txt")?;
    gix_testtools::git(repo, "commit -m fixture")?;
    Ok(gix_testtools::git(repo, "rev-parse HEAD")?)
}

fn create_fingerprint_fixture(repo: &Path) -> TestResult<String> {
    let head = create_fingerprint_repository(repo)?;
    create_many_pack_indexes(repo, head.trim())?;
    Ok(head)
}

fn run_fingerprint(repo: &Path) -> TestResult<std::process::Output> {
    let mut command = std::process::Command::new(env!("CARGO_BIN_EXE_brit"));
    gix_testtools::configure_git_environment(&mut command, repo);
    Ok(command
        .args(["build", "fingerprint"])
        .arg(repo.join("build-manifest.json"))
        .current_dir(repo)
        .output()?)
}

#[cfg(unix)]
struct RestoreOwner<'a> {
    path: &'a Path,
    uid: u32,
}

#[cfg(unix)]
impl Drop for RestoreOwner<'_> {
    fn drop(&mut self) {
        let _ = std::os::unix::fs::chown(self.path, Some(self.uid), None);
    }
}

#[test]
fn discovery_resolves_head_beyond_what_a_32_slot_store_can_load() -> TestResult {
    if gix_testtools::run_in_isolated_process()? {
        return Ok(());
    }
    let fixture = gix_testtools::tempfile::tempdir()?;
    let repo_path = fixture.path();
    gix_testtools::git(repo_path, "init")?;
    gix_testtools::git(repo_path, "commit --allow-empty -m fixture")?;
    let head = gix_testtools::git(repo_path, "rev-parse HEAD")?;
    create_many_pack_indexes(repo_path, head.trim())?;
    let head_id = gix::ObjectId::from_hex(head.trim().as_bytes())?;

    let capped = gix::open_opts(
        repo_path,
        gix::open::Options::default()
            .with_trust(gix::sec::Trust::Full)
            .object_store_slots(gix::odb::store::init::Slots::Given(32)),
    )?;
    let err = capped
        .find_object(head_id)
        .expect_err("an explicitly capped object store cannot load the commit in the 33rd pack");
    assert_slot_exhaustion(&err);

    let repo = build_repository::discover(repo_path)?;
    assert_eq!(
        repo.git_dir_trust(),
        gix::sec::Trust::Full,
        "a repository owned by the current user is fully trusted"
    );
    assert_eq!(
        repo.rev_parse_single("HEAD")?.detach().to_hex().to_string(),
        head.trim(),
        "the discovery entry point used by build commands sizes its object store from disk"
    );
    Ok(())
}

#[test]
#[cfg(unix)]
fn full_to_reduced_discovery_keeps_the_32_slot_cap() -> TestResult {
    if gix_testtools::run_in_isolated_process()? {
        return Ok(());
    }
    let fixture = gix_testtools::tempfile::tempdir()?;
    let repo_path = fixture.path();
    gix_testtools::git(repo_path, "init")?;
    gix_testtools::git(repo_path, "commit --allow-empty -m fixture")?;
    let head = gix_testtools::git(repo_path, "rev-parse HEAD")?;
    create_many_pack_indexes(repo_path, head.trim())?;
    let head_id = gix::ObjectId::from_hex(head.trim().as_bytes())?;

    // Keep `.git` owned by the current user so discovery starts with Full
    // trust, but make the worktree foreign-owned so gix's final trust becomes
    // Reduced without a safe.directory authorization.
    let original_uid = std::fs::metadata(repo_path)?.uid();
    let foreign_uid = if original_uid == 65_534 { 0 } else { 65_534 };
    match std::os::unix::fs::chown(repo_path, Some(foreign_uid), None) {
        Ok(()) => {}
        Err(err)
            if matches!(
                err.kind(),
                std::io::ErrorKind::PermissionDenied | std::io::ErrorKind::InvalidInput
            ) =>
        {
            eprintln!("skipping foreign-ownership portion: {err}");
            return Ok(());
        }
        Err(err) => return Err(err.into()),
    }
    let _restore_owner = RestoreOwner {
        path: repo_path,
        uid: original_uid,
    };

    let repo = build_repository::discover(repo_path)?;
    assert_eq!(
        repo.git_dir_trust(),
        gix::sec::Trust::Reduced,
        "a current-owned git directory with a foreign-owned worktree is downgraded to Reduced"
    );
    let err = repo
        .find_object(head_id)
        .expect_err("final Reduced trust keeps the 32-slot cap");
    assert_slot_exhaustion(&err);
    Ok(())
}

#[test]
fn reduced_trust_repository_keeps_its_capped_store() -> TestResult {
    if gix_testtools::run_in_isolated_process()? {
        return Ok(());
    }
    let fixture = gix_testtools::tempfile::tempdir()?;
    let repo_path = fixture.path();
    gix_testtools::git(repo_path, "init")?;
    gix_testtools::git(repo_path, "commit --allow-empty -m fixture")?;
    let head = gix_testtools::git(repo_path, "rev-parse HEAD")?;
    create_many_pack_indexes(repo_path, head.trim())?;
    let head_id = gix::ObjectId::from_hex(head.trim().as_bytes())?;

    // No explicit slots: the cap must come from the final Reduced trust alone.
    let reduced = gix::open_opts(
        repo_path,
        gix::open::Options::default().with_trust(gix::sec::Trust::Reduced),
    )?;
    assert_eq!(
        reduced.git_dir_trust(),
        gix::sec::Trust::Reduced,
        "the repository is opened with genuinely reduced trust"
    );
    let err = reduced
        .find_object(head_id)
        .expect_err("reduced trust retains the 32-slot resource cap");
    assert_slot_exhaustion(&err);
    Ok(())
}

#[test]
fn fingerprint_command_resolves_head_with_more_than_32_pack_indexes() -> TestResult {
    let fixture = gix_testtools::tempfile::tempdir()?;
    let repo = fixture.path();
    let head = create_fingerprint_fixture(repo)?;
    let output = run_fingerprint(repo)?;

    assert!(
        output.status.success(),
        "fingerprint command resolves HEAD in a 33-pack repository: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value = serde_json::from_slice(&output.stdout)?;
    assert_eq!(
        json["commit"],
        head.trim(),
        "command-level coverage resolves the commit stored beyond the first 32 pack indexes"
    );
    Ok(())
}

#[test]
fn configuration_parse_errors_do_not_disclose_included_file_contents() -> TestResult {
    const UTF8_SECRET: &str = "UTF8_SYNTHETIC_SECRET_8f4d";
    const INVALID_UTF8_SECRET: &str = "INVALID_UTF8_SYNTHETIC_SECRET_13ac";

    for (name, contents, secret) in [
        (
            "utf8-secret.config",
            format!("[{UTF8_SECRET}\n").into_bytes(),
            UTF8_SECRET,
        ),
        (
            "invalid-utf8-secret.config",
            [b"[\xff".as_slice(), INVALID_UTF8_SECRET.as_bytes(), b"\n"].concat(),
            INVALID_UTF8_SECRET,
        ),
    ] {
        let fixture = gix_testtools::tempfile::tempdir()?;
        let repo = fixture.path();
        create_fingerprint_repository(repo)?;
        let included = repo.join(name);
        std::fs::write(&included, contents)?;
        let mut config = std::fs::OpenOptions::new()
            .append(true)
            .open(repo.join(".git/config"))?;
        writeln!(config, "\n[include]\n\tpath = {}", included.display())?;

        let output = run_fingerprint(repo)?;
        assert!(
            !output.status.success(),
            "a malformed included configuration must fail repository discovery"
        );
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            stderr.contains("repository discovery failed"),
            "the original outer discovery message remains visible: {stderr}"
        );
        assert!(
            !stderr.contains(secret),
            "included configuration contents must not be rendered: {stderr}"
        );
    }
    Ok(())
}

#[test]
#[cfg(unix)]
fn fingerprint_resolves_head_with_more_than_32_pack_indexes_after_safe_directory_promotion() -> TestResult {
    let fixture = gix_testtools::tempfile::tempdir()?;
    let repo = fixture.path();
    let head = create_fingerprint_fixture(repo)?;

    // Changing only the git-dir owner reproduces the production sequence:
    // ownership starts at Reduced, while a global safe.directory entry promotes
    // the repository's final trust to Full. Non-privileged test environments
    // cannot manufacture foreign ownership, so they retain the pure policy test.
    let git_dir = repo.join(".git");
    let original_uid = std::fs::metadata(&git_dir)?.uid();
    // Use the conventional `nobody` uid: some container filesystems reject
    // arbitrary unmapped numeric owners with EINVAL even for root.
    let foreign_uid = if original_uid == 65_534 { 0 } else { 65_534 };
    match std::os::unix::fs::chown(&git_dir, Some(foreign_uid), None) {
        Ok(()) => {}
        Err(err)
            if matches!(
                err.kind(),
                std::io::ErrorKind::PermissionDenied | std::io::ErrorKind::InvalidInput
            ) =>
        {
            eprintln!("skipping foreign-ownership portion: {err}");
            return Ok(());
        }
        Err(err) => return Err(err.into()),
    }
    let _restore_owner = RestoreOwner {
        path: &git_dir,
        uid: original_uid,
    };

    let global_config = repo.join("global.gitconfig");
    std::fs::write(&global_config, format!("[safe]\n\tdirectory = {}\n", repo.display()))?;
    let mut command = Command::new(env!("CARGO_BIN_EXE_brit"));
    gix_testtools::configure_git_environment(&mut command, repo);
    let output = command
        .env("GIT_CONFIG_GLOBAL", &global_config)
        .env_remove("SUDO_UID")
        .args(["build", "fingerprint"])
        .arg(repo.join("build-manifest.json"))
        .current_dir(repo)
        .output()?;

    assert!(
        output.status.success(),
        "fingerprint resolves HEAD after final trust enables disk-sized slots: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value = serde_json::from_slice(&output.stdout)?;
    assert_eq!(
        json["commit"],
        head.trim(),
        "fingerprint resolves the commit stored beyond the first 32 pack indexes"
    );
    Ok(())
}
