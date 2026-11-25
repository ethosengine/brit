//! Preserve packed objects used by other worktrees and repositories across maintenance.
use std::{
    fmt::Write as _,
    fs,
    io::Write,
    path::Path,
    process::{Command, Output, Stdio},
};

use anyhow::{ensure, Context, Result};
use cli_journey::support::{runner::brit_bin, test_repo::TestRepo};

fn checked(out: Output, args: &[&str]) -> Result<Output> {
    ensure!(
        out.status.success(),
        "{args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    Ok(out)
}

fn git(path: &Path, args: &[&str]) -> Result<String> {
    let out = gix_testtools::git_command(path).args(args).output()?;
    Ok(String::from_utf8(checked(out, args)?.stdout)?)
}

fn brit_command(path: &Path) -> Command {
    let mut cmd = Command::new(brit_bin().expect("Brit binary"));
    gix_testtools::configure_git_environment(&mut cmd, path);
    cmd.current_dir(path).env("BRIT_QUIET", "1");
    cmd
}

fn brit(path: &Path, args: &[&str]) -> Result<Output> {
    checked(brit_command(path).args(args).output()?, args)
}

fn git_input(path: &Path, args: &[&str], input: &str) -> Result<String> {
    let mut stdin = tempfile::NamedTempFile::new()?;
    stdin.write_all(input.as_bytes())?;
    let out = gix_testtools::git_command(path)
        .args(args)
        .stdin(Stdio::from(stdin.reopen()?))
        .output()?;
    Ok(String::from_utf8(checked(out, args)?.stdout)?)
}

fn fixture(packs: usize) -> Result<TestRepo> {
    let repo = TestRepo::new("maintenance")?;
    git(repo.path(), &["config", "gc.auto", "0"])?;
    for n in 0..packs {
        repo.commit_file("tracked", &format!("version {n}\n"))?;
        git(repo.path(), &["repack", "-d"])?;
    }
    assert_eq!(pack_count(repo.path())?, packs, "fixture creates one pack per repack");
    Ok(repo)
}

fn pack_count(path: &Path) -> Result<usize> {
    pack_files(path, "idx")
}

fn pack_files(path: &Path, extension: &str) -> Result<usize> {
    Ok(fs::read_dir(path.join(".git/objects/pack"))?
        .collect::<std::io::Result<Vec<_>>>()?
        .iter()
        .filter(|entry| entry.path().extension().is_some_and(|ext| ext == extension))
        .count())
}

fn reduced(path: &Path, legacy: bool) -> gix::Result<gix::Repository> {
    let mut opts = gix::open::Options::isolated().with_trust(gix::sec::Trust::Reduced);
    if legacy {
        opts = opts.object_store_slots(gix::odb::store::init::Slots::Given(32));
    }
    let repo = gix::open_opts(path, opts)?;
    assert_eq!(repo.git_dir_trust(), gix::sec::Trust::Reduced);
    Ok(repo)
}

fn assert_reads(repo: &gix::Repository, listing: &str) -> Result<()> {
    for line in listing.lines() {
        let id = gix::ObjectId::from_hex(line.split(' ').next().context("Git object listing")?.as_bytes())?;
        repo.find_object(id)
            .with_context(|| format!("read packed object {id}"))?;
    }
    Ok(())
}

#[test]
fn forty_packs_become_readable_with_legacy_reduced_slots() -> Result<()> {
    let repo = fixture(40)?;
    let path = repo.path();
    let objects = git(path, &["rev-list", "--objects", "--all"])?;
    let head = gix::ObjectId::from_hex(repo.head_id()?.as_bytes())?;
    assert!(
        reduced(path, true)?.find_object(head).is_err(),
        "40 indexes exceed the old 32 slots"
    );
    assert_reads(&reduced(path, false)?, &objects)?;

    let plan = brit(path, &["maintenance", "--dry-run"])?;
    let plan = String::from_utf8(plan.stdout)?;
    assert!(
        plan.contains("40 packs now, 8 after") && plan.contains("bytes"),
        "{plan}"
    );
    assert_eq!(pack_count(path)?, 40, "dry run leaves source packs in place");
    assert!(
        !path.join(".git/objects/pack/.brit-maintenance.lock").exists(),
        "dry run creates no maintenance metadata"
    );
    assert!(
        !path.join(".git/brit-objects.pin").exists(),
        "dry run leaves the pin untouched"
    );

    brit(path, &["maintenance"])?;
    assert_eq!(pack_count(path)?, 8, "maintenance reaches its default target");
    assert_reads(&reduced(path, true)?, &objects)?;
    git(path, &["fsck", "--full"])?;
    Ok(())
}

#[cfg(unix)]
#[test]
fn maintenance_preserves_pack_read_permissions() -> Result<()> {
    use std::{collections::BTreeMap, os::unix::fs::PermissionsExt};

    let repo = fixture(12)?;
    let path = repo.path();
    let dir = path.join(".git/objects/pack");
    let mut indexes: Vec<_> = fs::read_dir(&dir)?
        .collect::<std::io::Result<Vec<_>>>()?
        .into_iter()
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "idx"))
        .collect();
    indexes.sort();
    for (n, index) in indexes.iter().enumerate() {
        let mode = if n < 6 { 0o600 } else { 0o444 };
        for ext in ["idx", "pack"] {
            fs::set_permissions(index.with_extension(ext), fs::Permissions::from_mode(mode))?;
        }
    }
    let permissions_by_object = || -> Result<_> {
        let mut modes = BTreeMap::new();
        for entry in fs::read_dir(&dir)? {
            let index = entry?.path();
            if index.extension().is_some_and(|ext| ext == "idx") {
                let pack_mode = index.with_extension("pack").metadata()?.permissions().mode() & 0o444;
                let index_mode = index.metadata()?.permissions().mode() & 0o444;
                let idx = gix::odb::pack::index::File::at(index, gix::hash::Kind::Sha1)?;
                for obj in idx.iter() {
                    modes
                        .entry(obj.oid)
                        .or_insert_with(Vec::new)
                        .push((pack_mode, index_mode));
                }
            }
        }
        for modes in modes.values_mut() {
            modes.sort_unstable();
            modes.dedup();
        }
        Ok(modes)
    };
    let before = permissions_by_object()?;
    brit(path, &["-c", "brit.maintenanceTarget=2", "maintenance"])?;
    assert_eq!(pack_count(path)?, 7, "first run consolidates one access class");
    assert_eq!(permissions_by_object()?, before);
    brit(path, &["-c", "brit.maintenanceTarget=2", "maintenance"])?;
    assert_eq!(pack_count(path)?, 2, "second run consolidates the other class");
    assert_eq!(permissions_by_object()?, before);
    Ok(())
}

#[test]
fn maintenance_recovers_a_store_over_the_current_reduced_trust_ceiling() -> Result<()> {
    let repo = fixture(1)?;
    let path = repo.path();
    let cap = gix::open::REDUCED_TRUST_MAX_PACK_INDEXES;
    let mut files = String::new();
    for n in 0..cap {
        let name = format!("filler-{n}");
        fs::write(path.join(&name), &name)?;
        writeln!(&mut files, "{name}")?;
    }
    let ids = git_input(path, &["hash-object", "-w", "--stdin-paths"], &files)?;
    let pack_args = ["pack-objects", ".git/objects/pack/pack"];
    for id in ids.lines() {
        git_input(path, &pack_args, &format!("{id}\n"))?;
    }
    git(path, &["prune-packed"])?;
    assert_eq!(pack_count(path)?, cap + 1, "one more pack than the current ceiling");
    assert!(
        reduced(path, false).is_err(),
        "the default reduced-trust open refuses an over-cap store"
    );
    brit(path, &["maintenance"])?;
    assert_eq!(pack_count(path)?, 8);
    let repo = reduced(path, false)?;
    assert_reads(&repo, &ids)?;
    assert_reads(&repo, &git(path, &["rev-list", "--objects", "--all"])?)?;
    Ok(())
}

#[test]
fn borrower_and_linked_worktree_keep_objects_outside_shared_refs() -> Result<()> {
    let repo = fixture(12)?;
    let path = repo.path();
    let scratch = tempfile::tempdir()?;
    let linked = scratch.path().join("linked");
    let borrower = scratch.path().join("borrower");
    let empty = scratch.path().join("empty");
    fs::create_dir(&empty)?;
    git(&empty, &["init", "-q", "--initial-branch=main"])?;
    git(
        path,
        &[
            "clone",
            "--no-local",
            "--reference",
            path.to_str().context("fixture path")?,
            empty.to_str().context("empty path")?,
            borrower.to_str().context("borrower path")?,
        ],
    )?;

    repo.commit_file("borrowed", "only the borrower needs this commit\n")?;
    git(path, &["repack", "-d"])?;
    let borrowed = repo.head_id()?;
    git(&borrower, &["update-ref", "refs/heads/main", &borrowed])?;
    git(path, &["reset", "--hard", "HEAD^"])?;
    git(path, &["reflog", "expire", "--expire=all", "--all"])?;
    assert!(!git(path, &["rev-list", "--all"])?.contains(&borrowed));
    assert!(borrower.join(".git/objects/info/alternates").is_file());

    let reflog_only = repo.commit_file("reflog-only", "reflog keeps this commit\n")?;
    git(path, &["repack", "-d"])?;
    git(path, &["reset", "--hard", "HEAD^"])?;

    git(
        path,
        &["worktree", "add", "--detach", linked.to_str().context("worktree path")?],
    )?;
    git(&linked, &["commit", "--allow-empty", "-m", "linked HEAD"])?;
    git(&linked, &["repack", "-d"])?;
    fs::write(
        linked.join("tracked"),
        "only the linked worktree index needs this blob\n",
    )?;
    git(&linked, &["add", "tracked"])?;
    let staged = git(&linked, &["rev-parse", ":tracked"])?;
    let staged = staged.trim();
    git_input(
        path,
        &["pack-objects", ".git/objects/pack/pack"],
        &format!("{staged}\n"),
    )?;
    git(path, &["prune-packed"])?;
    assert!(
        !path.join(".git/objects").join(&staged[..2]).join(&staged[2..]).exists(),
        "the staged blob is available only in a pack"
    );
    let linked_objects = git(&linked, &["rev-list", "--objects", "HEAD"])?;
    let borrower_objects = git(&borrower, &["rev-list", "--objects", "--all"])?;
    let index = git(&linked, &["ls-files", "--stage"])?;
    git(path, &["multi-pack-index", "write"])?;

    brit(&linked, &["-c", "brit.maintenanceTarget=1", "maintenance"])?;
    assert_eq!(
        pack_count(path)?,
        1,
        "the linked worktree invocation repacks the common store"
    );
    assert_reads(&reduced(&borrower, false)?, &borrower_objects)?;
    assert_reads(&reduced(&linked, false)?, &linked_objects)?;
    assert_reads(&reduced(path, false)?, &reflog_only)?;
    assert_eq!(git(&linked, &["ls-files", "--stage"])?, index);
    assert_eq!(
        git(&linked, &["show", ":tracked"])?,
        "only the linked worktree index needs this blob\n"
    );
    git(path, &["fsck", "--full"])?;
    Ok(())
}

fn fetch_fixture(packs: usize) -> Result<(TestRepo, TestRepo, String)> {
    let repo = fixture(packs)?;
    let path = repo.path();
    let remote = TestRepo::new("maintenance-remote")?;
    git(remote.path(), &["fetch", path.to_str().context("source path")?, "main"])?;
    git(remote.path(), &["reset", "--hard", "FETCH_HEAD"])?;
    let advanced = remote.commit_file("tracked", "fetched object\n")?;
    git(
        path,
        &[
            "remote",
            "add",
            "origin",
            remote.path().to_str().context("remote path")?,
        ],
    )?;
    Ok((repo, remote, advanced))
}

#[test]
fn fetch_crossing_threshold_reduces_pack_count() -> Result<()> {
    for packs in [2, 24, 40] {
        let (repo, _remote, advanced) = fetch_fixture(packs)?;
        let path = repo.path();
        if packs == 2 {
            git(path, &["config", "brit.maintenanceFetchThreshold", "3"])?;
        }
        if packs == 40 {
            git(path, &["config", "brit.maintenanceFetchThreshold", "40"])?;
            let head = gix::ObjectId::from_hex(repo.head_id()?.as_bytes())?;
            assert!(reduced(path, true)?.find_object(head).is_err());
        }
        brit(path, &["fetch", "--dry-run", "origin"])?;
        assert_eq!(pack_count(path)?, packs, "dry-run fetch skips maintenance");
        brit(path, &["fetch", "origin"])?;
        if packs == 2 {
            assert_eq!(pack_count(path)?, 3, "a fetch at the threshold leaves all packs");
            assert!(
                !path.join(".git/brit-objects.pin").exists(),
                "a fetch without consolidation leaves the pin untouched"
            );
        } else {
            assert_eq!(
                pack_count(path)?,
                if packs == 24 { 8 } else { 18 },
                "fetch repacks at most 24 inputs"
            );
        }
        assert_eq!(git(path, &["rev-parse", "origin/main"])?.trim(), advanced);
        assert_reads(&reduced(path, true)?, &git(path, &["rev-list", "--objects", "--all"])?)?;
    }
    Ok(())
}

#[test]
fn fetch_keeps_success_when_maintenance_fails() -> Result<()> {
    let (repo, _remote, advanced) = fetch_fixture(24)?;
    let path = repo.path();
    fs::write(path.join(".git/objects/pack/.brit-maintenance"), "path collision")?;
    let fetched = brit(path, &["fetch", "origin"])?;
    assert!(String::from_utf8(fetched.stderr)?.contains("warning: pack maintenance after fetch failed"));
    assert_eq!(git(path, &["rev-parse", "origin/main"])?.trim(), advanced);
    assert_eq!(pack_count(path)?, 25, "the new pack is still available");
    Ok(())
}

#[cfg(target_os = "linux")]
fn with_push_in_hook<T>(repo: &TestRepo, during_hook: impl FnOnce(&Path, u32) -> Result<T>) -> Result<T> {
    use std::{
        os::unix::fs::PermissionsExt,
        time::{Duration, Instant},
    };

    let path = repo.path();
    let scratch = tempfile::tempdir()?;
    let remote = scratch.path().join("remote.git");
    let linked = scratch.path().join("linked");
    let remote_path = remote.to_str().context("remote path")?;
    let linked_path = linked.to_str().context("worktree path")?;
    git(
        scratch.path(),
        &["init", "--bare", "--initial-branch=main", remote_path],
    )?;
    git(path, &["remote", "add", "origin", remote_path])?;
    git(path, &["push", "origin", "main"])?;
    git(path, &["worktree", "add", "-b", "landing", linked_path])?;
    fs::write(linked.join("landing"), "only the pushing worktree has this object\n")?;
    git(&linked, &["add", "landing"])?;
    git(&linked, &["commit", "-m", "landing commit"])?;
    git(&linked, &["repack", "-d"])?;
    let pushed = git(&linked, &["rev-parse", "HEAD"])?;
    let pushed = pushed.trim();
    let objects = git(&linked, &["rev-list", "--objects", pushed])?;

    let hook = path.join(".git/hooks/pre-push");
    fs::write(
        &hook,
        r#"#!/bin/sh
set -eu
cat >/dev/null
touch "$BRIT_TEST_HOOK_DIR/started"
for i in $(seq 600); do
    test ! -f "$BRIT_TEST_HOOK_DIR/release" || exit 0
    sleep 0.05
done
exit 1
"#,
    )?;
    fs::set_permissions(&hook, fs::Permissions::from_mode(0o755))?;
    let args = ["push", "origin", "landing"];
    let mut push = brit_command(&linked)
        .args(args)
        .env("BRIT_TEST_HOOK_DIR", scratch.path())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    let observed = (|| {
        let deadline = Instant::now() + Duration::from_secs(10);
        while !scratch.path().join("started").exists() {
            ensure!(push.try_wait()?.is_none(), "push exited before starting the hook");
            ensure!(Instant::now() < deadline, "push hook did not start");
            std::thread::sleep(Duration::from_millis(10));
        }
        during_hook(&remote, push.id())
    })();
    let release = fs::write(scratch.path().join("release"), "");
    let output = push.wait_with_output()?;
    release?;
    let observed = observed.with_context(|| format!("push stderr: {}", String::from_utf8_lossy(&output.stderr)))?;
    checked(output, &args)?;
    assert_eq!(git(&remote, &["rev-parse", "refs/heads/landing"])?.trim(), pushed);
    git(&remote, &["fsck", "--full"])?;
    assert_eq!(git(&remote, &["rev-list", "--objects", pushed])?, objects);
    Ok(observed)
}

#[cfg(target_os = "linux")]
#[test]
fn fetch_defers_consolidation_while_a_linked_worktree_push_runs_its_hook() -> Result<()> {
    let repo = fixture(2)?;
    let path = repo.path();
    git(path, &["config", "brit.maintenanceFetchThreshold", "3"])?;
    git(path, &["config", "brit.maintenanceTarget", "2"])?;
    let (stderr, packs, push_pid) = with_push_in_hook(&repo, |remote, push_pid| {
        ensure!(pack_count(path)? == 3, "push starts with the store at the threshold");
        fs::write(path.join("local"), "committed during the push hook\n")?;
        git(path, &["add", "local"])?;
        brit(path, &["commit", "-m", "commit during push"])?;
        ensure!(
            git(path, &["show", "HEAD:local"])? == "committed during the push hook\n",
            "the main worktree commit advances HEAD during the hook"
        );

        let upstream = TestRepo::new("maintenance-upstream")?;
        let remote_path = remote.to_str().context("remote path")?;
        git(upstream.path(), &["fetch", remote_path, "main"])?;
        git(upstream.path(), &["reset", "--hard", "FETCH_HEAD"])?;
        let advanced = upstream.commit_file("upstream", "only the remote has this object\n")?;
        git(upstream.path(), &["push", remote_path, "main"])?;
        let fetched = brit(path, &["fetch", "origin"])?;
        ensure!(
            git(path, &["rev-parse", "origin/main"])?.trim() == advanced,
            "fetch advances the remote-tracking ref during the hook"
        );
        let stderr = String::from_utf8(fetched.stderr)?;
        let packs = pack_count(path)?;
        Ok((stderr, packs, push_pid))
    })?;
    assert_eq!(packs, 4, "fetch adds a pack and leaves the original three in place");
    assert_eq!(
        pack_count(path)?,
        packs,
        "pack count stays steady through the end of the push"
    );
    let mut notices = stderr
        .lines()
        .filter(|line| line.contains("pack consolidation deferred because a push is in progress"));
    let notice = notices
        .next()
        .with_context(|| format!("fetch deferral missing from {stderr}"))?;
    assert!(notice.contains(&format!("pid {push_pid} ")), "{notice}");
    assert!(notices.next().is_none(), "fetch reports one deferral: {stderr}");

    brit(path, &["maintenance"])?;
    assert_eq!(
        pack_count(path)?,
        2,
        "maintenance consolidates after the push releases its pin"
    );
    Ok(())
}

#[cfg(target_os = "linux")]
#[test]
fn manual_maintenance_refuses_while_a_linked_worktree_push_runs_its_hook() -> Result<()> {
    let repo = fixture(2)?;
    let path = repo.path();
    git(path, &["config", "brit.maintenanceTarget", "2"])?;
    with_push_in_hook(&repo, |_, push_pid| {
        let result = brit_command(path).arg("maintenance").output()?;
        let stderr = String::from_utf8(result.stderr)?;
        ensure!(!result.status.success(), "maintenance refuses the push's shared pin");
        ensure!(stderr.contains("brit-objects.pin"), "{stderr}");
        ensure!(stderr.contains(&format!("pid {push_pid} ")), "{stderr}");
        ensure!(pack_count(path)? == 3, "refused maintenance leaves all packs in place");
        Ok(())
    })
}

#[cfg(feature = "switch-test-faults")]
#[test]
fn killed_maintenance_preserves_objects_and_can_be_retried() -> Result<()> {
    use std::time::{Duration, Instant};

    for point in [
        "maintenance-pack-prepared",
        "maintenance-staged",
        "maintenance-pack-published",
        "maintenance-index-published",
        "maintenance-midx-invalidated",
        "maintenance-retiring",
    ] {
        let repo = fixture(12)?;
        let path = repo.path();
        let objects = git(path, &["rev-list", "--objects", "--all"])?;
        git(path, &["multi-pack-index", "write"])?;
        let markers = tempfile::tempdir()?;
        let mut child = brit_command(path)
            .arg("maintenance")
            .env("BRIT_SWITCH_CHECKPOINT", point)
            .env("BRIT_SWITCH_CHECKPOINT_DIR", markers.path())
            .spawn()?;
        let wait = (|| -> Result<()> {
            let deadline = Instant::now() + Duration::from_secs(30);
            while !markers.path().join(format!("{point}.reached")).exists() {
                ensure!(child.try_wait()?.is_none(), "maintenance exited before {point}");
                ensure!(Instant::now() < deadline, "maintenance did not reach {point}");
                std::thread::sleep(Duration::from_millis(5));
            }
            if cfg!(target_os = "linux") && point == "maintenance-staged" {
                let other_writer = brit_command(path)
                    .args(["--wait=0", "branch", "blocked-during-maintenance"])
                    .output()?;
                ensure!(
                    !other_writer.status.success(),
                    "maintenance holds the repository mutation guard"
                );
                ensure!(
                    String::from_utf8_lossy(&other_writer.stderr).contains("maintenance"),
                    "the guard refusal identifies its holder"
                );
            }
            Ok(())
        })();
        let killed = child.kill();
        let status = child.wait()?;
        wait?;
        killed?;
        assert!(!status.success(), "child was killed at {point}");

        assert_reads(&reduced(path, false)?, &objects)?;
        git(path, &["fsck", "--full"])?;
        brit(path, &["maintenance"])?;
        assert!(pack_count(path)? <= 8, "retry reaches the target after {point}");
        assert_eq!(
            pack_files(path, "pack")?,
            pack_count(path)?,
            "retry leaves no orphan pack after {point}"
        );
        assert!(
            !path.join(".git/objects/pack/.brit-maintenance").exists(),
            "retry removes the completed transaction"
        );
        assert_reads(&reduced(path, false)?, &objects)?;
        git(path, &["fsck", "--full"])?;
    }
    Ok(())
}
