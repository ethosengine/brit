use std::{io::Read, path::Path};

use bstr::ByteSlice;
use gix_filter::{eol, pipeline::CrlfRoundTripCheck};

use crate::{driver::apply::driver_with_process, pipeline::pipeline};

#[test]
fn no_driver_but_filter_with_autocrlf() -> gix_testtools::TestResult {
    let (_cache, mut pipe) = pipeline("no-filter", || {
        (
            vec![],
            Vec::new(),
            CrlfRoundTripCheck::Fail,
            eol::Configuration {
                auto_crlf: eol::AutoCrlf::Enabled,
                eol: None,
            },
        )
    })?;

    let mut out = pipe.convert_to_git(
        "hi\r\n".as_bytes(),
        Path::new("any.txt"),
        &mut |_path, _attrs| {},
        &mut no_object_in_index,
    )?;

    assert_eq!(
        out.as_bytes().expect("read converted to buffer").as_bstr(),
        "hi\n",
        "the read is read into memory if there is no driver"
    );
    let mut buf = Vec::new();
    out.read_to_end(&mut buf)?;
    assert_eq!(buf.as_bstr(), "hi\n", "we can consume the output");
    Ok(())
}

#[test]
fn all_stages_mean_streaming_is_impossible() -> gix_testtools::TestResult {
    let (mut cache, mut pipe) = pipeline("all-filters", || {
        (
            vec![driver_with_process()],
            Vec::new(),
            CrlfRoundTripCheck::Fail,
            Default::default(),
        )
    })?;

    let source_hash = match gix_testtools::object_hash() {
        gix_hash::Kind::Sha1 => "2188d1cdee2b93a80084b61af431a49d21bc7cc0",
        gix_hash::Kind::Sha256 => "66b8b3bf4f18bcb5f74e09b24ac62e10934e9453a1de9793edb9390dc2ab1d6b",
        _ => unimplemented!(),
    };
    let src = format!("➡a\r\n➡b\r\n➡$Id: {source_hash}$");
    let mut out = pipe.convert_to_git(
        src.as_bytes(),
        Path::new("any.txt"),
        &mut |path, attrs| {
            cache
                .at_entry(path, None, &gix_object::find::Never)
                .expect("cannot fail")
                .matching_attributes(attrs);
        },
        &mut no_object_in_index,
    )?;
    assert!(out.is_changed(), "filters were applied");
    assert!(out.as_read().is_none(), "non-driver filters operate in-memory");
    let buf = out.as_bytes().expect("in-memory operation");
    assert_eq!(buf.as_bstr(), "a\nb\n$Id$", "filters were successfully reversed");
    Ok(())
}

#[test]
fn only_driver_means_streaming_is_possible() -> gix_testtools::TestResult {
    let (mut cache, mut pipe) = pipeline("driver-only", || {
        (
            vec![driver_with_process()],
            Vec::new(),
            CrlfRoundTripCheck::Skip,
            Default::default(),
        )
    })?;

    let source_hash = match gix_testtools::object_hash() {
        gix_hash::Kind::Sha1 => "2188d1cdee2b93a80084b61af431a49d21bc7cc0",
        gix_hash::Kind::Sha256 => "66b8b3bf4f18bcb5f74e09b24ac62e10934e9453a1de9793edb9390dc2ab1d6b",
        _ => unimplemented!(),
    };
    let src = format!("➡a\r\n➡b\r\n➡$Id: {source_hash}$");
    let mut out = pipe.convert_to_git(
        src.as_bytes(),
        Path::new("subdir/doesnot/matter/any.txt"),
        &mut |path, attrs| {
            cache
                .at_entry(path, None, &gix_object::find::Never)
                .expect("cannot fail")
                .matching_attributes(attrs);
        },
        &mut no_object_in_index,
    )?;
    assert!(out.is_changed(), "filters were applied");
    assert!(out.as_read().is_some(), "filter-only can be streamed");
    let mut buf = Vec::new();
    out.read_to_end(&mut buf)?;
    assert_eq!(
        buf.as_bstr(),
        format!("a\r\nb\r\n$Id: {source_hash}$"),
        "one filter was reversed"
    );
    Ok(())
}

#[test]
fn no_filter_means_reader_is_returned_unchanged() -> gix_testtools::TestResult {
    let (mut cache, mut pipe) = pipeline("no-filters", || {
        (vec![], Vec::new(), CrlfRoundTripCheck::Fail, Default::default())
    })?;

    let source_hash = match gix_testtools::object_hash() {
        gix_hash::Kind::Sha1 => "2188d1cdee2b93a80084b61af431a49d21bc7cc0",
        gix_hash::Kind::Sha256 => "66b8b3bf4f18bcb5f74e09b24ac62e10934e9453a1de9793edb9390dc2ab1d6b",
        _ => unimplemented!(),
    };
    let input = format!("➡a\r\n➡b\r\n➡$Id: {source_hash}$");
    let mut out = pipe.convert_to_git(
        input.as_bytes(),
        Path::new("other.txt"),
        &mut |path, attrs| {
            cache
                .at_entry(path, None, &gix_object::find::Never)
                .expect("cannot fail")
                .matching_attributes(attrs);
        },
        &mut no_call,
    )?;
    assert!(!out.is_changed(), "no filter was applied");
    let actual = out
        .as_read()
        .expect("input is unchanged, we get the original stream back");
    let mut buf = Vec::new();
    actual.read_to_end(&mut buf)?;
    assert_eq!(buf.as_bstr(), input, "input is unchanged");
    Ok(())
}

fn no_call(_buf: &mut Vec<u8>) -> gix_error::Result<Option<()>> {
    unreachable!("index function will not be called")
}

fn no_object_in_index(_buf: &mut Vec<u8>) -> gix_error::Result<Option<()>> {
    Ok(None)
}

#[test]
fn bounded_clean_matches_buffered_conversion_and_index_decisions() -> gix_testtools::TestResult {
    use gix_utils::stream::Budget;
    use std::io::{Seek, Write};
    let directory = gix_testtools::tempfile::tempdir()?;
    let budget = Budget::new(directory.path())?;
    for fixture in ["no-filters", "all-filters", "driver-only"] {
        for check in [CrlfRoundTripCheck::Skip, CrlfRoundTripCheck::Fail] {
            for indexed in [None, Some(&b"already\r\n"[..]), Some(&b"binary\0\r\n"[..])] {
                for tail in [
                    &b"\r\n$Id: expanded$\r\r\n$Id: no\nend$\r"[..],
                    b"\0\n\x1a",
                    b"\xe9\r\n",
                ] {
                    let (mut cache, mut pipe) = pipeline(fixture, || {
                        (
                            vec![driver_with_process()],
                            vec![encoding_rs::WINDOWS_1252],
                            check,
                            eol::Configuration {
                                auto_crlf: eol::AutoCrlf::Enabled,
                                eol: None,
                            },
                        )
                    })?;
                    let mut attributes = |path: &bstr::BStr, attrs: &mut gix_attributes::search::Outcome| {
                        cache
                            .at_entry(path, None, &gix_object::find::Never)
                            .expect("fixture attributes")
                            .matching_attributes(attrs);
                    };
                    let mut input = vec![b'x'; 65535];
                    input.extend_from_slice(tail);
                    let expected = (|| -> gix_error::Result<Vec<u8>> {
                        use gix_error::ResultExt;
                        let mut out =
                            pipe.convert_to_git(&input[..], Path::new("any.txt"), &mut attributes, &mut |out| {
                                Ok(indexed.map(|bytes| {
                                    out.clear();
                                    out.extend_from_slice(bytes);
                                }))
                            })?;
                        let mut bytes = Vec::new();
                        out.read_to_end(&mut bytes).or_error()?;
                        Ok(bytes)
                    })();
                    let mut source = budget.spool()?;
                    source.write_all(&input)?;
                    source.rewind()?;
                    let actual = pipe.convert_to_git_stream(
                        source,
                        "any.txt".into(),
                        &mut attributes,
                        &mut || {
                            use gix_error::ResultExt;
                            indexed
                                .map(|bytes| {
                                    let mut file = budget.spool().or_error()?;
                                    file.write_all(bytes).or_error()?;
                                    file.rewind().or_error()?;
                                    Ok(file)
                                })
                                .transpose()
                        },
                        &budget,
                        &Default::default(),
                    );
                    assert_eq!(
                        actual.is_ok(),
                        expected.is_ok(),
                        "{fixture} {check:?} {indexed:?} {tail:?}"
                    );
                    if let (Ok(mut actual), Ok(expected)) = (actual, expected) {
                        let mut bytes = Vec::new();
                        actual.read_to_end(&mut bytes)?;
                        assert_eq!(bytes, expected, "{fixture} {check:?} {indexed:?} {tail:?}");
                    }
                    assert_eq!(budget.usage().0, 0, "completed clean releases every spool");
                }
            }
        }
    }
    Ok(())
}
