//! Where per-repo brit state (agent key, object store) lives.

use std::{
    path::{Path, PathBuf},
    process::Command,
};

/// The repository's common git dir: `.git` in a normal checkout, the main repo's `.git` from a
/// linked worktree (where `<repo>/.git` is a file, not a directory).
pub fn common_git_dir(repo: &Path) -> anyhow::Result<PathBuf> {
    let out = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(["rev-parse", "--git-common-dir"])
        .output()
        .map_err(|err| anyhow::anyhow!("failed to resolve the common Git directory: {err}"))?;
    if !out.status.success() {
        let diagnostic = String::from_utf8_lossy(&out.stderr);
        anyhow::bail!("failed to resolve the common Git directory: {}", diagnostic.trim());
    }
    let text = std::str::from_utf8(&out.stdout)
        .map_err(|err| anyhow::anyhow!("common Git directory is not valid UTF-8: {err}"))?;
    let line = text.trim();
    if line.is_empty() {
        anyhow::bail!("failed to resolve the common Git directory: Git returned an empty path");
    }
    let p = PathBuf::from(line);
    if p.is_absolute() {
        Ok(p)
    } else {
        Ok(repo.join(p))
    }
}

/// `<git-common-dir>/brit/agent-key`.
pub fn agent_key_path(repo: &Path) -> anyhow::Result<PathBuf> {
    Ok(common_git_dir(repo)?.join("brit").join("agent-key"))
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
        assert_eq!(
            agent_key_path(&repo).expect("common Git directory resolves"),
            repo.join(".git/brit/agent-key")
        );
    }

    #[test]
    fn linked_worktree_resolves_to_the_main_repos_key() {
        // Git is handed plain paths: a canonicalized Windows path (`\\?\C:\…`) is refused by
        // `git worktree add`. The comparison canonicalizes both sides instead, which also absorbs
        // macOS's /var -> /private/var temp-dir symlink.
        let tmp = tempfile::TempDir::new().unwrap();
        let main = tmp.path().join("main");
        std::fs::create_dir(&main).unwrap();
        git(&main, &["init", "-q", "--initial-branch=main"]);
        git(&main, &["commit", "--allow-empty", "-q", "-m", "init"]);
        let wt = tmp.path().join("wt");
        git(&main, &["worktree", "add", "-q", wt.to_str().unwrap(), "-b", "side"]);
        assert!(wt.join(".git").is_file());
        assert_eq!(
            common_git_dir(&wt)
                .expect("linked worktree common Git directory resolves")
                .canonicalize()
                .expect("resolved common Git directory canonicalizes"),
            main.join(".git")
                .canonicalize()
                .expect("main Git directory canonicalizes")
        );
        assert!(agent_key_path(&wt)
            .expect("linked worktree key path resolves")
            .ends_with(Path::new("brit").join("agent-key")));
    }

    #[test]
    fn non_repository_is_an_error() {
        let tmp = tempfile::TempDir::new().expect("temporary non-repository directory");
        assert!(
            common_git_dir(tmp.path()).is_err(),
            "repository discovery must fail closed"
        );
        assert!(
            !tmp.path().join(".git").exists(),
            "resolution does not invent a Git directory"
        );
    }
}
