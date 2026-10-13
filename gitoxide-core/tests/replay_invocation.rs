//! Real supervisor SIGINT while a hook leaves a pipe-sized input unread.
#![cfg(target_os = "linux")]

use anyhow::{Context, Result, ensure};
use gitoxide_core::repository::authoring::{
    invocation::{InvocationOutcome, inspect, prepare},
    run_hook_in_context,
};
use std::{
    fs,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    process::Command,
    time::{Duration, Instant},
};

fn launched_pid(directory: &Path) -> Result<i32> {
    let envelope: serde_json::Value = serde_json::from_slice(&fs::read(directory.join("launch.json"))?)?;
    let payload: Vec<u8> = serde_json::from_value(envelope["payload"].clone())?;
    let value: serde_json::Value = serde_json::from_slice(&payload)?;
    Ok(i32::try_from(value["pid"].as_u64().context("launch PID")?)?)
}

#[test]
fn blocked_input_supervisor_helper() -> Result<()> {
    let Some(root) = std::env::var_os("BRIT_INVOCATION_INPUT_TEST") else {
        return Ok(());
    };
    let root = PathBuf::from(root);
    #[expect(unsafe_code)]
    unsafe {
        // SAFETY: The CLI uses the same handler; this callback performs no work.
        gix::interrupt::init_handler(1, || {})?;
    }
    let repo = gix::open(&root)?;
    let error = run_hook_in_context(
        &repo,
        "post-rewrite",
        &[
            root.join("launched").into_os_string(),
            std::env::var_os("BRIT_INVOCATION_INPUT_MODE").context("test mode")?,
        ],
        Some(&vec![b'x'; 1024 * 1024]),
        &repo.index_path(),
        &root.join("invocation"),
    )
    .expect_err("interrupted input delivery cannot report a known hook outcome");
    ensure!(
        error.to_string().contains("UNKNOWN"),
        "unexpected interruption: {error:#}"
    );
    fs::write(root.join("returned"), error.to_string())?;
    Ok(())
}

#[test]
fn large_unread_hook_input_interrupt_is_bounded_and_never_retried() -> Result<()> {
    use nix::{
        sys::signal::{Signal, kill, killpg},
        unistd::Pid,
    };
    for mode in ["cooperative", "ignore"] {
        let root = tempfile::tempdir()?;
        gix_testtools::git(root.path(), "init -b main").map_err(|error| anyhow::anyhow!("{error}"))?;
        let hook = root.path().join(".git/hooks/post-rewrite");
        fs::write(&hook, b"#!/usr/bin/env python3\nimport pathlib, signal, sys, time\nif sys.argv[2] == 'ignore':\n    signal.signal(signal.SIGINT, signal.SIG_IGN)\nwith pathlib.Path(sys.argv[1]).open('ab') as launched:\n    launched.write(b'launched once\\n')\npathlib.Path(sys.argv[1]).with_name('ready').write_bytes(b'ready')\nwhile True:\n    time.sleep(60)\n")?;
        fs::set_permissions(&hook, fs::Permissions::from_mode(0o700))?;
        let mut supervisor = Command::new(std::env::current_exe()?)
            .args(["--exact", "blocked_input_supervisor_helper", "--nocapture"])
            .env("BRIT_INVOCATION_INPUT_TEST", root.path())
            .env("BRIT_INVOCATION_INPUT_MODE", mode)
            .spawn()?;
        struct StopGroup<'a> {
            supervisor: &'a mut std::process::Child,
            directory: PathBuf,
        }
        impl Drop for StopGroup<'_> {
            fn drop(&mut self) {
                if let Ok(pid) = launched_pid(&self.directory) {
                    let _ =
                        nix::sys::signal::killpg(nix::unistd::Pid::from_raw(pid), nix::sys::signal::Signal::SIGKILL);
                }
                let _ = self.supervisor.kill();
                let _ = self.supervisor.wait();
            }
        }
        let directory = root.path().join("invocation");
        let guard = StopGroup {
            supervisor: &mut supervisor,
            directory: directory.clone(),
        };
        let deadline = Instant::now() + Duration::from_secs(15);
        while !root.path().join("ready").exists() {
            ensure!(
                guard.supervisor.try_wait()?.is_none(),
                "supervisor exited before hook launch ({mode})"
            );
            ensure!(Instant::now() < deadline, "hook never launched ({mode})");
            std::thread::sleep(Duration::from_millis(10));
        }
        // The hook has crossed the durable launch barrier and never reads stdin;
        // 1MiB exceeds the pipe capacity, leaving the supervisor in delivery.
        let started = Instant::now();
        kill(Pid::from_raw(i32::try_from(guard.supervisor.id())?), Signal::SIGINT)?;
        let status = loop {
            if let Some(status) = guard.supervisor.try_wait()? {
                break status;
            }
            ensure!(
                started.elapsed() < Duration::from_secs(12),
                "supervisor did not bound interruption ({mode})"
            );
            std::thread::sleep(Duration::from_millis(10));
        };
        ensure!(status.success(), "helper failed ({mode}): {status}");
        ensure!(fs::read_to_string(root.path().join("returned"))?.contains("UNKNOWN"));
        assert!(!directory.join("exit.json").exists());
        if mode == "ignore" {
            assert!(
                started.elapsed() >= Duration::from_secs(4),
                "uncooperative group must reach the bounded wait"
            );
            assert!(
                inspect(&directory).is_err(),
                "a live uncooperative group must block recovery"
            );
            killpg(Pid::from_raw(launched_pid(&directory)?), Signal::SIGKILL)?;
        } else {
            assert!(
                started.elapsed() < Duration::from_secs(4),
                "cooperative cancellation must not wait for the timeout"
            );
            assert_eq!(
                inspect(&directory)?,
                InvocationOutcome::Unknown,
                "cooperative child is quiescent when supervisor returns"
            );
        }
        let deadline = Instant::now() + Duration::from_secs(5);
        while inspect(&directory).is_err() {
            ensure!(Instant::now() < deadline, "owned group did not become quiescent");
            std::thread::sleep(Duration::from_millis(10));
        }
        let evidence = fs::read(root.path().join("launched"))?;
        assert_eq!(evidence, b"launched once\n");
        for _ in 0..2 {
            assert_eq!(inspect(&directory)?, InvocationOutcome::Unknown);
            assert_eq!(fs::read(root.path().join("launched"))?, evidence);
            assert!(
                prepare(&directory).is_err(),
                "the same invocation cannot be launched again"
            );
        }
        drop(guard);
    }
    Ok(())
}
