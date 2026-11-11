//! Optional local EPR capture through the installed executable, like the context adapter.
//! No sibling path dependency, raw argv, diagnostics, environment or network transmission.
use std::{
    path::PathBuf,
    process::{Command, Stdio},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

pub(crate) struct Capture {
    pub operation: &'static str,
    pub outcome: Option<&'static str>,
    started: Instant,
}

impl Capture {
    pub fn new() -> Self {
        Self {
            operation: "other",
            outcome: None,
            started: Instant::now(),
        }
    }

    pub fn operation(&mut self, name: &str) {
        self.operation = match name {
            "status" => "status",
            "log" => "log",
            "build" => "build",
            "snapshot" => "snapshot",
            "context" => "context",
            "integrate" => "integrate",
            "init" => "init",
            "clone" => "clone",
            "fetch" => "fetch",
            "push" => "push",
            "pull" => "pull",
            "add" => "add",
            "commit" => "commit",
            "diff" => "diff",
            "branch" => "branch",
            "switch" => "switch",
            "merge" => "merge",
            "rebase" => "rebase",
            "restore" => "restore",
            "tag" => "tag",
            "remote" => "remote",
            "submodule" => "submodule",
            _ => "other",
        };
    }

    pub fn finish(&self, code: i32) {
        // An explicit contextual policy enables capture. Standalone Brit has no implicit
        // workspace policy, and still builds/runs without EPR installed.
        let root = std::env::var_os("EPR_FEEDBACK_ROOT").map(PathBuf::from).or_else(|| {
            std::env::current_dir()
                .ok()?
                .ancestors()
                .find(|p| p.join(".epr-meta/feedback-policy.json").is_file())
                .map(PathBuf::from)
        });
        let Some(root) = root else { return };
        let result = self.record(root, code);
        if result.is_err() {
            eprintln!("brit: feedback capture gap; command result unchanged");
        }
    }

    fn record(&self, root: PathBuf, code: i32) -> std::io::Result<()> {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        // A supported host adapter may pass its opaque correlation token explicitly. Other
        // environment values are never retained; absent support gets a fresh invocation.
        let invocation = std::env::var("EPR_FEEDBACK_INVOCATION")
            .ok()
            .filter(|s| !s.is_empty() && s.len() <= 128 && s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-'))
            .unwrap_or_else(|| format!("{}-{nanos}", std::process::id()));
        let elapsed = self.started.elapsed().as_millis().to_string();
        let outcome = self.outcome.unwrap_or(if code == 0 { "passed" } else { "failed" });
        let mut child = Command::new("epr")
            .args([
                "feedback",
                "capture",
                "--producer",
                "brit",
                "--operation",
                self.operation,
                "--outcome",
                outcome,
                "--invocation",
                &invocation,
                "--elapsed-ms",
                &elapsed,
                "--root",
            ])
            .arg(root)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()?;
        let deadline = Instant::now() + Duration::from_secs(2);
        loop {
            if let Some(status) = child.try_wait()? {
                return if status.success() {
                    Ok(())
                } else {
                    Err(std::io::Error::other("recorder refused"))
                };
            }
            if Instant::now() >= deadline {
                let _ = child.kill();
                let _ = child.wait();
                return Err(std::io::Error::new(std::io::ErrorKind::TimedOut, "recorder timeout"));
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    }
}
