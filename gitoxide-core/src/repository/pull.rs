//! Explicit fast-forward pull and private durable recovery.
use anyhow::Result;
use gix::bstr::BStr;

/// Fetch one explicit branch and integrate only a proven fast-forward.
pub fn execute(repo: gix::Repository, remote: &str, branch: &BStr) -> Result<()> {
    #[cfg(target_os = "linux")]
    {
        super::switch::pull_execute(repo, remote, branch)
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = (repo, remote, branch);
        anyhow::bail!("durable pull requires Linux file and directory synchronization")
    }
}
/// Inspect either operation without automatically recovering it.
pub fn status(repo: &gix::Repository, out: &mut dyn std::io::Write) -> Result<()> {
    super::switch::status(repo, out)
}
/// Explicit recovery never repeats a hook whose start was recorded.
pub fn recover(repo: gix::Repository, abort: bool) -> Result<()> {
    #[cfg(target_os = "linux")]
    {
        super::switch::pull_recover(repo, abort)
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = (repo, abort);
        anyhow::bail!("durable pull recovery requires Linux")
    }
}
