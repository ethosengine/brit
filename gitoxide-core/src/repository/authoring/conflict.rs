//! Internal conflict/edit-stop authoring. This is not a replay sequencer or a CLI.
use anyhow::{Context, Result, ensure};
use gix::bstr::{BString, ByteSlice};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io::Write,
    path::{Path, PathBuf},
};

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum StopKind {
    Conflict,
    Edit,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct Entry {
    path: Vec<u8>,
    id: String,
    mode: u32,
    flags: u32,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
enum Image {
    File(Vec<u8>, u32),
    Link(Vec<u8>),
}
#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct State {
    version: u32,
    identities: Vec<(u64, u64)>,
    policy: String,
    original_index: String,
    original_head: Vec<u8>,
    original_log: Option<Vec<u8>>,
    owner: Vec<u8>,
    root: Vec<u8>,
    common: Vec<u8>,
    kind: StopKind,
    head: String,
    parent: String,
    original: Vec<Entry>,
    private: Vec<Entry>,
    public: Vec<Entry>,
    preserved: Vec<Vec<u8>>,
    active: Vec<Vec<u8>>,
    #[serde(with = "byte_map")]
    work: BTreeMap<Vec<u8>, Image>,
    #[serde(with = "byte_map")]
    before: BTreeMap<Vec<u8>, Option<Image>>,
    #[serde(with = "byte_map")]
    expanded: BTreeMap<Vec<u8>, Option<Image>>,
    split: bool,
    children: usize,
    invocation: usize,
    pending_hook: Option<(usize, bool)>,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Decision {
    before: State,
    after: State,
    old_index: Vec<u8>,
    new_index: Vec<u8>,
    old_head: Vec<u8>,
    new_head: Vec<u8>,
    old_log: Option<Vec<u8>>,
    new_log: Option<Vec<u8>>,
}
/// A private, worktree-owned authoring stop. All raw before images remain private.
pub struct Session {
    directory: PathBuf,
    state: State,
}
#[derive(Debug)]
pub struct Outcome {
    pub commit: gix::ObjectId,
    pub warnings: Vec<String>,
}
fn raw(path: &Path) -> Result<Vec<u8>> {
    Ok(gix::path::into_bstr(path)?.as_bytes().to_vec())
}

fn overlaps(a: &[u8], b: &[u8]) -> bool {
    a == b || (a.starts_with(b) && a.get(b.len()) == Some(&b'/')) || (b.starts_with(a) && b.get(a.len()) == Some(&b'/'))
}
fn entries(index: &gix::index::State) -> Vec<Entry> {
    index
        .entries()
        .iter()
        .map(|e| Entry {
            path: e.path(index).to_vec(),
            id: e.id.to_string(),
            mode: e.mode.bits(),
            flags: e.flags.bits(),
        })
        .collect()
}
fn index(repo: &gix::Repository, values: &[Entry]) -> Result<gix::index::State> {
    let mut state = gix::index::State::new(repo.object_hash());
    for e in values {
        state.dangerously_push_entry(
            Default::default(),
            gix::ObjectId::from_hex(e.id.as_bytes())?,
            gix::index::entry::Flags::from_bits_retain(e.flags),
            gix::index::entry::Mode::from_bits_retain(e.mode),
            e.path.as_bstr(),
        );
    }
    state.sort_entries();
    state.verify_entries()?;
    Ok(state)
}
fn bytes(repo: &gix::Repository, values: &[Entry]) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    gix::index::File::from_state(index(repo, values)?, repo.index_path()).write_to(&mut bytes, Default::default())?;
    Ok(bytes)
}
fn semantic(values: &[Entry], selected: &[u8]) -> Vec<(String, u32, u32)> {
    values
        .iter()
        .filter(|e| e.path == selected)
        .map(|e| {
            (
                e.id.clone(),
                e.mode,
                e.flags & (gix::index::entry::Flags::STAGE_MASK | gix::index::entry::Flags::INTENT_TO_ADD).bits(),
            )
        })
        .collect()
}
fn changed(a: &[Entry], b: &[Entry]) -> Vec<Vec<u8>> {
    a.iter()
        .chain(b)
        .map(|e| e.path.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .filter(|p| semantic(a, p) != semantic(b, p))
        .collect()
}
fn overlay(base: &[Entry], source: &[Entry], paths: &[Vec<u8>]) -> Vec<Entry> {
    let mut out: Vec<_> = base.iter().filter(|e| !paths.contains(&e.path)).cloned().collect();
    out.extend(source.iter().filter(|e| paths.contains(&e.path)).cloned());
    out.sort_by(|a, b| a.path.cmp(&b.path).then(a.flags.cmp(&b.flags)));
    out
}
fn snapshot(root: &Path) -> Result<BTreeMap<Vec<u8>, Image>> {
    fn walk(root: &Path, dir: &Path, out: &mut BTreeMap<Vec<u8>, Image>) -> Result<()> {
        for e in fs::read_dir(dir)? {
            let e = e?;
            if e.file_name() == ".git" {
                continue;
            }
            let p = e.path();
            let m = fs::symlink_metadata(&p)?;
            let rel = raw(p.strip_prefix(root)?)?;
            if m.file_type().is_symlink() {
                out.insert(rel, Image::Link(raw(&fs::read_link(p)?)?));
            } else if m.is_dir() {
                walk(root, &p, out)?;
            } else {
                ensure!(m.is_file(), "unsupported worktree file");
                #[cfg(unix)]
                let mode = {
                    use std::os::unix::fs::PermissionsExt;
                    m.permissions().mode() & 0o777
                };
                #[cfg(not(unix))]
                let mode = 0;
                out.insert(rel, Image::File(digest(&fs::read(p)?)?.into_bytes(), mode));
            }
        }
        Ok(())
    }
    let mut out = BTreeMap::new();
    walk(root, root, &mut out)?;
    Ok(out)
}
fn persist(path: &Path, data: &[u8]) -> Result<()> {
    let temp = path.with_extension("new");
    let mut options = fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(&temp)?;
    file.write_all(data)?;
    file.sync_all()?;
    fs::rename(temp, path)?;
    fs::File::open(path.parent().context("parent")?)?.sync_all()?;
    Ok(())
}
impl Session {
    pub fn prepare(repo: &gix::Repository, kind: StopKind, active: &[BString]) -> Result<Self> {
        validate_metadata_directories(repo)?;
        let _guard = super::super::switch::shared_guard(repo)?;
        super::super::switch::ensure_no_pending(repo)?;
        ensure!(repo.state().is_none(), "foreign operation in progress");
        ensure!(
            repo.head()?.referent_name().is_none(),
            "internal authoring requires detached HEAD"
        );
        ensure!(
            std::env::var_os("GIT_INDEX_FILE").is_none(),
            "alternate indexes are unsupported"
        );
        let original = read_public(repo)?;
        validate_public(&original)?;
        ensure!(
            original.entries().iter().all(|e| e.stage_raw() == 0),
            "unmerged starting index"
        );
        let raw_index = fs::read(repo.index_path())?;
        let (raw_state, _) =
            gix::index::State::from_bytes(&raw_index, original.timestamp(), repo.object_hash(), Default::default())?;
        ensure!(raw_state.link().is_none(), "split index unsupported");
        let head = repo.head_commit()?;
        ensure!(
            head.parent_ids().count() == 1,
            "internal authoring requires a non-root linear edit group"
        );
        let base = repo.index_from_tree(&head.tree_id()?)?;
        let original_entries = entries(&original);
        let private = entries(&base);
        let mut preserved = changed(&private, &original_entries);
        let active = super::super::index::authoring::literal_paths(repo, active)?
            .into_iter()
            .map(|p| p.to_vec())
            .collect::<Vec<_>>();
        for e in original.entries() {
            let p: BString = e.path(&original).into();
            let current = super::super::index::authoring::worktree_entry(repo, &original, &p)?;
            let equal = match current {
                Some((mode, data)) => mode == e.mode && repo.find_object(e.id)?.data == data,
                None => false,
            };
            if !equal && !preserved.contains(&p.to_vec()) {
                preserved.push(p.to_vec());
            }
        }
        ensure!(
            !preserved.iter().any(|p| active.iter().any(|a| overlaps(p, a))),
            "preserved edits overlap authoring scope"
        );
        let root = repo.workdir().context("worktree required")?;
        let work = snapshot(root)?;
        for a in &active {
            ensure!(
                !work
                    .keys()
                    .any(|p| overlaps(p, a) && !original_entries.iter().any(|e| e.path == *p)),
                "untracked collision in scope"
            );
        }
        let directory = repo.git_dir().join("brit-authoring");
        #[cfg(unix)]
        {
            use std::os::unix::fs::DirBuilderExt;
            fs::DirBuilder::new().mode(0o700).create(&directory)?;
        }
        #[cfg(not(unix))]
        fs::create_dir(&directory)?;
        let mut before = BTreeMap::new();
        for p in preserved.iter().chain(&active) {
            before.insert(p.clone(), image(root, p)?);
        }
        let state = State {
            version: 1,
            identities: identities(repo, &directory)?,
            policy: policy(repo)?,
            original_index: digest(&fs::read(repo.index_path())?)?,
            original_head: fs::read(repo.git_dir().join("HEAD"))?,
            original_log: read_optional(&repo.git_dir().join("logs/HEAD"))?,
            owner: raw(&fs::canonicalize(repo.git_dir())?)?,
            root: raw(&fs::canonicalize(root)?)?,
            common: raw(&fs::canonicalize(repo.common_dir())?)?,
            kind,
            head: head.id.to_string(),
            parent: head
                .parent_ids()
                .next()
                .context("edit group requires parent")?
                .to_string(),
            original: original_entries.clone(),
            private,
            public: original_entries,
            preserved,
            active,
            before,
            work,
            expanded: BTreeMap::new(),
            split: false,
            children: 0,
            invocation: 0,
            pending_hook: None,
        };
        let session = Self { directory, state };
        persist(&session.directory.join("index.original"), &fs::read(repo.index_path())?)?;
        session.save(repo)?;
        fs::File::open(repo.git_dir())?.sync_all()?;
        Ok(session)
    }
    pub fn directory(&self) -> &Path {
        &self.directory
    }
    /// Acknowledge a quiescent unknown post-hook without repeating its side effects.
    pub fn acknowledge_post_hook(&mut self, repo: &gix::Repository) -> Result<()> {
        self.ownership(repo)?;
        let (number, post) = self.state.pending_hook.context("no pending hook")?;
        ensure!(post, "unknown pre-hook outcome is abort-only");
        super::invocation::inspect(&self.directory.join(format!("invocation-{number}")))?;
        self.validate_observed(repo, &self.state.active)?;
        let staged = gix::index::File::at(
            self.directory.join("index.private"),
            repo.object_hash(),
            false,
            Default::default(),
        )?;
        let next = entries(&staged);
        ensure!(
            changed(&self.state.private, &next)
                .iter()
                .all(|p| self.state.active.contains(p)),
            "post-hook mutation outside scope"
        );
        let mut after = self.state.clone();
        after.pending_hook = None;
        after.private = next;
        after.public = overlay(&after.private, &after.original, &after.preserved);
        self.transition(repo, after)
    }

    fn save(&self, repo: &gix::Repository) -> Result<()> {
        persist(
            &self.directory.join("index.private"),
            &bytes(repo, &self.state.private)?,
        )?;
        persist(&self.directory.join("state.json"), &sealed(&self.state)?)
    }
    pub fn open(repo: &gix::Repository) -> Result<Self> {
        let directory = repo.git_dir().join("brit-authoring");
        let state: State = load(&directory.join("state.json"))?;
        let s = Self { directory, state };
        s.ownership(repo)?;
        Ok(s)
    }
    fn ownership(&self, repo: &gix::Repository) -> Result<()> {
        validate_metadata_directories(repo)?;
        use std::os::unix::fs::{MetadataExt, PermissionsExt};
        let meta = fs::symlink_metadata(&self.directory)?;
        ensure!(
            meta.is_dir()
                && meta.permissions().mode().trailing_zeros() >= 6
                && meta.uid() == fs::metadata(repo.git_dir())?.uid(),
            "unsafe authoring directory"
        );
        ensure!(
            self.state.identities == identities(repo, &self.directory)?,
            "foreign directory identity"
        );
        for member in fs::read_dir(&self.directory)? {
            let member = member?;
            let name = member.file_name();
            let name = name.to_str().context("foreign journal member")?;
            let member_meta = fs::symlink_metadata(member.path())?;
            ensure!(
                !member_meta.file_type().is_symlink() && member_meta.uid() == meta.uid(),
                "foreign journal member type or owner"
            );
            if name == "objects" || name.starts_with("invocation-") {
                ensure!(
                    member_meta.is_dir() && member_meta.permissions().mode().trailing_zeros() >= 6,
                    "unsafe journal subdirectory"
                );
            } else {
                ensure!(member_meta.is_file(), "unexpected journal directory");
            }
            let numbered = |prefix: &str, suffix: &str| {
                name.strip_prefix(prefix)
                    .and_then(|s| s.strip_suffix(suffix))
                    .is_some_and(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()))
            };
            ensure!(
                matches!(
                    name,
                    "index.original"
                        | "index.private"
                        | "index.private.lock"
                        | "index.new"
                        | "state.json"
                        | "state.new"
                        | "message"
                        | "message.new"
                        | "abort.json"
                        | "abort.new"
                        | "objects"
                ) || numbered("child-", ".json")
                    || numbered("child-", ".new")
                    || numbered("invocation-", ""),
                "foreign journal member"
            );
        }
        for name in ["index.original", "index.private"] {
            let m = fs::symlink_metadata(self.directory.join(name))?;
            ensure!(
                m.is_file()
                    && m.uid() == meta.uid()
                    && (name == "index.private" || m.permissions().mode().trailing_zeros() >= 6),
                "unsafe index member"
            );
        }
        ensure!(
            digest(&fs::read(self.directory.join("index.original"))?)? == self.state.original_index,
            "corrupt original index"
        );
        ensure!(
            self.state.version == 1
                && self.state.owner == raw(&fs::canonicalize(repo.git_dir())?)?
                && self.state.root == raw(&fs::canonicalize(repo.workdir().context("worktree required")?)?)?
                && self.state.common == raw(&fs::canonicalize(repo.common_dir())?)?,
            "foreign authoring ownership"
        );
        Ok(())
    }
    pub fn validate(&self, repo: &gix::Repository) -> Result<()> {
        self.validate_scope(repo, &self.state.active)
    }
    fn validate_scope(&self, repo: &gix::Repository, active: &[Vec<u8>]) -> Result<()> {
        self.ownership(repo)?;
        ensure!(
            load::<State>(&self.directory.join("state.json"))? == self.state,
            "stale authoring session"
        );
        ensure!(
            self.state.pending_hook.is_none(),
            "unknown hook outcome: inspect invocation; never automatically retry"
        );
        self.validate_observed(repo, active)
    }
    fn validate_observed(&self, repo: &gix::Repository, active: &[Vec<u8>]) -> Result<()> {
        ensure!(policy(repo)? == self.state.policy, "hook or configuration changed");
        ensure!(
            repo.head()?.referent_name().is_none()
                && repo.head_commit()?.id == gix::ObjectId::from_hex(self.state.head.as_bytes())?,
            "external HEAD movement"
        );
        let current = entries(&*read_public(repo)?);
        ensure!(
            changed(&self.state.public, &current)
                .iter()
                .all(|p| self.state.active.contains(p)),
            "index mutation outside resolution scope"
        );
        self.validate_scope_work(repo, active)
    }
    fn validate_scope_work(&self, repo: &gix::Repository, active: &[Vec<u8>]) -> Result<()> {
        let current_work = snapshot(repo.workdir().context("worktree required")?)?;
        for p in self.state.work.keys().chain(current_work.keys()) {
            if !active.contains(p) {
                ensure!(
                    self.state.work.get(p) == current_work.get(p),
                    "worktree mutation outside resolution scope"
                );
            }
        }
        Ok(())
    }
    pub fn install_step(&mut self, repo: &gix::Repository, step: &gix::index::State) -> Result<()> {
        self.validate(repo)?;
        step.verify_entries()?;
        let next = entries(step);
        ensure!(
            changed(&self.state.private, &next)
                .iter()
                .all(|p| self.state.active.contains(p)),
            "step changes outside scope"
        );
        let mut after = self.state.clone();
        after.private = next;
        after.public = overlay(&after.private, &after.original, &after.preserved);
        self.transition(repo, after)
    }
    pub fn stage(&mut self, repo: &gix::Repository, paths: &[BString], expand: bool) -> Result<()> {
        let paths = super::super::index::authoring::literal_paths(repo, paths)?;
        let mut after = self.state.clone();
        for p in &paths {
            if !after.active.contains(&p.to_vec()) {
                ensure!(expand && after.kind == StopKind::Edit, "path outside resolution scope");
                ensure!(
                    !after.preserved.iter().any(|other| overlaps(other, p)),
                    "scope expansion overlaps preservation"
                );
                ensure!(
                    after.original.iter().any(|e| e.path == p.as_bytes()) || !after.work.contains_key(p.as_bytes()),
                    "scope expansion collides with pre-existing untracked path"
                );
                after
                    .expanded
                    .insert(p.to_vec(), image(repo.workdir().context("worktree")?, p)?);
                after.before.insert(p.to_vec(), self.original_image(repo, p)?);
                after.active.push(p.to_vec());
            }
        }
        // Validate with only the explicitly requested expansion admitted.
        self.validate_scope(repo, &after.active)?;
        // Preserve independently staged resolutions while changing only named paths.
        let public = read_public(repo)?;
        validate_public(&public)?;
        after.private = overlay(&after.private, &entries(&public), &self.state.active);
        after.public = overlay(&after.private, &after.original, &after.preserved);
        // Journal scope/before-images before writing any new blob or staging.
        self.transition(repo, after)?;
        let mut after = self.state.clone();
        let current = index(repo, &after.private)?;
        for p in paths {
            after.private.retain(|e| e.path != p.as_bytes());
            if let Some((mode, data)) = super::super::index::authoring::worktree_entry(repo, &current, &p)? {
                let id = repo.write_blob(&data)?;
                after.private.push(Entry {
                    path: p.to_vec(),
                    id: id.to_string(),
                    mode: mode.bits(),
                    flags: 0,
                });
            }
        }
        after.private.sort_by(|a, b| a.path.cmp(&b.path));
        after.public = overlay(&after.private, &after.original, &after.preserved);
        self.transition(repo, after)
    }
    fn original_image(&self, repo: &gix::Repository, p: &[u8]) -> Result<Option<Image>> {
        let Some(entry) = self.state.original.iter().find(|e| e.path == p) else {
            return Ok(None);
        };
        let data = repo
            .find_object(gix::ObjectId::from_hex(entry.id.as_bytes())?)?
            .data
            .clone();
        match self.state.work.get(p) {
            Some(Image::Link(target)) => {
                ensure!(*target == data, "original symlink mismatch");
                Ok(Some(Image::Link(data)))
            }
            Some(Image::File(hash, mode)) => {
                ensure!(
                    *hash == digest(&data)?.as_bytes(),
                    "expanded path requires unsupported worktree conversion"
                );
                Ok(Some(Image::File(data, *mode)))
            }
            None => anyhow::bail!("missing original worktree image"),
        }
    }
    pub fn import_external(&mut self, repo: &gix::Repository) -> Result<()> {
        self.validate(repo)?;
        let current = read_public(repo)?;
        validate_public(&current)?;
        let mut after = self.state.clone();
        after.private = overlay(&after.private, &entries(&current), &after.active);
        if !after.split {
            self.clean(repo, &after.private)?;
        }
        after.public = overlay(&after.private, &after.original, &after.preserved);
        self.transition(repo, after)
    }
    fn clean(&self, repo: &gix::Repository, values: &[Entry]) -> Result<()> {
        let state = index(repo, values)?;
        super::super::index::authoring::validate_index(&state)?;
        for p in &self.state.active {
            let current = super::super::index::authoring::worktree_entry(repo, &state, &BString::from(p.clone()))?;
            let expected = state.entry_by_path(p.as_bstr());
            ensure!(
                match (current, expected) {
                    (None, None) => true,
                    (Some((mode, data)), Some(e)) => mode == e.mode && repo.find_object(e.id)?.data == data,
                    _ => false,
                },
                "unstaged resolution changes"
            );
        }
        Ok(())
    }
    /// Check the clean, linear authored chain before a future sequencer accepts this stop.
    pub fn validate_completion(&self, repo: &gix::Repository) -> Result<()> {
        self.validate(repo)?;
        ensure!(
            changed(&self.state.public, &entries(&*read_public(repo)?)).is_empty(),
            "external staging must be imported before completion"
        );
        self.clean(repo, &self.state.private)?;
        if self.state.kind == StopKind::Edit {
            let committed = repo.index_from_tree(&repo.head_commit()?.tree_id()?)?;
            ensure!(
                changed(&self.state.private, &entries(&committed)).is_empty(),
                "staged edit work must be committed before completion"
            );
            let mut current = self.state.head.clone();
            while current != self.state.parent {
                let commit = repo.find_commit(gix::ObjectId::from_hex(current.as_bytes())?)?;
                ensure!(commit.parent_ids().count() == 1, "authored chain must remain linear");
                current = commit.parent_ids().next().context("linear parent")?.to_string();
            }
        }
        Ok(())
    }
    pub fn private_tree(&self, repo: &gix::Repository) -> Result<gix::ObjectId> {
        self.validate(repo)?;
        let state = index(repo, &self.state.private)?;
        super::super::index::authoring::validate_index(&state)?;
        let mut tree = repo.empty_tree().edit()?;
        for e in state.entries() {
            tree.upsert(
                e.path(&state),
                e.mode.to_tree_entry_mode().context("mode")?.kind(),
                e.id,
            )?;
        }
        Ok(tree.write()?.detach())
    }
    fn transition(&mut self, repo: &gix::Repository, mut after: State) -> Result<()> {
        let _guard = super::super::switch::shared_guard(repo)?;
        if let Some((number, _)) = self.state.pending_hook {
            ensure!(after.pending_hook.is_none(), "pending invocation is not acknowledged");
            self.ownership(repo)?;
            ensure!(
                load::<State>(&self.directory.join("state.json"))? == self.state,
                "stale session"
            );
            super::invocation::inspect(&self.directory.join(format!("invocation-{number}")))?;
            self.validate_observed(repo, &after.active)?;
        } else {
            self.validate_scope(repo, &after.active)?;
        }
        sync_objects(&repo.common_dir().join("objects"))?;
        let old_index = fs::read(repo.index_path())?;
        after.children += 1;
        ensure!(
            !self.directory.join(format!("child-{}.json", after.children)).exists(),
            "decided child publication pending; recover before authoring"
        );
        let old_head = fs::read(repo.git_dir().join("HEAD"))?;
        let old_log = read_optional(&repo.git_dir().join("logs/HEAD"))?;
        let mut new_log = old_log.clone();
        if after.head != self.state.head {
            let log = new_log.get_or_insert_with(Vec::new);
            gix::refs::log::Line {
                previous_oid: gix::ObjectId::from_hex(self.state.head.as_bytes())?,
                new_oid: gix::ObjectId::from_hex(after.head.as_bytes())?,
                signature: repo.committer().context("committer")??.to_owned()?,
                message: "brit edit-stop authoring".into(),
            }
            .write_to(log)?;
        }
        let decision = Decision {
            before: self.state.clone(),
            new_index: bytes(repo, &after.public)?,
            after: after.clone(),
            old_index,
            old_head,
            new_head: format!("{}\n", after.head).into_bytes(),
            old_log,
            new_log,
        };
        persist(
            &self.directory.join(format!("child-{}.json", after.children)),
            &sealed(&decision)?,
        )?;
        self.apply(repo, &decision)?;
        self.state = after;
        self.save(repo)
    }
    fn validate_decision_resources(&self, repo: &gix::Repository, d: &Decision) -> Result<()> {
        let current = fs::read(repo.index_path())?;
        ensure!(
            current == d.old_index || current == d.new_index,
            "third-state index refuses recovery"
        );
        let head = repo.head()?;
        ensure!(head.referent_name().is_none(), "external HEAD attachment");
        let id = head.id().context("HEAD")?.to_string();
        ensure!(
            id == d.before.head || id == d.after.head,
            "third-state HEAD refuses recovery"
        );
        let head_path = repo.git_dir().join("HEAD");
        let log_path = repo.git_dir().join("logs/HEAD");
        for (resource, old, new) in [
            (&head_path, Some(d.old_head.clone()), Some(d.new_head.clone())),
            (&log_path, d.old_log.clone(), d.new_log.clone()),
            (&repo.index_path(), Some(d.old_index.clone()), Some(d.new_index.clone())),
        ] {
            validate_metadata_directories(repo)?;
            let observed = read_optional(resource)?;
            ensure!(
                observed == old || observed == new,
                "third-state metadata refuses recovery"
            );
        }
        Ok(())
    }
    fn apply(&self, repo: &gix::Repository, d: &Decision) -> Result<()> {
        validate_metadata_directories(repo)?;
        self.validate_decision_resources(repo, d)?;
        let head_path = repo.git_dir().join("HEAD");
        let log_path = repo.git_dir().join("logs/HEAD");
        for (resource, old, new) in [
            (&head_path, Some(d.old_head.clone()), Some(d.new_head.clone())),
            (&log_path, d.old_log.clone(), d.new_log.clone()),
            (&repo.index_path(), Some(d.old_index.clone()), Some(d.new_index.clone())),
        ] {
            if old == new {
                continue;
            }
            validate_metadata_directories(repo)?;
            let observed = read_optional(resource)?;
            ensure!(
                observed == old || observed == new,
                "metadata changed before publication"
            );
            if observed != new {
                let data = new.context("metadata deletion unsupported")?;
                let parent = resource.parent().context("metadata parent")?;
                fs::create_dir_all(parent)?;
                let mut lock = gix::lock::File::acquire_to_update_resource(
                    resource,
                    gix::lock::acquire::Fail::Immediately,
                    None,
                    0,
                )?;
                ensure!(read_optional(resource)? == old, "metadata changed under lock");
                lock.write_all(&data)?;
                lock.commit()?;
                fs::File::open(resource)?.sync_all()?;
                fs::File::open(parent)?.sync_all()?;
                #[cfg(feature = "switch-test-faults")]
                if resource == &head_path
                    && std::env::var_os("BRIT_AUTHORING_FAIL_AFTER").as_deref()
                        == Some(std::ffi::OsStr::new("head-publication"))
                {
                    anyhow::bail!("injected failure after HEAD publication");
                }
            }
        }
        fs::File::open(repo.git_dir())?.sync_all()?;
        Ok(())
    }
    pub fn recover(repo: &gix::Repository) -> Result<Self> {
        let _guard = super::super::switch::shared_guard(repo)?;
        let mut s = Self::open(repo)?;
        let next = s.directory.join(format!("child-{}.json", s.state.children + 1));
        if next.exists() {
            let d: Decision = load(&next)?;
            ensure!(d.before == s.state, "foreign child decision");
            if let Some((number, _)) = s.state.pending_hook {
                super::invocation::inspect(&s.directory.join(format!("invocation-{number}")))?;
            }
            s.validate_scope_work(repo, &d.after.active)?;
            s.apply(repo, &d)?;
            s.state = d.after;
            s.save(repo)?;
        } else {
            s.validate(repo)?;
        }
        Ok(s)
    }
    /// Retain all authored bytes before restoring the stop's original HEAD/index/files.
    /// Unrelated new work is left in place. Unknown pre-hooks can only take this path.
    pub fn abort(&mut self, repo: &gix::Repository) -> Result<PathBuf> {
        let _guard = super::super::switch::shared_guard(repo)?;
        self.ownership(repo)?;
        ensure!(
            !self
                .directory
                .join(format!("child-{}.json", self.state.children + 1))
                .exists(),
            "decided child publication pending; recover before abort"
        );
        if let Some((number, _)) = self.state.pending_hook {
            super::invocation::inspect(&self.directory.join(format!("invocation-{number}")))?;
            self.validate_scope_work(repo, &self.state.active)?;
            let staged = gix::index::File::at(
                self.directory.join("index.private"),
                repo.object_hash(),
                false,
                Default::default(),
            )?;
            ensure!(
                changed(&self.state.private, &entries(&staged))
                    .iter()
                    .all(|p| self.state.active.contains(p)),
                "unknown hook changed private index outside scope; inspection only"
            );
        }
        let receipt = self.directory.join("abort.json");
        let plan: Abort = if receipt.exists() {
            load(&receipt)?
        } else {
            ensure!(
                repo.head()?.referent_name().is_none()
                    && repo.head_commit()?.id == gix::ObjectId::from_hex(self.state.head.as_bytes())?,
                "external HEAD movement"
            );
            let root = repo.workdir().context("worktree")?;
            for p in &self.state.preserved {
                ensure!(
                    image(root, p)? == *self.state.before.get(p).context("preserved before image")?,
                    "preserved work changed"
                );
            }
            let observed = entries(&*read_public(repo)?);
            ensure!(
                changed(&self.state.public, &observed)
                    .iter()
                    .all(|p| self.state.active.contains(p)),
                "foreign index change during abort"
            );
            let mut after = self.state.clone();
            std::str::from_utf8(&self.state.original_head)?
                .trim()
                .clone_into(&mut after.head);
            after.pending_hook = None;
            after.public.clone_from(&after.original);
            after.private.clone_from(&after.original);
            let changes = self
                .state
                .active
                .iter()
                .map(|p| Ok((p.clone(), image(root, p)?, self.state.before.get(p).cloned().flatten())))
                .collect::<Result<Vec<_>>>()?;
            let plan = Abort {
                decision: Decision {
                    before: self.state.clone(),
                    after,
                    old_index: fs::read(repo.index_path())?,
                    new_index: fs::read(self.directory.join("index.original"))?,
                    old_head: fs::read(repo.git_dir().join("HEAD"))?,
                    new_head: self.state.original_head.clone(),
                    old_log: read_optional(&repo.git_dir().join("logs/HEAD"))?,
                    new_log: self.state.original_log.clone(),
                },
                changes,
                private: fs::read(self.directory.join("index.private"))?,
            };
            self.retain_objects(repo)?;
            persist(&receipt, &sealed(&plan)?)?;
            plan
        };
        ensure!(plan.decision.before == self.state, "foreign abort receipt");
        let root = repo.workdir().context("worktree")?;
        for (p, before, after) in &plan.changes {
            let current = image(root, p)?;
            ensure!(
                current == *before || current == *after,
                "third-state worktree refuses abort"
            );
        }
        for p in &self.state.preserved {
            ensure!(
                image(root, p)? == *self.state.before.get(p).context("preserved before image")?,
                "preserved work changed"
            );
        }
        self.validate_decision_resources(repo, &plan.decision)?;
        self.retain_objects(repo)?;
        // Validate metadata before any worktree write. apply itself compares each live resource again.
        for (p, before, after) in &plan.changes {
            if image(root, p)? != *after {
                restore_image(root, p, before.as_ref(), after.as_ref())?;
            }
        }
        self.apply(repo, &plan.decision)?;
        let retained = repo
            .git_dir()
            .join(format!("brit-authoring-retained-{}", digest(&fs::read(&receipt)?)?));
        ensure!(!retained.exists(), "retention name already exists");
        fs::rename(&self.directory, &retained)?;
        fs::File::open(repo.git_dir())?.sync_all()?;
        self.directory.clone_from(&retained);
        self.state = plan.decision.after;
        Ok(retained)
    }
    fn retain_objects(&self, repo: &gix::Repository) -> Result<()> {
        let directory = self.directory.join("objects");
        if !directory.exists() {
            use std::os::unix::fs::DirBuilderExt;
            fs::DirBuilder::new().mode(0o700).create(&directory)?;
        }
        let mut stack = self.state.private.iter().map(|e| e.id.clone()).collect::<Vec<_>>();
        stack.extend(read_public(repo)?.entries().iter().map(|e| e.id.to_string()));
        let staged = gix::index::File::at(
            self.directory.join("index.private"),
            repo.object_hash(),
            false,
            Default::default(),
        )?;
        stack.extend(staged.entries().iter().map(|e| e.id.to_string()));
        stack.push(self.state.head.clone());
        for n in 1..=self.state.children {
            let decision: Decision = load(&self.directory.join(format!("child-{n}.json")))?;
            stack.push(decision.after.head);
            stack.extend(decision.after.private.iter().map(|e| e.id.clone()));
        }
        let baseline = std::str::from_utf8(&self.state.original_head)?.trim();
        let mut seen = BTreeSet::new();
        while let Some(id) = stack.pop() {
            if id == baseline || id == self.state.parent || !seen.insert(id.clone()) {
                continue;
            }
            let receipt = directory.join(&id);
            let retained = if receipt.exists() {
                load::<RetainedObject>(&receipt)?
            } else {
                let object = repo.find_object(gix::ObjectId::from_hex(id.as_bytes())?)?;
                let retained = RetainedObject {
                    id: id.clone(),
                    kind: object.kind.to_string(),
                    data: object.data.clone(),
                };
                persist(&receipt, &sealed(&retained)?)?;
                retained
            };
            let kind = gix::objs::Kind::from_bytes(retained.kind.as_bytes())?;
            ensure!(
                retained.id == id
                    && gix::objs::compute_hash(repo.object_hash(), kind, &retained.data)?
                        == gix::ObjectId::from_hex(id.as_bytes())?,
                "retained object identity mismatch"
            );
            match kind {
                gix::objs::Kind::Commit => {
                    let commit = gix::objs::CommitRef::from_bytes(&retained.data, repo.object_hash())?;
                    stack.push(commit.tree().to_string());
                    stack.extend(commit.parents().map(|p| p.to_string()));
                }
                gix::objs::Kind::Tree => {
                    for entry in gix::objs::TreeRef::from_bytes(&retained.data, repo.object_hash())?.entries {
                        ensure!(!entry.mode.is_commit(), "gitlinks unsupported");
                        stack.push(entry.oid.to_string());
                    }
                }
                gix::objs::Kind::Blob => {}
                _ => anyhow::bail!("unexpected authored object kind"),
            }
        }
        fs::File::open(&directory)?.sync_all()?;
        fs::File::open(&self.directory)?.sync_all()?;
        Ok(())
    }
    pub fn begin_split(&mut self, repo: &gix::Repository) -> Result<()> {
        self.import_external(repo)?;
        ensure!(
            self.state.kind == StopKind::Edit && !self.state.split,
            "not an unsplit edit stop"
        );
        let parent = repo.find_commit(gix::ObjectId::from_hex(self.state.parent.as_bytes())?)?;
        let mut after = self.state.clone();
        after.head.clone_from(&after.parent);
        after.private = entries(&*repo.index_from_tree(&parent.tree_id()?)?);
        ensure!(
            changed(&self.state.private, &after.private)
                .iter()
                .all(|p| after.active.contains(p)),
            "split parent changes outside edit scope"
        );
        after.public = overlay(&after.private, &after.original, &after.preserved);
        after.split = true;
        self.transition(repo, after)
    }
    pub fn amend(&mut self, repo: &gix::Repository, message: &str) -> Result<Outcome> {
        ensure!(
            self.state.kind == StopKind::Edit && !self.state.split,
            "amend requires unsplit edit stop"
        );
        self.author(repo, message, true)
    }
    pub fn commit_split(&mut self, repo: &gix::Repository, message: &str) -> Result<Outcome> {
        ensure!(
            self.state.kind == StopKind::Edit && self.state.split,
            "split has not started"
        );
        self.author(repo, message, false)
    }
    fn author(&mut self, repo: &gix::Repository, message: &str, amend: bool) -> Result<Outcome> {
        self.import_external(repo)?;
        ensure!(!message.trim().is_empty(), "empty message");
        ensure!(
            !repo.config_snapshot().try_boolean("commit.gpgSign")?.unwrap_or(false),
            "required signing unsupported"
        );
        let message_path = self.directory.join("message");
        persist(&message_path, message.as_bytes())?;
        self.hook(repo, "pre-commit", &[], None, false)?;
        let mut args = vec![message_path.clone().into_os_string()];
        if amend {
            args.extend(["commit".into(), self.state.head.clone().into()]);
        } else {
            args.push("message".into());
        }
        self.hook(repo, "prepare-commit-msg", &args, None, false)?;
        self.hook(repo, "commit-msg", &[message_path.into_os_string()], None, false)?;
        let message = fs::read(self.directory.join("message"))?;
        ensure!(!message.iter().all(u8::is_ascii_whitespace), "empty hook message");
        let previous = repo.head_commit()?;
        let author = if amend {
            previous.author()?.to_owned()?
        } else {
            repo.author().context("author")??.to_owned()?
        };
        let committer = repo.committer().context("committer")??.to_owned()?;
        let parents = if amend {
            previous.parent_ids().map(gix::Id::detach).collect()
        } else {
            vec![previous.id].into_iter().collect()
        };
        let tree = self.private_tree(repo)?;
        let commit = repo
            .write_object(&gix::objs::Commit {
                tree,
                parents,
                author,
                committer,
                message: message.into(),
                encoding: None,
                extra_headers: Default::default(),
            })?
            .detach();
        // Synchronize object files before persisting a decision; private snapshots never enter the ODB.
        sync_objects(&repo.common_dir().join("objects"))?;
        let old = self.state.head.clone();
        let mut after = self.state.clone();
        after.head = commit.to_string();
        if let Err(error) = self.transition(repo, after) {
            let state = repo.head();
            return match state {
                Ok(head) if head.referent_name().is_none() && head.id().map(|id| id.detach()) == Some(commit) => {
                    Ok(Outcome {
                        commit,
                        warnings: vec![format!(
                            "commit {commit} published with publication pending: {error:#}; recover child receipt before any hook"
                        )],
                    })
                }
                Ok(_) => {
                    Err(error.context("commit publication incomplete; inspect durable child receipt before retrying"))
                }
                Err(inspect) => Err(error.context(format!(
                    "commit publication outcome uncertain ({inspect}); inspect child receipt"
                ))),
            };
        }
        let mut warnings = Vec::new();
        if let Err(e) = self.hook(repo, "post-commit", &[], None, true) {
            warnings.push(format!("commit {commit} published; {e:#}"));
        }
        if amend && self.state.pending_hook.is_none() {
            let mapping = format!("{old} {commit}\n");
            if let Err(e) = self.hook(repo, "post-rewrite", &["amend".into()], Some(mapping.as_bytes()), true) {
                warnings.push(format!("commit {commit} published; {e:#}"));
            }
        }
        Ok(Outcome { commit, warnings })
    }
    fn hook(
        &mut self,
        repo: &gix::Repository,
        name: &str,
        args: &[std::ffi::OsString],
        input: Option<&[u8]>,
        post: bool,
    ) -> Result<()> {
        if super::hook_path(repo, name)?.is_none() {
            return Ok(());
        }
        let prepared = {
            let _guard = super::super::switch::shared_guard(repo)?;
            self.validate(repo)?;
            self.state.invocation += 1;
            let prepared =
                super::invocation::prepare(&self.directory.join(format!("invocation-{}", self.state.invocation)))?;
            self.state.pending_hook = Some((self.state.invocation, post));
            self.save(repo)?;
            prepared
        };
        let outcome =
            super::run_prepared_hook(repo, name, args, input, &self.directory.join("index.private"), prepared)?;
        ensure!(
            outcome != super::invocation::InvocationOutcome::Unknown,
            "unknown hook outcome; explicit recovery required"
        );
        self.validate_observed(repo, &self.state.active)?;
        let staged = gix::index::File::at(
            self.directory.join("index.private"),
            repo.object_hash(),
            false,
            Default::default(),
        )?;
        let next = entries(&staged);
        ensure!(
            changed(&self.state.private, &next)
                .iter()
                .all(|p| self.state.active.contains(p)),
            "hook staged outside resolution scope"
        );
        if !post && !self.state.split {
            self.clean(repo, &next)?;
        }
        let mut after = self.state.clone();
        after.pending_hook = None;
        after.private = next;
        after.public = overlay(&after.private, &after.original, &after.preserved);
        self.transition(repo, after)?;
        ensure!(outcome.success(), "{name} hook rejected authoring");
        Ok(())
    }
}
fn sync_objects(path: &Path) -> Result<()> {
    for entry in fs::read_dir(path)? {
        let entry = entry?;
        let metadata = entry.file_type()?;
        if metadata.is_dir() {
            sync_objects(&entry.path())?;
        } else if metadata.is_file() {
            fs::File::open(entry.path())?.sync_all()?;
        } else {
            ensure!(!metadata.is_symlink(), "object symlink unsupported");
        }
    }
    fs::File::open(path)?.sync_all()?;
    Ok(())
}

mod byte_map {
    use super::*;
    pub fn serialize<S: serde::Serializer, V: Serialize>(
        map: &BTreeMap<Vec<u8>, V>,
        serializer: S,
    ) -> std::result::Result<S::Ok, S::Error> {
        map.iter().collect::<Vec<_>>().serialize(serializer)
    }
    pub fn deserialize<'de, D: serde::Deserializer<'de>, V: Deserialize<'de>>(
        deserializer: D,
    ) -> std::result::Result<BTreeMap<Vec<u8>, V>, D::Error> {
        Ok(Vec::<(Vec<u8>, V)>::deserialize(deserializer)?.into_iter().collect())
    }
}

fn digest(bytes: &[u8]) -> Result<String> {
    let mut hash = gix::hash::hasher(gix::hash::Kind::Sha1);
    hash.update(bytes);
    Ok(hash.try_finalize()?.to_string())
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Envelope {
    payload: Vec<u8>,
    checksum: String,
}
fn sealed(value: &impl Serialize) -> Result<Vec<u8>> {
    let payload = serde_json::to_vec(value)?;
    let checksum = digest(&payload)?;
    Ok(serde_json::to_vec(&Envelope { payload, checksum })?)
}
fn load<T: serde::de::DeserializeOwned>(path: &Path) -> Result<T> {
    ensure!(
        fs::symlink_metadata(path)?.is_file(),
        "journal member must be regular file"
    );
    use std::os::unix::fs::{MetadataExt, PermissionsExt};
    let metadata = fs::symlink_metadata(path)?;
    ensure!(
        metadata.permissions().mode().trailing_zeros() >= 6
            && metadata.uid() == fs::metadata(path.parent().context("parent")?)?.uid(),
        "unsafe journal member"
    );
    let envelope: Envelope = serde_json::from_slice(&fs::read(path)?)?;
    ensure!(
        digest(&envelope.payload)? == envelope.checksum,
        "corrupt journal checksum"
    );
    Ok(serde_json::from_slice(&envelope.payload)?)
}
fn image(root: &Path, p: &[u8]) -> Result<Option<Image>> {
    let file = root.join(gix::path::from_bstr(p.as_bstr())?);
    let metadata = match fs::symlink_metadata(&file) {
        Ok(m) => m,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e.into()),
    };
    if metadata.file_type().is_symlink() {
        return Ok(Some(Image::Link(raw(&fs::read_link(file)?)?)));
    }
    ensure!(metadata.is_file(), "before image must be file or symlink");
    #[cfg(unix)]
    let mode = {
        use std::os::unix::fs::PermissionsExt;
        metadata.permissions().mode() & 0o777
    };
    #[cfg(not(unix))]
    let mode = 0;
    Ok(Some(Image::File(fs::read(file)?, mode)))
}

fn read_optional(path: &Path) -> Result<Option<Vec<u8>>> {
    match fs::symlink_metadata(path) {
        Ok(m) => {
            ensure!(m.is_file(), "metadata must be regular file");
            Ok(Some(fs::read(path)?))
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e.into()),
    }
}

fn identities(repo: &gix::Repository, directory: &Path) -> Result<Vec<(u64, u64)>> {
    use std::os::unix::fs::MetadataExt;
    [
        repo.git_dir(),
        repo.common_dir(),
        repo.workdir().context("worktree")?,
        directory,
    ]
    .into_iter()
    .map(|p| {
        let m = fs::metadata(p)?;
        Ok((m.dev(), m.ino()))
    })
    .collect()
}
fn policy(repo: &gix::Repository) -> Result<String> {
    let mut values = Vec::new();
    for path in [repo.common_dir().join("config"), repo.git_dir().join("config.worktree")] {
        values.push((raw(&path)?, read_optional(&path)?));
    }
    for name in [
        "pre-commit",
        "prepare-commit-msg",
        "commit-msg",
        "post-commit",
        "post-rewrite",
    ] {
        if let Some(path) = super::hook_path(repo, name)? {
            values.push((raw(&path)?, read_optional(&path)?));
        }
    }
    digest(&serde_json::to_vec(&values)?)
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Abort {
    decision: Decision,
    changes: Vec<(Vec<u8>, Option<Image>, Option<Image>)>,
    private: Vec<u8>,
}
fn restore_image(root: &Path, p: &[u8], before: Option<&Image>, after: Option<&Image>) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    super::super::index::authoring::validate_relative_path(root, p.as_bstr())?;
    ensure!(
        image(root, p)?.as_ref() == before,
        "worktree changed before restoration"
    );
    let file = root.join(gix::path::from_bstr(p.as_bstr())?);
    let parent = file.parent().context("file parent")?;
    match after {
        None => {
            if fs::symlink_metadata(&file).is_ok() {
                fs::remove_file(&file)?;
            }
        }
        Some(Image::File(bytes, mode)) => {
            fs::create_dir_all(parent)?;
            let mut temporary = tempfile::NamedTempFile::new_in(parent)?;
            temporary.write_all(bytes)?;
            temporary.as_file().set_permissions(fs::Permissions::from_mode(*mode))?;
            temporary.as_file().sync_all()?;
            ensure!(image(root, p)?.as_ref() == before, "work changed before replace");
            temporary.persist(&file)?;
        }
        Some(Image::Link(bytes)) => {
            fs::create_dir_all(parent)?;
            let temporary = tempfile::NamedTempFile::new_in(parent)?.into_temp_path();
            fs::remove_file(&temporary)?;
            std::os::unix::fs::symlink(gix::path::from_bstr(bytes.as_bstr())?, &temporary)?;
            fs::File::open(parent)?.sync_all()?;
            ensure!(image(root, p)?.as_ref() == before, "work changed before replace");
            fs::rename(&temporary, &file)?;
        }
    }
    fs::File::open(parent)?.sync_all()?;
    Ok(())
}

#[derive(Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct RetainedObject {
    id: String,
    kind: String,
    data: Vec<u8>,
}

fn validate_public(index: &gix::index::State) -> Result<()> {
    use gix::index::entry::Flags;
    index.verify_entries()?;
    ensure!(
        !index.is_sparse() && index.link().is_none(),
        "sparse/split index unsupported"
    );
    ensure!(
        index
            .entries()
            .iter()
            .all(|e| !e.flags.intersects(Flags::ASSUME_VALID | Flags::SKIP_WORKTREE)),
        "special index flags unsupported"
    );
    Ok(())
}

fn read_public(repo: &gix::Repository) -> Result<gix::index::File> {
    ensure!(
        fs::symlink_metadata(repo.index_path())?.is_file(),
        "public index must be a regular file"
    );
    let file = repo.open_index()?;
    let (raw, _) = gix::index::State::from_bytes(
        &fs::read(repo.index_path())?,
        file.timestamp(),
        repo.object_hash(),
        Default::default(),
    )?;
    ensure!(raw.link().is_none(), "split index unsupported");
    validate_public(&file)?;
    Ok(file)
}

// A regular logs/HEAD leaf is insufficient: a symlinked logs directory would
// redirect publication outside the owned Git directory.
fn validate_metadata_directories(repo: &gix::Repository) -> Result<()> {
    use std::os::unix::fs::MetadataExt;
    for directory in [repo.git_dir(), repo.common_dir()] {
        let root = fs::symlink_metadata(directory)?;
        ensure!(root.is_dir(), "unsafe Git metadata directory");
        match fs::symlink_metadata(directory.join("logs")) {
            Ok(metadata) => ensure!(
                metadata.is_dir() && metadata.dev() == root.dev(),
                "unsafe reflog directory"
            ),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
    }
    Ok(())
}
