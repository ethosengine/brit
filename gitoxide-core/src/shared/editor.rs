//! Spawn the user's editor on a temp file, return post-edit bytes.
//!
//! Resolution order matches git's `launch_editor()` in `editor.c`:
//!   1. `$GIT_EDITOR` env (if non-empty)
//!   2. `core.editor` config (if set and non-empty, trusted sources only)
//!   3. `$VISUAL` env (if non-empty)
//!   4. `$EDITOR` env (if non-empty)
//!   5. `vi`
//!
//! The editor command is run through a shell so multi-word commands
//! (e.g. `vim -O`) work, mirroring git's `p.use_shell = 1` flag in
//! `launch_specified_editor()`. The temp file is passed as a separate
//! positional argument (not embedded in the shell command string), which
//! is the safe equivalent of git's `strvec_pushl(&p.args, editor, path, NULL)`.

use anyhow::Context as _;
use std::{ffi::OsString, io::Write as _, process::Stdio};

/// Resolve the editor to use, following git's precedence chain.
///
/// Returns an `OsString` so that paths containing non-UTF-8 bytes are handled
/// correctly on POSIX systems where `$EDITOR` may be set to a non-UTF-8 path.
fn resolve_editor(repo: &gix::Repository) -> OsString {
    // 1. GIT_EDITOR env var
    if let Some(v) = std::env::var_os("GIT_EDITOR") {
        if !v.is_empty() {
            return v;
        }
    }

    // 2. core.editor git config (trusted sources only — matches git's security model)
    if let Some(v) = repo.config_snapshot().trusted_program("core.editor") {
        if !v.is_empty() {
            return v.into_owned();
        }
    }

    // 3. VISUAL env var
    if let Some(v) = std::env::var_os("VISUAL") {
        if !v.is_empty() {
            return v;
        }
    }

    // 4. EDITOR env var
    if let Some(v) = std::env::var_os("EDITOR") {
        if !v.is_empty() {
            return v;
        }
    }

    // 5. fallback: vi (git's DEFAULT_EDITOR)
    OsString::from("vi")
}

/// Open a temp file (in `$GIT_DIR`) populated with `initial`, spawn the editor
/// on it, and return the post-edit bytes.
///
/// The temp file is named `.<prefix>~<pid>` so concurrent invocations don't
/// collide. The file is deleted on both success and failure paths via
/// `CleanupGuard`.
///
/// The editor is run through a shell (equivalent to git's `p.use_shell = 1`)
/// so multi-word editor commands like `vim -O` work. The temp file path is
/// passed as a separate positional argument — not embedded in the shell command
/// string — so paths with spaces or shell metacharacters are safe.
///
/// Stdin and stdout are inherited from the parent process so the editor can use
/// the terminal interactively.
///
/// Parity note: `$EDITOR=true` (the no-op editor used in parity fixtures) works
/// correctly: `true` exits 0, the file is left unchanged, and the original bytes
/// are returned.
pub fn edit_file(repo: &gix::Repository, initial: &[u8], prefix: &str) -> anyhow::Result<Vec<u8>> {
    let editor = resolve_editor(repo);

    let git_dir = repo.git_dir().to_owned();
    let temp_name = format!(".{prefix}~{}", std::process::id());
    let temp_path = git_dir.join(&temp_name);

    {
        let mut f =
            std::fs::File::create(&temp_path).with_context(|| format!("creating temp file {}", temp_path.display()))?;
        f.write_all(initial)
            .with_context(|| format!("writing initial content to temp file {}", temp_path.display()))?;
    }

    // Ensure the temp file is removed on every exit path (success, editor error,
    // spawn error, etc.).
    struct CleanupGuard(std::path::PathBuf);
    impl Drop for CleanupGuard {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.0);
        }
    }
    let _guard = CleanupGuard(temp_path.clone());

    // Run the editor through a shell so multi-word commands work (git's `use_shell = 1`).
    // Passing the path via `.arg()` after `.with_shell()` causes gix-command to expand
    // it as `"$@"` in the shell script — equivalent to git's:
    //   strvec_pushl(&p.args, editor, realpath.buf, NULL);  p.use_shell = 1;
    //
    // The resulting shell invocation is:
    //   sh -c '<editor> "$@"' -- <temp_path>
    //
    // stdin/stdout must be inherited so the editor can use the terminal.
    let status = gix::command::prepare(editor.clone())
        .with_shell()
        .arg(temp_path.as_os_str())
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .spawn()
        .with_context(|| format!("spawning editor {}", editor.display()))?
        .wait()
        .with_context(|| format!("waiting for editor {}", editor.display()))?;

    if !status.success() {
        anyhow::bail!("editor {} exited with {status}", editor.display());
    }

    let edited =
        std::fs::read(&temp_path).with_context(|| format!("reading edited temp file {}", temp_path.display()))?;
    Ok(edited)
}
