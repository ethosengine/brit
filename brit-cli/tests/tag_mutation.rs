//! A developer creates and deletes tags; stock Git reads back every object Brit writes.
use std::{path::Path, process::Command};

use gix_testtools::{git, git_command, tempfile, TestResult};

fn brit(root: &Path, args: &[&str]) -> std::process::Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_brit"));
    gix_testtools::configure_git_environment(&mut command, root);
    command
        .current_dir(root)
        .args(args)
        .output()
        .expect("run Brit in disposable repository")
}

fn git_args(root: &Path, args: &[&str]) -> gix_testtools::Result<String> {
    let output = git_command(root).args(args).output()?;
    assert!(output.status.success(), "git {args:?}: {output:?}");
    Ok(String::from_utf8(output.stdout)?.trim().to_owned())
}

fn rev(root: &Path, spec: &str) -> gix_testtools::Result<String> {
    Ok(git(root, &format!("rev-parse {spec}"))?.trim().to_owned())
}

fn stderr(output: &std::process::Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

fn fixture(root: &Path) -> TestResult {
    git(root, "init")?;
    git(root, "commit --allow-empty -m base")?;
    git(root, "commit --allow-empty -m next")?;
    Ok(())
}

#[test]
fn lightweight_tags_point_at_the_named_object() -> TestResult {
    let dir = tempfile::tempdir()?;
    let root = dir.path();
    fixture(root)?;
    let index = std::fs::read(root.join(".git/index"))?;
    let head = std::fs::read(root.join(".git/HEAD"))?;
    for args in [
        vec!["tag", "v1"],
        vec!["tag", "v0", "HEAD~1"],
        vec!["tag", "tree", "HEAD^{tree}"],
    ] {
        let output = brit(root, &args);
        assert!(output.status.success(), "{args:?}: {output:?}");
    }
    assert_eq!(rev(root, "refs/tags/v1")?, rev(root, "HEAD")?);
    assert_eq!(rev(root, "refs/tags/v0")?, rev(root, "HEAD~1")?);
    assert_eq!(rev(root, "refs/tags/tree")?, rev(root, "HEAD^{tree}")?);
    assert_eq!(
        git(root, "for-each-ref --format=%(objecttype) refs/tags/v1")?.trim(),
        "commit",
        "lightweight tags write no tag object"
    );
    assert_eq!(std::fs::read(root.join(".git/index"))?, index);
    assert_eq!(std::fs::read(root.join(".git/HEAD"))?, head);
    let listed = brit(root, &["tag", "list"]);
    assert!(listed.status.success());
    assert!(String::from_utf8_lossy(&listed.stdout).contains("v1"));
    Ok(())
}

#[test]
fn annotated_tag_objects_are_byte_identical_to_git() -> TestResult {
    let dir = tempfile::tempdir()?;
    let root = dir.path();
    fixture(root)?;
    let cases: [(&str, Vec<&str>, Option<&str>); 4] = [
        ("plain", vec!["-a", "-m", "release two"], None),
        (
            "cleaned",
            vec!["-a", "-m", "\n\n# a comment\nsubject  \n\n\n\nbody\t\n\n"],
            None,
        ),
        ("implied", vec!["-m", "message implies -a"], None),
        ("older", vec!["-a", "-m", "older"], Some("HEAD~1")),
    ];
    for (name, flags, target) in cases {
        // Brit accepts Git's own spelling, so one argument vector drives both tools.
        let mut args = vec!["tag"];
        args.extend(&flags);
        args.push(name);
        args.extend(target);
        let output = brit(root, &args);
        assert!(output.status.success(), "{args:?}: {output:?}");
        assert_eq!(git(root, &format!("cat-file -t refs/tags/{name}"))?.trim(), "tag");
        let ours = rev(root, &format!("refs/tags/{name}"))?;
        git(root, &format!("tag -d {name}"))?;
        git_args(root, &args)?;
        assert_eq!(
            ours,
            rev(root, &format!("refs/tags/{name}"))?,
            "{name}: same tag object as git"
        );
    }
    let body = git(root, "cat-file tag refs/tags/plain")?;
    assert!(
        body.contains("tagger committer <committer@example.com> 946771200 +0000"),
        "tagger uses the committer identity: {body}"
    );
    git(root, "fsck --no-progress --strict")?;
    Ok(())
}

#[test]
fn creation_refuses_overwrites_invalid_names_and_editor_annotation() -> TestResult {
    let dir = tempfile::tempdir()?;
    let root = dir.path();
    fixture(root)?;
    assert!(brit(root, &["tag", "v1", "HEAD~1"]).status.success());
    let old = rev(root, "refs/tags/v1")?;
    for args in [vec!["tag", "v1"], vec!["tag", "-a", "v1", "-m", "again"]] {
        let output = brit(root, &args);
        assert!(!output.status.success(), "must refuse overwrite {args:?}: {output:?}");
        assert!(stderr(&output).contains("tag 'v1' already exists"), "{output:?}");
        assert_eq!(rev(root, "refs/tags/v1")?, old, "existing tag kept");
    }
    let output = brit(root, &["tag", "-f", "v1"]);
    assert!(output.status.success(), "forced overwrite: {output:?}");
    assert!(
        String::from_utf8_lossy(&output.stdout).contains("Updated tag 'v1' (was "),
        "{output:?}"
    );
    assert_eq!(rev(root, "refs/tags/v1")?, rev(root, "HEAD")?);
    let output = brit(root, &["tag", "--force", "-a", "v1", "-m", "annotated now"]);
    assert!(output.status.success(), "forced annotated overwrite: {output:?}");
    assert_eq!(git(root, "cat-file -t refs/tags/v1")?.trim(), "tag");

    for name in ["-dash", "bad..name", "HEAD", "with space", "trailing.lock"] {
        let output = brit(root, &["tag", "--", name]);
        assert!(!output.status.success(), "must refuse {name:?}: {output:?}");
    }
    let output = brit(root, &["tag", "missing-target", "no-such-rev"]);
    assert!(!output.status.success(), "{output:?}");
    let output = brit(root, &["tag", "-a", "noeditor"]);
    assert!(!output.status.success(), "annotation without -m: {output:?}");
    assert!(stderr(&output).contains("-m"), "{output:?}");
    assert_eq!(
        git(root, "for-each-ref --format=%(refname) refs/tags")?.trim(),
        "refs/tags/v1",
        "refusals write no tags"
    );
    Ok(())
}

#[test]
fn delete_removes_loose_and_packed_tags_all_or_nothing() -> TestResult {
    let dir = tempfile::tempdir()?;
    let root = dir.path();
    fixture(root)?;
    for args in ["tag loose", "tag -a annotated -m note", "tag packed", "tag keep"] {
        git(root, args)?;
    }
    git(root, "pack-refs --all")?;
    git(root, "tag loose-after")?;
    let short = git(root, "rev-parse --short refs/tags/annotated")?;

    let output = brit(root, &["tag", "--delete", "annotated", "absent"]);
    assert!(!output.status.success(), "one missing name refuses all: {output:?}");
    assert!(stderr(&output).contains("tag 'absent' not found"), "{output:?}");
    assert_eq!(git(root, "cat-file -t refs/tags/annotated")?.trim(), "tag");

    let output = brit(root, &["tag", "--delete", "annotated", "packed"]);
    assert!(output.status.success(), "{output:?}");
    assert!(
        String::from_utf8_lossy(&output.stdout).contains(&format!("Deleted tag 'annotated' (was {})", short.trim())),
        "{output:?}"
    );
    let output = brit(root, &["tag", "-d", "loose-after"]);
    assert!(output.status.success(), "{output:?}");
    assert_eq!(
        git(root, "for-each-ref --format=%(refname) refs/tags")?,
        "refs/tags/keep\nrefs/tags/loose\n"
    );
    git(root, "fsck --no-progress")?;
    Ok(())
}

#[cfg(target_os = "linux")]
#[test]
fn tag_mutations_obey_shared_guard_but_listing_remains_available() -> TestResult {
    use std::{io::Read, process::Stdio};
    let dir = tempfile::tempdir()?;
    let root = dir.path();
    fixture(root)?;
    git(root, "tag existing")?;
    let mut command = Command::new("flock");
    gix_testtools::configure_git_environment(&mut command, root);
    let mut holder = command
        .arg(root.join(".git/brit-mutation.guard"))
        .args(["sh", "-c", "printf ready; cat"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()?;
    let mut ready = [0; 5];
    holder.stdout.take().expect("piped stdout").read_exact(&mut ready)?;
    for args in [
        vec!["tag", "blocked"],
        vec!["tag", "-a", "blocked", "-m", "m"],
        vec!["tag", "-d", "existing"],
    ] {
        let result = brit(root, &args);
        assert!(
            !result.status.success(),
            "must refuse active guard {args:?}: {result:?}"
        );
        assert!(
            stderr(&result).contains("another Brit mutation is active"),
            "{result:?}"
        );
    }
    for args in [vec!["tag"], vec!["tag", "list"]] {
        let listed = brit(root, &args);
        assert!(listed.status.success(), "{args:?}: {listed:?}");
        assert!(String::from_utf8_lossy(&listed.stdout).contains("existing"));
    }
    assert_eq!(
        git(root, "for-each-ref --format=%(refname) refs/tags")?.trim(),
        "refs/tags/existing"
    );
    drop(holder.stdin.take());
    assert!(holder.wait()?.success());
    Ok(())
}
