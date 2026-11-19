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
    let outcome = Outcome {
        status: vec![status],
        notes: Vec::new(),
    };
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
        message.contains("remote moved while the before-send callback ran; nothing was sent"),
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

/// Push `specs` from `src` to `dst` while `during` changes `dst` inside the before-send callback.
#[cfg(feature = "blocking-network-client")]
fn push_while(
    src: &std::path::Path,
    dst: &std::path::Path,
    specs: &[&str],
    prune: bool,
    during: impl FnOnce(),
) -> Result<gix::remote::push::Outcome, gix::remote::push::Error> {
    let repo = gix::open_opts(src, gix::open::Options::isolated()).expect("open source repo");
    let remote = repo
        .remote_at(format!("file://{}", dst.display()).as_str())
        .expect("url");
    remote
        .connect(gix::remote::Direction::Push)
        .expect("connect")
        .prepare_push(gix::progress::Discard)
        .expect("handshake")
        .with_refspecs(specs)
        .with_prune(prune)
        .transmit_with_before_send(gix::progress::Discard, &Default::default(), |_| {
            during();
            Ok(())
        })
}

/// A remote published with `main` and two more branches, `other` and `gone`, all at the first
/// commit, plus a commit in the remote that no local ref knows.
#[cfg(feature = "blocking-network-client")]
fn setup_branched_repos() -> (std::path::PathBuf, std::path::PathBuf, String, String, String) {
    let (src, dst, first, second) = setup_published_repos();
    let dst_path = dst.to_str().expect("utf8 path");
    git_out(&src, &["push", "-q", dst_path, &format!("{first}:refs/heads/other")]);
    git_out(&src, &["push", "-q", dst_path, &format!("{first}:refs/heads/gone")]);
    let tree = git_out(&dst, &["rev-parse", "main^{tree}"]);
    let elsewhere = git_out(&dst, &["commit-tree", &tree, "-p", "main", "-m", "elsewhere"]);
    (src, dst, first, second, elsewhere)
}

#[cfg(feature = "blocking-network-client")]
fn has_object(dst: &std::path::Path, id: &str) -> bool {
    gix_testtools::git_command(dst)
        .args(["cat-file", "-e", id])
        .status()
        .expect("spawn git")
        .success()
}

#[cfg(feature = "blocking-network-client")]
fn remote_ref(dst: &std::path::Path, name: &str) -> Option<String> {
    gix_testtools::git_command(dst)
        .args(["rev-parse", "--verify", "-q", name])
        .output()
        .ok()
        .filter(|out| out.status.success())
        .map(|out| String::from_utf8(out.stdout).expect("utf8").trim().to_owned())
}

/// Row: the remote already has the new id (someone pushed the same commit) → up to date.
#[cfg(feature = "blocking-network-client")]
#[test]
fn revalidation_new_id_already_on_the_remote_is_up_to_date() {
    let (src, dst, _first, second, _) = setup_branched_repos();
    let dst_path = dst.to_str().expect("utf8 path").to_owned();
    let outcome = push_while(&src, &dst, &["refs/heads/main:refs/heads/main"], false, || {
        git_out(&src, &["push", "-q", &dst_path, "main:main"]);
    })
    .expect("the remote is already in the requested state");
    assert_eq!(outcome.status.len(), 1, "{outcome:?}");
    let status = &outcome.status[0];
    assert!(status.result.is_ok(), "{outcome:?}");
    assert_eq!(status.old_oid.to_string(), second, "reported as up to date");
    assert_eq!(status.new_oid.to_string(), second);
    assert_eq!(remote_ref(&dst, "refs/heads/main").as_deref(), Some(second.as_str()));
}

/// Row: a creation target that now exists at the id we push → up to date.
#[cfg(feature = "blocking-network-client")]
#[test]
fn revalidation_creation_already_made_with_our_id_is_up_to_date() {
    let (src, dst, _first, second, _) = setup_branched_repos();
    let dst_path = dst.to_str().expect("utf8 path").to_owned();
    let outcome = push_while(&src, &dst, &["refs/heads/main:refs/heads/newb"], false, || {
        git_out(&src, &["push", "-q", &dst_path, "main:refs/heads/newb"]);
    })
    .expect("creation already made");
    assert!(outcome.status[0].result.is_ok(), "{outcome:?}");
    assert_eq!(remote_ref(&dst, "refs/heads/newb").as_deref(), Some(second.as_str()));
}

/// Row: a creation target that now exists at another id → refused.
#[cfg(feature = "blocking-network-client")]
#[test]
fn revalidation_creation_made_elsewhere_is_refused() {
    let (src, dst, _first, second, elsewhere) = setup_branched_repos();
    let err = push_while(&src, &dst, &["refs/heads/main:refs/heads/newb"], false, || {
        git_out(&dst, &["update-ref", "refs/heads/newb", &elsewhere]);
    })
    .expect_err("created elsewhere");
    assert!(matches!(err, gix::remote::push::Error::RemoteMoved { .. }), "{err:?}");
    assert_eq!(remote_ref(&dst, "refs/heads/newb").as_deref(), Some(elsewhere.as_str()));
    assert!(!has_object(&dst, &second), "nothing was sent");
}

/// Row: a deletion target already deleted → satisfied.
#[cfg(feature = "blocking-network-client")]
#[test]
fn revalidation_deletion_already_done_is_satisfied() {
    let (src, dst, ..) = setup_branched_repos();
    let outcome = push_while(&src, &dst, &[":refs/heads/gone"], false, || {
        git_out(&dst, &["update-ref", "-d", "refs/heads/gone"]);
    })
    .expect("already deleted");
    assert!(outcome.status[0].result.is_ok(), "{outcome:?}");
    assert!(remote_ref(&dst, "refs/heads/gone").is_none());
}

/// Row: a deletion target that moved → refused.
#[cfg(feature = "blocking-network-client")]
#[test]
fn revalidation_deletion_target_moved_is_refused() {
    let (src, dst, _first, _second, elsewhere) = setup_branched_repos();
    let err = push_while(&src, &dst, &[":refs/heads/gone"], false, || {
        git_out(&dst, &["update-ref", "refs/heads/gone", &elsewhere]);
    })
    .expect_err("the deletion target moved");
    assert!(matches!(err, gix::remote::push::Error::RemoteMoved { .. }), "{err:?}");
    assert_eq!(remote_ref(&dst, "refs/heads/gone").as_deref(), Some(elsewhere.as_str()));
}

/// Row: a prune target already deleted remotely → satisfied, and the rest is delivered.
#[cfg(feature = "blocking-network-client")]
#[test]
fn revalidation_prune_target_already_deleted_is_satisfied() {
    let (src, dst, _first, second, _) = setup_branched_repos();
    let outcome = push_while(&src, &dst, &["refs/heads/*:refs/heads/*"], true, || {
        git_out(&dst, &["update-ref", "-d", "refs/heads/gone"]);
    })
    .expect("prune already done");
    assert!(outcome.status.iter().all(|status| status.result.is_ok()), "{outcome:?}");
    assert_eq!(remote_ref(&dst, "refs/heads/main").as_deref(), Some(second.as_str()));
    assert!(remote_ref(&dst, "refs/heads/gone").is_none());
    assert!(
        remote_ref(&dst, "refs/heads/other").is_none(),
        "other was pruned as judged"
    );
}

/// Row: a ref the push does not name moved → irrelevant, the push proceeds.
#[cfg(feature = "blocking-network-client")]
#[test]
fn revalidation_ignores_refs_the_push_does_not_name() {
    let (src, dst, _first, second, elsewhere) = setup_branched_repos();
    push_while(&src, &dst, &["refs/heads/main:refs/heads/main"], false, || {
        git_out(&dst, &["update-ref", "refs/heads/other", &elsewhere]);
    })
    .expect("unrelated ref moved");
    assert_eq!(remote_ref(&dst, "refs/heads/main").as_deref(), Some(second.as_str()));
}

/// Row: one of two refs moved → the whole push is refused and nothing is sent.
#[cfg(feature = "blocking-network-client")]
#[test]
fn revalidation_one_moved_ref_refuses_the_whole_push() {
    let (src, dst, first, second, elsewhere) = setup_branched_repos();
    git_out(&src, &["branch", "-f", "other", "main"]);
    let err = push_while(
        &src,
        &dst,
        &["refs/heads/main:refs/heads/main", "refs/heads/other:refs/heads/other"],
        false,
        || {
            git_out(&dst, &["update-ref", "refs/heads/other", &elsewhere]);
        },
    )
    .expect_err("other moved");
    let gix::remote::push::Error::RemoteMoved { moved } = &err else {
        panic!("expected RemoteMoved, got {err:?}");
    };
    assert_eq!(moved.len(), 1);
    assert_eq!(moved[0].refname, "refs/heads/other");
    assert_eq!(remote_ref(&dst, "refs/heads/main").as_deref(), Some(first.as_str()));
    assert!(!has_object(&dst, &second), "nothing was sent");
}

/// After a failed send, refs that were not visible are reported as such, not as undelivered,
/// and the refs that did land are listed too.
#[cfg(feature = "blocking-network-client")]
#[test]
fn delivery_check_names_refs_that_landed_and_refs_not_visible() {
    use gix::remote::push::{Delivery, MovedRef};
    let id = |hex: u8| gix::ObjectId::from_hex(&[hex; 40]).expect("hex");
    let delivery = Delivery::NotVisible {
        missing: vec![MovedRef {
            refname: "refs/heads/main".into(),
            expected: id(b'b'),
            found: id(b'a'),
        }],
        landed: vec!["refs/heads/other".into()],
    };
    let message = delivery.to_string();
    assert!(
        message.contains("was not visible on the remote when checked and may still be applied"),
        "{message}"
    );
    assert!(
        message.contains(&format!("refs/heads/main is {}, not {}", id(b'a'), id(b'b'))),
        "{message}"
    );
    assert!(
        message.contains("already at their new id: refs/heads/other"),
        "{message}"
    );
    assert!(!message.contains("not delivered"), "{message}");
}
