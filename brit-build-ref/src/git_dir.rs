//! Where per-repo brit state (agent key, object store) lives.

use std::{
    path::{Path, PathBuf},
    process::Command,
};

/// The repository's common git dir: `.git` in a normal checkout, the main repo's `.git` from a
/// linked worktree (where `<repo>/.git` is a file, not a directory). Falls back to `<repo>/.git`
/// when git cannot answer.
pub fn common_git_dir(repo: &Path) -> PathBuf {
    let fallback = repo.join(".git");
    let Ok(out) = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(["rev-parse", "--git-common-dir"])
        .output()
    else {
        return fallback;
    };
    if !out.status.success() {
        return fallback;
    }
    let text = String::from_utf8_lossy(&out.stdout);
    let line = text.trim();
    if line.is_empty() {
        return fallback;
    }
    let p = PathBuf::from(line);
    if p.is_absolute() {
        p
    } else {
        repo.join(p)
    }
}

/// `<git-common-dir>/brit/agent-key`.
pub fn agent_key_path(repo: &Path) -> PathBuf {
    common_git_dir(repo).join("brit").join("agent-key")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn git(dir: &Path, args: &[&str]) {
        let st = Command::new("git")
            .arg("-C")
            .arg(dir)
            .args(["-c", "user.name=t", "-c", "user.email=t@t"])
            .args(args)
            .status()
            .unwrap();
        assert!(st.success(), "git {args:?}");
    }

    #[test]
    fn normal_checkout_keeps_dot_git_key_path() {
        let tmp = tempfile::TempDir::new().unwrap();
        let repo = tmp.path().canonicalize().unwrap();
        git(&repo, &["init", "-q", "--initial-branch=main"]);
        assert_eq!(agent_key_path(&repo), repo.join(".git/brit/agent-key"));
    }

    #[test]
    fn linked_worktree_resolves_to_the_main_repos_key() {
        let tmp = tempfile::TempDir::new().unwrap();
        let root = tmp.path().canonicalize().unwrap();
        let main = root.join("main");
        std::fs::create_dir(&main).unwrap();
        git(&main, &["init", "-q", "--initial-branch=main"]);
        git(&main, &["commit", "--allow-empty", "-q", "-m", "init"]);
        let wt = root.join("wt");
        git(&main, &["worktree", "add", "-q", wt.to_str().unwrap(), "-b", "side"]);
        assert!(wt.join(".git").is_file());
        assert_eq!(agent_key_path(&wt), main.join(".git/brit/agent-key"));
    }
}
