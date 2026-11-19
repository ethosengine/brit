/// Verify the `Outcome` and `RefStatus` public types are accessible and have
/// the expected fields without needing a live connection.
#[cfg(feature = "blocking-network-client")]
#[test]
fn push_outcome_types_are_accessible() {
    use gix::bstr::BString;
    use gix::remote::push::{Outcome, RefStatus};

    let status = RefStatus {
        local: BString::from("refs/heads/main"),
        remote: BString::from("refs/heads/main"),
        result: Ok(()),
        old_oid: gix::hash::Kind::Sha1.null(),
        new_oid: gix::hash::Kind::Sha1.null(),
    };
    let outcome = Outcome { status: vec![status] };
    assert_eq!(outcome.status.len(), 1);
    assert!(outcome.status[0].result.is_ok());
}

/// Smoke test for the entire push substrate.
///
/// Validates the full vertical: `Connection::prepare_push` → handshake →
/// `Prepare::with_refspecs` → `Prepare::transmit` → pack delivery → server
/// ref update.
///
/// Uses a `file://` URL so no daemon is needed.  The source repo is
/// constructed via `git init` + `git commit` and the destination is a bare
/// repo created with `git init --bare`.
#[cfg(feature = "blocking-network-client")]
#[test]
fn fast_forward_to_bare_file_remote_smoke() {
    let (src_dir, dst_dir) = setup_push_repos();

    // Open the source repo and create an in-memory remote pointing at the bare dst.
    let repo = gix::open_opts(&src_dir, gix::open::Options::isolated()).expect("open source repo");

    let dst_url = format!("file://{}", dst_dir.display());
    let remote = repo
        .remote_at(dst_url.as_str())
        .expect("valid file:// URL")
        .with_fetch_tags(gix::remote::fetch::Tags::None);

    // Connect in Push direction and perform the handshake.
    let conn = remote
        .connect(gix::remote::Direction::Push)
        .expect("connect to bare dst");

    let prepare = conn
        .prepare_push(gix::progress::Discard)
        .expect("prepare_push: handshake with bare dst");

    // Transmit the branch.
    let outcome = prepare
        .with_refspecs(["refs/heads/main:refs/heads/main"])
        .transmit(gix::progress::Discard, &std::sync::atomic::AtomicBool::new(false))
        .expect("transmit");

    // Verify the server acknowledged the ref update.
    assert_eq!(outcome.status.len(), 1, "expected one ref status");
    assert!(
        outcome.status[0].result.is_ok(),
        "push should succeed; got: {:?}",
        outcome.status[0].result
    );

    // Verify the bare remote actually has the ref pointing to the pushed commit.
    let bare = gix::open_opts(&dst_dir, gix::open::Options::isolated()).expect("open bare dst repo");
    let mut main_ref = bare
        .find_reference("refs/heads/main")
        .expect("refs/heads/main must exist after push");
    let _commit = main_ref.peel_to_id().expect("refs/heads/main must be resolvable");
}

/// Create a (src, dst.git) repo pair under a temporary directory and return
/// their paths.  The source repo has one empty commit on `main`; the
/// destination is a bare repo with no commits.
///
/// Panics on any `git` subprocess failure — these are test-setup steps, so
/// a clear panic is the right signal.
#[cfg(feature = "blocking-network-client")]
fn setup_push_repos() -> (std::path::PathBuf, std::path::PathBuf) {
    let tmp = gix_testtools::tempfile::TempDir::new().expect("create temp dir");

    let src = tmp.path().join("src");
    let dst = tmp.path().join("dst.git");

    let run = |dir: &std::path::Path, args: &[&str]| {
        let status = gix_testtools::git_command(dir)
            .args(args)
            .status()
            .unwrap_or_else(|e| panic!("failed to spawn git {args:?}: {e}"));
        assert!(status.success(), "git {args:?} in {dir:?} exited with {status}");
    };

    // Init source repo.
    std::fs::create_dir(&src).expect("create src dir");
    run(&src, &["init", "-q", "-b", "main"]);
    // Create an initial empty commit.
    run(
        &src,
        &[
            "-c",
            "user.email=test@example.com",
            "-c",
            "user.name=Test",
            "-c",
            "commit.gpgsign=false",
            "commit",
            "--allow-empty",
            "-q",
            "-m",
            "init",
        ],
    );

    // Init bare destination.
    std::fs::create_dir(&dst).expect("create dst dir");
    run(&dst, &["init", "--bare", "-q"]);

    // Keep the TempDir alive for the duration of the test by leaking it.
    // `keep()` consumes `TempDir`, returning a `PathBuf` and preventing cleanup.
    let tmp_path = tmp.keep();
    (tmp_path.join("src"), tmp_path.join("dst.git"))
}

/// Run `git` in `dir` and return its trimmed stdout.
#[cfg(feature = "blocking-network-client")]
fn git_out(dir: &std::path::Path, args: &[&str]) -> String {
    let out = gix_testtools::git_command(dir)
        .args(["-c", "user.email=test@example.com", "-c", "user.name=Test"])
        .args(args)
        .output()
        .unwrap_or_else(|e| panic!("failed to spawn git {args:?}: {e}"));
    assert!(out.status.success(), "git {args:?} in {dir:?}: {out:?}");
    String::from_utf8(out.stdout).expect("utf8").trim().to_owned()
}

/// A remote with `main` at the source's first commit, and a second local commit to push.
#[cfg(feature = "blocking-network-client")]
fn setup_published_repos() -> (std::path::PathBuf, std::path::PathBuf, String, String) {
    let (src, dst) = setup_push_repos();
    let first = git_out(&src, &["rev-parse", "HEAD"]);
    git_out(&src, &["push", "-q", dst.to_str().expect("utf8 path"), "main:main"]);
    git_out(&src, &["commit", "--allow-empty", "-q", "-m", "second"]);
    let second = git_out(&src, &["rev-parse", "HEAD"]);
    (src, dst, first, second)
}

/// A callback that may have waited gets a fresh connection, and the push lands there.
#[cfg(feature = "blocking-network-client")]
#[test]
fn revalidated_push_is_delivered_on_a_fresh_connection() {
    let (src, dst, first, second) = setup_published_repos();
    let repo = gix::open_opts(&src, gix::open::Options::isolated()).expect("open source repo");
    let remote = repo
        .remote_at(format!("file://{}", dst.display()).as_str())
        .expect("url");
    let mut judged = None;
    let outcome = remote
        .connect(gix::remote::Direction::Push)
        .expect("connect")
        .prepare_push(gix::progress::Discard)
        .expect("handshake")
        .with_refspecs(["refs/heads/main:refs/heads/main"])
        .transmit_with_before_send(gix::progress::Discard, &Default::default(), |updates| {
            judged = Some(updates[0].old_oid.to_string());
            Ok(())
        })
        .expect("revalidated push");
    assert!(outcome.status[0].result.is_ok(), "{outcome:?}");
    assert_eq!(
        judged.as_deref(),
        Some(first.as_str()),
        "the callback judged the prior id"
    );
    assert_eq!(git_out(&dst, &["rev-parse", "refs/heads/main"]), second);
}

/// A remote that moves while the callback runs refuses the push before any byte is sent.
#[cfg(feature = "blocking-network-client")]
#[test]
fn remote_moved_during_before_send_refuses_and_sends_nothing() {
    let (src, dst, first, second) = setup_published_repos();
    let repo = gix::open_opts(&src, gix::open::Options::isolated()).expect("open source repo");
    let remote = repo
        .remote_at(format!("file://{}", dst.display()).as_str())
        .expect("url");
    let mut moved_to = String::new();
    let err = remote
        .connect(gix::remote::Direction::Push)
        .expect("connect")
        .prepare_push(gix::progress::Discard)
        .expect("handshake")
        .with_refspecs(["refs/heads/main:refs/heads/main"])
        .transmit_with_before_send(gix::progress::Discard, &Default::default(), |_| {
            let tree = git_out(&dst, &["rev-parse", "main^{tree}"]);
            moved_to = git_out(&dst, &["commit-tree", &tree, "-p", "main", "-m", "elsewhere"]);
            git_out(&dst, &["update-ref", "refs/heads/main", &moved_to]);
            Ok(())
        })
        .expect_err("the remote moved");
    let gix::remote::push::Error::RemoteMoved { moved } = &err else {
        panic!("expected RemoteMoved, got {err:?}");
    };
    assert_eq!(moved.len(), 1);
    assert_eq!(moved[0].refname, "refs/heads/main");
    assert_eq!(moved[0].expected.to_string(), first);
    assert_eq!(moved[0].found.to_string(), moved_to);
    let message = err.to_string();
    assert!(
        message.contains("remote moved while pre-push ran; nothing was sent"),
        "{message}"
    );
    assert!(message.contains(&first) && message.contains(&moved_to), "{message}");
    assert_eq!(git_out(&dst, &["rev-parse", "refs/heads/main"]), moved_to);
    let missing = gix_testtools::git_command(&dst)
        .args(["cat-file", "-e", &second])
        .status()
        .expect("spawn git");
    assert!(!missing.success(), "no object was sent");
}
