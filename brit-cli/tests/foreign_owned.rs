//! Command-scope `safe.directory` for a checkout owned by another user.
//!
//! A device runs brit as one uid over checkouts owned by another and passes the checkouts it
//! trusts as command-scope configuration: `GIT_CONFIG_COUNT`/`KEY_n`/`VALUE_n`, or `-c`. Git
//! honours both; so must brit, for the exact root only.
//!
//! Staging foreign ownership needs root to chown, so these tests are ignored by default and run
//! with `-- --ignored`. They panic instead of passing when they cannot stage it.
#![cfg(unix)]

use std::{
    ffi::OsString,
    path::{Path, PathBuf},
    process::{Command, Output},
};

/// The conventional `nobody` uid: container filesystems may reject arbitrary unmapped owners.
const FOREIGN_UID: &str = "65534";
const REDUCED_TRUST_TRACE: &str = "Applied a default allocation limit";
const HOOK_REFUSAL: &str = "refusing to execute hooks in an untrusted repository";

fn isolate(command: &mut Command, home: &Path) {
    let inherited: Vec<OsString> = std::env::vars_os()
        .map(|(name, _)| name)
        .filter(|name| {
            let name = name.to_string_lossy();
            name.starts_with("GIT_") || name.starts_with("SUDO_")
        })
        .collect();
    for name in inherited {
        command.env_remove(name);
    }
    command
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("HOME", home)
        .env("XDG_CONFIG_HOME", home.join(".config"))
        .env("GIT_AUTHOR_NAME", "author")
        .env("GIT_AUTHOR_EMAIL", "author@example.com")
        .env("GIT_COMMITTER_NAME", "committer")
        .env("GIT_COMMITTER_EMAIL", "committer@example.com");
}

fn git(home: &Path, dir: &Path, args: &[&str]) -> String {
    let mut command = Command::new("git");
    isolate(&mut command, home);
    let output = command
        .args(["-c", "commit.gpgsign=false", "-c", "init.defaultBranch=main"])
        .args(args)
        .current_dir(dir)
        .output()
        .expect("git runs");
    assert!(
        output.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).trim().to_string()
}

/// A checkout owned by [`FOREIGN_UID`] with `pack_indexes` pack indexes, a staged change and a
/// `pre-commit` hook that leaves a marker outside the repository.
struct Foreign {
    _dir: gix_testtools::tempfile::TempDir,
    home: gix_testtools::tempfile::TempDir,
    root: PathBuf,
    marker: PathBuf,
}

impl Foreign {
    fn stage(pack_indexes: usize) -> Self {
        Self::stage_with(pack_indexes, |_, _| {})
    }

    /// [`Self::stage`], running `prepare(home, root)` just before the checkout is chowned.
    fn stage_with(pack_indexes: usize, prepare: impl FnOnce(&Path, &Path)) -> Self {
        let dir = gix_testtools::tempfile::TempDir::new().expect("tempdir");
        let home = gix_testtools::tempfile::TempDir::new().expect("home");
        let root = dir.path().join("checkout");
        std::fs::create_dir(&root).expect("create checkout");
        let root = root.canonicalize().expect("canonicalize");
        let marker = home.path().join("hook-ran");
        let h = home.path();
        git(h, &root, &["init", "-q"]);
        std::fs::write(root.join("file"), "head\n").expect("write");
        git(h, &root, &["add", "file"]);
        git(h, &root, &["commit", "-q", "-m", "head"]);
        if pack_indexes > 0 {
            git(h, &root, &["repack", "-q", "-a", "-d"]);
            for index in 1..pack_indexes {
                let path = root.join(format!("filler-{index}"));
                std::fs::write(&path, format!("filler {index}\n")).expect("write");
                let id = git(h, &root, &["hash-object", "-w", path.to_str().expect("utf8")]);
                let mut child = Command::new("git");
                isolate(&mut child, h);
                let mut child = child
                    .args(["pack-objects", "-q", ".git/objects/pack/pack"])
                    .current_dir(&root)
                    .stdin(std::process::Stdio::piped())
                    .stdout(std::process::Stdio::null())
                    .spawn()
                    .expect("pack-objects");
                std::io::Write::write_all(child.stdin.as_mut().expect("stdin"), format!("{id}\n").as_bytes())
                    .expect("write id");
                assert!(child.wait().expect("pack-objects").success());
                std::fs::remove_file(path).expect("remove filler");
            }
            git(h, &root, &["prune-packed"]);
            let indexes = std::fs::read_dir(root.join(".git/objects/pack"))
                .expect("pack dir")
                .filter_map(Result::ok)
                .filter(|entry| entry.path().extension().is_some_and(|ext| ext == "idx"))
                .count();
            assert_eq!(indexes, pack_indexes, "the fixture has the pack indexes asked for");
        }
        std::fs::write(root.join("staged"), "staged\n").expect("write");
        git(h, &root, &["add", "staged"]);
        let hook = root.join(".git/hooks/pre-commit");
        std::fs::write(&hook, format!("#!/bin/sh\necho ran > '{}'\n", marker.display())).expect("hook");
        std::fs::set_permissions(&hook, std::os::unix::fs::PermissionsExt::from_mode(0o755)).expect("chmod");

        prepare(h, &root);
        let chown = Command::new("chown")
            .args(["-R", &format!("{FOREIGN_UID}:{FOREIGN_UID}")])
            .arg(&root)
            .output()
            .expect("chown runs");
        assert!(
            chown.status.success(),
            "cannot stage foreign ownership (run as root): {}",
            String::from_utf8_lossy(&chown.stderr)
        );
        let mut status = Command::new("git");
        isolate(&mut status, h);
        let status = status.arg("status").current_dir(&root).output().expect("git runs");
        assert!(
            String::from_utf8_lossy(&status.stderr).contains("dubious ownership"),
            "git itself must refuse the staged checkout: {status:?}"
        );
        Foreign {
            _dir: dir,
            home,
            root,
            marker,
        }
    }

    fn brit(&self, grant: &Grant, args: &[&str]) -> Output {
        let mut command = Command::new(env!("CARGO_BIN_EXE_brit"));
        isolate(&mut command, self.home.path());
        match grant {
            Grant::None => {}
            Grant::Environment(value) => {
                command
                    .env("GIT_CONFIG_COUNT", "1")
                    .env("GIT_CONFIG_KEY_0", "safe.directory")
                    .env("GIT_CONFIG_VALUE_0", value);
            }
            Grant::CommandLine(value) => {
                command.arg("-c").arg(format!("safe.directory={value}"));
            }
        }
        command.args(args).current_dir(&self.root).output().expect("brit runs")
    }

    fn display(&self) -> String {
        self.root.display().to_string()
    }
}

#[derive(Debug)]
enum Grant {
    None,
    Environment(String),
    CommandLine(String),
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

fn assert_full_trust(grant: &Grant) {
    let repo = Foreign::stage(33);
    let grant = match grant {
        Grant::Environment(_) => Grant::Environment(repo.display()),
        Grant::CommandLine(_) => Grant::CommandLine(repo.display()),
        Grant::None => unreachable!("only grants give full trust"),
    };
    let log = repo.brit(&grant, &["--trace", "log", "-n", "1"]);
    assert!(
        log.status.success(),
        "{grant:?}: 33 pack indexes open: {}",
        stderr(&log)
    );
    assert!(
        !stderr(&log).contains(REDUCED_TRUST_TRACE),
        "{grant:?}: no reduced-trust allocation limit: {}",
        stderr(&log)
    );
    let commit = repo.brit(&grant, &["commit", "-m", "hooked"]);
    assert!(
        commit.status.success(),
        "{grant:?}: hooks are allowed: {}",
        stderr(&commit)
    );
    assert!(repo.marker.is_file(), "{grant:?}: the pre-commit hook ran");
}

#[test]
#[ignore = "needs root to chown a scratch repo"]
fn environment_trust_for_the_exact_root_gives_full_trust() {
    assert_full_trust(&Grant::Environment(String::new()));
}

#[test]
#[ignore = "needs root to chown a scratch repo"]
fn command_line_trust_for_the_exact_root_gives_full_trust() {
    assert_full_trust(&Grant::CommandLine(String::new()));
}

#[test]
#[ignore = "needs root to chown a scratch repo"]
fn trust_for_any_other_path_or_none_stays_reduced() {
    let repo = Foreign::stage(0);
    let root = repo.display();
    let parent = repo.root.parent().expect("parent").display().to_string();
    let sibling = root[..root.len() - 1].to_string();
    let other = format!("{root}-other");
    let mut grants = vec![Grant::None];
    for value in [parent, sibling, other] {
        grants.push(Grant::Environment(value.clone()));
        grants.push(Grant::CommandLine(value));
    }
    for grant in grants {
        let log = repo.brit(&grant, &["--trace", "log", "-n", "1"]);
        assert!(
            stderr(&log).contains(REDUCED_TRUST_TRACE),
            "{grant:?}: the repository opens at reduced trust: {}",
            stderr(&log)
        );
        let commit = repo.brit(&grant, &["commit", "-m", "refused"]);
        assert!(
            !commit.status.success() && stderr(&commit).contains(HOOK_REFUSAL),
            "{grant:?}: hooks are refused: {}",
            stderr(&commit)
        );
        assert!(!repo.marker.exists(), "{grant:?}: the hook never ran");
    }
}

const MANIFEST: &str = r#"{
    "manifestVersion":"1.0",
    "pipeline":"trust-regression",
    "description":"reduced-trust fingerprint fixture",
    "steps":{
        "fingerprint":{
            "description":"read every committed file",
            "inputs":{"sources":["**"],"buildProcess":[]},
            "outputs":{"artifacts":[],"verify":null},
            "depends":[],
            "executor":{"stage":"test","function":null}
        }
    },
    "gate":{},
    "deployment":{}
}"#;

fn write_manifest(_: &Path, root: &Path) {
    std::fs::write(root.join("build-manifest.json"), MANIFEST).expect("manifest");
}

fn fingerprint(repo: &Foreign) -> Output {
    repo.brit(
        &Grant::None,
        &[
            "build",
            "fingerprint",
            repo.root.join("build-manifest.json").to_str().expect("utf8"),
            "--commit",
            "HEAD",
        ],
    )
}

fn last_line(output: &Output) -> String {
    stderr(output)
        .lines()
        .rev()
        .find(|line| !line.trim().is_empty())
        .unwrap_or_default()
        .to_string()
}

#[test]
#[ignore = "needs root to chown a scratch repo"]
fn reduced_trust_reads_and_fingerprints_beyond_32_pack_indexes() {
    for indexes in [33, 100] {
        let repo = Foreign::stage_with(indexes, write_manifest);
        let log = repo.brit(&Grant::None, &["--trace", "log", "-n", "1"]);
        assert!(log.status.success(), "{indexes} indexes: log opens: {}", stderr(&log));
        assert!(
            stderr(&log).contains(REDUCED_TRUST_TRACE),
            "{indexes} indexes: still reduced trust: {}",
            stderr(&log)
        );
        let fingerprint = fingerprint(&repo);
        assert!(
            fingerprint.status.success(),
            "{indexes} indexes: fingerprint resolves HEAD: {}",
            stderr(&fingerprint)
        );
    }
}

#[test]
#[ignore = "needs root to chown a scratch repo"]
fn reduced_trust_ceiling_is_one_named_error_with_both_cures() {
    let repo = Foreign::stage_with(1, |_, root| {
        write_manifest(root, root);
        let pack_dir = root.join(".git/objects/pack");
        std::fs::create_dir_all(&pack_dir).expect("pack dir");
        for index in 0..=gix::open::REDUCED_TRUST_MAX_PACK_INDEXES {
            std::fs::write(pack_dir.join(format!("pack-padding-{index}.idx")), []).expect("idx");
            std::fs::write(pack_dir.join(format!("pack-padding-{index}.pack")), []).expect("pack");
        }
    });
    for output in [repo.brit(&Grant::None, &["log", "-n", "1"]), fingerprint(&repo)] {
        let text = stderr(&output);
        assert!(!output.status.success(), "refused: {text}");
        let last = last_line(&output);
        assert!(
            last.contains("object database index slots exhausted")
                && last.contains("git repack -a -d")
                && last.contains("safe.directory"),
            "the last line is the named error with both cures: {text}"
        );
        assert!(
            !text.contains("rev-spec is malformed") && !text.contains("too few index slots"),
            "no wrapper and no raw slot diagnostic: {text}"
        );
    }
    let trusted = repo.brit(&Grant::Environment(repo.display()), &["log", "-n", "1"]);
    assert!(
        !stderr(&trusted).contains("index slots exhausted"),
        "trust for the checkout lifts the ceiling: {}",
        stderr(&trusted)
    );
}
