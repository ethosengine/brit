//! Process-isolated bounded loop probes, distinct from unbounded object/I/O work.
use std::{
    cell::RefCell,
    time::{Duration, Instant},
};

use super::*;

#[derive(Default)]
struct Probe {
    point: &'static str,
    completed: usize,
    interrupted: Option<Instant>,
}

thread_local! {
    static PROBE: RefCell<Probe> = RefCell::new(Probe::default());
}

pub(super) fn completed(point: &str) {
    PROBE.with_borrow_mut(|probe| {
        if probe.point == point {
            probe.completed += 1;
            if probe.completed == 16 {
                probe.interrupted = Some(Instant::now());
                gix::interrupt::trigger();
            }
        }
    });
}

fn measure<T>(point: &'static str, size: usize, action: impl FnOnce() -> Result<T>) -> Result<()> {
    PROBE.with_borrow_mut(|probe| {
        *probe = Probe {
            point,
            ..Default::default()
        }
    });
    let result = action();
    let returned = Instant::now();
    gix::interrupt::reset();
    let probe = PROBE.with_borrow_mut(std::mem::take);
    let elapsed = returned.duration_since(probe.interrupted.context("operation finished before interruption")?);
    let error = result.err().context("interrupted loop must refuse")?;
    assert_eq!(error.to_string(), "replay selection interrupted");
    assert_eq!(probe.completed, 16, "must refuse before processing another item");
    eprintln!(
        "cancellation point={point} size={size} completed={} elapsed_ns={} outcome=refused executable={}",
        probe.completed,
        elapsed.as_nanos(),
        std::env::current_exe()?.display()
    );
    assert!(elapsed < Duration::from_secs(1), "{point}: {elapsed:?}");
    // The isolated-test runner captures successful child output. Retain optional
    // measurement receipts in the caller's private evidence directory instead.
    if let Some(directory) = std::env::var_os("BRIT_REPLAY_MEASUREMENTS") {
        use std::io::Write;
        let row = serde_json::json!({
            "point": point, "size": size, "completed": probe.completed,
            "elapsedNs": elapsed.as_nanos(), "outcome": "refused",
            "executable": std::env::current_exe()?,
        });
        let path = std::path::Path::new(&directory).join(format!("cancellation-worker-{}.jsonl", std::process::id()));
        let mut file = std::fs::OpenOptions::new().create(true).append(true).open(path)?;
        writeln!(file, "{row}")?;
    }
    Ok(())
}

fn object_id(n: usize) -> gix::ObjectId {
    let mut bytes = [0; 20];
    bytes[..8].copy_from_slice(&(n as u64).to_be_bytes());
    gix::ObjectId::from_bytes_or_panic(&bytes)
}

fn index(size: usize, mode: gix::index::entry::Mode) -> gix::index::State {
    let mut state = gix::index::State::new(gix::hash::Kind::Sha1);
    for n in 0..size {
        state.dangerously_push_entry(
            Default::default(),
            object_id(n),
            gix::index::entry::Flags::empty(),
            mode,
            format!("file-{n:06}").as_bytes().as_bstr(),
        );
    }
    state.sort_entries();
    state
}

fn fixture() -> Result<(tempfile::TempDir, gix::Repository, gix::ObjectId)> {
    let temp = tempfile::tempdir()?;
    for args in [
        vec!["init", "-q", "-b", "main"],
        vec!["config", "user.name", "Loop Author"],
        vec!["config", "user.email", "loop@example.invalid"],
    ] {
        let output = gix_testtools::git_command(temp.path()).args(args).output()?;
        ensure!(
            output.status.success(),
            "fixture: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    let repo = gix::open(temp.path())?;
    let blob = repo.write_blob(b"same content")?.detach();
    let tree = repo
        .write_object(&gix::objs::Tree {
            entries: (0..32)
                .map(|n| gix::objs::tree::Entry {
                    mode: gix::objs::tree::EntryKind::Blob.into(),
                    filename: format!("file-{n:06}").into(),
                    oid: blob,
                })
                .collect(),
        })?
        .detach();
    let commit = repo.commit("HEAD", "base", tree, gix::commit::NO_PARENT_IDS)?.detach();
    Ok((temp, repo, commit))
}

#[test]
fn index_loops_cancel_after_sixteen_items() -> Result<()> {
    if gix_testtools::run_in_isolated_process().map_err(anyhow::Error::from_boxed)? {
        return Ok(());
    }
    for size in [10_000, 100_000] {
        let before = index(size, gix::index::entry::Mode::FILE);
        let after = index(size, gix::index::entry::Mode::FILE_EXECUTABLE);
        let paths = before
            .entries()
            .iter()
            .map(|entry| entry.path(&before).to_owned())
            .collect();
        measure("replay-footprint-path", size, || collect_footprint(paths))?;
        for point in ["replay-index-before", "replay-index-after", "replay-changed-path"] {
            measure(point, size, || {
                changed_index_paths(&before, &after, &mut BTreeSet::new())
            })?;
        }
    }
    Ok(())
}

#[test]
fn history_loops_cancel_after_sixteen_items() -> Result<()> {
    if gix_testtools::run_in_isolated_process().map_err(anyhow::Error::from_boxed)? {
        return Ok(());
    }
    for size in [1_000, 10_000] {
        let ours: BTreeMap<_, _> = (1..=size).map(|n| (object_id(n), vec![object_id(n - 1)])).collect();
        for point in ["replay-selected-commit", "replay-candidate-chain"] {
            measure(point, size, || {
                selected_candidates(object_id(size), &ours, &BTreeMap::new())
            })?;
        }
        let candidates = selected_candidates(object_id(size), &ours, &BTreeMap::new())?;
        assert_eq!(candidates.len(), size);
        assert_eq!(candidates[0].parent, object_id(0));
        assert_eq!(candidates[size - 1].commit, object_id(size));
        assert!(selected_candidates(object_id(size), &ours, &ours)?.is_empty());
    }
    Ok(())
}

#[test]
fn tree_and_footprint_loops_cancel_after_sixteen_items() -> Result<()> {
    if gix_testtools::run_in_isolated_process().map_err(anyhow::Error::from_boxed)? {
        return Ok(());
    }
    let (_temp, repo, commit) = fixture()?;
    for size in [1_000, 10_000] {
        measure("replay-selection-tree", size, || {
            collect_trees(&repo, std::iter::repeat_n(commit, size))
        })?;
        assert_eq!(
            collect_trees(&repo, [commit, commit])?.len(),
            1,
            "trees stay deduplicated"
        );
        let selection = Selection {
            original: commit,
            upstream: commit,
            attachment: None,
            argument: "main".into(),
            candidates: vec![Candidate { commit, parent: commit }; size],
            kind: SelectionKind::Replay,
            closure: BTreeSet::new(),
        };
        measure("replay-footprint-candidate", size, || selection.footprint(&repo))?;
        assert!(
            selection.footprint(&repo)?.is_empty(),
            "unchanged candidates have empty footprint"
        );
        // A configured driver with no matching attribute exercises the outer tree loop.
        let mut repo = repo.clone();
        repo.config_snapshot_mut().set_raw_value("filter.unused.clean", "cat")?;
        let tree = repo.find_commit(commit)?.tree_id()?.detach();
        measure("replay-filter-tree", size, || {
            refuse_external_filters(&repo, std::iter::repeat_n(tree, size), &["file".into()])
        })?;
    }
    Ok(())
}

#[test]
fn changed_paths_keep_modes_order_deduplication_and_gitlink_refusal() -> Result<()> {
    if gix_testtools::run_in_isolated_process().map_err(anyhow::Error::from_boxed)? {
        return Ok(());
    }
    use gix::index::entry::Mode;
    let empty = index(0, Mode::FILE);
    let files = index(32, Mode::FILE);
    let executable = index(32, Mode::FILE_EXECUTABLE);
    let links = index(32, Mode::COMMIT);
    let mut paths = BTreeSet::new();
    changed_index_paths(&empty, &empty, &mut paths)?;
    changed_index_paths(&files, &files, &mut paths)?;
    changed_index_paths(&links, &links, &mut paths)?;
    assert!(paths.is_empty());
    for (before, after) in [(&files, &executable), (&files, &empty), (&empty, &files)] {
        changed_index_paths(before, after, &mut paths)?;
        assert_eq!(paths.len(), 32);
        assert_eq!(
            paths.first().expect("changed paths").as_bstr(),
            b"file-000000".as_bstr()
        );
    }
    for (before, after) in [(&files, &links), (&links, &empty), (&empty, &links)] {
        let error = changed_index_paths(before, after, &mut paths).expect_err("affected gitlinks must refuse");
        assert_eq!(error.to_string(), "affected gitlinks are unsupported");
    }
    gix::interrupt::trigger();
    let result = changed_index_paths(&empty, &empty, &mut paths);
    gix::interrupt::reset();
    assert!(result.is_err(), "even an empty comparison observes cancellation");
    Ok(())
}

#[cfg(target_os = "linux")]
#[test]
fn cancelled_merge_keeps_prepared_evidence_and_can_resume_or_abort() -> Result<()> {
    if gix_testtools::run_in_isolated_process().map_err(anyhow::Error::from_boxed)? {
        return Ok(());
    }
    fn snapshot(root: &std::path::Path) -> Result<BTreeMap<std::path::PathBuf, Vec<u8>>> {
        fn walk(
            root: &std::path::Path,
            path: &std::path::Path,
            out: &mut BTreeMap<std::path::PathBuf, Vec<u8>>,
        ) -> Result<()> {
            for entry in std::fs::read_dir(path)? {
                let path = entry?.path();
                if path == root.join(".git/objects") {
                    continue;
                }
                if path.is_dir() {
                    walk(root, &path, out)?;
                } else {
                    out.insert(path.strip_prefix(root)?.to_owned(), std::fs::read(&path)?);
                }
            }
            Ok(())
        }
        let mut files = BTreeMap::new();
        walk(root, root, &mut files)?;
        Ok(files)
    }
    for point in ["replay-index-before", "replay-index-after", "replay-changed-path"] {
        for resume in [false, true] {
            let (temp, repo, base) = fixture()?;
            let base_tree = repo.find_commit(base)?.tree()?.decode()?.to_owned();
            for (reference, ordinal) in [("HEAD", 0), ("refs/heads/upstream", 31)] {
                let mut tree = base_tree.clone();
                tree.entries[ordinal].oid = repo.write_blob(reference)?.detach();
                let tree_id = repo.write_object(&tree)?.detach();
                repo.commit(reference, "independent change", tree_id, [base])?;
            }
            let output = gix_testtools::git_command(temp.path())
                .args(["reset", "--hard", "HEAD"])
                .output()?;
            ensure!(output.status.success());
            let original = repo.head_commit()?.id;
            let original_index = std::fs::read(repo.index_path())?;
            let mut session = Session::prepare(&repo, b"upstream".as_bstr())?.context("divergence")?;
            assert_eq!(session.advance(&repo)?, Advance::Started);
            let before = snapshot(temp.path())?;
            measure(point, 32, || session.advance(&repo))?;
            assert_eq!(
                snapshot(temp.path())?,
                before,
                "only unreachable merge objects may be added"
            );
            assert!(repo.git_dir().join("brit-rebase").is_dir(), "durable evidence remains");
            let mut recovered = Session::open(&repo)?;
            if resume {
                assert!(matches!(recovered.advance(&repo)?, Advance::Step { .. }));
                assert_eq!(recovered.advance(&repo)?, Advance::ReadyFinal);
            }
            let receipt = recovered.abort(&repo)?;
            assert_eq!(repo.head_commit()?.id, original);
            assert_eq!(std::fs::read(repo.index_path())?, original_index);
            assert_eq!(Session::open(&repo)?.phase(), "aborted");
            retained::inspect(&receipt.path)?;
        }
    }
    Ok(())
}
