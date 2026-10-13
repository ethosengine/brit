use super::*;
use crate::repository::authoring::conflict::ParentBinding;
use crate::repository::{authoring::conflict::Preservation, switch};
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
    ) -> Result<()> {
        self.validate(repo)?;
        let envelope: Envelope = serde_json::from_slice(&fs::read(repo.git_dir().join("brit-rebase/state.json"))?)?;
        let state: ParentState = serde_json::from_slice(&envelope.payload)?;
        ensure!(
            state.authorized_stop.as_ref() == Some(&stop_digest(preservation, kind, active, private)?),
            "replay parent does not authorize supplied child context"
        );
        Ok(())
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
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Advance {
    Started,
    Step { commit: gix::ObjectId },
    Empty,
    Conflict,
    ReadyFinal,
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
        use std::{io::Read, os::unix::fs::DirBuilderExt};
        let selection = Selection::prepare(repo, argument)?;
        if selection.kind == SelectionKind::Noop {
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
        let preservation = Preservation::capture(repo)?;
        let active = selection
            .footprint(repo)?
            .into_iter()
            .map(|p| p.to_vec())
            .collect::<Vec<_>>();
        preservation.validate_scope(&active)?;
        let root = repo.workdir().context("worktree required")?;
        let original_tree = repo.index_from_tree(&repo.find_commit(selection.original)?.tree_id()?)?;
        let witness = crate::repository::authoring::conflict::worktree_witness(repo, &active)?;
        let mut work = Vec::new();
        for path in active {
            validate_scope_ancestors(root, &path, &original_tree)?;
            let image = switch::snapshot(&root.join(gix::path::from_bstr(path.as_bstr())?.as_ref()))?;

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
        fs::DirBuilder::new().mode(0o700).create(&directory)?;
        let identities = [repo.git_dir(), root, repo.common_dir(), directory.as_path()]
            .into_iter()
            .map(|path| fs::symlink_metadata(path).map(|m| (m.dev(), m.ino())))
            .collect::<std::io::Result<_>>()?;
        let state = ParentState {
            version: 1,
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
            index: switch::snapshot(&repo.index_path())?,
            head: switch::snapshot(&repo.git_dir().join("HEAD"))?,
            log: switch::snapshot(&repo.git_dir().join("logs/HEAD"))?,
            original_log: switch::snapshot(&repo.git_dir().join("logs/HEAD"))?,
            original_index_state: switch::snapshot(&repo.index_path())?,
            original_head_state: switch::snapshot(&repo.git_dir().join("HEAD"))?,
            original_orig_head: switch::snapshot(&repo.git_dir().join("ORIG_HEAD"))?,
            original_branch_log: selection
                .attachment
                .as_ref()
                .map(|name| {
                    switch::snapshot(
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
        };
        let mut session = Self { directory, state };
        session.validate_branch(repo, None)?;
        session.save()?;
        switch::sync_dir(repo.git_dir())?;
        switch::checkpoint("replay-prepared")?;
        session.complete_preparation(repo)?;
        Ok(Some(session))
    }

    fn complete_preparation(&mut self, repo: &gix::Repository) -> Result<()> {
        switch::validate_local_history(repo)?;
        switch::validate_object_boundaries(repo)?;
        ensure!(self.state.phase == "preparing", "preparation already completed");
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
            let id = gix::ObjectId::from_hex(value.as_bytes())?;
            let object = repo.find_object(id)?;
            ensure!(
                gix::objs::compute_hash(repo.object_hash(), object.kind, &object.data)? == id,
                "pinned closure checksum mismatch"
            );
        }
        for (label, value) in &self.state.retained {
            let name = format!("refs/brit-rebase-retain/{}/{label}", self.state.nonce);
            let id = gix::ObjectId::from_hex(value.as_bytes())?;
            match repo.try_find_reference(name.as_str())? {
                Some(reference) => ensure!(
                    matches!(reference.target(), gix::refs::TargetRef::Object(_)) && reference.id() == id,
                    "foreign replay retention reference"
                ),
                None => {
                    repo.reference(name, id, gix::refs::transaction::PreviousValue::MustNotExist, "")?;
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
        let excluded = self.state.work.iter().map(|c| c.path.clone()).collect::<Vec<_>>();
        ensure!(
            crate::repository::authoring::conflict::worktree_witness(repo, &excluded)? == self.state.witness,
            "worktree changed outside replay scope"
        );
        self.state.preservation.validate_observed(repo, &*repo.open_index()?)?;
        Ok(())
    }

    /// Execute at most one logical replay step. READY_FINAL is deliberately
    /// separate so callers can still abort every completed step before final
    /// publication is decided.
    pub fn advance(&mut self, repo: &gix::Repository) -> Result<Advance> {
        if self.state.phase == "prepared" {
            {
                let _guard = switch::shared_guard(repo)?;
                self.validate_current(repo)?;
                let index = repo.index_from_tree(
                    &repo
                        .find_commit(gix::ObjectId::from_hex(self.state.original.as_bytes())?)?
                        .tree_id()?,
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
        let _guard = switch::shared_guard(repo)?;
        self.validate_current(repo)?;
        if self.state.phase == "pre-approved" {
            let tip = gix::ObjectId::from_hex(self.state.upstream.as_bytes())?;
            let tree = repo.find_commit(tip)?.tree_id()?.detach();
            let private = repo.index_from_tree(&tree)?;
            let (mut after, transition) =
                self.checkout_transition(repo, tree, &private, tip, super::transition::Kind::Checkout)?;
            after.phase = "replaying".into();
            self.publish_decision(repo, after, transition)?;
            return Ok(Advance::Started);
        }
        if self.state.phase == "step-proposed" {
            return self.finish_proposal(repo);
        }
        ensure!(
            matches!(self.state.phase.as_str(), "replaying" | "empty"),
            "replay is stopped; use the action named by its phase"
        );
        if self.state.step == self.state.candidates.len() {
            self.state.phase = "ready-final".into();
            self.save()?;
            return Ok(Advance::ReadyFinal);
        }
        let (original, parent) = &self.state.candidates[self.state.step];
        let candidate = Candidate {
            commit: gix::ObjectId::from_hex(original.as_bytes())?,
            parent: gix::ObjectId::from_hex(parent.as_bytes())?,
        };
        let step = Step::merge(repo, &candidate, self.tip()?)?;
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
        let path = self.directory.join(format!("proposal-{}.json", self.state.step));
        match fs::symlink_metadata(&path) {
            Ok(metadata) => ensure!(
                metadata.is_file()
                    && metadata.permissions().mode().trailing_zeros() >= 6
                    && metadata.uid() == fs::metadata(&self.directory)?.uid(),
                "unsafe saved replay proposal"
            ),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                let proposal = step.propose(repo)?;
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
        let (commit, parent) = &self.state.candidates[self.state.step];
        let step = Step::merge(
            repo,
            &Candidate {
                commit: gix::ObjectId::from_hex(commit.as_bytes())?,
                parent: gix::ObjectId::from_hex(parent.as_bytes())?,
            },
            self.tip()?,
        )?;
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
            aborting || retained.pending_hook.is_none(),
            "resolve or acknowledge the child hook before skipping; unknown pre-hook permits only abort"
        );
        self.state.accepted_child = Some((retained.binding, retained.receipt));
        self.state
            .closure
            .extend(retained.objects.iter().map(ToString::to_string));
        self.state.closure.sort();
        self.state.closure.dedup();
        self.state.index = switch::snapshot(&repo.index_path())?;
        let root = repo.workdir().context("worktree required")?;
        for change in &mut self.state.work {
            change.after = switch::snapshot(&root.join(gix::path::from_bstr(change.path.as_bstr())?.as_ref()))?;
        }
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
        let authorization = self.child_authorization()?;
        let mut child = crate::repository::authoring::conflict::Session::open_owned(repo, &authorization)?;
        child.stage(repo, paths, expand)
    }

    /// Import only validated external staging, then accept the exact child
    /// receipt before replacing its live journal with an owned archive.
    pub fn continue_conflict(&mut self, repo: &gix::Repository) -> Result<Advance> {
        let authorization = self.child_authorization()?;
        let mut child = crate::repository::authoring::conflict::Session::open_owned(repo, &authorization)?;
        child.import_external(repo)?;
        let _guard = switch::shared_guard(repo)?;
        self.validate_current(repo)?;
        let completion = child.owned_completion(repo, &authorization)?;
        ensure!(completion.head == self.tip()?, "conflict authoring moved replay HEAD");
        let tree = child.private_tree(repo)?;
        let step = Step {
            original: gix::ObjectId::from_hex(self.state.candidates[self.state.step].0.as_bytes())?,
            parent: self.tip()?,
            tree,
            index: gix::index::File::from_state(completion.private, repo.index_path()),
            conflicts: Vec::new(),
            footprint: completion.active,
            initially_empty: false,
            newly_empty: false,
        };
        let path = self.directory.join(format!("proposal-{}.json", self.state.step));
        if !path.exists() {
            let proposal = step.propose(repo)?;
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
        self.state.index = switch::snapshot(&repo.index_path())?;
        let root = repo.workdir().context("worktree required")?;
        for change in &mut self.state.work {
            change.after = switch::snapshot(&root.join(gix::path::from_bstr(change.path.as_bstr())?.as_ref()))?;
        }
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
                let mut after = self.state.clone();
                after.phase = "final-published".into();
                let mut metadata = Vec::new();
                if let Some(name) = &self.state.attachment {
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
                        let mut bytes = match &before {
                            switch::State::Absent => Vec::new(),
                            switch::State::File { bytes, .. } => bytes.clone(),
                            _ => anyhow::bail!("unsafe original branch log"),
                        };
                        gix::refs::log::Line {
                            previous_oid: gix::ObjectId::from_hex(self.state.original.as_bytes())?,
                            new_oid: self.tip()?,
                            signature: repo
                                .committer()
                                .context("committer required for final reflog")??
                                .to_owned()?,
                            message: format!("brit rebase {} finish", self.state.nonce).into(),
                        }
                        .write_to(&mut bytes)?;
                        let desired = switch::State::File { mode: 0o644, bytes };
                        after.published_branch_log = Some(desired.clone());
                        metadata.push(switch::Change {
                            path: gix::path::into_bstr(path)?.to_vec(),
                            before,
                            after: desired,
                        });
                    }
                }
                if repo.config_snapshot().try_boolean("core.logAllRefUpdates")? != Some(false) {
                    let mut bytes = match &self.state.log {
                        switch::State::Absent => Vec::new(),
                        switch::State::File { bytes, .. } => bytes.clone(),
                        _ => anyhow::bail!("unsafe HEAD reflog"),
                    };
                    gix::refs::log::Line {
                        previous_oid: self.tip()?,
                        new_oid: self.tip()?,
                        signature: repo
                            .committer()
                            .context("committer required for final reflog")??
                            .to_owned()?,
                        message: format!("brit rebase {} finish", self.state.nonce).into(),
                    }
                    .write_to(&mut bytes)?;
                    after.log = switch::State::File { mode: 0o644, bytes };
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
                    packed_refs: switch::snapshot(&repo.common_dir().join("packed-refs"))?,
                };
                self.publish_decision(repo, after, transition)?;
            }
            self.validate_published_resources(repo)?;
        }
        let mut mapping = Vec::new();
        for (old, results) in &self.state.mappings {
            for new in results {
                mapping.extend_from_slice(format!("{old} {new}\n").as_bytes());
            }
        }
        let invocation = self.directory.join("post-rewrite");
        let index = self.directory.join("index.post-rewrite");
        let private = repo.index_from_tree(&repo.find_commit(self.tip()?)?.tree_id()?)?;
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
            repo.head_commit()?.id == self.tip()?,
            "published branch or detached HEAD moved from final tip"
        );
        ensure!(
            switch::snapshot(&repo.git_dir().join("HEAD"))? == self.state.head
                && switch::snapshot(&repo.index_path())? == self.state.index
                && switch::snapshot(&repo.git_dir().join("logs/HEAD"))? == self.state.log,
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
                switch::snapshot(
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
                switch::snapshot(&root.join(gix::path::from_bstr(change.path.as_bstr())?.as_ref()))? == change.after,
                "published replay worktree changed"
            );
        }
        Ok(())
    }

    /// Acknowledge an unknown post-rewrite result without launching it again.
    pub fn acknowledge_post_hook(&mut self, repo: &gix::Repository) -> Result<()> {
        let _guard = switch::shared_guard(repo)?;
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

    fn adopt_abandoned_plan(&mut self) -> Result<()> {
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
                // A private preparation directory can become visible before its
                // ownership recipe. No live write precedes the decision, so abort
                // can restore the pinned parent without adopting this orphan.
                // The retained copy preserves its bytes; cleanup's exact inventory
                // deliberately continues to refuse the unproved directory.
                for member in ["owner", "plan.json"] {
                    match fs::symlink_metadata(directory.join(member)) {
                        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
                        Err(error) => return Err(error.into()),
                        Ok(_) => {}
                    }
                }
                ensure!(
                    fs::read(directory.join("owner"))?
                        == serde_json::to_vec(&(self.state.nonce.as_str(), ordinal, before.as_str()))?,
                    "foreign proposed transition owner"
                );
                let bytes = fs::read(directory.join("plan.json"))?;
                let envelope: Envelope = serde_json::from_slice(&bytes)?;
                ensure!(
                    switch::digest(&envelope.payload)? == envelope.checksum,
                    "corrupt abandoned plan"
                );
                let plan: Decision = serde_json::from_slice(&envelope.payload)?;
                ensure!(
                    plan.before == before && plan.ordinal == ordinal && plan.nonce == self.state.nonce,
                    "abandoned plan belongs to another parent state"
                );
                self.state.abandoned_plans.push((name, switch::digest(&bytes)?));
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
        Ok(())
    }

    fn adopt_retention(&mut self, repo: &gix::Repository) -> Result<()> {
        for step in 0..self.state.candidates.len() {
            for label in [step.to_string(), format!("stop-{step}")] {
                let path = self.directory.join(format!("retention-{label}.json"));
                if !path.exists() {
                    continue;
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
                match repo.find_object(id)?.kind {
                    gix::objs::Kind::Commit => {
                        history(repo, id, &mut closure)?;
                    }
                    gix::objs::Kind::Tree => {
                        validate_tree(repo, id, &mut BTreeSet::new(), &mut closure)?;
                    }
                    _ => anyhow::bail!("invalid retained root kind"),
                }
                ensure!(
                    closure.iter().map(ToString::to_string).collect::<Vec<_>>() == values,
                    "retention closure changed"
                );
                sync_tree(&repo.common_dir().join("objects"))?;
                self.protect(repo, &label, id, &closure)?;
                if let Some((_, expected)) = self.state.retained.iter().find(|(name, _)| name == &label) {
                    ensure!(expected == &value, "retention ledger changed");
                } else {
                    self.state.retained.push((label, value));
                }
                self.state.closure.extend(values);
            }
        }
        self.state.closure.sort();
        self.state.closure.dedup();
        Ok(())
    }

    fn retain_stop(&self, repo: &gix::Repository, after: &mut ParentState, tree: gix::ObjectId) -> Result<()> {
        let mut closure = BTreeSet::new();
        validate_tree(repo, tree, &mut BTreeSet::new(), &mut closure)?;
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
                step: self.state.step,
                parent_state: switch::digest(&serde_json::to_vec(&self.state)?)?,
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
        let original = repo.index_from_tree(
            &repo
                .find_commit(gix::ObjectId::from_hex(self.state.original.as_bytes())?)?
                .tree_id()?,
        )?;
        for path in active
            .iter()
            .filter(|p| !self.state.work.iter().any(|c| c.path == **p))
            .cloned()
            .collect::<Vec<_>>()
        {
            validate_scope_ancestors(root, &path, &original)?;
            let image = switch::snapshot(&root.join(gix::path::from_bstr(path.as_bstr())?.as_ref()))?;
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
            decoded.parents().collect::<Vec<_>>() == [self.tip()?],
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
        after.mappings.push((
            self.state.candidates[self.state.step].0.clone(),
            vec![commit.to_string()],
        ));
        after.step += 1;
        after.stop_tree = None;
        if let Some(receipt) = after.accepted_child.take() {
            after.accepted_children.push(receipt);
        }
        after.authorized_stop = None;
        after.closure.extend(closure.iter().map(ToString::to_string));
        after.closure.sort();
        after.closure.dedup();
        self.publish_decision(repo, after, transition)?;
        Ok(Advance::Step { commit })
    }

    /// Skip only the current input, with durable intent and retained bytes
    /// before restoring its entry tree.
    pub fn skip(&mut self, repo: &gix::Repository) -> Result<super::retained::Receipt> {
        let _guard = switch::shared_guard(repo)?;
        self.validate_current(repo)?;
        ensure!(
            matches!(self.state.phase.as_str(), "empty" | "authoring" | "retaining-skip"),
            "skip requires a current replay input"
        );
        ensure!(
            self.state.step < self.state.candidates.len(),
            "no input remains to skip"
        );
        if self.state.phase != "retaining-skip" {
            self.adopt_abandoned_plan()?;
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
        let tree = repo.find_commit(self.tip()?)?.tree_id()?.detach();
        let private = repo.index_from_tree(&tree)?;
        let (mut after, transition) =
            self.checkout_transition(repo, tree, &private, self.tip()?, super::transition::Kind::Skip)?;
        after
            .mappings
            .push((self.state.candidates[self.state.step].0.clone(), Vec::new()));
        after.step += 1;
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
        let current = Self::open(repo)?;
        ensure!(
            serde_json::to_vec(&current.state)? == serde_json::to_vec(&self.state)?,
            "stale replay session"
        );
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
                    | "replaying"
                    | "ready-final"
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
            self.adopt_abandoned_plan()?;
        }
        if self.state.phase == "authoring" && repo.git_dir().join("brit-authoring").exists() {
            self.accept_discard_child(repo, true)?;
        }
        self.validate_branch(repo, None)?;
        ensure!(
            switch::snapshot(&repo.git_dir().join("HEAD"))? == self.state.head
                && switch::snapshot(&repo.index_path())? == self.state.index
                && switch::snapshot(&repo.git_dir().join("logs/HEAD"))? == self.state.log,
            "foreign replay metadata before abort"
        );
        self.state.preservation.validate_observed(repo, &*repo.open_index()?)?;
        let root = repo.workdir().context("worktree required")?;
        let hook_stopped = matches!(
            self.state.phase.as_str(),
            "invoking-pre" | "pre-unknown" | "pre-rejected" | "pre-approved"
        );
        for change in &mut self.state.work {
            let observed = switch::snapshot(&root.join(gix::path::from_bstr(change.path.as_bstr())?.as_ref()))?;
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
                packed_refs: switch::snapshot(&repo.common_dir().join("packed-refs"))?,
            },
        )?;
        Ok(receipt)
    }

    /// Retire only a terminal operation, after an independent retained copy
    /// proves every object whose private retention reference is released.
    pub fn cleanup(&mut self, repo: &gix::Repository) -> Result<super::retained::Receipt> {
        let _guard = switch::shared_guard(repo)?;
        ensure!(
            matches!(self.state.phase.as_str(), "aborted" | "completed"),
            "replay is not terminal"
        );
        if self.state.phase == "completed" {
            ensure!(
                repo.head_commit()?.id == self.tip()?
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
        super::cleanup::resume(repo, receipt)
    }

    fn cleanup_inventory(&self, repo: &gix::Repository) -> Result<BTreeSet<Vec<u8>>> {
        let mut allowed = BTreeSet::from([b"state.json".to_vec()]);
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
        for name in ["pre-rebase", "post-rewrite"] {
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
        use super::retained::{Area, Kind, Member};
        let mut members = vec![
            Member {
                area: Area::Original,
                path: b"preservation.json".to_vec(),
                stage: None,
                kind: Kind::File,
                mode: 0o600,
                data: serde_json::to_vec(&self.state.preservation)?,
            },
            Member {
                area: Area::Indexes,
                path: b"original".to_vec(),
                stage: None,
                kind: Kind::File,
                mode: 0o600,
                data: self.state.preservation.original_index().to_vec(),
            },
            Member {
                area: Area::Indexes,
                path: b"observed".to_vec(),
                stage: None,
                kind: Kind::File,
                mode: 0o600,
                data: fs::read(repo.index_path())?,
            },
        ];
        for change in &self.state.work {
            members.push(retained_image(Area::Before, &change.path, &change.before)?);
            let observed = switch::snapshot(
                &repo
                    .workdir()
                    .context("worktree required")?
                    .join(gix::path::from_bstr(change.path.as_bstr())?.as_ref()),
            )?;
            members.push(retained_image(Area::Worktree, &change.path, &observed)?);
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
                let blob = repo.find_object(entry.id)?.try_into_blob()?;
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
                    data: blob.data.clone(),
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
        retained_objects.extend(observed.entries().iter().map(|entry| entry.id.to_string()));
        for value in &retained_objects {
            let id = gix::ObjectId::from_hex(value.as_bytes())?;
            let object = repo.find_object(id)?;
            members.push(Member {
                area: Area::Objects,
                path: value.as_bytes().to_vec(),
                stage: None,
                kind: Kind::Object {
                    id: value.clone(),
                    kind: object.kind.to_string(),
                },
                mode: 0o600,
                data: object.data.clone(),
            });
        }
        retain_journal_members(&self.directory, &self.directory, &mut members)?;
        let original = gix::ObjectId::from_hex(self.state.original.as_bytes())?;
        let tip = gix::ObjectId::from_hex(self.state.tip.as_bytes())?;
        super::retained::capture(
            repo,
            &self.state.nonce,
            self.state.copies,
            original,
            tip,
            repo.head_commit()?.id,
            &members,
        )
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
        let envelope: Envelope = serde_json::from_slice(&fs::read(path)?)?;
        ensure!(
            switch::digest(&envelope.payload)? == envelope.checksum,
            "replay journal checksum mismatch"
        );
        let state: ParentState = serde_json::from_slice(&envelope.payload)?;
        ensure!(
            state.version == 1 && state.nonce.len() == 40 && state.nonce.as_bytes().iter().all(u8::is_ascii_hexdigit),
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
        if session.state.phase == "prepared" {
            session.validate_retention(repo)?;
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
                && repo.head_commit()?.id == gix::ObjectId::from_hex(session.state.original.as_bytes())?,
            "foreign original HEAD movement"
        );
        ensure!(
            switch::snapshot(&repo.index_path())? == session.state.index,
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
    pub fn abort_prepared(&self, repo: &gix::Repository) -> Result<()> {
        let _guard = switch::shared_guard(repo)?;
        let mut current = Self::open(repo)?;
        ensure!(
            serde_json::to_vec(&current.state)? == serde_json::to_vec(&self.state)?,
            "stale replay session"
        );
        ensure!(
            matches!(
                self.state.phase.as_str(),
                "preparing" | "prepared" | "cancelling-prepared"
            ),
            "only an unpublished preparation can be cancelled here"
        );
        ensure!(
            switch::snapshot(&repo.git_dir().join("HEAD"))? == self.state.head,
            "foreign HEAD movement"
        );
        ensure!(
            repo.head_commit()?.id == gix::ObjectId::from_hex(self.state.original.as_bytes())?,
            "original branch moved"
        );
        ensure!(
            switch::snapshot(&repo.index_path())? == self.state.index,
            "foreign index mutation"
        );
        let pending = self.directory.join("state.pending");
        if fs::symlink_metadata(&pending).is_ok() {
            let candidate = Self::open_record(repo, "state.pending")?;
            let mut comparable = candidate.state.clone();
            ensure!(
                matches!(
                    comparable.phase.as_str(),
                    "preparing" | "prepared" | "cancelling-prepared"
                ),
                "invalid pending cancellation"
            );
            comparable.phase.clone_from(&current.state.phase);
            ensure!(
                serde_json::to_vec(&comparable)? == serde_json::to_vec(&current.state)?,
                "foreign pending cancellation"
            );
            fs::rename(&pending, self.directory.join("state.json"))?;
            switch::sync_dir(&self.directory)?;
            current = candidate;
        }
        for entry in fs::read_dir(&self.directory)? {
            ensure!(
                entry?.file_name() == "state.json",
                "unknown replay journal member; inspect before cleanup"
            );
        }
        if current.state.phase != "cancelling-prepared" {
            current.state.phase = "cancelling-prepared".into();
            current.save()?;
        }
        for (label, id) in [("original", &self.state.original), ("upstream", &self.state.upstream)] {
            let name = format!("refs/brit-rebase-retain/{}/{label}", self.state.nonce);
            if let Some(reference) = repo.try_find_reference(name.as_str())? {
                ensure!(
                    matches!(reference.target(), gix::refs::TargetRef::Object(_))
                        && reference.id() == gix::ObjectId::from_hex(id.as_bytes())?,
                    "foreign replay retention reference"
                );
                reference.delete()?;
            }
        }
        sync_reference_storage(repo)?;
        let tombstone = repo.git_dir().join(format!("brit-rebase-done-{}", self.state.nonce));
        ensure!(!tombstone.exists(), "replay tombstone collision");
        fs::rename(&self.directory, &tombstone)?;
        switch::sync_dir(repo.git_dir())?;
        switch::checkpoint("replay-preparation-tombstoned")?;
        Self::cleanup_prepared_inner(repo, &tombstone)
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
                let session = Self::open_directory(repo, tombstone.to_owned(), "state.json")?;
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
            state.version == 1
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
                for entry in fs::read_dir(tombstone)? {
                    let entry = entry?;
                    ensure!(
                        entry.file_name() == "state.json"
                            && fs::symlink_metadata(entry.path())?.is_file()
                            && fs::read(entry.path())? == fs::read(&receipt)?,
                        "unknown tombstone member"
                    );
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

    fn save(&self) -> Result<()> {
        let payload = serde_json::to_vec(&self.state)?;
        let bytes = serde_json::to_vec(&Envelope {
            checksum: switch::digest(&payload)?,
            payload,
        })?;
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
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => switch::private_write(&temporary, &bytes)?,
            Err(error) => return Err(error.into()),
        }
        switch::checkpoint("replay-state-file-synced")?;
        fs::rename(&temporary, self.directory.join("state.json"))?;
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
        &self,
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
            switch::snapshot(&repo.index_path())? == self.state.index,
            "foreign replay index change"
        );
        ensure!(
            switch::snapshot(&repo.git_dir().join("HEAD"))? == self.state.head,
            "foreign replay HEAD change"
        );
        ensure!(
            switch::snapshot(&repo.git_dir().join("logs/HEAD"))? == self.state.log,
            "foreign replay HEAD reflog change"
        );
        let root = repo.workdir().context("worktree required")?;
        let desired = repo.index_from_tree(&tree)?;
        let attributes = repo.attributes_only(&desired, gix::worktree::stack::state::attributes::Source::IdMapping)?;
        let mut pipeline = gix::filter::Pipeline::new(repo, attributes.detach())?;
        let mut after = self.state.clone();
        let mut changes = Vec::new();
        for tracked in &mut after.work {
            let relative = gix::path::from_bstr(tracked.path.as_bstr())?;
            let absolute = root.join(relative.as_ref());

            let before = switch::snapshot(&absolute)?;
            ensure!(before == tracked.after, "foreign worktree mutation in replay footprint");
            let image = match desired.entry_by_path(tracked.path.as_bstr()) {
                None => switch::State::Absent,
                Some(entry) => {
                    let blob = repo.find_object(entry.id)?.try_into_blob()?;
                    match entry.mode {
                        gix::index::entry::Mode::SYMLINK => {
                            ensure!(
                                repo.config_snapshot().try_boolean("core.symlinks")? != Some(false),
                                "emulated replay symlinks are unsupported"
                            );
                            ensure!(
                                !blob.data.is_empty() && !blob.data.contains(&0),
                                "invalid replay symlink"
                            );
                            switch::State::Symlink(blob.data.clone())
                        }
                        gix::index::entry::Mode::FILE | gix::index::entry::Mode::FILE_EXECUTABLE => {
                            let mut bytes = Vec::new();
                            pipeline
                                .convert_to_worktree(&blob.data, tracked.path.as_bstr(), Default::default())?
                                .read_to_end(&mut bytes)?;
                            switch::State::File {
                                mode: switch::creation_mode(
                                    if entry.mode == gix::index::entry::Mode::FILE_EXECUTABLE {
                                        0o777
                                    } else {
                                        0o666
                                    },
                                )?,
                                bytes,
                            }
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
        for change in &changes {
            if let Some(tracked) = after.work.iter_mut().find(|tracked| tracked.path == change.path) {
                tracked.after = change.after.clone();
            } else {
                after.work.push(change.clone());
            }
        }
        after.witness = crate::repository::authoring::conflict::worktree_witness(
            repo,
            &after.work.iter().map(|change| change.path.clone()).collect::<Vec<_>>(),
        )?;
        let public = self.state.preservation.public_index(repo, private)?;
        let mut bytes = Vec::new();
        gix::index::File::from_state(public, repo.index_path()).write_to(&mut bytes, Default::default())?;
        after.index = switch::State::File { mode: 0o644, bytes };
        after.head = switch::State::File {
            mode: 0o644,
            bytes: format!("{tip}\n").into_bytes(),
        };
        if gix::ObjectId::from_hex(self.state.tip.as_bytes())? != tip
            && repo.config_snapshot().try_boolean("core.logAllRefUpdates")? != Some(false)
        {
            let mut bytes = match &after.log {
                switch::State::Absent => Vec::new(),
                switch::State::File { bytes, .. } => bytes.clone(),
                _ => anyhow::bail!("unsafe replay HEAD log"),
            };
            gix::refs::log::Line {
                previous_oid: gix::ObjectId::from_hex(self.state.tip.as_bytes())?,
                new_oid: tip,
                signature: repo
                    .committer()
                    .context("committer required for replay log")??
                    .to_owned()?,
                message: format!("brit rebase {} step {} {:?}", self.state.nonce, self.state.step, kind).into(),
            }
            .write_to(&mut bytes)?;
            after.log = switch::State::File { mode: 0o644, bytes };
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
                packed_refs: switch::snapshot(&repo.common_dir().join("packed-refs"))?,
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
            let object = repo.find_object(id)?;
            ensure!(
                gix::objs::compute_hash(repo.object_hash(), object.kind, &object.data)? == id,
                "decision closure corrupted"
            );
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
            if plan.exists() {
                let envelope: Envelope = serde_json::from_slice(&fs::read(&plan)?)?;
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
        let envelope: Envelope = serde_json::from_slice(&fs::read(&path)?)?;
        ensure!(
            switch::digest(&envelope.payload)? == envelope.checksum,
            "corrupt replay decision"
        );
        let decision: Decision = serde_json::from_slice(&envelope.payload)?;
        ensure!(
            !aborting || decision.transition.kind != super::transition::Kind::Final,
            "a durable final decision cannot be aborted"
        );
        switch::sync_path(&path)?;
        switch::sync_dir(&session.directory)?;
        session.apply_decision(repo, decision, false)?;
        Ok(session)
    }
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

fn retained_image(area: super::retained::Area, path: &[u8], image: &switch::State) -> Result<super::retained::Member> {
    use super::retained::{Kind, Member};
    let (kind, mode, data) = match image {
        switch::State::Absent => (Kind::Deleted, 0, Vec::new()),
        switch::State::File { mode, bytes } => (Kind::File, *mode, bytes.clone()),
        switch::State::Symlink(bytes) => (Kind::Symlink, 0, bytes.clone()),
        switch::State::Directory { mode, device, inode } => (
            Kind::Directory {
                device: *device,
                inode: *inode,
            },
            *mode,
            Vec::new(),
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

fn sync_tree(path: &Path) -> Result<()> {
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

fn raw(path: &Path) -> Result<Vec<u8>> {
    Ok(gix::path::into_bstr(fs::canonicalize(path)?)?.to_vec())
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
    ensure!(
        digest == envelope.checksum && digest == binding.parent_state,
        "replay parent state binding changed"
    );
    let state: ParentState = serde_json::from_slice(&envelope.payload)?;
    ensure!(
        state.version == 1 && state.nonce == binding.nonce && state.step == binding.step && state.phase == "authoring",
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

fn retain_journal_members(root: &Path, directory: &Path, members: &mut Vec<super::retained::Member>) -> Result<()> {
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
        let image = switch::snapshot(&path)?;
        members.push(retained_image(super::retained::Area::Invocations, &relative, &image)?);
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
