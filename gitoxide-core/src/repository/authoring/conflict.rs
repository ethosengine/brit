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
/// Original staging and dirty files, captured once before replay changes the checkout.
/// This value is private journal data; its file payloads must never enter a commit tree.
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Preservation {
    original_index: Vec<u8>,
    original: Vec<Entry>,
    preserved: Vec<Vec<u8>>,
    clean_preserved: Vec<Entry>,
    #[serde(with = "byte_map")]
    before: BTreeMap<Vec<u8>, Option<Image>>,
}

impl Preservation {
    /// Capture without changing the index, object store or worktree.
    /// The caller must hold the repository mutation guard until this is journaled.
    pub fn capture(repo: &gix::Repository) -> Result<Self> {
        validate_metadata_directories(repo)?;
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
        let base = repo.index_from_tree(&repo.head_commit()?.tree_id()?)?;
        let original_entries = entries(&original);
        let mut preserved = changed(&entries(&base), &original_entries);
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
        let root = repo.workdir().context("worktree required")?;
        let mut before = BTreeMap::new();
        for p in &preserved {
            before.insert(p.clone(), image(root, p)?);
        }
        let clean_preserved = entries(&base)
            .into_iter()
            .filter(|e| preserved.contains(&e.path))
            .collect();
        Ok(Self {
            original_index: raw_index,
            original: original_entries,
            preserved,
            clean_preserved,
            before,
        })
    }

    pub fn original_index(&self) -> &[u8] {
        &self.original_index
    }

    /// Original staged blobs may be absent from every commit closure. Retained
    /// copies must include them as raw private payloads before releasing refs.
    pub(crate) fn original_object_ids(&self) -> Result<BTreeSet<gix::ObjectId>> {
        self.original
            .iter()
            .map(|entry| gix::ObjectId::from_hex(entry.id.as_bytes()).map_err(Into::into))
            .collect()
    }

    pub fn preserved_paths(&self) -> &[Vec<u8>] {
        &self.preserved
    }

    /// Preserved public staging and working bytes must still match the original
    /// capture at every transition, including after a hook or external staging.
    pub fn validate_observed(&self, repo: &gix::Repository, public: &gix::index::State) -> Result<()> {
        validate_public(public)?;
        let observed = entries(public);
        ensure!(
            self.preserved
                .iter()
                .all(|p| semantic(&self.original, p) == semantic(&observed, p)),
            "preserved staging changed"
        );
        ensure!(
            !observed
                .iter()
                .any(|e| self.preserved.iter().any(|p| e.path != *p && overlaps(&e.path, p))),
            "public index collides with a preserved ancestor or descendant"
        );
        let root = repo.workdir().context("worktree required")?;
        for (path, original) in &self.before {
            ensure!(image(root, path)? == *original, "preserved working bytes changed");
        }
        Ok(())
    }

    /// Replaying or accepting a reordered footprint cannot consume preserved edits.
    pub fn validate_scope(&self, active: &[Vec<u8>]) -> Result<()> {
        ensure!(
            !self.preserved.iter().any(|p| active.iter().any(|a| overlaps(p, a))),
            "preserved edits overlap authoring scope"
        );
        Ok(())
    }

    /// Overlay only original preserved staging onto a clean replay index.
    pub fn public_index(&self, repo: &gix::Repository, private: &gix::index::State) -> Result<gix::index::State> {
        let private = entries(private);
        ensure!(
            !private
                .iter()
                .any(|e| self.preserved.iter().any(|p| e.path != *p && overlaps(&e.path, p))),
            "replay index collides with a preserved ancestor or descendant"
        );
        let clean: Vec<_> = private
            .iter()
            .filter(|e| self.preserved.contains(&e.path))
            .cloned()
            .collect();
        ensure!(
            changed(&self.clean_preserved, &clean).is_empty(),
            "replay changed a preserved path"
        );
        index(repo, &overlay(&private, &self.original, &self.preserved))
    }
}

/// Ownership of one child is tied to one immutable parent journal state.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub(crate) struct ParentBinding {
    pub(crate) nonce: String,
    pub(crate) step: usize,
    pub(crate) parent_state: String,
}
#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct State {
    version: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    replay_parent: Option<ParentBinding>,
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    author_message: Option<AuthorMessage>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    post_hooks: Option<PostHooks>,
}
#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct PostHooks {
    old: Option<String>,
    commit: String,
    rewrite: bool,
}
fn consume_post_hook(state: &mut State, name: &str) -> Result<()> {
    if let Some(pending) = &mut state.post_hooks {
        ensure!(
            name == if pending.rewrite { "post-rewrite" } else { "post-commit" },
            "post-hook queue changed"
        );
        if !pending.rewrite && pending.old.is_some() {
            pending.rewrite = true;
        } else {
            state.post_hooks = None;
        }
    }
    Ok(())
}
#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct AuthorMessage {
    amend: bool,
    invocation: usize,
    plan: super::super::replay::editors::Plan,
    work: String,
    accepted: bool,
    commit_msg_done: bool,
    checked_message: Option<String>,
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
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct OwnedPreparation {
    version: u32,
    binding: ParentBinding,
    state: State,
    original: Vec<u8>,
    private: Vec<u8>,
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
/// A checked handoff to the replay parent, not permission to remove the child.
/// The parent must durably accept the binding and receipt before retirement.
pub(crate) struct OwnedCompletion {
    pub(crate) decisions: usize,
    pub(crate) binding: ParentBinding,
    pub(crate) receipt: String,
    pub(crate) private: gix::index::State,
    pub(crate) head: gix::ObjectId,
    pub(crate) active: Vec<BString>,
    pub(crate) chain: Vec<gix::ObjectId>,
    pub(crate) objects: BTreeSet<gix::ObjectId>,
    pub(crate) work: Vec<super::super::switch::Change>,
}
pub(crate) struct OwnedRetention {
    pub(crate) queued_post_hooks: bool,
    pub(crate) decisions: usize,
    pub(crate) binding: ParentBinding,
    pub(crate) receipt: String,
    pub(crate) objects: BTreeSet<gix::ObjectId>,
    pub(crate) pending_hook: Option<(usize, bool)>,
    pub(crate) head: gix::ObjectId,
    pub(crate) work: Vec<super::super::switch::Change>,
}
fn raw(path: &Path) -> Result<Vec<u8>> {
    Ok(gix::path::into_bstr(path)?.as_bytes().to_vec())
}

fn overlaps(a: &[u8], b: &[u8]) -> bool {
    a == b || (a.starts_with(b) && a.get(b.len()) == Some(&b'/')) || (b.starts_with(a) && b.get(a.len()) == Some(&b'/'))
}

/// Fingerprint neighboring work without placing its bytes in the object database.
/// Only exact active leaves are excluded; descendants and ancestors remain guarded.
pub(crate) fn worktree_witness(repo: &gix::Repository, excluded: &[Vec<u8>]) -> Result<String> {
    let work = snapshot(repo.workdir().context("worktree required")?)?;
    let neighbors: Vec<_> = work.into_iter().filter(|(path, _)| !excluded.contains(path)).collect();
    digest(&serde_json::to_vec(&neighbors)?)
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
    let object_receipt = path.parent().and_then(Path::file_name) == Some(std::ffi::OsStr::new("objects"));
    if object_receipt {
        super::super::switch::checkpoint("replay-child-object-temp-created")?;
    }
    file.write_all(data)?;
    if object_receipt {
        super::super::switch::checkpoint("replay-child-object-temp-written")?;
    }
    super::super::switch::sync_file(&file, &temp)?;
    if object_receipt {
        super::super::switch::checkpoint("replay-child-object-temp-synced")?;
    }
    fs::rename(temp, path)?;
    if object_receipt {
        super::super::switch::checkpoint("replay-child-object-renamed")?;
    }
    super::super::switch::sync_dir(path.parent().context("parent")?)?;
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
        let preservation = Preservation::capture(repo)?;
        let head = repo.head_commit()?;
        ensure!(
            head.parent_ids().count() == 1,
            "internal authoring requires a non-root linear edit group"
        );
        let base = repo.index_from_tree(&head.tree_id()?)?;
        let original_entries = preservation.original.clone();
        let private = entries(&base);
        let preserved = preservation.preserved.clone();
        let active = super::super::index::authoring::literal_paths(repo, active)?
            .into_iter()
            .map(|p| p.to_vec())
            .collect::<Vec<_>>();
        preservation.validate_scope(&active)?;
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
        let mut before = preservation.before.clone();
        for p in &active {
            before.insert(p.clone(), image(root, p)?);
        }
        let state = State {
            version: 1,
            replay_parent: None,
            identities: identities(repo, &directory)?,
            policy: policy(repo)?,
            original_index: digest(preservation.original_index())?,
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
            author_message: None,
            post_hooks: None,
        };
        let session = Self { directory, state };
        persist(&session.directory.join("index.original"), preservation.original_index())?;
        session.save(repo)?;
        super::super::switch::sync_dir(repo.git_dir())?;
        Ok(session)
    }
    pub fn directory(&self) -> &Path {
        &self.directory
    }
    /// Enter a replay-owned stop using the operation's original preservation capture.
    /// The replay parent has already installed and validated this stop's checkout.
    /// Caller holds the shared mutation guard through parent and child publication.
    /// After the child advances, use `open_owned` / `recover_owned`; the immutable
    /// preparation recipe deliberately refuses changed child state.
    pub(crate) fn prepare_owned(
        repo: &gix::Repository,
        authorization: &super::super::replay::ChildAuthorization,
        preservation: &Preservation,
        kind: StopKind,
        active: &[BString],
        private: &gix::index::State,
    ) -> Result<Self> {
        authorization.validate_stop(repo, preservation, kind, active, private)?;
        let binding = authorization.binding();
        let parent_directory = repo.git_dir().join("brit-rebase");
        let recipe_path = parent_directory.join(format!("child-{}-prepare.json", binding.step));
        if fs::symlink_metadata(&recipe_path).is_ok() {
            return Self::resume_owned_preparation(repo, authorization, &recipe_path);
        }
        validate_metadata_directories(repo)?;
        ensure!(repo.state().is_none(), "foreign operation in progress");
        ensure!(
            repo.head()?.referent_name().is_none(),
            "replay child requires detached HEAD"
        );
        private.verify_entries()?;
        validate_public(private)?;
        let active = super::super::index::authoring::literal_paths(repo, active)?
            .into_iter()
            .map(|p| p.to_vec())
            .collect::<Vec<_>>();
        preservation.validate_scope(&active)?;
        let public = preservation.public_index(repo, private)?;
        ensure!(
            changed(&entries(&public), &entries(&*read_public(repo)?)).is_empty(),
            "public stop index differs from the authorized private index overlay"
        );
        let root = repo.workdir().context("worktree required")?;
        for (path, original) in &preservation.before {
            ensure!(
                image(root, path)? == *original,
                "preserved work changed before child preparation"
            );
        }
        let head = repo.head_commit()?;
        let parent = head.parent_ids().next().map(|id| id.to_string());
        ensure!(kind != StopKind::Edit || parent.is_some(), "edit group requires parent");
        let mut before = preservation.before.clone();
        for path in &active {
            before.insert(path.clone(), image(root, path)?);
        }
        let work = snapshot(root)?;
        ensure!(
            fs::symlink_metadata(repo.git_dir().join("brit-authoring")).is_err(),
            "child destination exists without its preparation recipe"
        );
        let directory = parent_directory.join(format!("child-{}-preparing", binding.step));
        use std::os::unix::fs::DirBuilderExt;
        fs::DirBuilder::new().mode(0o700).create(&directory)?;
        super::super::switch::checkpoint("replay-child-preparation-directory-created")?;
        let state = State {
            version: 1,
            replay_parent: Some(authorization.binding()),
            identities: identities(repo, &directory)?,
            policy: policy(repo)?,
            original_index: digest(preservation.original_index())?,
            original_head: fs::read(repo.git_dir().join("HEAD"))?,
            original_log: read_optional(&repo.git_dir().join("logs/HEAD"))?,
            owner: raw(&fs::canonicalize(repo.git_dir())?)?,
            root: raw(&fs::canonicalize(root)?)?,
            common: raw(&fs::canonicalize(repo.common_dir())?)?,
            kind,
            head: head.id.to_string(),
            // Conflict stops cannot amend/split; their tip may legitimately be a root.
            parent: parent.unwrap_or_else(|| head.id.to_string()),
            original: preservation.original.clone(),
            private: entries(private),
            public: entries(&public),
            preserved: preservation.preserved.clone(),
            active,
            before,
            work,
            expanded: BTreeMap::new(),
            split: false,
            children: 0,
            invocation: 0,
            pending_hook: None,
            author_message: None,
            post_hooks: None,
        };
        let recipe = OwnedPreparation {
            version: 1,
            binding,
            original: preservation.original_index().to_vec(),
            private: bytes(repo, &state.private)?,
            state,
        };
        // The complete recipe, including the staging inode, precedes every child
        // member. Recovery never captures work or preservation a second time.
        super::super::switch::private_write(&recipe_path, &sealed(&recipe)?)?;
        super::super::switch::sync_dir(&directory)?;
        super::super::switch::sync_dir(&parent_directory)?;
        super::super::switch::checkpoint("replay-child-recipe-durable")?;
        Self::resume_owned_preparation(repo, authorization, &recipe_path)
    }

    fn resume_owned_preparation(
        repo: &gix::Repository,
        authorization: &super::super::replay::ChildAuthorization,
        recipe_path: &Path,
    ) -> Result<Self> {
        use std::os::unix::fs::{MetadataExt, PermissionsExt};
        authorization.validate(repo)?;
        let uid = fs::metadata(repo.git_dir())?.uid();
        let metadata = fs::symlink_metadata(recipe_path)?;
        ensure!(
            metadata.is_file() && metadata.uid() == uid && metadata.permissions().mode().trailing_zeros() >= 6,
            "unsafe child preparation recipe"
        );
        let recipe: OwnedPreparation = load(recipe_path)?;
        ensure!(
            recipe.version == 1
                && recipe.binding == authorization.binding()
                && recipe.state.replay_parent.as_ref() == Some(&recipe.binding)
                && digest(&recipe.original)? == recipe.state.original_index
                && recipe.private == bytes(repo, &recipe.state.private)?,
            "foreign child preparation recipe"
        );
        let parent_directory = recipe_path.parent().context("parent journal")?;
        // Readable bytes can have survived a process crash before their original
        // synchronization. Reestablish durable ownership before child publication.
        super::super::switch::sync_path(recipe_path)?;
        super::super::switch::sync_dir(parent_directory)?;
        let staging = parent_directory.join(format!("child-{}-preparing", recipe.binding.step));
        let active = repo.git_dir().join("brit-authoring");
        let exists = |path: &Path| -> Result<bool> {
            match fs::symlink_metadata(path) {
                Ok(_) => Ok(true),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
                Err(e) => Err(e.into()),
            }
        };
        let staged = exists(&staging)?;
        ensure!(staged != exists(&active)?, "ambiguous child preparation directories");
        let directory = if staged { staging } else { active.clone() };
        let metadata = fs::symlink_metadata(&directory)?;
        ensure!(
            metadata.is_dir()
                && metadata.uid() == uid
                && metadata.permissions().mode().trailing_zeros() >= 6
                && identities(repo, &directory)? == recipe.state.identities,
            "foreign child preparation directory"
        );
        let members = [
            ("index.original", recipe.original),
            ("index.private", recipe.private),
            ("state.json", sealed(&recipe.state)?),
        ];
        for entry in fs::read_dir(&directory)? {
            ensure!(
                members
                    .iter()
                    .any(|(name, _)| entry.as_ref().is_ok_and(|e| e.file_name() == *name)),
                "unknown child preparation member"
            );
        }
        for (name, bytes) in members {
            let path = directory.join(name);
            if exists(&path)? {
                let metadata = fs::symlink_metadata(&path)?;
                ensure!(
                    metadata.is_file()
                        && metadata.uid() == uid
                        && metadata.permissions().mode().trailing_zeros() >= 6
                        && fs::read(&path)? == bytes,
                    "changed child preparation member"
                );
                super::super::switch::sync_path(&path)?;
            } else {
                super::super::switch::private_write(&path, &bytes)?;
            }
        }
        let mut session = Self {
            directory,
            state: recipe.state,
        };
        session.validate(repo)?;
        ensure!(
            changed(&session.state.public, &entries(&*read_public(repo)?)).is_empty()
                && snapshot(repo.workdir().context("worktree required")?)? == session.state.work,
            "checkout changed during child preparation"
        );
        super::super::switch::sync_dir(&session.directory)?;
        if staged {
            fs::rename(&session.directory, &active)?;
            session.directory = active;
        }
        super::super::switch::sync_dir(parent_directory)?;
        super::super::switch::sync_dir(repo.git_dir())?;
        super::super::switch::checkpoint("replay-child-published")?;
        Ok(session)
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
        let name = if after.post_hooks.as_ref().is_some_and(|pending| pending.rewrite) {
            "post-rewrite"
        } else {
            "post-commit"
        };
        consume_post_hook(&mut after, name)?;
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
        let s = Self::open_inner(repo)?;
        ensure!(
            s.state.replay_parent.is_none(),
            "replay-owned authoring requires its parent session"
        );
        Ok(s)
    }
    fn open_inner(repo: &gix::Repository) -> Result<Self> {
        let directory = repo.git_dir().join("brit-authoring");
        let state: State = load(&directory.join("state.json"))?;
        let s = Self { directory, state };
        s.ownership(repo)?;
        Ok(s)
    }
    pub(crate) fn open_owned(
        repo: &gix::Repository,
        authorization: &super::super::replay::ChildAuthorization,
    ) -> Result<Self> {
        authorization.validate(repo)?;
        let session = Self::open_inner(repo)?;
        ensure!(
            session.state.replay_parent.as_ref() == Some(&authorization.binding()),
            "foreign replay child"
        );
        Ok(session)
    }
    fn ownership(&self, repo: &gix::Repository) -> Result<()> {
        self.ownership_inner(repo, None)
    }
    fn ownership_inner(&self, repo: &gix::Repository, accepted: Option<&str>) -> Result<()> {
        validate_metadata_directories(repo)?;
        if let Some(binding) = &self.state.replay_parent {
            if let Some(receipt) = accepted {
                super::super::replay::validate_accepted_child(repo, binding, receipt)?;
            } else {
                super::super::replay::validate_child_binding(repo, binding)?;
            }
        }
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
                    || numbered("child-", "-proposed.json")
                    || numbered("child-", "-proposed.new")
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
    /// Parent-held mutation guard and a durable accepted-child receipt authorize
    /// this rename. Keep all messages, decisions and invocation evidence for the
    /// parent's retained-copy capture and eventual owned tombstone cleanup.
    pub(crate) fn archive_owned(repo: &gix::Repository, binding: &ParentBinding, receipt: &str) -> Result<PathBuf> {
        super::super::replay::validate_accepted_child(repo, binding, receipt)?;
        let parent = repo.git_dir().join("brit-rebase");
        super::super::switch::sync_path(&parent.join("state.json"))?;
        super::super::switch::sync_dir(&parent)?;
        let active = repo.git_dir().join("brit-authoring");
        let archive = parent.join(format!("child-{}-accepted", binding.step));
        let present = |path: &Path| -> Result<bool> {
            match fs::symlink_metadata(path) {
                Ok(_) => Ok(true),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
                Err(e) => Err(e.into()),
            }
        };
        let is_active = present(&active)?;
        ensure!(is_active != present(&archive)?, "ambiguous accepted child directories");
        let directory = if is_active { active } else { archive.clone() };
        let state_bytes = fs::read(directory.join("state.json"))?;
        ensure!(digest(&state_bytes)? == receipt, "accepted child receipt changed");
        let state: State = load(&directory.join("state.json"))?;
        ensure!(state.replay_parent.as_ref() == Some(binding), "unaccepted child");
        let session = Self { directory, state };
        session.ownership_inner(repo, Some(receipt))?;
        for number in 1..=session.state.invocation {
            // Unknown is permitted only once the supervisor and process group
            // are proven quiescent; inspection never repeats the invocation.
            super::invocation::inspect(&session.directory.join(format!("invocation-{number}")))?;
        }
        sync_objects(&session.directory)?;
        if is_active {
            fs::rename(&session.directory, &archive)?;
        }
        super::super::switch::sync_dir(&parent)?;
        super::super::switch::sync_dir(repo.git_dir())?;
        super::super::switch::checkpoint("replay-child-archived")?;
        Ok(archive)
    }
    /// Derive cleanup ownership from the child schema and bounded counters.
    /// Returned paths are relative to the parent journal.
    pub(crate) fn accepted_inventory(
        repo: &gix::Repository,
        binding: &ParentBinding,
        receipt: &str,
    ) -> Result<BTreeSet<Vec<u8>>> {
        use std::os::unix::fs::{MetadataExt, PermissionsExt};
        super::super::replay::validate_accepted_child(repo, binding, receipt)?;
        let parent = repo.git_dir().join("brit-rebase");
        let directory = parent.join(format!("child-{}-accepted", binding.step));
        let state: State = load(&directory.join("state.json"))?;
        ensure!(
            digest(&fs::read(directory.join("state.json"))?)? == receipt
                && state.replay_parent.as_ref() == Some(binding),
            "accepted child state changed"
        );
        let session = Self { directory, state };
        session.ownership_inner(repo, Some(receipt))?;
        let recipe_path = parent.join(format!("child-{}-prepare.json", binding.step));
        let metadata = fs::symlink_metadata(&recipe_path)?;
        ensure!(
            metadata.is_file()
                && metadata.uid() == fs::metadata(&parent)?.uid()
                && metadata.permissions().mode().trailing_zeros() >= 6,
            "unsafe accepted child recipe"
        );
        let recipe: OwnedPreparation = load(&recipe_path)?;
        ensure!(
            recipe.version == 1
                && &recipe.binding == binding
                && recipe.state.replay_parent.as_ref() == Some(binding)
                && recipe.state.identities == session.state.identities
                && digest(&recipe.original)? == session.state.original_index
                && recipe.private == bytes(repo, &recipe.state.private)?,
            "accepted child recipe changed"
        );
        let mut owned = BTreeSet::from([
            raw(session.directory.strip_prefix(&parent)?)?,
            raw(recipe_path.strip_prefix(&parent)?)?,
        ]);
        let mut previous = &recipe.state;
        let mut decisions = Vec::new();
        for number in 1..=session.state.children {
            decisions.push(load::<Decision>(
                &session.directory.join(format!("child-{number}.json")),
            )?);
        }
        for decision in &decisions {
            ensure!(
                decision.before.head == previous.head
                    && decision.before.private == previous.private
                    && decision.before.public == previous.public
                    && decision.before.children == previous.children,
                "accepted child decision chain is incomplete"
            );
            previous = &decision.after;
        }
        ensure!(
            session.state.head == previous.head
                && session.state.private == previous.private
                && session.state.public == previous.public
                && session.state.children == previous.children,
            "accepted child state differs from decision chain"
        );
        for number in 1..=session.state.invocation {
            super::invocation::inspect(&session.directory.join(format!("invocation-{number}")))?;
        }
        for entry in fs::read_dir(&session.directory)? {
            let entry = entry?;
            let name = entry.file_name();
            let name = name.to_str().context("invalid child member name")?;
            let path = entry.path();
            if let Some(number) = name.strip_prefix("invocation-") {
                let number: usize = number.parse()?;
                ensure!(
                    (1..=session.state.invocation).contains(&number) && name == format!("invocation-{number}"),
                    "unowned child invocation"
                );
                super::invocation::inspect(&path)?;
                for member in fs::read_dir(&path)? {
                    owned.insert(raw(member?.path().strip_prefix(&parent)?)?);
                }
            } else if let Some(number) = name
                .strip_prefix("child-")
                .and_then(|n| n.strip_suffix("-proposed.json"))
            {
                let number: usize = number.parse()?;
                ensure!(
                    (1..=session.state.children + 1).contains(&number)
                        && name == format!("child-{number}-proposed.json"),
                    "unowned child proposal"
                );
                let proposal: Decision = load(&path)?;
                ensure!(
                    proposal.before.replay_parent.as_ref() == Some(binding)
                        && proposal.after.replay_parent.as_ref() == Some(binding)
                        && proposal.before.children + 1 == number
                        && proposal.after.children == number,
                    "foreign child proposal"
                );
                if number <= session.state.children {
                    ensure!(
                        fs::read(&path)? == fs::read(session.directory.join(format!("child-{number}.json")))?,
                        "proposal differs from decided child"
                    );
                } else {
                    ensure!(
                        proposal.before == session.state,
                        "pending proposal differs from accepted child state"
                    );
                }
            } else if let Some(number) = name.strip_prefix("child-").and_then(|n| n.strip_suffix(".json")) {
                let number: usize = number.parse()?;
                ensure!(
                    (1..=session.state.children).contains(&number) && name == format!("child-{number}.json"),
                    "unowned child decision"
                );
                let decision: Decision = load(&path)?;
                ensure!(
                    decision.before.replay_parent.as_ref() == Some(binding)
                        && decision.after.replay_parent.as_ref() == Some(binding)
                        && decision.before.children + 1 == number
                        && decision.after.children == number,
                    "child decision ownership mismatch"
                );
            } else if name == "objects" {
                for member in fs::read_dir(&path)? {
                    let member = member?;
                    let metadata = fs::symlink_metadata(member.path())?;
                    ensure!(
                        metadata.is_file() && metadata.uid() == fs::metadata(&parent)?.uid(),
                        "unsafe child object receipt"
                    );
                    let object: RetainedObject = load(&member.path())?;
                    let id = gix::ObjectId::from_hex(object.id.as_bytes())?;
                    ensure!(
                        member.file_name().as_encoded_bytes() == object.id.as_bytes()
                            && gix::objs::compute_hash(
                                id.kind(),
                                gix::objs::Kind::from_bytes(object.kind.as_bytes())?,
                                &object.data
                            )? == id,
                        "corrupt child object receipt"
                    );
                    owned.insert(raw(member.path().strip_prefix(&parent)?)?);
                }
            } else {
                ensure!(
                    matches!(name, "index.original" | "index.private" | "state.json" | "message"),
                    "unresolved or unknown accepted child member"
                );
            }
            owned.insert(raw(path.strip_prefix(&parent)?)?);
        }
        Ok(owned)
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
        ensure!(
            !self
                .directory
                .join(format!("child-{}-proposed.json", self.state.children + 1))
                .exists(),
            "durable child proposal pending; recover the fixed proposal before authoring"
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
        ensure!(
            self.state.post_hooks.is_none(),
            "finish pending post-authoring hooks before staging"
        );
        ensure!(
            self.state.author_message.is_none(),
            "accept the pending author message before staging"
        );
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
        ensure!(
            self.state.post_hooks.is_none(),
            "finish pending post-authoring hooks before authoring"
        );
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
        ensure!(
            self.state.post_hooks.is_none(),
            "finish pending post-authoring hooks before continuing"
        );
        ensure!(
            self.state.author_message.is_none(),
            "accept the pending author message before continuing"
        );
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
    pub(crate) fn owned_completion(
        &self,
        repo: &gix::Repository,
        authorization: &super::super::replay::ChildAuthorization,
    ) -> Result<OwnedCompletion> {
        authorization.validate(repo)?;
        ensure!(
            self.state.replay_parent.as_ref() == Some(&authorization.binding()),
            "foreign replay child completion"
        );
        self.validate_completion(repo)?;
        self.restore_owned_objects(repo)?;
        ensure!(
            !self
                .directory
                .join(format!("child-{}.json", self.state.children + 1))
                .exists(),
            "recover the durable child decision before accepting completion"
        );
        let private = index(repo, &self.state.private)?;
        ensure!(
            fs::read(self.directory.join("index.private"))? == bytes(repo, &self.state.private)?,
            "private child index differs from its accepted state"
        );
        Ok(OwnedCompletion {
            decisions: self.state.children,
            binding: authorization.binding(),
            receipt: digest(&fs::read(self.directory.join("state.json"))?)?,
            private,
            head: gix::ObjectId::from_hex(self.state.head.as_bytes())?,
            active: self.state.active.iter().cloned().map(Into::into).collect(),
            chain: self.receipted_chain(repo)?,
            objects: self.receipted_heads(repo)?,
            work: self.owned_work(repo)?,
        })
    }
    fn owned_work(&self, repo: &gix::Repository) -> Result<Vec<super::super::switch::Change>> {
        use super::super::switch::{Change, State};
        let convert = |value: Option<Image>| match value {
            None => State::Absent,
            Some(Image::File(bytes, mode)) => State::File { bytes, mode },
            Some(Image::Link(bytes)) => State::Symlink(bytes),
        };
        let root = repo.workdir().context("worktree required")?;
        self.state
            .active
            .iter()
            .map(|path| {
                Ok(Change {
                    path: path.clone(),
                    before: convert(
                        self.state
                            .before
                            .get(path)
                            .context("missing child before image")?
                            .clone(),
                    ),
                    after: convert(image(root, path)?),
                })
            })
            .collect()
    }
    /// Prove the final chain from durable child decisions, not merely ancestry.
    fn receipted_heads(&self, repo: &gix::Repository) -> Result<BTreeSet<gix::ObjectId>> {
        let binding = self.state.replay_parent.as_ref().context("owned child required")?;
        let recipe: OwnedPreparation = load(
            &repo
                .git_dir()
                .join("brit-rebase")
                .join(format!("child-{}-prepare.json", binding.step)),
        )?;
        ensure!(
            recipe.version == 1
                && recipe.binding == *binding
                && recipe.state.replay_parent.as_ref() == Some(binding)
                && recipe.state.identities == self.state.identities
                && digest(&recipe.original)? == self.state.original_index
                && recipe.private == bytes(repo, &recipe.state.private)?,
            "child preparation receipt changed"
        );
        let mut previous = recipe.state;
        let mut authored = BTreeSet::from([previous.head.clone()]);
        for number in 1..=self.state.children {
            let decision: Decision = load(&self.directory.join(format!("child-{number}.json")))?;
            ensure!(
                decision.before.replay_parent.as_ref() == Some(binding)
                    && decision.after.replay_parent.as_ref() == Some(binding)
                    && decision.before.children + 1 == number
                    && decision.after.children == number
                    && decision.before.head == previous.head
                    && decision.before.private == previous.private
                    && decision.before.public == previous.public
                    && decision.before.children == previous.children,
                "child decision chain is incomplete"
            );
            authored.insert(decision.after.head.clone());
            previous = decision.after;
        }
        ensure!(
            self.state.head == previous.head
                && self.state.private == previous.private
                && self.state.public == previous.public
                && self.state.children == previous.children,
            "child state differs from decision chain"
        );
        authored
            .into_iter()
            .map(|head| gix::ObjectId::from_hex(head.as_bytes()).map_err(Into::into))
            .collect()
    }
    fn receipted_chain(&self, repo: &gix::Repository) -> Result<Vec<gix::ObjectId>> {
        let mut authored = self.receipted_heads(repo)?;
        if self.state.kind != StopKind::Edit {
            return Ok(Vec::new());
        }
        let mut current = self.state.head.clone();
        let mut chain = Vec::new();
        while current != self.state.parent {
            let id = gix::ObjectId::from_hex(current.as_bytes())?;
            ensure!(
                authored.remove(&id),
                "authored commit lacks a unique child decision receipt"
            );
            let commit = repo.find_commit(id)?;
            ensure!(commit.parent_ids().count() == 1, "authored chain must remain linear");
            chain.push(id);
            current = commit.parent_ids().next().context("linear parent")?.to_string();
        }
        chain.reverse();
        Ok(chain)
    }
    /// Capture an unresolved child for skip/abort without requiring its active
    /// work to be clean. Caller holds the parent guard through retained capture.
    pub(crate) fn owned_retention(
        &self,
        repo: &gix::Repository,
        authorization: &super::super::replay::ChildAuthorization,
        allow_unrelated_work: bool,
    ) -> Result<OwnedRetention> {
        authorization.validate(repo)?;
        self.ownership(repo)?;
        ensure!(
            self.state.replay_parent.as_ref() == Some(&authorization.binding())
                && load::<State>(&self.directory.join("state.json"))? == self.state,
            "stale or foreign replay child retention"
        );
        ensure!(
            !self
                .directory
                .join(format!("child-{}.json", self.state.children + 1))
                .exists(),
            "recover child decision before retaining or restoring"
        );
        for number in 1..=self.state.invocation {
            super::invocation::inspect(&self.directory.join(format!("invocation-{number}")))?;
        }
        ensure!(
            repo.head()?.referent_name().is_none()
                && repo.head_commit()?.id == gix::ObjectId::from_hex(self.state.head.as_bytes())?,
            "foreign child HEAD movement"
        );
        if allow_unrelated_work {
            // Abort restores only parent-owned replay paths. Unrelated new work
            // remains in place, while frozen preservation and path collisions
            // must still be checked before the parent authorizes restoration.
            let current = snapshot(repo.workdir().context("worktree required")?)?;
            for path in self.state.work.keys().chain(current.keys()) {
                if self.state.preserved.contains(path)
                    || (!self.state.active.contains(path)
                        && self.state.active.iter().any(|active| overlaps(path, active)))
                {
                    ensure!(
                        self.state.work.get(path) == current.get(path),
                        "retention work collides with preserved or active scope"
                    );
                }
            }
        } else {
            self.validate_scope_work(repo, &self.state.active)?;
        }
        let public = read_public(repo)?;
        let private = gix::index::File::at(
            self.directory.join("index.private"),
            repo.object_hash(),
            false,
            Default::default(),
        )?;
        validate_public(&private)?;
        for (expected, observed) in [
            (&self.state.public, entries(&public)),
            (&self.state.private, entries(&private)),
        ] {
            ensure!(
                changed(expected, &observed)
                    .iter()
                    .all(|path| self.state.active.contains(path)),
                "child staging changed outside active retention scope"
            );
        }
        let mut objects: BTreeSet<_> = private
            .entries()
            .iter()
            .chain(public.entries())
            .map(|entry| entry.id)
            .collect();
        objects.extend(self.receipted_heads(repo)?);
        objects.extend(self.restore_owned_objects(repo)?);
        let proposal_path = self
            .directory
            .join(format!("child-{}-proposed.json", self.state.children + 1));
        if proposal_path.exists() {
            let proposal: Decision = load(&proposal_path)?;
            ensure!(
                proposal.before == self.state && proposal.after.children == self.state.children + 1,
                "foreign pending proposal"
            );
            objects.insert(gix::ObjectId::from_hex(proposal.after.head.as_bytes())?);
        }
        Ok(OwnedRetention {
            queued_post_hooks: self.state.post_hooks.is_some(),
            decisions: self.state.children,
            binding: authorization.binding(),
            receipt: digest(&fs::read(self.directory.join("state.json"))?)?,
            objects,
            pending_hook: self.state.pending_hook,
            head: gix::ObjectId::from_hex(self.state.head.as_bytes())?,
            work: self.owned_work(repo)?,
        })
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
        if after.head != self.state.head
            && let Some(binding) = &self.state.replay_parent
        {
            self.retain_objects_with(repo, std::slice::from_ref(&after.head))?;
            persist(
                &self.directory.join(format!("child-{}-proposed.json", after.children)),
                &sealed(&decision)?,
            )?;
            super::super::switch::checkpoint("replay-child-proposal-durable")?;
            super::super::replay::protect_child_commit(
                repo,
                binding,
                after.children,
                gix::ObjectId::from_hex(after.head.as_bytes())?,
            )?;
        }
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
                super::super::switch::sync_path(resource)?;
                super::super::switch::sync_dir(parent)?;
                #[cfg(feature = "switch-test-faults")]
                if resource == &head_path
                    && std::env::var_os("BRIT_AUTHORING_FAIL_AFTER").as_deref()
                        == Some(std::ffi::OsStr::new("head-publication"))
                {
                    anyhow::bail!("injected failure after HEAD publication");
                }
            }
        }
        super::super::switch::sync_dir(repo.git_dir())?;
        Ok(())
    }
    pub fn recover(repo: &gix::Repository) -> Result<Self> {
        let _guard = super::super::switch::shared_guard(repo)?;
        Self::open(repo)?.recover_pending(repo)
    }
    pub(crate) fn recover_owned(
        repo: &gix::Repository,
        authorization: &super::super::replay::ChildAuthorization,
    ) -> Result<Self> {
        // The replay parent holds the shared guard across its child handoff.
        Self::open_owned(repo, authorization)?.recover_pending(repo)
    }
    fn recover_pending(mut self, repo: &gix::Repository) -> Result<Self> {
        let s = &mut self;
        let next = s.directory.join(format!("child-{}.json", s.state.children + 1));
        let proposal = s
            .directory
            .join(format!("child-{}-proposed.json", s.state.children + 1));
        if !next.exists() && proposal.exists() {
            let d: Decision = load(&proposal)?;
            ensure!(
                d.before == s.state
                    && d.after.children == s.state.children + 1
                    && d.after.replay_parent == s.state.replay_parent,
                "foreign proposed child decision"
            );
            s.validate_scope_work(repo, &d.after.active)?;
            s.validate_decision_resources(repo, &d)?;
            ensure!(
                fs::read(repo.index_path())? == d.old_index
                    && fs::read(repo.git_dir().join("HEAD"))? == d.old_head
                    && read_optional(&repo.git_dir().join("logs/HEAD"))? == d.old_log,
                "predecision resources changed"
            );
            s.restore_owned_objects(repo)?;
            super::super::replay::protect_child_commit(
                repo,
                s.state.replay_parent.as_ref().context("owned proposal required")?,
                d.after.children,
                gix::ObjectId::from_hex(d.after.head.as_bytes())?,
            )?;
            persist(&next, &sealed(&d)?)?;
        }
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
        Ok(self)
    }
    /// Retain all authored bytes before restoring the stop's original HEAD/index/files.
    /// Unrelated new work is left in place. Unknown pre-hooks can only take this path.
    pub fn abort(&mut self, repo: &gix::Repository) -> Result<PathBuf> {
        ensure!(
            self.state.replay_parent.is_none(),
            "replay-owned child restoration belongs to its parent"
        );
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
        super::super::switch::sync_dir(repo.git_dir())?;
        self.directory.clone_from(&retained);
        self.state = plan.decision.after;
        Ok(retained)
    }
    fn retain_objects(&self, repo: &gix::Repository) -> Result<()> {
        self.retain_objects_with(repo, &[])
    }
    fn restore_owned_objects(&self, repo: &gix::Repository) -> Result<BTreeSet<gix::ObjectId>> {
        let directory = self.directory.join("objects");
        let mut roots = BTreeSet::new();
        if !directory.exists() {
            return Ok(roots);
        }
        for entry in fs::read_dir(&directory)? {
            let entry = entry?;
            ensure!(entry.file_type()?.is_file(), "unsafe authored object receipt");
            if let Some(stem) = entry.file_name().to_str().and_then(|name| name.strip_suffix(".new")) {
                gix::ObjectId::from_hex(stem.as_bytes()).context("unowned partial object receipt")?;
                // Never adopt or erase an interrupted private write. The parent
                // retained copy carries it; exact cleanup inventory still refuses.
                continue;
            }
            let object: RetainedObject = load(&entry.path())?;
            let id = gix::ObjectId::from_hex(object.id.as_bytes())?;
            let kind = gix::objs::Kind::from_bytes(object.kind.as_bytes())?;
            ensure!(
                entry.file_name().as_encoded_bytes() == object.id.as_bytes()
                    && gix::objs::compute_hash(id.kind(), kind, &object.data)? == id,
                "corrupt authored object receipt"
            );
            ensure!(
                repo.write_object(&gix::objs::ObjectRef::from_bytes(&object.data, kind, id.kind())?)?
                    .detach()
                    == id,
                "restored authored object changed identity"
            );
            roots.insert(id);
        }
        sync_objects(&repo.common_dir().join("objects"))?;
        Ok(roots)
    }
    fn retain_objects_with(&self, repo: &gix::Repository, extra: &[String]) -> Result<()> {
        let directory = self.directory.join("objects");
        if !directory.exists() {
            use std::os::unix::fs::DirBuilderExt;
            fs::DirBuilder::new().mode(0o700).create(&directory)?;
        }
        let mut stack = self.state.private.iter().map(|e| e.id.clone()).collect::<Vec<_>>();
        stack.extend_from_slice(extra);
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
        super::super::switch::sync_dir(&directory)?;
        super::super::switch::sync_dir(&self.directory)?;
        Ok(())
    }
    pub fn begin_split(&mut self, repo: &gix::Repository) -> Result<()> {
        ensure!(
            self.state.author_message.is_none(),
            "accept the pending author message before splitting"
        );
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
        self.amend_bytes(repo, message.as_bytes())
    }
    pub fn amend_bytes(&mut self, repo: &gix::Repository, message: &[u8]) -> Result<Outcome> {
        ensure!(
            self.state.kind == StopKind::Edit && !self.state.split,
            "amend requires unsplit edit stop"
        );
        self.author(repo, message, true)
    }
    pub fn commit_split(&mut self, repo: &gix::Repository, message: &str) -> Result<Outcome> {
        self.commit_split_bytes(repo, message.as_bytes())
    }
    pub fn commit_split_bytes(&mut self, repo: &gix::Repository, message: &[u8]) -> Result<Outcome> {
        ensure!(
            self.state.kind == StopKind::Edit && self.state.split,
            "split has not started"
        );
        self.author(repo, message, false)
    }
    fn author(&mut self, repo: &gix::Repository, message: &[u8], amend: bool) -> Result<Outcome> {
        ensure!(
            self.state.author_message.is_none(),
            "accept the pending author message without rerunning its editor"
        );
        self.import_external(repo)?;
        ensure!(!message.iter().all(u8::is_ascii_whitespace), "empty message");
        ensure!(
            !repo.config_snapshot().try_boolean("commit.gpgSign")?.unwrap_or(false),
            "required signing unsupported"
        );
        let message_path = self.directory.join("message");
        persist(&message_path, message)?;
        self.hook(repo, "pre-commit", &[], None, false)?;
        let mut args = vec![message_path.clone().into_os_string()];
        if amend {
            args.extend(["commit".into(), self.state.head.clone().into()]);
        } else {
            args.push("message".into());
        }
        self.hook(repo, "prepare-commit-msg", &args, None, false)?;
        self.author_prepared(repo, amend)
    }
    /// Start an explicit amend with the normal message editor contract.
    pub fn amend_edit(&mut self, repo: &gix::Repository) -> Result<Outcome> {
        self.author_edit(repo, true)
    }
    /// Start an explicit split commit with the normal message editor contract.
    pub fn commit_split_edit(&mut self, repo: &gix::Repository) -> Result<Outcome> {
        self.author_edit(repo, false)
    }
    pub fn has_pending_author_message(&self) -> bool {
        self.state.author_message.is_some()
    }
    pub fn has_pending_post_hook(&self) -> bool {
        self.state.pending_hook.is_some_and(|(_, post)| post)
    }
    pub fn has_pending_post_hooks(&self) -> bool {
        self.state.post_hooks.is_some()
    }
    /// Run notifications durably queued with the commit decision. Unknown
    /// invocations require acknowledgement before advancing this queue.
    pub fn resume_post_hooks(&mut self, repo: &gix::Repository) -> Result<Vec<String>> {
        let mut warnings = Vec::new();
        while let Some(pending) = self.state.post_hooks.clone() {
            ensure!(
                self.state.pending_hook.is_none(),
                "unknown post-hook requires explicit acknowledgement"
            );
            ensure!(self.state.head == pending.commit, "post-hook commit changed");
            let (name, args, input) = if pending.rewrite {
                (
                    "post-rewrite",
                    vec!["amend".into()],
                    Some(
                        format!(
                            "{} {}\n",
                            pending.old.context("missing amendment mapping")?,
                            pending.commit
                        )
                        .into_bytes(),
                    ),
                )
            } else {
                ("post-commit", Vec::new(), None)
            };
            if let Err(error) = self.hook(repo, name, &args, input.as_deref(), true) {
                warnings.push(format!("commit {} published; {error:#}", pending.commit));
                if self.state.pending_hook.is_some() {
                    break;
                }
                // A pre-launch error must not loop or silently consume an item.
                if self
                    .state
                    .post_hooks
                    .as_ref()
                    .is_some_and(|item| item.rewrite == pending.rewrite)
                {
                    break;
                }
            }
        }
        Ok(warnings)
    }
    fn author_edit(&mut self, repo: &gix::Repository, amend: bool) -> Result<Outcome> {
        ensure!(
            self.state.author_message.is_none(),
            "pending editor output requires explicit acceptance"
        );
        ensure!(
            self.state.kind == StopKind::Edit && self.state.split != amend,
            "invalid edit authoring action"
        );
        self.import_external(repo)?;
        ensure!(
            !repo.config_snapshot().try_boolean("commit.gpgSign")?.unwrap_or(false),
            "required signing unsupported"
        );
        let path = self.directory.join("message");
        let initial = if amend {
            repo.head_commit()?.decode()?.message.to_vec()
        } else {
            Vec::new()
        };
        persist(&path, &initial)?;
        // Refuse missing/untrusted editor configuration before running hooks.
        super::super::replay::editors::resolve(repo, super::super::replay::editors::Kind::Message, &path)?;
        self.hook(repo, "pre-commit", &[], None, false)?;
        let mut args = vec![path.clone().into_os_string()];
        if amend {
            args.extend(["commit".into(), self.state.head.clone().into()]);
        }
        self.hook(repo, "prepare-commit-msg", &args, None, false)?;
        let plan = super::super::replay::editors::resolve(repo, super::super::replay::editors::Kind::Message, &path)?;
        let number = self.state.invocation + 1;
        let directory = self.directory.join(format!("invocation-{number}"));
        let index = self.directory.join("index.private");
        let outcome = super::super::replay::editors::run(repo, &directory, &plan, &index, || {
            let _guard = super::super::switch::shared_guard(repo)?;
            self.validate(repo)?;
            ensure!(self.state.author_message.is_none(), "editor launch authority changed");
            self.state.invocation = number;
            self.state.author_message = Some(AuthorMessage {
                amend,
                invocation: number,
                plan: plan.clone(),
                work: worktree_witness(repo, &[])?,
                accepted: false,
                commit_msg_done: false,
                checked_message: None,
            });
            self.save(repo)
        })?;
        use std::os::unix::fs::MetadataExt;
        let metadata = fs::symlink_metadata(&path)?;
        ensure!(
            metadata.is_file() && metadata.uid() == fs::metadata(&self.directory)?.uid(),
            "unsafe message editor output"
        );
        super::super::switch::sync_path(&path)?;
        super::super::switch::sync_dir(&self.directory)?;
        ensure!(
            outcome.success(),
            "author message editor failed or has unknown outcome; saved output requires explicit acceptance"
        );
        self.accept_author_message(repo)
    }
    /// Accept saved bytes after a failed or interrupted editor; never relaunch it.
    pub fn accept_author_message(&mut self, repo: &gix::Repository) -> Result<Outcome> {
        let pending = self.state.author_message.clone().context("no pending author message")?;
        {
            let _guard = super::super::switch::shared_guard(repo)?;
            self.validate(repo)?;
            super::invocation::inspect(&self.directory.join(format!("invocation-{}", pending.invocation)))?;
            ensure!(
                worktree_witness(repo, &[])? == pending.work,
                "editor changed working files"
            );
            ensure!(
                changed(&self.state.public, &entries(&*read_public(repo)?)).is_empty()
                    && fs::read(self.directory.join("index.private"))? == bytes(repo, &self.state.private)?,
                "editor changed staging"
            );
            let path = self.directory.join("message");
            use std::os::unix::fs::MetadataExt;
            let metadata = fs::symlink_metadata(&path)?;
            ensure!(
                metadata.is_file() && metadata.uid() == fs::metadata(&self.directory)?.uid(),
                "unsafe message output"
            );
            let message = fs::read(&path)?;
            ensure!(
                !message.iter().all(u8::is_ascii_whitespace) && !message.contains(&0),
                "invalid empty or NUL message"
            );
            if let Some(checked) = &pending.checked_message {
                ensure!(
                    digest(&message)? == *checked,
                    "message changed after commit-msg validation"
                );
            }
            super::super::switch::sync_path(&path)?;
            super::super::switch::sync_dir(&self.directory)?;
            self.state.author_message.as_mut().context("pending message")?.accepted = true;
            self.save(repo)?;
        }
        self.author_prepared(repo, pending.amend)
    }
    fn author_prepared(&mut self, repo: &gix::Repository, amend: bool) -> Result<Outcome> {
        if !self
            .state
            .author_message
            .as_ref()
            .is_some_and(|message| message.commit_msg_done)
        {
            self.hook(
                repo,
                "commit-msg",
                &[self.directory.join("message").into_os_string()],
                None,
                false,
            )?;
        }
        let message = fs::read(self.directory.join("message"))?;
        ensure!(!message.iter().all(u8::is_ascii_whitespace), "empty hook message");
        if let Some(checked) = self
            .state
            .author_message
            .as_ref()
            .and_then(|pending| pending.checked_message.as_ref())
        {
            ensure!(
                digest(&message)? == *checked,
                "message changed after commit-msg validation"
            );
        }
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
                encoding: if amend {
                    previous.decode()?.encoding.map(ToOwned::to_owned)
                } else {
                    None
                },
                extra_headers: Default::default(),
            })?
            .detach();
        // Synchronize object files before persisting a decision; private snapshots never enter the ODB.
        sync_objects(&repo.common_dir().join("objects"))?;
        let old = self.state.head.clone();
        let mut after = self.state.clone();
        after.head = commit.to_string();
        after.author_message = None;
        after.post_hooks = Some(PostHooks {
            old: amend.then_some(old),
            commit: commit.to_string(),
            rewrite: false,
        });
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
        let warnings = self.resume_post_hooks(repo)?;
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
            let _guard = super::super::switch::shared_guard(repo)?;
            self.validate(repo)?;
            if post {
                consume_post_hook(&mut self.state, name)?;
                self.save(repo)?;
            }
            if name == "commit-msg"
                && let Some(message) = &mut self.state.author_message
            {
                ensure!(message.accepted, "author message is not accepted");
                message.commit_msg_done = true;
                message.checked_message = Some(digest(&fs::read(self.directory.join("message"))?)?);
                self.save(repo)?;
            }
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
        if post {
            consume_post_hook(&mut after, name)?;
        }
        if name == "commit-msg"
            && let Some(message) = &mut after.author_message
        {
            ensure!(message.accepted, "author message is not accepted");
            message.commit_msg_done = outcome.success();
            message.checked_message = outcome
                .success()
                .then(|| digest(&fs::read(self.directory.join("message"))?))
                .transpose()?;
            message.work = worktree_witness(repo, &[])?;
        }
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
            super::super::switch::sync_path(&entry.path())?;
        } else {
            ensure!(!metadata.is_symlink(), "object symlink unsupported");
        }
    }
    super::super::switch::sync_dir(path)?;
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
            super::super::switch::sync_file(temporary.as_file(), temporary.path())?;
            ensure!(image(root, p)?.as_ref() == before, "work changed before replace");
            temporary.persist(&file)?;
        }
        Some(Image::Link(bytes)) => {
            fs::create_dir_all(parent)?;
            let temporary = tempfile::NamedTempFile::new_in(parent)?.into_temp_path();
            fs::remove_file(&temporary)?;
            std::os::unix::fs::symlink(gix::path::from_bstr(bytes.as_bstr())?, &temporary)?;
            super::super::switch::sync_dir(parent)?;
            ensure!(image(root, p)?.as_ref() == before, "work changed before replace");
            fs::rename(&temporary, &file)?;
        }
    }
    super::super::switch::sync_dir(parent)?;
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
