use std::io::Read;

use bstr::ByteSlice;
use gix_filter::pipeline::{CrlfRoundTripCheck, convert::to_worktree};

use crate::{driver::apply::driver_with_process, pipeline::pipeline};

#[test]
fn all_stages() -> gix_testtools::TestResult {
    let (mut cache, mut pipe) = pipeline("all-filters", || {
        (
            vec![driver_with_process()],
            Vec::new(),
            CrlfRoundTripCheck::Skip,
            Default::default(),
        )
    })?;

    let mut out = pipe.convert_to_worktree(
        b"a\nb\n$Id$",
        "any.txt".into(),
        &mut |path, attrs| {
            cache
                .at_entry(path, None, &gix_object::find::Never)
                .expect("cannot fail")
                .matching_attributes(attrs);
        },
        to_worktree::Options {
            can_delay: gix_filter::driver::apply::Delay::Forbid,
            unknown_encoding: to_worktree::UnknownEncoding::Fail,
        },
    )?;
    assert!(out.is_changed(), "filters were applied");
    assert!(
        out.as_bytes().is_none(),
        "the last filter is a driver which is applied, yielding a stream"
    );
    assert!(out.as_read().is_some(), "process filter is last");
    let mut buf = Vec::new();
    out.read_to_end(&mut buf)?;
    let expected_hash = match gix_testtools::object_hash() {
        gix_hash::Kind::Sha1 => "2188d1cdee2b93a80084b61af431a49d21bc7cc0",
        gix_hash::Kind::Sha256 => "66b8b3bf4f18bcb5f74e09b24ac62e10934e9453a1de9793edb9390dc2ab1d6b",
        _ => unimplemented!(),
    };
    assert_eq!(
        buf.as_bstr(),
        format!("➡a\r\n➡b\r\n➡$Id: {expected_hash}$"),
        "the buffer shows that a lot of transformations were applied"
    );
    Ok(())
}

#[test]
fn all_stages_no_filter() -> gix_testtools::TestResult {
    let (mut cache, mut pipe) = pipeline("all-filters", || {
        (vec![], Vec::new(), CrlfRoundTripCheck::Skip, Default::default())
    })?;

    let mut out = pipe.convert_to_worktree(
        b"$Id$a\nb\n",
        "other.txt".into(),
        &mut |path, attrs| {
            cache
                .at_entry(path, None, &gix_object::find::Never)
                .expect("cannot fail")
                .matching_attributes(attrs);
        },
        to_worktree::Options {
            can_delay: gix_filter::driver::apply::Delay::Forbid,
            unknown_encoding: to_worktree::UnknownEncoding::Fail,
        },
    )?;
    assert!(out.is_changed(), "filters were applied");
    assert!(
        out.as_read().is_none(),
        "there is no filter process, so no chance for getting a stream"
    );
    let buf = out.as_bytes().expect("no filter process");
    let expected_hash = match gix_testtools::object_hash() {
        gix_hash::Kind::Sha1 => "a77d7acbc809ac8df987a769221c83137ba1b9f9",
        gix_hash::Kind::Sha256 => "5ac811252c70ca9761feaa6fe00a74fbf558378ff4fc2853e43b097b153bd7eb",
        _ => unimplemented!(),
    };
    assert_eq!(
        buf.as_bstr(),
        format!("$Id: {expected_hash}$a\r\nb\r\n"),
        "the buffer shows that a lot of transformations were applied"
    );
    Ok(())
}

#[test]
fn no_filter() -> gix_testtools::TestResult {
    let (mut cache, mut pipe) = pipeline("no-filters", || {
        (vec![], Vec::new(), CrlfRoundTripCheck::Skip, Default::default())
    })?;

    let input = b"$Id$a\nb\n";
    let out = pipe.convert_to_worktree(
        input,
        "other.txt".into(),
        &mut |path, attrs| {
            cache
                .at_entry(path, None, &gix_object::find::Never)
                .expect("cannot fail")
                .matching_attributes(attrs);
        },
        to_worktree::Options {
            can_delay: gix_filter::driver::apply::Delay::Forbid,
            unknown_encoding: to_worktree::UnknownEncoding::Fail,
        },
    )?;
    assert!(!out.is_changed(), "no filter was applied");
    let actual = out.as_bytes().expect("input is unchanged");
    assert_eq!(actual, input, "so the input is unchanged…");
    assert_eq!(actual.as_ptr(), input.as_ptr(), "…which means it's exactly the same");
    Ok(())
}

#[test]
fn unknown_encoding_is_ignored_after_other_conversions() -> gix_testtools::TestResult {
    let (mut cache, mut pipe) = pipeline("unknown-encoding", || {
        (vec![], Vec::new(), CrlfRoundTripCheck::Skip, Default::default())
    })?;
    let out = pipe.convert_to_worktree(
        b"a\nb\n",
        "file".into(),
        &mut |path, attrs| {
            cache
                .at_entry(path, None, &gix_object::find::Never)
                .expect("cannot fail")
                .matching_attributes(attrs);
        },
        to_worktree::Options {
            can_delay: gix_filter::driver::apply::Delay::Forbid,
            ..Default::default()
        },
    )?;
    assert_eq!(
        out.as_bytes().expect("converted in memory").as_bstr(),
        "a\r\nb\r\n",
        "an unavailable encoding leaves the result of earlier conversions intact"
    );
    Ok(())
}

#[test]
fn encoding_failure_is_ignored_after_other_conversions() -> gix_testtools::TestResult {
    let (mut cache, mut pipe) = pipeline("all-filters", || {
        (vec![], Vec::new(), CrlfRoundTripCheck::Skip, Default::default())
    })?;
    let out = pipe.convert_to_worktree(
        b"a\n\xF0\x9F\x98\x80\n",
        "file".into(),
        &mut |path, attrs| {
            cache
                .at_entry(path, None, &gix_object::find::Never)
                .expect("cannot fail")
                .matching_attributes(attrs);
        },
        to_worktree::Options {
            can_delay: gix_filter::driver::apply::Delay::Forbid,
            unknown_encoding: to_worktree::UnknownEncoding::Ignore,
        },
    )?;
    assert_eq!(
        out.as_bytes().expect("converted in memory").as_bstr(),
        "a\r\n😀\r\n",
        "an unrepresentable character leaves earlier conversions intact"
    );
    Ok(())
}

#[test]
fn encoding_failure_can_be_an_error() -> gix_testtools::TestResult {
    let (mut cache, mut pipe) = pipeline("all-filters", || {
        (vec![], Vec::new(), CrlfRoundTripCheck::Skip, Default::default())
    })?;
    let err = pipe
        .convert_to_worktree(
            "😀".as_bytes(),
            "file".into(),
            &mut |path, attrs| {
                cache
                    .at_entry(path, None, &gix_object::find::Never)
                    .expect("cannot fail")
                    .matching_attributes(attrs);
            },
            to_worktree::Options {
                can_delay: gix_filter::driver::apply::Delay::Forbid,
                unknown_encoding: to_worktree::UnknownEncoding::Fail,
            },
        )
        .err()
        .expect("unrepresentable characters can be rejected explicitly");
    insta::assert_debug_snapshot!(err, "encoding failure can be an error", @"The character '😀' could not be mapped to the windows-1252");
    Ok(())
}

#[test]
fn unknown_encoding_can_be_an_error() -> gix_testtools::TestResult {
    let (mut cache, mut pipe) = pipeline("unknown-encoding", || {
        (vec![], Vec::new(), CrlfRoundTripCheck::Skip, Default::default())
    })?;

    let err = pipe
        .convert_to_worktree(
            b"content",
            "file".into(),
            &mut |path, attrs| {
                cache
                    .at_entry(path, None, &gix_object::find::Never)
                    .expect("cannot fail")
                    .matching_attributes(attrs);
            },
            to_worktree::Options {
                can_delay: gix_filter::driver::apply::Delay::Forbid,
                unknown_encoding: to_worktree::UnknownEncoding::Fail,
            },
        )
        .err()
        .expect("unknown encodings can be rejected explicitly");
    insta::assert_debug_snapshot!(err, "unknown encoding can be an error", @"The encoding named 'not-an-encoding' isn't available");
    assert!(err.is_validation(), "unknown encodings are validation failures");
    Ok(())
}

#[test]
fn bounded_stream_matches_buffered_conversions_across_chunks() -> gix_testtools::TestResult {
    use gix_filter::pipeline::stream::Outcome;
    use gix_utils::stream::Budget;
    use std::io::{Seek, Write};
    let temp = gix_testtools::tempfile::tempdir()?;
    let budget = Budget::new(temp.path())?;
    for path in ["any.txt", "other.txt"] {
        for offset in [8190, 8191, 65534, 65535, 65536] {
            let mut input = vec![b'x'; offset];
            input.extend_from_slice("$Id$\r\n$Id$\nmultibyte: €\n".as_bytes());
            let (mut cache, mut pipe) = pipeline("all-filters", || {
                (vec![], Vec::new(), CrlfRoundTripCheck::Skip, Default::default())
            })?;
            let mut attributes = |path: &bstr::BStr, attrs: &mut gix_attributes::search::Outcome| {
                cache
                    .at_entry(path, None, &gix_object::find::Never)
                    .expect("fixture attributes")
                    .matching_attributes(attrs);
            };
            let options = || to_worktree::Options {
                can_delay: gix_filter::driver::apply::Delay::Forbid,
                unknown_encoding: to_worktree::UnknownEncoding::Fail,
            };
            let mut expected = Vec::new();
            pipe.convert_to_worktree(&input, path.into(), &mut attributes, options())?
                .read_to_end(&mut expected)?;
            let mut source = budget.spool()?;
            source.write_all(&input)?;
            source.rewind()?;
            let Outcome::Ready(mut out) = pipe.convert_to_worktree_stream(
                source,
                path.into(),
                &mut attributes,
                options(),
                &budget,
                &Default::default(),
            )?
            else {
                panic!("no driver is configured")
            };
            let mut actual = Vec::new();
            out.read_to_end(&mut actual)?;
            assert_eq!(actual, expected, "{path}, split at {offset}");
            drop(out);
            assert_eq!(budget.usage().0, 0);
        }
    }
    Ok(())
}

#[test]
fn bounded_process_filters_preserve_immediate_and_delayed_protocols() -> gix_testtools::TestResult {
    use gix_filter::{driver::apply::Delay, pipeline::stream::Outcome};
    use gix_utils::stream::{self, Budget};
    use std::io::{Seek, Write};
    let temp = gix_testtools::tempfile::tempdir()?;
    let budget = Budget::new(temp.path())?;
    for can_delay in [Delay::Allow, Delay::Forbid] {
        let (mut cache, mut pipe) = pipeline("driver-only", || {
            (
                vec![driver_with_process()],
                Vec::new(),
                CrlfRoundTripCheck::Skip,
                Default::default(),
            )
        })?;
        let mut input = budget.spool()?;
        input.write_all(b"a\nb\n")?;
        input.rewind()?;
        let out = pipe.convert_to_worktree_stream(
            input,
            "file".into(),
            &mut |path, attrs| {
                cache
                    .at_entry(path, None, &gix_object::find::Never)
                    .expect("fixture attributes")
                    .matching_attributes(attrs);
            },
            to_worktree::Options {
                can_delay,
                unknown_encoding: to_worktree::UnknownEncoding::Fail,
            },
            &budget,
            &Default::default(),
        )?;
        let mut out = match out {
            Outcome::Ready(out) => {
                assert!(matches!(can_delay, Delay::Forbid));
                out
            }
            Outcome::Delayed(key) => {
                assert!(matches!(can_delay, Delay::Allow));
                assert_eq!(
                    budget.usage().0,
                    0,
                    "the driver owns delayed input, no Brit spool is retained"
                );
                let paths = pipe.driver_state_mut().list_delayed_paths(&key)?;
                assert_eq!(paths, vec![bstr::BString::from("file")]);
                let mut reader = pipe.driver_state_mut().fetch_delayed(
                    &key,
                    "file".into(),
                    gix_filter::driver::Operation::Smudge,
                )?;
                let mut out = budget.spool()?;
                stream::copy(&mut reader, &mut out, &Default::default())?;
                out.rewind()?;
                out
            }
        };
        let mut content = String::new();
        out.read_to_string(&mut content)?;
        assert_eq!(content, "➡a\n➡b\n");
        drop(out);
        assert_eq!(budget.usage().0, 0);
        pipe.driver_state_mut()
            .shutdown(gix_filter::driver::shutdown::Mode::WaitForProcesses)?
            .into_result()?;
    }
    Ok(())
}
