//! Interactive sequencing state. Repository writes remain in the parent driver;
//! the accepted todo grammar and suffix rules belong to gix-sequencer.
use super::*;
use gix_sequencer::{Command, Todo};

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Interactive {
    pub todo: Todo,
    pub cursor: usize,
    pub initial_accepted: bool,
    pub group: Option<Group>,
    pub edit: Option<Edit>,
    pub message: Option<Vec<u8>>,
    pub editors: Vec<Editor>,
    pub ledger: gix_sequencer::MappingLedger,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Group {
    pub originals: Vec<String>,
    pub author: String,
    pub parent: String,
    pub message: Vec<u8>,
    pub tip: String,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Edit {
    pub original: String,
    pub parent: String,
    pub active: Vec<Vec<u8>>,
    pub previous_group: Option<Group>,
}

impl Interactive {
    pub fn new(candidates: &[Candidate]) -> Result<Self> {
        let ids = candidates
            .iter()
            .map(|candidate| candidate.commit.to_string())
            .collect::<Vec<_>>();
        use std::fmt::Write;
        let mut text = String::new();
        for id in &ids {
            writeln!(text, "pick {id}")?;
        }
        let bytes = text.into_bytes();
        Ok(Self {
            todo: Todo::parse(&bytes, &ids)?,
            cursor: 0,
            initial_accepted: false,
            group: None,
            edit: None,
            message: None,
            editors: Vec::new(),
            ledger: gix_sequencer::MappingLedger::new(ids)?,
        })
    }
}

impl Session {
    fn todo_boundary(&self) -> Result<usize> {
        let interactive = self
            .state
            .interactive
            .as_ref()
            .context("noninteractive replay has no todo")?;
        Ok(interactive.cursor
            + usize::from(matches!(self.state.phase.as_str(), "authoring" | "empty") && interactive.edit.is_none()))
    }

    fn todo_has_group(&self) -> Result<bool> {
        let interactive = self
            .state
            .interactive
            .as_ref()
            .context("noninteractive replay has no todo")?;
        Ok(interactive.group.is_some() || self.todo_boundary()? > interactive.cursor)
    }

    pub fn todo(&self) -> Result<Vec<u8>> {
        Ok(self
            .state
            .interactive
            .as_ref()
            .context("noninteractive replay has no todo")?
            .todo
            .bytes
            .clone())
    }

    /// Accept validated saved bytes explicitly. Rejected bytes never replace the
    /// last accepted todo or move either execution or mapping cursor.
    pub fn accept_todo(&mut self, repo: &gix::Repository, bytes: &[u8]) -> Result<()> {
        let _guard = switch::shared_guard(repo)?;
        self.validate_current(repo)?;
        ensure!(
            matches!(
                self.state.phase.as_str(),
                "prepared" | "pre-approved" | "replaying" | "empty" | "break" | "authoring"
            ),
            "todo editing requires a stable stop"
        );
        let interactive = self
            .state
            .interactive
            .as_ref()
            .context("noninteractive replay has no todo")?;
        ensure!(
            interactive.editors.last().is_none_or(|editor| editor.accepted),
            "accept the saved editor output before replacing todo bytes"
        );
        let accepted = interactive
            .todo
            .replace_suffix(self.todo_boundary()?, bytes, self.todo_has_group()?)?;
        // R is a conservative union of every candidate delta, independent of
        // order. Recheck its complete preserved-path collision relation here;
        // native merge outputs extend it before each later live transition.
        self.state.preservation.validate_scope(
            &self
                .state
                .work
                .iter()
                .map(|change| change.path.clone())
                .collect::<Vec<_>>(),
        )?;
        let interactive = self.state.interactive.as_mut().context("interactive state missing")?;
        interactive.todo = accepted;
        interactive.initial_accepted = true;
        self.save()
    }

    pub(super) fn current_command(&self) -> Result<Command> {
        match &self.state.interactive {
            Some(interactive) => Ok(interactive
                .todo
                .entries
                .get(interactive.cursor)
                .context("no current todo entry")?
                .command),
            None => Ok(Command::Pick),
        }
    }

    pub(super) fn current_candidate(&self) -> Result<Candidate> {
        let pair = if let Some(interactive) = &self.state.interactive {
            let id = interactive
                .todo
                .entries
                .get(interactive.cursor)
                .and_then(|entry| entry.commit.as_ref())
                .context("current todo entry has no commit")?;
            self.state
                .candidates
                .iter()
                .find(|(candidate, _)| candidate == id)
                .context("todo candidate absent from pinned selection")?
        } else {
            self.state
                .candidates
                .get(self.state.step)
                .context("no replay input remains")?
        };
        Ok(Candidate {
            commit: gix::ObjectId::from_hex(pair.0.as_bytes())?,
            parent: gix::ObjectId::from_hex(pair.1.as_bytes())?,
        })
    }

    pub(super) fn proposed_parent(&self) -> Result<gix::ObjectId> {
        if matches!(self.current_command()?, Command::Squash | Command::Fixup) {
            let group = self
                .state
                .interactive
                .as_ref()
                .and_then(|interactive| interactive.group.as_ref())
                .context("squash has no retained group")?;
            gix::ObjectId::from_hex(group.parent.as_bytes()).map_err(Into::into)
        } else {
            self.tip()
        }
    }

    pub(super) fn propose_interactive(&self, repo: &gix::Repository, step: &Step) -> Result<Proposal> {
        let Some(interactive) = &self.state.interactive else {
            return step.propose(repo);
        };
        let command = self.current_command()?;
        let group = interactive.group.as_ref();
        let author = if matches!(command, Command::Squash | Command::Fixup) {
            gix::ObjectId::from_hex(group.context("squash has no retained group")?.author.as_bytes())?
        } else {
            step.original
        };
        let message = if command == Command::Fixup {
            Some(group.context("fixup has no retained group")?.message.as_slice())
        } else {
            interactive.message.as_deref()
        };
        step.propose_message(repo, author, self.proposed_parent()?, message)
    }

    pub(super) fn record_result(
        &self,
        repo: &gix::Repository,
        after: &mut ParentState,
        commit: gix::ObjectId,
    ) -> Result<()> {
        let original = self.current_candidate()?.commit.to_string();
        let command = self.current_command()?;
        if let Some(interactive) = &mut after.interactive {
            if matches!(command, Command::Squash | Command::Fixup) {
                let group = interactive.group.as_mut().context("squash has no retained group")?;
                group.originals.push(original.clone());
                interactive
                    .ledger
                    .replace_group(&group.originals, &[commit.to_string()])?;
                for (old, results) in &mut after.mappings {
                    if group.originals.contains(old) {
                        *results = vec![commit.to_string()];
                    }
                }
                group.tip = commit.to_string();
                group.message = repo.find_commit(commit)?.decode()?.message.to_vec();
            } else {
                interactive.ledger.record(&original, &[commit.to_string()])?;
                interactive.group = Some(Group {
                    originals: vec![original.clone()],
                    author: original.clone(),
                    parent: self.state.tip.clone(),
                    message: repo.find_commit(commit)?.decode()?.message.to_vec(),
                    tip: commit.to_string(),
                });
            }
            interactive.cursor += 1;
            interactive.message = None;
        }
        after.mappings.push((original, vec![commit.to_string()]));
        Ok(())
    }
}

impl Session {
    pub fn message(&self, repo: &gix::Repository) -> Result<Vec<u8>> {
        let interactive = self
            .state
            .interactive
            .as_ref()
            .context("noninteractive replay has no message stop")?;
        if let Some(message) = &interactive.message {
            return Ok(message.clone());
        }
        let original = repo.find_commit(self.current_candidate()?.commit)?;
        let message = original.decode()?.message.to_vec();
        if self.current_command()? == Command::Squash {
            let mut combined = interactive
                .group
                .as_ref()
                .context("squash requires prior group")?
                .message
                .clone();
            combined.extend_from_slice(b"\n\n");
            combined.extend_from_slice(&message);
            Ok(combined)
        } else {
            Ok(message)
        }
    }

    pub(super) fn needs_message(&self) -> Result<bool> {
        Ok(matches!(self.current_command()?, Command::Reword | Command::Squash)
            && self
                .state
                .interactive
                .as_ref()
                .is_some_and(|interactive| interactive.message.is_none()))
    }
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Editor {
    pub plan: crate::repository::replay::editors::Plan,
    pub ordinal: usize,
    pub input: Vec<u8>,
    pub index: Vec<u8>,
    pub work: String,
    pub metadata: Vec<(Vec<u8>, switch::State)>,
    pub accepted: bool,
    pub outcome: Option<crate::repository::authoring::invocation::InvocationOutcome>,
}

impl Session {
    /// Explicitly request a new sequence editor; an interrupted previous editor
    /// is inspected for quiescence, never automatically relaunched.
    pub fn edit_todo(&mut self, repo: &gix::Repository) -> Result<()> {
        use crate::repository::replay::editors::Kind;
        ensure!(
            matches!(
                self.state.phase.as_str(),
                "prepared" | "pre-approved" | "replaying" | "empty" | "break" | "authoring"
            ),
            "todo editing requires a stable stop"
        );
        let interactive = self
            .state
            .interactive
            .as_ref()
            .context("noninteractive replay has no todo")?;
        let mut bytes = Vec::new();
        for entry in &interactive.todo.entries[self.todo_boundary()?..] {
            let command = match entry.command {
                Command::Pick => "pick",
                Command::Reword => "reword",
                Command::Edit => "edit",
                Command::Squash => "squash",
                Command::Fixup => "fixup",
                Command::Drop => "drop",
                Command::Break => "break",
            };
            bytes.extend_from_slice(command.as_bytes());
            if let Some(id) = &entry.commit {
                bytes.push(b' ');
                bytes.extend_from_slice(id.as_bytes());
            }
            bytes.push(b'\n');
        }
        self.run_editor(repo, Kind::Sequence, &bytes)
    }

    pub fn edit_message(&mut self, repo: &gix::Repository) -> Result<()> {
        ensure!(self.state.phase == "message", "no replay message stop");
        let bytes = self.message(repo)?;
        self.run_editor(repo, crate::repository::replay::editors::Kind::Message, &bytes)
    }

    pub fn editor_output(&self) -> Option<std::path::PathBuf> {
        self.state
            .interactive
            .as_ref()?
            .editors
            .last()
            .map(|editor| self.directory.join(format!("editor-{}-output", editor.ordinal)))
    }

    fn run_editor(
        &mut self,
        repo: &gix::Repository,
        kind: crate::repository::replay::editors::Kind,
        input: &[u8],
    ) -> Result<()> {
        use crate::repository::{authoring::invocation, replay::editors};
        let record;
        {
            let _guard = switch::shared_guard(repo)?;
            self.validate_current(repo)?;
            let ordinal = self
                .state
                .interactive
                .as_ref()
                .context("editor requires interactive replay")?
                .editors
                .len();
            let output = self.directory.join(format!("editor-{ordinal}-output"));
            let plan = editors::resolve_input(repo, kind, &output, input)?;
            let mut paths = vec![
                repo.index_path(),
                repo.git_dir().join("HEAD"),
                repo.git_dir().join("logs/HEAD"),
            ];
            let index = if self.state.phase == "authoring" {
                let authorization = self.child_authorization()?;
                crate::repository::authoring::conflict::Session::open_owned(repo, &authorization)?.validate(repo)?;
                paths.extend([
                    repo.git_dir().join("brit-authoring/state.json"),
                    repo.git_dir().join("brit-authoring/index.private"),
                ]);
                fs::read(repo.git_dir().join("brit-authoring/index.private"))?
            } else {
                let tree = if self.state.phase == "message" {
                    gix::ObjectId::from_hex(
                        self.state
                            .stop_tree
                            .as_ref()
                            .context("message tree missing")?
                            .as_bytes(),
                    )?
                } else {
                    repo.find_commit(self.tip()?)?.tree_id()?.detach()
                };
                let private = repo.index_from_tree(&tree)?;
                let mut bytes = Vec::new();
                private.write_to(&mut bytes, Default::default())?;
                bytes
            };
            let metadata = paths
                .iter()
                .map(|path| Ok((gix::path::into_bstr(path.clone())?.to_vec(), switch::snapshot(path)?)))
                .collect::<Result<Vec<_>>>()?;
            record = Editor {
                plan,
                ordinal,
                input: input.to_vec(),
                index,
                work: crate::repository::authoring::conflict::worktree_witness(repo, &[])?,
                metadata,
                accepted: false,
                outcome: None,
            };
            self.state
                .interactive
                .as_mut()
                .context("interactive state missing")?
                .editors
                .push(record.clone());
            self.save()?;
            switch::checkpoint("replay-editor-planned")?;
            switch::private_write(&output, &record.input)?;
            switch::private_write(&self.directory.join(format!("editor-{ordinal}-index")), &record.index)?;
            switch::sync_dir(&self.directory)?;
        }
        let index = self.directory.join(format!("editor-{}-index", record.ordinal));
        let outcome = editors::run(
            repo,
            &self.directory.join(format!("editor-{}", record.ordinal)),
            &record.plan,
            &index,
            || {
                let _guard = switch::shared_guard(repo)?;
                let saved = Self::open(repo)?;
                ensure!(
                    serde_json::to_vec(&saved.state)? == serde_json::to_vec(&self.state)?,
                    "editor launch authorization changed"
                );
                self.validate_editor_effects(repo, &record)?;
                ensure!(
                    super::super::hooks::policy(repo)? == self.state.policy,
                    "policy changed before editor launch"
                );
                self.validate_branch(repo, None)?;
                switch::sync_path(&self.directory.join("state.json"))?;
                switch::sync_dir(&self.directory)
            },
        )?;
        {
            let _guard = switch::shared_guard(repo)?;
            self.sync_editor_output(record.ordinal)?;
            self.validate_current(repo)?;
            self.validate_editor_effects(repo, &record)?;
            self.state
                .interactive
                .as_mut()
                .context("interactive state missing")?
                .editors
                .last_mut()
                .context("editor receipt missing")?
                .outcome = Some(outcome);
            self.save()?;
        }
        ensure!(
            outcome == invocation::InvocationOutcome::Succeeded,
            "editor did not succeed; inspect or explicitly accept saved output without rerun"
        );
        self.accept_editor(repo)
    }

    fn sync_editor_output(&self, ordinal: usize) -> Result<()> {
        let output = self.directory.join(format!("editor-{ordinal}-output"));
        let metadata = fs::symlink_metadata(&output)?;
        ensure!(
            metadata.is_file() && metadata.uid() == fs::metadata(&self.directory)?.uid(),
            "unsafe editor output"
        );
        switch::sync_path(&output)?;
        switch::sync_dir(&self.directory)
    }

    fn validate_editor_effects(&self, repo: &gix::Repository, editor: &Editor) -> Result<()> {
        ensure!(
            crate::repository::authoring::conflict::worktree_witness(repo, &[])? == editor.work,
            "editor changed work outside its named buffer"
        );
        for (path, expected) in &editor.metadata {
            ensure!(
                switch::snapshot(gix::path::from_bstr(path.as_bstr())?.as_ref())? == *expected,
                "editor changed repository metadata outside its named buffer"
            );
        }
        let index = self.directory.join(format!("editor-{}-index", editor.ordinal));
        if index.exists() {
            ensure!(fs::read(index)? == editor.index, "editor changed its private index");
        }
        Ok(())
    }

    pub(super) fn inspect_editor(&self) -> Result<()> {
        if let Some(editor) = self
            .state
            .interactive
            .as_ref()
            .and_then(|interactive| interactive.editors.last())
            && !editor.accepted
        {
            let path = self.directory.join(format!("editor-{}", editor.ordinal));
            if path.exists() {
                crate::repository::authoring::invocation::inspect(&path)?;
            }
        }
        Ok(())
    }

    /// Explicit continue accepts saved bytes, including an UNKNOWN but quiescent
    /// editor outcome; it does not assume or report that the editor succeeded.
    pub fn accept_editor(&mut self, repo: &gix::Repository) -> Result<()> {
        use crate::repository::{authoring::invocation, replay::editors};
        let _guard = switch::shared_guard(repo)?;
        self.validate_current(repo)?;
        let editor = self
            .state
            .interactive
            .as_ref()
            .and_then(|interactive| interactive.editors.last())
            .context("no editor output")?
            .clone();
        ensure!(!editor.accepted, "editor output already accepted");
        let invocation_path = self.directory.join(format!("editor-{}", editor.ordinal));
        let outcome = if invocation_path.exists() {
            invocation::inspect(&invocation_path)?
        } else {
            invocation::InvocationOutcome::Unknown
        };
        self.validate_editor_effects(repo, &editor)?;
        let output = self.directory.join(format!("editor-{}-output", editor.ordinal));
        if !output.try_exists()? && !invocation_path.exists() {
            switch::private_write(&output, &editor.input)?;
            switch::sync_dir(&self.directory)?;
        }
        let metadata = fs::symlink_metadata(&output)?;
        ensure!(
            metadata.is_file() && metadata.uid() == fs::metadata(&self.directory)?.uid(),
            "unsafe editor output"
        );
        let bytes = fs::read(&output)?;
        self.sync_editor_output(editor.ordinal)?;
        let boundary = self.todo_boundary()?;
        let has_group = self.todo_has_group()?;
        let interactive = self.state.interactive.as_mut().context("interactive state missing")?;
        match editor.plan.kind {
            editors::Kind::Sequence => {
                let todo = interactive.todo.replace_suffix(boundary, &bytes, has_group)?;
                self.state.preservation.validate_scope(
                    &self
                        .state
                        .work
                        .iter()
                        .map(|change| change.path.clone())
                        .collect::<Vec<_>>(),
                )?;
                interactive.todo = todo;
                interactive.initial_accepted = true;
            }
            editors::Kind::Message => {
                ensure!(self.state.phase == "message", "message editor belongs to another phase");
                ensure!(!bytes.iter().all(u8::is_ascii_whitespace), "empty replay message");
                interactive.message = Some(bytes);
                self.state.phase = "message-ready".into();
            }
        }
        let editor = interactive.editors.last_mut().context("editor receipt missing")?;
        editor.accepted = true;
        editor.outcome = Some(outcome);
        self.save()?;
        switch::checkpoint("replay-editor-accepted")
    }
}

impl Session {
    pub fn is_edit_stop(&self) -> bool {
        self.state.phase == "authoring"
            && self
                .state
                .interactive
                .as_ref()
                .is_some_and(|interactive| interactive.edit.is_some())
    }

    fn ensure_edit(&self) -> Result<()> {
        ensure!(
            self.state.phase == "authoring"
                && self
                    .state
                    .interactive
                    .as_ref()
                    .is_some_and(|interactive| interactive.edit.is_some()),
            "authoring action requires an edit stop"
        );
        Ok(())
    }

    pub fn amend(
        &mut self,
        repo: &gix::Repository,
        message: &[u8],
    ) -> Result<crate::repository::authoring::conflict::Outcome> {
        self.ensure_edit()?;
        let authorization = self.child_authorization()?;
        let mut child = crate::repository::authoring::conflict::Session::open_owned(repo, &authorization)?;
        child.amend_bytes(repo, message)
    }

    pub fn begin_split(&mut self, repo: &gix::Repository) -> Result<()> {
        self.ensure_edit()?;
        let authorization = self.child_authorization()?;
        let mut child = crate::repository::authoring::conflict::Session::open_owned(repo, &authorization)?;
        child.begin_split(repo)
    }

    pub fn commit_split(
        &mut self,
        repo: &gix::Repository,
        message: &[u8],
    ) -> Result<crate::repository::authoring::conflict::Outcome> {
        self.ensure_edit()?;
        let authorization = self.child_authorization()?;
        let mut child = crate::repository::authoring::conflict::Session::open_owned(repo, &authorization)?;
        child.commit_split_bytes(repo, message)
    }

    pub fn continue_edit(&mut self, repo: &gix::Repository) -> Result<Advance> {
        self.ensure_edit()?;
        let authorization = self.child_authorization()?;
        let mut child = crate::repository::authoring::conflict::Session::open_owned(repo, &authorization)?;
        if child.has_pending_post_hook() {
            child.acknowledge_post_hook(repo)?;
        }
        if child.has_pending_author_message() {
            child.accept_author_message(repo)?;
            ensure!(
                !child.has_pending_post_hook(),
                "authored commit published with unknown post-hook; continue again to acknowledge without rerun"
            );
        }
        if child.has_pending_post_hooks() {
            let warnings = child.resume_post_hooks(repo)?;
            ensure!(
                !child.has_pending_post_hook(),
                "authored commit published with unknown post-hook; continue again to acknowledge without rerun"
            );
            ensure!(
                warnings.is_empty(),
                "authored commit published; post-hook warnings require inspection before continuing: {}",
                warnings.join("; ")
            );
        }
        child.import_external(repo)?;
        let _guard = switch::shared_guard(repo)?;
        self.validate_current(repo)?;
        let completion = child.owned_completion(repo, &authorization)?;
        let edit = self
            .state
            .interactive
            .as_ref()
            .and_then(|interactive| interactive.edit.as_ref())
            .context("edit context missing")?
            .clone();
        ensure!(!completion.chain.is_empty(), "empty split requires explicit skip");
        let mut parent = gix::ObjectId::from_hex(edit.parent.as_bytes())?;
        for commit in &completion.chain {
            ensure!(
                repo.find_commit(*commit)?
                    .parent_ids()
                    .map(gix::Id::detach)
                    .collect::<Vec<_>>()
                    == [parent],
                "authored chain is not rooted at the edit parent"
            );
            parent = *commit;
        }
        ensure!(
            parent == completion.head,
            "authored chain tip differs from child receipt"
        );
        self.adopt_child_retention(repo, &completion.binding, completion.decisions)?;
        self.adopt_child_work(repo, &completion.work, completion.head)?;
        self.state.accepted_child = Some((completion.binding, completion.receipt));
        let mut closure = BTreeSet::new();
        history(repo, completion.head, &mut closure)?;
        let tip_closure = closure.clone();
        for id in &completion.objects {
            match repo.find_object(*id)?.kind {
                gix::objs::Kind::Commit => {
                    history(repo, *id, &mut closure)?;
                }
                gix::objs::Kind::Tree => {
                    validate_tree(repo, *id, &mut BTreeSet::new(), &mut closure)?;
                }
                gix::objs::Kind::Blob => {
                    closure.insert(*id);
                }
                _ => anyhow::bail!("unsupported authored object kind"),
            }
        }
        sync_tree(&repo.common_dir().join("objects"))?;
        let label = format!("edit-{}", self.state.step - 1);
        self.protect(repo, &label, completion.head, &tip_closure)?;
        self.state.retained.push((label, completion.head.to_string()));
        self.state.closure.extend(closure.iter().map(ToString::to_string));
        self.state.closure.sort();
        self.state.closure.dedup();
        let mapping = self
            .state
            .mappings
            .iter_mut()
            .find(|(original, _)| original == &edit.original)
            .context("edit mapping missing")?;
        mapping.1 = completion.chain.iter().map(ToString::to_string).collect();
        let mut message = Vec::new();
        for commit in &completion.chain {
            if !message.is_empty() {
                message.extend_from_slice(b"\n\n");
            }
            message.extend_from_slice(repo.find_commit(*commit)?.decode()?.message);
        }
        let interactive = self.state.interactive.as_mut().context("interactive state missing")?;
        interactive.ledger.replace_group(
            std::slice::from_ref(&edit.original),
            &completion.chain.iter().map(ToString::to_string).collect::<Vec<_>>(),
        )?;
        interactive.group = Some(Group {
            originals: vec![edit.original],
            author: completion.chain[0].to_string(),
            parent: edit.parent,
            message,
            tip: completion.head.to_string(),
        });
        interactive.edit = None;
        self.state.phase = "edit-accepted".into();
        self.save()?;
        switch::checkpoint("replay-edit-accepted")?;
        self.archive_accepted(repo)?;
        let accepted = self
            .state
            .accepted_child
            .take()
            .context("edit acceptance receipt missing")?;
        self.state.accepted_children.push(accepted);
        self.state.authorized_stop = None;
        self.state.phase = "replaying".into();
        self.save()?;
        Ok(Advance::Step {
            commit: completion.head,
        })
    }

    pub(super) fn adopt_child_work(
        &mut self,
        repo: &gix::Repository,
        work: &[switch::Change],
        head: gix::ObjectId,
    ) -> Result<()> {
        self.state
            .preservation
            .validate_scope(&work.iter().map(|change| change.path.clone()).collect::<Vec<_>>())?;
        for change in work {
            if let Some(existing) = self.state.work.iter_mut().find(|existing| existing.path == change.path) {
                existing.after = change.after.clone();
            } else {
                self.state.work.push(change.clone());
            }
        }
        self.state.index = switch::snapshot(&repo.index_path())?;
        self.state.head = switch::snapshot(&repo.git_dir().join("HEAD"))?;
        self.state.log = switch::snapshot(&repo.git_dir().join("logs/HEAD"))?;
        self.state.tip = head.to_string();
        self.state.witness = crate::repository::authoring::conflict::worktree_witness(
            repo,
            &self
                .state
                .work
                .iter()
                .map(|change| change.path.clone())
                .collect::<Vec<_>>(),
        )?;
        Ok(())
    }
}

impl Session {
    pub fn amend_no_edit(&mut self, repo: &gix::Repository) -> Result<crate::repository::authoring::conflict::Outcome> {
        let message = repo.head_commit()?.decode()?.message.to_vec();
        self.amend(repo, &message)
    }

    pub fn amend_edit(&mut self, repo: &gix::Repository) -> Result<crate::repository::authoring::conflict::Outcome> {
        self.ensure_edit()?;
        let authorization = self.child_authorization()?;
        crate::repository::authoring::conflict::Session::open_owned(repo, &authorization)?.amend_edit(repo)
    }

    pub fn commit_split_edit(
        &mut self,
        repo: &gix::Repository,
    ) -> Result<crate::repository::authoring::conflict::Outcome> {
        self.ensure_edit()?;
        let authorization = self.child_authorization()?;
        crate::repository::authoring::conflict::Session::open_owned(repo, &authorization)?.commit_split_edit(repo)
    }
}
