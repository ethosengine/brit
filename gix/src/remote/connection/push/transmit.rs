use std::{ops::Deref, sync::atomic::AtomicBool};

use gix_transport::client::blocking_io::Transport;

use super::{Error, Prepare};
use crate::remote::push::Outcome;

impl<T> Prepare<'_, '_, T>
where
    T: Transport,
{
    /// Execute the push, driving the following pipeline:
    ///
    /// 1. Parse refspecs.
    /// 2. Enumerate local refs.
    /// 3. Match via [`gix_refspec::MatchGroup::match_push`].
    /// 4. Resolve local names to `ObjectId`s; look up old OIDs from the handshake advertisement.
    /// 5. Build [`gix_protocol::send_pack::Request`].
    /// 6. Enumerate objects to pack (commits reachable from new_oid, not from old_oid).
    /// 7. If dry_run, synthesise an outcome and return early.
    /// 8. Call [`gix_protocol::send_pack::send_pack`].
    /// 9. Map the server report into a porcelain `Outcome`.
    ///
    /// This method is only available when the `blocking-network-client` feature is enabled
    /// (the entire `push` module is gated behind that feature).
    pub fn transmit(
        mut self,
        _progress: impl gix_features::progress::NestedProgress,
        should_interrupt: &AtomicBool,
    ) -> Result<Outcome, Error> {
        use crate::bstr::BString;
        use gix_refspec::{
            match_group::{Item, SourceRef},
            parse::Operation,
            MatchGroup,
        };

        if self.refspecs.is_empty() {
            return Err(Error::NoRefspecs);
        }

        // ── Step 1: Parse refspecs ────────────────────────────────────────────
        let parsed_refspecs: Vec<gix_refspec::RefSpec> = self
            .refspecs
            .iter()
            .map(|s| gix_refspec::parse(s.as_slice().into(), Operation::Push).map(|r| r.to_owned()))
            .collect::<Result<_, _>>()?;

        // ── Step 2: Enumerate local refs ──────────────────────────────────────
        let repo = self.connection.remote.repo;
        let hash_kind = repo.object_hash();

        let local_refs: Vec<crate::Reference<'_>> = repo.references()?.all()?.filter_map(Result::ok).collect();

        // ── Step 3: MatchGroup::match_push ────────────────────────────────────
        // Build `Item` from direct (non-symbolic) local refs only.
        // We need to own the ObjectIds separately so `item.target` can borrow them.
        let item_ids: Vec<(gix_hash::ObjectId, &crate::Reference<'_>)> = local_refs
            .iter()
            .filter_map(|r| r.try_id().map(|id| (id.detach(), r)))
            .collect();
        let items: Vec<Item<'_>> = item_ids
            .iter()
            .map(|(id, r)| Item {
                full_ref_name: r.name().as_bstr(),
                target: id.as_ref(),
                object: None,
            })
            .collect();

        let group = MatchGroup::from_push_specs(parsed_refspecs.iter().map(|s| s.to_ref()));
        let (push_outcome, _fixes) = group.match_push(items.iter().copied()).validated()?;
        let mappings = push_outcome.mappings;

        if mappings.is_empty() {
            let spec = self.refspecs.into_iter().next().expect("non-empty by guard above");
            return Err(Error::NoMatch { spec });
        }

        // ── Step 4: Resolve OIDs + look up remote's current state ─────────────
        let remote_refs: &[gix_protocol::handshake::Ref] = self
            .connection
            .handshake
            .as_ref()
            .and_then(|h| h.refs.as_deref())
            .unwrap_or(&[]);

        let null_oid = hash_kind.null();

        // Map remote ref name → current ObjectId for O(1) lookup below.
        let remote_oid_by_name: std::collections::HashMap<BString, gix_hash::ObjectId> = {
            use gix_protocol::handshake::Ref;
            remote_refs
                .iter()
                .filter_map(|r| {
                    let (name, oid) = match r {
                        Ref::Direct { full_ref_name, object } => (full_ref_name.clone(), *object),
                        Ref::Peeled { full_ref_name, tag, .. } => (full_ref_name.clone(), *tag),
                        Ref::Symbolic {
                            full_ref_name, object, ..
                        } => (full_ref_name.clone(), *object),
                        Ref::Unborn { .. } => return None,
                    };
                    Some((name, oid))
                })
                .collect()
        };

        let mut commands: Vec<gix_protocol::send_pack::Command> = Vec::with_capacity(mappings.len());
        // Parallel vec: (local_name, remote_name) for populating RefStatus.
        let mut name_pairs: Vec<(BString, BString)> = Vec::with_capacity(mappings.len());

        for mapping in &mappings {
            let remote_name: BString = match &mapping.rhs {
                Some(rhs) => rhs.as_ref().into(),
                None => continue, // push spec without a destination — skip
            };

            let (local_name, new_oid) = match &mapping.lhs {
                SourceRef::FullName(name) if !name.is_empty() => {
                    // Resolve the local ref to its peeled object ID.
                    let mut reference = repo
                        .find_reference(name.as_ref())
                        .map_err(|e| Error::Resolve(Box::new(e)))?;
                    let resolved = reference.peel_to_id().map_err(|e| Error::Resolve(Box::new(e)))?;
                    (BString::from(name.as_ref()), resolved.detach())
                }
                SourceRef::FullName(_empty) => {
                    // Delete spec — new_oid is zero (signals deletion to the server).
                    (BString::default(), null_oid)
                }
                SourceRef::ObjectId(id) => {
                    // Literal OID spec (e.g. `<sha>:refs/heads/main`).
                    (BString::from(id.to_string().as_bytes()), *id)
                }
            };

            let old_oid = remote_oid_by_name.get(&remote_name).copied().unwrap_or(null_oid);

            commands.push(gix_protocol::send_pack::Command {
                refname: remote_name.clone(),
                old_oid,
                new_oid,
            });
            name_pairs.push((local_name, remote_name));
        }

        if commands.is_empty() {
            return Err(Error::NoRefspecs);
        }

        // ── Step 5: Build the Request ─────────────────────────────────────────
        let (agent_key, agent_val) = repo.config.user_agent_tuple();
        let agent_cap: BString = match agent_val {
            Some(v) => format!("{agent_key}={v}").into(),
            None => BString::from(agent_key.as_bytes()),
        };
        let capabilities: Vec<BString> = vec![
            b"report-status".as_slice().into(),
            b"side-band-64k".as_slice().into(),
            b"ofs-delta".as_slice().into(),
            agent_cap,
        ];

        let request = gix_protocol::send_pack::Request {
            commands: commands.clone(),
            capabilities,
        };

        // ── Step 6: Enumerate objects to pack ────────────────────────────────
        //
        // APPROACH: use `objects_unthreaded` with `TreeAdditionsComparedToAncestor`
        // to walk commits reachable from new_oid but not from old_oid, and expand
        // each commit to the tree + blob objects it introduces.  Entries are produced
        // by `Entry::from_data` which re-compresses objects as base entries.
        //
        // NOTE: This is correct but not maximally efficient — re-compression is
        // more expensive than pack-copying.  A follow-up task should switch to
        // `iter_from_counts` + `InOrderIter` for pack-copy mode.  See Task 7.1 plan.
        let entries = build_pack_entries(repo, &commands, hash_kind, should_interrupt)?;

        // ── Step 7: Dry run early return ─────────────────────────────────────
        if self.dry_run {
            let status = name_pairs
                .into_iter()
                .map(|(local, remote)| crate::remote::push::RefStatus {
                    local,
                    remote,
                    result: Ok(()),
                })
                .collect();
            return Ok(Outcome { status });
        }

        // ── Step 8: Call send_pack ────────────────────────────────────────────
        let protocol_outcome = gix_protocol::send_pack::send_pack(
            self.connection.transport_mut(),
            request,
            entries,
            gix_protocol::send_pack::Options::default(),
            hash_kind,
        )?;

        // ── Step 9: Map to porcelain Outcome ─────────────────────────────────
        let status = protocol_outcome
            .report
            .refs
            .into_iter()
            .map(|r| {
                let local = name_pairs
                    .iter()
                    .find(|(_, remote)| remote == &r.refname)
                    .map(|(local, _)| local.clone())
                    .unwrap_or_default();
                crate::remote::push::RefStatus {
                    local,
                    remote: r.refname,
                    result: r.result,
                }
            })
            .collect();
        Ok(Outcome { status })
    }
}

/// Collect `gix_pack::data::output::Entry` items for all objects reachable from
/// non-delete commands' `new_oid` but not already present via `old_oid`.
///
/// # Design note
///
/// Uses `objects_unthreaded` + `Entry::from_data` (decompress + recompress each
/// object as a base entry).  This is correct but not the fastest approach.
/// A follow-up should switch to `iter_from_counts` + pack-copy to avoid
/// re-compressing objects that already exist in the local pack store.
fn build_pack_entries(
    repo: &crate::Repository,
    commands: &[gix_protocol::send_pack::Command],
    _hash_kind: gix_hash::Kind,
    should_interrupt: &AtomicBool,
) -> Result<Vec<gix_pack::data::output::Entry>, Error> {
    use gix_pack::data::output::{count::objects::ObjectExpansion, Entry};
    use gix_traverse::commit::Simple;

    let tips: Vec<gix_hash::ObjectId> = commands
        .iter()
        .filter(|c| !c.is_delete() && !c.new_oid.is_null())
        .map(|c| c.new_oid)
        .collect();

    if tips.is_empty() {
        // Pure deletion push — no objects needed in the pack.
        return Ok(Vec::new());
    }

    let hides: Vec<gix_hash::ObjectId> = commands
        .iter()
        .filter(|c| !c.old_oid.is_null())
        .map(|c| c.old_oid)
        .collect();

    // Walk commits reachable from tips but hidden behind hides.
    // `Simple::new` + `.hide()` encodes "new_oid reachable minus old_oid reachable".
    //
    // `Simple::new` requires `gix_object::Find`; `repo.objects` implements that
    // via `Proxy<T>: gix_object::Find`.
    let mut commit_ids: Vec<gix_hash::ObjectId> = Simple::new(tips.iter().copied(), &repo.objects)
        .hide(hides)
        .map_err(|e| Error::Walk(e.to_string()))?
        .filter_map(Result::ok)
        .map(|info| info.id)
        .collect();

    // Include the tip commits themselves (`.hide()` seeds them; they may not be
    // re-emitted by the iterator when old_oid is null and no ancestors exist).
    for &tip in &tips {
        if !commit_ids.contains(&tip) {
            commit_ids.push(tip);
        }
    }

    if commit_ids.is_empty() {
        return Ok(Vec::new());
    }

    // `objects_unthreaded` requires `&dyn gix_pack::Find`.
    // `repo.objects` is `Proxy<Cache<Handle<Arc<Store>>>>`.
    // `Proxy<T>` implements `Deref<Target = T>`, and `Cache<S>` implements
    // `gix_pack::Find` when `S` implements it.  So `&*repo.objects` gives us
    // a `&Cache<Handle<Arc<Store>>>` which implements `gix_pack::Find`.
    let odb_pack: &dyn gix_pack::Find = repo.objects.deref();

    let progress_discard = gix_features::progress::Discard;
    let (counts, _) = gix_pack::data::output::count::objects_unthreaded(
        odb_pack,
        &mut commit_ids
            .iter()
            .map(|id| Ok::<_, Box<dyn std::error::Error + Send + Sync + 'static>>(*id)),
        &progress_discard,
        should_interrupt,
        ObjectExpansion::TreeAdditionsComparedToAncestor,
    )
    .map_err(|e| Error::Pack(e.to_string()))?;

    // Convert Count → Entry by reading each object from the ODB and encoding it
    // as a base (non-delta) entry.  We use `gix_pack::Find::try_find` here
    // (not `gix_object::Find::try_find`) because it returns the pack location
    // alongside the object data, which we pass through to `Entry::from_data`.
    let mut buf = Vec::new();
    let mut entries = Vec::with_capacity(counts.len());
    for count in &counts {
        let (obj, _loc) = odb_pack
            .try_find(&count.id, &mut buf)
            .map_err(|e| Error::Pack(e.to_string()))?
            .ok_or_else(|| Error::Pack(format!("object {} missing during pack build", count.id)))?;
        let entry = Entry::from_data(count, &obj).map_err(|e| Error::Pack(e.to_string()))?;
        entries.push(entry);
    }
    Ok(entries)
}
