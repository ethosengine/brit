use gix::{bstr::BString, error::ResultExt};

#[test]
fn previous_branches_preserves_order_json_and_output_failures() -> gix::Result<()> {
    let fixture = gix_testtools::tempfile::tempdir().or_error()?;
    assert!(
        gix_testtools::git_command(fixture.path())
            .args(["init", "-q", "--initial-branch=main"])
            .status()
            .or_error()?
            .success()
    );
    let repo = gix::open_opts(fixture.path(), gix::open::Options::isolated())?;
    let mut first = gix::ObjectId::null(repo.object_hash());
    first.as_mut_slice()[0] = 1;
    let mut second = first;
    second.as_mut_slice()[0] = 2;
    std::fs::write(repo.git_dir().join("refs/heads/main"), format!("{second}\n")).or_error()?;
    std::fs::create_dir_all(repo.git_dir().join("logs")).or_error()?;
    let log = format!(
        "{first} {second} Fixture <fixture@example.com> 1600000000 +0000\tcheckout: moving from earlier to main\n{second} {first} Fixture <fixture@example.com> 1600000001 +0000\tcommit: ignored\n{second} {first} Fixture <fixture@example.com> 1600000002 +0000\tcheckout: moving from later to main"
    );
    std::fs::write(repo.git_dir().join("logs/HEAD"), log).or_error()?;
    let expected = vec![(BString::from("earlier"), first), (BString::from("later"), second)];
    assert_eq!(repo.head()?.prior_checked_out_branches()?.expect("reflog"), expected);
    let mut actual = Vec::new();
    gitoxide_core::repository::revision::previous_branches(
        repo.clone(),
        &mut actual,
        gitoxide_core::OutputFormat::Human,
    )?;
    assert_eq!(actual, format!("{first} earlier\n{second} later\n").as_bytes());
    #[cfg(feature = "serde")]
    {
        actual.clear();
        gitoxide_core::repository::revision::previous_branches(
            repo.clone(),
            &mut actual,
            gitoxide_core::OutputFormat::Json,
        )?;
        assert_eq!(
            actual,
            serde_json::to_vec_pretty(&expected).or_error()?,
            "streamed JSON retains the existing wire format"
        );
    }
    struct Broken;
    impl std::io::Write for Broken {
        fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
            Err(std::io::ErrorKind::BrokenPipe.into())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let error =
        gitoxide_core::repository::revision::previous_branches(repo, Broken, gitoxide_core::OutputFormat::Human)
            .expect_err("output failure");
    assert_eq!(
        error
            .downcast_any_ref::<std::io::Error>()
            .expect("original IO error")
            .kind(),
        std::io::ErrorKind::BrokenPipe
    );
    Ok(())
}
