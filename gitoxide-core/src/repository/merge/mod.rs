mod file;
pub use file::file;

pub mod tree;
pub use tree::function::tree;

mod commit;
pub use commit::commit;

/// Fast-forward the checked-out branch to a local revision.
pub fn ff_only(repo: gix::Repository, revision: &gix::bstr::BStr) -> anyhow::Result<()> {
    #[cfg(target_os = "linux")]
    {
        super::switch::merge_ff_only(repo, revision)
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = repo;
        anyhow::bail!("durable merge requires Linux; run git merge --ff-only -- {revision}")
    }
}
