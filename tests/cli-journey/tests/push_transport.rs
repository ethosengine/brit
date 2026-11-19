//! Push over transports that fail: the real cause reaches the user, a `pre-push` hook never
//! leaves the push on a stale connection, and a remote that moved during the hook is refused.
//!
//! Every shipped brit is built `max-pure` and speaks HTTP through reqwest, while a default
//! build uses curl. Each journey therefore runs against both: the curl build is the usual
//! `BRIT_BIN` (or the profile binary), the reqwest build is named by `BRIT_REQWEST_BIN`.
//! The dedicated CI step sets `BRIT_REQWEST_BIN` and `BRIT_REQWEST_REQUIRED=1`; with the
//! latter, a missing reqwest build fails the journeys instead of skipping its leg.
//!
//! Linux only, like the other journeys that run hooks and `git http-backend`.
#![cfg(target_os = "linux")]
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
            std::env::var_os("BRIT_REQWEST_REQUIRED").is_none(),
            "BRIT_REQWEST_BIN must name a max-pure brit: the shipped HTTP backend is otherwise untested"
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
    push_with(build, repo, &[])
}

fn push_with(build: &Build, repo: &Path, env: &[(&str, &str)]) -> Result<Capture> {
    let mut invocation = BritInvocation::new(&build.bin)
        .args(["push", "origin", "main:main"])
        .current_dir(repo)
        .deadline(DEADLINE);
    for (key, value) in env {
        invocation = invocation.env(key, value);
    }
    invocation.run().with_context(|| format!("{} push", build.http))
}

/// The test knob that replaces the 60 s request deadline of the reqwest backend.
const SHORT_DEADLINE: (&str, &str) = ("BRIT_HTTP_REQUEST_DEADLINE_SECONDS", "3");

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
            out.stderr.matches("the server's report was lost").count() == 1
                && out
                    .stderr
                    .contains("A fresh advertisement shows every ref this push named at its requested id"),
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

/// The rule and its exception, in seconds: with the request deadline shortened to 3 s, a push
/// whose response the remote holds for 5 s still completes, because the push upload has no
/// deadline, while a handshake GET held for 5 s fails naming a timeout. This is the regression
/// test for the production failure (a 30 s cap on every request), so it runs by default.
#[test]
fn push_upload_has_no_deadline_while_handshakes_keep_theirs() -> Result<()> {
    let Some(build) = reqwest_build() else {
        return Ok(());
    };
    let published = Published::new("push-transport-held")?;
    let server = HttpRemote::start(published.root.path(), Mode::HoldResponse(Duration::from_secs(5)))?;
    published.origin(&server.url("r.git"))?;
    let new = published.repo.commit_file("payload", "second")?;
    let out = push_with(&build, published.repo.path(), &[SHORT_DEADLINE])?;
    ensure!(
        out.status.success(),
        "the held push response is past the deadline but the upload has none:\n{}\nserver: {:?}",
        out.stderr,
        server.log()
    );
    assert_eq!(published.remote_main()?, new);

    let published = Published::new("push-transport-held-get")?;
    let server = HttpRemote::start(published.root.path(), Mode::HoldGet(Duration::from_secs(5)))?;
    published.origin(&server.url("r.git"))?;
    published.repo.commit_file("payload", "second")?;
    let started = std::time::Instant::now();
    let out = push_with(&build, published.repo.path(), &[SHORT_DEADLINE])?;
    let took = started.elapsed();
    ensure!(!out.status.success(), "the held handshake must fail");
    ensure!(
        out.stderr.to_lowercase().contains("timed out"),
        "a timeout is named:\n{}",
        out.stderr
    );
    ensure!(took < Duration::from_secs(5), "cut at the 3 s deadline, took {took:?}");
    assert_eq!(published.remote_main()?, published.first);
    Ok(())
}

/// `http.postBuffer` decides how a push body is sent: whole with a `Content-Length` up to it,
/// streamed in chunks above it.
#[test]
fn post_buffer_switches_between_whole_and_streamed_bodies() -> Result<()> {
    for build in builds() {
        for (size, framing) in [(32 * 1024, "(content-length"), (128 * 1024, "(chunked)")] {
            let what = format!("{} {size} bytes", build.http);
            let published = Published::new(&format!("push-transport-postbuffer-{}", build.http))?;
            git(published.repo.path(), &["config", "http.postBuffer", "65536"])?;
            let server = HttpRemote::start(published.root.path(), Mode::Serve)?;
            published.origin(&server.url("r.git"))?;
            let new = published.commit_bytes(size)?;
            let out = push(&build, published.repo.path())?;
            ensure!(out.status.success(), "{what}: {}", out.stderr);
            assert_eq!(published.remote_main()?, new, "{what}");
            let log = server.log();
            ensure!(
                log.iter()
                    .any(|line| line.starts_with("POST /r.git/git-receive-pack") && line.contains(framing)),
                "{what}: the body was sent {framing}: {log:?}"
            );
        }
    }
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
