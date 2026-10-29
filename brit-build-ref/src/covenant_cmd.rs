//! `covenant` subcommand — put/list/show session covenant clauses.
//!
//! brit carries clauses; it does not judge them (see the session-covenant spec).

use std::path::Path;

use brit_epr::{
    elohim::{
        attestation::covenant::{validate_payload, CovenantClauseContentNode, CovenantClauseKind},
        refs::BritRefManager,
    },
    engine::{cid::BritCid, content_node::ContentNode, object_store::LocalObjectStore, signing::AgentKey},
};

use crate::git_dir::{agent_key_path, common_git_dir};

pub struct PutArgs<'a> {
    pub session: &'a str,
    pub kind: &'a str,
    pub party: &'a str,
    pub quote: &'a str,
    pub payload_json: &'a str,
    pub valid_until: Option<&'a str>,
    pub supersedes: Option<&'a str>,
    pub output_json: bool,
}

pub fn put(repo: &Path, args: &PutArgs<'_>) -> anyhow::Result<()> {
    let kind: CovenantClauseKind = args.kind.parse().map_err(|e: String| anyhow::anyhow!(e))?;
    let payload: serde_json::Value =
        serde_json::from_str(args.payload_json).map_err(|e| anyhow::anyhow!("invalid --payload-json: {e}"))?;
    validate_payload(kind, &payload).map_err(|e| anyhow::anyhow!(e))?;
    let supersedes: Option<BritCid> = args
        .supersedes
        .map(|s| {
            s.parse()
                .map_err(|e| anyhow::anyhow!("invalid --supersedes CID: {e:?}"))
        })
        .transpose()?;
    if let Some(until) = args.valid_until {
        chrono::DateTime::parse_from_rfc3339(until)
            .map_err(|e| anyhow::anyhow!("invalid --valid-until (ISO-8601 expected): {e}"))?;
    }

    let refs = BritRefManager::new(repo)?;
    let agent_key = AgentKey::load_or_generate(&agent_key_path(repo)?)?;
    let store = LocalObjectStore::for_git_dir(&common_git_dir(repo)?);

    let mut node = CovenantClauseContentNode {
        kind,
        session: args.session.to_string(),
        party: args.party.to_string(),
        quote: args.quote.to_string(),
        payload,
        valid_from: chrono::Utc::now().to_rfc3339(),
        valid_until: args.valid_until.map(str::to_string),
        supersedes,
        agent_id: agent_key.agent_id(),
        signature: String::new(),
    };
    let canonical = node.canonical_bytes()?;
    node.signature = agent_key.sign(&canonical);

    let cid = store.put(&node)?.to_string();
    let clause = serde_json::to_value(&node)?;
    refs.put_covenant_clause(args.session, &cid, &clause)?;

    if args.output_json {
        println!(
            "{}",
            serde_json::to_string(&serde_json::json!({ "cid": cid, "clause": clause }))?
        );
    } else {
        println!("{cid}");
    }
    Ok(())
}

pub fn list(repo: &Path, session: &str, output_json: bool) -> anyhow::Result<()> {
    let refs = BritRefManager::new(repo)?;
    let rows = refs.list_covenant_clauses(session)?;
    if output_json {
        println!("{}", serde_json::to_string(&rows)?);
    } else if rows.is_empty() {
        println!("[]");
    } else {
        for r in &rows {
            println!(
                "{} {} {} {}",
                r["cid"].as_str().unwrap_or(""),
                r["clause"]["validFrom"].as_str().unwrap_or(""),
                r["clause"]["kind"].as_str().unwrap_or(""),
                r["clause"]["party"].as_str().unwrap_or("")
            );
        }
    }
    Ok(())
}

pub fn show(repo: &Path, cid: &str, output_json: bool) -> anyhow::Result<()> {
    let refs = BritRefManager::new(repo)?;
    let Some(row) = refs.get_covenant_clause(cid)? else {
        anyhow::bail!("no covenant clause with cid {cid}");
    };
    if output_json {
        println!("{}", serde_json::to_string(&row)?);
    } else {
        println!("{}", serde_json::to_string_pretty(&row)?);
    }
    Ok(())
}
