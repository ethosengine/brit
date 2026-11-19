//! The durable replay journal under `<git-dir>/brit-rebase`.
//!
//! # Preparation is cancellable without its own success
//!
//! A preparation that fails, for any reason including the very verification it
//! was running, must never leave a journal that only its own success can clear.
//!
//! 1. Nothing live is changed before the phase leaves preparation. `prepare_mode`
//!    and `complete_preparation` read HEAD, the index, branches and worktree files
//!    and write only the private journal directory, its records and payload copies,
//!    and the two retention references; the object directory is only synchronized.
//!    The first live write is the initial checkout decision, which `advance` makes
//!    only from phase `prepared`.
//! 2. Every artifact preparation creates is named in the journal before it exists:
//!    `state.json` is published before any payload or reference is created, payload
//!    files are the `payloads` keys, and the references are named by `nonce` and the
//!    `retained` labels. A directory without `state.json` therefore holds nothing else.
//! 3. Cancellation ([`Session::cancel_preparation`]) reads only those named artifacts
//!    and the pinned HEAD, index and branch fingerprints. It never reads or verifies
//!    an object, never re-captures preservation, never builds a retained copy, and is
//!    exempt from the journal-open scratch quota.
//! 4. If live state differs from the pinned fingerprints, cancellation still removes
//!    Brit's own artifacts and reports each difference; it restores nothing.
//!
//! In phase `prepared`, `advance` may already have written two artifacts of fixed
//! name before it records `invoking-pre`: the `index.pre-rebase` private index and a
//! `pre-rebase` invocation directory. The hook is spawned only after that record is
//! durable, so while the phase is `prepared` the hook never ran, and cancellation
//! also removes exactly these two when the directory holds no launch or exit
//! receipt. Any other member, or a later phase, leaves the journal to the full abort.
//!
//! # A successor record is never in effect before its rename
//!
//! Every journal record is published the same way: `save` (or `save_bytes`)
//! writes the whole successor as `state.pending` and synchronizes it (checkpoint
//! `replay-state-file-synced`), then `publish_state` renames it onto `state.json`
//! and synchronizes the directory (`replay-state-directory-synced`). Nothing else
//! happens between those two points, in any phase. The write order per transition:
//!
//! | Transition (writer) | Live writes before the record is written | Between "pending synced" and "renamed" | A `state.pending` found beside a valid `state.json` |
//! |---|---|---|---|
//! | first record, `preparing` (`prepare_mode`, `save_bytes`) | none | nothing | (no `state.json`: the pending record is the only one; cancellation releases it) |
//! | `preparing` → `prepared` (`complete_preparation`) | none (refs and payloads are named first) | nothing | rolled forward when it equals `state.json` but for the phase (`recover_preparation`, cancellation); otherwise discarded |
//! | `prepared` → `invoking-pre` and the hook's outcome phases | private only (`index.pre-rebase`, the invocation directory) | nothing | discarded |
//! | every decision: checkout, stop, step, skip, final, abort (`apply_decision`) | the durable decision record, then the transition's live writes | nothing | discarded: the decision record remains and is rolled forward again, as after a kill just before the save |
//! | phase-only records (`step-proposed`, `message`, `replaying`, `ready-final`, `break`, `edit-accepted`, `message-ready`, `retaining-skip`, `retaining-abort`, `completed`, `invoking-post` and its outcomes, the cleanup copy count) | whatever the step did before, guarded by the record already in effect | nothing | discarded |
//! | the legacy payload upgrade (`migrate_inline_work`) | none (private payload files only) | nothing | its own two successors are rolled forward; any other is discarded |
//! | cancellation (`cancelling-prepared`) | none | nothing | discarded; cancellation runs again |
//!
//! No transition writes anything live between the two points, so discarding a
//! complete pending record leaves the journal exactly as a kill at the instant
//! before it was written would have, a state every command already recovers
//! from. `save` therefore discards an unrenamed `state.pending` before writing
//! its own (a retry builds a different successor than the killed command did, so
//! comparing them, as Brit once did, stranded the rebase), and a torn one too.
//! Without `state.json` a pending record is the journal's only record and is kept.
//!
//! The same holds for the other records written in one sequential pass: a
//! decision record cut short (its JSON ends early) never became durable and is
//! discarded, since the transition writes nothing live before it; a staged
//! proposal (`plan.json`) cut short beside its whole owner record is written
//! again, and an abort does not adopt a staging directory whose owner record or
//! plan is absent or cut short (it is kept as evidence, as an absent one was).
//!
//! # The scratch quota never walls in recovery
//!
//! The scratch quota meters the journal, and a skip's retained copy; it is a
//! policy about starting and continuing work only:
//!
//! - Opening a journal never applies it (one record is bounded before it is read),
//!   so `--status`, `--abort` and `--continue` can always read the journal.
//! - Admission at the end of preparation projects the whole planned replay's
//!   journal, with one stop and its resolution, and cancels the preparation unless
//!   it fits (see the `copy_bound` module). Every later forward step
//!   (`--continue`, a staged resolution, `--skip`, an edit) is reserved for before
//!   anything of it is written, and a refusal names `--abort`. The retained copy
//!   an abort or a cleanup writes is not metered and is not projected: it is
//!   tested only at the preflight and at admission, and after admission it can
//!   grow past the quota with the rebase.
//! - Undoing and finishing are never metered: [`Session::abort`] with every
//!   decision it makes (accepting and discarding an authoring child, rolling a
//!   durable decision forward, the abort's own transition), the retained copy it
//!   writes, [`Session::cleanup`] and its resumption, and everything after a final
//!   decision is durable run inside `switch::payload::Recovery`. Real disk space
//!   still bounds them; a failed write removes its own partial file, so after an
//!   `ENOSPC` the same command resumes once space is free.
//!
//! `--retire` is only for a journal of this repository whose remaining work is its
//! retained copy: `completed`, `aborted`, or an abort retaining before it changed
//! anything live. A journal this repository cannot open as its own is reported by
//! `--status` and touched by no command.
//!
//! Not covered: a `.brit-checkout-*` scratch directory in the Git directory is
//! named by no journal. A process killed while one exists (any journal save, a
//! payload copy, a switch or pull start) leaves it; it blocks no command, and
//! `rebase --status` and `switch --status` report it with the command that removes it.
use super::*;
#[path = "interactive.rs"]
mod interactive;
#[path = "payload.rs"]
mod payload;
use crate::repository::authoring::conflict::ParentBinding;
use crate::repository::{authoring::conflict::Preservation, switch};
use gix::objs::FindStream;
use gix_sequencer::Command;
use serde::{Deserialize, Serialize};
use std::{
    fs,
    os::unix::fs::{MetadataExt, PermissionsExt},
    path::Path,
};

/// A capability issued only from the exact persisted parent state. Its
/// fields are private so callers cannot manufacture a pending-check bypass.
pub(crate) struct ChildAuthorization {
    binding: ParentBinding,
}

impl ChildAuthorization {
    pub(crate) fn validate(&self, repo: &gix::Repository) -> Result<()> {
        validate_child_binding(repo, &self.binding)
    }
    pub(crate) fn binding(&self) -> ParentBinding {
        self.binding.clone()
    }

    pub(crate) fn validate_stop(
        &self,
        repo: &gix::Repository,
        preservation: &crate::repository::authoring::conflict::Preservation,
        kind: crate::repository::authoring::conflict::StopKind,
        active: &[BString],
        private: &gix::index::State,
    ) -> Result<BTreeMap<Vec<u8>, switch::State>> {
        self.validate(repo)?;
        let envelope: Envelope = serde_json::from_slice(&fs::read(repo.git_dir().join("brit-rebase/state.json"))?)?;
        let state: ParentState = serde_json::from_slice(&envelope.payload)?;
        ensure!(
            state.authorized_stop.as_ref() == Some(&stop_digest(preservation, kind, active, private)?),
            "replay parent does not authorize supplied child context"
        );
        Ok(state
            .work
            .into_iter()
            .map(|change| (change.path, change.after))
            .collect())
    }
}

fn stop_digest(
    preservation: &crate::repository::authoring::conflict::Preservation,
    kind: crate::repository::authoring::conflict::StopKind,
    active: &[BString],
    private: &gix::index::State,
) -> Result<String> {
    let active: Vec<_> = active.iter().map(|path| path.to_vec()).collect();
    let entries: Vec<_> = private
        .entries()
        .iter()
        .map(|entry| {
            (
                entry.path(private).to_vec(),
                entry.id.to_string(),
                entry.mode.bits(),
                entry.flags.bits(),
            )
        })
        .collect();
    let bytes = serde_json::to_vec(&(preservation, kind, active, entries))?;
    let mut hash = gix::hash::hasher(gix::hash::Kind::Sha1);
    hash.update(&bytes);
    Ok(hash.try_finalize()?.to_string())
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Envelope {
    payload: Vec<u8>,
    checksum: String,
}

// Versioned ownership header. The durable replay driver is the sole writer;
// child sessions only consume its checked authorization.
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ParentState {
    version: u32,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    payloads: BTreeMap<String, u64>,
    nonce: String,
    step: usize,
    phase: String,
    owner: Vec<u8>,
    worktree: Vec<u8>,
    common: Vec<u8>,
    identities: Vec<(u64, u64)>,
    original_head: Vec<u8>,
    original: String,
    upstream: String,
    argument: Vec<u8>,
    candidates: Vec<(String, String)>,
    closure: Vec<String>,
    retained: Vec<(String, String)>,
    preservation: crate::repository::authoring::conflict::Preservation,
    authorized_stop: Option<String>,
    accepted_child: Option<(ParentBinding, String)>,
    accepted_children: Vec<(ParentBinding, String)>,
    tip: String,
    attachment: Option<Vec<u8>>,
    work: Vec<switch::Change>,
    index: switch::State,
    head: switch::State,
    log: switch::State,
    original_log: switch::State,
    original_index_state: switch::State,
    original_head_state: switch::State,
    original_orig_head: switch::State,
    original_branch_log: Option<switch::State>,
    published_branch_log: Option<switch::State>,
    decisions: usize,
    mappings: Vec<(String, Vec<String>)>,
    copies: u64,
    cleanup_copy: bool,
    abandoned_plans: Vec<(String, String)>,
    witness: String,
    stop_tree: Option<String>,
    policy: String,
    post_outcome: Option<crate::repository::authoring::invocation::InvocationOutcome>,
    #[serde(default)]
    interactive: Option<interactive::Interactive>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Advance {
    Started,
    Step { commit: gix::ObjectId },
    Empty,
    Conflict,
    ReadyFinal,
    Edit,
    Break,
    Message,
    Dropped,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Decision {
    version: u32,
    nonce: String,
    ordinal: usize,
    before: String,
    after: ParentState,
    transition: super::transition::Transition,
}

/// A private preparation journal. Live replay transitions are driven explicitly;
/// opening or inspecting it never launches a hook or advances a step.
pub struct Session {
    directory: std::path::PathBuf,
    state: ParentState,
}

impl Session {
    /// Capture preservation once, reserve this repository against cooperating
    /// commands, and protect both pinned histories against local object pruning.
    /// A no-op returns no session and changes no metadata.
    pub fn prepare(repo: &gix::Repository, argument: &BStr) -> Result<Option<Self>> {
        Self::prepare_mode(repo, argument, false)
    }

    pub fn prepare_interactive(repo: &gix::Repository, argument: &BStr) -> Result<Option<Self>> {
        Self::prepare_mode(repo, argument, true)
    }

    fn prepare_mode(repo: &gix::Repository, argument: &BStr, interactive: bool) -> Result<Option<Self>> {
        use std::{io::Read, os::unix::fs::DirBuilderExt};
        let selection = Selection::prepare_for_session(repo, argument, interactive, switch::payload::scratch_limit())?;
        if selection.kind == SelectionKind::Noop && (!interactive || selection.candidates.is_empty()) {
            return Ok(None);
        }
        ensure!(
            repo.config_snapshot()
                .string("extensions.refStorage")
                .is_none_or(|backend| backend.as_bstr() == b"files"),
            "replay requires the files reference backend"
        );
        let _guard = switch::shared_guard(repo)?;
        switch::ensure_no_pending(repo)?;
        selection.validate_head(repo)?;
        ensure!(!gix::interrupt::is_triggered(), "replay preparation interrupted");
        let preservation = Preservation::capture_for_replay(repo)?;
        let active = selection
            .footprint(repo)?
            .into_iter()
            .map(|p| p.to_vec())
            .collect::<Vec<_>>();
        preservation.validate_scope(&active)?;
        let root = repo.workdir().context("worktree required")?;
        let original_tree = repo.index_from_tree(&repo.tree_id_of_commit(selection.original)?)?;
        let witness = crate::repository::authoring::conflict::worktree_witness(repo, &active)?;
        let mut work = Vec::new();
        for path in active {
            ensure!(!gix::interrupt::is_triggered(), "replay preparation interrupted");
            validate_scope_ancestors(root, &path, &original_tree)?;
            let image = switch::payload::fingerprint(&root.join(gix::path::from_bstr(path.as_bstr())?.as_ref()))?;

            ensure!(
                original_tree.entry_by_path(path.as_bstr()).is_some()
                    || image == switch::State::Absent
                    || (matches!(image, switch::State::Directory { .. })
                        && original_tree
                            .entries()
                            .iter()
                            .any(|e| e.path(&original_tree).starts_with(&path)
                                && e.path(&original_tree).get(path.len()) == Some(&b'/'))),
                "untracked or ignored collision in replay footprint"
            );
            if matches!(image, switch::State::Directory { .. }) {
                validate_original_directory(root, &path, &original_tree)?;
            }
            work.push(switch::Change {
                path,
                before: image.clone(),
                after: image,
            });
        }
        let mut entropy = [0; 32];
        fs::File::open("/dev/urandom")?.read_exact(&mut entropy)?;
        let nonce = switch::digest(&entropy)?;
        for label in ["original", "upstream"] {
            ensure!(
                repo.try_find_reference(format!("refs/brit-rebase-retain/{nonce}/{label}").as_str())?
                    .is_none(),
                "replay nonce collides with existing retention ownership"
            );
        }
        if let Some(name) = &selection.attachment {
            let owner = fs::canonicalize(repo.git_dir())?;
            for worktree in repo.worktrees_including_main()? {
                let worktree = worktree?;
                let directory = fs::canonicalize(worktree.current_dir().join(worktree.git_dir()))?;
                if directory != owner {
                    let expected = [b"ref: ".as_slice(), name.as_bytes(), b"\n"].concat();
                    ensure!(
                        fs::read(directory.join("HEAD"))? != expected,
                        "original branch occupied by another worktree"
                    );
                }
            }
        }
        let directory = repo.git_dir().join("brit-rebase");
        let identities = [repo.git_dir(), root, repo.common_dir()]
            .into_iter()
            .map(|path| fs::symlink_metadata(path).map(|m| (m.dev(), m.ino())))
            .collect::<std::io::Result<_>>()?;
        let mut state = ParentState {
            version: 2,
            payloads: work
                .iter()
                .filter_map(|change| match &change.before {
                    switch::State::StoredFile { size, checksum, .. } => Some((checksum.clone(), *size)),
                    _ => None,
                })
                .chain(preservation.raw_images().filter_map(|(_, image)| match image {
                    switch::State::StoredFile { size, checksum, .. } => Some((checksum, size)),
                    _ => None,
                }))
                .collect(),
            nonce,
            step: 0,
            phase: "preparing".into(),
            owner: raw(repo.git_dir())?,
            worktree: raw(root)?,
            common: raw(repo.common_dir())?,
            identities,
            original_head: fs::read(repo.git_dir().join("HEAD"))?,
            original: selection.original.to_string(),
            upstream: selection.upstream.to_string(),
            argument: selection.argument.to_vec(),
            candidates: selection
                .candidates
                .iter()
                .map(|c| (c.commit.to_string(), c.parent.to_string()))
                .collect(),
            closure: selection.closure.iter().map(ToString::to_string).collect(),
            preservation,
            retained: vec![
                ("original".into(), selection.original.to_string()),
                ("upstream".into(), selection.upstream.to_string()),
            ],
            authorized_stop: None,
            accepted_child: None,
            accepted_children: Vec::new(),
            tip: selection.original.to_string(),
            attachment: selection.attachment.as_ref().map(|p| p.to_vec()),
            work,
            index: switch::payload::fingerprint(&repo.index_path())?,
            head: switch::snapshot(&repo.git_dir().join("HEAD"))?,
            log: switch::payload::fingerprint(&repo.git_dir().join("logs/HEAD"))?,
            original_log: switch::payload::fingerprint(&repo.git_dir().join("logs/HEAD"))?,
            original_index_state: switch::payload::fingerprint(&repo.index_path())?,
            original_head_state: switch::snapshot(&repo.git_dir().join("HEAD"))?,
            original_orig_head: switch::snapshot(&repo.git_dir().join("ORIG_HEAD"))?,
            original_branch_log: selection
                .attachment
                .as_ref()
                .map(|name| {
                    switch::payload::fingerprint(
                        &repo
                            .common_dir()
                            .join("logs")
                            .join(gix::path::from_bstr(name.as_bstr())?.as_ref()),
                    )
                })
                .transpose()?,
            published_branch_log: None,
            decisions: 0,
            mappings: Vec::new(),
            copies: 0,
            cleanup_copy: false,
            abandoned_plans: Vec::new(),
            witness,
            stop_tree: None,
            policy: super::hooks::policy(repo)?,
            post_outcome: None,
            interactive: (interactive && !selection.candidates.is_empty())
                .then(|| interactive::Interactive::new(&selection.candidates))
                .transpose()?,
        };
        let metadata_payloads = state
            .metadata_images()
            .filter_map(|image| match image {
                switch::State::StoredFile { size, checksum, .. } => Some((checksum.clone(), *size)),
                _ => None,
            })
            .collect::<Vec<_>>();
        state.payloads.extend(metadata_payloads);
        // All preservation and metadata reads precede journal allocation. Once
        // save_bytes starts, interruption leaves a recoverable preparation record.
        switch::checkpoint("replay-before-journal")?;
        fs::DirBuilder::new().mode(0o700).create(&directory)?;
        let allocated = fs::symlink_metadata(&directory)?;
        let identity = (allocated.dev(), allocated.ino());
        state.identities.push(identity);
        let session = Self { directory, state };
        let unpublished = (|| -> Result<Vec<u8>> {
            session.validate_branch(repo, None)?;
            let bytes = session.state_bytes()?;
            switch::checkpoint("replay-journal-ready")?;
            Ok(bytes)
        })();
        let bytes = match unpublished {
            Ok(bytes) => bytes,
            Err(error) => {
                let current = fs::symlink_metadata(&session.directory)?;
                ensure!(
                    (current.dev(), current.ino()) == identity,
                    "foreign preparation directory"
                );
                // Never remove members or a replacement directory: an empty,
                // still-owned unpublished allocation alone is ours to release.
                fs::remove_dir(&session.directory)?;
                switch::sync_dir(repo.git_dir())?;
                return Err(error);
            }
        };
        let mut session = session;
        session.save_bytes(&bytes, true)?;
        drop(bytes);
        switch::sync_dir(repo.git_dir())?;
        switch::checkpoint("replay-prepared")?;
        session.complete_preparation(repo)?;
        if let Err(error) = session.admit(repo) {
            // Nothing live has changed: release the preparation under the guard
            // still held, so the refusal leaves no journal behind.
            Self::cancel_locked(repo, &session.directory)?;
            return Err(error);
        }
        Ok(Some(session))
    }

    fn complete_preparation(&mut self, repo: &gix::Repository) -> Result<()> {
        switch::validate_local_history(repo)?;
        switch::validate_object_boundaries(repo)?;
        ensure!(self.state.phase == "preparing", "preparation already completed");
        self.prepare_payloads(repo)?;
        sync_tree(&repo.common_dir().join("objects"))?;
        let mut retained_objects = self.state.closure.iter().cloned().collect::<BTreeSet<_>>();
        retained_objects.extend(
            self.state
                .preservation
                .original_object_ids()?
                .iter()
                .map(ToString::to_string),
        );
        for value in &retained_objects {
            switch::checkpoint("replay-retention-verify")?;
            let id = gix::ObjectId::from_hex(value.as_bytes())?;
            super::validation::verify_object(repo, id)?;
            ensure!(
                !gix::interrupt::is_triggered(),
                "replay retention verification interrupted"
            );
        }
        for (label, value) in &self.state.retained {
            switch::checkpoint("replay-retention-reference")?;
            let name = format!("refs/brit-rebase-retain/{}/{label}", self.state.nonce);
            let id = gix::ObjectId::from_hex(value.as_bytes())?;
            match repo.try_find_reference(name.as_str())? {
                Some(reference) => ensure!(
                    matches!(reference.target(), gix::refs::TargetRef::Object(_)) && reference.id() == id,
                    "foreign replay retention reference"
                ),
                None => {
                    ensure!(
                        !gix::interrupt::is_triggered(),
                        "replay retention publication interrupted"
                    );
                    repo.reference(name, id, gix::refs::transaction::PreviousValue::MustNotExist, "")?;
                    switch::checkpoint("replay-retention-reference-published")?;
                }
            }
        }
        sync_reference_storage(repo)?;
        switch::checkpoint("replay-retained")?;
        self.state.phase = "prepared".into();
        self.save()?;
        Ok(())
    }

    pub fn directory(&self) -> &Path {
        &self.directory
    }

    pub fn post_hook_outcome(&self) -> Option<crate::repository::authoring::invocation::InvocationOutcome> {
        self.state.post_outcome
    }

    pub fn phase(&self) -> &str {
        &self.state.phase
    }
    pub fn completed_steps(&self) -> usize {
        self.state.step
    }
    pub fn tip(&self) -> Result<gix::ObjectId> {
        Ok(gix::ObjectId::from_hex(self.state.tip.as_bytes())?)
    }

    fn validate_current(&self, repo: &gix::Repository) -> Result<()> {
        self.inspect_editor()?;
        super::validate_execution_policy(repo)?;
        ensure!(
            super::hooks::policy(repo)? == self.state.policy,
            "replay configuration or hooks changed"
        );
        let current = Self::open(repo)?;
        ensure!(
            serde_json::to_vec(&current.state)? == serde_json::to_vec(&self.state)?,
            "stale replay session"
        );
        ensure!(
            !self
                .directory
                .join(format!("decision-{}.json", self.state.decisions + 1))
                .exists(),
            "recover the durable replay decision before advancing"
        );
        self.validate_retention(repo)?;
        self.validate_branch(
            repo,
            matches!(
                self.state.phase.as_str(),
                "final-published" | "invoking-post" | "post-unknown" | "completed"
            )
            .then_some(self.state.tip.as_str()),
        )?;
        if self
            .state
            .interactive
            .as_ref()
            .is_some_and(|interactive| interactive.edit.is_some())
            && self.state.phase == "authoring"
            && repo.git_dir().join("brit-authoring").exists()
        {
            // A child-owned scope expansion is recorded in its immutable parent
            // binding and child decisions; validate that ledger until acceptance.
            let authorization = self.child_authorization()?;
            crate::repository::authoring::conflict::Session::open_owned(repo, &authorization)?.validate(repo)?;
        } else {
            let excluded = self.state.work.iter().map(|c| c.path.clone()).collect::<Vec<_>>();
            ensure!(
                crate::repository::authoring::conflict::worktree_witness(repo, &excluded)? == self.state.witness,
                "worktree changed outside replay scope"
            );
        }
        self.state.preservation.validate_observed(repo, &*repo.open_index()?)?;
        Ok(())
    }

    /// Execute at most one logical replay step. READY_FINAL is deliberately
    /// separate so callers can still abort every completed step before final
    /// publication is decided.
    pub fn advance(&mut self, repo: &gix::Repository) -> Result<Advance> {
        if self.is_edit_stop() {
            if self
                .state
                .interactive
                .as_ref()
                .is_some_and(|interactive| interactive.editors.last().is_some_and(|editor| !editor.accepted))
            {
                self.accept_editor(repo)?;
            }
            return self.continue_edit(repo);
        }
        if self.state.phase == "prepared" {
            {
                let _guard = switch::shared_guard(repo)?;
                self.validate_current(repo)?;
                let index = repo.index_from_tree(
                    &repo.tree_id_of_commit(gix::ObjectId::from_hex(self.state.original.as_bytes())?)?,
                )?;
                let mut bytes = Vec::new();
                index.write_to(&mut bytes, Default::default())?;
                let path = self.directory.join("index.pre-rebase");
                match fs::symlink_metadata(&path) {
                    Ok(metadata) => {
                        ensure!(
                            metadata.is_file()
                                && metadata.uid() == fs::metadata(&self.directory)?.uid()
                                && metadata.permissions().mode().trailing_zeros() >= 6
                                && fs::read(&path)? == bytes,
                            "pre-rebase private index changed"
                        );
                        switch::sync_path(&path)?;
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => switch::private_write(&path, &bytes)?,
                    Err(error) => return Err(error.into()),
                }
                switch::sync_dir(&self.directory)?;
            }
            let directory = self.directory.join("pre-rebase");
            let argument = gix::path::from_bstr(self.state.argument.as_bstr())?
                .into_owned()
                .into_os_string();
            let index = self.directory.join("index.pre-rebase");
            let outcome = super::hooks::run(repo, &directory, "pre-rebase", &[argument], None, &index, || {
                let _guard = switch::shared_guard(repo)?;
                self.validate_current(repo)?;
                ensure!(
                    switch::snapshot(&repo.git_dir().join("HEAD"))? == self.state.head
                        && switch::payload::fingerprint(&repo.index_path())? == self.state.index
                        && switch::payload::fingerprint(&repo.git_dir().join("logs/HEAD"))? == self.state.log,
                    "replay metadata changed before pre-rebase launch"
                );
                for change in &self.state.work {
                    ensure!(
                        switch::snapshot(
                            &repo
                                .workdir()
                                .context("worktree required")?
                                .join(gix::path::from_bstr(change.path.as_bstr())?.as_ref(),)
                        )? == change.after,
                        "replay work changed before pre-rebase launch"
                    );
                }
                self.state.phase = "invoking-pre".into();
                self.save()
            })?;
            ensure!(
                outcome != crate::repository::authoring::invocation::InvocationOutcome::Unknown,
                "unknown pre-rebase outcome; inspect or abort, never rerun"
            );
            let _guard = switch::shared_guard(repo)?;
            self.validate_current(repo)?;
            self.state.phase = if outcome.success() {
                "pre-approved"
            } else {
                "pre-rejected"
            }
            .into();
            self.save()?;
            ensure!(
                outcome.success(),
                "pre-rebase hook rejected replay; abort remains available"
            );
        }
        if self
            .state
            .interactive
            .as_ref()
            .is_some_and(|interactive| interactive.editors.last().is_some_and(|editor| !editor.accepted))
        {
            self.accept_editor(repo)?;
        }
        if self.state.phase == "pre-approved"
            && self
                .state
                .interactive
                .as_ref()
                .is_some_and(|interactive| !interactive.initial_accepted)
        {
            self.edit_todo(repo)?;
        }
        if self.state.phase == "message" {
            // No unaccepted invocation remains here. Preparation may have stopped
            // before editor resolution, so this explicit continue can start it.
            self.edit_message(repo)?;
        }
        let _guard = switch::shared_guard(repo)?;
        self.validate_current(repo)?;
        if self.state.phase == "pre-approved" {
            let tip = gix::ObjectId::from_hex(self.state.upstream.as_bytes())?;
            let tree = repo.tree_id_of_commit(tip)?.detach();
            let private = repo.index_from_tree(&tree)?;
            let (mut after, transition) =
                self.checkout_transition(repo, tree, &private, tip, super::transition::Kind::Checkout)?;
            after.phase = "replaying".into();
            self.publish_decision(repo, after, transition)?;
            return Ok(Advance::Started);
        }
        if self.state.phase == "edit-accepted" {
            self.archive_accepted(repo)?;
            if let Some(receipt) = self.state.accepted_child.take() {
                self.state.accepted_children.push(receipt);
            }
            self.state.authorized_stop = None;
            self.state.phase = "replaying".into();
            self.save()?;
            return Ok(Advance::Step { commit: self.tip()? });
        }
        if self.state.phase == "step-proposed" {
            return self.finish_proposal(repo);
        }
        if self.state.phase == "break" {
            self.state.phase = "replaying".into();
            self.save()?;
        }
        ensure!(
            matches!(self.state.phase.as_str(), "replaying" | "empty" | "message-ready"),
            "replay is stopped; use the action named by its phase"
        );
        if self
            .state
            .interactive
            .as_ref()
            .map_or(self.state.step == self.state.candidates.len(), |interactive| {
                interactive.cursor == interactive.todo.entries.len()
            })
        {
            self.state.phase = "ready-final".into();
            self.save()?;
            return Ok(Advance::ReadyFinal);
        }
        match self.current_command()? {
            Command::Break => {
                self.state
                    .interactive
                    .as_mut()
                    .context("break requires interactive todo")?
                    .cursor += 1;
                self.state.phase = "break".into();
                self.save()?;
                return Ok(Advance::Break);
            }
            Command::Drop => {
                let original = self.current_candidate()?.commit.to_string();
                self.state
                    .interactive
                    .as_mut()
                    .context("drop requires interactive todo")?
                    .ledger
                    .record(&original, &[])?;
                self.state.mappings.push((original, Vec::new()));
                self.state.step += 1;
                self.state
                    .interactive
                    .as_mut()
                    .context("drop requires interactive todo")?
                    .cursor += 1;
                self.save()?;
                return Ok(Advance::Dropped);
            }
            _ => {}
        }
        let candidate = self.current_candidate()?;
        let step = if self.state.phase == "message-ready" {
            let tree = gix::ObjectId::from_hex(
                self.state
                    .stop_tree
                    .as_ref()
                    .context("message tree missing")?
                    .as_bytes(),
            )?;
            Step {
                original: candidate.commit,
                parent: self.tip()?,
                tree,
                index: repo.index_from_tree(&tree)?,
                conflicts: Vec::new(),
                footprint: Vec::new(),
                initially_empty: false,
                newly_empty: false,
            }
        } else {
            Step::merge(repo, &candidate, self.tip()?)?
        };
        // What this step stores, reserved before anything of it is written.
        self.reserve(repo, self.step_need(repo, Some(&step.index), false, &step.footprint)?)?;
        self.grow_scope(repo, &step.footprint)?;
        if !step.conflicts.is_empty() {
            let (mut after, transition) =
                self.checkout_transition(repo, step.tree, &step.index, self.tip()?, super::transition::Kind::Stop)?;
            after.phase = "authoring".into();
            after.stop_tree = Some(step.tree.to_string());
            self.retain_stop(repo, &mut after, step.tree)?;
            after.authorized_stop = Some(stop_digest(
                &after.preservation,
                crate::repository::authoring::conflict::StopKind::Conflict,
                &step.footprint,
                &step.index,
            )?);
            self.publish_decision(repo, after, transition)?;
            let authorization = self.child_authorization()?;
            crate::repository::authoring::conflict::Session::prepare_owned(
                repo,
                &authorization,
                &self.state.preservation,
                crate::repository::authoring::conflict::StopKind::Conflict,
                &step.footprint,
                &step.index,
            )?;
            return Ok(Advance::Conflict);
        }
        if step.newly_empty && self.state.phase != "empty" {
            let (mut after, transition) =
                self.checkout_transition(repo, step.tree, &step.index, self.tip()?, super::transition::Kind::Stop)?;
            after.phase = "empty".into();
            after.stop_tree = Some(step.tree.to_string());
            self.retain_stop(repo, &mut after, step.tree)?;
            self.publish_decision(repo, after, transition)?;
            return Ok(Advance::Empty);
        }
        if self.needs_message()? {
            let mut after = self.state.clone();
            self.retain_stop(repo, &mut after, step.tree)?;
            after.stop_tree = Some(step.tree.to_string());
            after.phase = "message".into();
            self.state = after;
            self.save()?;
            drop(_guard);
            self.edit_message(repo)?;
            return self.advance(repo);
        }
        let path = self.directory.join(format!("proposal-{}.json", self.state.step));
        match fs::symlink_metadata(&path) {
            Ok(metadata) => ensure!(
                metadata.is_file()
                    && metadata.permissions().mode().trailing_zeros() >= 6
                    && metadata.uid() == fs::metadata(&self.directory)?.uid(),
                "unsafe saved replay proposal"
            ),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                let proposal = self.propose_interactive(repo, &step)?;
                let payload = serde_json::to_vec(&(proposal.commit.to_string(), proposal.bytes))?;
                let bytes = serde_json::to_vec(&Envelope {
                    checksum: switch::digest(&payload)?,
                    payload,
                })?;
                switch::private_write(&path, &bytes)?;
            }
            Err(error) => return Err(error.into()),
        }
        // A visible proposal can precede the state rename after interruption.
        // Preserve its author/committer bytes and establish durability again.
        switch::sync_path(&path)?;
        switch::sync_dir(&self.directory)?;
        self.state.phase = "step-proposed".into();
        self.state.stop_tree = Some(step.tree.to_string());
        self.save()?;
        self.finish_proposal(repo)
    }

    /// Inspect the supervised pre-hook after interruption without relaunching.
    pub fn recover_pre_hook(&mut self, repo: &gix::Repository) -> Result<()> {
        let _guard = switch::shared_guard(repo)?;
        self.validate_current(repo)?;
        ensure!(
            matches!(self.state.phase.as_str(), "prepared" | "invoking-pre"),
            "no interrupted pre-rebase hook"
        );
        let outcome = crate::repository::authoring::invocation::inspect(&self.directory.join("pre-rebase"))?;
        self.state.phase = match outcome {
            crate::repository::authoring::invocation::InvocationOutcome::Succeeded => "pre-approved",
            crate::repository::authoring::invocation::InvocationOutcome::Failed => "pre-rejected",
            crate::repository::authoring::invocation::InvocationOutcome::Unknown => "pre-unknown",
        }
        .into();
        self.save()
    }

    /// Resume only the pinned child preparation at a durable conflict stop.
    pub fn recover_stop(&mut self, repo: &gix::Repository) -> Result<()> {
        let _guard = switch::shared_guard(repo)?;
        self.validate_current(repo)?;
        let authorization = self.child_authorization()?;
        if repo.git_dir().join("brit-authoring").exists() {
            crate::repository::authoring::conflict::Session::open_owned(repo, &authorization)?;
            return Ok(());
        }
        if let Some(edit) = self
            .state
            .interactive
            .as_ref()
            .and_then(|interactive| interactive.edit.as_ref())
        {
            let private = repo.index_from_tree(&repo.tree_id_of_commit(self.tip()?)?)?;
            let active = edit.active.iter().cloned().map(Into::into).collect::<Vec<BString>>();
            crate::repository::authoring::conflict::Session::prepare_owned(
                repo,
                &authorization,
                &self.state.preservation,
                crate::repository::authoring::conflict::StopKind::Edit,
                &active,
                &private,
            )?;
            return Ok(());
        }
        let step = Step::merge(repo, &self.current_candidate()?, self.tip()?)?;
        ensure!(
            self.state.stop_tree.as_deref() == Some(step.tree.to_string().as_str()),
            "recovered conflict tree changed"
        );
        crate::repository::authoring::conflict::Session::prepare_owned(
            repo,
            &authorization,
            &self.state.preservation,
            crate::repository::authoring::conflict::StopKind::Conflict,
            &step.footprint,
            &step.index,
        )?;
        Ok(())
    }

    fn accept_discard_child(&mut self, repo: &gix::Repository, aborting: bool) -> Result<()> {
        let authorization = self.child_authorization()?;
        let child = crate::repository::authoring::conflict::Session::open_owned(repo, &authorization)?;
        let retained = child.owned_retention(repo, &authorization, aborting)?;
        ensure!(
            aborting || (retained.pending_hook.is_none() && !retained.queued_post_hooks),
            "resolve or acknowledge the child hook before skipping; unknown pre-hook permits only abort"
        );
        self.adopt_child_retention(repo, &retained.binding, retained.decisions)?;
        self.adopt_child_work(repo, &retained.work, retained.head)?;
        self.state.accepted_child = Some((retained.binding, retained.receipt));
        let mut closure = BTreeSet::new();
        history(repo, retained.head, &mut closure)?;
        for id in &retained.objects {
            match super::validation::verify_object(repo, *id)? {
                gix::objs::Kind::Commit => {
                    history(repo, *id, &mut closure)?;
                }
                gix::objs::Kind::Tree => {
                    validate_tree(repo, *id, &mut closure)?;
                }
                gix::objs::Kind::Blob => {
                    closure.insert(*id);
                }
                _ => anyhow::bail!("unsupported object in authored retention receipt"),
            }
        }
        sync_tree(&repo.common_dir().join("objects"))?;
        self.state.closure.extend(closure.iter().map(ToString::to_string));
        self.state.closure.sort();
        self.state.closure.dedup();
        self.capture_after(repo)?;
        Ok(())
    }

    fn archive_accepted(&self, repo: &gix::Repository) -> Result<()> {
        if let Some((binding, receipt)) = &self.state.accepted_child {
            crate::repository::authoring::conflict::Session::archive_owned(repo, binding, receipt)?;
        }
        Ok(())
    }

    /// Recover a child's already durable authoring decision without accepting
    /// its completion or advancing replay.
    pub fn recover_child(&mut self, repo: &gix::Repository) -> Result<()> {
        let _guard = switch::shared_guard(repo)?;
        let authorization = self.child_authorization()?;
        crate::repository::authoring::conflict::Session::recover_owned(repo, &authorization)?;
        Ok(())
    }

    /// Explicitly stage literal paths within the owned conflict stop.
    pub fn stage(&mut self, repo: &gix::Repository, paths: &[BString], expand: bool) -> Result<()> {
        // The staged bytes are kept in this journal: reserve for them first.
        let root = repo.workdir().context("worktree required")?;
        let mut requested = RECORD_SLACK;
        for path in paths {
            if let Ok(metadata) = fs::symlink_metadata(root.join(gix::path::from_bstr(path.as_bstr())?.as_ref())) {
                requested = requested.saturating_add(metadata.len());
            }
        }
        self.reserve(repo, requested)?;
        let authorization = self.child_authorization()?;
        let mut child = crate::repository::authoring::conflict::Session::open_owned(repo, &authorization)?;
        child.stage(repo, paths, expand)
    }

    /// Import only validated external staging, then accept the exact child
    /// receipt before replacing its live journal with an owned archive.
    pub fn continue_conflict(&mut self, repo: &gix::Repository) -> Result<Advance> {
        // The resolution and the step it completes, before anything is written.
        self.reserve(repo, self.step_need(repo, None, true, &[])?)?;
        let authorization = self.child_authorization()?;
        let mut child = crate::repository::authoring::conflict::Session::open_owned(repo, &authorization)?;
        child.import_external(repo)?;
        let _guard = switch::shared_guard(repo)?;
        self.validate_current(repo)?;
        let completion = child.owned_completion(repo, &authorization)?;
        ensure!(completion.head == self.tip()?, "conflict authoring moved replay HEAD");
        let tree = child.private_tree(repo)?;
        let step = Step {
            original: self.current_candidate()?.commit,
            parent: self.tip()?,
            tree,
            index: gix::index::File::from_state(completion.private, repo.index_path()),
            conflicts: Vec::new(),
            footprint: completion.active,
            initially_empty: false,
            newly_empty: false,
        };
        if self.needs_message()? {
            self.state.index = self.capture_payload(repo, &repo.index_path())?;
            self.capture_after(repo)?;
            self.state.accepted_child = Some((completion.binding, completion.receipt));
            let mut closure = BTreeSet::new();
            validate_tree(repo, tree, &mut closure)?;
            let label = format!("message-{}", self.state.step);
            self.protect(repo, &label, tree, &closure)?;
            self.state.retained.push((label, tree.to_string()));
            self.state.closure.extend(closure.iter().map(ToString::to_string));
            self.state.closure.sort();
            self.state.closure.dedup();
            self.state.stop_tree = Some(tree.to_string());
            self.state.phase = "message".into();
            self.save()?;
            self.archive_accepted(repo)?;
            drop(_guard);
            self.edit_message(repo)?;
            return self.advance(repo);
        }
        let path = self.directory.join(format!("proposal-{}.json", self.state.step));
        if !path.exists() {
            let proposal = self.propose_interactive(repo, &step)?;
            let payload = serde_json::to_vec(&(proposal.commit.to_string(), proposal.bytes))?;
            switch::private_write(
                &path,
                &serde_json::to_vec(&Envelope {
                    checksum: switch::digest(&payload)?,
                    payload,
                })?,
            )?;
        }
        switch::sync_path(&path)?;
        switch::sync_dir(&self.directory)?;
        self.state.index = self.capture_payload(repo, &repo.index_path())?;
        self.capture_after(repo)?;
        self.state.accepted_child = Some((completion.binding, completion.receipt));
        self.state.phase = "step-proposed".into();
        self.state.stop_tree = Some(tree.to_string());
        self.save()?;
        self.finish_proposal(repo)
    }

    /// Publish the final tip only after callers explicitly leave READY_FINAL.
    /// Post-rewrite is supervised separately from the irrevocable final decision.
    pub fn finish(&mut self, repo: &gix::Repository) -> Result<()> {
        {
            let _guard = switch::shared_guard(repo)?;
            self.validate_current(repo)?;
            ensure!(
                self.state.phase == "ready-final" || self.state.phase == "final-published",
                "replay has not reached final publication"
            );
            if self.state.phase == "ready-final" {
                self.validate_final_metadata(repo)?;
                let mut after = self.state.clone();
                after.phase = "final-published".into();
                let mut metadata = Vec::new();
                if let Some(name) = self.state.attachment.clone() {
                    let branch = repo.common_dir().join(gix::path::from_bstr(name.as_bstr())?.as_ref());
                    metadata.push(switch::Change {
                        path: gix::path::into_bstr(branch.clone())?.to_vec(),
                        before: switch::snapshot(&branch)?,
                        after: switch::State::File {
                            mode: 0o644,
                            bytes: format!("{}\n", self.state.tip).into_bytes(),
                        },
                    });
                    after.head = self.state.original_head_state.clone();
                    metadata.push(switch::Change {
                        path: gix::path::into_bstr(repo.git_dir().join("HEAD"))?.to_vec(),
                        before: self.state.head.clone(),
                        after: after.head.clone(),
                    });
                    if repo.config_snapshot().try_boolean("core.logAllRefUpdates")? != Some(false) {
                        let path = repo
                            .common_dir()
                            .join("logs")
                            .join(gix::path::from_bstr(name.as_bstr())?.as_ref());
                        let before = self
                            .state
                            .original_branch_log
                            .clone()
                            .context("original branch log was not pinned")?;
                        let line = gix::refs::log::Line {
                            previous_oid: gix::ObjectId::from_hex(self.state.original.as_bytes())?,
                            new_oid: self.tip()?,
                            signature: repo
                                .committer()
                                .context("committer required for final reflog")??
                                .to_owned()?,
                            message: format!("brit rebase {} finish", self.state.nonce).into(),
                        };
                        let desired = self.append_metadata(repo, before.clone(), line)?;
                        after.published_branch_log = Some(desired.clone());
                        metadata.push(switch::Change {
                            path: gix::path::into_bstr(path)?.to_vec(),
                            before,
                            after: desired,
                        });
                    }
                }
                if repo.config_snapshot().try_boolean("core.logAllRefUpdates")? != Some(false) {
                    let line = gix::refs::log::Line {
                        previous_oid: self.tip()?,
                        new_oid: self.tip()?,
                        signature: repo
                            .committer()
                            .context("committer required for final reflog")??
                            .to_owned()?,
                        message: format!("brit rebase {} finish", self.state.nonce).into(),
                    };
                    after.log = self.append_metadata(repo, self.state.log.clone(), line)?;
                    metadata.push(switch::Change {
                        path: gix::path::into_bstr(repo.git_dir().join("logs/HEAD"))?.to_vec(),
                        before: self.state.log.clone(),
                        after: after.log.clone(),
                    });
                }
                let path = repo.git_dir().join("ORIG_HEAD");
                metadata.push(switch::Change {
                    path: gix::path::into_bstr(path.clone())?.to_vec(),
                    before: self.state.original_orig_head.clone(),
                    after: switch::State::File {
                        mode: 0o644,
                        bytes: format!("{}\n", self.state.original).into_bytes(),
                    },
                });
                let transition = super::transition::Transition {
                    kind: super::transition::Kind::Final,
                    work: self
                        .state
                        .work
                        .iter()
                        .map(|change| switch::Change {
                            path: change.path.clone(),
                            before: change.after.clone(),
                            after: change.after.clone(),
                        })
                        .collect(),
                    metadata,
                    branch: self
                        .state
                        .attachment
                        .clone()
                        .map(|name| (name, self.state.original.clone())),
                    final_tip: Some(self.state.tip.clone()),
                    packed_refs: switch::payload::fingerprint(&repo.common_dir().join("packed-refs"))?,
                };
                self.publish_decision(repo, after, transition)?;
            }
            self.validate_published_resources(repo)?;
        }
        // The final decision is durable: the rest finishes what nothing can undo,
        // so it is never refused by the quota.
        let _recovery = switch::payload::Recovery::enter();
        let mut mapping = Vec::new();
        if let Some(interactive) = &self.state.interactive {
            for (old, new) in interactive.ledger.edges() {
                mapping.extend_from_slice(format!("{old} {new}\n").as_bytes());
            }
        } else {
            for (old, _) in &self.state.candidates {
                if let Some((_, results)) = self.state.mappings.iter().find(|(mapped, _)| mapped == old) {
                    for new in results {
                        mapping.extend_from_slice(format!("{old} {new}\n").as_bytes());
                    }
                }
            }
        }
        let invocation = self.directory.join("post-rewrite");
        let index = self.directory.join("index.post-rewrite");
        let private = repo.index_from_tree(&repo.tree_id_of_commit(self.tip()?)?)?;
        let mut bytes = Vec::new();
        private.write_to(&mut bytes, Default::default())?;
        match fs::symlink_metadata(&index) {
            Ok(metadata) => {
                ensure!(
                    metadata.is_file()
                        && metadata.uid() == fs::metadata(&self.directory)?.uid()
                        && metadata.permissions().mode().trailing_zeros() >= 6
                        && fs::read(&index)? == bytes,
                    "post-rewrite private index changed"
                );
                switch::sync_path(&index)?;
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => switch::private_write(&index, &bytes)?,
            Err(error) => return Err(error.into()),
        }
        switch::sync_dir(&self.directory)?;
        let outcome = super::hooks::run(
            repo,
            &invocation,
            "post-rewrite",
            &["rebase".into()],
            Some(&mapping),
            &index,
            || {
                let _guard = switch::shared_guard(repo)?;
                self.validate_current(repo)?;
                self.validate_published_resources(repo)?;
                self.state.phase = "invoking-post".into();
                self.save()
            },
        )?;
        let _guard = switch::shared_guard(repo)?;
        let validation = self
            .validate_current(repo)
            .and_then(|()| self.validate_published_resources(repo));
        self.state.post_outcome = Some(outcome);
        if let Err(error) = validation {
            self.state.phase = "published-foreign-state".into();
            self.save()?;
            anyhow::bail!("rebase committed; post-rewrite left foreign state; inspect retained journal: {error:#}");
        }

        self.state.phase = if outcome == crate::repository::authoring::invocation::InvocationOutcome::Unknown {
            "post-unknown"
        } else {
            "completed"
        }
        .into();
        self.save()?;
        ensure!(
            outcome != crate::repository::authoring::invocation::InvocationOutcome::Unknown,
            "rebase committed; post-rewrite outcome unknown and requires acknowledgement without rerun"
        );
        ensure!(outcome.success(), "rebase committed; post-rewrite hook failed");
        Ok(())
    }

    fn validate_published_resources(&self, repo: &gix::Repository) -> Result<()> {
        ensure!(
            super::head_commit_id(repo)? == self.tip()?,
            "published branch or detached HEAD moved from final tip"
        );
        ensure!(
            switch::snapshot(&repo.git_dir().join("HEAD"))? == self.state.head
                && switch::payload::fingerprint(&repo.index_path())? == self.state.index
                && switch::payload::fingerprint(&repo.git_dir().join("logs/HEAD"))? == self.state.log,
            "published replay metadata changed"
        );
        let orig_head = switch::State::File {
            mode: 0o644,
            bytes: format!("{}\n", self.state.original).into_bytes(),
        };
        ensure!(
            switch::snapshot(&repo.git_dir().join("ORIG_HEAD"))? == orig_head,
            "published ORIG_HEAD changed"
        );
        if let (Some(name), Some(expected)) = (&self.state.attachment, &self.state.published_branch_log) {
            ensure!(
                switch::payload::fingerprint(
                    &repo
                        .common_dir()
                        .join("logs")
                        .join(gix::path::from_bstr(name.as_bstr())?.as_ref())
                )? == *expected,
                "published original branch reflog changed"
            );
        }
        let root = repo.workdir().context("worktree required")?;
        for change in &self.state.work {
            ensure!(
                switch::payload::fingerprint(&root.join(gix::path::from_bstr(change.path.as_bstr())?.as_ref()))?
                    == change.after,
                "published replay worktree changed"
            );
        }
        Ok(())
    }

    /// Acknowledge an unknown post-rewrite result without launching it again.
    pub fn acknowledge_post_hook(&mut self, repo: &gix::Repository) -> Result<()> {
        let _guard = switch::shared_guard(repo)?;
        // Published already: acknowledging it is never refused by the quota.
        let _recovery = switch::payload::Recovery::enter();
        ensure!(
            matches!(
                self.state.phase.as_str(),
                "final-published" | "invoking-post" | "post-unknown"
            ),
            "no unknown post-rewrite invocation"
        );
        ensure!(
            serde_json::to_vec(&Self::open(repo)?.state)? == serde_json::to_vec(&self.state)?,
            "stale post-rewrite session"
        );
        let outcome = crate::repository::authoring::invocation::inspect(&self.directory.join("post-rewrite"))?;
        let validation = self
            .validate_current(repo)
            .and_then(|()| self.validate_published_resources(repo));
        self.state.post_outcome = Some(outcome);
        if let Err(error) = validation {
            self.state.phase = "published-foreign-state".into();
            self.save()?;
            anyhow::bail!("rebase committed; post-rewrite left foreign state; inspect retained journal: {error:#}");
        }
        self.state.phase = "completed".into();
        self.save()
    }

    fn protect(
        &self,
        repo: &gix::Repository,
        label: &str,
        id: gix::ObjectId,
        closure: &BTreeSet<gix::ObjectId>,
    ) -> Result<()> {
        let path = self.directory.join(format!("retention-{label}.json"));
        let payload = serde_json::to_vec(&(
            self.state.nonce.as_str(),
            label,
            id.to_string(),
            closure.iter().map(ToString::to_string).collect::<Vec<_>>(),
        ))?;
        let bytes = serde_json::to_vec(&Envelope {
            checksum: switch::digest(&payload)?,
            payload,
        })?;
        if path.exists() {
            ensure!(fs::read(&path)? == bytes, "foreign intermediate retention recipe");
            switch::sync_path(&path)?;
        } else {
            switch::private_write(&path, &bytes)?;
        }
        switch::sync_dir(&self.directory)?;
        switch::checkpoint("replay-retention-recipe")?;
        let name = format!("refs/brit-rebase-retain/{}/{label}", self.state.nonce);
        if let Some(reference) = repo.try_find_reference(name.as_str())? {
            ensure!(
                matches!(reference.target(), gix::refs::TargetRef::Object(_)) && reference.id() == id,
                "foreign intermediate retention reference"
            );
        } else {
            repo.reference(name, id, gix::refs::transaction::PreviousValue::MustNotExist, "")?;
        }
        sync_reference_storage(repo)?;
        switch::checkpoint("replay-intermediate-retained")?;
        Ok(())
    }

    fn adopt_abandoned_plan(&mut self, repo: &gix::Repository) -> Result<()> {
        let before = switch::digest(&serde_json::to_vec(&self.state)?)?;
        let ordinal = self.state.decisions + 1;
        let name = format!("transition-{ordinal}-{before}");
        let directory = self.directory.join(&name);
        match fs::symlink_metadata(&directory) {
            Ok(metadata) => {
                ensure!(
                    metadata.is_dir()
                        && metadata.uid() == fs::metadata(&self.directory)?.uid()
                        && metadata.permissions().mode().trailing_zeros() >= 6,
                    "foreign proposed transition directory"
                );
                // A private preparation directory becomes visible before its
                // ownership record, which precedes its plan; each is written in one
                // pass, so a kill can leave either absent or cut short. No live
                // write precedes the decision, so abort restores the pinned parent
                // without adopting such an orphan. The retained copy preserves its
                // bytes; cleanup's exact inventory deliberately continues to refuse
                // the unproved directory, which `--retire` then sets aside.
                let uid = fs::metadata(&self.directory)?.uid();
                let read = |member: &str| -> Result<Option<Vec<u8>>> {
                    let path = directory.join(member);
                    if !present(&path)? {
                        return Ok(None);
                    }
                    ensure!(owned_private_file(&path, uid)?, "foreign proposed transition member");
                    Ok(Some(fs::read(path)?))
                };
                let expected = serde_json::to_vec(&(self.state.nonce.as_str(), ordinal, before.as_str()))?;
                let owner = read("owner")?;
                ensure!(
                    owner.as_ref().is_none_or(|owner| expected.starts_with(owner)),
                    "foreign proposed transition owner"
                );
                let plan = read("plan.json")?;
                let complete = match &plan {
                    Some(bytes) => torn_record(bytes)?,
                    None => None,
                };
                let (Some(envelope), Some(bytes), true) = (complete, plan, owner == Some(expected)) else {
                    // Not proven (an owner record absent or cut short, a plan
                    // absent or cut short): not adopted and kept as evidence, as
                    // cleanup's exact inventory then reports; nothing followed it.
                    return Ok(());
                };
                ensure!(
                    switch::digest(&envelope.payload)? == envelope.checksum,
                    "corrupt abandoned plan"
                );
                let plan: Decision = serde_json::from_slice(&envelope.payload)?;
                ensure!(
                    plan.before == before && plan.ordinal == ordinal && plan.nonce == self.state.nonce,
                    "abandoned plan belongs to another parent state"
                );
                // A command killed after this plan took its locks and before its
                // decision record (or with that record cut short) left them held;
                // the plan is never decided now, so its own locks are released.
                plan.transition.release_undecided_locks(repo, &directory)?;
                self.state.abandoned_plans.push((name, switch::digest(&bytes)?));
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
        Ok(())
    }

    fn adopt_retention(&mut self, repo: &gix::Repository) -> Result<()> {
        for step in 0..self.state.candidates.len() {
            for label in [
                step.to_string(),
                format!("stop-{step}"),
                format!("message-{step}"),
                format!("edit-{step}"),
            ] {
                self.adopt_retention_label(repo, &label)?;
            }
        }
        Ok(())
    }

    fn adopt_child_retention(
        &mut self,
        repo: &gix::Repository,
        binding: &ParentBinding,
        decisions: usize,
    ) -> Result<()> {
        ensure!(binding.nonce == self.state.nonce, "foreign child retention namespace");
        for ordinal in 1..=decisions.checked_add(1).context("child decision count overflow")? {
            self.adopt_retention_label(repo, &format!("child-{}-{ordinal}", binding.step))?;
        }
        Ok(())
    }

    fn adopt_retention_label(&mut self, repo: &gix::Repository, label: &str) -> Result<()> {
        let path = self.directory.join(format!("retention-{label}.json"));
        if !path.exists() {
            return Ok(());
        }
        let envelope: Envelope = serde_json::from_slice(&fs::read(&path)?)?;
        ensure!(
            switch::digest(&envelope.payload)? == envelope.checksum,
            "corrupt retention recipe"
        );
        let (nonce, saved, value, values): (String, String, String, Vec<String>) =
            serde_json::from_slice(&envelope.payload)?;
        ensure!(nonce == self.state.nonce && saved == label, "foreign retention recipe");
        let id = gix::ObjectId::from_hex(value.as_bytes())?;
        let mut closure = BTreeSet::new();
        match repo.find_header(id)?.kind() {
            gix::objs::Kind::Commit => {
                history(repo, id, &mut closure)?;
            }
            gix::objs::Kind::Tree => {
                validate_tree(repo, id, &mut closure)?;
            }
            _ => anyhow::bail!("invalid retained root kind"),
        }
        ensure!(
            closure.iter().map(ToString::to_string).collect::<Vec<_>>() == values,
            "retention closure changed"
        );
        sync_tree(&repo.common_dir().join("objects"))?;
        self.protect(repo, label, id, &closure)?;
        if let Some((_, expected)) = self.state.retained.iter().find(|(name, _)| name == label) {
            ensure!(expected == &value, "retention ledger changed");
        } else {
            self.state.retained.push((label.to_owned(), value));
        }
        self.state.closure.extend(values);
        self.state.closure.sort();
        self.state.closure.dedup();
        Ok(())
    }

    fn retain_stop(&self, repo: &gix::Repository, after: &mut ParentState, tree: gix::ObjectId) -> Result<()> {
        let mut closure = BTreeSet::new();
        validate_tree(repo, tree, &mut closure)?;
        sync_tree(&repo.common_dir().join("objects"))?;
        let label = format!("stop-{}", self.state.step);
        self.protect(repo, &label, tree, &closure)?;
        after.retained.push((label, tree.to_string()));
        after.closure.extend(closure.iter().map(ToString::to_string));
        after.closure.sort();
        after.closure.dedup();
        Ok(())
    }

    fn child_authorization(&self) -> Result<ChildAuthorization> {
        ensure!(
            self.state.phase == "authoring" && self.state.authorized_stop.is_some(),
            "parent is not at an authoring stop"
        );
        Ok(ChildAuthorization {
            binding: ParentBinding {
                nonce: self.state.nonce.clone(),
                step: self.state.decisions,
                parent_state: child_state_digest(&self.state)?,
            },
        })
    }

    fn grow_scope(&mut self, repo: &gix::Repository, paths: &[BString]) -> Result<()> {
        let mut active = self.state.work.iter().map(|c| c.path.clone()).collect::<Vec<_>>();
        active.extend(paths.iter().map(|p| p.to_vec()));
        active.sort();
        active.dedup();
        self.state.preservation.validate_scope(&active)?;
        let root = repo.workdir().context("worktree required")?;
        let original =
            repo.index_from_tree(&repo.tree_id_of_commit(gix::ObjectId::from_hex(self.state.original.as_bytes())?)?)?;
        for path in active
            .iter()
            .filter(|p| !self.state.work.iter().any(|c| c.path == **p))
            .cloned()
            .collect::<Vec<_>>()
        {
            validate_scope_ancestors(root, &path, &original)?;
            let image = self.capture_payload(repo, &root.join(gix::path::from_bstr(path.as_bstr())?.as_ref()))?;
            ensure!(
                original.entry_by_path(path.as_bstr()).is_some() || image == switch::State::Absent,
                "merge expansion collides with untracked or ignored content"
            );
            self.state.work.push(switch::Change {
                path,
                before: image.clone(),
                after: image,
            });
        }
        self.state.witness = crate::repository::authoring::conflict::worktree_witness(repo, &active)?;
        self.save()
    }

    fn finish_proposal(&mut self, repo: &gix::Repository) -> Result<Advance> {
        use gix::prelude::Write;
        if let Some((binding, receipt)) = &self.state.accepted_child {
            crate::repository::authoring::conflict::Session::archive_owned(repo, binding, receipt)?;
        }
        let path = self.directory.join(format!("proposal-{}.json", self.state.step));
        let envelope: Envelope = serde_json::from_slice(&fs::read(path)?)?;
        ensure!(
            switch::digest(&envelope.payload)? == envelope.checksum,
            "corrupt replay proposal"
        );
        let (value, bytes): (String, Vec<u8>) = serde_json::from_slice(&envelope.payload)?;
        let commit = gix::ObjectId::from_hex(value.as_bytes())?;
        ensure!(
            gix::objs::compute_hash(repo.object_hash(), gix::objs::Kind::Commit, &bytes)? == commit,
            "replay proposal hash mismatch"
        );
        let decoded = gix::objs::CommitRef::from_bytes(&bytes, repo.object_hash())?;
        ensure!(
            decoded.parents().collect::<Vec<_>>() == [self.proposed_parent()?],
            "replay proposal parent changed"
        );
        ensure!(
            Some(decoded.tree().to_string()) == self.state.stop_tree,
            "replay proposal tree changed"
        );
        ensure!(
            repo.write_buf(gix::objs::Kind::Commit, &bytes)? == commit,
            "replay object write changed identity"
        );
        let mut closure = BTreeSet::new();
        history(repo, commit, &mut closure)?;
        sync_tree(&repo.common_dir().join("objects"))?;
        self.protect(repo, &self.state.step.to_string(), commit, &closure)?;
        let tree = decoded.tree();
        let private = repo.index_from_tree(&tree)?;
        let (mut after, transition) =
            self.checkout_transition(repo, tree, &private, commit, super::transition::Kind::Step)?;
        after.phase = "replaying".into();
        after.retained.push((self.state.step.to_string(), commit.to_string()));
        self.record_result(repo, &mut after, commit)?;
        after.step += 1;
        after.stop_tree = None;
        if let Some(receipt) = after.accepted_child.take() {
            after.accepted_children.push(receipt);
        }
        after.authorized_stop = None;
        let edit = self.current_command()? == Command::Edit;
        let mut active = BTreeSet::new();
        if edit {
            let parent = repo.index_from_tree(&repo.tree_id_of_commit(self.tip()?)?)?;
            changed_index_paths(&parent, &private, &mut active)?;
            let active = active.iter().cloned().collect::<Vec<_>>();
            after.authorized_stop = Some(stop_digest(
                &after.preservation,
                crate::repository::authoring::conflict::StopKind::Edit,
                &active,
                &private,
            )?);
            after.phase = "authoring".into();
            after
                .interactive
                .as_mut()
                .context("edit requires interactive todo")?
                .edit = Some(interactive::Edit {
                original: self.current_candidate()?.commit.to_string(),
                parent: self.state.tip.clone(),
                active: active.iter().map(|path| path.to_vec()).collect(),
                previous_group: self
                    .state
                    .interactive
                    .as_ref()
                    .and_then(|interactive| interactive.group.clone()),
            });
        }
        after.closure.extend(closure.iter().map(ToString::to_string));
        after.closure.sort();
        after.closure.dedup();
        self.publish_decision(repo, after, transition)?;
        if edit {
            crate::repository::authoring::conflict::Session::prepare_owned(
                repo,
                &self.child_authorization()?,
                &self.state.preservation,
                crate::repository::authoring::conflict::StopKind::Edit,
                &active.into_iter().collect::<Vec<_>>(),
                &private,
            )?;
            Ok(Advance::Edit)
        } else {
            Ok(Advance::Step { commit })
        }
    }

    /// Skip only the current input, with durable intent and retained bytes
    /// before restoring its entry tree.
    pub fn skip(&mut self, repo: &gix::Repository) -> Result<super::retained::Receipt> {
        let _guard = switch::shared_guard(repo)?;
        self.validate_current(repo)?;
        if self.state.phase == "break" {
            // The retained copy, before anything of the skip is written.
            self.reserve_with_copy(repo, self.step_need(repo, None, false, &[])?)?;
            self.state.copies += 1;
            self.save()?;
            let receipt = self.capture_current(repo)?;
            self.state.phase = "replaying".into();
            self.save()?;
            return Ok(receipt);
        }
        let edit = self
            .state
            .interactive
            .as_ref()
            .and_then(|interactive| interactive.edit.clone());
        ensure!(
            matches!(self.state.phase.as_str(), "empty" | "authoring" | "retaining-skip"),
            "skip requires a current replay input"
        );
        ensure!(
            self.state.step < self.state.candidates.len() || edit.is_some(),
            "no input remains to skip"
        );
        if self.state.phase != "retaining-skip" {
            // The retained copy and the restoring checkout, before either is written.
            let target = edit
                .as_ref()
                .map(|edit| gix::ObjectId::from_hex(edit.parent.as_bytes()))
                .transpose()?
                .unwrap_or(self.tip()?);
            let desired = repo.index_from_tree(&repo.tree_id_of_commit(target)?)?;
            self.reserve_with_copy(repo, self.step_need(repo, Some(&desired), true, &[])?)?;
        }
        if self.state.phase != "retaining-skip" {
            self.adopt_abandoned_plan(repo)?;
        }
        if self.state.phase == "authoring" {
            self.accept_discard_child(repo, false)?;
        }
        self.adopt_retention(repo)?;
        if self.state.phase != "retaining-skip" {
            self.state.phase = "retaining-skip".into();
            self.state.copies += 1;
            self.save()?;
        }
        self.archive_accepted(repo)?;
        let receipt = self.capture_current(repo)?;
        switch::checkpoint("replay-skip-copy-complete")?;
        let target = edit
            .as_ref()
            .map(|edit| gix::ObjectId::from_hex(edit.parent.as_bytes()))
            .transpose()?
            .unwrap_or(self.tip()?);
        let tree = repo.tree_id_of_commit(target)?.detach();
        let private = repo.index_from_tree(&tree)?;
        let (mut after, transition) =
            self.checkout_transition(repo, tree, &private, target, super::transition::Kind::Skip)?;
        if let Some(edit) = edit {
            let mapping = after
                .mappings
                .iter_mut()
                .find(|(old, _)| old == &edit.original)
                .context("edit mapping missing")?;
            mapping.1.clear();
            let interactive = after.interactive.as_mut().context("interactive state missing")?;
            interactive.ledger.replace_group(&[edit.original], &[])?;
            interactive.edit = None;
            interactive.group = edit.previous_group;
        } else {
            after
                .mappings
                .push((self.current_candidate()?.commit.to_string(), Vec::new()));
            if let Some(interactive) = &mut after.interactive {
                interactive
                    .ledger
                    .record(&self.current_candidate()?.commit.to_string(), &[])?;
                interactive.cursor += 1;
                interactive.message = None;
            }
            after.step += 1;
        }
        after.phase = "replaying".into();
        after.stop_tree = None;
        if let Some(receipt) = after.accepted_child.take() {
            after.accepted_children.push(receipt);
        }
        after.authorized_stop = None;
        self.publish_decision(repo, after, transition)?;
        Ok(receipt)
    }

    /// Retain every operation-scoped byte before restoring an unpublished
    /// replay. A final decision never enters this path.
    pub fn abort(&mut self, repo: &gix::Repository) -> Result<super::retained::Receipt> {
        let _guard = switch::shared_guard(repo)?;
        // Undoing is never refused by the quota, whatever the journal holds.
        let _recovery = switch::payload::Recovery::enter();
        let current = Self::open(repo)?;
        ensure!(
            serde_json::to_vec(&current.state)? == serde_json::to_vec(&self.state)?,
            "stale replay session"
        );
        drop(current);
        if self
            .directory
            .join(format!("decision-{}.json", self.state.decisions + 1))
            .exists()
        {
            *self = Self::recover_decision_guarded(repo, true)?;
            if self.state.phase == "aborted" {
                let copy = repo
                    .git_dir()
                    .join("brit-rebase-retained")
                    .join(&self.state.nonce)
                    .join(self.state.copies.to_string());
                return Ok(super::retained::inspect(&copy)?.receipt);
            }
        }
        ensure!(
            matches!(
                self.state.phase.as_str(),
                "prepared"
                    | "pre-approved"
                    | "pre-rejected"
                    | "pre-unknown"
                    | "invoking-pre"
                    | "step-proposed"
                    | "edit-accepted"
                    | "replaying"
                    | "ready-final"
                    | "break"
                    | "message"
                    | "message-ready"
                    | "empty"
                    | "authoring"
                    | "retaining-abort"
                    | "retaining-skip"
            ),
            "this replay phase cannot be aborted without resolving its pending decision or invocation"
        );
        ensure!(
            !self
                .directory
                .join(format!("decision-{}.json", self.state.decisions + 1))
                .exists(),
            "a durable replay decision must be recovered before abort"
        );
        self.inspect_editor()?;
        if matches!(self.state.phase.as_str(), "invoking-pre" | "pre-unknown") {
            crate::repository::authoring::invocation::inspect(&self.directory.join("pre-rebase"))?;
        }
        let retry_preparation = self.state.phase == "retaining-abort"
            && self
                .directory
                .join(format!(
                    "transition-{}-{}",
                    self.state.decisions + 1,
                    switch::digest(&serde_json::to_vec(&self.state)?)?
                ))
                .try_exists()?;
        if self.state.phase != "retaining-abort" || retry_preparation {
            self.adopt_abandoned_plan(repo)?;
        }
        if self.state.phase == "authoring" && repo.git_dir().join("brit-authoring").exists() {
            self.accept_discard_child(repo, true)?;
        }
        self.validate_branch(repo, None)?;
        ensure!(
            switch::snapshot(&repo.git_dir().join("HEAD"))? == self.state.head
                && switch::payload::fingerprint(&repo.index_path())? == self.state.index
                && switch::payload::fingerprint(&repo.git_dir().join("logs/HEAD"))? == self.state.log,
            "foreign replay metadata before abort"
        );
        self.state.preservation.validate_observed(repo, &*repo.open_index()?)?;
        let root = repo.workdir().context("worktree required")?;
        let hook_stopped = matches!(
            self.state.phase.as_str(),
            "invoking-pre" | "pre-unknown" | "pre-rejected" | "pre-approved"
        );
        for change in &mut self.state.work {
            let observed =
                switch::payload::fingerprint(&root.join(gix::path::from_bstr(change.path.as_bstr())?.as_ref()))?;
            ensure!(
                hook_stopped || observed == change.after,
                "foreign replay worktree mutation before abort"
            );
            if hook_stopped {
                change.after = observed;
            }
        }
        self.adopt_retention(repo)?;
        if self.state.phase == "retaining-skip" {
            self.archive_accepted(repo)?;
            self.capture_current(repo)?;
        }
        if self.state.phase != "retaining-abort" || retry_preparation {
            self.state.phase = "retaining-abort".into();
            self.state.copies += 1;
            self.save()?;
        }
        self.migrate_inline_work(repo)?;
        self.archive_accepted(repo)?;
        let receipt = self.capture_current(repo)?;
        switch::checkpoint("replay-abort-copy-complete")?;
        let mut after = self.state.clone();
        after.phase = "aborted".into();
        after.tip.clone_from(&self.state.original);
        after.index = self.state.original_index_state.clone();
        after.head = self.state.original_head_state.clone();
        after.log = self.state.original_log.clone();
        let mut work = Vec::new();
        for change in &mut after.work {
            work.push(switch::Change {
                path: change.path.clone(),
                before: change.after.clone(),
                after: change.before.clone(),
            });
            change.after = change.before.clone();
        }
        let mut metadata = Vec::new();
        for (path, before, desired) in [
            (repo.index_path(), &self.state.index, &after.index),
            (repo.git_dir().join("HEAD"), &self.state.head, &after.head),
            (repo.git_dir().join("logs/HEAD"), &self.state.log, &after.log),
        ] {
            if before != desired {
                metadata.push(switch::Change {
                    path: gix::path::into_bstr(path)?.to_vec(),
                    before: before.clone(),
                    after: desired.clone(),
                });
            }
        }
        self.publish_decision(
            repo,
            after,
            super::transition::Transition {
                kind: super::transition::Kind::Abort,
                work,
                metadata,
                branch: self
                    .state
                    .attachment
                    .clone()
                    .map(|name| (name, self.state.original.clone())),
                final_tip: None,
                packed_refs: switch::payload::fingerprint(&repo.common_dir().join("packed-refs"))?,
            },
        )?;
        Ok(receipt)
    }

    /// Retire only a terminal operation, after an independent retained copy
    /// proves every object whose private retention reference is released.
    pub fn cleanup(&mut self, repo: &gix::Repository) -> Result<super::retained::Receipt> {
        let _guard = switch::shared_guard(repo)?;
        // Finishing a terminal journal is never refused by the quota.
        let _recovery = switch::payload::Recovery::enter();
        ensure!(
            matches!(self.state.phase.as_str(), "aborted" | "completed"),
            "replay is not terminal"
        );
        if self.state.phase == "completed" {
            ensure!(
                super::head_commit_id(repo)? == self.tip()?
                    && switch::snapshot(&repo.git_dir().join("HEAD"))? == self.state.head,
                "published branch or HEAD moved before cleanup"
            );
        }
        let saved = Self::open(repo)?;
        ensure!(
            serde_json::to_vec(&saved.state)? == serde_json::to_vec(&self.state)?,
            "stale terminal replay"
        );
        if !self.state.cleanup_copy {
            self.state.cleanup_copy = true;
            self.state.copies += 1;
            self.save()?;
        }
        let allowed = self.cleanup_inventory(repo)?;
        let retained = self.capture_current(repo)?;
        super::cleanup::run(
            repo,
            &self.directory,
            &self.state.nonce,
            &fs::read(self.directory.join("state.json"))?,
            &self.state.retained,
            &allowed,
            &retained,
        )?;
        Ok(retained)
    }

    pub fn recover_cleanup(repo: &gix::Repository, receipt: &Path) -> Result<()> {
        let _guard = switch::shared_guard(repo)?;
        let _recovery = switch::payload::Recovery::enter();
        super::cleanup::resume(repo, receipt)
    }

    fn cleanup_inventory(&self, repo: &gix::Repository) -> Result<BTreeSet<Vec<u8>>> {
        self.validate_payloads()?;
        let mut allowed = BTreeSet::from([b"state.json".to_vec()]);
        for hash in self.child_payload_licenses()?.keys() {
            allowed.insert(format!("payload-license-{hash}.json").into_bytes());
            if fs::symlink_metadata(self.directory.join(format!("payload-{hash}"))).is_ok() {
                allowed.insert(format!("payload-{hash}").into_bytes());
            }
        }
        for hash in self.state.payloads.keys() {
            let name = format!("payload-{hash}");
            if fs::symlink_metadata(self.directory.join(&name)).is_ok() {
                allowed.insert(name.into_bytes());
            }
        }
        if self.directory.join("state.pending").exists() {
            ensure!(
                fs::read(self.directory.join("state.pending"))? == fs::read(self.directory.join("state.json"))?,
                "unaccepted pending terminal state"
            );
            allowed.insert(b"state.pending".to_vec());
        }
        for name in ["index.pre-rebase", "index.post-rewrite"] {
            if self.directory.join(name).exists() {
                allowed.insert(name.as_bytes().to_vec());
            }
        }
        let mut invocations = vec!["pre-rebase".to_owned(), "post-rewrite".to_owned()];
        if let Some(interactive) = &self.state.interactive {
            for editor in &interactive.editors {
                invocations.push(format!("editor-{}", editor.ordinal));
                for name in [
                    format!("editor-{}-output", editor.ordinal),
                    format!("editor-{}-index", editor.ordinal),
                ] {
                    if self.directory.join(&name).exists() {
                        allowed.insert(name.into_bytes());
                    }
                }
            }
        }
        for name in &invocations {
            let directory = self.directory.join(name);
            if directory.exists() {
                crate::repository::authoring::invocation::inspect(&directory)?;
                allowed.insert(name.as_bytes().to_vec());
                for member in fs::read_dir(directory)? {
                    let member = member?;
                    allowed.insert(gix::path::into_bstr(Path::new(name).join(member.file_name()))?.to_vec());
                }
            }
        }
        for step in 0..self.state.candidates.len() {
            let name = format!("proposal-{step}.json");
            let path = self.directory.join(&name);
            if path.exists() {
                let envelope: Envelope = serde_json::from_slice(&fs::read(path)?)?;
                ensure!(
                    switch::digest(&envelope.payload)? == envelope.checksum,
                    "corrupt terminal proposal"
                );
                let (id, bytes): (String, Vec<u8>) = serde_json::from_slice(&envelope.payload)?;
                ensure!(
                    gix::objs::compute_hash(repo.object_hash(), gix::objs::Kind::Commit, &bytes)?
                        == gix::ObjectId::from_hex(id.as_bytes())?,
                    "terminal proposal identity changed"
                );
                allowed.insert(name.into_bytes());
            }
        }
        for (label, id) in &self.state.retained {
            if matches!(label.as_str(), "original" | "upstream") {
                continue;
            }
            let name = format!("retention-{label}.json");
            let envelope: Envelope = serde_json::from_slice(&fs::read(self.directory.join(&name))?)?;
            ensure!(
                switch::digest(&envelope.payload)? == envelope.checksum,
                "corrupt terminal retention recipe"
            );
            let (nonce, saved, value, _): (String, String, String, Vec<String>) =
                serde_json::from_slice(&envelope.payload)?;
            ensure!(
                nonce == self.state.nonce && &saved == label && &value == id,
                "foreign terminal retention recipe"
            );
            allowed.insert(name.into_bytes());
        }
        for ordinal in 1..=self.state.decisions {
            let name = format!("decision-{ordinal}.json");
            let envelope: Envelope = serde_json::from_slice(&fs::read(self.directory.join(&name))?)?;
            ensure!(
                switch::digest(&envelope.payload)? == envelope.checksum,
                "corrupt terminal decision"
            );
            let decision: Decision = serde_json::from_slice(&envelope.payload)?;
            ensure!(
                decision.version == 1
                    && decision.nonce == self.state.nonce
                    && decision.ordinal == ordinal
                    && decision.after.decisions == ordinal,
                "foreign terminal decision"
            );
            allowed.insert(name.into_bytes());
            let name = format!("transition-{ordinal}-{}", decision.before);
            let directory = self.directory.join(&name);
            ensure!(
                fs::read(directory.join("owner"))?
                    == serde_json::to_vec(&(decision.nonce.as_str(), ordinal, decision.before.as_str()))?,
                "foreign terminal transition"
            );
            let plan: Envelope = serde_json::from_slice(&fs::read(directory.join("plan.json"))?)?;
            ensure!(
                switch::digest(&plan.payload)? == plan.checksum,
                "corrupt terminal transition plan"
            );
            let proposed: Decision = serde_json::from_slice(&plan.payload)?;
            ensure!(
                proposed.nonce == decision.nonce
                    && proposed.ordinal == ordinal
                    && proposed.before == decision.before
                    && proposed.after.tip == decision.after.tip
                    && proposed.transition.kind == decision.transition.kind,
                "foreign terminal transition plan"
            );
            allowed.insert(name.as_bytes().to_vec());
            for member in decision.transition.cleanup_inventory(repo, &directory)? {
                allowed.insert(
                    gix::path::into_bstr(Path::new(&name).join(gix::path::from_bstr(member.as_bstr())?.as_ref()))?
                        .to_vec(),
                );
            }
        }
        for (name, checksum) in &self.state.abandoned_plans {
            let directory = self.directory.join(name);
            let bytes = fs::read(directory.join("plan.json"))?;
            ensure!(switch::digest(&bytes)? == *checksum, "abandoned replay plan changed");
            let envelope: Envelope = serde_json::from_slice(&bytes)?;
            let mut plan: Decision = serde_json::from_slice(&envelope.payload)?;
            ensure!(
                *name == format!("transition-{}-{}", plan.ordinal, plan.before)
                    && fs::read(directory.join("owner"))?
                        == serde_json::to_vec(&(plan.nonce.as_str(), plan.ordinal, plan.before.as_str()))?,
                "abandoned replay staging ownership changed"
            );
            for change in &mut plan.transition.work {
                if matches!(change.after, switch::State::Directory { inode: 0, .. }) {
                    let receipt: Envelope = serde_json::from_slice(&fs::read(
                        directory.join(format!("directory-{}.json", switch::digest(&change.path)?)),
                    )?)?;
                    ensure!(
                        switch::digest(&receipt.payload)? == receipt.checksum,
                        "corrupt abandoned directory allocation"
                    );
                    let (path, state): (Vec<u8>, switch::State) = serde_json::from_slice(&receipt.payload)?;
                    ensure!(
                        path == change.path && matches!(state, switch::State::Directory { .. }),
                        "foreign abandoned directory allocation"
                    );
                    change.after = state;
                }
            }
            allowed.insert(name.as_bytes().to_vec());
            for member in plan.transition.cleanup_inventory(repo, &directory)? {
                allowed.insert(
                    gix::path::into_bstr(Path::new(name).join(gix::path::from_bstr(member.as_bstr())?.as_ref()))?
                        .to_vec(),
                );
            }
        }
        for (binding, receipt) in self
            .state
            .accepted_children
            .iter()
            .chain(self.state.accepted_child.iter())
        {
            allowed.extend(crate::repository::authoring::conflict::Session::accepted_inventory(
                repo, binding, receipt,
            )?);
        }
        Ok(allowed)
    }

    fn capture_current(&self, repo: &gix::Repository) -> Result<super::retained::Receipt> {
        let members = self.current_members(repo)?;
        let original = gix::ObjectId::from_hex(self.state.original.as_bytes())?;
        let tip = gix::ObjectId::from_hex(self.state.tip.as_bytes())?;
        super::retained::capture(
            repo,
            &self.state.nonce,
            self.state.copies,
            original,
            tip,
            super::head_commit_id(repo)?,
            &members,
        )
    }

    /// Every member a retained copy of the current state holds.
    fn current_members<'repo>(
        &self,
        repo: &'repo gix::Repository,
    ) -> Result<Vec<super::retained::Member<super::retained::Source<'repo>>>> {
        use super::retained::{Area, Kind, Member, Source};
        let mut members = vec![
            Member {
                area: Area::Original,
                path: b"preservation.json".to_vec(),
                stage: None,
                kind: Kind::File,
                mode: 0o600,
                data: Source::Bytes(serde_json::to_vec(&self.state.preservation)?),
            },
            Member {
                area: Area::Indexes,
                path: b"original".to_vec(),
                stage: None,
                kind: Kind::File,
                mode: 0o600,
                data: Source::Bytes(self.state.preservation.original_index().to_vec()),
            },
            Member {
                area: Area::Indexes,
                path: b"observed".to_vec(),
                stage: None,
                kind: Kind::File,
                mode: 0o600,
                data: Source::File(repo.index_path()),
            },
        ];
        for change in &self.state.work {
            members.push(retained_image(
                &self.directory,
                Area::Before,
                &change.path,
                &change.before,
            )?);
            let path = repo
                .workdir()
                .context("worktree required")?
                .join(gix::path::from_bstr(change.path.as_bstr())?.as_ref());
            members.push(retained_path(Area::Worktree, &change.path, &path)?);
        }
        let observed = repo.open_index()?;
        for entry in observed.entries() {
            let path = entry.path(&observed);
            if self
                .state
                .work
                .iter()
                .any(|change| change.path.as_slice() == path.as_bytes())
            {
                ensure!(
                    repo.find_header(entry.id)?.kind() == gix::objs::Kind::Blob,
                    "staged object is not a blob"
                );
                members.push(Member {
                    area: Area::Staged,
                    path: path.to_vec(),
                    stage: (entry.stage_raw() != 0).then_some(u8::try_from(entry.stage_raw())?),
                    kind: if entry.mode == gix::index::entry::Mode::SYMLINK {
                        Kind::Symlink
                    } else {
                        Kind::File
                    },
                    mode: entry.mode.bits(),
                    data: Source::Object(repo, entry.id),
                });
            }
        }
        let mut retained_objects = self.state.closure.iter().cloned().collect::<BTreeSet<_>>();
        retained_objects.extend(
            self.state
                .preservation
                .original_object_ids()?
                .iter()
                .map(ToString::to_string),
        );
        // A gitlink's id names a submodule commit, never a local object.
        retained_objects.extend(
            observed
                .entries()
                .iter()
                .filter(|entry| entry.mode != gix::index::entry::Mode::COMMIT)
                .map(|entry| entry.id.to_string()),
        );
        for value in &retained_objects {
            let id = gix::ObjectId::from_hex(value.as_bytes())?;
            let object = repo.find_header(id)?;
            members.push(Member {
                area: Area::Objects,
                path: value.as_bytes().to_vec(),
                stage: None,
                kind: Kind::Object {
                    id: value.clone(),
                    kind: object.kind().to_string(),
                },
                mode: 0o600,
                data: Source::Object(repo, id),
            });
        }
        retain_journal_members(&self.directory, &self.directory, &mut members)?;
        Ok(members)
    }

    /// The bytes the retained copy of the current state needs at most (members
    /// and an upper bound of its manifest), the bytes it certainly holds (its
    /// members alone), and the journal's own bytes.
    fn quota_usage(&self, repo: &gix::Repository) -> Result<(u64, u64, u64)> {
        use super::retained::Content;
        let members = self.current_members(repo)?;
        let mut held = 0u64;
        for member in &members {
            held = held.saturating_add(member.data.size()?);
        }
        let copy = super::retained::manifest_bound(members.iter().map(|member| member.path.len())).saturating_add(held);
        Ok((copy, held, tree_bytes(&self.directory)?))
    }

    /// The records one more decision adds at most: its state images (see
    /// `copy_bound::DECISION_IMAGES`), slack for its proposal, retention and lock
    /// records, and the public index and HEAD reflog every decision stores again.
    fn decision_bytes(&self, repo: &gix::Repository) -> Result<u64> {
        self.decision_bytes_for(repo, self.state_bytes()?.len() as u64)
    }

    /// [`Self::decision_bytes`] for a state record of `image` bytes.
    fn decision_bytes_for(&self, repo: &gix::Repository, image: u64) -> Result<u64> {
        Ok(image
            .saturating_mul(super::copy_bound::DECISION_IMAGES)
            .saturating_add(RECORD_SLACK)
            .saturating_add(super::copy_bound::file_size(&repo.index_path())?)
            .saturating_add(super::copy_bound::file_size(&repo.git_dir().join("logs/HEAD"))?))
    }

    /// What the next forward step stores beyond what the journal already keeps:
    /// one decision's records; with `worktree`, every footprint file whose bytes
    /// are not yet a payload (a resolution, an edit); and every blob `desired`
    /// holds for the footprint and for `extra` paths that is not yet a payload.
    /// Content already kept is stored once, so it costs nothing again.
    fn step_need(
        &self,
        repo: &gix::Repository,
        desired: Option<&gix::index::State>,
        worktree: bool,
        extra: &[BString],
    ) -> Result<u64> {
        self.step_need_for(repo, self.state_bytes()?.len() as u64, desired, worktree, extra)
    }

    /// [`Self::step_need`] when the record the step saves is `image` bytes, which
    /// the caller knows better than the state in memory does.
    fn step_need_for(
        &self,
        repo: &gix::Repository,
        image: u64,
        desired: Option<&gix::index::State>,
        worktree: bool,
        extra: &[BString],
    ) -> Result<u64> {
        let mut need = self.decision_bytes_for(repo, image)?;
        // An open authoring child's journal moves into this one when it is accepted.
        let child = repo.git_dir().join("brit-authoring");
        if present(&child)? {
            need = need.saturating_add(tree_bytes(&child)?);
        }
        let root = repo.workdir().context("worktree required")?;
        let paths = self.state.work.iter().map(|change| change.path.as_slice()).chain(
            extra
                .iter()
                .map(|path| path.as_slice())
                .filter(|path| !self.state.work.iter().any(|change| change.path.as_slice() == *path)),
        );
        for path in paths {
            if worktree
                && let switch::State::StoredFile { size, checksum, .. } =
                    switch::payload::fingerprint(&root.join(gix::path::from_bstr(path.as_bstr())?.as_ref()))?
                && !self.state.payloads.contains_key(&checksum)
            {
                need = need.saturating_add(size);
            }
            if let Some(entry) = desired.and_then(|desired| desired.entry_by_path(path.as_bstr()))
                && !self.state.payloads.contains_key(&entry.id.to_string())
            {
                need = need.saturating_add(repo.find_header(entry.id)?.size());
            }
        }
        Ok(need)
    }

    /// Refuse, before anything of a forward step is written, a step after which
    /// the journal could exceed the quota with the safety factor (`requested` is
    /// what the step adds). The retained copy an abort or a cleanup writes is not
    /// metered (see the `copy_bound` module), so it never refuses a step that
    /// admission promised. Recovery (an abort, a cleanup, a decision nothing can
    /// undo) is never refused here.
    fn reserve(&self, repo: &gix::Repository, requested: u64) -> Result<()> {
        self.reserve_for(repo, requested, false)
    }

    /// [`Self::reserve`] for a step that itself writes a retained copy under the
    /// quota (a skip): that copy, with `requested`, is reserved with the safety
    /// factor as the journal is.
    fn reserve_with_copy(&self, repo: &gix::Repository, requested: u64) -> Result<()> {
        self.reserve_for(repo, requested, true)
    }

    fn reserve_for(&self, repo: &gix::Repository, requested: u64, metered_copy: bool) -> Result<()> {
        if !switch::payload::metered() {
            return Ok(());
        }
        let limit = switch::payload::scratch_limit();
        let (copy, _, journal) = self.quota_usage(repo)?;
        let need = |bytes: u64| super::copy_bound::with_safety(bytes.saturating_add(requested));
        ensure!(
            need(journal) <= limit,
            "rebase refused this step before writing anything of it: its journal ({journal} bytes), with {requested} \
             bytes for the step and the {}/{} safety factor, could exceed the {limit}-byte scratch limit the journal \
             lives under; brit rebase --abort restores the original state",
            super::copy_bound::SAFETY.0,
            super::copy_bound::SAFETY.1
        );
        ensure!(
            !metered_copy || need(copy) <= limit,
            "rebase refused this step before writing anything of it: the retained copy it writes ({copy} bytes), \
             with {requested} bytes for the step and the {}/{} safety factor, could exceed the {limit}-byte scratch \
             limit that copy lives under; brit rebase --abort restores the original state",
            super::copy_bound::SAFETY.0,
            super::copy_bound::SAFETY.1
        );
        Ok(())
    }

    /// Admit, once preparation knows its exact records, only a linear replay whose
    /// peak journal fits the quota with the safety factor to spare, and whose
    /// retained copy does not already exceed it. To the present journal the peak
    /// adds every planned decision's records, one stop's and the final
    /// publication's, what the replay writes ([`super::copy_bound::Plan`], with a
    /// stop and a resolution as large as the largest step) and the largest
    /// publication staged at once. The retained copy an abort or a cleanup writes
    /// is not metered, so only the bytes it already certainly holds (its members,
    /// without its manifest) are tested, unscaled. Steps beyond
    /// the plan (further stops, edits) are reserved for when they happen.
    fn admit(&self, repo: &gix::Repository) -> Result<()> {
        use super::copy_bound::{DECISION_IMAGES, STATE_BYTES_PER_OBJECT, with_safety};
        let limit = switch::payload::scratch_limit();
        let (_, held, journal) = self.quota_usage(repo)?;
        let replayed = self
            .state
            .candidates
            .iter()
            .map(|(commit, _)| gix::ObjectId::from_hex(commit.as_bytes()))
            .collect::<std::result::Result<Vec<_>, _>>()?;
        let plan = super::copy_bound::plan(
            repo,
            gix::ObjectId::from_hex(self.state.original.as_bytes())?,
            gix::ObjectId::from_hex(self.state.upstream.as_bytes())?,
            &replayed,
            &mut gix::objs::find::Buffer::default(),
        )?;
        // Later state images also list every object the replay creates.
        let image =
            (self.state_bytes()?.len() as u64).saturating_add(plan.new_objects.saturating_mul(STATE_BYTES_PER_OBJECT));
        let per_decision = image
            .saturating_mul(DECISION_IMAGES)
            .saturating_add(RECORD_SLACK)
            .saturating_add(super::copy_bound::file_size(&repo.index_path())?)
            .saturating_add(super::copy_bound::file_size(&repo.git_dir().join("logs/HEAD"))?);
        // The initial checkout, each replayed commit, one stop and the final
        // publication, and the stop's authoring child, archived into this journal.
        let decisions = replayed.len() as u64 + 3;
        let journal_peak = journal.saturating_add(
            per_decision
                .saturating_mul(decisions)
                .saturating_add(image.saturating_mul(CHILD_IMAGES))
                .saturating_add(plan.payloads())
                .saturating_add(plan.largest_version),
        );
        ensure!(
            with_safety(journal_peak) <= limit,
            "rebase refused before it changed anything: with its exact records known, its journal can reach \
             {journal_peak} bytes, and with the {}/{} safety factor that exceeds the {limit}-byte scratch limit the \
             journal lives under; the preparation was cancelled and nothing was changed; git rebase is the fallback",
            super::copy_bound::SAFETY.0,
            super::copy_bound::SAFETY.1
        );
        ensure!(
            held <= limit,
            "rebase refused before it changed anything: the retained copy an abort would write already holds {held} \
             bytes, over the {limit}-byte scratch limit; the preparation was cancelled and nothing was changed; git \
             rebase is the fallback"
        );
        Ok(())
    }

    /// Prove that the active journal, if it holds a state record, is this
    /// repository's own: directory and record ownership and mode, the checksum, and
    /// the recorded repository paths and inode identities. Reads nothing else and
    /// changes nothing. A journal that fails this is touched by no command.
    pub fn check_own(repo: &gix::Repository) -> Result<()> {
        let directory = repo.git_dir().join("brit-rebase");
        for name in ["state.json", "state.pending"] {
            if present(&directory.join(name))? {
                return Self::open_directory_checked(repo, directory, name, false).map(drop);
            }
        }
        Ok(())
    }

    /// Before a command reads this repository's journal, discard a successor
    /// record a killed command left unrenamed beside `state.json` (see the
    /// module's write-order table: it never took effect, and nothing live was
    /// written after it), so no step, copy or inventory ever sees it. A
    /// preparation's own successor is left to preparation recovery, which rolls
    /// it forward. Holds the mutation guard, so it never races a live writer.
    pub fn settle_pending(repo: &gix::Repository) -> Result<()> {
        const PREPARATION: [&str; 3] = ["preparing", "prepared", "cancelling-prepared"];
        let _guard = switch::shared_guard(repo)?;
        let directory = repo.git_dir().join("brit-rebase");
        if !present(&directory.join("state.pending"))? || !present(&directory.join("state.json"))? {
            return Ok(());
        }
        let session = Self::open_directory_checked(repo, directory.clone(), "state.json", false)?;
        if PREPARATION.contains(&session.state.phase.as_str())
            && read_record(&directory.join("state.pending"))
                .and_then(|bytes| decode_state(&bytes))
                .is_ok_and(|pending| PREPARATION.contains(&pending.phase.as_str()))
        {
            return Ok(());
        }
        session.discard_unrenamed_pending().map(drop)
    }

    /// For a journal that does not open because the repository was moved: the
    /// worktree location its checksummed record names, when that location no
    /// longer exists and is not this repository's. Reads only that one field of a
    /// record whose checksum holds; trusts nothing else in it and changes nothing.
    pub fn recorded_location(repo: &gix::Repository) -> Option<std::path::PathBuf> {
        let directory = repo.git_dir().join("brit-rebase");
        let name = ["state.json", "state.pending"]
            .into_iter()
            .find(|name| present(&directory.join(name)).unwrap_or(false))?;
        let state = decode_state(&read_record(&directory.join(name)).ok()?).ok()?;
        use std::os::unix::ffi::OsStringExt;
        let current = raw(repo.workdir()?).ok()?;
        let differs = state.worktree != current;
        let recorded = std::path::PathBuf::from(std::ffi::OsString::from_vec(state.worktree));
        (differs && recorded.is_absolute() && !present(&recorded).unwrap_or(true)).then_some(recorded)
    }

    /// Open without changing any repository resource or replaying any action.
    pub fn open(repo: &gix::Repository) -> Result<Self> {
        match fs::symlink_metadata(repo.git_dir().join("brit-rebase/state.json")) {
            Ok(_) => Self::open_record(repo, "state.json"),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Self::open_record(repo, "state.pending"),
            Err(error) => Err(error.into()),
        }
    }

    fn open_record(repo: &gix::Repository, name: &str) -> Result<Self> {
        let directory = repo.git_dir().join("brit-rebase");
        Self::open_directory(repo, directory, name)
    }

    fn open_directory(repo: &gix::Repository, directory: std::path::PathBuf, name: &str) -> Result<Self> {
        Self::open_directory_checked(repo, directory, name, true)
    }

    /// `full` is every ordinary open: it validates payload contents and retention
    /// references. Cancelling a preparation opens with `full = false`: it still
    /// proves ownership, identity and the checksum, but reads nothing it is about to
    /// remove, so a journal that lost a ref can be released; [`Self::check_own`]
    /// uses it to decide whether any command may touch the journal at all. No open
    /// applies the journal quota; only the forward writes that grow a journal do.
    fn open_directory_checked(
        repo: &gix::Repository,
        directory: std::path::PathBuf,
        name: &str,
        full: bool,
    ) -> Result<Self> {
        let metadata = fs::symlink_metadata(&directory)?;
        ensure!(
            metadata.is_dir()
                && metadata.permissions().mode().trailing_zeros() >= 6
                && metadata.uid() == fs::metadata(repo.git_dir())?.uid(),
            "unsafe replay directory"
        );
        let path = directory.join(name);
        let file = fs::symlink_metadata(&path)?;
        ensure!(
            file.is_file() && file.permissions().mode().trailing_zeros() >= 6 && file.uid() == metadata.uid(),
            "unsafe replay state"
        );
        // Reading is never refused for the quota: recovery must be able to open a
        // journal that outgrew it. One record is bounded before it is allocated.
        let state = decode_state(&read_record(&path)?)?;
        ensure!(
            matches!(state.version, 1 | 2)
                && state.nonce.len() == 40
                && state.nonce.as_bytes().iter().all(u8::is_ascii_hexdigit),
            "unsupported replay journal identity"
        );
        let root = repo.workdir().context("worktree required")?;
        ensure!(
            state.owner == raw(repo.git_dir())?
                && state.worktree == raw(root)?
                && state.common == raw(repo.common_dir())?,
            "replay repository identity mismatch"
        );
        let identities: Vec<_> = [repo.git_dir(), root, repo.common_dir(), directory.as_path()]
            .into_iter()
            .map(|path| fs::symlink_metadata(path).map(|m| (m.dev(), m.ino())))
            .collect::<std::io::Result<_>>()?;
        ensure!(identities == state.identities, "replay inode identity mismatch");
        let session = Self { directory, state };
        if full {
            session.validate_payloads()?;
            if session.state.phase == "prepared" {
                session.validate_retention(repo)?;
            }
        }
        Ok(session)
    }

    /// Finish only a verified interrupted preparation. This never detaches HEAD
    /// or invokes a hook; it synchronizes the pinned closure and its GC roots.
    pub fn recover_preparation(repo: &gix::Repository) -> Result<Self> {
        let _guard = switch::shared_guard(repo)?;
        switch::validate_local_history(repo)?;
        switch::validate_object_boundaries(repo)?;
        let mut session = Self::open(repo)?;
        ensure!(
            matches!(session.state.phase.as_str(), "preparing" | "prepared"),
            "journal has progressed beyond preparation"
        );
        ensure!(
            switch::snapshot(&repo.git_dir().join("HEAD"))? == session.state.head
                && super::head_commit_id(repo)? == gix::ObjectId::from_hex(session.state.original.as_bytes())?,
            "foreign original HEAD movement"
        );
        ensure!(
            switch::payload::fingerprint(&repo.index_path())? == session.state.index,
            "foreign index mutation during preparation"
        );
        let pending = session.directory.join("state.pending");
        if fs::symlink_metadata(&pending).is_ok() {
            let candidate = Self::open_record(repo, "state.pending")?;
            let mut comparable = candidate.state.clone();
            ensure!(
                matches!(comparable.phase.as_str(), "preparing" | "prepared"),
                "invalid preparation successor"
            );
            comparable.phase.clone_from(&session.state.phase);
            ensure!(
                serde_json::to_vec(&comparable)? == serde_json::to_vec(&session.state)?,
                "foreign pending preparation state"
            );
            fs::rename(pending, session.directory.join("state.json"))?;
            switch::sync_dir(&session.directory)?;
            switch::sync_dir(repo.git_dir())?;
            session = candidate;
        }
        if session.state.phase == "preparing" {
            // Older Brit captured gitlinks it could not prepare; finishing their
            // preparation now would let a refused rebase proceed.
            if let Some(path) = session.state.preservation.gitlink_paths().next() {
                anyhow::bail!(
                    "gitlink {} is unsupported by brit rebase; this preparation cannot continue; brit rebase --abort cancels it",
                    path.as_bstr()
                );
            }
            session.complete_preparation(repo)?;
        }
        session.validate_retention(repo)?;
        Ok(session)
    }

    fn validate_retention(&self, repo: &gix::Repository) -> Result<()> {
        for (label, value) in &self.state.retained {
            let name = format!("refs/brit-rebase-retain/{}/{label}", self.state.nonce);
            let reference = repo.find_reference(name.as_str())?;
            ensure!(
                matches!(reference.target(), gix::refs::TargetRef::Object(_))
                    && reference.id() == gix::ObjectId::from_hex(value.as_bytes())?,
                "missing or foreign replay retention reference"
            );
        }
        Ok(())
    }

    fn validate_branch(&self, repo: &gix::Repository, decided_final: Option<&str>) -> Result<()> {
        if let Some(name) = &self.state.attachment {
            let reference = repo.find_reference(name.as_bstr())?;
            ensure!(
                matches!(reference.target(), gix::refs::TargetRef::Object(_)),
                "original branch became symbolic"
            );
            let id = reference.id().to_string();
            ensure!(
                id == self.state.original || decided_final == Some(id.as_str()),
                "original branch moved during replay"
            );
            let owner = fs::canonicalize(repo.git_dir())?;
            for worktree in repo.worktrees_including_main()? {
                let worktree = worktree?;
                if fs::canonicalize(worktree.current_dir().join(worktree.git_dir()))? != owner {
                    ensure!(
                        worktree
                            .head_name()?
                            .as_ref()
                            .is_none_or(|other| other.as_bstr() != name.as_bstr()),
                        "original branch is occupied by another worktree"
                    );
                }
            }
        }
        Ok(())
    }

    /// Cancel an unpublished preparation. This performs no restoration because
    /// preparation has not modified live files, the index, HEAD, or branches.
    /// Returns the live-state differences found, which are left as they are.
    pub fn abort_prepared(&self, repo: &gix::Repository) -> Result<Vec<String>> {
        let _guard = switch::shared_guard(repo)?;
        let current = Self::open_directory_checked(repo, self.directory.clone(), "state.json", false)?;
        ensure!(
            serde_json::to_vec(&current.state)? == serde_json::to_vec(&self.state)?,
            "stale replay session"
        );
        Self::cancel_locked(repo, &self.directory)?.context("only an unpublished preparation can be cancelled here")
    }

    /// Cancel the active journal if its phase shows preparation never left
    /// preparation: `preparing`, `cancelling-prepared`, or `prepared` before
    /// anything beyond preparation's own members was added. Returns `None`,
    /// changing nothing, for any other journal; the caller then uses [`Self::abort`].
    ///
    /// The rule this implements is the module's preparation contract: preparation
    /// changes nothing live, names every artifact in `state.json` before creating
    /// it, and is therefore cancellable without its own success. Cancellation
    /// reads only those named artifacts and the pinned HEAD, index and branch
    /// fingerprints: it never reads or verifies an object, never re-captures
    /// preservation, never builds a retained copy, and does not apply the journal
    /// quota. Live state that differs from the pinned fingerprints is someone
    /// else's change; it is left as found and returned for the caller to report.
    pub fn cancel_preparation(repo: &gix::Repository) -> Result<Option<Vec<String>>> {
        let _guard = switch::shared_guard(repo)?;
        Self::cancel_locked(repo, &repo.git_dir().join("brit-rebase"))
    }

    fn cancel_locked(repo: &gix::Repository, directory: &Path) -> Result<Option<Vec<String>>> {
        const PREPARATION: [&str; 3] = ["preparing", "prepared", "cancelling-prepared"];
        let owner = fs::metadata(repo.git_dir())?.uid();
        let metadata = fs::symlink_metadata(directory)?;
        ensure!(
            metadata.is_dir() && metadata.permissions().mode().trailing_zeros() >= 6 && metadata.uid() == owner,
            "unsafe replay directory"
        );
        if !present(&directory.join("state.json"))? {
            return Self::release_unpublished(repo, directory, owner).map(Some);
        }
        let mut current = Self::open_directory_checked(repo, directory.to_owned(), "state.json", false)?;
        if !PREPARATION.contains(&current.state.phase.as_str()) {
            return Ok(None);
        }
        let pending = directory.join("state.pending");
        if present(&pending)? {
            ensure!(
                owned_private_file(&pending, owner)?,
                "unsafe pending replay state; inspect it before cancelling"
            );
            match decode_state(&read_record(&pending)?) {
                Ok(candidate) if !PREPARATION.contains(&candidate.phase.as_str()) => {
                    // A successor that already left preparation, e.g. an
                    // operation hook's durable start: the full abort owns it.
                    ensure!(
                        current.state.phase == "prepared",
                        "pending replay state has progressed beyond preparation; inspect with --status"
                    );
                    return Ok(None);
                }
                Ok(candidate) => {
                    let mut comparable = candidate.clone();
                    comparable.phase.clone_from(&current.state.phase);
                    ensure!(
                        serde_json::to_vec(&comparable)? == serde_json::to_vec(&current.state)?,
                        "foreign pending preparation state; inspect it before cancelling"
                    );
                    fs::rename(&pending, directory.join("state.json"))?;
                    switch::sync_dir(directory)?;
                    current.state = candidate;
                }
                // A record torn by process death never became state; it is
                // brit's own private member and holds nothing else.
                Err(_) => {
                    fs::remove_file(&pending)?;
                    switch::sync_dir(directory)?;
                }
            }
        }
        let mut unlaunched = Vec::new();
        for entry in fs::read_dir(directory)? {
            let entry = entry?;
            let name = entry.file_name();
            let named = name == "state.json"
                || current
                    .state
                    .payloads
                    .keys()
                    .any(|hash| name == format!("payload-{hash}").as_str());
            if named {
                continue;
            }
            // `advance` may have written the pre-rebase private index and prepared
            // the hook's invocation while the phase was still `prepared`. The hook
            // is spawned only after its durable start records `invoking-pre`, so in
            // `prepared` it never ran; both are private and cancellation removes them.
            if current.state.phase != "preparing" && unlaunched_operation_artifact(&entry.path(), owner)? {
                unlaunched.push(entry.path());
                continue;
            }
            // Anything else beyond preparation belongs to the full abort.
            if current.state.phase == "prepared" {
                return Ok(None);
            }
            anyhow::bail!("unknown replay journal member {name:?}; inspect it before cancelling");
        }
        // Defence in depth beside the mutation guard: an invocation whose lease is
        // still held belongs to a live supervisor, which cancellation never races.
        // The lease is held until its directory is removed.
        let mut _leases = Vec::new();
        for path in &unlaunched {
            if fs::symlink_metadata(path)?.is_dir() {
                _leases.extend(crate::repository::authoring::invocation::hold_lease(path)?);
            }
        }
        let differences = current.live_differences(repo)?;
        if current.state.phase != "cancelling-prepared" {
            current.state.phase = "cancelling-prepared".into();
            // Unmetered: a journal the quota refuses must still be cancellable.
            current.save_bytes(&current.state_bytes()?, false)?;
        }
        // Only the records cancellation names may reach the tombstone.
        for path in unlaunched {
            if fs::symlink_metadata(&path)?.is_dir() {
                for member in fs::read_dir(&path)? {
                    fs::remove_file(member?.path())?;
                }
                switch::sync_dir(&path)?;
                fs::remove_dir(&path)?;
            } else {
                fs::remove_file(&path)?;
            }
        }
        switch::sync_dir(directory)?;
        for (label, id) in [
            ("original", &current.state.original),
            ("upstream", &current.state.upstream),
        ] {
            let name = format!("refs/brit-rebase-retain/{}/{label}", current.state.nonce);
            if let Some(reference) = repo.try_find_reference(name.as_str())? {
                ensure!(
                    matches!(reference.target(), gix::refs::TargetRef::Object(_))
                        && reference.id() == gix::ObjectId::from_hex(id.as_bytes())?,
                    "foreign replay retention reference {name}; inspect it before cancelling"
                );
                reference.delete()?;
            }
        }
        sync_reference_storage(repo)?;
        let tombstone = repo.git_dir().join(format!("brit-rebase-done-{}", current.state.nonce));
        ensure!(!tombstone.exists(), "replay tombstone collision");
        fs::rename(directory, &tombstone)?;
        switch::sync_dir(repo.git_dir())?;
        switch::checkpoint("replay-preparation-tombstoned")?;
        Self::cleanup_prepared_inner(repo, &tombstone)?;
        Ok(Some(differences))
    }

    /// Set aside a journal of this repository whose only remaining work is its
    /// retained copy, which can take long on a large repository or wait for disk
    /// space: `completed` and `aborted`, whose live state is final, and an abort
    /// retaining before its first checkout decision, when nothing live changed.
    /// The journal must open as this repository's own (identity, checksum and
    /// ownership); no other journal is touched. Refused, naming the command that
    /// applies: preparation (`--abort` cancels it), every phase that still has live
    /// state to restore or a decision in flight (`--abort`, which the quota never
    /// refuses), and an open conflict-authoring session. The journal directory is
    /// renamed to `brit-rebase-kept-<nonce>` beside it; its retention references
    /// and any retained copy stay where they are, and no lock or live file is
    /// touched. Returns `None` when there is nothing to retire.
    pub fn retire(repo: &gix::Repository) -> Result<Option<Retired>> {
        let _guard = switch::shared_guard(repo)?;
        let directory = repo.git_dir().join("brit-rebase");
        if !present(&directory)? {
            return Ok(None);
        }
        let session = Self::open(repo).context(
            "brit rebase --retire takes only a journal this repository opens as its own; it changed nothing. \
             brit rebase --status says what failed",
        )?;
        let state = &session.state;
        let phase = state.phase.as_str();
        ensure!(
            !matches!(phase, "preparing" | "prepared" | "cancelling-prepared"),
            "this rebase never left preparation; brit rebase --abort cancels it"
        );
        ensure!(
            matches!(phase, "completed" | "aborted" | "retaining-abort" | "retaining-skip"),
            "rebase phase {phase} still has live state to finish or restore, so --retire refuses it; brit rebase \
             --abort restores the original state, or --continue carries the rebase on"
        );
        let next = state.decisions + 1;
        let mut in_flight = present(&directory.join(format!("decision-{next}.json")))?;
        for entry in fs::read_dir(&directory)? {
            in_flight |= entry?
                .file_name()
                .as_encoded_bytes()
                .starts_with(format!("transition-{next}-").as_bytes());
        }
        ensure!(
            !in_flight,
            "rebase phase {phase} has a decision part-way through changing the checkout, so --retire refuses it; \
             brit rebase {} finishes it",
            if phase == "completed" { "--continue" } else { "--abort" }
        );
        ensure!(
            !repo.git_dir().join("brit-authoring").try_exists()?,
            "a conflict-authoring session of this rebase is still open, so --retire refuses it; brit rebase --abort \
             closes it and restores the original state"
        );
        ensure!(
            !matches!(phase, "retaining-abort" | "retaining-skip") || state.decisions == 0,
            "rebase phase {phase}: the checkout is part-way through the replay and must be restored, so --retire \
             refuses it; brit rebase --abort restores the original state"
        );
        let receipt = repo.git_dir().join(format!("brit-rebase-cleanup-{}.json", state.nonce));
        ensure!(
            !present(&receipt)?,
            "cleanup already proved its retained copy and is releasing it; brit rebase --continue finishes it"
        );
        let kept = repo.git_dir().join(format!("brit-rebase-kept-{}", state.nonce));
        ensure!(!present(&kept)?, "{} already exists; inspect it", kept.display());
        let mut refs = Vec::new();
        let prefix = format!("refs/brit-rebase-retain/{}/", state.nonce);
        for reference in repo.references()?.prefixed(prefix.as_str())? {
            let reference = reference.map_err(|error| anyhow::anyhow!("{error}"))?;
            refs.push((reference.name().as_bstr().to_string(), reference.id().to_string()));
        }
        let copies = repo.git_dir().join("brit-rebase-retained").join(&state.nonce);
        let retired = Retired {
            nonce: state.nonce.clone(),
            published: phase == "completed",
            tip: state.tip.clone(),
            original: state.original.clone(),
            journal: kept,
            journal_bytes: tree_bytes(&directory)?,
            refs,
            copies: if present(&copies)? {
                Some((copies.clone(), tree_bytes(&copies)?))
            } else {
                None
            },
        };
        drop(session);
        fs::rename(&directory, &retired.journal)?;
        switch::sync_dir(repo.git_dir())?;
        switch::checkpoint("replay-retired")?;
        Ok(Some(retired))
    }

    /// Release a journal directory that never published `state.json`. Preparation
    /// publishes its first record before it creates any payload or retention
    /// reference, so such an allocation holds at most an unpublished
    /// `state.pending` and nothing else of the operation exists.
    fn release_unpublished(repo: &gix::Repository, directory: &Path, owner: u32) -> Result<Vec<String>> {
        let entries = fs::read_dir(directory)?.collect::<std::io::Result<Vec<_>>>()?;
        for entry in &entries {
            ensure!(
                entry.file_name() == "state.pending" && owned_private_file(&entry.path(), owner)?,
                "unknown member {:?} in an unpublished replay journal; inspect it before cancelling",
                entry.file_name()
            );
        }
        for entry in entries {
            fs::remove_file(entry.path())?;
        }
        switch::sync_dir(directory)?;
        fs::remove_dir(directory)?;
        switch::sync_dir(repo.git_dir())?;
        Ok(Vec::new())
    }

    /// Live state that no longer matches what preparation pinned.
    fn live_differences(&self, repo: &gix::Repository) -> Result<Vec<String>> {
        let mut differences = Vec::new();
        let original = gix::ObjectId::from_hex(self.state.original.as_bytes())?;
        if switch::snapshot(&repo.git_dir().join("HEAD"))? != self.state.head {
            differences.push("HEAD changed since preparation".to_owned());
        }
        match super::head_commit_id(repo) {
            Ok(id) if id == original => {}
            Ok(id) => differences.push(format!(
                "HEAD now names {id}; preparation pinned {}",
                self.state.original
            )),
            Err(error) => differences.push(format!("HEAD no longer names a readable commit: {error}")),
        }
        if switch::payload::fingerprint(&repo.index_path())? != self.state.index {
            differences.push("the index changed since preparation".to_owned());
        }
        if let Some(name) = &self.state.attachment {
            match repo.try_find_reference(name.as_bstr())? {
                Some(reference)
                    if matches!(reference.target(), gix::refs::TargetRef::Object(_)) && reference.id() == original => {}
                _ => differences.push(format!(
                    "branch {} no longer names {}",
                    name.as_bstr(),
                    self.state.original
                )),
            }
        }
        Ok(differences)
    }

    /// Finish cleanup after an already cancelled preparation was tombstoned.
    /// The exact returned/diagnosed tombstone leaf is required; never scan or
    /// expire other private operation records.
    pub fn cleanup_prepared(repo: &gix::Repository, tombstone: &Path) -> Result<()> {
        let _guard = switch::shared_guard(repo)?;
        Self::cleanup_prepared_inner(repo, tombstone)
    }

    fn cleanup_prepared_inner(repo: &gix::Repository, tombstone: &Path) -> Result<()> {
        ensure!(
            fs::canonicalize(tombstone.parent().context("tombstone parent")?)? == fs::canonicalize(repo.git_dir())?,
            "tombstone is not in owning Git directory"
        );
        let receipt = tombstone.with_extension("receipt");
        let state = match fs::symlink_metadata(&receipt) {
            Ok(metadata) => {
                ensure!(
                    metadata.is_file()
                        && metadata.permissions().mode().trailing_zeros() >= 6
                        && metadata.uid() == fs::metadata(repo.git_dir())?.uid(),
                    "unsafe preparation cleanup receipt"
                );
                let envelope: Envelope = serde_json::from_slice(&fs::read(&receipt)?)?;
                ensure!(
                    switch::digest(&envelope.payload)? == envelope.checksum,
                    "corrupt cleanup receipt"
                );
                serde_json::from_slice::<ParentState>(&envelope.payload)?
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                let session = Self::open_directory_checked(repo, tombstone.to_owned(), "state.json", false)?;
                ensure!(
                    session.state.phase == "cancelling-prepared",
                    "tombstone is not cancelled"
                );
                ensure!(
                    tombstone.file_name()
                        == Some(std::ffi::OsStr::new(&format!(
                            "brit-rebase-done-{}",
                            session.state.nonce
                        ))),
                    "invalid preparation tombstone name"
                );
                fs::hard_link(tombstone.join("state.json"), &receipt)?;
                switch::sync_path(&receipt)?;
                switch::sync_dir(repo.git_dir())?;
                session.state
            }
            Err(error) => return Err(error.into()),
        };
        ensure!(
            matches!(state.version, 1 | 2)
                && state.phase == "cancelling-prepared"
                && tombstone.file_name() == Some(std::ffi::OsStr::new(&format!("brit-rebase-done-{}", state.nonce))),
            "invalid preparation tombstone"
        );
        let root = repo.workdir().context("worktree required")?;
        ensure!(
            state.owner == raw(repo.git_dir())?
                && state.worktree == raw(root)?
                && state.common == raw(repo.common_dir())?,
            "cleanup receipt repository mismatch"
        );
        let identities: Vec<_> = [repo.git_dir(), root, repo.common_dir()]
            .into_iter()
            .map(|path| fs::symlink_metadata(path).map(|m| (m.dev(), m.ino())))
            .collect::<std::io::Result<_>>()?;
        ensure!(
            state.identities.len() == 4 && state.identities[..3] == identities,
            "cleanup receipt inode mismatch"
        );
        // Existing receipt names may have survived a process crash before their
        // first directory sync. Re-establish durability before removing the
        // last in-directory proof of ownership on every recovery path.
        switch::sync_path(&receipt)?;
        switch::sync_dir(repo.git_dir())?;
        match fs::symlink_metadata(tombstone) {
            Ok(metadata) => {
                ensure!(
                    metadata.is_dir() && (metadata.dev(), metadata.ino()) == state.identities[3],
                    "foreign tombstone directory"
                );
                let entries = fs::read_dir(tombstone)?.collect::<std::io::Result<Vec<_>>>()?;
                // Validate the complete inventory before deleting any member.
                for entry in &entries {
                    if entry.file_name() == "state.json" {
                        ensure!(
                            fs::symlink_metadata(entry.path())?.is_file()
                                && fs::read(entry.path())? == fs::read(&receipt)?,
                            "unknown tombstone member"
                        );
                    } else {
                        let (hash, size) = state
                            .payloads
                            .iter()
                            .find(|(hash, _)| entry.file_name() == format!("payload-{hash}").as_str())
                            .context("unknown tombstone member")?;
                        switch::payload::open(tombstone, *size, hash)?;
                    }
                }
                for entry in entries {
                    fs::remove_file(entry.path())?;
                }
                switch::sync_dir(tombstone)?;
                switch::checkpoint("replay-preparation-last-member-removed")?;
                fs::remove_dir(tombstone)?;
                switch::sync_dir(repo.git_dir())?;
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
        fs::remove_file(receipt)?;
        switch::sync_dir(repo.git_dir())
    }

    /// An existing `state.pending` beside a published `state.json` is the
    /// successor record of a command killed before its rename. It was never in
    /// effect, and no live write lies between writing it and renaming it (see the
    /// module's write-order table), so the journal is exactly as if that command
    /// had stopped before writing it: remove it. Returns whether one was removed.
    /// Without `state.json` a pending record is the only record and stays.
    fn discard_unrenamed_pending(&self) -> Result<bool> {
        let pending = self.directory.join("state.pending");
        if !present(&pending)? || !present(&self.directory.join("state.json"))? {
            return Ok(false);
        }
        ensure!(
            owned_private_file(&pending, fs::metadata(&self.directory)?.uid())?,
            "unsafe pending replay state"
        );
        fs::remove_file(&pending)?;
        switch::sync_dir(&self.directory)?;
        Ok(true)
    }

    fn save(&self) -> Result<()> {
        use std::io::{BufWriter, Seek, Write};
        #[cfg(feature = "switch-test-faults")]
        switch::checkpoint("replay-state-file-writing")?;
        self.discard_unrenamed_pending()?;
        let payload = serde_json::to_vec(&self.state)?;
        let envelope = Envelope {
            checksum: switch::digest(&payload)?,
            payload,
        };
        let budget = gix::utils::stream::Budget::new(self.directory.parent().context("journal parent")?)?;
        let _charge = switch::payload::charge(&budget, &self.directory)?;
        let mut source = budget.spool()?;
        {
            let mut writer = BufWriter::new(&mut source);
            serde_json::to_writer(&mut writer, &envelope)?;
            writer.flush()?;
        }
        drop(envelope);
        source.rewind()?;
        let temporary = self.directory.join("state.pending");
        match fs::symlink_metadata(&temporary) {
            Ok(metadata) => {
                ensure!(
                    metadata.is_file()
                        && metadata.permissions().mode().trailing_zeros() >= 6
                        && metadata.uid() == fs::metadata(&self.directory)?.uid()
                        && gix::utils::stream::equal(
                            fs::File::open(&temporary)?,
                            &mut source,
                            &gix::interrupt::IS_INTERRUPTED
                        )?,
                    "foreign pending replay state"
                );
                switch::sync_path(&temporary)?;
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => switch::private_publish(&temporary, source)?,
            Err(error) => return Err(error.into()),
        }
        self.publish_state()
    }

    fn state_bytes(&self) -> Result<Vec<u8>> {
        record_bytes(&self.state)
    }

    /// `metered` applies the journal quota to a new record (outside a
    /// `switch::payload::Recovery` scope); cancellation writes unmetered, so an
    /// exhausted journal can still be released.
    fn save_bytes(&self, bytes: &[u8], metered: bool) -> Result<()> {
        #[cfg(feature = "switch-test-faults")]
        switch::checkpoint("replay-state-file-writing")?;
        self.discard_unrenamed_pending()?;
        let temporary = self.directory.join("state.pending");
        match fs::symlink_metadata(&temporary) {
            Ok(metadata) => {
                ensure!(
                    metadata.is_file()
                        && metadata.permissions().mode().trailing_zeros() >= 6
                        && metadata.uid() == fs::metadata(&self.directory)?.uid()
                        && fs::read(&temporary)? == bytes,
                    "foreign pending replay state"
                );
                switch::sync_path(&temporary)?;
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                if metered {
                    switch::payload::check_growth(&self.directory, bytes.len() as u64)?;
                }
                switch::private_write(&temporary, bytes)?;
            }
            Err(error) => return Err(error.into()),
        }
        self.publish_state()
    }

    fn publish_state(&self) -> Result<()> {
        switch::checkpoint("replay-state-file-synced")?;
        fs::rename(self.directory.join("state.pending"), self.directory.join("state.json"))?;
        switch::sync_dir(&self.directory)?;
        switch::checkpoint("replay-state-directory-synced")?;
        Ok(())
    }

    fn publish_decision(
        &mut self,
        repo: &gix::Repository,
        mut after: ParentState,
        mut transition: super::transition::Transition,
    ) -> Result<()> {
        self.validate_retention(repo)?;
        self.validate_branch(repo, None)?;
        switch::validate_local_history(repo)?;
        switch::validate_object_boundaries(repo)?;
        for (path, before, desired) in [
            (repo.index_path(), &self.state.index, &after.index),
            (repo.git_dir().join("HEAD"), &self.state.head, &after.head),
        ] {
            let path = gix::path::into_bstr(path)?.to_vec();
            if !transition.metadata.iter().any(|change| change.path == path) {
                transition.metadata.push(switch::Change {
                    path,
                    before: before.clone(),
                    after: desired.clone(),
                });
            }
        }
        after.decisions = self.state.decisions + 1;
        after.version = self.state.version;
        after.payloads.extend(self.state.payloads.clone());
        let decision = Decision {
            version: 1,
            nonce: self.state.nonce.clone(),
            ordinal: after.decisions,
            before: switch::digest(&serde_json::to_vec(&self.state)?)?,
            after,
            transition,
        };
        self.apply_decision(repo, decision, true)
    }

    fn checkout_transition(
        &mut self,
        repo: &gix::Repository,
        tree: gix::ObjectId,
        private: &gix::index::State,
        tip: gix::ObjectId,
        kind: super::transition::Kind,
    ) -> Result<(ParentState, super::transition::Transition)> {
        use std::io::Read;
        let observed = repo.open_index()?;
        self.state.preservation.validate_observed(repo, &observed)?;
        ensure!(
            switch::payload::fingerprint(&repo.index_path())? == self.state.index,
            "foreign replay index change"
        );
        ensure!(
            switch::snapshot(&repo.git_dir().join("HEAD"))? == self.state.head,
            "foreign replay HEAD change"
        );
        ensure!(
            switch::payload::fingerprint(&repo.git_dir().join("logs/HEAD"))? == self.state.log,
            "foreign replay HEAD reflog change"
        );
        let root = repo.workdir().context("worktree required")?;
        let desired = repo.index_from_tree(&tree)?;
        if kind != super::transition::Kind::Abort && switch::payload::metered() {
            // Every image this transition keeps, before the first is written.
            self.reserve(repo, self.step_need(repo, Some(&desired), false, &[])?)?;
        }
        let attributes = repo.attributes_only(&desired, gix::worktree::stack::state::attributes::Source::IdMapping)?;
        let (mut pipeline, mut attributes) = gix::filter::Pipeline::new(repo, attributes.detach())?.into_parts();
        let budget = gix::utils::stream::Budget::new(repo.git_dir())?;
        let mut after = self.state.clone();
        let mut changes = Vec::new();
        for tracked in &mut after.work {
            let relative = gix::path::from_bstr(tracked.path.as_bstr())?;
            let absolute = root.join(relative.as_ref());

            let before = switch::payload::fingerprint(&absolute)?;
            ensure!(before == tracked.after, "foreign worktree mutation in replay footprint");
            let image = match desired.entry_by_path(tracked.path.as_bstr()) {
                None => switch::State::Absent,
                Some(entry) => {
                    let _charge = switch::payload::charge(&budget, &self.directory)?;
                    let (kind, mut source) =
                        repo.objects
                            .find_stream(&entry.id, &budget, &gix::interrupt::IS_INTERRUPTED)?;
                    ensure!(
                        kind == gix::objs::Kind::Blob,
                        "replay destination is not a blob: {}",
                        entry.id
                    );
                    match entry.mode {
                        gix::index::entry::Mode::SYMLINK => {
                            ensure!(
                                repo.config_snapshot().try_boolean("core.symlinks")? != Some(false),
                                "emulated replay symlinks are unsupported"
                            );
                            let mut bytes = Vec::new();
                            source.read_to_end(&mut bytes)?;
                            ensure!(!bytes.is_empty() && !bytes.contains(&0), "invalid replay symlink");
                            switch::State::Symlink(bytes)
                        }
                        gix::index::entry::Mode::FILE | gix::index::entry::Mode::FILE_EXECUTABLE => {
                            let entry_attributes = attributes.at_entry(tracked.path.as_bstr(), None, &repo.objects)?;
                            let converted = pipeline.convert_to_worktree_stream(
                                source,
                                tracked.path.as_bstr(),
                                &mut |_, attrs| {
                                    entry_attributes.matching_attributes(attrs);
                                },
                                Default::default(),
                                &budget,
                                &gix::interrupt::IS_INTERRUPTED,
                            )?;
                            let gix::filter::plumbing::pipeline::stream::Outcome::Ready(source) = converted else {
                                unreachable!("delay is forbidden")
                            };
                            self.store_payload(
                                repo,
                                source,
                                switch::creation_mode(if entry.mode == gix::index::entry::Mode::FILE_EXECUTABLE {
                                    0o777
                                } else {
                                    0o666
                                })?,
                            )?
                        }
                        _ => anyhow::bail!("affected unsupported replay entry"),
                    }
                }
            };
            changes.push(switch::Change {
                path: tracked.path.clone(),
                before,
                after: image.clone(),
            });
            tracked.after = image;
        }
        after.version = self.state.version;
        after.payloads.clone_from(&self.state.payloads);
        let source_paths = observed
            .entries()
            .iter()
            .map(|entry| entry.path(&observed).to_vec())
            .collect::<BTreeSet<_>>();
        let destination_paths = desired
            .entries()
            .iter()
            .map(|entry| entry.path(&desired).to_vec())
            .collect::<BTreeSet<_>>();
        let mut ledger = changes
            .into_iter()
            .map(|change| (change.path.clone(), change))
            .collect::<BTreeMap<_, _>>();
        switch::complete_directory_ledger(root, &source_paths, &destination_paths, &mut ledger)?;
        // Directories are absent from tree indexes. The completed ledger restores
        // their actual inode postimages, including unchanged owned ancestors.
        // Record these before omitting no-op directory writes from the transition.
        for change in ledger.values() {
            if let Some(tracked) = after.work.iter_mut().find(|tracked| tracked.path == change.path) {
                tracked.after = change.after.clone();
            } else if change.before != change.after || !matches!(change.before, switch::State::Directory { .. }) {
                // Merely observing an unchanged ancestor does not grant writable
                // scope over preserved siblings beneath that directory.
                after.work.push(change.clone());
            }
        }
        let changes = ledger
            .into_values()
            .filter(|change| change.before != change.after || !matches!(change.before, switch::State::Directory { .. }))
            .collect::<Vec<_>>();
        self.state.preservation.validate_scope(
            &changes
                .iter()
                .filter(|change| change.before != change.after)
                .map(|change| change.path.clone())
                .collect::<Vec<_>>(),
        )?;
        after.witness = crate::repository::authoring::conflict::worktree_witness(
            repo,
            &after.work.iter().map(|change| change.path.clone()).collect::<Vec<_>>(),
        )?;
        let public = self.state.preservation.public_index(repo, private)?;
        let mut source = budget.spool()?;
        gix::index::File::from_state(public, repo.index_path()).write_to(&mut source, Default::default())?;
        after.index = self.store_payload(repo, source, 0o644)?;
        after.head = switch::State::File {
            mode: 0o644,
            bytes: format!("{tip}\n").into_bytes(),
        };
        if gix::ObjectId::from_hex(self.state.tip.as_bytes())? != tip
            && repo.config_snapshot().try_boolean("core.logAllRefUpdates")? != Some(false)
        {
            let line = gix::refs::log::Line {
                previous_oid: gix::ObjectId::from_hex(self.state.tip.as_bytes())?,
                new_oid: tip,
                signature: repo
                    .committer()
                    .context("committer required for replay log")??
                    .to_owned()?,
                message: format!("brit rebase {} step {} {:?}", self.state.nonce, self.state.step, kind).into(),
            };
            after.log = self.append_metadata(repo, after.log.clone(), line)?;
        }
        after.tip = tip.to_string();
        let mut metadata = Vec::new();
        for (path, before, desired) in [
            (repo.index_path(), &self.state.index, &after.index),
            (repo.git_dir().join("HEAD"), &self.state.head, &after.head),
            (repo.git_dir().join("logs/HEAD"), &self.state.log, &after.log),
        ] {
            if before != desired {
                metadata.push(switch::Change {
                    path: gix::path::into_bstr(path)?.to_vec(),
                    before: before.clone(),
                    after: desired.clone(),
                });
            }
        }
        Ok((
            after,
            super::transition::Transition {
                kind,
                work: changes,
                metadata,
                branch: self
                    .state
                    .attachment
                    .clone()
                    .map(|name| (name, self.state.original.clone())),
                final_tip: None,
                packed_refs: switch::payload::fingerprint(&repo.common_dir().join("packed-refs"))?,
            },
        ))
    }

    fn apply_decision(&mut self, repo: &gix::Repository, mut decision: Decision, new: bool) -> Result<()> {
        ensure!(
            decision.version == 1 && decision.nonce == self.state.nonce && decision.ordinal == self.state.decisions + 1,
            "foreign replay decision identity"
        );
        ensure!(
            decision.before == switch::digest(&serde_json::to_vec(&self.state)?)?,
            "replay decision has a different parent state"
        );
        self.validate_retention(repo)?;
        ensure!(
            decision.transition.branch
                == self
                    .state
                    .attachment
                    .as_ref()
                    .map(|name| (name.clone(), self.state.original.clone())),
            "decision branch ownership changed"
        );
        switch::validate_local_history(repo)?;
        switch::validate_object_boundaries(repo)?;
        for (label, value) in &decision.after.retained {
            let reference =
                repo.find_reference(format!("refs/brit-rebase-retain/{}/{label}", self.state.nonce).as_str())?;
            ensure!(
                matches!(reference.target(), gix::refs::TargetRef::Object(_))
                    && reference.id() == gix::ObjectId::from_hex(value.as_bytes())?,
                "decision lost its owned retention reference"
            );
        }
        for value in &decision.after.closure {
            let id = gix::ObjectId::from_hex(value.as_bytes())?;
            super::validation::verify_object(repo, id)?;
        }
        sync_tree(&repo.common_dir().join("objects"))?;
        sync_reference_storage(repo)?;
        self.validate_branch(
            repo,
            (decision.transition.kind == super::transition::Kind::Final).then_some(decision.after.tip.as_str()),
        )?;
        ensure!(
            decision.transition.final_tip
                == (decision.transition.kind == super::transition::Kind::Final).then(|| decision.after.tip.clone()),
            "decision final tip ownership changed"
        );
        let staging = self
            .directory
            .join(format!("transition-{}-{}", decision.ordinal, decision.before));
        if new && decision.transition.kind != super::transition::Kind::Abort && !staging.exists() {
            // The publication staged for the largest file is the transient peak.
            let largest = decision
                .transition
                .work
                .iter()
                .filter_map(|change| match &change.after {
                    switch::State::StoredFile { size, .. } => Some(*size),
                    _ => None,
                })
                .max()
                .unwrap_or(0);
            self.reserve(repo, self.decision_bytes(repo)?.saturating_add(largest))?;
        }
        // Past its reservation a final or abort decision is finished unmetered:
        // once it is durable nothing can undo it.
        let _recovery = matches!(
            decision.transition.kind,
            super::transition::Kind::Final | super::transition::Kind::Abort
        )
        .then(switch::payload::Recovery::enter);
        let owner = serde_json::to_vec(&(decision.nonce.as_str(), decision.ordinal, decision.before.as_str()))?;
        match fs::symlink_metadata(&staging) {
            Ok(metadata) => ensure!(
                metadata.is_dir()
                    && metadata.permissions().mode().trailing_zeros() >= 6
                    && metadata.uid() == fs::metadata(&self.directory)?.uid()
                    && fs::symlink_metadata(staging.join("owner"))?.is_file()
                    && fs::read(staging.join("owner"))? == owner,
                "foreign replay transition staging directory"
            ),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                use std::os::unix::fs::DirBuilderExt;
                fs::DirBuilder::new().mode(0o700).create(&staging)?;
                switch::checkpoint("replay-transition-directory-created")?;
                switch::private_write(&staging.join("owner"), &owner)?;
                switch::sync_dir(&staging)?;
                switch::sync_dir(&self.directory)?;
            }
            Err(error) => return Err(error.into()),
        }
        if new {
            let plan = staging.join("plan.json");
            // A proposal cut short while written proposed nothing: no decision
            // record exists yet (`new`), so it is written again below.
            let proposal = if plan.exists() {
                let proposal = torn_record(&fs::read(&plan)?)?;
                if proposal.is_none() {
                    fs::remove_file(&plan)?;
                    switch::sync_dir(&staging)?;
                }
                proposal
            } else {
                None
            };
            if let Some(envelope) = proposal {
                ensure!(
                    switch::digest(&envelope.payload)? == envelope.checksum,
                    "corrupt proposed replay transition"
                );
                let proposed: Decision = serde_json::from_slice(&envelope.payload)?;
                ensure!(
                    proposed.nonce == decision.nonce
                        && proposed.ordinal == decision.ordinal
                        && proposed.before == decision.before
                        && proposed.transition.kind == decision.transition.kind
                        && proposed.after.tip == decision.after.tip,
                    "proposed transition belongs to another action"
                );
                decision = proposed;
                switch::sync_path(&plan)?;
            } else {
                let payload = serde_json::to_vec(&decision)?;
                switch::private_write(
                    &plan,
                    &serde_json::to_vec(&Envelope {
                        checksum: switch::digest(&payload)?,
                        payload,
                    })?,
                )?;
            }
            switch::sync_dir(&staging)?;
            for change in &mut decision.transition.work {
                if let switch::State::Directory { mode, inode: 0, .. } = change.after {
                    let name = switch::digest(&change.path)?;
                    let receipt = staging.join(format!("directory-{name}.json"));
                    let directory = staging.join(format!("new-dir-{name}"));
                    if receipt.exists() {
                        let envelope: Envelope = serde_json::from_slice(&fs::read(&receipt)?)?;
                        ensure!(
                            switch::digest(&envelope.payload)? == envelope.checksum,
                            "corrupt directory allocation receipt"
                        );
                        let (path, state): (Vec<u8>, switch::State) = serde_json::from_slice(&envelope.payload)?;
                        ensure!(
                            path == change.path
                                && matches!(state, switch::State::Directory {mode: saved, ..} if mode == saved)
                                && switch::snapshot(&directory)? == state,
                            "directory allocation identity changed"
                        );
                        switch::sync_path(&directory)?;
                        switch::sync_path(&receipt)?;
                        change.after = state;
                    } else {
                        switch::prepare_directories(&staging, std::slice::from_mut(change))?;
                        switch::checkpoint("replay-transition-directory-allocated")?;
                        let payload = serde_json::to_vec(&(&change.path, &change.after))?;
                        switch::private_write(
                            &receipt,
                            &serde_json::to_vec(&Envelope {
                                checksum: switch::digest(&payload)?,
                                payload,
                            })?,
                        )?;
                    }
                    switch::sync_dir(&staging)?;
                    let tracked = decision
                        .after
                        .work
                        .iter_mut()
                        .find(|tracked| tracked.path == change.path)
                        .context("directory absent from parent footprint")?;
                    tracked.after = change.after.clone();
                }
            }
            switch::checkpoint("replay-directories-prepared")?;
        }
        self.prepare_restored_directories(repo, &staging, &decision.transition.work)?;
        decision.transition.apply(repo, &staging, || {
            if new {
                let payload = serde_json::to_vec(&decision)?;
                let bytes = serde_json::to_vec(&Envelope {
                    checksum: switch::digest(&payload)?,
                    payload,
                })?;
                switch::private_write(
                    &self.directory.join(format!("decision-{}.json", decision.ordinal)),
                    &bytes,
                )?;
                switch::sync_dir(&self.directory)?;
                switch::checkpoint("replay-decision-durable")?;
            }
            Ok(())
        })?;
        self.state = decision.after;
        self.save()?;
        switch::checkpoint("replay-decision-applied")?;
        Ok(())
    }

    fn prepare_restored_directories(
        &self,
        repo: &gix::Repository,
        staging: &Path,
        changes: &[switch::Change],
    ) -> Result<()> {
        for change in changes {
            if !matches!(change.after, switch::State::Directory { .. }) {
                continue;
            }
            let live = repo
                .workdir()
                .context("worktree required")?
                .join(gix::path::from_bstr(change.path.as_bstr())?.as_ref());
            if switch::snapshot(&live)? == change.after {
                continue;
            }
            let digest = switch::digest(&change.path)?;
            let target = staging.join(format!("new-dir-{digest}"));
            if switch::snapshot(&target)? == change.after {
                switch::sync_dir(&target)?;
                switch::sync_dir(staging)?;
                continue;
            }
            let receipt = staging.join(format!("restore-directory-{digest}.json"));
            let source = if receipt.exists() {
                let envelope: Envelope = serde_json::from_slice(&fs::read(&receipt)?)?;
                ensure!(
                    switch::digest(&envelope.payload)? == envelope.checksum,
                    "corrupt directory restoration receipt"
                );
                let (source, state): (Vec<u8>, switch::State) = serde_json::from_slice(&envelope.payload)?;
                ensure!(state == change.after, "directory restoration identity changed");
                switch::sync_path(&receipt)?;
                gix::path::from_bstr(source.as_bstr())?.into_owned()
            } else {
                let mut found = None;
                for ordinal in 1..=self.state.decisions {
                    let envelope: Envelope =
                        serde_json::from_slice(&fs::read(self.directory.join(format!("decision-{ordinal}.json")))?)?;
                    ensure!(
                        switch::digest(&envelope.payload)? == envelope.checksum,
                        "corrupt prior directory decision"
                    );
                    let prior: Decision = serde_json::from_slice(&envelope.payload)?;
                    let source = self
                        .directory
                        .join(format!("transition-{ordinal}-{}", prior.before))
                        .join(format!("saved-dir-{digest}"));
                    if switch::snapshot(&source)? == change.after {
                        ensure!(found.is_none(), "ambiguous original directory identity");
                        found = Some(source);
                    }
                }
                let source = found.context("original directory inode is unavailable")?;
                let payload = serde_json::to_vec(&(gix::path::into_bstr(source.clone())?.to_vec(), &change.after))?;
                switch::private_write(
                    &receipt,
                    &serde_json::to_vec(&Envelope {
                        checksum: switch::digest(&payload)?,
                        payload,
                    })?,
                )?;
                source
            };
            ensure!(
                source.starts_with(&self.directory)
                    && switch::snapshot(&source)? == change.after
                    && switch::snapshot(&target)? == switch::State::Absent,
                "foreign directory restoration source"
            );
            switch::sync_dir(staging)?;
            fs::rename(&source, &target)?;
            switch::sync_dir(source.parent().context("saved directory parent")?)?;
            switch::sync_dir(staging)?;
        }
        Ok(())
    }

    /// Roll forward a durable decision only. This never invents a new decision
    /// from observed HEAD, launches a hook, or resumes a stopped replay.
    pub fn recover_decision(repo: &gix::Repository) -> Result<Self> {
        let _guard = switch::shared_guard(repo)?;
        Self::recover_decision_guarded(repo, false)
    }

    fn recover_decision_guarded(repo: &gix::Repository, aborting: bool) -> Result<Self> {
        switch::validate_local_history(repo)?;
        switch::validate_object_boundaries(repo)?;
        let mut session = Self::open(repo)?;
        let path = session
            .directory
            .join(format!("decision-{}.json", session.state.decisions + 1));
        let metadata = fs::symlink_metadata(&path)?;
        ensure!(
            metadata.is_file()
                && metadata.permissions().mode().trailing_zeros() >= 6
                && metadata.uid() == fs::metadata(&session.directory)?.uid(),
            "unsafe replay decision"
        );
        let envelope: Envelope = match torn_record(&fs::read(&path)?)? {
            Some(envelope) => envelope,
            None => {
                // Cut short while it was written: the decision never became
                // durable, and nothing live is written before it is (see
                // `transition::apply`). The journal is as if it never began.
                fs::remove_file(&path)?;
                switch::sync_dir(&session.directory)?;
                return Ok(session);
            }
        };
        ensure!(
            switch::digest(&envelope.payload)? == envelope.checksum,
            "corrupt replay decision"
        );
        let decision: Decision = serde_json::from_slice(&envelope.payload)?;
        ensure!(
            !aborting || decision.transition.kind != super::transition::Kind::Final,
            "a durable final decision cannot be aborted"
        );
        // A durable decision is rolled forward unmetered when an abort needs it
        // applied first, and when it is final or an abort, which nothing undoes.
        let _recovery = (aborting
            || matches!(
                decision.transition.kind,
                super::transition::Kind::Final | super::transition::Kind::Abort
            ))
        .then(switch::payload::Recovery::enter);
        switch::sync_path(&path)?;
        switch::sync_dir(&session.directory)?;
        session.apply_decision(repo, decision, false)?;
        Ok(session)
    }
}

/// Called by an already-authorized child while it holds the shared guard,
/// before publishing its durable commit decision. The parent state is immutable;
/// the bounded recipe is adopted when that child's receipt is accepted/discarded.
pub(crate) fn protect_child_commit(
    repo: &gix::Repository,
    binding: &ParentBinding,
    ordinal: usize,
    commit: gix::ObjectId,
) -> Result<()> {
    ensure!(ordinal > 0, "child retention requires a positive decision ordinal");
    validate_child_binding(repo, binding)?;
    let session = Session::open(repo)?;
    let mut closure = BTreeSet::new();
    history(repo, commit, &mut closure)?;
    sync_tree(&repo.common_dir().join("objects"))?;
    session.protect(repo, &format!("child-{}-{ordinal}", binding.step), commit, &closure)
}

/// Validate an active journal's ownership before distinguishing it from a
/// previously retired operation. This never adopts an unrecognized directory.
pub(crate) fn active_nonce(repo: &gix::Repository) -> Result<Option<String>> {
    match fs::symlink_metadata(repo.git_dir().join("brit-rebase")) {
        Ok(_) => Ok(Some(Session::open(repo)?.state.nonce)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error.into()),
    }
}

fn sync_reference_storage(repo: &gix::Repository) -> Result<()> {
    for name in ["refs", "packed-refs", "logs"] {
        let path = repo.common_dir().join(name);
        match fs::symlink_metadata(&path) {
            Ok(_) => sync_tree(&path)?,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
    }
    switch::sync_dir(repo.common_dir())
}

fn retained_image<'a>(
    root: &Path,
    area: super::retained::Area,
    path: &[u8],
    image: &switch::State,
) -> Result<super::retained::Member<super::retained::Source<'a>>> {
    use super::retained::{Kind, Member, Source};
    let (kind, mode, data) = match image {
        switch::State::Absent => (Kind::Deleted, 0, Source::Bytes(Vec::new())),
        switch::State::File { mode, bytes } => (Kind::File, *mode, Source::Bytes(bytes.clone())),
        switch::State::StoredFile { mode, size, checksum } => {
            switch::payload::open(root, *size, checksum)?;
            (
                Kind::File,
                *mode,
                Source::File(root.join(format!("payload-{checksum}"))),
            )
        }
        switch::State::Symlink(bytes) => (Kind::Symlink, 0, Source::Bytes(bytes.clone())),
        switch::State::Directory { mode, device, inode } => (
            Kind::Directory {
                device: *device,
                inode: *inode,
            },
            *mode,
            Source::Bytes(Vec::new()),
        ),
    };
    Ok(Member {
        area,
        path: path.to_vec(),
        stage: None,
        kind,
        mode,
        data,
    })
}

fn retained_path<'a>(
    area: super::retained::Area,
    path: &[u8],
    absolute: &Path,
) -> Result<super::retained::Member<super::retained::Source<'a>>> {
    use super::retained::{Kind, Member, Source};
    use std::os::unix::fs::OpenOptionsExt;
    if fs::symlink_metadata(absolute).is_ok_and(|metadata| metadata.is_file()) {
        let file = fs::OpenOptions::new()
            .read(true)
            .custom_flags(rustix::fs::OFlags::NOFOLLOW.bits() as i32)
            .open(absolute)?;
        let mode = file.metadata()?.permissions().mode() & 0o777;
        Ok(Member {
            area,
            path: path.to_vec(),
            stage: None,
            kind: Kind::File,
            mode,
            data: Source::File(absolute.to_owned()),
        })
    } else {
        retained_image(absolute, area, path, &switch::snapshot(absolute)?)
    }
}

fn sync_tree(path: &Path) -> Result<()> {
    ensure!(!gix::interrupt::is_triggered(), "replay durability scan interrupted");
    let metadata = fs::symlink_metadata(path)?;
    if metadata.is_dir() {
        for entry in fs::read_dir(path)? {
            sync_tree(&entry?.path())?;
        }
        switch::sync_dir(path)
    } else {
        ensure!(metadata.is_file(), "unsafe durability path");
        switch::sync_path(path)
    }
}

/// What [`Session::retire`] kept, and where.
#[derive(Debug)]
pub struct Retired {
    /// The operation's nonce.
    pub nonce: String,
    /// `true` when the rebase published `tip`; otherwise its branch still names
    /// `original` and nothing live differs from before the rebase.
    pub published: bool,
    /// The published tip.
    pub tip: String,
    /// The original tip before the rebase.
    pub original: String,
    /// The kept journal directory.
    pub journal: std::path::PathBuf,
    /// Bytes under the kept journal directory.
    pub journal_bytes: u64,
    /// Retention references left in place, with the objects they keep alive.
    pub refs: Vec<(String, String)>,
    /// The operation's retained copies, if any, and their bytes.
    pub copies: Option<(std::path::PathBuf, u64)>,
}

/// The record `save` writes for `state`: its checksummed envelope.
fn record_bytes(state: &ParentState) -> Result<Vec<u8>> {
    let payload = serde_json::to_vec(state)?;
    let bytes = serde_json::to_vec(&Envelope {
        checksum: switch::digest(&payload)?,
        payload,
    })?;
    Ok(bytes)
}

/// Bytes of regular files under `root`, without following links.
pub(crate) fn tree_bytes(root: &Path) -> Result<u64> {
    let mut total = 0u64;
    let mut pending = vec![root.to_owned()];
    while let Some(directory) = pending.pop() {
        for entry in fs::read_dir(directory)? {
            let entry = entry?;
            let metadata = fs::symlink_metadata(entry.path())?;
            if metadata.is_dir() {
                pending.push(entry.path());
            } else {
                total = total.saturating_add(metadata.len());
            }
        }
    }
    Ok(total)
}

/// Slack for one decision's records beyond its state images: lock receipts, the
/// started ledger, retention and proposal records. Measured at about 10 KiB.
const RECORD_SLACK: u64 = 16 * 1024;
/// State images a conflict stop's authoring child adds when it is archived into
/// the journal: its own state and its decision records. Measured at about eight
/// for one staged path and a continue.
const CHILD_IMAGES: u64 = 10;

/// Read one journal record, refusing before allocation a file larger than any
/// record the journal quota lets brit write.
fn read_record(path: &Path) -> Result<Vec<u8>> {
    let size = fs::symlink_metadata(path)?.len();
    ensure!(
        size <= gix::utils::stream::SCRATCH_LIMIT,
        "replay journal record {} is {size} bytes, larger than brit ever writes; inspect it",
        path.display()
    );
    Ok(fs::read(path)?)
}

/// The envelope of a record written in one sequential pass, or `None` when the
/// bytes end before the JSON does: a prefix of the record, written by a process
/// killed part-way or lost with the page cache before its file was synchronized.
/// Any other defect is an error, never mistaken for an interrupted write.
fn torn_record(bytes: &[u8]) -> Result<Option<Envelope>> {
    match serde_json::from_slice(bytes) {
        Ok(envelope) => Ok(Some(envelope)),
        Err(error) if error.is_eof() => Ok(None),
        Err(error) => Err(error.into()),
    }
}

/// Decode one journal record and verify its checksum.
fn decode_state(bytes: &[u8]) -> Result<ParentState> {
    let envelope: Envelope = serde_json::from_slice(bytes)?;
    ensure!(
        switch::digest(&envelope.payload)? == envelope.checksum,
        "replay journal checksum mismatch"
    );
    Ok(serde_json::from_slice(&envelope.payload)?)
}

fn present(path: &Path) -> Result<bool> {
    match fs::symlink_metadata(path) {
        Ok(_) => Ok(true),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error.into()),
    }
}

/// `index.pre-rebase`, or a `pre-rebase` invocation prepared but never launched:
/// its directory holds no launch or exit receipt, only its own start records.
fn unlaunched_operation_artifact(path: &Path, owner: u32) -> Result<bool> {
    let metadata = fs::symlink_metadata(path)?;
    let private = metadata.uid() == owner && metadata.permissions().mode().trailing_zeros() >= 6;
    match path.file_name().and_then(|name| name.to_str()) {
        Some("index.pre-rebase") => Ok(metadata.is_file() && private),
        Some("pre-rebase") if metadata.is_dir() && private => {
            for member in fs::read_dir(path)? {
                let member = member?;
                if !(["lease", "checkpoint", "start.json", "start.json.pending"]
                    .iter()
                    .any(|name| member.file_name() == *name)
                    && owned_private_file(&member.path(), owner)?)
                {
                    return Ok(false);
                }
            }
            Ok(true)
        }
        _ => Ok(false),
    }
}

/// A private regular file this user owns, as every journal member is written.
fn owned_private_file(path: &Path, owner: u32) -> Result<bool> {
    let metadata = fs::symlink_metadata(path)?;
    Ok(metadata.is_file() && metadata.uid() == owner && metadata.permissions().mode().trailing_zeros() >= 6)
}

fn raw(path: &Path) -> Result<Vec<u8>> {
    Ok(gix::path::into_bstr(fs::canonicalize(path)?)?.to_vec())
}

// Todo/editor bookkeeping is not mutation authority for an existing child.
// Every other parent field, including cursor/group/stop digest, remains frozen.
fn child_state_digest(state: &ParentState) -> Result<String> {
    let mut authorization = state.clone();
    if let Some(interactive) = &mut authorization.interactive {
        interactive.todo.bytes.clear();
        let boundary = interactive.cursor + usize::from(state.phase == "authoring" && interactive.edit.is_none());
        interactive.todo.entries.truncate(boundary);
        interactive.editors.clear();
        interactive.initial_accepted = false;
    }
    switch::digest(&serde_json::to_vec(&authorization)?)
}

pub(crate) fn validate_child_binding(repo: &gix::Repository, binding: &ParentBinding) -> Result<()> {
    ensure!(
        repo.state().is_none(),
        "foreign Git operation while replay child is active"
    );
    let owner = fs::canonicalize(repo.git_dir())?;
    for worktree in repo.worktrees_including_main()? {
        let worktree = worktree?;
        let git_dir = fs::canonicalize(worktree.current_dir().join(worktree.git_dir()))?;
        for marker in ["brit-switch", "brit-rebase", "brit-authoring"] {
            match fs::symlink_metadata(git_dir.join(marker)) {
                Ok(_) => ensure!(
                    git_dir == owner && marker != "brit-switch",
                    "competing operation while replay child is active"
                ),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => return Err(error.into()),
            }
        }
    }
    let directory = repo.git_dir().join("brit-rebase");
    let meta = fs::symlink_metadata(&directory)?;
    ensure!(
        meta.is_dir()
            && meta.permissions().mode().trailing_zeros() >= 6
            && meta.uid() == fs::metadata(repo.git_dir())?.uid(),
        "unsafe replay parent directory"
    );
    let path = directory.join("state.json");
    let file = fs::symlink_metadata(&path)?;
    ensure!(
        file.is_file() && file.permissions().mode().trailing_zeros() >= 6 && file.uid() == meta.uid(),
        "unsafe replay parent state"
    );
    let envelope: Envelope = serde_json::from_slice(&fs::read(path)?)?;
    let mut hash = gix::hash::hasher(gix::hash::Kind::Sha1);
    hash.update(&envelope.payload);
    let digest = hash.try_finalize()?.to_string();
    ensure!(digest == envelope.checksum, "replay parent state binding changed");
    let state: ParentState = serde_json::from_slice(&envelope.payload)?;
    ensure!(
        child_state_digest(&state)? == binding.parent_state,
        "replay child authorization context changed"
    );
    ensure!(
        matches!(state.version, 1 | 2)
            && state.nonce == binding.nonce
            && state.decisions == binding.step
            && state.phase == "authoring",
        "replay parent does not authorize this child"
    );
    let worktree = repo.workdir().context("worktree required")?;
    ensure!(
        state.owner == raw(repo.git_dir())?
            && state.worktree == raw(worktree)?
            && state.common == raw(repo.common_dir())?,
        "replay parent repository identity mismatch"
    );
    let identities: Vec<_> = [repo.git_dir(), worktree, repo.common_dir(), directory.as_path()]
        .into_iter()
        .map(|path| fs::symlink_metadata(path).map(|m| (m.dev(), m.ino())))
        .collect::<std::io::Result<_>>()?;
    ensure!(state.identities == identities, "replay parent inode identity mismatch");
    switch::validate_local_history(repo)?;
    switch::validate_object_boundaries(repo)?;
    ensure!(
        super::hooks::policy(repo)? == state.policy,
        "replay parent configuration or hooks changed"
    );
    let parent = Session { directory, state };
    parent.inspect_editor()?;
    parent.validate_retention(repo)?;
    parent.validate_branch(repo, None)?;
    Ok(())
}

pub(crate) fn validate_accepted_child(repo: &gix::Repository, binding: &ParentBinding, receipt: &str) -> Result<()> {
    let session = Session::open_record(repo, "state.json")?;
    let expected = serde_json::to_vec(binding)?;
    ensure!(
        session
            .state
            .accepted_child
            .iter()
            .chain(session.state.accepted_children.iter())
            .any(
                |(accepted, digest)| serde_json::to_vec(accepted).is_ok_and(|bytes| bytes == expected)
                    && digest == receipt
            ),
        "parent accepted a different child receipt"
    );
    Ok(())
}

fn retain_journal_members(
    root: &Path,
    directory: &Path,
    members: &mut Vec<super::retained::Member<super::retained::Source<'_>>>,
) -> Result<()> {
    let owner = fs::metadata(root)?.uid();
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        let path = entry.path();
        let metadata = fs::symlink_metadata(&path)?;
        ensure!(metadata.uid() == owner, "unsafe replay journal member owner");
        // Capture is not ownership adoption: the enclosing operation directory
        // is private, and orphan members may have ordinary work-file modes.
        // Snapshot symlink payloads without following them and never read devices.
        let relative = gix::path::into_bstr(path.strip_prefix(root)?.to_path_buf())?.to_vec();
        members.push(retained_path(super::retained::Area::Invocations, &relative, &path)?);
        if metadata.is_dir() {
            retain_journal_members(root, &path, members)?;
        }
    }
    Ok(())
}

fn validate_scope_ancestors(root: &Path, path: &[u8], source: &gix::index::State) -> Result<()> {
    let relative = gix::path::from_bstr(path.as_bstr())?;
    let mut ancestor = std::path::PathBuf::new();
    let parts = relative.components().collect::<Vec<_>>();
    for part in parts.iter().take(parts.len().saturating_sub(1)) {
        ensure!(!gix::interrupt::is_triggered(), "replay preparation interrupted");
        ensure!(matches!(part, std::path::Component::Normal(_)), "invalid replay path");
        ancestor.push(part.as_os_str());
        let bytes = gix::path::into_bstr(ancestor.clone())?;
        let state = switch::snapshot(&root.join(&ancestor))?;
        ensure!(
            matches!(state, switch::State::Absent | switch::State::Directory { .. })
                || (matches!(state, switch::State::File { .. }) && source.entry_by_path(bytes.as_bstr()).is_some()),
            "unsafe or untracked replay ancestor"
        );
        ensure!(
            !root.join(&ancestor).join(".git").exists(),
            "nested repository in replay footprint"
        );
    }
    Ok(())
}

fn validate_original_directory(root: &Path, relative: &[u8], source: &gix::index::State) -> Result<()> {
    for entry in fs::read_dir(root.join(gix::path::from_bstr(relative.as_bstr())?.as_ref()))? {
        ensure!(!gix::interrupt::is_triggered(), "replay preparation interrupted");
        let entry = entry?;
        let path = gix::path::into_bstr(entry.path().strip_prefix(root)?.to_path_buf())?;
        if entry.file_type()?.is_dir() {
            ensure!(
                source
                    .entries()
                    .iter()
                    .any(|entry| entry.path(source).starts_with(path.as_bytes())
                        && entry.path(source).get(path.len()) == Some(&b'/')),
                "untracked directory collides with replay footprint"
            );
            validate_original_directory(root, path.as_bytes(), source)?;
        } else {
            ensure!(
                source.entry_by_path(path.as_bstr()).is_some(),
                "untracked or ignored descendant collides with replay footprint"
            );
        }
    }
    Ok(())
}

impl Session {
    pub fn has_pending_editor(&self) -> bool {
        self.state
            .interactive
            .as_ref()
            .is_some_and(|i| i.editors.last().is_some_and(|editor| !editor.accepted))
    }

    /// Read-only indication; recovery revalidates the complete decision under guard.
    pub fn has_pending_decision(&self) -> bool {
        self.directory
            .join(format!("decision-{}.json", self.state.decisions + 1))
            .exists()
    }

    /// Inspect the exact authorized child without accepting or recovering it.
    pub fn has_pending_child_decision(&self, repo: &gix::Repository) -> Result<bool> {
        let authorization = self.child_authorization()?;
        let child = crate::repository::authoring::conflict::Session::open_owned(repo, &authorization)?;
        Ok(child.has_pending_publication())
    }

    pub fn show_current_patch(&self, repo: &gix::Repository, out: &mut dyn std::io::Write) -> Result<()> {
        let original = if self.state.interactive.as_ref().is_some_and(|i| i.edit.is_some())
            && matches!(self.phase(), "authoring" | "retaining-skip" | "retaining-abort")
        {
            self.state
                .interactive
                .as_ref()
                .and_then(|i| i.edit.as_ref())
                .map(|edit| edit.original.clone())
        } else if matches!(
            self.phase(),
            "authoring" | "empty" | "step-proposed" | "message" | "message-ready" | "retaining-skip"
        ) {
            Some(self.current_candidate()?.commit.to_string())
        } else {
            None
        };
        if let Some(original) = original {
            let id = gix::ObjectId::from_hex(original.as_bytes())?;
            writeln!(out, "commit {id}")?;
            crate::repository::diff::pinned_commit(repo, out, id).map_err(|e| anyhow::anyhow!("{e}"))?;
        } else {
            writeln!(out, "No current patch.")?;
        }
        Ok(())
    }

    pub fn child_status(&self, repo: &gix::Repository, out: &mut dyn std::io::Write) -> Result<()> {
        if self.phase() == "authoring" {
            let authorization = self.child_authorization()?;
            match crate::repository::authoring::conflict::Session::open_owned(repo, &authorization) {
                Ok(child) => child.status(out)?,
                Err(error) => writeln!(out, "Child inspection unavailable (no recovery attempted): {error:#}")?,
            }
        }
        Ok(())
    }

    /// Read-only status deliberately does not validate mutable stop work or recover.
    pub fn status(&self, out: &mut dyn std::io::Write) -> Result<()> {
        writeln!(out, "Rebase phase: {}", self.phase())?;
        writeln!(
            out,
            "Inputs consumed: {}/{}; replay tip: {}",
            self.state.step,
            self.state.candidates.len(),
            self.state.tip
        )?;
        writeln!(
            out,
            "Original HEAD: {}; upstream: {}",
            self.state.original, self.state.upstream
        )?;
        if self.has_pending_decision() {
            let path = self
                .directory
                .join(format!("decision-{}.json", self.state.decisions + 1));
            let kind = fs::read(&path).ok().and_then(|bytes| match torn_record(&bytes) {
                Ok(Some(envelope)) => Some(
                    serde_json::from_slice::<Decision>(&envelope.payload)
                        .map(|decision| decision.transition.kind)
                        .ok(),
                ),
                Ok(None) => None,
                Err(_) => Some(None),
            });
            let line = match kind {
                None => "An interrupted command left its decision record incomplete; it never became durable, and \
                         the next --continue or --abort discards it and goes on from the state above."
                    .to_owned(),
                Some(Some(super::transition::Kind::Final)) => {
                    "Durable final decision pending: --continue publishes the rebase; a final decision cannot be \
                     aborted."
                        .to_owned()
                }
                Some(Some(super::transition::Kind::Abort)) => {
                    "Durable abort decision pending: --abort (or --continue) finishes restoring the original state."
                        .to_owned()
                }
                Some(Some(_)) => "Durable decision pending: --continue applies it and carries the rebase on; --abort \
                                  applies it, then restores the original state."
                    .to_owned(),
                Some(None) => {
                    format!("Decision record {path:?} does not decode; --continue and --abort refuse it; inspect it.")
                }
            };
            writeln!(out, "{line}")?;
        }
        if self.directory.join("state.json").exists() && self.directory.join("state.pending").exists() {
            writeln!(
                out,
                "An interrupted command left an unrenamed successor record (state.pending); it never took effect. \
                 The state above is the journal's, and the next --continue, --skip or --abort discards it."
            )?;
        }
        if self.phase() == "authoring" {
            if self.is_edit_stop() {
                writeln!(
                    out,
                    "Edit stop: use --stage, --amend or --begin-split/--commit, then --continue."
                )?;
            } else {
                writeln!(
                    out,
                    "Conflict stop: stage 2 (ours) is replay/upstream; stage 3 (theirs) is the original input. Resolve, --stage, then --continue; or --skip/--abort."
                )?;
            }
        }
        if self.phase() == "empty" {
            writeln!(
                out,
                "Empty stop: --continue keeps an empty commit; --skip drops this input."
            )?;
        }
        if self.phase() == "preparing" && self.state.preservation.gitlink_paths().next().is_none() {
            writeln!(
                out,
                "Preparation did not finish and changed nothing live: --abort cancels it without reading objects; --continue retries it."
            )?;
        } else if matches!(self.phase(), "preparing" | "cancelling-prepared") {
            writeln!(
                out,
                "Preparation did not finish and changed nothing live: --abort cancels it without reading objects."
            )?;
        }
        if matches!(self.phase(), "pre-unknown" | "invoking-pre")
            || (self.phase() == "prepared" && self.directory.join("pre-rebase").try_exists()?)
        {
            writeln!(
                out,
                "Pre-rebase outcome may be unknown: inspect or --abort; never rerun the hook."
            )?;
        }
        if matches!(self.phase(), "invoking-post" | "post-unknown")
            || (self.phase() == "final-published" && self.directory.join("post-rewrite").try_exists()?)
        {
            writeln!(
                out,
                "Rebase published; --continue acknowledges the post-hook outcome without rerun."
            )?;
        }
        // Phases the lines above give no cure for: a command stopped between steps.
        let cure = match self.phase() {
            "prepared" | "pre-approved" | "pre-rejected" | "step-proposed" | "replaying" | "ready-final" | "break"
            | "message" | "message-ready" | "edit-accepted" => {
                Some("Stopped between steps: --continue carries the rebase on; --abort restores the original state.")
            }
            "retaining-skip" => Some(
                "A skip stopped while retaining its copy: --continue (or --skip) finishes the skip; --abort restores \
                 the original state.",
            ),
            "retaining-abort" => Some("An abort stopped part-way: --abort finishes restoring the original state."),
            "aborted" => Some(
                "The original state is restored: --abort finishes removing the journal; if it refuses a member it \
                 cannot prove its own, --retire sets the journal aside, deleting nothing.",
            ),
            "completed" | "final-published" => {
                Some("The rebase is published and cannot be aborted: --continue finishes it and removes the journal.")
            }
            _ => None,
        };
        if let Some(cure) = cure
            && !(self.phase() == "prepared" && self.directory.join("pre-rebase").try_exists()?)
            && !(self.phase() == "final-published" && self.directory.join("post-rewrite").try_exists()?)
        {
            writeln!(out, "{cure}")?;
        }
        if let Some(outcome) = self.state.post_outcome {
            writeln!(out, "Post-rewrite outcome: {outcome:?}")?;
        }
        if let Some(editor) = self.state.interactive.as_ref().and_then(|i| i.editors.last()) {
            writeln!(
                out,
                "Saved editor output: {:?}; accepted: {}; recorded outcome: {:?}",
                self.editor_output(),
                editor.accepted,
                editor.outcome
            )?;
            if !editor.accepted {
                writeln!(
                    out,
                    "--continue explicitly accepts validated saved bytes without relaunch."
                )?;
            }
        }
        for number in 1..=self.state.copies {
            let path = self
                .directory
                .parent()
                .context("Git directory")?
                .join("brit-rebase-retained")
                .join(&self.state.nonce)
                .join(number.to_string());
            writeln!(out, "Retained copy candidate (inspect completeness): {:?}", path)?;
        }
        writeln!(out, "Journal: {:?}", self.directory)?;
        Ok(())
    }
}
