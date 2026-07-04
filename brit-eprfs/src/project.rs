use std::{
    collections::VecDeque,
    path::{Path, PathBuf},
};

use eprfs_core::{
    BlobCid, EntryKind, EprRef, ProjectionEntry, ProjectionId, ProjectionManifest, ProjectionPath, ProjectionRoot,
    ProjectionStatus,
};
use gix::{
    bstr::{BStr, BString, ByteSlice, ByteVec},
    objs::tree::{EntryKind as GitEntryKind, EntryRef},
    traverse::tree::visit::Action,
};

pub type Result<T> = std::result::Result<T, BritEprfsError>;

#[derive(Debug, thiserror::Error)]
pub enum BritEprfsError {
    #[error("failed to open git repository at {path:?}: {source}")]
    OpenRepo {
        path: PathBuf,
        #[source]
        source: Box<dyn std::error::Error + Send + Sync>,
    },

    #[error("failed to resolve treeish {rev:?}: {source}")]
    ResolveTree {
        rev: String,
        #[source]
        source: Box<dyn std::error::Error + Send + Sync>,
    },

    #[error("failed to walk git tree: {0}")]
    TreeWalk(String),

    #[error("git path is not valid UTF-8: {0:?}")]
    NonUtf8Path(BString),

    #[error(transparent)]
    Eprfs(#[from] eprfs_core::EprfsError),
}

/// Project a git treeish from a repository path into an EPR filesystem manifest.
pub fn project_tree(repo_path: impl AsRef<Path>, rev: &str) -> Result<ProjectionManifest> {
    let repo_path = repo_path.as_ref();
    let repo = gix::open(repo_path).map_err(|source| BritEprfsError::OpenRepo {
        path: repo_path.to_path_buf(),
        source: Box::new(source),
    })?;
    project_tree_from_repo(&repo, rev)
}

/// Project a git treeish from an already-open repository.
pub fn project_tree_from_repo(repo: &gix::Repository, rev: &str) -> Result<ProjectionManifest> {
    let tree = treeish_to_tree(repo, rev)?;
    let tree_id = tree.id.to_hex().to_string();

    let mut collector = ProjectionCollector::default();
    tree.traverse()
        .breadthfirst(&mut collector)
        .map_err(|source| BritEprfsError::TreeWalk(source.to_string()))?;

    if let Some(error) = collector.errors.into_iter().next() {
        return Err(error);
    }

    collector.entries.sort_by(|left, right| {
        left.path
            .as_path()
            .cmp(right.path.as_path())
            .then_with(|| entry_rank(&left.kind).cmp(&entry_rank(&right.kind)))
    });

    Ok(ProjectionManifest {
        root: ProjectionRoot {
            id: ProjectionId::new(format!("brit-tree:{tree_id}")),
            root: EprRef::new(format!("brit:tree:{tree_id}")),
        },
        entries: collector.entries,
        metadata: serde_json::json!({
            "adapter": "brit-eprfs",
            "rev": rev,
            "treeObjectId": tree_id,
        }),
    })
}

fn treeish_to_tree<'repo>(repo: &'repo gix::Repository, rev: &str) -> Result<gix::Tree<'repo>> {
    let spec = format!("{rev}^{{tree}}");
    let id = repo
        .rev_parse_single(spec.as_str())
        .map_err(|source| BritEprfsError::ResolveTree {
            rev: rev.to_string(),
            source: Box::new(source),
        })?;
    let object = id.object().map_err(|source| BritEprfsError::ResolveTree {
        rev: rev.to_string(),
        source: Box::new(source),
    })?;
    Ok(object.into_tree())
}

fn entry_rank(kind: &EntryKind) -> u8 {
    match kind {
        EntryKind::Directory => 0,
        EntryKind::File => 1,
        EntryKind::Symlink => 2,
    }
}

#[derive(Default)]
struct ProjectionCollector {
    entries: Vec<ProjectionEntry>,
    errors: Vec<BritEprfsError>,
    path: BString,
    path_deque: VecDeque<BString>,
}

impl ProjectionCollector {
    fn push_element(&mut self, component: &BStr) {
        if !self.path.is_empty() {
            self.path.push(b'/');
        }
        self.path.push_str(component);
    }

    fn pop_element(&mut self) {
        if let Some(pos) = self.path.rfind_byte(b'/') {
            self.path.resize(pos, 0);
        } else {
            self.path.clear();
        }
    }

    fn current_projection_path(&mut self) -> Option<ProjectionPath> {
        match std::str::from_utf8(&self.path) {
            Ok(path) => match ProjectionPath::new(path) {
                Ok(path) => Some(path),
                Err(error) => {
                    self.errors.push(error.into());
                    None
                }
            },
            Err(_) => {
                self.errors.push(BritEprfsError::NonUtf8Path(self.path.clone()));
                None
            }
        }
    }

    fn push_entry(&mut self, entry: &EntryRef<'_>, kind: EntryKind, blob: Option<BlobCid>) {
        let Some(path) = self.current_projection_path() else {
            return;
        };

        self.entries.push(ProjectionEntry {
            path,
            kind,
            epr: None,
            blob,
            size_bytes: None,
            executable: matches!(entry.mode.kind(), GitEntryKind::BlobExecutable),
            status: ProjectionStatus::Remote,
            metadata: serde_json::json!({
                "gitObjectId": entry.oid.to_hex().to_string(),
                "gitMode": format!("{:o}", entry.mode.value()),
                "gitEntryKind": format!("{:?}", entry.mode.kind()),
            }),
        });
    }
}

impl gix::traverse::tree::Visit for ProjectionCollector {
    fn pop_back_tracked_path_and_set_current(&mut self) {
        self.path = self.path_deque.pop_back().unwrap_or_default();
    }

    fn pop_front_tracked_path_and_set_current(&mut self) {
        self.path = self
            .path_deque
            .pop_front()
            .expect("every queued tree path must be restored once");
    }

    fn push_back_tracked_path_component(&mut self, component: &BStr) {
        self.push_element(component);
        self.path_deque.push_back(self.path.clone());
    }

    fn push_path_component(&mut self, component: &BStr) {
        self.push_element(component);
    }

    fn pop_path_component(&mut self) {
        self.pop_element();
    }

    fn visit_tree(&mut self, entry: &EntryRef<'_>) -> Action {
        self.push_entry(entry, EntryKind::Directory, None);
        std::ops::ControlFlow::Continue(true)
    }

    fn visit_nontree(&mut self, entry: &EntryRef<'_>) -> Action {
        match entry.mode.kind() {
            GitEntryKind::Blob | GitEntryKind::BlobExecutable => {
                let blob = BlobCid::new(format!("git-blob:{}", entry.oid.to_hex()));
                self.push_entry(entry, EntryKind::File, Some(blob));
            }
            GitEntryKind::Link => {
                let blob = BlobCid::new(format!("git-blob:{}", entry.oid.to_hex()));
                self.push_entry(entry, EntryKind::Symlink, Some(blob));
            }
            GitEntryKind::Commit => {
                self.push_entry(entry, EntryKind::Directory, None);
            }
            GitEntryKind::Tree => unreachable!("tree entries are handled by visit_tree"),
        }

        std::ops::ControlFlow::Continue(true)
    }
}

#[cfg(test)]
mod tests {
    use std::{fs, process::Command};

    use eprfs_core::{EntryKind, MaterializationPolicy};
    use eprfs_local::LocalMaterializer;
    use tempfile::TempDir;

    use crate::GitObjectStorage;

    use super::*;

    #[test]
    fn projects_committed_tree_into_manifest() {
        let repo = TestRepo::new();
        repo.write("Cargo.toml", "[package]\nname = \"demo\"\n");
        repo.write("src/main.rs", "fn main() {}\n");
        repo.write("scripts/run.sh", "#!/bin/sh\ntrue\n");
        repo.git(["add", "."]);
        repo.git(["update-index", "--chmod=+x", "scripts/run.sh"]);
        repo.git(["commit", "-m", "initial"]);

        let manifest = project_tree(repo.path(), "HEAD").expect("projection");

        let paths: Vec<_> = manifest
            .entries
            .iter()
            .map(|entry| entry.path.as_path().to_string_lossy().to_string())
            .collect();

        assert_eq!(
            paths,
            vec!["Cargo.toml", "scripts", "scripts/run.sh", "src", "src/main.rs"]
        );

        let cargo = entry(&manifest, "Cargo.toml");
        assert_eq!(cargo.kind, EntryKind::File);
        assert!(cargo.blob.as_ref().unwrap().as_str().starts_with("git-blob:"));
        assert_eq!(cargo.metadata["adapter"], serde_json::Value::Null);
        assert_eq!(cargo.metadata["gitEntryKind"], "Blob");

        let script = entry(&manifest, "scripts/run.sh");
        assert_eq!(script.kind, EntryKind::File);
        assert!(script.executable);
        assert_eq!(script.metadata["gitEntryKind"], "BlobExecutable");

        let src = entry(&manifest, "src");
        assert_eq!(src.kind, EntryKind::Directory);
        assert!(src.blob.is_none());

        assert_eq!(manifest.metadata["adapter"], "brit-eprfs");
        assert_eq!(manifest.metadata["rev"], "HEAD");
    }

    #[tokio::test]
    async fn materializes_projected_tree_from_git_objects() {
        let repo = TestRepo::new();
        repo.write("Cargo.toml", "[package]\nname = \"demo\"\n");
        repo.write("src/main.rs", "fn main() {}\n");
        repo.write("scripts/run.sh", "#!/bin/sh\ntrue\n");
        repo.git(["add", "."]);
        repo.git(["update-index", "--chmod=+x", "scripts/run.sh"]);
        repo.git(["commit", "-m", "initial"]);

        let manifest = project_tree(repo.path(), "HEAD").expect("projection");
        let storage = GitObjectStorage::open(repo.path()).expect("git object storage");
        let materializer = LocalMaterializer::new(storage);
        let target = repo.path().join("projected");

        let report = materializer
            .materialize(&manifest, &target, MaterializationPolicy::LocalOnly)
            .await
            .expect("materialize");

        assert_eq!(report.directories, 2);
        assert_eq!(report.files_written, 3);
        assert_eq!(
            fs::read_to_string(target.join("Cargo.toml")).unwrap(),
            "[package]\nname = \"demo\"\n"
        );
        assert_eq!(
            fs::read_to_string(target.join("src/main.rs")).unwrap(),
            "fn main() {}\n"
        );
        assert_eq!(
            fs::read_to_string(target.join("scripts/run.sh")).unwrap(),
            "#!/bin/sh\ntrue\n"
        );

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = fs::metadata(target.join("scripts/run.sh"))
                .unwrap()
                .permissions()
                .mode();
            assert_ne!(mode & 0o111, 0, "executable bit should be materialized");
        }
    }

    fn entry<'a>(manifest: &'a ProjectionManifest, path: &str) -> &'a ProjectionEntry {
        manifest
            .entries
            .iter()
            .find(|entry| entry.path.as_path() == Path::new(path))
            .unwrap_or_else(|| panic!("missing projection entry {path}"))
    }

    struct TestRepo {
        dir: TempDir,
    }

    impl TestRepo {
        fn new() -> Self {
            let dir = TempDir::new().expect("tempdir");
            let repo = Self { dir };
            repo.git(["init"]);
            repo.git(["config", "user.email", "test@example.test"]);
            repo.git(["config", "user.name", "Test User"]);
            repo
        }

        fn path(&self) -> &Path {
            self.dir.path()
        }

        fn write(&self, path: &str, body: &str) {
            let path = self.path().join(path);
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).expect("mkdir");
            }
            fs::write(path, body).expect("write");
        }

        fn git<const N: usize>(&self, args: [&str; N]) {
            let output = Command::new("git")
                .args(args)
                .current_dir(self.path())
                .output()
                .expect("git command");
            assert!(
                output.status.success(),
                "git failed: {}\nstdout: {}\nstderr: {}",
                output.status,
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
        }
    }
}
