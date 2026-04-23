/// Smoke test for the `gix::remote::connection::push` module.
///
/// # Note on Task 7.2 dependency
///
/// `Connection::prepare_push` is not yet implemented (Task 7.2), so there is
/// currently no public way to obtain a `Prepare` from a live connection.
/// Once Task 7.2 lands, update this test to call `remote.connect(...)?.prepare_push()`
/// and remove the `#[ignore]` attribute.
///
/// The test is marked `#[ignore]` to keep CI green while the dependency is absent.
/// Run it manually with:
///
///   `cargo test -p gix --test gix -- remote::push --include-ignored`
#[ignore = "unblocked by Task 7.2 (Connection::prepare_push)"]
#[test]
fn fast_forward_to_bare_file_remote_smoke() {
    // Placeholder: when Task 7.2 is complete this should:
    //   1. Create a source repo + bare destination using a scripted fixture.
    //   2. Open the source, call `prepare_push` with a fast-forward refspec.
    //   3. Call `transmit`.
    //   4. Open the bare remote and assert the pushed commit is reachable from HEAD.
}

/// Verify the `Outcome` and `RefStatus` public types are accessible and have
/// the expected fields without needing a live connection.
#[test]
fn push_outcome_types_are_accessible() {
    use gix::bstr::BString;
    use gix::remote::push::{Outcome, RefStatus};

    let status = RefStatus {
        local: BString::from("refs/heads/main"),
        remote: BString::from("refs/heads/main"),
        result: Ok(()),
    };
    let outcome = Outcome {
        status: vec![status],
    };
    assert_eq!(outcome.status.len(), 1);
    assert!(outcome.status[0].result.is_ok());
}
