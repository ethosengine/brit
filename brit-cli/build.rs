use std::{path::Path, process::Command};

fn git(manifest_dir: &Path, args: &[&str]) -> Option<String> {
    let mut command = Command::new("git");
    command.current_dir(manifest_dir).args(args);
    for (name, _) in std::env::vars_os() {
        if name.to_string_lossy().to_ascii_uppercase().starts_with("GIT_") {
            command.env_remove(name);
        }
    }
    let output = command.output().ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8(output.stdout).ok())?
        .map(|s| s.trim().to_owned())
}

fn watch_git_path(manifest_dir: &Path, path: &str) {
    if let Some(resolved) = git(
        manifest_dir,
        &["rev-parse", "--path-format=absolute", "--git-path", path],
    ) {
        println!("cargo:rerun-if-changed={resolved}");
    }
}

fn main() {
    let manifest_dir = std::env::var_os("CARGO_MANIFEST_DIR")
        .map(std::path::PathBuf::from)
        .expect("Cargo supplies CARGO_MANIFEST_DIR to build scripts");
    let checkout_root = manifest_dir.parent().and_then(|path| path.canonicalize().ok());
    let git_root = git(&manifest_dir, &["rev-parse", "--show-toplevel"])
        .and_then(|path| std::path::PathBuf::from(path).canonicalize().ok());
    let in_checkout = checkout_root.is_some() && checkout_root == git_root;
    let source = in_checkout
        .then(|| git(&manifest_dir, &["rev-parse", "HEAD"]))
        .flatten()
        .unwrap_or_else(|| "unavailable".into());
    let features = ["max", "max-pure", "small", "lean", "lean-async"]
        .into_iter()
        .filter(|feature| {
            std::env::var_os(format!(
                "CARGO_FEATURE_{}",
                feature.to_ascii_uppercase().replace('-', "_")
            ))
            .is_some()
        })
        .collect::<Vec<_>>()
        .join(",");
    println!("cargo:rustc-env=BRIT_GIT_SHA={source}");
    println!("cargo:rustc-env=BRIT_SOURCE_STATE=unverified; HEAD does not attest working-tree bytes");
    println!("cargo:rustc-env=BRIT_FEATURES={features}");

    // In submodules and linked worktrees `.git` is a pointer file. Resolve the
    // actual gitdir paths so a commit, checkout, or packed-ref update reruns us.
    if in_checkout {
        let dot_git = manifest_dir.join("..").join(".git");
        if dot_git.is_file() {
            println!("cargo:rerun-if-changed={}", dot_git.display());
        }
        watch_git_path(&manifest_dir, "HEAD");
        if let Some(head_ref) = git(&manifest_dir, &["symbolic-ref", "-q", "HEAD"]) {
            watch_git_path(&manifest_dir, &head_ref);
        }
        watch_git_path(&manifest_dir, "packed-refs");
    }
}
