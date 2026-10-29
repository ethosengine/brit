//! Read-only inspection of the repository signing identity.
//!
//! `signer` loads the AgentKey writers would use and never mints one. Only a key whose directory
//! entry does not exist reads as absence; a dangling symlink, a non-file, an unreadable file or a
//! seed of the wrong length is an error. Neither the key path nor seed bytes are ever printed.

use std::{io::ErrorKind, path::Path};

use anyhow::Context;
use brit_epr::engine::signing::{AgentKey, AgentKeyError};

const LOAD_FAILED: &str = "cannot load the repository signer key";

pub fn show(repo: &Path, json: bool) -> anyhow::Result<()> {
    let key_path = crate::git_dir::agent_key_path(repo)?;
    let agent_id = match AgentKey::load(&key_path) {
        Ok(key) => Some(key.agent_id()),
        // A read's `NotFound` also covers a dangling symlink: only a missing entry is absence.
        Err(AgentKeyError::Io(err)) if err.kind() == ErrorKind::NotFound => {
            match std::fs::symlink_metadata(&key_path) {
                Err(metadata_err) if metadata_err.kind() == ErrorKind::NotFound => None,
                // The entry exists (a dangling link, or a key published since the first read):
                // whatever a load says now is the answer.
                Ok(_) => Some(AgentKey::load(&key_path).context(LOAD_FAILED)?.agent_id()),
                Err(metadata_err) => return Err(AgentKeyError::Io(metadata_err)).context(LOAD_FAILED),
            }
        }
        Err(err) => return Err(err).context(LOAD_FAILED),
    };

    if json {
        println!("{}", serde_json::json!({ "agentId": agent_id }));
    } else if let Some(agent_id) = agent_id {
        println!("{agent_id}");
    } else {
        println!("none");
    }
    Ok(())
}
