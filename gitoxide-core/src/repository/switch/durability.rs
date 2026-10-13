//! A volatile/durable namespace model fed by successful synchronization calls in the real engine.
//! It qualifies ordering; it is deliberately not hardware power-cut qualification.
use super::*;
use std::{cell::RefCell, os::unix::fs::MetadataExt};
type Identity = (u64, u64);
#[derive(Clone)]
enum Event {
    File(Identity, State),
    Directory(Identity, State, BTreeMap<Vec<u8>, Identity>),
}
thread_local! { static TRACE: RefCell<Option<Vec<Event>>> = const { RefCell::new(None) }; }
fn identity(path: &Path) -> Result<Identity> {
    let metadata = fs::symlink_metadata(path)?;
    Ok((metadata.dev(), metadata.ino()))
}
fn emit(event: Event) {
    TRACE.with(|trace| {
        if let Some(trace) = trace.borrow_mut().as_mut() {
            trace.push(event);
        }
    });
}
pub(super) fn record_file(path: &Path) -> Result<()> {
    emit(Event::File(identity(path)?, snapshot(path)?));
    Ok(())
}
pub(super) fn record_dir(path: &Path) -> Result<()> {
    // Directory synchronization persists symlink inode contents along with its name.
    let mut entries = BTreeMap::new();
    for entry in fs::read_dir(path)? {
        let entry = entry?;
        let id = identity(&entry.path())?;
        if entry.file_type()?.is_symlink() {
            emit(Event::File(id, snapshot(&entry.path())?));
        }
        entries.insert(raw(Path::new(&entry.file_name()))?, id);
    }
    emit(Event::Directory(identity(path)?, snapshot(path)?, entries));
    Ok(())
}
fn baseline(path: &Path) -> Result<()> {
    if fs::symlink_metadata(path)?.is_dir() {
        for entry in fs::read_dir(path)? {
            baseline(&entry?.path())?;
        }
        record_dir(path)
    } else {
        record_file(path)
    }
}
#[derive(Default)]
struct Durable {
    data: BTreeMap<Identity, State>,
    names: BTreeMap<Identity, BTreeMap<Vec<u8>, Identity>>,
}
impl Durable {
    fn apply(&mut self, event: &Event) {
        match event {
            Event::File(id, data) => {
                self.data.insert(*id, data.clone());
            }
            Event::Directory(id, data, names) => {
                self.data.insert(*id, data.clone());
                self.names.insert(*id, names.clone());
            }
        }
    }
    fn resolve(&self, root: Identity, relative: &Path) -> Option<Identity> {
        let mut current = root;
        for component in relative.components() {
            let name = raw(Path::new(component.as_os_str())).ok()?;
            let next = self.names.get(&current).and_then(|names| names.get(&name))?;
            current = *next;
        }
        Some(current)
    }
    fn state(&self, root: Identity, relative: &Path) -> State {
        self.resolve(root, relative)
            .and_then(|id| self.data.get(&id))
            .cloned()
            .unwrap_or(State::Absent)
    }
}

#[test]
fn power_loss_drops_unsynchronized_bytes_and_names_at_every_real_sync_boundary() -> gix_testtools::TestResult {
    if gix_testtools::run_in_isolated_process()? {
        return Ok(());
    }
    for shape in ["file", "directory", "symlink", "directory-to-file"] {
        let fixture = tempfile::tempdir()?;
        let root = fixture.path();
        for command in [
            "init -b main",
            "config user.name Switch",
            "config user.email switch@example.invalid",
        ] {
            gix_testtools::git(root, command)?;
        }
        if shape == "directory-to-file" {
            fs::create_dir(root.join("selected"))?;
            fs::write(root.join("selected/child"), b"original\n")?;
        } else {
            fs::write(root.join("selected"), b"original\n")?;
        }
        fs::write(root.join("neighbor"), b"neighbor\n")?;
        gix_testtools::git(root, "add .")?;
        gix_testtools::git(root, "commit -m base")?;
        gix_testtools::git(root, "switch -c destination")?;
        match shape {
            "directory" => {
                fs::remove_file(root.join("selected"))?;
                fs::create_dir(root.join("selected"))?;
                fs::write(root.join("selected/child"), b"destination\n")?;
            }
            "symlink" => {
                fs::remove_file(root.join("selected"))?;
                std::os::unix::fs::symlink("neighbor", root.join("selected"))?;
            }
            "directory-to-file" => {
                fs::remove_file(root.join("selected/child"))?;
                fs::remove_dir(root.join("selected"))?;
                fs::write(root.join("selected"), b"destination\n")?;
            }
            _ => fs::write(root.join("selected"), b"destination\n")?,
        }
        gix_testtools::git(root, "add .")?;
        gix_testtools::git(root, "commit -m target")?;
        gix_testtools::git(root, "switch main")?;
        let root_id = identity(root)?;
        let original = snapshot(&root.join("selected"))?;
        TRACE.with(|trace| *trace.borrow_mut() = Some(Vec::new()));
        baseline(root)?;
        let baseline_len = TRACE.with(|trace| trace.borrow().as_ref().map_or(0, Vec::len));
        execute(gix::open(root)?, b"destination".as_bstr())?;
        let events = TRACE.with(|trace| trace.borrow_mut().take().expect("trace enabled"));
        check_trace(&events, baseline_len, root, root_id, &original)?;
        if shape != "symlink" {
            let missing_data_sync: Vec<_> = events
                .iter()
                .filter(
                    |event| !matches!(event, Event::File(_, State::File { bytes, .. }) if bytes == b"destination\n"),
                )
                .cloned()
                .collect();
            let rejected =
                std::panic::catch_unwind(|| check_trace(&missing_data_sync, baseline_len, root, root_id, &original));
            assert!(
                rejected.is_err() || rejected.is_ok_and(|result| result.is_err()),
                "negative control must detect missing destination data synchronization"
            );
        }
    }
    Ok(())
}

fn check_trace(events: &[Event], baseline_len: usize, root: &Path, root_id: Identity, original: &State) -> Result<()> {
    let mut durable = Durable::default();
    let mut decisions = 0;
    let mut pull_seen = false;
    let mut fetch_decisions = 0;
    let mut completed = false;
    let mut committed = None::<Manifest>;
    for (position, event) in events.iter().enumerate() {
        durable.apply(event);
        if position < baseline_len {
            continue;
        }
        // Reconstruct the post-power-loss namespace: unsynchronized writes and renames
        // are absent by construction; only synchronized directory entries reach data.
        let active = durable.state(root_id, Path::new(".git/brit-switch"));
        let manifest = durable.state(root_id, Path::new(".git/brit-switch/manifest.json"));
        let mut active_manifest = None;
        if let State::File { bytes, .. } = manifest {
            let envelope: Envelope = serde_json::from_slice(&bytes)?;
            assert_eq!(
                digest(&envelope.payload)?,
                envelope.checksum,
                "durable manifest checksum at boundary {position}"
            );
            let state: Manifest = serde_json::from_slice(&envelope.payload)?;
            active_manifest = Some(state.clone());
            if let Some(pull) = &state.pull {
                pull_seen = true;
                if pull.fetch_decision {
                    fetch_decisions += 1;
                    assert!(pull.fetched, "fetch decision requires received objects");
                    for (name, checksum) in &pull.objects {
                        let resource = Path::new(".git/objects").join(path(name)?);
                        let State::File { bytes, .. } = durable.state(root_id, &resource) else {
                            panic!(
                                "fetch decision precedes durable object {} at boundary {position}",
                                resource.display()
                            );
                        };
                        assert_eq!(
                            digest(&bytes)?,
                            *checksum,
                            "durable object checksum at boundary {position}"
                        );
                    }
                }
                if pull.fetch_done {
                    assert!(pull.fetch_decision, "fetch completion requires fetch decision");
                    for change in &pull.fetch_metadata {
                        assert_eq!(
                            durable.state(root_id, path(&change.path)?.strip_prefix(root)?),
                            change.after,
                            "fetch completion precedes durable metadata at boundary {position}"
                        );
                    }
                }
                if state.decision {
                    assert!(pull.fetch_done, "integration decision requires completed fetch");
                }
            }
            if state.decision {
                decisions += 1;
                for change in &state.changes {
                    assert_eq!(
                        durable.state(root_id, &path(&change.path)?),
                        change.after,
                        "decision cannot precede durable destination at boundary {position}"
                    );
                }
                if state.hook != "not-started" {
                    for change in [&state.index, &state.head, &state.reflog]
                        .into_iter()
                        .chain(state.pull.iter().flat_map(|pull| pull.metadata.iter()))
                    {
                        assert_eq!(
                            durable.state(root_id, path(&change.path)?.strip_prefix(root)?),
                            change.after,
                            "hook cannot precede durable metadata at boundary {position}"
                        );
                    }
                }
                committed = Some(state);
            }
        } else if matches!(active, State::Directory { .. }) {
            // The initial directory entry is synced only after the first manifest is durable.
            panic!("active journal has no durable manifest at boundary {position}");
        }
        if active == State::Absent && committed.is_none() {
            assert_eq!(
                &durable.state(root_id, Path::new("selected")),
                original,
                "worktree publication must not precede durable journal/backups at boundary {position}"
            );
        }
        for resource in [
            ".git/index.lock",
            ".git/HEAD.lock",
            ".git/packed-refs.lock",
            ".git/refs/heads/main.lock",
            ".git/refs/heads/destination.lock",
        ] {
            if let Some(id) = durable.resolve(root_id, Path::new(resource)) {
                let state = active_manifest
                    .as_ref()
                    .expect("durable ordinary lock requires durable recovery journal");
                let record = state
                    .locks
                    .iter()
                    .find(|record| path(&record.path).ok().as_deref() == Some(root.join(resource).as_path()))
                    .expect("durable ordinary lock requires prior ownership intent");
                assert_eq!(
                    id,
                    (record.device, record.inode),
                    "lock ownership must precede publication"
                );
                let State::File { bytes, .. } = durable.state(root_id, Path::new(resource)) else {
                    panic!("lock token data not durable");
                };
                assert_eq!(
                    digest(&bytes)?,
                    record.checksum,
                    "lock token bytes must be durable before publication"
                );
            }
        }
        if active == State::Absent
            && let Some(state) = &committed
        {
            completed = true;
            for change in [&state.index, &state.head, &state.reflog]
                .into_iter()
                .chain(state.pull.iter().flat_map(|pull| pull.metadata.iter()))
            {
                assert_eq!(
                    durable.state(root_id, path(&change.path)?.strip_prefix(root)?),
                    change.after,
                    "cleanup cannot precede durable metadata at boundary {position}"
                );
            }
        }
    }
    assert!(
        decisions > 0 && completed,
        "model must observe actual decision and cleanup"
    );
    assert!(
        !pull_seen || fetch_decisions > 0,
        "pull trace must observe both decisions"
    );
    assert!(
        events.len() > baseline_len + 20,
        "real execution must exercise the synchronization model"
    );
    Ok(())
}

#[cfg(feature = "blocking-client")]
#[test]
fn pull_power_loss_preserves_objects_before_both_durable_decisions() -> gix_testtools::TestResult {
    if gix_testtools::run_in_isolated_process()? {
        return Ok(());
    }
    for no_pack in [false, true] {
        let fixture = tempfile::tempdir()?;
        let remote = fixture.path().join("remote");
        let root = fixture.path().join("local");
        fs::create_dir(&remote)?;
        for command in [
            "init -b main",
            "config user.name Pull",
            "config user.email pull@example.invalid",
        ] {
            gix_testtools::git(&remote, command)?;
        }
        fs::write(remote.join("selected"), b"original\n")?;
        gix_testtools::git(&remote, "add .")?;
        gix_testtools::git(&remote, "commit -m base")?;
        let clone = gix_testtools::git_command(fixture.path())
            .args(["clone", "remote", "local"])
            .output()?;
        assert!(
            clone.status.success(),
            "clone fixture: {}",
            String::from_utf8_lossy(&clone.stderr)
        );
        for command in ["config user.name Pull", "config user.email pull@example.invalid"] {
            gix_testtools::git(&root, command)?;
        }
        fs::write(remote.join("selected"), b"destination\n")?;
        gix_testtools::git(&remote, "add .")?;
        gix_testtools::git(&remote, "commit -m target")?;
        if no_pack {
            gix_testtools::git(&root, "fetch origin")?;
        }
        let root_id = identity(&root)?;
        let original = snapshot(&root.join("selected"))?;
        TRACE.with(|trace| *trace.borrow_mut() = Some(Vec::new()));
        baseline(&root)?;
        let baseline_len = TRACE.with(|trace| trace.borrow().as_ref().map_or(0, Vec::len));
        pull_execute(gix::open(&root)?, "origin", b"main".as_bstr())?;
        let events = TRACE.with(|trace| trace.borrow_mut().take().expect("trace enabled"));
        check_trace(&events, baseline_len, &root, root_id, &original)?;
        // Every received object listed in the durable decision must survive loss of
        // volatile inode bytes AND volatile directory entries. Delete each class of
        // sync event separately to prove the checker is sensitive to both.
        let mut object_ids = BTreeSet::new();
        let mut object_dirs = BTreeSet::new();
        fn collect(path: &Path, files: &mut BTreeSet<Identity>, dirs: &mut BTreeSet<Identity>) -> Result<()> {
            if path.is_dir() {
                dirs.insert(identity(path)?);
                for entry in fs::read_dir(path)? {
                    collect(&entry?.path(), files, dirs)?;
                }
            } else {
                files.insert(identity(path)?);
            }
            Ok(())
        }
        collect(&root.join(".git/objects"), &mut object_ids, &mut object_dirs)?;
        for missing in ["object-data", "object-names", "destination-data"] {
            // Existing object bytes in a no-pack baseline are already durable. The
            // no-pack positive case covers that path; negative object controls must
            // remove their baseline records too to model their initial volatility.
            let retain = |event: &&Event| match (missing, *event) {
                ("object-data", Event::File(id, _)) => !object_ids.contains(id),
                ("object-names", Event::Directory(id, _, _)) => !object_dirs.contains(id),
                ("destination-data", Event::File(_, State::File { bytes, .. })) => bytes != b"destination\n",
                _ => true,
            };
            let filtered_baseline = events[..baseline_len].iter().filter(retain).count();
            let filtered: Vec<_> = events.iter().filter(retain).cloned().collect();
            let rejected =
                std::panic::catch_unwind(|| check_trace(&filtered, filtered_baseline, &root, root_id, &original));
            assert!(
                rejected.is_err() || rejected.is_ok_and(|result| result.is_err()),
                "negative control must detect missing {missing} sync (no_pack={no_pack})"
            );
        }
    }
    Ok(())
}
