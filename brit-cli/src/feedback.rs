//! Optional local EPR capture through the installed executable, like the context adapter.
//! No sibling path dependency, raw argv, diagnostics, environment or network transmission.
use std::{
    io::{BufRead, BufReader, Read},
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
            "show" => "show",
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
        if let Err(error) = result {
            brit_epr::diagnostic::emit(format_args!(
                "brit: feedback capture gap: {error}; command result unchanged"
            ));
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
            .stderr(Stdio::piped())
            .spawn()?;
        let deadline = Instant::now() + Duration::from_secs(2);
        let mut stderr = child
            .stderr
            .take()
            .ok_or_else(|| std::io::Error::other("recorder stderr unavailable"))?;
        let (send, receive) = std::sync::mpsc::sync_channel(1);
        // Drain concurrently so a verbose refusal cannot fill the pipe and become a
        // false timeout. Bound the diagnostic, and never join a reader whose pipe
        // may still be held by a descendant after the recorder exits.
        if let Err(error) = std::thread::Builder::new()
            .name("feedback-stderr".into())
            .spawn(move || {
                let mut line = Vec::new();
                let diagnostic = BufReader::new(stderr.by_ref().take(4097))
                    .read_until(b'\n', &mut line)
                    .map(|_| {
                        let truncated = line.len() > 4096;
                        line.truncate(4096);
                        let mut line = String::from_utf8_lossy(&line).trim_end_matches(['\r', '\n']).to_owned();
                        if truncated {
                            line.push_str(" [truncated]");
                        }
                        line
                    });
                let _ = send.send(diagnostic);
                let _ = std::io::copy(&mut stderr, &mut std::io::sink());
            })
        {
            let _ = child.kill();
            let _ = child.wait();
            return Err(error);
        }
        loop {
            if let Some(status) = child.try_wait()? {
                return if status.success() {
                    Ok(())
                } else {
                    let diagnostic = receive
                        .recv_timeout(deadline.saturating_duration_since(Instant::now()))
                        .ok()
                        .and_then(Result::ok)
                        .filter(|line| !line.is_empty())
                        .unwrap_or_else(|| format!("recorder refused ({status})"));
                    Err(std::io::Error::other(diagnostic))
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
