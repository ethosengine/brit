//! A developer creates a branch without disturbing staged or unstaged work.
use std::{path::Path, process::Command};

use gix_testtools::{git, tempfile, TestResult};

fn brit(root: &Path, args: &[&str]) -> std::process::Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_brit"));
    gix_testtools::configure_git_environment(&mut command, root);
    command
        .current_dir(root)
        .args(args)
        .output()
        .expect("run Brit in disposable repository")
}

#[test]
fn create_preserves_checkout_and_refuses_existing_or_invalid_names() -> TestResult {
    let dir = tempfile::tempdir()?;
    let root = dir.path();
    git(root, "init")?;
    git(root, "commit --allow-empty -m base")?;
    let base = git(root, "rev-parse HEAD")?;
    std::fs::write(root.join("staged"), "staged bytes\n")?;
    git(root, "add staged")?;
    std::fs::write(root.join("staged"), "unstaged bytes\n")?;
    let index = std::fs::read(root.join(".git/index"))?;
    let head = std::fs::read(root.join(".git/HEAD"))?;

    let output = brit(root, &["branch", "create", "topic/nested"]);
    assert!(output.status.success(), "branch creation: {output:?}");
    assert_eq!(git(root, "rev-parse topic/nested")?, base);
    assert_eq!(std::fs::read(root.join(".git/index"))?, index);
    assert_eq!(std::fs::read(root.join(".git/HEAD"))?, head);
    assert_eq!(std::fs::read_to_string(root.join("staged"))?, "unstaged bytes\n");
    assert!(git(root, "reflog show topic/nested")?.contains("branch: Created from HEAD"));

    git(root, "commit -m advance")?;
    for name in ["topic/nested", "bad..name", "HEAD", "-option"] {
        let output = brit(root, &["branch", "create", "--", name]);
        assert!(!output.status.success(), "must refuse {name}: {output:?}");
    }
    assert_eq!(
        git(root, "rev-parse topic/nested")?,
        base,
        "existing branch never overwritten"
    );
    Ok(())
}

#[test]
fn create_resolves_start_point_and_refuses_non_commit_objects() -> TestResult {
    let dir = tempfile::tempdir()?;
    let root = dir.path();
    git(root, "init")?;
    git(root, "commit --allow-empty -m base")?;
    let base = git(root, "rev-parse HEAD")?;
    git(root, "tag -a start -m start")?;
    git(root, "commit --allow-empty -m next")?;
    let output = brit(root, &["branch", "create", "from-tag", "start"]);
    assert!(output.status.success(), "peel annotated tag: {output:?}");
    assert_eq!(git(root, "rev-parse from-tag")?, base);
    for source in ["missing", "HEAD^{tree}"] {
        let output = brit(root, &["branch", "create", "invalid-source", source]);
        assert!(!output.status.success(), "must refuse {source}: {output:?}");
        assert!(!root.join(".git/refs/heads/invalid-source").exists());
    }
    Ok(())
}

#[test]
fn create_from_linked_worktree_uses_its_head_and_shared_refs() -> TestResult {
    let dir = tempfile::tempdir()?;
    let root = dir.path();
    git(root, "init")?;
    git(root, "commit --allow-empty -m base")?;
    let added = brit(root, &["worktree", "add", "-b", "side", "linked"]);
    assert!(added.status.success(), "native worktree creation: {added:?}");
    let linked = root.join("linked");
    git(&linked, "commit --allow-empty -m linked")?;
    let expected = git(&linked, "rev-parse HEAD")?;
    let main_head = git(root, "rev-parse HEAD")?;
    let output = brit(&linked, &["branch", "create", "from-linked"]);
    assert!(output.status.success(), "linked worktree creation: {output:?}");
    assert_eq!(git(root, "rev-parse from-linked")?, expected);
    assert_eq!(git(root, "rev-parse HEAD")?, main_head);
    assert_eq!(git(&linked, "symbolic-ref --short HEAD")?.trim(), "side");
    Ok(())
}

#[cfg(target_os = "linux")]
#[test]
fn creation_obeys_shared_guard_but_listing_remains_available() -> TestResult {
    use std::{io::Read, process::Stdio};
    let dir = tempfile::tempdir()?;
    let root = dir.path();
    git(root, "init")?;
    git(root, "commit --allow-empty -m base")?;
    git(root, "worktree add -b side linked")?;
    let linked = root.join("linked");
    for checkout in [root, linked.as_path()] {
        std::fs::write(checkout.join("selected"), "staged\n")?;
        git(checkout, "add selected")?;
        std::fs::write(checkout.join("selected"), "unstaged\n")?;
    }
    let guard_path = root.join(".git/brit-mutation.guard");
    let mut command = Command::new("flock");
    gix_testtools::configure_git_environment(&mut command, root);
    let mut holder = command
        .arg(&guard_path)
        .args(["sh", "-c", "printf ready; cat"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()?;
    let mut ready = [0; 5];
    holder.stdout.take().expect("piped stdout").read_exact(&mut ready)?;
    assert_eq!(&ready, b"ready");
    for checkout in [root, linked.as_path()] {
        let git_dir = git(checkout, "rev-parse --absolute-git-dir")?;
        let git_dir = Path::new(git_dir.trim());
        let index = std::fs::read(git_dir.join("index"))?;
        let head = std::fs::read(git_dir.join("HEAD"))?;
        let refs = git(checkout, "show-ref")?;
        let foreign_lock = git_dir.join("index.lock");
        std::fs::write(&foreign_lock, "foreign owner\n")?;
        let result = brit(checkout, &["branch", "create", "blocked"]);
        assert!(!result.status.success(), "must refuse active guard: {result:?}");
        assert!(String::from_utf8_lossy(&result.stderr).contains("another Brit mutation is active"));
        assert!(brit(checkout, &["branch", "list"]).status.success());
        assert_eq!(git(checkout, "show-ref")?, refs);
        assert_eq!(std::fs::read(git_dir.join("HEAD"))?, head);
        assert_eq!(std::fs::read(git_dir.join("index"))?, index);
        assert_eq!(std::fs::read_to_string(&foreign_lock)?, "foreign owner\n");
        assert_eq!(std::fs::read_to_string(checkout.join("selected"))?, "unstaged\n");
    }
    drop(holder.stdin.take());
    assert!(holder.wait()?.success());
    Ok(())
}
