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

/// Qualify preparation only: existing objects start durable; no replay child,
/// hook, step publication or physical power-cut claim is made by this model.
#[test]
fn replay_preparation_power_loss_requires_journal_refs_and_names_before_success() -> gix_testtools::TestResult {
    use crate::repository::replay::{Selection, Session};
    if gix_testtools::run_in_isolated_process()? {
        return Ok(());
    }
    let fixture = tempfile::tempdir()?;
    let root = fixture.path();
    for command in [
        "init -b main",
        "config user.name Replay",
        "config user.email replay@example.invalid",
    ] {
        gix_testtools::git(root, command)?;
    }
    fs::write(root.join("selected"), b"base\n")?;
    for command in ["add selected", "commit -m base", "switch -c upstream"] {
        gix_testtools::git(root, command)?;
    }
    fs::write(root.join("upstream"), b"upstream\n")?;
    for command in ["add upstream", "commit -m upstream", "switch main"] {
        gix_testtools::git(root, command)?;
    }
    fs::write(root.join("selected"), b"topic\n")?;
    for command in ["add selected", "commit -m topic"] {
        gix_testtools::git(root, command)?;
    }
    let repo = gix::open(root)?;
    let selection = Selection::prepare(&repo, b"upstream".as_bstr())?;
    let originals = [
        "selected",
        ".git/HEAD",
        ".git/index",
        ".git/refs/heads/main",
        ".git/logs/HEAD",
    ]
    .into_iter()
    .map(|name| Ok((PathBuf::from(name), snapshot(&root.join(name))?)))
    .collect::<Result<Vec<_>>>()?;
    let closure = selection
        .closure
        .iter()
        .map(|id| {
            let hex = id.to_string();
            let name = PathBuf::from(".git/objects").join(&hex[..2]).join(&hex[2..]);
            Ok((name.clone(), snapshot(&root.join(name))?))
        })
        .collect::<Result<Vec<_>>>()?;
    let root_id = identity(root)?;
    TRACE.with(|trace| *trace.borrow_mut() = Some(Vec::new()));
    baseline(root)?;
    let baseline_len = TRACE.with(|trace| trace.borrow().as_ref().expect("trace enabled").len());
    let session = Session::prepare(&repo, b"upstream".as_bstr())?.expect("divergence");
    let journal_id = identity(session.directory())?;
    let git_id = identity(repo.git_dir())?;
    let refs_id = identity(&repo.common_dir().join("refs"))?;
    let prepared_events = TRACE.with(|trace| trace.borrow().as_ref().expect("trace enabled").clone());
    check_replay_preparation_trace(&prepared_events, baseline_len, root_id, &originals, &closure, true)?;
    for omitted in [
        "journal-file",
        "ref-file",
        "journal-directory",
        "git-directory",
        "refs-directory",
    ] {
        let trace: Vec<_> = prepared_events
            .iter()
            .enumerate()
            .filter(|(position, event)| {
                if *position < baseline_len {
                    return true;
                }
                match (omitted, event) {
                    ("journal-file", Event::File(_, State::File { bytes, .. })) => {
                        serde_json::from_slice::<Envelope>(bytes).is_err()
                    }
                    ("ref-file", Event::File(_, State::File { bytes, .. })) => {
                        bytes != format!("{}\n", selection.original).as_bytes()
                            && bytes != format!("{}\n", selection.upstream).as_bytes()
                    }
                    ("journal-directory", Event::Directory(id, ..)) => *id != journal_id,
                    ("git-directory", Event::Directory(id, ..)) => *id != git_id,
                    ("refs-directory", Event::Directory(id, ..)) => *id != refs_id,
                    _ => true,
                }
            })
            .map(|(_, event)| event.clone())
            .collect();
        let rejected = std::panic::catch_unwind(|| {
            check_replay_preparation_trace(&trace, baseline_len, root_id, &originals, &closure, true)
        });
        assert!(
            rejected.is_err() || rejected.is_ok_and(|result| result.is_err()),
            "omission control must reject absent {omitted} synchronization, not pass vacuously"
        );
    }
    Session::recover_preparation(&repo)?.abort_prepared(&repo)?;
    let events = TRACE.with(|trace| trace.borrow_mut().take().expect("trace enabled"));
    check_replay_preparation_trace(&events, baseline_len, root_id, &originals, &closure, false)?;
    Ok(())
}

fn check_replay_preparation_trace(
    events: &[Event],
    baseline_len: usize,
    root_id: Identity,
    originals: &[(PathBuf, State)],
    closure: &[(PathBuf, State)],
    terminal_prepared: bool,
) -> Result<()> {
    let mut durable = Durable::default();
    let mut saw_prepared = false;
    let mut final_prepared = false;
    for (position, event) in events.iter().enumerate() {
        durable.apply(event);
        if position < baseline_len {
            continue;
        }
        for (name, expected) in originals {
            assert_eq!(
                durable.state(root_id, name),
                *expected,
                "preparation changed {} at sync {position}",
                name.display()
            );
        }
        final_prepared = false;
        if let State::File { bytes, .. } = durable.state(root_id, Path::new(".git/brit-rebase/state.json")) {
            let envelope: Envelope = serde_json::from_slice(&bytes)?;
            assert_eq!(digest(&envelope.payload)?, envelope.checksum);
            let state: serde_json::Value = serde_json::from_slice(&envelope.payload)?;
            if state["phase"] == "prepared" {
                saw_prepared = true;
                final_prepared = true;
                let nonce = state["nonce"].as_str().expect("nonce");
                for label in ["original", "upstream"] {
                    let name = PathBuf::from(format!(".git/refs/brit-rebase-retain/{nonce}/{label}"));
                    let State::File { bytes, .. } = durable.state(root_id, &name) else {
                        panic!("prepared journal precedes durable retention {label} at sync {position}");
                    };
                    assert_eq!(bytes, format!("{}\n", state[label].as_str().expect("OID")).as_bytes());
                }
                for (name, expected) in closure {
                    assert_eq!(
                        durable.state(root_id, name),
                        *expected,
                        "prepared closure missing {}",
                        name.display()
                    );
                }
            }
        }
    }
    assert!(
        saw_prepared,
        "successful preparation must have durable prepared state, even under omission controls"
    );
    assert_eq!(
        final_prepared, terminal_prepared,
        "returned outcome must match terminal durable state"
    );
    if !terminal_prepared {
        assert_eq!(
            durable.state(root_id, Path::new(".git/brit-rebase")),
            State::Absent,
            "returned cancellation must durably retire active journal"
        );
    }
    Ok(())
}

struct TraceMember {
    paths: Vec<PathBuf>,
    identity: Identity,
    state: State,
}
struct ChildTraceProof {
    decision: TraceMember,
    value: serde_json::Value,
    proposal: Option<TraceMember>,
    raw: Vec<TraceMember>,
    invocations: Vec<TraceMember>,
    directories: Vec<Identity>,
}
struct ReplayTraceProof {
    root_id: Identity,
    baseline_len: usize,
    original: BTreeMap<PathBuf, State>,
    terminal: BTreeMap<PathBuf, State>,
    decisions: Vec<(PathBuf, State, serde_json::Value)>,
    children: Vec<ChildTraceProof>,
    invocations: Vec<[TraceMember; 3]>,
    invocation_directories: Vec<Identity>,
    objects: BTreeMap<PathBuf, State>,
    references: BTreeMap<PathBuf, State>,
    retained: BTreeMap<PathBuf, State>,
    abort: bool,
}

#[test]
fn replay_steps_final_abort_and_cleanup_power_loss_follow_real_sync_boundaries() -> gix_testtools::TestResult {
    use crate::repository::replay::{Advance, Session};
    if gix_testtools::run_in_isolated_process()? {
        return Ok(());
    }
    let selected_mode = std::env::var("BRIT_REPLAY_MODEL_MODE").ok();
    let mut executed = 0;
    for (mode, abort) in [
        "linear",
        "reword",
        "squash",
        "fixup",
        "drop",
        "break",
        "conflict",
        "conflict-skip",
        "empty",
        "empty-skip",
        "edit-amend",
        "edit-split",
        "edit-skip",
        "nested",
        "file-to-directory",
        "directory-to-file",
    ]
    .into_iter()
    .flat_map(|mode| [false, true].map(|abort| (mode, abort)))
    {
        if selected_mode.as_deref().is_some_and(|selected| selected != mode) {
            continue;
        }
        executed += 1;
        eprintln!("replay loss model: mode={mode}, abort={abort}");
        let fixture = tempfile::tempdir()?;
        let root = fixture.path();
        for command in [
            "init -b main",
            "config user.name Replay",
            "config user.email replay@example.invalid",
        ] {
            gix_testtools::git(root, command)?;
        }
        fs::write(root.join("selected"), b"base\n")?;
        fs::write(root.join("neighbor"), b"clean neighbor\n")?;
        if mode == "file-to-directory" {
            fs::write(root.join("slot"), b"baseline file\n")?;
        }
        if mode == "directory-to-file" {
            fs::create_dir(root.join("slot"))?;
            fs::write(root.join("slot/child"), b"baseline child\n")?;
        }
        for command in ["add .", "commit -m base", "switch -c upstream"] {
            gix_testtools::git(root, command)?;
        }
        fs::write(root.join("upstream"), b"upstream\n")?;
        if mode.starts_with("conflict") {
            fs::write(root.join("selected"), b"upstream conflict\n")?;
        }
        if mode.starts_with("empty") {
            fs::write(root.join("selected"), b"topic\n")?;
        }
        for command in ["add .", "commit -m upstream", "switch main"] {
            gix_testtools::git(root, command)?;
        }
        fs::write(root.join("selected"), b"topic\n")?;
        for command in ["add selected", "commit -m topic"] {
            gix_testtools::git(root, command)?;
        }
        let first = gix::open(root)?.head_id()?.to_string();
        let second = if mode != "linear" {
            fs::write(root.join("second"), b"second topic\n")?;
            if mode == "nested" {
                fs::create_dir_all(root.join("new/deep"))?;
                fs::write(root.join("new/deep/leaf"), b"nested replay\n")?;
            }
            if mode == "file-to-directory" {
                fs::remove_file(root.join("slot"))?;
                fs::create_dir(root.join("slot"))?;
                fs::write(root.join("slot/child"), b"replayed child\n")?;
            }
            if mode == "directory-to-file" {
                fs::remove_file(root.join("slot/child"))?;
                fs::remove_dir(root.join("slot"))?;
                fs::write(root.join("slot"), b"replayed file\n")?;
            }
            for command in ["add .", "commit -m second"] {
                gix_testtools::git(root, command)?;
            }
            Some(gix::open(root)?.head_id()?.to_string())
        } else {
            None
        };
        if mode.starts_with("edit-") {
            use std::os::unix::fs::PermissionsExt;
            for name in [
                "pre-commit",
                "prepare-commit-msg",
                "commit-msg",
                "post-commit",
                "post-rewrite",
            ] {
                let path = root.join(".git/hooks").join(name);
                fs::write(&path, b"#!/bin/sh\nexit 0\n")?;
                fs::set_permissions(path, fs::Permissions::from_mode(0o700))?;
            }
        }
        let _editor = gix_testtools::Env::new().set("GIT_EDITOR", ":");
        fs::write(root.join("neighbor"), b"staged neighbor\n")?;
        gix_testtools::git(root, "add neighbor")?;
        fs::write(root.join("neighbor"), b"unstaged neighbor\n")?;
        let repo = gix::open(root)?;
        let mut proof = ReplayTraceProof {
            root_id: identity(root)?,
            baseline_len: 0,
            original: [
                ".git/HEAD",
                ".git/index",
                ".git/refs/heads/main",
                "selected",
                "neighbor",
            ]
            .into_iter()
            .map(|name| Ok((PathBuf::from(name), snapshot(&root.join(name))?)))
            .collect::<Result<_>>()?,
            terminal: BTreeMap::new(),
            decisions: Vec::new(),
            children: Vec::new(),
            invocations: Vec::new(),
            invocation_directories: Vec::new(),
            objects: BTreeMap::new(),
            references: BTreeMap::new(),
            retained: BTreeMap::new(),
            abort,
        };
        for name in ["second", "slot", "slot/child", "new", "new/deep", "new/deep/leaf"] {
            if fs::symlink_metadata(root.join(name)).is_ok() {
                proof.original.insert(PathBuf::from(name), snapshot(&root.join(name))?);
            }
        }
        TRACE.with(|trace| *trace.borrow_mut() = Some(Vec::new()));
        baseline(root)?;
        proof.baseline_len = TRACE.with(|trace| trace.borrow().as_ref().expect("trace enabled").len());
        let mut session = if let Some(second) = second {
            let mut session = Session::prepare_interactive(&repo, b"upstream".as_bstr())?.expect("interactive");
            let todo = if mode == "break" {
                format!("pick {first}\nbreak\npick {second}\n")
            } else {
                let command = if mode.starts_with("edit-") {
                    "edit"
                } else if matches!(mode, "reword" | "squash" | "fixup" | "drop") {
                    mode
                } else {
                    "pick"
                };
                format!("pick {first}\n{command} {second}\n")
            };
            session.accept_todo(&repo, todo.as_bytes())?;
            session
        } else {
            Session::prepare(&repo, b"upstream".as_bstr())?.expect("divergence")
        };
        assert_eq!(session.advance(&repo)?, Advance::Started);
        loop {
            let phase = session.phase().to_owned();
            match session
                .advance(&repo)
                .with_context(|| format!("model {mode}, abort={abort}, advance from {phase}"))?
            {
                Advance::ReadyFinal => break,
                Advance::Step { .. } | Advance::Dropped | Advance::Break => {}
                Advance::Conflict => {
                    if mode.ends_with("skip") {
                        session.skip(&repo)?;
                    } else {
                        fs::write(root.join("selected"), b"resolved conflict\n")?;
                        session.stage(&repo, &["selected".into()], false)?;
                        session.continue_conflict(&repo)?;
                    }
                }
                Advance::Empty => {
                    if mode.ends_with("skip") {
                        session.skip(&repo)?;
                    }
                }
                Advance::Edit => {
                    if mode == "edit-amend" {
                        fs::write(root.join("second"), b"amended second\n")?;
                        session.stage(&repo, &["second".into()], false)?;
                        session.amend(&repo, b"amended message\n")?;
                    } else {
                        session.begin_split(&repo)?;
                        fs::write(root.join("second"), b"split partial\n")?;
                        session.stage(&repo, &["second".into()], false)?;
                        session.commit_split(&repo, b"split first\n")?;
                        if mode == "edit-split" {
                            fs::write(root.join("second"), b"split completed\n")?;
                            session.stage(&repo, &["second".into()], false)?;
                            session.commit_split(&repo, b"split second\n")?;
                        }
                    }
                    if mode == "edit-skip" {
                        session.skip(&repo)?;
                    } else {
                        session.continue_edit(&repo)?;
                    }
                }
                other => panic!("unexpected {mode} model stop: {other:?}"),
            }
        }
        if abort {
            session.abort(&repo)?;
        } else {
            session.finish(&repo)?;
        }
        proof.children = collect_replay_child_proofs(root, session.directory())?;
        collect_replay_invocations(root, session.directory(), None, &mut proof.invocations)?;
        proof.invocation_directories = proof
            .invocations
            .iter()
            .map(|receipts| identity(&root.join(receipts[0].paths[0].parent().expect("invocation directory"))))
            .collect::<Result<_>>()?;
        for entry in fs::read_dir(session.directory())? {
            let entry = entry?;
            if entry.file_name().to_string_lossy().starts_with("decision-") {
                let state = snapshot(&entry.path())?;
                let State::File { bytes, .. } = &state else {
                    return Err(anyhow::anyhow!("decision is not a file").into());
                };
                let envelope: Envelope = serde_json::from_slice(bytes)?;
                proof.decisions.push((
                    entry.path().strip_prefix(root)?.to_owned(),
                    state.clone(),
                    serde_json::from_slice(&envelope.payload)?,
                ));
            }
        }
        proof
            .decisions
            .sort_by_key(|(_, _, value)| value["ordinal"].as_u64().expect("decision ordinal"));
        for (_, _, decision) in &proof.decisions {
            for value in decision["after"]["closure"].as_array().expect("closure") {
                let oid = value.as_str().expect("object ID");
                let name = PathBuf::from(".git/objects").join(&oid[..2]).join(&oid[2..]);
                proof.objects.insert(name.clone(), snapshot(&root.join(name))?);
            }
        }
        fn collect_files(root: &Path, directory: &Path, out: &mut BTreeMap<PathBuf, State>) -> Result<()> {
            for entry in fs::read_dir(directory)? {
                let entry = entry?;
                if entry.file_type()?.is_dir() {
                    collect_files(root, &entry.path(), out)?;
                } else {
                    out.insert(entry.path().strip_prefix(root)?.to_owned(), snapshot(&entry.path())?);
                }
            }
            Ok(())
        }
        collect_files(
            root,
            &repo.common_dir().join("refs/brit-rebase-retain"),
            &mut proof.references,
        )?;
        proof.terminal = proof
            .original
            .keys()
            .map(|name| Ok((name.clone(), snapshot(&root.join(name))?)))
            .collect::<Result<_>>()?;
        let receipt = session.cleanup(&repo)?;
        collect_files(root, &receipt.path, &mut proof.retained)?;
        let retained_directory_ids = [
            receipt.path.as_path(),
            receipt.path.parent().expect("copy operation"),
            receipt.path.parent().and_then(Path::parent).expect("retained root"),
        ]
        .into_iter()
        .map(identity)
        .collect::<Result<Vec<_>>>()?;
        let events = TRACE.with(|trace| trace.borrow_mut().take().expect("trace enabled"));
        check_replay_execution_trace(&events, &proof)?;
        let mut omissions = vec!["decision", "new-object", "retained-member", "retained-directory"];
        if !proof.children.is_empty() {
            omissions.extend(["child-decision", "child-namespace"]);
        }
        if proof.children.iter().any(|child| child.proposal.is_some()) {
            omissions.push("child-proposal");
        }
        if proof.children.iter().any(|child| !child.raw.is_empty()) {
            omissions.push("child-raw-object");
        }
        if proof.children.iter().any(|child| !child.invocations.is_empty()) {
            omissions.push("child-invocation");
        }
        if !proof.invocations.is_empty() {
            omissions.extend([
                "invocation-start",
                "invocation-launch",
                "invocation-exit",
                "invocation-namespace",
            ]);
        }
        for omission in omissions {
            let baseline_objects: Vec<_> = events[..proof.baseline_len]
                .iter()
                .filter_map(|event| match event {
                    Event::File(_, state) if proof.objects.values().any(|expected| expected == state) => {
                        Some(state.clone())
                    }
                    _ => None,
                })
                .collect();
            let reduced = events
                .iter()
                .enumerate()
                .filter(|(position, event)| {
                    if *position < proof.baseline_len {
                        return true;
                    }
                    match (omission, event) {
                        ("decision", Event::File(_, state)) => {
                            !proof.decisions.iter().any(|(_, expected, _)| expected == state)
                        }
                        ("new-object", Event::File(_, state)) => {
                            baseline_objects.contains(state)
                                || !proof.objects.values().any(|expected| expected == state)
                        }
                        ("retained-member", Event::File(_, state)) => {
                            !proof.retained.values().any(|expected| expected == state)
                        }
                        ("retained-directory", Event::Directory(id, ..)) => !retained_directory_ids.contains(id),
                        ("invocation-start" | "invocation-launch" | "invocation-exit", Event::File(id, _)) => {
                            let ordinal = match omission {
                                "invocation-start" => 0,
                                "invocation-launch" => 1,
                                _ => 2,
                            };
                            !proof
                                .invocations
                                .iter()
                                .any(|receipts| receipts[ordinal].identity == *id)
                        }
                        ("invocation-namespace", Event::Directory(id, ..)) => {
                            !proof.invocation_directories.contains(id)
                        }
                        ("child-decision", Event::File(id, _)) => {
                            !proof.children.iter().any(|child| child.decision.identity == *id)
                        }
                        ("child-proposal", Event::File(id, _)) => !proof
                            .children
                            .iter()
                            .any(|child| child.proposal.as_ref().is_some_and(|proposal| proposal.identity == *id)),
                        ("child-raw-object", Event::File(id, _)) => !proof
                            .children
                            .iter()
                            .flat_map(|child| &child.raw)
                            .any(|raw| raw.identity == *id),
                        ("child-invocation", Event::File(id, _)) => !proof
                            .children
                            .iter()
                            .flat_map(|child| &child.invocations)
                            .any(|receipt| receipt.identity == *id),
                        ("child-namespace", Event::Directory(id, ..)) => {
                            !proof.children.iter().any(|child| child.directories.contains(id))
                        }
                        _ => true,
                    }
                })
                .map(|(_, event)| event.clone())
                .collect::<Vec<_>>();
            assert!(
                reduced.len() < events.len(),
                "omission control must remove observed {omission} synchronization events"
            );
            let rejected = std::panic::catch_unwind(|| check_replay_execution_trace(&reduced, &proof));
            assert!(
                rejected.is_err() || rejected.is_ok_and(|result| result.is_err()),
                "missing {omission} synchronization must break a real replay invariant (mode={mode}, abort={abort})"
            );
        }
    }
    assert!(executed > 0, "unknown BRIT_REPLAY_MODEL_MODE selector");
    Ok(())
}

fn collect_replay_invocations(
    root: &Path,
    directory: &Path,
    archive: Option<&Path>,
    out: &mut Vec<[TraceMember; 3]>,
) -> Result<()> {
    if directory.join("start.json").exists() {
        let receipts = ["start.json", "launch.json", "exit.json"]
            .map(|name| {
                let path = directory.join(name);
                let mut paths = vec![path.strip_prefix(root)?.to_owned()];
                if let Some(archive) = archive {
                    paths.push(Path::new(".git/brit-authoring").join(path.strip_prefix(archive)?));
                }
                Ok(TraceMember {
                    paths,
                    identity: identity(&path)?,
                    state: snapshot(&path)?,
                })
            })
            .into_iter()
            .collect::<Result<Vec<_>>>()?;
        out.push(
            receipts
                .try_into()
                .map_err(|_| anyhow::anyhow!("three invocation receipts required"))?,
        );
        return Ok(());
    }
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        if entry.file_type()?.is_dir() {
            let path = entry.path();
            let archive = if entry.file_name().to_string_lossy().ends_with("-accepted") {
                Some(path.as_path())
            } else {
                archive
            };
            collect_replay_invocations(root, &path, archive, out)?;
        }
    }
    Ok(())
}

fn collect_replay_child_proofs(root: &Path, parent: &Path) -> Result<Vec<ChildTraceProof>> {
    fn member(root: &Path, archive: &Path, path: &Path) -> Result<TraceMember> {
        Ok(TraceMember {
            paths: vec![
                path.strip_prefix(root)?.to_owned(),
                Path::new(".git/brit-authoring").join(path.strip_prefix(archive)?),
            ],
            identity: identity(path)?,
            state: snapshot(path)?,
        })
    }
    fn payload(path: &Path) -> Result<serde_json::Value> {
        let envelope: Envelope = serde_json::from_slice(&fs::read(path)?)?;
        Ok(serde_json::from_slice(&envelope.payload)?)
    }
    let mut out = Vec::new();
    for entry in fs::read_dir(parent)? {
        let entry = entry?;
        if !entry.file_type()?.is_dir() || !entry.file_name().to_string_lossy().ends_with("-accepted") {
            continue;
        }
        let archive = entry.path();
        let mut raw = BTreeMap::new();
        if archive.join("objects").exists() {
            for object in fs::read_dir(archive.join("objects"))? {
                let path = object?.path();
                raw.insert(
                    path.file_name()
                        .context("raw object name")?
                        .to_string_lossy()
                        .into_owned(),
                    (path.clone(), payload(&path)?),
                );
            }
        }
        for decision in fs::read_dir(&archive)? {
            let decision = decision?;
            let name = decision.file_name().to_string_lossy().into_owned();
            let Some(number) = name
                .strip_prefix("child-")
                .and_then(|name| name.strip_suffix(".json"))
                .and_then(|number| number.parse::<usize>().ok())
            else {
                continue;
            };
            let value = payload(&decision.path())?;
            let proposed = archive.join(format!("child-{number}-proposed.json"));
            let proposal = proposed
                .exists()
                .then(|| member(root, &archive, &proposed))
                .transpose()?;
            let mut required_raw = Vec::new();
            if proposal.is_some() {
                let mut stack = vec![value["after"]["head"].as_str().context("child head")?.to_owned()];
                stack.extend(
                    value["after"]["private"]
                        .as_array()
                        .context("private entries")?
                        .iter()
                        .map(|entry| entry["id"].as_str().expect("entry object ID").to_owned()),
                );
                let mut visited = BTreeSet::new();
                while let Some(id) = stack.pop() {
                    if !visited.insert(id.clone()) {
                        continue;
                    }
                    let Some((path, object)) = raw.get(&id) else {
                        let original_head: Vec<u8> = serde_json::from_value(value["before"]["original_head"].clone())?;
                        let baseline = std::str::from_utf8(&original_head)?.trim();
                        ensure!(
                            id == baseline || value["before"]["parent"].as_str() == Some(id.as_str()),
                            "child proposal closure is missing raw object {id} outside its pinned baseline"
                        );
                        continue;
                    };
                    required_raw.push(member(root, &archive, path)?);
                    let data: Vec<u8> = serde_json::from_value(object["data"].clone())?;
                    match object["kind"].as_str().context("retained object kind")? {
                        "commit" => {
                            let commit = gix::objs::CommitRef::from_bytes(&data, gix::hash::Kind::Sha1)?;
                            stack.push(commit.tree().to_string());
                            stack.extend(commit.parents().map(|parent| parent.to_string()));
                        }
                        "tree" => stack.extend(
                            gix::objs::TreeRef::from_bytes(&data, gix::hash::Kind::Sha1)?
                                .entries
                                .iter()
                                .map(|entry| entry.oid.to_string()),
                        ),
                        "blob" => {}
                        kind => anyhow::bail!("unexpected child raw kind {kind}"),
                    }
                }
            }
            let mut invocations = Vec::new();
            let mut directories = vec![identity(&archive)?];
            if archive.join("objects").exists() {
                directories.push(identity(&archive.join("objects"))?);
            }
            for number in 1..=value["after"]["invocation"].as_u64().context("invocation number")? {
                let invocation = archive.join(format!("invocation-{number}"));
                directories.push(identity(&invocation)?);
                for name in ["start.json", "launch.json", "exit.json"] {
                    invocations.push(member(root, &archive, &invocation.join(name))?);
                }
            }
            out.push(ChildTraceProof {
                decision: member(root, &archive, &decision.path())?,
                value,
                proposal,
                raw: required_raw,
                invocations,
                directories,
            });
        }
    }
    out.sort_by_key(|child| {
        (
            child.value["after"]["replay_parent"]["step"].as_u64(),
            child.value["after"]["children"].as_u64(),
        )
    });
    Ok(out)
}

fn check_replay_execution_trace(events: &[Event], proof: &ReplayTraceProof) -> Result<()> {
    let mut durable = Durable::default();
    let mut decisions = BTreeSet::new();
    let mut completed_invocations = BTreeSet::new();
    let mut child_decisions = BTreeSet::new();
    let mut child_proposals = BTreeSet::new();
    let mut child_index_images: Vec<Vec<u8>> = Vec::new();
    let mut child_head_images: Vec<Vec<u8>> = Vec::new();
    let mut seen_refs = BTreeSet::new();
    let mut final_decided = false;
    let mut abort_decided = false;
    let mut index_images = vec![proof.original[Path::new(".git/index")].clone()];
    let mut head_images = vec![proof.original[Path::new(".git/HEAD")].clone()];
    for (position, event) in events.iter().enumerate() {
        durable.apply(event);
        if position < proof.baseline_len {
            continue;
        }
        for (name, expected, decision) in &proof.decisions {
            if !decisions.contains(name) && durable.state(proof.root_id, name) == *expected {
                for oid in decision["after"]["closure"].as_array().expect("decision closure") {
                    let oid = oid.as_str().expect("object ID");
                    let object = PathBuf::from(".git/objects").join(&oid[..2]).join(&oid[2..]);
                    assert_eq!(
                        durable.state(proof.root_id, &object),
                        proof.objects[&object],
                        "decision precedes object durability at sync {position}"
                    );
                }
                let nonce = decision["nonce"].as_str().expect("nonce");
                for item in decision["after"]["retained"].as_array().expect("owned refs") {
                    let name = PathBuf::from(format!(
                        ".git/refs/brit-rebase-retain/{nonce}/{}",
                        item[0].as_str().expect("ref label")
                    ));
                    let State::File { bytes, .. } = durable.state(proof.root_id, &name) else {
                        panic!("decision has no durable retention ref at sync {position}");
                    };
                    assert_eq!(bytes, format!("{}\n", item[1].as_str().expect("ref OID")).as_bytes());
                }
                final_decided |= decision["transition"]["kind"] == "Final";
                abort_decided |= decision["transition"]["kind"] == "Abort";
                index_images.push(serde_json::from_value(decision["after"]["index"].clone())?);
                head_images.push(serde_json::from_value(decision["after"]["head"].clone())?);
                decisions.insert(name.clone());
            }
        }
        for (ordinal, receipts) in proof.invocations.iter().enumerate() {
            if completed_invocations.contains(&ordinal) {
                continue;
            }
            let visible = |member: &TraceMember| {
                member
                    .paths
                    .iter()
                    .any(|path| durable.state(proof.root_id, path) == member.state)
            };
            if visible(&receipts[1]) {
                assert!(
                    visible(&receipts[0]),
                    "invocation launch precedes durable start at sync {position}"
                );
            }
            if visible(&receipts[2]) {
                assert!(
                    visible(&receipts[0]) && visible(&receipts[1]),
                    "invocation exit precedes durable launch/start at sync {position}"
                );
                completed_invocations.insert(ordinal);
            }
        }
        for (ordinal, child) in proof.children.iter().enumerate() {
            let visible = |member: &TraceMember| {
                member
                    .paths
                    .iter()
                    .any(|path| durable.state(proof.root_id, path) == member.state)
            };
            if let Some(proposal) = &child.proposal
                && !child_proposals.contains(&ordinal)
                && visible(proposal)
            {
                for raw in &child.raw {
                    assert!(
                        visible(raw),
                        "child proposal precedes raw closure durability at sync {position}: {:?}",
                        raw.paths
                    );
                }
                child_proposals.insert(ordinal);
            }
            if !child_decisions.contains(&ordinal) && visible(&child.decision) {
                if child.proposal.is_some() {
                    assert!(
                        child_proposals.contains(&ordinal),
                        "child publication requires prior durable proposal at sync {position}"
                    );
                    let binding = &child.value["after"]["replay_parent"];
                    let name = PathBuf::from(format!(
                        ".git/refs/brit-rebase-retain/{}/child-{}-{}",
                        binding["nonce"].as_str().expect("nonce"),
                        binding["step"].as_u64().expect("parent ordinal"),
                        child.value["after"]["children"].as_u64().expect("child ordinal")
                    ));
                    let State::File { bytes, .. } = durable.state(proof.root_id, &name) else {
                        panic!("child commit decision has no owned durable retention ref at sync {position}");
                    };
                    assert_eq!(
                        bytes,
                        format!("{}\n", child.value["after"]["head"].as_str().expect("child head")).as_bytes()
                    );
                    if child.value["after"]["head"] != child.value["before"]["parent"] {
                        assert!(
                            child.value["after"]["post_hooks"].is_object(),
                            "authored child decision durably queues post notifications"
                        );
                    }
                }
                for receipt in &child.invocations {
                    assert!(
                        visible(receipt),
                        "completed child transition precedes invocation receipt durability at sync {position}: {:?}",
                        receipt.paths
                    );
                }
                child_index_images.push(serde_json::from_value(child.value["new_index"].clone())?);
                child_head_images.push(serde_json::from_value(child.value["new_head"].clone())?);
                child_decisions.insert(ordinal);
            }
        }
        assert!(
            index_images.contains(&durable.state(proof.root_id, Path::new(".git/index")))
                || matches!(durable.state(proof.root_id, Path::new(".git/index")), State::File { bytes, .. } if child_index_images.contains(&bytes)),
            "index publication precedes a durable decision at sync {position}"
        );
        assert!(
            head_images.contains(&durable.state(proof.root_id, Path::new(".git/HEAD")))
                || matches!(durable.state(proof.root_id, Path::new(".git/HEAD")), State::File { bytes, .. } if child_head_images.contains(&bytes)),
            "HEAD publication precedes a durable decision at sync {position}"
        );
        assert_eq!(
            durable.state(proof.root_id, Path::new("neighbor")),
            proof.original[Path::new("neighbor")],
            "preserved work is immutable across every sync boundary"
        );
        if durable.state(proof.root_id, Path::new(".git/refs/heads/main"))
            != proof.original[Path::new(".git/refs/heads/main")]
        {
            assert!(
                final_decided,
                "original branch cannot publish before durable Final decision and closure"
            );
        }
        for (name, expected) in &proof.references {
            let actual = durable.state(proof.root_id, name);
            if actual == *expected {
                seen_refs.insert(name.clone());
            } else if seen_refs.contains(name) && actual == State::Absent {
                for (payload, expected) in &proof.retained {
                    assert_eq!(
                        durable.state(proof.root_id, payload),
                        *expected,
                        "released ref requires complete independent copy including payload/manifest/COPY_COMPLETE at sync {position}"
                    );
                }
            }
        }
    }
    assert_eq!(
        decisions.len(),
        proof.decisions.len(),
        "every returned decision must be durably observed; omission controls cannot pass vacuously"
    );
    assert_eq!(
        child_decisions.len(),
        proof.children.len(),
        "every completed child decision must be durably observed"
    );
    assert_eq!(
        child_proposals.len(),
        proof.children.iter().filter(|child| child.proposal.is_some()).count(),
        "every authored proposal must be durably observed"
    );
    assert_eq!(
        completed_invocations.len(),
        proof.invocations.len(),
        "every completed invocation must have durable start, launch and exit receipts"
    );
    assert_eq!(abort_decided, proof.abort);
    assert_eq!(final_decided, !proof.abort);
    for (name, expected) in &proof.terminal {
        assert_eq!(
            durable.state(proof.root_id, name),
            *expected,
            "terminal resource {} must be durable",
            name.display()
        );
    }
    if proof.abort {
        assert_eq!(
            durable.state(proof.root_id, Path::new(".git/index")),
            proof.original[Path::new(".git/index")],
            "completed Abort restores byte-exact original index"
        );
    }
    assert_eq!(
        durable.state(proof.root_id, Path::new(".git/brit-rebase")),
        State::Absent
    );
    for name in proof.references.keys() {
        assert_eq!(
            durable.state(proof.root_id, name),
            State::Absent,
            "cleanup return releases every retained ref"
        );
    }
    Ok(())
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
