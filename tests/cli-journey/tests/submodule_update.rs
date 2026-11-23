//! Initialize a submodule in a fresh linked worktree and update it to the index pin.
use anyhow::{ensure, Result};
use cli_journey::support::{
    runner::{brit_bin, BritInvocation, Capture},
    test_repo::TestRepo,
};
use std::{fs, path::Path};

fn brit(repo: &Path, args: &[&str]) -> Result<Capture> {
    BritInvocation::new(brit_bin().expect("built Brit binary"))
        .current_dir(repo)
        .args(args.iter().copied())
        .run()
}

fn git(repo: &Path, args: &[&str]) -> Result<String> {
    let output = gix_testtools::git_command(repo).args(args).output()?;
    ensure!(
        output.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    Ok(String::from_utf8(output.stdout)?.trim().to_owned())
}

fn stage_pin(repo: &Path, commit: &str) -> Result<()> {
    git(
        repo,
        &[
            "update-index",
            "--add",
            "--cacheinfo",
            &format!("160000,{commit},deps/child"),
        ],
    )?;
    Ok(())
}

#[test]
fn initializes_linked_worktree_and_refuses_to_overwrite_local_changes() -> Result<()> {
    let child = TestRepo::new("submodule-origin")?;
    let first = child.commit_file("tracked", "first\n")?;
    child.commit_file("tracked", "remote HEAD\n")?;
    let parent = TestRepo::new("submodule-parent")?;
    let url = child.path().to_string_lossy().replace(std::path::MAIN_SEPARATOR, "/");
    stage_pin(parent.path(), &first)?;
    parent.commit_file(
        ".gitmodules",
        &format!(
            "[submodule \"child-name\"]
    path = deps/child
    url = {url}
"
        ),
    )?;

    let destinations = tempfile::tempdir()?;
    let linked = destinations.path().join("linked");
    let linked_path = linked.to_str().expect("UTF-8 fixture path");
    let added = brit(parent.path(), &["worktree", "add", "--detach", linked_path])?;
    ensure!(added.status.success(), "worktree add: {}", added.stderr);

    let unknown = brit(&linked, &["submodule", "update", "--init", "unknown"])?;
    assert!(!unknown.status.success(), "unknown paths must refuse");
    assert!(unknown.stderr.contains("Unknown submodule path"), "{}", unknown.stderr);
    for flag in ["--recursive", "--remote"] {
        let refused = brit(
            parent.path(),
            &["-C", linked_path, "submodule", "update", "--init", flag],
        )?;
        assert!(!refused.status.success(), "{flag} must refuse");
        assert!(
            refused.stderr.contains(&format!("does not support {flag}")),
            "{}",
            refused.stderr
        );
    }

    let args = ["submodule", "update", "--init", "deps/child"];
    let result = brit(&linked, &args)?;
    ensure!(result.status.success(), "submodule update: {}", result.stderr);
    let checkout = linked.join("deps/child");
    assert_eq!(git(&checkout, &["rev-parse", "HEAD"])?, first);
    assert_eq!(fs::read(checkout.join("tracked"))?, b"first\n");
    assert_eq!(git(&linked, &["config", "--local", "submodule.child-name.url"])?, url);
    assert!(checkout.join(".git").is_file(), "submodule must have a gitfile");
    let gitdir = git(&checkout, &["rev-parse", "--absolute-git-dir"])?;
    let expected = git(
        &linked,
        &[
            "rev-parse",
            "--path-format=absolute",
            "--git-path",
            "modules/child-name",
        ],
    )?;
    assert_eq!(Path::new(&gitdir).canonicalize()?, Path::new(&expected).canonicalize()?);
    let head = Path::new(&gitdir).join("HEAD");
    assert_eq!(fs::read_to_string(&head)?.trim(), first, "HEAD must be detached");

    let index = Path::new(&gitdir).join("index");
    let index_before = fs::metadata(&index)?.modified()?;
    git(&checkout, &["config", "remote.origin.url", "./offline"])?;
    let result = brit(&linked, &args)?;
    ensure!(result.status.success(), "repeat update: {}", result.stderr);
    assert_eq!(
        fs::metadata(&index)?.modified()?,
        index_before,
        "repeat must leave index untouched"
    );
    git(&checkout, &["config", "remote.origin.url", &url])?;

    git(child.path(), &["rm", "tracked"])?;
    child.commit_file("tracked/nested", "new gitlink\n")?;
    let next = child.commit_file("blocked", "remote file\n")?;
    stage_pin(&linked, &next)?;
    fs::write(checkout.join("tracked"), "local edits\n")?;
    let dirty = brit(&linked, &args)?;
    assert!(!dirty.status.success(), "dirty submodule must refuse");
    assert!(dirty.stderr.contains("local modifications"), "{}", dirty.stderr);
    assert_eq!(fs::read(checkout.join("tracked"))?, b"local edits\n");
    assert_eq!(git(&checkout, &["rev-parse", "HEAD"])?, first);

    fs::write(checkout.join("tracked"), "first\n")?;
    fs::write(Path::new(&gitdir).join("info/exclude"), "/blocked\n")?;
    fs::write(checkout.join("blocked"), "ignored local file\n")?;
    let blocked = brit(&linked, &args)?;
    assert!(
        !blocked.status.success(),
        "update must refuse an ignored path collision"
    );
    assert!(blocked.stderr.contains("would overwrite"), "{}", blocked.stderr);
    assert_eq!(git(&checkout, &["rev-parse", "HEAD"])?, first);
    assert_eq!(fs::read(checkout.join("tracked"))?, b"first\n");
    assert_eq!(fs::read(checkout.join("blocked"))?, b"ignored local file\n");
    assert_eq!(fs::metadata(&index)?.modified()?, index_before);
    fs::remove_file(checkout.join("blocked"))?;
    git(&checkout, &["config", "remote.origin.url", "./offline"])?;
    let result = brit(&linked, &["submodule", "update", "deps/child"])?;
    ensure!(result.status.success(), "fetch and update: {}", result.stderr);
    assert_eq!(git(&checkout, &["rev-parse", "HEAD"])?, next);
    assert_eq!(fs::read(checkout.join("tracked/nested"))?, b"new gitlink\n");
    assert_eq!(fs::read(checkout.join("blocked"))?, b"remote file\n");
    Ok(())
}
