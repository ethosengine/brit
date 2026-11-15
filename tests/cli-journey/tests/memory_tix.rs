//! Tix owns separate history readers; ordinary log qualification does not cover them.
use anyhow::{ensure, Result};
use cli_journey::support::{runner::brit_bin, test_repo::TestRepo};
use std::{
    fs,
    io::Write,
    path::Path,
    process::{Command, Stdio},
};

fn git(root: &Path, args: &[&str]) -> Result<Vec<u8>> {
    let output = gix_testtools::git_command(root)
        .args(["-c", &format!("safe.directory={}", root.display())])
        .args(args)
        .output()?;
    ensure!(
        output.status.success(),
        "Git {args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    Ok(output.stdout)
}

fn run(root: &Path, args: &[&str], limit: Option<&str>, output: &Path) -> Result<()> {
    run_without_env(root, args, limit, output, &[])
}

fn run_without_env(root: &Path, args: &[&str], limit: Option<&str>, output: &Path, removed: &[&str]) -> Result<()> {
    let mut command = Command::new(brit_bin().expect("built Brit"));
    gix_testtools::configure_git_environment(&mut command, root);
    for name in removed {
        command.env_remove(name);
    }
    command.current_dir(root);
    if let Some(limit) = limit {
        command.args(["-c", &format!("gitoxide.objects.allocLimit={limit}")]);
    }
    eprintln!("qualifying Tix {args:?}, policy {limit:?}");
    let result = command
        .args(args)
        .stdout(Stdio::from(fs::File::create(output)?))
        .output()?;
    ensure!(
        result.status.success(),
        "{args:?}: {}",
        String::from_utf8_lossy(&result.stderr)
    );
    Ok(())
}

fn history(reduced: bool) -> Result<()> {
    let repo = TestRepo::new("memory-tix-history")?;
    let root = repo.path();
    let hidden = String::from_utf8(git(root, &["rev-parse", "HEAD"])?)?;
    let message = root.join(".git/message-input");
    let mut file = fs::File::create(&message)?;
    file.write_all(b"large governance history\n\n")?;
    let mut chunk = [b'x'; 65536];
    chunk[65535] = b'\n';
    for _ in 0..320 {
        file.write_all(&chunk)?;
    }
    drop(file);
    git(
        root,
        &[
            "commit",
            "--allow-empty",
            "-q",
            "-F",
            message.to_str().expect("fixture path"),
        ],
    )?;
    let head = git(root, &["rev-parse", "HEAD"])?;
    let index = fs::read(root.join(".git/index"))?;
    let setup_output = root.join(".git/setup-tix");
    for args in [
        ["tix", "enrich", "commit", "todo", "HEAD"],
        ["tix", "enrich", "tree", "checks-pass", "HEAD"],
    ] {
        run(root, &args, Some("1073741824"), &setup_output)?;
    }
    let refs = git(root, &["for-each-ref", "--format=%(refname) %(objectname)"])?;
    let queries = [
        vec!["tix", "show", "--no-auto-hide", "--hide", hidden.trim(), "HEAD"],
        vec!["tix", "ref-tree", "--no-auto-hide", "--hide", hidden.trim(), "HEAD"],
        vec!["tix", "worktrunk", "show"],
        vec!["tix", "op", "log"],
    ];
    let mut expected = Vec::new();
    for args in &queries {
        let output = root.join(".git/expected-tix");
        run(root, args, Some("1073741824"), &output)?;
        expected.push(fs::read(output)?);
    }
    ensure!(String::from_utf8_lossy(&expected[0]).contains("large governance history"));
    ensure!(String::from_utf8_lossy(&expected[0]).contains("🚧"));
    ensure!(String::from_utf8_lossy(&expected[0]).contains("✔"));
    if reduced {
        #[cfg(target_os = "linux")]
        {
            use std::os::unix::fs::{chown, MetadataExt};
            ensure!(fs::metadata(root)?.uid() == 0, "qualification requires chown authority");
            chown(root, Some(65534), None)?;
            chown(root.join(".git"), Some(65534), None)?;
            let trust = root.join(".git/trust");
            run(root, &["free", "trust", ".", ".git"], None, &trust)?;
            ensure!(
                fs::read_to_string(trust)?
                    .lines()
                    .filter(|line| line.starts_with("Reduced"))
                    .count()
                    == 2
            );
        }
        #[cfg(not(target_os = "linux"))]
        anyhow::bail!("actual ownership qualification requires Linux");
    }
    for packed in [false, true] {
        if packed {
            git(root, &["-c", "pack.threads=1", "repack", "-ad"])?;
        }
        for limit in [None, Some("16777216")] {
            for (args, expected) in queries.iter().zip(&expected) {
                let actual = root.join(".git/actual-tix");
                run(root, args, limit, &actual)?;
                ensure!(
                    fs::read(actual)? == *expected,
                    "allocation policy must not alter the displayed history"
                );
                ensure!(git(root, &["rev-parse", "HEAD"])? == head && fs::read(root.join(".git/index"))? == index);
                ensure!(git(root, &["for-each-ref", "--format=%(refname) %(objectname)"])? == refs);
            }
        }
    }
    Ok(())
}

#[test]
fn tix_history_with_twenty_mib_commit_metadata() -> Result<()> {
    history(false)
}

#[cfg(target_os = "linux")]
#[test]
#[ignore = "requires chown authority: qualification runs this explicitly"]
fn tix_history_with_actual_reduced_trust() -> Result<()> {
    history(true)
}

fn reword(bytes: usize, reduced: bool, editor: bool) -> Result<()> {
    for packed in [false, true] {
        for limit in [None, Some("1073741824"), Some("16777216"), Some("1024")] {
            let repo = TestRepo::new("memory-tix-reword")?;
            let root = repo.path();
            let input = root.join(".git/large-message");
            let mut file = fs::File::create(&input)?;
            file.write_all(b"large edit metadata\n\n")?;
            let mut chunk = [b'x'; 65536];
            chunk[65535] = b'\n';
            for _ in 0..bytes / chunk.len() {
                file.write_all(&chunk)?;
            }
            drop(file);
            let input_arg = input.to_str().expect("fixture path");
            git(root, &["commit", "--allow-empty", "-q", "-F", input_arg])?;
            let target = String::from_utf8(git(root, &["rev-parse", "HEAD"])?)?;
            git(root, &["commit", "--allow-empty", "-q", "-F", input_arg])?;
            let tip = git(root, &["rev-parse", "HEAD"])?;
            let message = git(root, &["show", "-s", "--format=%B", "HEAD"])?;
            let tree = git(root, &["rev-parse", "HEAD^{tree}"])?;
            let index = fs::read(root.join(".git/index"))?;
            if packed {
                git(root, &["-c", "pack.threads=1", "repack", "-ad"])?;
            }
            let output = root.join(".git/reword-output");
            run(root, &["tix", "pin", "HEAD"], Some("1073741824"), &output)?;
            if reduced {
                #[cfg(target_os = "linux")]
                {
                    use std::os::unix::fs::{chown, MetadataExt};
                    ensure!(fs::metadata(root)?.uid() == 0, "qualification requires chown authority");
                    chown(root, Some(65534), None)?;
                    chown(root.join(".git"), Some(65534), None)?;
                    run(root, &["free", "trust", ".", ".git"], None, &output)?;
                    ensure!(
                        fs::read_to_string(&output)?
                            .lines()
                            .filter(|line| line.starts_with("Reduced"))
                            .count()
                            == 2,
                        "both paths have actual reduced trust"
                    );
                }
                #[cfg(not(target_os = "linux"))]
                anyhow::bail!("ownership qualification runs on Linux");
            }
            eprintln!("qualifying Tix reword: packed={packed}, reduced={reduced}, editor={editor}");
            if editor {
                run(
                    root,
                    &["-c", "core.editor=:", "tix", "reword", target.trim()],
                    limit,
                    &output,
                )?;
                ensure!(fs::read_to_string(&output)?.contains("the editor document was unchanged"));
                ensure!(git(root, &["rev-parse", "HEAD"])? == tip);
                ensure!(git(root, &["show", "-s", "--format=%B", "HEAD^"])? == message);
            } else {
                run(
                    root,
                    &["tix", "reword", target.trim(), "-m", "rewritten metadata"],
                    limit,
                    &output,
                )?;
                ensure!(git(root, &["show", "-s", "--format=%B", "HEAD^"])? == b"rewritten metadata\n\n");
            }
            ensure!(git(root, &["show", "-s", "--format=%B", "HEAD"])? == message);
            ensure!(git(root, &["show", "-s", "--format=%B", target.trim()])? == message);
            ensure!(git(root, &["rev-parse", "HEAD^{tree}"])? == tree);
            ensure!(git(root, &["rev-parse", "HEAD^^"])? == git(root, &["rev-parse", &format!("{}^", target.trim())])?);
            ensure!(fs::read(root.join(".git/index"))? == index);
        }
    }
    Ok(())
}

#[test]
fn tix_reword_preserves_metadata_above_buffer_policy() -> Result<()> {
    reword(128 * 1024, false, false)
}

#[test]
#[ignore = "whole-group qualification runs the 20 MiB editing matrix explicitly"]
fn tix_reword_with_twenty_mib_metadata() -> Result<()> {
    reword(20 * 1024 * 1024, false, false)
}

#[cfg(target_os = "linux")]
#[test]
#[ignore = "requires chown authority: qualification runs this explicitly"]
fn tix_reword_metadata_with_actual_reduced_trust() -> Result<()> {
    reword(20 * 1024 * 1024, true, false)
}

#[test]
fn tix_reword_editor_preserves_metadata_above_buffer_policy() -> Result<()> {
    reword(128 * 1024, false, true)
}

#[test]
#[ignore = "whole-group qualification runs the 20 MiB editor handoff explicitly"]
fn tix_reword_editor_with_twenty_mib_metadata() -> Result<()> {
    reword(20 * 1024 * 1024, false, true)
}

#[cfg(target_os = "linux")]
#[test]
#[ignore = "requires chown authority: qualification runs this explicitly"]
fn tix_reword_editor_metadata_with_actual_reduced_trust() -> Result<()> {
    reword(20 * 1024 * 1024, true, true)
}

fn create(bytes: usize, reduced: bool) -> Result<()> {
    create_with_policies(
        bytes,
        reduced,
        &[None, Some("1073741824"), Some("16777216"), Some("1024")],
    )
}

fn create_with_policies(bytes: usize, reduced: bool, policies: &[Option<&str>]) -> Result<()> {
    for packed in [false, true] {
        for staged in [false, true] {
            for &limit in policies {
                let repo = TestRepo::new("memory-tix-create")?;
                let root = repo.path();
                let input = root.join(".git/large-parent");
                let mut file = fs::File::create(&input)?;
                file.write_all(b"large creation parent\n\n")?;
                let mut chunk = [b'x'; 65536];
                chunk[65535] = b'\n';
                for _ in 0..bytes / chunk.len() {
                    file.write_all(&chunk)?;
                }
                drop(file);
                git(
                    root,
                    &[
                        "commit",
                        "--allow-empty",
                        "-q",
                        "-F",
                        input.to_str().expect("fixture path"),
                    ],
                )?;
                let parent = git(root, &["rev-parse", "HEAD"])?;
                let message = git(root, &["show", "-s", "--format=%B", "HEAD"])?;
                if packed {
                    git(root, &["-c", "pack.threads=1", "repack", "-ad"])?;
                }
                fs::write(root.join("file"), b"staged bytes\n")?;
                if staged {
                    git(root, &["add", "file"])?;
                }
                fs::write(root.join("file"), b"local bytes\n")?;
                let tree = git(root, &["write-tree"])?;
                let index = root.join(".git/index");
                let before_index = index.exists().then(|| fs::read(&index)).transpose()?;
                let before_refs = git(root, &["for-each-ref", "--format=%(refname) %(objectname)"])?;
                let before_config = fs::read(root.join(".git/config"))?;
                let output = root.join(".git/create-output");
                if reduced {
                    #[cfg(target_os = "linux")]
                    {
                        use std::os::unix::fs::{chown, MetadataExt};
                        ensure!(fs::metadata(root)?.uid() == 0, "qualification requires chown authority");
                        chown(root, Some(65534), None)?;
                        chown(root.join(".git"), Some(65534), None)?;
                        run(root, &["free", "trust", ".", ".git"], None, &output)?;
                        ensure!(
                            fs::read_to_string(&output)?
                                .lines()
                                .filter(|line| line.starts_with("Reduced"))
                                .count()
                                == 2
                        );
                    }
                    #[cfg(not(target_os = "linux"))]
                    anyhow::bail!("ownership qualification runs on Linux");
                }
                eprintln!("qualifying Tix create: packed={packed}, staged={staged}, reduced={reduced}");
                let result = run_without_env(
                    root,
                    &[
                        "-c",
                        "core.editor=:",
                        "-c",
                        "committer.name=Invocation Committer",
                        "-c",
                        "committer.email=invocation@example.com",
                        "tix",
                        "new",
                        "--allow-empty",
                        "-m",
                        "created metadata",
                    ],
                    limit,
                    &output,
                    &["GIT_COMMITTER_NAME", "GIT_COMMITTER_EMAIL"],
                );
                if reduced {
                    let error = result.expect_err("the owned Git reset retains its ownership protection");
                    let diagnostic = format!("{error:#}");
                    ensure!(
                        diagnostic.contains("git reset failed") && diagnostic.contains("dubious ownership"),
                        "{diagnostic}"
                    );
                    ensure!(git(root, &["rev-parse", "HEAD"])? == parent);
                    ensure!(git(root, &["show", "-s", "--format=%B", "HEAD"])? == message);
                    ensure!(git(root, &["for-each-ref", "--format=%(refname) %(objectname)"])? == before_refs);
                    git(root, &["fsck", "--full"])?;
                } else {
                    result?;
                    ensure!(git(root, &["rev-parse", "HEAD^"])? == parent);
                    ensure!(git(root, &["rev-parse", "HEAD^{tree}"])? == tree);
                    ensure!(git(root, &["show", "-s", "--format=%B", "HEAD"])? == b"created metadata\n\n");
                    ensure!(
                        git(root, &["show", "-s", "--format=%cn <%ce>", "HEAD"])?
                            == b"Invocation Committer <invocation@example.com>\n"
                    );
                    ensure!(git(root, &["show", "-s", "--format=%B", "HEAD^"])? == message);
                }
                ensure!(fs::read(root.join("file"))? == b"local bytes\n");
                ensure!(index.exists().then(|| fs::read(&index)).transpose()? == before_index);
                ensure!(fs::read(root.join(".git/config"))? == before_config);
            }
        }
    }
    Ok(())
}

#[test]
fn tix_creation_preserves_metadata_above_buffer_policy() -> Result<()> {
    create(128 * 1024, false)
}

#[test]
#[ignore = "whole-group qualification runs the 20 MiB creation matrix explicitly"]
fn tix_creation_with_twenty_mib_metadata() -> Result<()> {
    create(20 * 1024 * 1024, false)
}

#[cfg(target_os = "linux")]
#[test]
#[ignore = "requires chown authority: qualification runs this explicitly"]
fn tix_creation_metadata_retains_git_ownership_refusal() -> Result<()> {
    create(20 * 1024 * 1024, true)
}

#[cfg(target_os = "linux")]
#[test]
#[ignore = "requires chown authority: isolates the pre-existing Git ownership boundary"]
fn tix_creation_ownership_refusal_preserves_state() -> Result<()> {
    create_with_policies(128 * 1024, true, &[None])
}

fn notes(bytes: usize, reduced: bool) -> Result<()> {
    let repo = TestRepo::new("memory-tix-notes")?;
    let root = repo.path();
    let message = root.join(".git/note-input");
    let mut file = fs::File::create(&message)?;
    let mut chunk = [b'n'; 65536];
    chunk[65535] = b'\n';
    for _ in 0..bytes / chunk.len() {
        file.write_all(&chunk)?;
    }
    drop(file);
    let output = root.join(".git/note-output");
    let message_arg = message.to_str().expect("fixture path");
    git(root, &["notes", "add", "-F", message_arg, "HEAD"])?;
    run(
        root,
        &["tix", "enrich", "commit", "note", "-f", message_arg, "HEAD"],
        Some("1073741824"),
        &output,
    )?;
    let head = git(root, &["rev-parse", "HEAD"])?;
    let refs = git(root, &["for-each-ref", "--format=%(refname) %(objectname)"])?;
    if reduced {
        #[cfg(target_os = "linux")]
        {
            use std::os::unix::fs::{chown, MetadataExt};
            ensure!(fs::metadata(root)?.uid() == 0, "qualification requires chown authority");
            chown(root, Some(65534), None)?;
            chown(root.join(".git"), Some(65534), None)?;
            run(root, &["free", "trust", ".", ".git"], None, &output)?;
            ensure!(
                fs::read_to_string(&output)?
                    .lines()
                    .filter(|line| line.starts_with("Reduced"))
                    .count()
                    == 2
            );
        }
        #[cfg(not(target_os = "linux"))]
        anyhow::bail!("actual ownership qualification requires Linux");
    }
    for packed in [false, true] {
        if packed {
            git(root, &["-c", "pack.threads=1", "repack", "-ad"])?;
        }
        for limit in [Some("1073741824"), Some("16777216"), Some("1024"), None] {
            for kind in ["git-note", "note"] {
                run(
                    root,
                    &["tix", "enrich", "commit", kind, "-f", message_arg, "HEAD"],
                    limit,
                    &output,
                )?;
                ensure!(
                    String::from_utf8_lossy(&fs::read(&output)?).contains("unchanged"),
                    "an identical note must remain unchanged"
                );
                ensure!(git(root, &["rev-parse", "HEAD"])? == head);
                ensure!(git(root, &["for-each-ref", "--format=%(refname) %(objectname)"])? == refs);
            }
        }
    }
    let old_head = String::from_utf8(head)?.trim().to_owned();
    let note_blob = git(root, &["notes", "list", "HEAD"])?;
    run(
        root,
        &["tix", "reword", "-m", "rewritten with a large note", "HEAD"],
        Some("1024"),
        &output,
    )?;
    ensure!(String::from_utf8(git(root, &["rev-parse", "HEAD"])?)?.trim() != old_head);
    ensure!(
        git(root, &["notes", "list", "HEAD"])? == note_blob,
        "successor retains the exact note blob"
    );
    ensure!(
        git(root, &["notes", "list", &old_head])? == note_blob,
        "predecessor retains its note"
    );
    Ok(())
}

#[test]
fn tix_notes_preserve_bytes_above_explicit_buffer_limit() -> Result<()> {
    notes(65536, false)
}

#[test]
#[ignore = "whole-group qualification runs the 20 MiB note matrix explicitly"]
fn tix_notes_with_twenty_mib_payloads() -> Result<()> {
    notes(20 * 1024 * 1024, false)
}

#[cfg(target_os = "linux")]
#[test]
#[ignore = "requires chown authority: qualification runs this explicitly"]
fn tix_notes_with_actual_reduced_trust() -> Result<()> {
    notes(20 * 1024 * 1024, true)
}
