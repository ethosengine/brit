//! Synthetic credential fields only: no real account data enters these fixtures or receipts.
#![cfg(target_os = "linux")]

use anyhow::{ensure, Result};
use cli_journey::support::{runner::brit_bin, test_repo::TestRepo};
use std::{
    collections::BTreeMap,
    fs,
    io::{BufRead, BufReader, BufWriter, Write},
    path::Path,
    process::{Command, Stdio},
};

fn fields(path: &Path) -> Result<BTreeMap<Vec<u8>, Vec<u8>>> {
    let mut result = BTreeMap::new();
    for line in BufReader::new(fs::File::open(path)?).split(b'\n') {
        let line = line?;
        if line.is_empty() {
            break;
        }
        let separator = line.iter().position(|b| *b == b'=').expect("protocol field");
        result.insert(line[..separator].to_vec(), line[separator + 1..].to_vec());
    }
    Ok(result)
}

fn invoke(root: &Path, stock: bool, action: &str, input: &Path, output: &Path, helper: bool) -> Result<()> {
    let mut command = if stock {
        gix_testtools::git_command(root)
    } else {
        let mut command = Command::new(brit_bin().expect("built Brit"));
        gix_testtools::configure_git_environment(&mut command, root);
        command.current_dir(root);
        command
    };
    command.args(["-c", "credential.helper="]);
    if helper {
        command.args([
            "-c",
            "credential.helper=!f() { cat > \"$BRIT_TEST_CREDENTIAL_INPUT\"; if test \"$1\" = get; then cat \"$BRIT_TEST_CREDENTIAL_OUTPUT\"; fi; }; f",
        ]);
    }
    let result = command
        .args(["credential", action])
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("BRIT_TEST_CREDENTIAL_INPUT", root.join(".git/helper-input"))
        .env("BRIT_TEST_CREDENTIAL_OUTPUT", root.join(".git/helper-output"))
        .stdin(Stdio::from(fs::File::open(input)?))
        .stdout(Stdio::from(fs::File::create(output)?))
        .output()?;
    // Do not include protocol bytes in failures, even though every field here is synthetic.
    ensure!(
        result.status.success(),
        "credential {action} stock={stock} helper={helper} failed: {}",
        result.status
    );
    Ok(())
}

#[test]
#[ignore = "resource qualification: requires BRIT_BIN; synthetic long credential fields"]
fn large_credential_input_and_helper_output() -> Result<()> {
    let repo = TestRepo::new("memory-credentials")?;
    let root = repo.path();
    let head = repo.head_id()?;
    let index = fs::read(root.join(".git/index"))?;
    let complete = root.join(".git/complete-input");
    let query = root.join(".git/query-input");
    fs::write(&query, "protocol=https\nhost=example.invalid\n\n")?;
    let mut input = BufWriter::new(fs::File::create(&complete)?);
    input.write_all(b"protocol=https\nhost=example.invalid\nusername=fixture\npassword=")?;
    for _ in 0..320 {
        input.write_all(&[b's'; 65536])?;
    }
    input.write_all(b"\n\n")?;
    input.flush()?;
    drop(input);
    fs::copy(&complete, root.join(".git/helper-output"))?;
    let expected = root.join(".git/expected-output");
    let actual = root.join(".git/actual-output");
    for helper in [false, true] {
        let input = if helper { &query } else { &complete };
        invoke(root, true, "fill", input, &expected, helper)?;
        invoke(root, false, "fill", input, &actual, helper)?;
        ensure!(
            fields(&expected)? == fields(&actual)?,
            "credential fill fields differ from Git"
        );
    }
    for action in ["approve", "reject"] {
        invoke(root, true, action, &complete, &expected, true)?;
        let expected_fields = fields(&root.join(".git/helper-input"))?;
        invoke(root, false, action, &complete, &actual, true)?;
        ensure!(
            expected_fields == fields(&root.join(".git/helper-input"))?,
            "credential {action} helper input differs from Git"
        );
        ensure!(fs::metadata(&expected)?.len() == 0 && fs::metadata(&actual)?.len() == 0);
    }
    ensure!(repo.head_id()? == head && fs::read(root.join(".git/index"))? == index);
    Ok(())
}
