//! BritInvocation — process invocation + capture + optional normalization.

use std::{
    ffi::OsString,
    path::{Path, PathBuf},
    process::{Command, ExitStatus},
};

use anyhow::{anyhow, Context, Result};

use crate::support::normalize::Normalizer;

pub struct BritInvocation {
    program: PathBuf,
    args: Vec<OsString>,
    env: Vec<(OsString, OsString)>,
    cwd: Option<PathBuf>,
    normalize: bool,
}

pub struct Capture {
    pub stdout: String,
    pub stderr: String,
    pub status: ExitStatus,
}

/// Resolve the one required `brit` binary for journey tests.
pub fn brit_bin() -> Option<PathBuf> {
    let path = std::env::var_os("BRIT_BIN").map(PathBuf::from).unwrap_or_else(|| {
        profile_binary(
            &std::env::current_exe().expect("current test executable"),
            "brit",
            std::env::consts::EXE_SUFFIX,
        )
    });
    Some(path.canonicalize().unwrap_or_else(|_| {
        panic!(
            "brit binary missing at {}; build `cargo build -p brit-cli --bin brit` or set BRIT_BIN",
            path.display()
        )
    }))
}

fn profile_binary(test_executable: &Path, name: &str, suffix: &str) -> PathBuf {
    test_executable
        .parent()
        .and_then(|deps| deps.parent())
        .expect("Cargo target profile directory")
        .join(format!("{name}{suffix}"))
}

impl BritInvocation {
    pub fn new<P: Into<PathBuf>>(program: P) -> Self {
        Self {
            program: program.into(),
            args: Vec::new(),
            env: Vec::new(),
            cwd: None,
            normalize: false,
        }
    }

    pub fn arg<A: Into<OsString>>(mut self, arg: A) -> Self {
        self.args.push(arg.into());
        self
    }

    pub fn args<I, A>(mut self, args: I) -> Self
    where
        I: IntoIterator<Item = A>,
        A: Into<OsString>,
    {
        self.args.extend(args.into_iter().map(Into::into));
        self
    }

    pub fn env<K, V>(mut self, key: K, value: V) -> Self
    where
        K: Into<OsString>,
        V: Into<OsString>,
    {
        self.env.push((key.into(), value.into()));
        self
    }

    pub fn current_dir<P: Into<PathBuf>>(mut self, cwd: P) -> Self {
        self.cwd = Some(cwd.into());
        self
    }

    /// Apply the default Normalizer to captured stdout/stderr.
    pub fn normalize(mut self, on: bool) -> Self {
        self.normalize = on;
        self
    }

    pub fn run(self) -> Result<Capture> {
        let mut cmd = Command::new(&self.program);
        let config_dir = tempfile::tempdir().context("create isolated Git configuration directory")?;
        gix_testtools::configure_git_environment(&mut cmd, self.cwd.as_deref().unwrap_or(config_dir.path()));
        cmd.args(&self.args);
        for (k, v) in &self.env {
            cmd.env(k, v);
        }
        if let Some(cwd) = &self.cwd {
            cmd.current_dir(cwd);
        }
        let out = cmd.output().with_context(|| format!("invoke {:?}", self.program))?;
        let mut stdout = String::from_utf8(out.stdout).map_err(|e| anyhow!("non-utf8 stdout: {e}"))?;
        let mut stderr = String::from_utf8(out.stderr).map_err(|e| anyhow!("non-utf8 stderr: {e}"))?;
        if self.normalize {
            let n = Normalizer::new();
            stdout = n.normalize(&stdout);
            stderr = n.normalize(&stderr);
        }
        Ok(Capture {
            stdout,
            stderr,
            status: out.status,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn profile_binary_uses_the_platform_executable_suffix() {
        let test_executable = Path::new("target").join("debug").join("deps").join("journey-test.exe");
        assert_eq!(
            profile_binary(&test_executable, "brit", ".exe"),
            Path::new("target").join("debug").join("brit.exe")
        );
        assert_eq!(
            profile_binary(&test_executable, "brit", ""),
            Path::new("target").join("debug").join("brit")
        );
    }
}
