use gix::{prelude::ObjectIdExt, revision::Spec};
use gix_testtools::TestResult;

use crate::{
    revision::spec::from_bytes::{parse_spec, parse_spec_no_baseline, repo},
    util::hex_to_id_sha1_only,
};

#[test]
fn reflog_failures_are_not_absence_or_older_entries() -> TestResult {
    use gix::objs::{Kind, Write};
    let cases: &[bool] = if cfg!(unix) { &[true, false] } else { &[true] };
    for &corrupt in cases {
        let fixture = gix_testtools::tempfile::tempdir()?;
        assert!(
            gix_testtools::git_command(fixture.path())
                .args(["init", "-q", "--initial-branch=main"])
                .status()?
                .success()
        );
        let repo = gix::open_opts(fixture.path(), gix::open::Options::isolated())?;
        let tree = repo.objects.write_buf(Kind::Tree, b"")?;
        let commit = format!(
            "tree {tree}\nauthor Fixture <fixture@example.com> 1600000000 +0000\ncommitter Fixture <fixture@example.com> 1600000000 +0000\n\nfixture\n"
        );
        let id = repo.objects.write_buf(Kind::Commit, commit.as_bytes())?;
        std::fs::write(repo.git_dir().join("refs/heads/main"), format!("{id}\n"))?;
        let line = format!(
            "{id} {id} Fixture <fixture@example.com> 1600000000 +0000\tcheckout: moving from main to main\nmalformed reflog\n"
        );
        std::fs::create_dir_all(repo.git_dir().join("logs/refs/heads"))?;
        for name in ["HEAD", "refs/heads/main"] {
            let path = repo.git_dir().join("logs").join(name);
            if corrupt {
                std::fs::write(path, &line)?;
            } else if name != "HEAD" {
                std::fs::write(path, line.strip_suffix("malformed reflog\n").expect("fixture suffix"))?;
            } else {
                #[cfg(unix)]
                std::os::unix::fs::symlink(path.file_name().expect("reflog filename"), &path)?;
            }
        }
        for spec in ["HEAD@{0}", "HEAD@{2010-01-01 00:00:00 +0000}", "@{-1}"] {
            let error = repo
                .rev_parse(spec)
                .expect_err("failed reads must not become an older result");
            if corrupt {
                assert!(error.is_corrupted(), "{spec}: {error:?}");
            } else {
                assert!(
                    error
                        .iter_errors()
                        .any(|cause| cause.downcast_ref::<std::io::Error>().is_some()),
                    "{spec}: {error:?}"
                );
            }
        }
        let error = repo
            .head()?
            .prior_checked_out_branches()
            .expect_err("listing retains failures");
        if corrupt {
            assert!(error.is_corrupted());
        } else {
            assert!(
                error
                    .iter_errors()
                    .any(|cause| cause.downcast_ref::<std::io::Error>().is_some())
            );
        }
    }
    Ok(())
}

#[test]
fn prior_checkout_uses_current_reference_without_loading_its_object() -> TestResult {
    let fixture = gix_testtools::tempfile::tempdir()?;
    for args in [
        vec!["init", "-q", "--initial-branch=main"],
        vec!["commit", "--allow-empty", "-qm", "initial"],
    ] {
        assert!(
            gix_testtools::git_command(fixture.path())
                .args(args)
                .status()?
                .success()
        );
    }
    let repo = gix::open_opts(fixture.path(), gix::open::Options::isolated())?;
    let old = repo.head_id()?.detach();
    let mut current = gix::ObjectId::null(repo.object_hash());
    current.as_mut_slice()[0] = 1;
    std::fs::write(repo.git_dir().join("refs/heads/main"), format!("{current}\n"))?;
    std::fs::write(
        repo.git_dir().join("logs/HEAD"),
        format!("{old} {old} Fixture <fixture@example.com> 1600000000 +0000\tcheckout: moving from main to main\n"),
    )?;
    let oracle = gix_testtools::git_command(fixture.path())
        .args(["rev-parse", "--verify", "@{-1}"])
        .output()?;
    assert!(oracle.status.success(), "{}", String::from_utf8_lossy(&oracle.stderr));
    assert_eq!(oracle.stdout, format!("{current}\n").as_bytes());
    assert_eq!(
        repo.rev_parse_single("@{-1}")?.detach(),
        current,
        "the current reference wins over the old reflog ID"
    );
    Ok(())
}

#[test]
fn reflog_queries_observe_cancellation() -> TestResult {
    if gix_testtools::run_in_isolated_process()? {
        return Ok(());
    }
    let fixture = gix_testtools::tempfile::tempdir()?;
    for args in [
        vec!["init", "-q", "--initial-branch=main"],
        vec!["commit", "--allow-empty", "-qm", "initial"],
    ] {
        assert!(
            gix_testtools::git_command(fixture.path())
                .args(args)
                .status()?
                .success()
        );
    }
    let repo = gix::open_opts(fixture.path(), gix::open::Options::isolated())?;
    let id = repo.head_id()?.detach();
    std::fs::write(
        repo.git_dir().join("logs/HEAD"),
        format!("{id} {id} Fixture <fixture@example.com> 1600000000 +0000\tcheckout: moving from main to main\n"),
    )?;
    for spec in ["HEAD@{0}", "HEAD@{2010-01-01 00:00:00 +0000}", "@{-1}"] {
        gix::interrupt::IS_INTERRUPTED.store(true, std::sync::atomic::Ordering::SeqCst);
        let start = std::time::Instant::now();
        let result = repo.rev_parse(spec);
        let elapsed = start.elapsed();
        gix::interrupt::IS_INTERRUPTED.store(false, std::sync::atomic::Ordering::SeqCst);
        assert!(
            result.expect_err("cancelled query must refuse").is_cancelled(),
            "{spec}"
        );
        assert!(elapsed < std::time::Duration::from_secs(1), "{spec}: {elapsed:?}");
    }
    let head = repo.head()?;
    gix::interrupt::IS_INTERRUPTED.store(true, std::sync::atomic::Ordering::SeqCst);
    let start = std::time::Instant::now();
    let result = head.prior_checked_out_branches();
    let elapsed = start.elapsed();
    gix::interrupt::IS_INTERRUPTED.store(false, std::sync::atomic::Ordering::SeqCst);
    assert!(result.expect_err("cancelled listing").is_cancelled());
    assert!(elapsed < std::time::Duration::from_secs(1));
    Ok(())
}

#[test]
fn symbolic_references_use_their_own_log_or_the_final_targets() -> gix_testtools::TestResult {
    let fixture = gix_testtools::scripted_fixture_read_only("make_symbolic_ref_reflogs.sh")?;
    let repo = gix::open_opts(fixture, crate::restricted())?;
    for name in ["refs/symref", "refs/symref-chain"] {
        assert!(
            !repo.find_reference(name)?.log_exists()?,
            "{name} must exercise fallback to the final target's reflog"
        );
        for query in ["0", "1", "2", "1979-02-26 00:00:00 +0000"] {
            let revspec = format!("{name}@{{{query}}}");
            assert_eq!(
                parse_spec(&revspec, &repo)?.single(),
                parse_spec(format!("main@{{{query}}}"), &repo)?.single(),
                "{revspec} uses the final target's reflog"
            );
        }
    }
    assert!(
        repo.find_reference("refs/heads/symref")?.log_exists()?,
        "a symbolic branch has its own reflog"
    );
    assert!(
        parse_spec("refs/heads/symref@{1}", &repo).is_err(),
        "an existing reflog takes precedence over the target's longer log"
    );
    Ok(())
}

#[test]
fn nth_prior_checkout() {
    let repo = repo("complex_graph").unwrap();

    for (spec, prior_branch) in [
        ("@{-1}", "refs/heads/i"),
        ("@{-2}", "refs/heads/main"),
        ("@{-3}", "refs/heads/e"),
        ("@{-4}", "refs/heads/j"),
        ("@{-5}", "refs/heads/h"),
    ] {
        let parsed = parse_spec(spec, &repo).unwrap_or_else(|_| panic!("{spec} to be parsed successfully"));
        assert_eq!(parsed.first_reference().expect("present"), prior_branch);
        assert_eq!(parsed.second_reference(), None);
    }

    insta::assert_debug_snapshot!(parse_spec("@{-6}", &repo).expect_err("nth prior checkout").probable_cause(), "nth prior checkout", @r#"
    Message {
        message: "HEAD has 5 prior checkouts and checkout number 6 is out of range",
    }
    "#);
}

#[test]
fn nth_prior_checkout_to_deleted_branch_fails_like_git() -> TestResult {
    let repo = repo("deleted_prior_checkout")?;
    let err = parse_spec("@{-1}", &repo).expect_err("deleted prior checkout branch must not resolve by object id");
    insta::assert_debug_snapshot!(err.probable_cause(), "error should explain that the reflog name no longer resolves", @r#"
    Message {
        message: "Previous checkout 'prev-target' does not resolve to an existing revision",
    }
    "#);
    Ok(())
}

#[test]
fn nth_prior_checkout_to_deleted_branch_named_like_object_matches_git() -> TestResult {
    let repo = repo("deleted_prior_checkout_named_like_object")?;
    assert_eq!(
        parse_spec("@{-1}", &repo)?,
        Spec::from_id(hex_to_id_sha1_only("0123456789012345678901234567890123456789").attach(&repo)),
        "full object ids are accepted as previous checkout names, even without matching objects"
    );
    Ok(())
}

#[test]
fn by_index_unborn_head() {
    let repo = &repo("new").unwrap();

    insta::assert_debug_snapshot!(parse_spec("@{1}", repo).expect_err("by index unborn head").probable_cause(), "by index unborn head", @r#"
    Message {
        message: "Unborn heads do not have a reflog yet",
    }
    "#);
}

#[test]
fn by_index() {
    let repo = &repo("complex_graph").unwrap();
    {
        let spec = parse_spec("@{0}", repo).unwrap();
        assert_eq!(
            spec,
            Spec::from_id(hex_to_id_sha1_only("55e825ebe8fd2ff78cad3826afb696b96b576a7e").attach(repo))
        );
        assert_eq!(
            spec.first_reference().expect("set"),
            "refs/heads/main",
            "it sets the reference name even if it is implied"
        );
        assert_eq!(spec.second_reference(), None);
    }

    {
        let spec = parse_spec("HEAD@{5}", repo).unwrap();
        assert_eq!(
            spec,
            Spec::from_id(hex_to_id_sha1_only("5b3f9e24965d0b28780b7ce5daf2b5b7f7e0459f").attach(repo))
        );
        assert_eq!(
            spec.first_reference().map(|r| r.name.to_string()),
            Some("HEAD".into()),
            "explicit references are picked up as usual"
        );
        assert_eq!(spec.second_reference(), None);
    }

    insta::assert_debug_snapshot!(parse_spec("main@{12345}", repo)
            .expect_err("by index")
            .probable_cause(), "by index", @r#"
    Message {
        message: "Reference 'refs/heads/main' has 4 ref-log entries and entry number 12345 is out of range",
    }
    "#);
}

#[test]
fn by_date() {
    let repo = repo("complex_graph").unwrap();

    let spec = parse_spec_no_baseline("main@{42 +0030}", &repo).unwrap();

    assert_eq!(
        spec,
        Spec::from_id(hex_to_id_sha1_only("9f9eac6bd1cd4b4cc6a494f044b28c985a22972b").attach(&repo))
    );
}
