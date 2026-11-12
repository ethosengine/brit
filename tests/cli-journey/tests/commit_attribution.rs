//! Checked-in passive EPR adapter through public commits and stock Git.
#![cfg(unix)]

use anyhow::{ensure, Result};
use cli_journey::support::{runner::brit_bin, test_repo::TestRepo};
use std::{
    fs,
    os::unix::fs::{symlink, PermissionsExt},
    path::{Path, PathBuf},
    process::{Command, Output},
    time::{Duration, Instant},
};

const HOOK: &str = include_str!("../../../.githooks/prepare-commit-msg");
const MEASURED: &str = r#"{"status":"measured","trailers":[["Co-Authored-By","Test Agent <agent@example.com>"],["Codex-Session","fixture-session"]]}"#;
const MESSAGE: &str = "Scoped change\n\nBody.\n\nCo-Authored-By: Existing Human <human@example.com>\n";
const DECORATED: &str = "Scoped change\n\nBody.\n\nCo-Authored-By: Existing Human <human@example.com>\nCo-Authored-By: Test Agent <agent@example.com>\nCodex-Session: fixture-session\n";

fn executable(path: &Path, text: &str) -> Result<()> {
    fs::write(path, text)?;
    fs::set_permissions(path, fs::Permissions::from_mode(0o755))?;
    Ok(())
}

fn command(root: &Path, native: bool) -> Command {
    let mut cmd = if native {
        let mut cmd = Command::new(brit_bin().expect("required Brit binary"));
        gix_testtools::configure_git_environment(&mut cmd, root);
        cmd.current_dir(root);
        cmd
    } else {
        gix_testtools::git_command(root)
    };
    cmd.env("GIT_CONFIG_GLOBAL", root.join("absent-global"))
        .env("GIT_AUTHOR_NAME", "Attribution Author")
        .env("GIT_AUTHOR_EMAIL", "author@example.com")
        .env("GIT_COMMITTER_NAME", "Attribution Committer")
        .env("GIT_COMMITTER_EMAIL", "committer@example.com")
        .env_remove("EPR_BIN");
    cmd
}

fn success(output: Output) -> Result<Vec<u8>> {
    ensure!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    Ok(output.stdout)
}

fn git(root: &Path, args: &[&str]) -> Result<Vec<u8>> {
    success(command(root, false).args(args).output()?)
}

struct Fixture {
    _repo: TestRepo,
    root: PathBuf,
    hook: PathBuf,
    epr: PathBuf,
}

impl Fixture {
    fn new(linked: bool) -> Result<Self> {
        let repo = TestRepo::new("attribution with spaces")?;
        repo.commit_file("selected", "original\n")?;
        let root = if linked {
            git(repo.path(), &["worktree", "add", "-b", "linked", "linked space"])?;
            repo.path().join("linked space")
        } else {
            repo.path().to_path_buf()
        };
        let hooks = root.join("test hooks");
        fs::create_dir(&hooks)?;
        let hook = hooks.join("prepare-commit-msg");
        executable(&hook, HOOK)?;
        git(
            &root,
            &["config", "core.hooksPath", hooks.to_str().expect("UTF-8 fixture")],
        )?;
        let epr = root.join("fake epr");
        executable(
            &epr,
            &format!(
                "#!/bin/sh\n[ \"$*\" = 'session commit-trailers --json' ] || exit 12\nprintf '%s\\n' '{MEASURED}'\n"
            ),
        )?;
        Ok(Self {
            _repo: repo,
            root,
            hook,
            epr,
        })
    }

    fn commit(&self, native: bool, args: &[&str]) -> Result<Output> {
        Ok(command(&self.root, native)
            .env("EPR_BIN", &self.epr)
            .args(args)
            .output()?)
    }

    fn hook_command(&self, message: &Path) -> Command {
        let mut cmd = Command::new("bash");
        gix_testtools::configure_git_environment(&mut cmd, &self.root);
        cmd.current_dir(&self.root)
            .arg(&self.hook)
            .arg(message)
            .env("EPR_BIN", &self.epr);
        cmd
    }
}

#[test]
fn public_commits_preserve_scope_and_exact_attribution_in_main_and_linked_worktrees() -> Result<()> {
    for native in [true, false] {
        for linked in [false, true] {
            for shape in ["ordinary", "empty", "amend", "merge", "no-verify"] {
                let fixture = Fixture::new(linked)?;
                let root = &fixture.root;
                if shape == "merge" {
                    let branch = if linked { "linked" } else { "main" };
                    git(root, &["checkout", "-b", "side"])?;
                    fs::write(root.join("selected"), "side\n")?;
                    git(root, &["add", "selected"])?;
                    success(fixture.commit(false, &["commit", "-m", "side"])?)?;
                    git(root, &["checkout", branch])?;
                    fs::write(root.join("selected"), "main\n")?;
                    git(root, &["add", "selected"])?;
                    success(fixture.commit(false, &["commit", "-m", "main"])?)?;
                    let conflict = command(root, false).args(["merge", "--no-commit", "side"]).output()?;
                    assert!(!conflict.status.success(), "fixture must have a real merge conflict");
                }
                if shape != "empty" {
                    fs::write(root.join("selected"), "staged resolution\n")?;
                    git(root, &["add", "selected"])?;
                }
                fs::write(root.join("selected"), "unstaged neighbor\n")?;
                fs::write(root.join("private"), "untracked neighbor\n")?;
                let staged_tree = git(root, &["write-tree"])?;
                let mut args = vec!["commit", "-m", MESSAGE];
                if shape == "empty" {
                    args.push("--allow-empty");
                }
                if shape == "amend" {
                    args.push("--amend");
                }
                if shape == "no-verify" {
                    executable(&fixture.hook.with_file_name("pre-commit"), "#!/bin/sh\nexit 1\n")?;
                    args.push("--no-verify");
                }
                success(fixture.commit(native, &args)?)?;
                assert_eq!(
                    git(root, &["show", "-s", "--format=%B", "HEAD"])?,
                    format!("{DECORATED}\n").as_bytes(),
                    "native={native}, linked={linked}, shape={shape}"
                );
                assert_eq!(
                    git(root, &["rev-parse", "HEAD^{tree}"])?,
                    staged_tree,
                    "only staged bytes are committed"
                );
                assert_eq!(git(root, &["write-tree"])?, staged_tree, "staged scope is unchanged");
                assert_eq!(fs::read(root.join("selected"))?, b"unstaged neighbor\n");
                assert_eq!(fs::read(root.join("private"))?, b"untracked neighbor\n");
                if shape == "merge" {
                    let parents = git(root, &["show", "-s", "--format=%P"])?;
                    assert_eq!(String::from_utf8(parents)?.split_whitespace().count(), 2);
                }
                success(fixture.commit(
                    native,
                    &["commit", "--amend", "--no-edit", "--no-verify", "--allow-empty"],
                )?)?;
                assert_eq!(
                    git(root, &["show", "-s", "--format=%B", "HEAD"])?,
                    format!("{DECORATED}\n").as_bytes(),
                    "amending preserves contributors without duplicates"
                );
            }
        }
    }
    Ok(())
}

fn message(fixture: &Fixture) -> Result<PathBuf> {
    let path = fixture.root.join("message with spaces");
    fs::write(&path, MESSAGE)?;
    fs::set_permissions(&path, fs::Permissions::from_mode(0o640))?;
    Ok(path)
}

fn preserved(fixture: &Fixture, path: &Path, output: Output, warning: bool) -> Result<()> {
    assert!(
        output.status.success(),
        "attribution must never gate the commit: {output:?}"
    );
    assert_eq!(
        fs::read(path)?,
        MESSAGE.as_bytes(),
        "failure preserves every original byte"
    );
    assert_eq!(fs::metadata(path)?.permissions().mode() & 0o777, 0o640);
    assert_eq!(
        String::from_utf8_lossy(&output.stderr).contains("UNMEASURED"),
        warning,
        "{output:?}"
    );
    assert!(
        !fs::read_dir(&fixture.root)?.any(|entry| entry
            .expect("directory entry")
            .file_name()
            .to_string_lossy()
            .contains(".attribution.")),
        "owned temporary files are cleaned"
    );
    Ok(())
}

#[test]
fn absent_incompatible_and_malformed_responses_are_passive_and_atomic() -> Result<()> {
    let fixture = Fixture::new(false)?;
    let path = message(&fixture)?;
    for response in [
        "",
        "not-json",
        "{}",
        r#"{"status":"unmeasured","trailers":[]}"#,
        r#"{"status":"measured","trailers":[]}"#,
        r#"{"status":"measured","trailers":[["Valid","ok"],["Bad","injected\nTrailer: bad"]]}"#,
        r#"{"status":"measured","trailers":[["Key","nul\u0000value"]]}"#,
        r#"{"status":"measured","trailers":[["Key","tab\tvalue"]]}"#,
        r#"{"status":"measured","trailers":[["Key","cr\rvalue"]]}"#,
        r#"{"status":"measured","trailers":[["Key","del\u007fvalue"]]}"#,
        r#"{"status":"measured","trailers":[["Key","c1\u0085value"]]}"#,
        r#"{"status":"measured","trailers":[["Bad:Key","value"]]}"#,
        r#"{"status":"measured","trailers":[["Key",12]]}"#,
        r#"{"status":"measured","trailers":[["Key","value","extra"]]}"#,
        r#"{"status":"measured","trailers":[["Key","   "]]}"#,
        r#"{"status":"not-applicable"} {"status":"measured"}"#,
    ] {
        fs::write(fixture.root.join("response"), response)?;
        executable(&fixture.epr, "#!/bin/sh\ncat response\n")?;
        preserved(&fixture, &path, fixture.hook_command(&path).output()?, true)?;
    }
    executable(
        &fixture.epr,
        "#!/bin/sh\nprintf '%s\\n' '{\"status\":\"not-applicable\",\"trailers\":[]}'\n",
    )?;
    preserved(&fixture, &path, fixture.hook_command(&path).output()?, false)?;
    executable(&fixture.epr, "#!/bin/sh\necho 'unknown command' >&2\nexit 2\n")?;
    preserved(&fixture, &path, fixture.hook_command(&path).output()?, true)?;
    fs::remove_file(&fixture.epr)?;
    preserved(&fixture, &path, fixture.hook_command(&path).output()?, true)?;
    preserved(
        &fixture,
        &path,
        fixture.hook_command(&path).env("EPR_BIN", "").output()?,
        true,
    )?;
    Ok(())
}

// A narrow PATH makes dependency absence deterministic without touching the host.
fn tool_path(fixture: &Fixture, omit: &str, gtimeout: bool) -> Result<PathBuf> {
    let directory = fixture.root.join(format!("tools-{omit}-{gtimeout}"));
    fs::create_dir(&directory)?;
    for tool in ["bash", "jq", "git", "timeout", "mktemp", "cp", "mv", "rm"] {
        if tool == omit {
            continue;
        }
        let mut cmd = Command::new("bash");
        gix_testtools::configure_git_environment(&mut cmd, &fixture.root);
        let located = success(cmd.args(["-c", "command -v \"$1\"", "lookup", tool]).output()?)?;
        let source = String::from_utf8(located)?;
        if tool == "timeout" && gtimeout {
            // Ubuntu's coreutils requires argv[0] to match /proc/self/exe.
            // Delegate to timeout at its real name instead of renaming its symlink.
            executable(
                &directory.join("gtimeout"),
                &format!("#!/bin/sh\nexec '{}' \"$@\"\n", source.trim().replace('\'', "'\\''")),
            )?;
        } else {
            symlink(source.trim(), directory.join(tool))?;
        }
    }
    symlink(&fixture.epr, directory.join("epr"))?;
    Ok(directory)
}

#[test]
fn prerequisites_parser_failures_and_path_resolution_preserve_the_message() -> Result<()> {
    let fixture = Fixture::new(false)?;
    let path = message(&fixture)?;
    for missing in ["jq", "git", "timeout", "mktemp", "cp", "mv", "rm"] {
        let tools = tool_path(&fixture, missing, false)?;
        preserved(
            &fixture,
            &path,
            fixture.hook_command(&path).env("PATH", tools).output()?,
            true,
        )?;
    }
    let tools = tool_path(&fixture, "none", true)?;
    let output = fixture
        .hook_command(&path)
        .env("PATH", &tools)
        .env_remove("EPR_BIN")
        .output()?;
    ensure!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    assert_eq!(
        fs::read(&path)?,
        DECORATED.as_bytes(),
        "PATH EPR and gtimeout fallback work: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(fs::metadata(&path)?.permissions().mode() & 0o777, 0o640);
    success(fixture.hook_command(&path).env("PATH", &tools).output()?)?;
    assert_eq!(
        fs::read(&path)?,
        DECORATED.as_bytes(),
        "repeated invocation is idempotent"
    );
    fs::write(&path, MESSAGE)?;
    preserved(
        &fixture,
        &path,
        fixture
            .hook_command(&path)
            .env("PATH", &tools)
            .env("EPR_BIN", fixture.root.join("obsolete"))
            .output()?,
        true,
    )?;
    fs::remove_file(tools.join("git"))?;
    executable(
        &tools.join("git"),
        "#!/bin/sh\nprintf 'partial corruption\\n'\nexit 1\n",
    )?;
    preserved(
        &fixture,
        &path,
        fixture.hook_command(&path).env("PATH", &tools).output()?,
        true,
    )?;
    Ok(())
}

#[test]
fn lookup_and_processing_timeouts_are_bounded_and_passive() -> Result<()> {
    for step in ["epr", "jq", "git"] {
        let fixture = Fixture::new(false)?;
        let path = message(&fixture)?;
        let tools = tool_path(&fixture, "none", false)?;
        let stalled = if step == "epr" {
            fixture.epr.clone()
        } else {
            fs::remove_file(tools.join(step))?;
            tools.join(step)
        };
        executable(
            &stalled,
            "#!/bin/sh\ntrap '' TERM\nprintf 'partial output\\n'\nexec /bin/sleep 30\n",
        )?;
        let start = Instant::now();
        let output = fixture.hook_command(&path).env("PATH", tools).output()?;
        assert!(
            start.elapsed() >= Duration::from_secs(5),
            "real timeout must be reached"
        );
        assert!(
            start.elapsed() < Duration::from_secs(10),
            "forced termination must bound the hook"
        );
        preserved(&fixture, &path, output, true)?;
    }
    Ok(())
}
