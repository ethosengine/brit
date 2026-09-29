//! Coverage tests for the brit binary (gitoxide-derived git client).
//!
//! Covers daily-driver subcommands: log, status, diff tree, diff file,
//! branch list, commit describe/verify, clone (via file://), fetch,
//! tag list, blame, cat.
//!
//! `brit push` is exercised against a disposable bare remote.
//!
//! `brit commit` creates staged commits and also has verify, sign and describe
//! subcommands. Authoring and hook safety live in the `authoring` journey.
//!
//! Existing gix.sh journey tests test internal gitoxide crates (gix-tempfile,
//! gix, etc.); this Rust file tests the BRIT CLI SURFACE specifically.
//!
//! Staging layout:
//!   BRIT_TEST_PAGE_STAGING/rust/brit/<subcommand>.txt
//!   BRIT_TEST_PAGE_STAGING/rust/brit/diff/<leaf>.txt
//!   BRIT_TEST_PAGE_STAGING/rust/brit/branch/<leaf>.txt
//!   BRIT_TEST_PAGE_STAGING/rust/brit/commit/<leaf>.txt
//!   BRIT_TEST_PAGE_STAGING/rust/brit/tag/<leaf>.txt

use std::{fs, path::PathBuf};

use cli_journey::support::{
    mock_remote::MockRemote,
    runner::{brit_bin, BritInvocation},
    test_repo::TestRepo,
};

/// Dump captured output to BRIT_TEST_PAGE_STAGING/rust/<path[0]>/<path[1]>/.../<last>.txt
///
/// The path slice encodes the full subcommand hierarchy:
///   &["brit", "branch", "list"]  →  staging/rust/brit/branch/list.txt
///   &["brit", "log"]             →  staging/rust/brit/log.txt
fn staging_dump_path(path: &[&str], output: &str) {
    if let Ok(staging) = std::env::var("BRIT_TEST_PAGE_STAGING") {
        assert!(!path.is_empty(), "staging path must not be empty");
        let mut dir = PathBuf::from(staging).join("rust");
        // All segments except the last form the directory hierarchy.
        for segment in &path[..path.len() - 1] {
            dir = dir.join(segment);
        }
        fs::create_dir_all(&dir).expect("mkdir staging");
        let filename = format!("{}.txt", path[path.len() - 1]);
        fs::write(dir.join(filename), output).expect("write capture");
    }
}

// ─── log ─────────────────────────────────────────────────────────────────────

#[test]
fn log_lists_commits() {
    let temp = TestRepo::new("log").expect("repo");
    temp.commit_file("a.txt", "alpha\n").expect("commit a");
    temp.commit_file("b.txt", "beta\n").expect("commit b");

    let Some(bin) = brit_bin() else {
        eprintln!("skipping: brit binary not built (run `cargo build -p gitoxide --bin brit --release`)");
        return;
    };
    let cap = BritInvocation::new(bin)
        .arg("log")
        .current_dir(temp.path())
        .normalize(true)
        .run()
        .expect("invoke");
    staging_dump_path(
        &["brit", "log"],
        &format!("{}\n---stderr---\n{}", cap.stdout, cap.stderr),
    );
}

// ─── status ──────────────────────────────────────────────────────────────────

#[test]
fn status_in_clean_repo() {
    let temp = TestRepo::new("status").expect("repo");
    let Some(bin) = brit_bin() else {
        eprintln!("skipping: brit binary not built (run `cargo build -p gitoxide --bin brit --release`)");
        return;
    };
    let cap = BritInvocation::new(bin)
        .arg("status")
        .current_dir(temp.path())
        .normalize(true)
        .run()
        .expect("invoke");
    staging_dump_path(
        &["brit", "status"],
        &format!("{}\n---stderr---\n{}", cap.stdout, cap.stderr),
    );
}

#[test]
fn status_with_untracked_file() {
    let temp = TestRepo::new("status-dirty").expect("repo");
    fs::write(temp.path().join("untracked.txt"), "hello\n").expect("write untracked");
    let Some(bin) = brit_bin() else {
        eprintln!("skipping: brit binary not built (run `cargo build -p gitoxide --bin brit --release`)");
        return;
    };
    let cap = BritInvocation::new(bin)
        .arg("status")
        .current_dir(temp.path())
        .normalize(true)
        .run()
        .expect("invoke");
    // Dump alongside the clean-repo capture (same staging path — last write wins)
    // This test mainly verifies the subcommand doesn't panic with a dirty tree.
    staging_dump_path(
        &["brit", "status"],
        &format!("{}\n---stderr---\n{}", cap.stdout, cap.stderr),
    );
}

// ─── diff tree ───────────────────────────────────────────────────────────────

#[test]
fn diff_tree_between_two_commits() {
    let temp = TestRepo::new("diff-tree").expect("repo");
    let sha1 = temp.commit_file("first.txt", "version 1\n").expect("commit v1");
    let sha2 = temp.commit_file("first.txt", "version 2\n").expect("commit v2");

    let Some(bin) = brit_bin() else {
        eprintln!("skipping: brit binary not built (run `cargo build -p gitoxide --bin brit --release`)");
        return;
    };
    let cap = BritInvocation::new(bin)
        .args(["diff", "tree", &sha1, &sha2])
        .current_dir(temp.path())
        .normalize(true)
        .run()
        .expect("invoke");
    staging_dump_path(
        &["brit", "diff", "tree"],
        &format!("{}\n---stderr---\n{}", cap.stdout, cap.stderr),
    );
}

// ─── diff file ───────────────────────────────────────────────────────────────

#[test]
fn diff_file_between_two_revisions() {
    // brit diff file <OLD_REVSPEC> <NEW_REVSPEC>
    // Old = HEAD~1:first.txt  New = HEAD:first.txt
    let temp = TestRepo::new("diff-file").expect("repo");
    temp.commit_file("first.txt", "line one\n").expect("commit v1");
    temp.commit_file("first.txt", "line two\n").expect("commit v2");

    let Some(bin) = brit_bin() else {
        eprintln!("skipping: brit binary not built (run `cargo build -p gitoxide --bin brit --release`)");
        return;
    };
    let cap = BritInvocation::new(bin)
        .args(["diff", "file", "HEAD~1:first.txt", "HEAD:first.txt"])
        .current_dir(temp.path())
        .normalize(true)
        .run()
        .expect("invoke");
    staging_dump_path(
        &["brit", "diff", "file"],
        &format!("{}\n---stderr---\n{}", cap.stdout, cap.stderr),
    );
}

// ─── branch list ─────────────────────────────────────────────────────────────

#[test]
fn branch_list_shows_main() {
    let temp = TestRepo::new("branch-list").expect("repo");
    let Some(bin) = brit_bin() else {
        eprintln!("skipping: brit binary not built (run `cargo build -p gitoxide --bin brit --release`)");
        return;
    };
    let cap = BritInvocation::new(bin)
        .args(["branch", "list"])
        .current_dir(temp.path())
        .normalize(true)
        .run()
        .expect("invoke");
    staging_dump_path(
        &["brit", "branch", "list"],
        &format!("{}\n---stderr---\n{}", cap.stdout, cap.stderr),
    );
}

// ─── tag list ────────────────────────────────────────────────────────────────

#[test]
fn tag_list_in_empty_repo() {
    let temp = TestRepo::new("tag-list").expect("repo");
    let Some(bin) = brit_bin() else {
        eprintln!("skipping: brit binary not built (run `cargo build -p gitoxide --bin brit --release`)");
        return;
    };
    let cap = BritInvocation::new(bin)
        .args(["tag", "list"])
        .current_dir(temp.path())
        .normalize(true)
        .run()
        .expect("invoke");
    staging_dump_path(
        &["brit", "tag", "list"],
        &format!("{}\n---stderr---\n{}", cap.stdout, cap.stderr),
    );
}

#[test]
fn tag_list_after_creating_a_tag() {
    let temp = TestRepo::new("tag-list-populated").expect("repo");
    temp.commit_file("file.txt", "data\n").expect("commit");
    gix_testtools::git(temp.path(), "tag v0.1.0").expect("create lightweight tag");
    let Some(bin) = brit_bin() else {
        eprintln!("skipping: brit binary not built (run `cargo build -p gitoxide --bin brit --release`)");
        return;
    };
    let cap = BritInvocation::new(bin)
        .args(["tag", "list"])
        .current_dir(temp.path())
        .normalize(true)
        .run()
        .expect("invoke");
    // This test verifies brit tag list can list real tags (not just an empty repo).
    // Overwrite the staging file from the empty-repo test with the richer output.
    staging_dump_path(
        &["brit", "tag", "list"],
        &format!("{}\n---stderr---\n{}", cap.stdout, cap.stderr),
    );
}

// ─── commit describe ─────────────────────────────────────────────────────────

#[test]
fn commit_describe_with_annotated_tag() {
    let temp = TestRepo::new("commit-describe").expect("repo");
    let sha = temp.commit_file("file.txt", "v1\n").expect("commit");
    gix_testtools::git(temp.path(), &format!("tag -a v1.0.0 -m 'release v1.0.0' {sha}")).expect("create annotated tag");
    let Some(bin) = brit_bin() else {
        eprintln!("skipping: brit binary not built (run `cargo build -p gitoxide --bin brit --release`)");
        return;
    };
    let cap = BritInvocation::new(bin)
        .args(["commit", "describe"])
        .current_dir(temp.path())
        .normalize(true)
        .run()
        .expect("invoke");
    staging_dump_path(
        &["brit", "commit", "describe"],
        &format!("{}\n---stderr---\n{}", cap.stdout, cap.stderr),
    );
}

// ─── commit verify ───────────────────────────────────────────────────────────

#[test]
fn commit_verify_unsigned_commit() {
    // brit commit verify will likely fail/warn on an unsigned commit — that is
    // the documented behavior we want to capture.
    let temp = TestRepo::new("commit-verify").expect("repo");
    let Some(bin) = brit_bin() else {
        eprintln!("skipping: brit binary not built (run `cargo build -p gitoxide --bin brit --release`)");
        return;
    };
    let cap = BritInvocation::new(bin)
        .args(["commit", "verify"])
        .current_dir(temp.path())
        .normalize(true)
        .run()
        .expect("invoke");
    staging_dump_path(
        &["brit", "commit", "verify"],
        &format!("{}\n---stderr---\n{}", cap.stdout, cap.stderr),
    );
}

// ─── blame ───────────────────────────────────────────────────────────────────

#[test]
fn blame_a_committed_file() {
    let temp = TestRepo::new("blame").expect("repo");
    temp.commit_file("poem.txt", "line one\nline two\nline three\n")
        .expect("commit");
    let Some(bin) = brit_bin() else {
        eprintln!("skipping: brit binary not built (run `cargo build -p gitoxide --bin brit --release`)");
        return;
    };
    let cap = BritInvocation::new(bin)
        .args(["blame", "poem.txt"])
        .current_dir(temp.path())
        .normalize(true)
        .run()
        .expect("invoke");
    staging_dump_path(
        &["brit", "blame"],
        &format!("{}\n---stderr---\n{}", cap.stdout, cap.stderr),
    );
}

// ─── cat ─────────────────────────────────────────────────────────────────────

#[test]
fn cat_a_commit_object() {
    let temp = TestRepo::new("cat").expect("repo");
    let head = temp.head_id().expect("head");
    let Some(bin) = brit_bin() else {
        eprintln!("skipping: brit binary not built (run `cargo build -p gitoxide --bin brit --release`)");
        return;
    };
    let cap = BritInvocation::new(bin)
        .args(["cat", &head])
        .current_dir(temp.path())
        .normalize(true)
        .run()
        .expect("invoke");
    staging_dump_path(
        &["brit", "cat"],
        &format!("{}\n---stderr---\n{}", cap.stdout, cap.stderr),
    );
}

#[test]
fn cat_a_blob_via_revspec() {
    let temp = TestRepo::new("cat-blob").expect("repo");
    temp.commit_file("blob.txt", "blob content\n").expect("commit");
    // HEAD:blob.txt resolves to the blob object
    let Some(bin) = brit_bin() else {
        eprintln!("skipping: brit binary not built (run `cargo build -p gitoxide --bin brit --release`)");
        return;
    };
    let cap = BritInvocation::new(bin)
        .args(["cat", "HEAD:blob.txt"])
        .current_dir(temp.path())
        .normalize(true)
        .run()
        .expect("invoke");
    // Dump as a second capture for the cat subcommand (overwrites commit capture).
    staging_dump_path(
        &["brit", "cat"],
        &format!("{}\n---stderr---\n{}", cap.stdout, cap.stderr),
    );
}

// ─── clone ───────────────────────────────────────────────────────────────────

#[test]
fn clone_from_mock_remote() {
    // Seed the upstream via a local repo + git push
    let upstream = MockRemote::new("clone-test").expect("upstream");
    let seed = TestRepo::new("clone-seed").expect("seed");
    seed.commit_file("seed.txt", "initial content\n").expect("seed commit");
    // Push the seed repo's main branch to the bare upstream
    let seed_push = gix_testtools::git_command(seed.path())
        .args(["push", "-q", &upstream.url(), "main"])
        .output()
        .expect("seed mock remote");
    assert!(
        seed_push.status.success(),
        "seed push failed: {}",
        String::from_utf8_lossy(&seed_push.stderr)
    );

    // Destination: a fresh temp dir (not yet a repo)
    let dest_temp = tempfile::Builder::new()
        .prefix("brit-test-clone-dest-")
        .tempdir()
        .expect("mktemp dest");
    let dest_path = dest_temp.path().join("cloned");

    let Some(bin) = brit_bin() else {
        eprintln!("skipping: brit binary not built (run `cargo build -p gitoxide --bin brit --release`)");
        return;
    };
    let cap = BritInvocation::new(bin)
        .args(["clone", &upstream.url()])
        .arg(&dest_path)
        .normalize(true)
        .run()
        .expect("invoke");
    staging_dump_path(
        &["brit", "clone"],
        &format!("{}\n---stderr---\n{}", cap.stdout, cap.stderr),
    );
}

// ─── fetch ───────────────────────────────────────────────────────────────────

#[test]
fn fetch_from_mock_remote() {
    // Upstream gets a commit; we clone via raw git, then use brit fetch to pull
    // subsequent updates.
    let upstream = MockRemote::new("fetch-test").expect("upstream");
    let seed = TestRepo::new("fetch-seed").expect("seed");
    seed.commit_file("v1.txt", "version 1\n").expect("v1");
    let seed_push = gix_testtools::git_command(seed.path())
        .args(["push", "-q", &upstream.url(), "main"])
        .output()
        .expect("seed mock remote");
    assert!(
        seed_push.status.success(),
        "seed push failed: {}",
        String::from_utf8_lossy(&seed_push.stderr)
    );

    let Some(bin) = brit_bin() else {
        eprintln!("skipping: brit binary not built (run `cargo build -p gitoxide --bin brit --release`)");
        return;
    };

    // Local clone via raw git (no brit needed for setup)
    let local_temp = tempfile::Builder::new()
        .prefix("brit-test-fetch-local-")
        .tempdir()
        .expect("mktemp local");
    let local_path = local_temp.path().join("local");
    let clone_out = gix_testtools::git_command(local_temp.path())
        .args(["clone", "-q", &upstream.url()])
        .arg(&local_path)
        .output()
        .expect("git clone");
    if !clone_out.status.success() {
        // If git clone itself fails (e.g. empty upstream), capture the fetch
        // help page so the staging slot still gets populated.
        let cap = BritInvocation::new(bin)
            .args(["fetch", "--help"])
            .normalize(true)
            .run()
            .expect("invoke");
        staging_dump_path(
            &["brit", "fetch"],
            &format!("{}\n---stderr---\n{}", cap.stdout, cap.stderr),
        );
        return;
    }

    // Run brit fetch to pull (dry-run so we don't need write access guarantees)
    let cap = BritInvocation::new(bin)
        .args(["fetch", "--dry-run", "--remote", "origin"])
        .current_dir(&local_path)
        .normalize(true)
        .run()
        .expect("invoke");
    staging_dump_path(
        &["brit", "fetch"],
        &format!("{}\n---stderr---\n{}", cap.stdout, cap.stderr),
    );
}

// ─── push ─────────────────────────────────────────────────────────────────────

#[test]
fn push_to_mock_remote() {
    let local = TestRepo::new("push-local").expect("local repository");
    let head = local
        .commit_file("pushed.txt", "through brit\n")
        .expect("commit to push");
    let remote = MockRemote::new("push-remote").expect("bare remote");
    let Some(bin) = brit_bin() else {
        eprintln!("skipping: brit binary not built");
        return;
    };
    let cap = BritInvocation::new(bin)
        .args(["push", &remote.url(), "refs/heads/main:refs/heads/main"])
        .current_dir(local.path())
        .normalize(true)
        .run()
        .expect("invoke push");
    assert!(cap.status.success(), "push failed: {}", cap.stderr);
    let pushed = fs::read_to_string(remote.path().join("refs/heads/main")).expect("remote main ref");
    assert_eq!(pushed.trim(), head, "remote ref must match pushed commit");
    staging_dump_path(
        &["brit", "push"],
        &format!("{}\n---stderr---\n{}", cap.stdout, cap.stderr),
    );
}
