//! Optional local EPR capture through the installed executable, like the context adapter.
//! Capture sends a fixed argument list and compiled claim IDs to the local recorder.
pub(crate) mod claims;
mod markers;

use std::{
    io::{BufRead, BufReader, ErrorKind, Read, Write},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use self::{
    claims::RecorderCause,
    markers::{GapWrite, Recorder},
};

#[derive(clap::Subcommand)]
pub(crate) enum CommandLine {
    /// Print the compiled feedback claim declarations.
    Claims {
        /// Write the declarations as JSON.
        #[arg(long)]
        json: bool,
    },
}

pub(crate) fn write_claims(mut out: impl Write, json: bool) -> crate::error::Result<()> {
    if json {
        serde_json::to_writer(&mut out, &claims::Manifest::COMPILED)?;
        writeln!(out)?;
    } else {
        for row in claims::DECLARATIONS {
            writeln!(out, "{}", row.id)?;
        }
    }
    Ok(())
}

pub(crate) struct Capture {
    pub claim: claims::Claim,
    pub operation: &'static str,
    pub outcome: Option<&'static str>,
    pub quiet: bool,
    pub skip: bool,
    cures: &'static [claims::Cure],
    cured_claim: Option<claims::Claim>,
    started: Instant,
}

impl Capture {
    pub fn new() -> Self {
        Self {
            claim: claims::OTHER,
            operation: "other",
            outcome: None,
            quiet: false,
            skip: false,
            cures: claims::CURES,
            cured_claim: None,
            started: Instant::now(),
        }
    }

    pub fn operation(&mut self, name: &str) {
        self.claim = claims::command(name).unwrap_or(claims::UNKNOWN_COMMAND);
        self.operation = recorder_operation(name);
    }

    pub fn note_successful_form(&mut self, matches: &clap::ArgMatches) {
        if !self.cures.is_empty() {
            self.cured_claim =
                claims::form_claim(matches).filter(|claim| claims::cure_standing(self.cures, claim.id()));
        }
    }

    pub fn finish(&self, code: i32) {
        if self.skip {
            return;
        }
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
        let Some(emitter) = markers::emitter() else { return };
        let outcome = self.outcome.unwrap_or(if code == 0 { "passed" } else { "failed" });
        let session = gap_notice_key();
        self.observe(&root, &session, &emitter, self.claim, outcome, false);
        if let Some(claim) = self.cured_claim.filter(|_| code == 0 && outcome == "passed") {
            self.observe(&root, &session, &emitter, claim, "passed", true);
        }
    }

    fn observe(&self, root: &Path, session: &str, emitter: &str, claim: claims::Claim, outcome: &str, cure: bool) {
        let success = markers::success_file(session, emitter, claim, outcome);
        if success.as_ref().is_some_and(|path| path.is_file()) {
            return;
        }
        let _cure_lock = if cure {
            let Some(marker) = &success else { return };
            let Ok(lock) = markers::lock_file(&marker.with_extension("lock")) else {
                return;
            };
            if marker.is_file() {
                return;
            }
            Some(lock)
        } else {
            None
        };
        let recorder = Recorder::resolve();
        let spool = recorder.spool_file(session);
        let full_spool = spool.as_deref().and_then(markers::full_spool_marker);
        // A recorder that refuses one record still records others, so a refusal is kept under a
        // key that also names the claim: it stops retries of that claim only, never the session.
        let claim_spool = recorder.spool_file(&format!("{session}\0{}", claim.id()));
        let full_claim_spool = claim_spool.as_deref().and_then(markers::full_spool_marker);
        // Older empty markers have no cause to report. Refresh them on the next failure.
        if let Some(gap) = [&full_spool, &full_claim_spool]
            .into_iter()
            .flatten()
            .filter(|path| path.is_file())
            .find_map(|path| markers::read_gap(path))
        {
            self.report_full_spool(session, gap.cause);
            return;
        }
        for path in [&spool, &claim_spool].into_iter().flatten() {
            if path.is_file() {
                if let Some(gap) = markers::read_gap(path) {
                    self.report_gap(session, gap.cause);
                }
                return;
            }
        }
        match self.record(root, claim, outcome, emitter, &recorder) {
            Ok(()) => {
                if let Some(path) = success {
                    let _ = markers::mark(&path);
                }
            }
            Err(gap) => {
                let (spool, full_marker) = if matches!(gap.cause, RecorderCause::Refused) {
                    (claim_spool, full_claim_spool)
                } else {
                    (spool, full_spool)
                };
                if spool.is_some_and(|path| {
                    matches!(
                        markers::write_gap(&path, &recorder, gap.cause, emitter, session, gap.stderr.as_deref()),
                        Ok(GapWrite::Full)
                    )
                }) {
                    if let Some(path) = full_marker {
                        let _ = markers::mark_full_spool(&path, gap.cause);
                    }
                    self.report_full_spool(session, gap.cause);
                    return;
                }
                self.report_gap(session, gap.cause);
            }
        }
    }

    fn gap_is_visible(&self) -> bool {
        !self.quiet && std::env::var_os("BRIT_QUIET").is_none_or(|value| value != "1")
    }

    fn report_gap(&self, session: &str, cause: RecorderCause) {
        if self.gap_is_visible() && markers::claim_notice(session) {
            brit_epr::diagnostic::emit_with_claim(
                format_args!(
                    "brit: feedback capture gap: {}; command result unchanged",
                    cause.as_str()
                ),
                cause.claim().id(),
            );
        }
    }

    fn report_full_spool(&self, session: &str, cause: RecorderCause) {
        if self.gap_is_visible() && markers::claim_notice(&format!("{session}.spool-full")) {
            brit_epr::diagnostic::emit_with_claim(
                format_args!(
                    "brit: feedback spool full ({} entries); recorder gaps are not kept until epr drains it; command result unchanged",
                    markers::SPOOL_MAX_ENTRIES
                ),
                cause.claim().id(),
            );
        }
    }

    fn record(
        &self,
        root: &Path,
        claim: claims::Claim,
        outcome: &str,
        emitter: &str,
        recorder: &Recorder,
    ) -> Result<(), RecorderFailure> {
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
            .env("EPR_FEEDBACK_CLAIM", claim.id())
            .env("EPR_FEEDBACK_EMITTER", emitter)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|error| {
                let cause = if error.kind() == ErrorKind::NotFound && !recorder.found() {
                    RecorderCause::NotFound
                } else {
                    RecorderCause::SpawnFailed
                };
                RecorderFailure::new(cause, None)
            })?;
        let deadline = Instant::now() + Duration::from_secs(2);
        let mut stderr = child.stderr.take().expect("the recorder stderr is piped");
        let (send, receive) = std::sync::mpsc::sync_channel(1);
        // Drain concurrently to keep a verbose refusal from blocking on a full pipe.
        // A descendant can hold the pipe open, so wait for its first line only until the deadline.
        if std::thread::Builder::new()
            .name("feedback-stderr".into())
            .spawn(move || {
                let mut line = Vec::new();
                let diagnostic = BufReader::new(stderr.by_ref().take((markers::STDERR_LIMIT + 4) as u64))
                    .read_until(b'\n', &mut line)
                    .ok()
                    .filter(|count| *count != 0)
                    .map(|_| {
                        let line = String::from_utf8_lossy(&line);
                        let line = line.trim_end_matches(['\r', '\n']);
                        line[..line.floor_char_boundary(markers::STDERR_LIMIT)].to_owned()
                    });
                let _ = send.send(diagnostic);
                let _ = std::io::copy(&mut stderr, &mut std::io::sink());
            })
            .is_err()
        {
            let _ = child.kill();
            let _ = child.wait();
            return Err(RecorderFailure::new(RecorderCause::SpawnFailed, None));
        }
        loop {
            match child.try_wait() {
                Ok(Some(status)) if status.success() => return Ok(()),
                Ok(Some(_)) => {
                    let stderr = receive
                        .recv_timeout(deadline.saturating_duration_since(Instant::now()))
                        .ok()
                        .flatten();
                    return Err(RecorderFailure::new(RecorderCause::Refused, stderr));
                }
                Ok(None) if Instant::now() < deadline => std::thread::sleep(Duration::from_millis(10)),
                result => {
                    let cause = if result.is_ok() {
                        RecorderCause::Timeout
                    } else {
                        RecorderCause::SpawnFailed
                    };
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err(RecorderFailure::new(cause, receive.try_recv().ok().flatten()));
                }
            }
        }
    }
}

/// The `--operation` words every installed `epr feedback capture` accepts. It refuses any
/// other word, so the argument keeps this closed vocabulary and the claim, carried in
/// `EPR_FEEDBACK_CLAIM`, says which command it was. Never add a word here: older recorders
/// would refuse it.
const RECORDER_OPERATIONS: &[&str] = &[
    "status",
    "log",
    "help",
    "version",
    "surface",
    "parse",
    "bridge",
    "guard",
    "build",
    "snapshot",
    "context",
    "integrate",
    "init",
    "clone",
    "fetch",
    "push",
    "pull",
    "add",
    "commit",
    "diff",
    "branch",
    "switch",
    "merge",
    "rebase",
    "restore",
    "tag",
    "remote",
    "submodule",
    "other",
];

fn recorder_operation(name: &str) -> &'static str {
    RECORDER_OPERATIONS
        .iter()
        .find(|word| **word == name)
        .copied()
        .unwrap_or("other")
}

struct RecorderFailure {
    cause: RecorderCause,
    stderr: Option<String>,
}

impl RecorderFailure {
    fn new(cause: RecorderCause, stderr: Option<String>) -> Self {
        Self { cause, stderr }
    }
}

fn gap_notice_key() -> String {
    // EPR accepts ELOHIM_SESSION_ID as a declaration. Claude and Codex also
    // export native session IDs. These values are only a local reporting hint.
    let session = [
        "ELOHIM_SESSION_ID",
        "CLAUDE_SESSION_ID",
        "CLAUDE_CODE_SESSION_ID",
        "CODEX_THREAD_ID",
        "CODEX_SESSION_ID",
    ]
    .into_iter()
    .filter_map(|name| std::env::var(name).ok())
    .find(|value| !value.trim().is_empty());
    // A declared invocation (EPR_FEEDBACK_INVOCATION, as `epr feedback` qualification sets per
    // probe) is its own reporting unit: each one shows its gap once, inside any session.
    let invocation = std::env::var("EPR_FEEDBACK_INVOCATION")
        .ok()
        .filter(|value| !value.trim().is_empty());
    if session.is_some() || invocation.is_some() {
        let mut hasher = gix::hash::hasher(gix::hash::Kind::Sha256);
        hasher.update(session.as_deref().unwrap_or_default().trim().as_bytes());
        let prefix = if let Some(invocation) = &invocation {
            hasher.update(b"\0");
            hasher.update(invocation.trim().as_bytes());
            "invocation"
        } else {
            "session"
        };
        if let Ok(hash) = hasher.try_finalize() {
            return format!("{prefix}-{hash}");
        }
    }
    // A UTC day keeps retries quiet across shells and platforms with no exported ID.
    let elapsed = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default();
    let day = elapsed.as_secs() / 86_400;
    format!("day-{day}")
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use std::{fs, os::unix::fs::PermissionsExt};

    use gix_testtools::TestResult;

    use super::*;

    #[test]
    fn successful_form_observes_an_injected_cure_once_per_session_and_emitter() -> TestResult {
        const CURES: &[claims::Cure] = &[claims::Cure {
            claim: "brit/capability-absent:cherry-pick.status",
            release: "0.3.12",
            state: claims::CureState::Cured,
            cite: "test note",
            summary: "Brit can report cherry-pick status on this platform.",
        }];
        if let Some(action) = std::env::var_os("BRIT_TEST_CURE_ACTION") {
            let mut capture = Capture::new();
            capture.cures = CURES;
            if action == "symptom" {
                capture.claim = claims::refusal(gitoxide::plumbing::FeedbackCode::CherryPickStatus);
                capture.finish(1);
                return Ok(());
            }
            let dir = std::env::var_os("BRIT_TEST_CURE_REPO").expect("fixture repository");
            let args = ["brit".into(), "-r".into(), dir, "cherry-pick".into(), action];
            let result = crate::run(args.into(), &mut capture, &mut false);
            let (code, _) = crate::exit_status(&result, false, false);
            let expected = std::env::var("BRIT_TEST_CURE_EXIT")?.parse::<i32>()?;
            assert_eq!(code, expected, "{result:?}");
            capture.finish(code);
            return Ok(());
        }

        let repo = gix_testtools::scripted_fixture_read_only("feedback-cure.sh")?;
        let temp = tempfile::tempdir()?;
        let root = temp.path();
        let recorder = root.join("epr");
        fs::write(
            &recorder,
            r#"#!/bin/sh
printf '%s %s %s %s\n' "$EPR_FEEDBACK_CLAIM" "$EPR_FEEDBACK_EMITTER" "$6" "$8" >> "$EPR_FEEDBACK_ROOT/calls"
"#,
        )?;
        fs::set_permissions(recorder, fs::Permissions::from_mode(0o700))?;
        let thread = std::thread::current();
        let name = thread.name().expect("named test thread");
        let run = |dir: &std::path::Path, action: &str, session: &str, emitter: &str, expected: i32| -> TestResult {
            let mut child = Command::new(std::env::current_exe()?);
            gix_testtools::configure_git_environment(&mut child, root);
            let out = child
                .current_dir(root)
                .args(["--exact", name, "--nocapture"])
                .env("BRIT_TEST_CURE_ACTION", action)
                .env("BRIT_TEST_CURE_REPO", dir)
                .env("BRIT_TEST_CURE_EXIT", expected.to_string())
                .env("PATH", root)
                .env("EPR_FEEDBACK_ROOT", root)
                .env("XDG_CACHE_HOME", root.join("cache"))
                .env("XDG_STATE_HOME", root.join("state"))
                .env("ELOHIM_SESSION_ID", session)
                .env("EPR_BIN_RECORD", emitter)
                .env_remove("EPR_FEEDBACK_INVOCATION")
                .output()?;
            assert!(out.status.success(), "{action}: {out:?}");
            Ok(())
        };
        run(root, "--status", "first", "release-a", 1)?;
        run(&repo, "--help", "first", "release-a", 0)?;
        let calls = root.join("calls");
        assert!(
            !fs::read_to_string(&calls)?.contains(CURES[0].claim),
            "failure and help do not observe the cure"
        );

        // A symptom recorded with the same session and emitter has its own marker.
        run(&repo, "symptom", "first", "release-a", 1)?;
        run(&repo, "--status", "first", "release-a", 0)?;
        run(&repo, "--status", "first", "release-a", 0)?;
        run(&repo, "--status", "second", "release-a", 0)?;
        run(&repo, "--status", "first", "release-b", 0)?;
        let calls = fs::read_to_string(calls)?;
        assert_eq!(
            calls
                .lines()
                .filter(|line| line.starts_with(CURES[0].claim))
                .collect::<Vec<_>>(),
            [
                "brit/capability-absent:cherry-pick.status release-a other failed",
                "brit/capability-absent:cherry-pick.status release-a other passed",
                "brit/capability-absent:cherry-pick.status release-a other passed",
                "brit/capability-absent:cherry-pick.status release-b other passed",
            ],
            "a prior symptom does not block the passage, which is recorded once for each session and emitter"
        );
        Ok(())
    }
}

#[cfg(test)]
mod operation_tests {
    use super::*;

    #[test]
    fn every_operation_word_is_one_installed_recorders_accept() {
        // The vocabulary of `epr feedback capture --operation` in every released epr. Changing
        // this list breaks capture on installed recorders; the claim carries anything finer.
        assert_eq!(
            RECORDER_OPERATIONS,
            [
                "status",
                "log",
                "help",
                "version",
                "surface",
                "parse",
                "bridge",
                "guard",
                "build",
                "snapshot",
                "context",
                "integrate",
                "init",
                "clone",
                "fetch",
                "push",
                "pull",
                "add",
                "commit",
                "diff",
                "branch",
                "switch",
                "merge",
                "rebase",
                "restore",
                "tag",
                "remote",
                "submodule",
                "other",
            ]
        );
        for row in claims::DECLARATIONS {
            let Some(verb) = row.id.strip_prefix("brit/command:") else {
                continue;
            };
            let mut capture = Capture::new();
            capture.operation(verb);
            assert!(
                RECORDER_OPERATIONS.contains(&capture.operation),
                "{} sends --operation {}",
                row.id,
                capture.operation
            );
            assert_eq!(capture.claim.id(), row.id, "the claim still names the command");
        }
        let mut capture = Capture::new();
        capture.operation("show");
        assert_eq!(
            (capture.operation, capture.claim.id()),
            ("other", "brit/command:show"),
            "a verb outside the recorder's list is sent as other"
        );
        capture.operation("no-such-verb");
        assert_eq!(capture.operation, "other");
    }
}
