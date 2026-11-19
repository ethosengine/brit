//! Push over transports that fail: the real cause reaches the user, a `pre-push` hook never
//! leaves the push on a stale connection, and a remote that moved during the hook is refused.
//!
//! Every shipped brit is built `max-pure` and speaks HTTP through reqwest, while a default
//! build uses curl. Each journey therefore runs against both: the curl build is the usual
//! `BRIT_BIN` (or the profile binary), the reqwest build is named by `BRIT_REQWEST_BIN`.
//! CI must set `BRIT_REQWEST_BIN`; without it these journeys fail there rather than skip.
use std::{
    fs,
    path::{Path, PathBuf},
    time::Duration,
};

use anyhow::{ensure, Context, Result};
use cli_journey::support::{
    http_remote::{HttpRemote, Mode},
    runner::{brit_bin, BritInvocation, Capture},
    test_repo::TestRepo,
};

/// How long a push may take before the journey fails instead of hanging.
const DEADLINE: Duration = Duration::from_secs(90);

#[derive(Clone)]
struct Build {
    /// `curl` or `reqwest`, for messages.
    http: &'static str,
    bin: PathBuf,
}

fn presets(bin: &Path) -> String {
    let out = std::process::Command::new(bin)
        .arg("--version")
        .output()
        .unwrap_or_else(|err| panic!("run {} --version: {err}", bin.display()));
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .find_map(|line| line.strip_prefix("frontend presets: "))
        .unwrap_or_default()
        .to_owned()
}

/// The reqwest (`max-pure`) build named by `BRIT_REQWEST_BIN`.
fn reqwest_build() -> Option<Build> {
    let Some(path) = std::env::var_os("BRIT_REQWEST_BIN") else {
        assert!(
            std::env::var_os("CI").is_none(),
            "BRIT_REQWEST_BIN must name a max-pure brit in CI: the shipped HTTP backend is otherwise untested"
        );
        eprintln!("BRIT_REQWEST_BIN is unset: the reqwest journeys did NOT run");
        return None;
    };
    let bin = PathBuf::from(path)
        .canonicalize()
        .expect("BRIT_REQWEST_BIN names an existing brit");
    assert_eq!(
        presets(&bin),
        "max-pure",
        "BRIT_REQWEST_BIN must be a max-pure (reqwest) build: {}",
        bin.display()
    );
    Some(Build { http: "reqwest", bin })
}

fn curl_build() -> Build {
    let bin = brit_bin().expect("required binary");
    assert_ne!(
        presets(&bin),
        "max-pure",
        "BRIT_BIN is expected to be a default (curl) build"
    );
    Build { http: "curl", bin }
}

fn builds() -> Vec<Build> {
    let mut out = vec![curl_build()];
    out.extend(reqwest_build());
    out
}

fn git(dir: &Path, args: &[&str]) -> Result<String> {
    let output = gix_testtools::git_command(dir)
        .args(["-c", &format!("safe.directory={}", dir.display())])
        .args(args)
        .output()?;
    ensure!(
        output.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    Ok(String::from_utf8(output.stdout)?.trim().to_owned())
}

fn push(build: &Build, repo: &Path) -> Result<Capture> {
    BritInvocation::new(&build.bin)
        .args(["push", "origin", "main:main"])
        .current_dir(repo)
        .deadline(DEADLINE)
        .run()
        .with_context(|| format!("{} push", build.http))
}

/// A local repository whose `main` is published to a bare `r.git` below a project root.
struct Published {
    repo: TestRepo,
    root: tempfile::TempDir,
    bare: PathBuf,
    first: String,
}

impl Published {
    fn new(label: &str) -> Result<Self> {
        let repo = TestRepo::new(label)?;
        let first = repo.commit_file("payload", "first")?;
        let root = tempfile::tempdir()?;
        git(root.path(), &["init", "--bare", "-q", "r.git"])?;
        let bare = root.path().join("r.git");
        let published = Published {
            repo,
            root,
            bare,
            first,
        };
        git(published.bare(), &["config", "http.receivepack", "true"])?;
        git(
            published.repo.path(),
            &["push", "-q", published.bare().to_str().expect("utf8 path"), "main:main"],
        )?;
        Ok(published)
    }

    fn bare(&self) -> &Path {
        &self.bare
    }

    fn origin(&self, url: &str) -> Result<()> {
        git(self.repo.path(), &["remote", "add", "origin", url]).map(drop)
    }

    fn remote_main(&self) -> Result<String> {
        git(self.bare(), &["rev-parse", "refs/heads/main"])
    }

    fn remote_has(&self, id: &str) -> bool {
        git(self.bare(), &["cat-file", "-e", id]).is_ok()
    }

    fn hook(&self, body: &str) -> Result<()> {
        let dir = self.repo.path().join("hooks");
        fs::create_dir_all(&dir)?;
        git(self.repo.path(), &["config", "core.hooksPath", "hooks"])?;
        let path = dir.join("pre-push");
        fs::write(&path, format!("#!/bin/sh\n{body}\n"))?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(path, fs::Permissions::from_mode(0o755))?;
        }
        Ok(())
    }

    /// Commit `len` incompressible bytes so the pack is at least that large.
    fn commit_bytes(&self, len: usize) -> Result<String> {
        let mut state = 0x9e37_79b9_7f4a_7c15_u64;
        let bytes: Vec<u8> = (0..len)
            .map(|_| {
                state ^= state << 13;
                state ^= state >> 7;
                state ^= state << 17;
                state.to_le_bytes()[0]
            })
            .collect();
        fs::write(self.repo.path().join("noise.bin"), bytes)?;
        git(self.repo.path(), &["add", "noise.bin"])?;
        git(self.repo.path(), &["commit", "-q", "-m", "noise"])?;
        git(self.repo.path(), &["rev-parse", "HEAD"])
    }
}

fn never_masked(stderr: &str) -> Result<()> {
    ensure!(
        !stderr.contains("closed channel"),
        "the dropped channel was reported instead of the cause:\n{stderr}"
    );
    ensure!(
        !stderr.contains("unexpected end of stream"),
        "a read error was reported as end of stream:\n{stderr}"
    );
    Ok(())
}

/// A keep-alive connection the server forgot while the hook ran must not carry the pack.
#[test]
fn connection_dropped_during_the_hook_still_delivers() -> Result<()> {
    for build in builds() {
        let published = Published::new(&format!("push-transport-idle-{}", build.http))?;
        let server = HttpRemote::start(published.root.path(), Mode::DropIdle(Duration::from_millis(300)))?;
        published.origin(&server.url("r.git"))?;
        published.hook("cat >hook-input; sleep 1")?;
        let new = published.repo.commit_file("payload", "second")?;
        let out = push(&build, published.repo.path())?;
        ensure!(
            out.status.success(),
            "{}: push failed\n{}\nserver: {:?}",
            build.http,
            out.stderr,
            server.log()
        );
        assert_eq!(
            published.remote_main()?,
            new,
            "{}: the remote has the commit",
            build.http
        );
    }
    Ok(())
}

/// An early 413 is named, not hidden behind the closed upload, and nothing claims delivery.
#[test]
fn early_rejection_names_the_http_status() -> Result<()> {
    for build in builds() {
        for status in [413u16, 401] {
            let published = Published::new(&format!("push-transport-early-{status}-{}", build.http))?;
            let server = HttpRemote::start(published.root.path(), Mode::EarlyStatus(status))?;
            published.origin(&server.url("r.git"))?;
            // A large body is what turned the cause into a closed channel. The curl backend
            // can hang forever on an early answer to a large body (upstream), so keep it small there.
            let new = if build.http == "reqwest" {
                published.commit_bytes(64 * 1024)?
            } else {
                published.repo.commit_file("payload", "second")?
            };
            let out = push(&build, published.repo.path())?;
            ensure!(!out.status.success(), "{}: {status} must fail the push", build.http);
            ensure!(
                out.stderr.contains(&status.to_string()),
                "{}: the {status} is named:\n{}\nserver: {:?}",
                build.http,
                out.stderr,
                server.log()
            );
            never_masked(&out.stderr)?;
            ensure!(
                out.stderr.contains("was not visible on the remote when checked"),
                "{}: delivery was checked:\n{}",
                build.http,
                out.stderr
            );
            assert_eq!(published.remote_main()?, published.first, "{}", build.http);
            ensure!(!published.remote_has(&new), "{}: nothing arrived", build.http);
        }
    }
    Ok(())
}

/// A remote that went away after advertising its refs is named as such, and nothing is sent.
#[test]
fn listener_gone_after_the_advertisement_is_named() -> Result<()> {
    for build in builds() {
        let published = Published::new(&format!("push-transport-gone-{}", build.http))?;
        let server = HttpRemote::start(published.root.path(), Mode::GoneAfterGet)?;
        published.origin(&server.url("r.git"))?;
        published.hook("cat >hook-input")?;
        published.repo.commit_file("payload", "second")?;
        let out = push(&build, published.repo.path())?;
        ensure!(!out.status.success(), "{}: the push cannot succeed", build.http);
        ensure!(
            out.stderr.contains("nothing was sent"),
            "{}: the user learns nothing was sent:\n{}",
            build.http,
            out.stderr
        );
        ensure!(
            ["refused", "couldn't connect", "failed to connect"]
                .iter()
                .any(|cause| out.stderr.to_lowercase().contains(cause)),
            "{}: the refused connection is named:\n{}",
            build.http,
            out.stderr
        );
        never_masked(&out.stderr)?;
        assert_eq!(published.remote_main()?, published.first, "{}", build.http);
    }
    Ok(())
}

/// A report lost after the remote applied the push: verified on a fresh handshake, the push
/// succeeds and says its report was lost.
#[test]
fn lost_report_after_delivery_says_the_push_arrived() -> Result<()> {
    for build in builds() {
        let published = Published::new(&format!("push-transport-lost-{}", build.http))?;
        let server = HttpRemote::start(published.root.path(), Mode::LoseReport)?;
        published.origin(&server.url("r.git"))?;
        let new = published.repo.commit_file("payload", "second")?;
        let out = push(&build, published.repo.path())?;
        ensure!(
            out.status.success(),
            "{}: a verified delivery succeeds:\n{}",
            build.http,
            out.stderr
        );
        ensure!(
            out.stderr.contains("the server's report was lost")
                && out
                    .stderr
                    .contains("a fresh handshake shows every update on the remote"),
            "{}: the lost report is noted:\n{}\nserver: {:?}",
            build.http,
            out.stderr,
            server.log()
        );
        never_masked(&out.stderr)?;
        assert_eq!(published.remote_main()?, new, "{}: the push arrived once", build.http);
    }
    Ok(())
}

/// The remote URLs each journey that applies to every transport runs against.
fn remotes<'a>(published: &Published, server: &'a HttpRemote, builds: &'a [Build]) -> Vec<(String, &'a Build)> {
    let mut out = vec![(published.bare().display().to_string(), &builds[0])];
    out.extend(builds.iter().map(|build| (server.url("r.git"), build)));
    out
}

/// A hook that moves the remote ref sees the push refused with both ids, and the remote untouched.
#[test]
fn remote_moved_by_the_hook_is_refused() -> Result<()> {
    let builds = builds();
    for transport in 0..=builds.len() {
        let published = Published::new(&format!("push-transport-moved-{transport}"))?;
        let server = HttpRemote::start(published.root.path(), Mode::Serve)?;
        let (url, build) = remotes(&published, &server, &builds).swap_remove(transport);
        published.origin(&url)?;
        let tree = git(published.bare(), &["rev-parse", "main^{tree}"])?;
        let elsewhere = git(
            published.bare(),
            &[
                "-c",
                "user.name=t",
                "-c",
                "user.email=t@example.invalid",
                "commit-tree",
                &tree,
                "-p",
                "main",
                "-m",
                "elsewhere",
            ],
        )?;
        published.hook(&format!(
            "cat >hook-input; git --git-dir='{}' update-ref refs/heads/main {elsewhere}",
            published.bare().display()
        ))?;
        let new = published.repo.commit_file("payload", "second")?;
        let out = push(build, published.repo.path())?;
        let what = format!("{} via {url}", build.http);
        ensure!(!out.status.success(), "{what}: the push must be refused");
        ensure!(
            out.stderr
                .contains("remote moved while the before-send callback ran; nothing was sent"),
            "{what}:\n{}",
            out.stderr
        );
        ensure!(
            out.stderr.contains(&published.first) && out.stderr.contains(&elsewhere),
            "{what}: both the judged and the found id are named:\n{}",
            out.stderr
        );
        assert_eq!(published.remote_main()?, elsewhere, "{what}: the remote is unchanged");
        ensure!(!published.remote_has(&new), "{what}: no object was sent");
    }
    Ok(())
}

/// The hook judges the remote's prior id, and the push lands exactly there.
#[test]
fn hook_input_old_id_is_the_remote_prior_id() -> Result<()> {
    let builds = builds();
    for transport in 0..=builds.len() {
        let published = Published::new(&format!("push-transport-stdin-{transport}"))?;
        let server = HttpRemote::start(published.root.path(), Mode::Serve)?;
        let (url, build) = remotes(&published, &server, &builds).swap_remove(transport);
        published.origin(&url)?;
        published.hook("cat >hook-input")?;
        let new = published.repo.commit_file("payload", "second")?;
        let out = push(build, published.repo.path())?;
        let what = format!("{} via {url}", build.http);
        ensure!(out.status.success(), "{what}: {}", out.stderr);
        let input = fs::read_to_string(published.repo.path().join("hook-input"))?;
        assert_eq!(
            input,
            format!("refs/heads/main {new} refs/heads/main {}\n", published.first),
            "{what}: the hook saw the remote's prior id"
        );
        assert_eq!(published.remote_main()?, new, "{what}");
    }
    Ok(())
}

/// The push upload has no deadline: a remote answering after 35 s still completes the push.
/// This is the regression test for the production failure, so it runs by default on the
/// reqwest build (the build that had a 30 s cap).
#[test]
fn response_held_past_thirty_seconds_completes() -> Result<()> {
    let Some(build) = reqwest_build() else {
        return Ok(());
    };
    let published = Published::new("push-transport-held")?;
    let server = HttpRemote::start(published.root.path(), Mode::HoldResponse(Duration::from_secs(35)))?;
    published.origin(&server.url("r.git"))?;
    let new = published.repo.commit_file("payload", "second")?;
    let out = push(&build, published.repo.path())?;
    ensure!(out.status.success(), "{}\nserver: {:?}", out.stderr, server.log());
    assert_eq!(published.remote_main()?, new);
    Ok(())
}

/// An upload cut in the middle of the pack, by an early answer or a reset, names that cause
/// and never the dropped channel the transport saw it as.
#[test]
fn upload_cut_mid_pack_names_the_cause() -> Result<()> {
    for build in builds() {
        // Large bodies are what the cut needs; the curl backend can hang forever on an early
        // answer to a large body (upstream), so it gets small ones.
        let (size, cut) = if build.http == "reqwest" {
            (1024 * 1024, 100_000)
        } else {
            (0, 64)
        };
        for mode in [Mode::EarlyStatusReset(413), Mode::ResetAfter(cut)] {
            let what = format!("{} {mode:?}", build.http);
            let published = Published::new(&format!("push-transport-cut-{}", build.http))?;
            let server = HttpRemote::start(published.root.path(), mode)?;
            published.origin(&server.url("r.git"))?;
            let new = if size > 0 {
                published.commit_bytes(size)?
            } else {
                published.repo.commit_file("payload", "second")?
            };
            let out = push(&build, published.repo.path())?;
            ensure!(!out.status.success(), "{what}: the push cannot succeed");
            let stderr = out.stderr.to_lowercase();
            // curl retries a send on a reset connection and, lacking a seek callback in the
            // upstream backend, reports its failed rewind ("[65] Send failed …") instead.
            let curl_retry = build.http == "curl" && stderr.contains("send failed");
            ensure!(
                stderr.contains("413") || stderr.contains("reset") || curl_retry,
                "{what}: the status or the reset is named:\n{}\nserver: {:?}",
                out.stderr,
                server.log()
            );
            never_masked(&out.stderr)?;
            ensure!(
                out.stderr.contains("was not visible on the remote when checked"),
                "{what}: delivery was checked:\n{}",
                out.stderr
            );
            assert_eq!(published.remote_main()?, published.first, "{what}");
            ensure!(!published.remote_has(&new), "{what}: nothing arrived");
        }
    }
    Ok(())
}

/// After the second handshake only receive-pack's compare-and-swap guards the ref; a writer
/// that moves it when the upload arrives gets the push rejected by the server, and brit says so.
#[test]
fn server_compare_and_swap_after_the_second_handshake_is_reported() -> Result<()> {
    for build in builds() {
        let published = Published::new(&format!("push-transport-cas-{}", build.http))?;
        let server = HttpRemote::start(published.root.path(), Mode::MoveOnPost)?;
        published.origin(&server.url("r.git"))?;
        published.hook("cat >hook-input")?;
        let new = published.repo.commit_file("payload", "second")?;
        let out = push(&build, published.repo.path())?;
        ensure!(!out.status.success(), "{}: the server rejected the update", build.http);
        ensure!(
            [
                "incorrect old value provided",
                "failed to update ref",
                "cannot lock ref"
            ]
            .iter()
            .any(|reason| out.stderr.contains(reason)),
            "{}: the server's rejection is reported:\n{}\nserver: {:?}",
            build.http,
            out.stderr,
            server.log()
        );
        ensure!(published.remote_main()? != new, "{}: nothing was delivered", build.http);
        ensure!(
            published.remote_main()? != published.first,
            "{}: the competing writer's id stands",
            build.http
        );
    }
    Ok(())
}

/// A handshake GET held past the 60 s deadline fails naming a timeout instead of hanging.
#[test]
#[ignore = "takes 60 s"]
fn handshake_held_past_the_deadline_names_a_timeout() -> Result<()> {
    let build = reqwest_build().context("BRIT_REQWEST_BIN is required for this journey")?;
    let published = Published::new("push-transport-holdget")?;
    let server = HttpRemote::start(published.root.path(), Mode::HoldGet(Duration::from_secs(65)))?;
    published.origin(&server.url("r.git"))?;
    published.repo.commit_file("payload", "second")?;
    let started = std::time::Instant::now();
    let out = push(&build, published.repo.path())?;
    let took = started.elapsed();
    ensure!(!out.status.success(), "the held handshake cannot succeed");
    ensure!(
        out.stderr.to_lowercase().contains("timed out"),
        "a timeout is named:\n{}",
        out.stderr
    );
    ensure!(
        (Duration::from_secs(55)..Duration::from_secs(65)).contains(&took),
        "cut at the 60 s deadline, took {took:?}"
    );
    assert_eq!(published.remote_main()?, published.first);
    Ok(())
}

/// A fetch download that stalls is cut by the per-read stall guard instead of hanging.
#[test]
#[ignore = "takes 60 s"]
fn stalled_clone_download_is_cut_by_the_stall_guard() -> Result<()> {
    let build = reqwest_build().context("BRIT_REQWEST_BIN is required for this journey")?;
    let published = Published::new("push-transport-stallfetch")?;
    let big = published.commit_bytes(2 * 1024 * 1024)?;
    git(
        published.repo.path(),
        &["push", "-q", published.bare().to_str().expect("utf8 path"), "main:main"],
    )?;
    assert_eq!(published.remote_main()?, big);
    let server = HttpRemote::start(published.root.path(), Mode::StallFetch(64 * 1024))?;
    let target = tempfile::tempdir()?;
    let started = std::time::Instant::now();
    let out = BritInvocation::new(&build.bin)
        .args(["clone".to_owned(), server.url("r.git"), "cloned".to_owned()])
        .current_dir(target.path())
        .deadline(DEADLINE)
        .run()?;
    let took = started.elapsed();
    ensure!(!out.status.success(), "the stalled clone cannot succeed");
    ensure!(
        out.stderr.to_lowercase().contains("timed out"),
        "a timeout is named:\n{}\nserver: {:?}",
        out.stderr,
        server.log()
    );
    ensure!(took < Duration::from_secs(80), "cut by the stall guard, took {took:?}");
    Ok(())
}
