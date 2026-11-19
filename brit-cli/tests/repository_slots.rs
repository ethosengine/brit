//! Regression coverage for build-command repository object-store sizing.

use std::{io::Write, path::Path, process::Stdio};

#[cfg(unix)]
use std::{os::unix::fs::MetadataExt, process::Command};

use gix_testtools::TestResult;

#[path = "../src/repository.rs"]
mod build_repository;

/// The build commands' error formatting.
#[path = "../src/error.rs"]
#[allow(dead_code, reason = "only the slot-exhaustion formatting is exercised here")]
mod build_error;

/// Stands in for a build command's outer message around a gix error.
#[derive(Debug, thiserror::Error)]
#[error("repository discovery failed")]
struct Wrapped {
    #[source]
    source: gix::Error,
}

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
    create_pack_indexes(repo, head, 33)
}

/// Put `head` in one pack and `total - 1` filler blobs in a pack each: `total` pack indexes.
fn create_pack_indexes(repo: &Path, head: &str, total: usize) -> TestResult {
    let mut paths = String::new();
    for index in 1..total {
        let name = format!("filler-{index}");
        std::fs::write(repo.join(&name), format!("filler object {index}\n"))?;
        paths.push_str(&name);
        paths.push('\n');
    }
    let ids = git_with_input(repo, &["hash-object", "-w", "--stdin-paths"], paths.as_bytes())?;
    let ids: Vec<_> = ids.lines().collect();
    assert_eq!(ids.len(), total - 1, "every filler file produces one loose object");

    for id in ids {
        git_with_input(
            repo,
            &["pack-objects", ".git/objects/pack/pack"],
            format!("{id}\n").as_bytes(),
        )?;
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
    assert_eq!(
        index_count, total,
        "fixture has more indexes than the former 32-slot cap"
    );
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
            .with(gix::sec::Trust::Full)
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
fn full_to_reduced_discovery_sizes_slots_from_the_indexes_present() -> TestResult {
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
    assert_eq!(
        repo.objects.store_ref().metrics().unused_slots,
        49,
        "final Reduced trust sizes slots from the 33 indexes present plus headroom"
    );
    repo.find_object(head_id)?;
    Ok(())
}

#[test]
fn reduced_trust_repository_reads_beyond_32_pack_indexes() -> TestResult {
    if gix_testtools::run_in_isolated_process()? {
        return Ok(());
    }
    for (indexes, slots) in [(33, 49), (100, 150)] {
        let fixture = gix_testtools::tempfile::tempdir()?;
        let repo_path = fixture.path();
        gix_testtools::git(repo_path, "init")?;
        gix_testtools::git(repo_path, "commit --allow-empty -m fixture")?;
        let head = gix_testtools::git(repo_path, "rev-parse HEAD")?;
        create_pack_indexes(repo_path, head.trim(), indexes)?;
        let head_id = gix::ObjectId::from_hex(head.trim().as_bytes())?;

        // No explicit slots: the size must come from the final Reduced trust alone.
        let reduced = gix::open_opts(repo_path, gix::open::Options::default().with(gix::sec::Trust::Reduced))?;
        assert_eq!(
            reduced.git_dir_trust(),
            gix::sec::Trust::Reduced,
            "the repository is opened with genuinely reduced trust"
        );
        assert_eq!(
            reduced.objects.store_ref().metrics().unused_slots,
            slots,
            "{indexes} indexes get their own slots plus headroom"
        );
        assert_eq!(
            reduced.rev_parse_single("HEAD")?.detach(),
            head_id,
            "reduced trust reads the commit in the last of {indexes} pack indexes"
        );
    }
    Ok(())
}

#[test]
fn reduced_trust_ceiling_is_one_named_error_with_both_cures() -> TestResult {
    // A global `safe.directory` (as on these devices) would promote the repository to full trust.
    if gix_testtools::run_in_isolated_process()? {
        return Ok(());
    }
    let fixture = gix_testtools::tempfile::tempdir()?;
    let repo_path = fixture.path();
    gix_testtools::git(repo_path, "init")?;
    gix_testtools::git(repo_path, "commit --allow-empty -m fixture")?;
    let pack_dir = repo_path.join(".git/objects/pack");
    // Pairs present at open are counted; a refused repository's indexes are never read.
    for index in 0..=gix::open::REDUCED_TRUST_MAX_PACK_INDEXES {
        std::fs::write(pack_dir.join(format!("pack-padding-{index}.idx")), [])?;
        std::fs::write(pack_dir.join(format!("pack-padding-{index}.pack")), [])?;
    }
    let err = gix::open_opts(repo_path, gix::open::Options::default().with(gix::sec::Trust::Reduced))
        .expect_err("more indexes than the ceiling are refused at reduced trust");
    let wrapped = Wrapped { source: err };
    let rendered = build_error::format_error(&wrapped);
    assert_eq!(
        rendered,
        named_slot_error(
            gix::open::REDUCED_TRUST_MAX_PACK_INDEXES + 1,
            gix::open::REDUCED_TRUST_MAX_PACK_INDEXES
        ),
        "one first-class error, not its wrapper"
    );
    Ok(())
}

fn named_slot_error(needed: usize, current: usize) -> String {
    format!(
        "object database index slots exhausted: the repository needs at least {needed} index slots and this process \
         may use {current}; its owner can consolidate its packs (`git repack -a -d`), or, if this checkout is \
         trusted, declare it with `safe.directory` (a global entry, `-c safe.directory=<path>` or GIT_CONFIG_COUNT)"
    )
}

fn open_reduced(repo_path: &Path) -> gix::Result<gix::Repository> {
    gix::open_opts(repo_path, gix::open::Options::default().with(gix::sec::Trust::Reduced))
}

fn fixture_with_pack_indexes(total: usize) -> TestResult<(gix_testtools::tempfile::TempDir, gix::ObjectId)> {
    let fixture = gix_testtools::tempfile::tempdir()?;
    let repo_path = fixture.path();
    gix_testtools::git(repo_path, "init")?;
    gix_testtools::git(repo_path, "commit --allow-empty -m fixture")?;
    let head = gix_testtools::git(repo_path, "rev-parse HEAD")?;
    create_pack_indexes(repo_path, head.trim(), total)?;
    let head_id = gix::ObjectId::from_hex(head.trim().as_bytes())?;
    Ok((fixture, head_id))
}

#[test]
fn reduced_trust_counts_a_multi_pack_index_as_one_slot() -> TestResult {
    if gix_testtools::run_in_isolated_process()? {
        return Ok(());
    }
    let (fixture, head_id) = fixture_with_pack_indexes(gix::open::REDUCED_TRUST_MAX_PACK_INDEXES + 88)?;
    gix_testtools::git(fixture.path(), "multi-pack-index write")?;
    let repo = open_reduced(fixture.path())?;
    assert_eq!(
        repo.objects.store_ref().metrics().unused_slots,
        32,
        "600 packs under one multi-pack index need one slot, as the store counts them"
    );
    assert_eq!(repo.rev_parse_single("HEAD")?.detach(), head_id);
    Ok(())
}

#[test]
fn reduced_trust_ceiling_leaves_no_headroom_and_a_new_pack_is_the_named_error() -> TestResult {
    if gix_testtools::run_in_isolated_process()? {
        return Ok(());
    }
    let ceiling = gix::open::REDUCED_TRUST_MAX_PACK_INDEXES;
    let (fixture, head_id) = fixture_with_pack_indexes(ceiling)?;
    let repo = open_reduced(fixture.path())?;
    assert_eq!(repo.objects.store_ref().metrics().unused_slots, ceiling);
    assert_eq!(
        repo.rev_parse_single("HEAD")?.detach(),
        head_id,
        "the ceiling itself opens"
    );

    // A concurrent fetch writes one more pack while the repository is open.
    std::fs::write(fixture.path().join("late"), "arrived while open\n")?;
    let late = gix_testtools::git(fixture.path(), "hash-object -w late")?;
    git_with_input(
        fixture.path(),
        &["pack-objects", ".git/objects/pack/pack"],
        format!("{}\n", late.trim()).as_bytes(),
    )?;
    gix_testtools::git(fixture.path(), "prune-packed")?;
    let late = gix::ObjectId::from_hex(late.trim().as_bytes())?;
    let err = repo
        .find_object(late)
        .expect_err("a store with no free slot cannot load the new pack");
    assert_eq!(
        build_error::format_error(&Wrapped { source: err }),
        named_slot_error(ceiling + 1, ceiling),
        "not a silent miss: the lookup fails with the named error and its cures"
    );
    Ok(())
}

#[test]
#[cfg(unix)]
fn an_unreadable_alternate_pack_directory_fails_the_first_lookup_not_a_reduced_trust_open() -> TestResult {
    use std::os::unix::fs::PermissionsExt;

    // A host `safe.directory = *` (as on hosted CI runners) would promote this repository to full trust,
    // which is a different path: see the full-trust assertion below.
    if gix_testtools::run_in_isolated_process()? {
        return Ok(());
    }
    let fixture = gix_testtools::tempfile::tempdir()?;
    let probe = fixture.path().join("probe");
    std::fs::write(&probe, "")?;
    if std::fs::metadata(&probe)?.uid() == 0 {
        // Root reads a mode-000 directory anyway, so the unreadable case cannot be staged.
        eprintln!("skipping: running as root, which ignores directory permissions");
        return Ok(());
    }
    let repo_path = fixture.path().join("repo");
    std::fs::create_dir(&repo_path)?;
    gix_testtools::git(&repo_path, "init")?;
    gix_testtools::git(&repo_path, "commit --allow-empty -m fixture")?;
    let head = gix::ObjectId::from_hex(gix_testtools::git(&repo_path, "rev-parse HEAD")?.trim().as_bytes())?;
    let alternate = fixture.path().join("alternate-objects");
    std::fs::create_dir_all(alternate.join("pack"))?;
    std::fs::write(
        repo_path.join(".git/objects/info/alternates"),
        format!("{}\n", alternate.display()),
    )?;
    std::fs::set_permissions(alternate.join("pack"), std::fs::Permissions::from_mode(0o000))?;
    let reduced = open_reduced(&repo_path);
    let lookup = reduced.as_ref().ok().map(|repo| repo.find_object(head).map(|_| ()));
    let full = gix::open_opts(&repo_path, gix::open::Options::default().with(gix::sec::Trust::Full));
    std::fs::set_permissions(alternate.join("pack"), std::fs::Permissions::from_mode(0o755))?;

    // Reduced trust, as in 0.2.6 (measured as a non-root user): the open succeeds, because reduced trust sizes its
    // slots without failing on an unreadable directory, and the first lookup fails naming the directory.
    let reduced = reduced?;
    assert_eq!(reduced.git_dir_trust(), gix::sec::Trust::Reduced);
    let err = lookup
        .expect("the open succeeded")
        .expect_err("not a silent partial view: the lookup reports the directory");
    assert!(
        format!("{err:?}").contains("Could not read pack directory"),
        "the first lookup names the unreadable directory: {err:?}"
    );

    // Full trust, also as in 0.2.6: the store sizes its slots by listing every pack directory and fails at open.
    let err = full.expect_err("full trust lists the pack directories at open");
    assert!(
        format!("{err:?}").contains("Could not read pack directory"),
        "the open names the unreadable directory: {err:?}"
    );
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
