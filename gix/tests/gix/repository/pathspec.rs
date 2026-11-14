use gix::{bstr::ByteSlice, config::tree::gitoxide};
use gix_testtools::TestResult;
use gix_worktree::stack::state::attributes::Source;

use crate::util::named_repo;

#[test]
fn defaults_are_taken_from_repo_config() -> TestResult {
    let mut repo = named_repo("make_basic_repo.sh")?;
    repo.config_snapshot_mut()
        .set_value(&gitoxide::Pathspec::ICASE, "true")?;
    let inherit_ignore_case = true;
    let empty_pathspecs_match_prefix = true;
    let mut pathspec = repo.pathspec(
        empty_pathspecs_match_prefix,
        [
            "hi",
            ":!hip",
            gix::path::to_unix_separators_on_windows(gix::path::into_bstr(
                repo.workdir().expect("present").join("for-normalization"),
            )?)
            .to_str_lossy()
            .as_ref(),
        ],
        inherit_ignore_case,
        &**repo.index()?,
        Source::WorktreeThenIdMapping.adjust_for_bare(repo.is_bare()),
    )?;
    assert!(pathspec.is_included("hi", Some(false)));
    assert!(!pathspec.is_included("ho", Some(false)));
    assert!(!pathspec.is_included("hip", Some(false)));
    assert!(
        pathspec
            .pattern_matching_relative_path("hip", Some(false))
            .expect("match")
            .is_excluded()
    );

    assert!(
        pathspec.is_included("HI", Some(false)),
        "icase is enabled, so filesystem doesn't matter"
    );
    Ok(())
}

#[test]
fn attribute_selection_reads_indexed_patterns_above_allocation_policy() -> TestResult {
    let dir = gix_testtools::tempfile::tempdir()?;
    assert!(
        gix_testtools::git_command(dir.path())
            .args(["init", "-q"])
            .status()?
            .success()
    );
    let mut patterns = vec![b'#'; 128 * 1024];
    patterns.extend_from_slice(b"\nselected custom=yes\n");
    std::fs::write(dir.path().join(".gitattributes"), patterns)?;
    std::fs::write(dir.path().join("selected"), "unchanged\n")?;
    assert!(
        gix_testtools::git_command(dir.path())
            .args(["add", ".gitattributes", "selected"])
            .status()?
            .success()
    );
    std::fs::remove_file(dir.path().join(".gitattributes"))?;
    let repo = gix::open_opts(
        dir.path(),
        gix::open::Options::isolated().cli_overrides(["gitoxide.objects.allocLimit=1024"]),
    )?;
    let index = repo.index()?;
    let mut pathspec = repo.pathspec(true, [":(attr:custom=yes)selected"], true, &index, Source::IdMapping)?;
    assert!(
        pathspec.is_included("selected", Some(false)),
        "allocation policy must not change attribute matching"
    );
    let attributes_id = index
        .entry_by_path(b".gitattributes".as_bstr())
        .expect("indexed patterns")
        .id;
    let hex = attributes_id.to_string();
    std::fs::remove_file(repo.git_dir().join("objects").join(&hex[..2]).join(&hex[2..]))?;
    let repo = gix::open_opts(
        dir.path(),
        gix::open::Options::isolated().cli_overrides(["gitoxide.objects.allocLimit=1024"]),
    )?;
    let mut pathspec = repo.pathspec(true, [":(attr:custom=yes)selected"], true, &index, Source::IdMapping)?;
    let error = pathspec
        .try_is_included("selected", Some(false))
        .expect_err("missing patterns are not a non-match");
    assert!(
        format!("{error:?}").contains(&hex),
        "the failed object remains identifiable"
    );
    let mut pathspec = repo.pathspec(true, [":(attr:custom=yes)selected"], true, &index, Source::IdMapping)?;
    let mut entries = pathspec.try_index_entries_with_paths(&index).expect("selected prefix");
    assert!(entries.next().expect("failed lookup").is_err());
    assert!(
        entries.next().is_none(),
        "do not continue after a failed attribute read"
    );
    Ok(())
}
