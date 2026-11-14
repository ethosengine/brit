//! Noninteractive editor handoff; interactive terminal behavior has separate authoring tests.
#![cfg(target_os = "linux")]

use anyhow::{ensure, Result};
use cli_journey::support::{runner::brit_bin, test_repo::TestRepo};
use std::{
    fs,
    io::{BufWriter, Write},
    process::Command,
};

#[test]
#[ignore = "resource qualification: requires BRIT_BIN and Python 3"]
fn editor_child_loads_sixty_four_and_two_fifty_six_mib() -> Result<()> {
    let repo = TestRepo::new("memory-editor")?;
    let root = repo.path();
    let head = repo.head_id()?;
    let index = fs::read(root.join(".git/index"))?;
    let editor = root.join(".git/editor.py");
    let receipt = root.join(".git/editor-receipt.json");
    fs::write(
        &editor,
        r#"import hashlib, json, os, pathlib, sys
rows = []
for argument in sys.argv[1:]:
    data = pathlib.Path(argument).read_bytes()
    rows.append([argument, len(data), hashlib.sha256(data).hexdigest()])
    del data
pathlib.Path(os.environ['BRIT_TEST_EDITOR_RECEIPT']).write_text(json.dumps(rows))
sys.exit(int(os.environ['BRIT_TEST_EDITOR_EXIT']))
"#,
    )?;
    let path = root.join("file with spaces");
    for mib in [64, 256] {
        let mut file = BufWriter::new(fs::File::create(&path)?);
        for _ in 0..mib * 16 {
            file.write_all(&[b'x'; 65536])?;
        }
        file.flush()?;
        drop(file);
        let oracle = Command::new("sha256sum").arg(&path).output()?;
        ensure!(oracle.status.success());
        let hash = String::from_utf8(oracle.stdout)?;
        let expected = serde_json::json!([[
            path,
            mib * 1024 * 1024,
            hash.split_whitespace().next().expect("SHA-256")
        ]]);
        for limit in [None, Some("1")] {
            for exit in ["0", "37"] {
                fs::remove_file(&receipt).or_else(|err| {
                    if err.kind() == std::io::ErrorKind::NotFound {
                        Ok(())
                    } else {
                        Err(err)
                    }
                })?;
                let mut command = Command::new(brit_bin().expect("built Brit"));
                gix_testtools::configure_git_environment(&mut command, root);
                command
                    .current_dir(root)
                    .env("GIT_EDITOR", "python3 \"$BRIT_TEST_EDITOR\"")
                    .env("BRIT_TEST_EDITOR", &editor)
                    .env("BRIT_TEST_EDITOR_RECEIPT", &receipt)
                    .env("BRIT_TEST_EDITOR_EXIT", exit);
                if let Some(limit) = limit {
                    command.args(["-c", &format!("gitoxide.objects.allocLimit={limit}")]);
                }
                let output = command.arg("editor").arg(&path).output()?;
                ensure!(output.status.success() == (exit == "0"), "editor exit status lost");
                if exit == "37" {
                    ensure!(
                        String::from_utf8_lossy(&output.stderr).contains("37"),
                        "editor exit detail lost"
                    );
                }
                ensure!(
                    serde_json::from_slice::<serde_json::Value>(&fs::read(&receipt)?)? == expected,
                    "editor path/bytes differ"
                );
            }
        }
        let after = Command::new("sha256sum").arg(&path).output()?;
        ensure!(after.status.success() && String::from_utf8(after.stdout)? == hash);
    }
    ensure!(repo.head_id()? == head && fs::read(root.join(".git/index"))? == index);
    Ok(())
}
