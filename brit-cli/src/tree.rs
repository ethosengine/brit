//! Explicit local tree operations. Storage, codecs and host projection belong to EPRFS;
//! Git object translation belongs to brit-bridge. None of these operations publish.

use std::path::PathBuf;

use anyhow::{Context, Result};
use brit_bridge::gix;
use clap::Subcommand;
use eprfs_core::{BlobCid, FetchPolicy, TreeLimits};
use eprfs_storage::DirectoryStorage;

#[derive(Subcommand)]
pub enum TreeCommand {
    /// Seal a committed Git tree into an explicitly selected local content store.
    Seal {
        #[arg(long, default_value = ".")]
        repo: PathBuf,
        /// HEAD, a simple ref name, or a full Git object ID.
        #[arg(long, default_value = "HEAD")]
        revision: String,
        #[arg(long)]
        store: PathBuf,
    },
    /// Verify locally available content; external boundaries are reported, not fetched.
    Verify {
        #[arg(long)]
        root: String,
        #[arg(long)]
        store: PathBuf,
    },
    /// Restore an exact tree to a NEW directory. Refuses external boundaries and overwrites.
    Restore {
        #[arg(long)]
        root: String,
        #[arg(long)]
        store: PathBuf,
        #[arg(long)]
        destination: PathBuf,
    },
    /// Recreate Git objects, checking the original tree OID. Does not move refs or the index.
    ExportGit {
        #[arg(long)]
        root: String,
        #[arg(long)]
        store: PathBuf,
        #[arg(long)]
        repo: PathBuf,
        #[arg(long)]
        expected_tree: String,
    },
}

pub fn run(command: TreeCommand) -> Result<()> {
    tokio::runtime::Builder::new_current_thread()
        .build()
        .context("create local tree runtime")?
        .block_on(run_async(command))
}

async fn run_async(command: TreeCommand) -> Result<()> {
    let limits = TreeLimits::default();
    match command {
        TreeCommand::Seal { repo, revision, store } => {
            let repository = gix::open(repo).context("open source repository")?;
            let tree_id = brit_bridge::resolve_tree_revision(&repository, &revision, &limits)?;
            let storage = DirectoryStorage::create(store)?;
            let sealed = brit_bridge::seal_git_tree(&repository, tree_id, &storage, &limits).await?;
            crate::output::print_json(&serde_json::json!({
                "root": sealed.root.to_string(),
                "gitTree": tree_id.to_string(),
                "published": false,
            }))?;
        }
        TreeCommand::Verify { root, store } => {
            let root = BlobCid::parse(&root).context("parse tree CID")?;
            let storage = DirectoryStorage::open(store)?;
            let verified = eprfs_core::load_verified_tree(&storage, &root, FetchPolicy::LocalOnly, &limits).await?;
            crate::output::print_json(&serde_json::json!({
                "root": root.to_string(),
                "localClosureVerified": true,
                "externalContentIncluded": verified.external_count() == 0,
                "externalBoundaries": verified.external_count(),
                "authorityVerified": false,
            }))?;
        }
        TreeCommand::Restore {
            root,
            store,
            destination,
        } => {
            let root = BlobCid::parse(&root).context("parse tree CID")?;
            let storage = DirectoryStorage::open(store)?;
            let report = eprfs_local::restore_exact_tree(
                &storage,
                &root,
                &destination,
                FetchPolicy::LocalOnly,
                &limits,
                &eprfs_host::HostProfile::current_platform(),
            )
            .await?;
            crate::output::print_json(&serde_json::json!({
                "root": root.to_string(), "destination": destination,
                "files": report.files, "directories": report.directories,
                "symlinks": report.symlinks, "bytesWritten": report.bytes_written,
            }))?;
        }
        TreeCommand::ExportGit {
            root,
            store,
            repo,
            expected_tree,
        } => {
            let root = BlobCid::parse(&root).context("parse tree CID")?;
            let expected = gix::ObjectId::from_hex(expected_tree.as_bytes()).context("parse expected Git tree OID")?;
            let repository = gix::open(repo).context("open destination Git object database")?;
            let storage = DirectoryStorage::open(store)?;
            let actual = brit_bridge::restore_git_tree(&repository, &root, expected, &storage, &limits).await?;
            crate::output::print_json(&serde_json::json!({
                "root": root.to_string(), "gitTree": actual.to_string(), "refsChanged": false,
            }))?;
        }
    }
    Ok(())
}
