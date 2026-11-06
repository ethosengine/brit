//! `covenant` subcommand — put/list/show session covenant clauses.
//!
//! brit carries clauses; it does not judge them (see the session-covenant spec).

use std::path::Path;

use brit_epr::{
    elohim::{
        attestation::{
            covenant::{validate_payload, CovenantClause, CovenantClauseBody, CovenantClauseKind, CovenantRecord},
            note::EnvelopeNote,
        },
        refs::{BritRefManager, Lookup},
    },
    engine::{cid::BritCid, object_store::LocalObjectStore, signing::AgentKey},
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

    let body = CovenantClauseBody {
        kind,
        session: args.session.to_string(),
        party: args.party.to_string(),
        quote: args.quote.to_string(),
        payload,
        valid_from: chrono::Utc::now().to_rfc3339(),
        valid_until: args.valid_until.map(str::to_string),
        supersedes,
        agent_id: agent_key.agent_id(),
    };
    // New clauses are written in the envelope format (`brit.covenant@2`).
    let note = EnvelopeNote::sign(body, &agent_key)?;

    store.put(&note)?;
    let clause = CovenantClause::Envelope(note);
    let view = clause.view()?;
    let cid = refs.put_covenant_clause(clause)?;

    if args.output_json {
        println!(
            "{}",
            serde_json::to_string(&serde_json::json!({ "cid": cid, "clause": view }))?
        );
    } else {
        println!("{cid}");
    }
    Ok(())
}

/// A record as tools print it: `{cid, clause}` with the clause's view (a frozen clause as stored;
/// an envelope clause as its body plus `epoch`, `scheme`, `key`, `sig`).
fn record_row(record: &CovenantRecord) -> serde_json::Result<serde_json::Value> {
    Ok(serde_json::json!({ "cid": record.cid, "clause": record.clause.view()? }))
}

/// The session's verified clauses in validFrom order; each record that failed verification is
/// dropped and named on stderr as `unverifiable: <ref> <reason>`, each held record (an epoch or
/// scheme this reader does not implement) as `held: <ref> <reason>`.
pub fn list(repo: &Path, session: &str, output_json: bool) -> anyhow::Result<()> {
    let refs = BritRefManager::new(repo)?;
    let scan = refs.list_covenant_clauses(session)?;
    if output_json {
        let rows = scan
            .verified
            .iter()
            .map(|(_, record)| record_row(record))
            .collect::<Result<Vec<_>, _>>()?;
        println!("{}", serde_json::to_string(&rows)?);
    } else if scan.verified.is_empty() {
        println!("[]");
    } else {
        for (_, r) in &scan.verified {
            let body = r.clause.body();
            println!("{} {} {} {}", r.cid, body.valid_from, body.kind.as_str(), body.party);
        }
    }
    for (at, reason) in &scan.unverifiable {
        crate::lookup_out::unverifiable(at, reason);
    }
    for (at, reason) in &scan.held {
        crate::lookup_out::held(at, *reason);
    }
    Ok(())
}

/// One verified clause by CID; absent, unverifiable or held is an error.
pub fn show(repo: &Path, cid: &str, output_json: bool) -> anyhow::Result<()> {
    let refs = BritRefManager::new(repo)?;
    let row = match refs.get_covenant_clause(cid)? {
        Lookup::Verified(record) => record_row(&record)?,
        Lookup::Absent => anyhow::bail!("no covenant clause with cid {cid}"),
        Lookup::Unverifiable { at, reason } => {
            anyhow::bail!("no covenant clause with cid {cid}: unverifiable: {at} {reason}")
        }
        Lookup::Held { at, reason } => anyhow::bail!("no covenant clause with cid {cid}: held: {at} {reason}"),
    };
    if output_json {
        println!("{}", serde_json::to_string(&row)?);
    } else {
        println!("{}", serde_json::to_string_pretty(&row)?);
    }
    Ok(())
}
