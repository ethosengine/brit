//! Bounded helper interface; no repository discovery or Git subprocesses.
use std::{ffi::OsString, path::PathBuf};

/// `println!` that returns its I/O error instead of panicking when stdout's reader has closed.
macro_rules! outln {
    ($($arg:tt)*) => {{
        use std::io::Write as _;
        let mut out = crate::shared::closed_reader::Watch(std::io::stdout().lock());
        writeln!(out, $($arg)*).and_then(|()| out.flush())
    }};
}

/// Run the separately shipped retained-copy helper.
pub fn run(arguments: impl IntoIterator<Item = OsString>) -> anyhow::Result<()> {
    let arguments: Vec<_> = arguments.into_iter().collect();
    if arguments.len() == 1 && matches!(arguments[0].to_str(), Some("--help" | "-h")) {
        outln!(
            "brit-rebase-retained inspect --copy <copy>\nbrit-rebase-retained extract --copy <copy> --to <empty-directory>"
        )?;
        return Ok(());
    }
    let usage = "expected inspect --copy <copy> or extract --copy <copy> --to <empty-directory>";
    let (copy, destination) = match arguments.as_slice() {
        [action, flag, copy] if action == "inspect" && flag == "--copy" => (PathBuf::from(copy), None),
        [action, flag, copy, to, destination] if action == "extract" && flag == "--copy" && to == "--to" => {
            (PathBuf::from(copy), Some(PathBuf::from(destination)))
        }
        _ => anyhow::bail!(usage),
    };
    #[cfg(target_os = "linux")]
    {
        use gitoxide_core::repository::replay::retained;
        if let Some(destination) = destination {
            let receipt = retained::extract(&copy, &destination)?;
            outln!(
                "Destination path bytes: {:?}",
                destination.as_os_str().as_encoded_bytes()
            )?;
            outln!("Copy path bytes: {:?}", receipt.path.as_os_str().as_encoded_bytes())?;
            outln!(
                "Extracted verified payloads to {}\nCopy: {}\nReceipt: {}\nRetained copy remains; delete only its exact printed leaf after verifying the export.",
                destination.display(),
                receipt.path.display(),
                receipt.digest
            )?;
        } else {
            let copy = retained::inspect(&copy)?;
            outln!("{}", copy.inspection_json()?)?;
        }
        Ok(())
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = (copy, destination);
        anyhow::bail!("retained rebase copy inspection/extraction currently requires Linux")
    }
}
