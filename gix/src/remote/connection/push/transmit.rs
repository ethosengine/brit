use std::sync::atomic::AtomicBool;

use gix_transport::client::blocking_io::Transport;

use super::{
    Authorized, Delivery, Error, MovedRef, Prepare,
    prepare::{Contract, SignMode},
};
use crate::{
    bstr::BString,
    remote::{AuthenticateFn, Connection, Direction, push::Outcome},
};

impl<T> Prepare<'_, '_, '_, T>
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
        self,
        progress: impl crate::progress::NestedProgress,
        should_interrupt: &AtomicBool,
    ) -> Result<Outcome, Error> {
        self.transmit_with_before_send(progress, should_interrupt, |_| Ok(Authorized::OnThisConnection))
    }

    /// Resolve updates once, then let the caller authorize them before packing or sending.
    ///
    /// The callback also runs for dry runs and includes unchanged updates so leases can be checked.
    /// Returning an error prevents all transmission.
    ///
    /// What the callback returns decides which connection carries the push:
    /// [`Authorized::OnThisConnection`] sends on the connection whose advertisement the callback
    /// judged. [`Authorized::Revalidate`], which `()` converts to, drops that connection, connects
    /// and performs the handshake again (asking for credentials again), and sends only if every
    /// remote ref still has the id the callback judged and the capabilities the push relies on are
    /// unchanged; otherwise nothing is sent. Dry runs never reconnect.
    ///
    /// If sending fails after the request was handed to the transport, one further handshake on a
    /// fresh connection checks whether the remote refs carry the new ids, and the error says which
    /// is the case. Nothing is resent.
    pub fn transmit_with_before_send<A>(
        self,
        _progress: impl crate::progress::NestedProgress,
        should_interrupt: &AtomicBool,
        before_send: impl FnOnce(&[crate::remote::push::Update]) -> Result<A, Box<dyn std::error::Error + Send + Sync>>,
    ) -> Result<Outcome, Error>
    where
        A: Into<Authorized>,
    {
        use gix_refspec::{
            MatchGroup,
            match_group::{Item, SourceRef},
            parse::Operation,
        };

        let Prepare {
            connection,
            refspecs,
            dry_run,
            prune,
            push_options,
            sign,
            atomic,
        } = self;

        if refspecs.is_empty() {
            return Err(Error::NoRefspecs);
        }

        // Capability guard for `--push-option`: git dies 128 with
        // "the receiving end does not support push options" when the client
        // passed options but the server didn't advertise the `push-options`
        // capability during handshake (send-pack.c::check_push_options).
        if !push_options.is_empty() {
            let supported = connection
                .handshake
                .as_ref()
                .is_some_and(|h| h.capabilities.contains("push-options"));
            if !supported {
                return Err(Error::PushOptionsNotSupported);
            }
        }

        // Capability guard for `--signed=yes`: git dies 128 with
        // "the receiving end does not support --signed push" when
        // SEND_PACK_PUSH_CERT_ALWAYS is requested but the server didn't
        // advertise the `push-cert` capability (send-pack.c::send_pack).
        // `--signed=if-asked` silently falls back to an unsigned push.
        if matches!(sign, SignMode::Always) {
            let supported = connection
                .handshake
                .as_ref()
                .is_some_and(|h| h.capabilities.contains("push-cert"));
            if !supported {
                return Err(Error::SignedPushNotSupported);
            }
        }

        // Capability guard for `--atomic`, as send-pack.c: the remote must offer all-or-nothing.
        if atomic
            && !connection
                .handshake
                .as_ref()
                .is_some_and(|h| h.capabilities.contains("atomic"))
        {
            return Err(Error::AtomicNotSupported);
        }

        // ── Step 1: Parse refspecs ────────────────────────────────────────────
        let parsed_refspecs: Vec<gix_refspec::RefSpec> = refspecs
            .iter()
            .map(|s| gix_refspec::parse(s.as_slice().into(), Operation::Push).map(|r| r.to_owned()))
            .collect::<Result<_, _>>()
            .map_err(Error::ParseRefspec)?;

        // ── Step 2: Enumerate local refs ──────────────────────────────────────
        let repo = connection.remote.repo;
        let hash_kind = repo.object_hash();

        let local_refs: Vec<crate::Reference<'_>> = repo
            .references()
            .map_err(Error::ReferencesInit)?
            .all()
            .map_err(Error::References)?
            .filter_map(Result::ok)
            .collect();

        // ── Step 3: MatchGroup::match_push ────────────────────────────────────
        // Build `Item` from direct (non-symbolic) local refs only.
        // We need to own the ObjectIds separately so `item.target` can borrow them.
        let item_ids: Vec<(gix_hash::ObjectId, &crate::Reference<'_>)> = local_refs
            .iter()
            .filter_map(|r| r.try_id().map(|id| (id.detach(), r)))
            .collect();
        let mut items: Vec<Item<'_>> = item_ids
            .iter()
            .map(|(id, r)| Item {
                full_ref_name: r.name().as_bstr(),
                target: Some(id.as_ref()),
                object: None,
            })
            .collect();
        // HEAD is outside refs/ and can be detached. Include it only for an
        // explicit source so wildcard pushes keep their ordinary ref inventory.
        let head_id = parsed_refspecs
            .iter()
            .any(|spec| spec.to_ref().source().is_some_and(|name| name == "HEAD"))
            .then(|| repo.head_id().map(|id| id.detach()))
            .transpose()
            .map_err(|error| Error::Resolve(Box::new(error)))?;
        if let Some(id) = &head_id {
            items.push(Item {
                full_ref_name: "HEAD".into(),
                target: Some(id.as_ref()),
                object: None,
            });
        }

        let group = MatchGroup::from_push_specs(parsed_refspecs.iter().map(|s| s.to_ref()));
        let (push_outcome, _fixes) = group
            .match_push(items.iter().copied())
            .validated()
            .map_err(Error::RefspecConflict)?;
        let mappings = push_outcome.mappings;

        if mappings.is_empty() {
            let spec = refspecs.into_iter().next().expect("non-empty by guard above");
            return Err(Error::NoMatch { spec });
        }

        // ── Step 4: Resolve OIDs + look up remote's current state ─────────────
        let null_oid = hash_kind.null();

        // Map remote ref name → current ObjectId for O(1) lookup below.
        let remote_oid_by_name = advertised_ids(connection.handshake.as_ref());

        let mut commands: Vec<gix_protocol::send_pack::Command> = Vec::with_capacity(mappings.len());
        // Parallel vec: (local_name, remote_name) for populating RefStatus.
        let mut name_pairs: Vec<(BString, BString)> = Vec::with_capacity(mappings.len());

        for mapping in &mappings {
            let (local_name, new_oid) = match &mapping.lhs {
                SourceRef::FullName(name) if name.as_ref() == "HEAD" => (
                    BString::from("HEAD"),
                    head_id.expect("HEAD is matched only after resolving its target"),
                ),
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

            // One-sided push spec (e.g. `main`) inherits its destination from
            // the matched local ref — mirrors git's `match_push_refs` which
            // pushes to the same-named remote ref when no `:dst` is given.
            // Skip delete-specs (empty lhs) that arrive here with rhs=None:
            // a delete spec without a destination is nonsensical.
            let remote_name: BString = match (&mapping.rhs, local_name.is_empty()) {
                (Some(rhs), _) => rhs.as_ref().into(),
                (None, false) => local_name.clone(),
                (None, true) => continue,
            };

            let old_oid = remote_oid_by_name.get(&remote_name).copied().unwrap_or(null_oid);

            commands.push(gix_protocol::send_pack::Command {
                refname: remote_name.clone(),
                old_oid,
                new_oid,
            });
            // Matching a literal OID may identify a local ref holding that object.
            // Preserve the user's source spelling for pre-push and porcelain output.
            let local_name = parsed_refspecs[mapping.spec_index]
                .to_ref()
                .source()
                .filter(|source| gix_hash::ObjectId::from_hex(source).is_ok())
                .map(BString::from)
                .unwrap_or(local_name);
            name_pairs.push((local_name, remote_name));
        }

        // Prune: for each remote ref matching a spec's RHS pattern but with no
        // local counterpart in the mapping set, synthesize a delete command.
        // Mirrors MATCH_REFS_PRUNE in transport.c + the prune loop in
        // match_push_refs (vendor/git/remote.c). Only glob-style specs drive
        // pruning; a one-sided spec like `main` has no RHS pattern to match
        // remote names against.
        if prune {
            fn glob_match(pattern: &crate::bstr::BStr, name: &crate::bstr::BStr) -> bool {
                use crate::bstr::ByteSlice as _;
                match pattern.find_byte(b'*') {
                    None => pattern == name,
                    Some(pos) => {
                        let prefix = &pattern[..pos];
                        let suffix = &pattern[pos + 1..];
                        name.len() >= prefix.len() + suffix.len() && name.starts_with(prefix) && name.ends_with(suffix)
                    }
                }
            }
            let mapped: std::collections::HashSet<BString> =
                name_pairs.iter().map(|(_, remote)| remote.clone()).collect();
            for (remote_name, oid) in &remote_oid_by_name {
                if mapped.contains(remote_name) {
                    continue;
                }
                let matched_by_any_rhs = parsed_refspecs.iter().any(|spec| {
                    spec.to_ref()
                        .destination()
                        .is_some_and(|dst| glob_match(dst, remote_name.as_ref()))
                });
                if matched_by_any_rhs {
                    commands.push(gix_protocol::send_pack::Command {
                        refname: remote_name.clone(),
                        old_oid: *oid,
                        new_oid: null_oid,
                    });
                    name_pairs.push((BString::default(), remote_name.clone()));
                }
            }
        }

        if commands.is_empty() {
            return Err(Error::NoRefspecs);
        }

        let updates: Vec<_> = commands
            .iter()
            .zip(&name_pairs)
            .map(|(command, (local, remote))| crate::remote::push::Update {
                local: local.clone(),
                remote: remote.clone(),
                old_oid: command.old_oid,
                new_oid: command.new_oid,
            })
            .collect();
        let authorized: Authorized = before_send(&updates).map_err(Error::BeforeSend)?.into();

        let plan = Plan {
            repo,
            commands,
            name_pairs,
            dry_run,
            atomic,
            should_interrupt,
            already: Vec::new(),
        };
        if dry_run || authorized == Authorized::OnThisConnection {
            return plan.deliver(connection);
        }

        // The callback may have run for a long time: the pre-hook connection may be stale (an idle
        // HTTP connection the server dropped, a receive-pack process left waiting) and the remote
        // may have moved. Close it, then judge the push again against a fresh advertisement.
        let judged = Contract::of(connection.handshake.as_ref());
        let Connection {
            remote,
            authenticate,
            transport_options,
            transport,
            ..
        } = connection;
        drop(transport);
        let fresh = reconnect(remote, authenticate, transport_options).map_err(|err| match err {
            Error::Handshake(source) if is_authentication_failure(&source) => {
                Error::ReconnectAuthentication(Box::new(Error::Handshake(source)))
            }
            err => Error::Reconnect(Box::new(err)),
        })?;
        if let Some(capability) = judged.first_difference(&Contract::of(fresh.handshake.as_ref())) {
            return Err(Error::CapabilitiesChanged { capability });
        }
        // Re-validation, per command, against the id the second advertisement shows ("found"):
        //
        // | judged old | requested new       | found         | outcome                                   |
        // |------------|---------------------|---------------|-------------------------------------------|
        // | A          | any                 | A             | send as judged                            |
        // | A          | B                   | B             | already there: up to date, not sent       |
        // | A          | B                   | C (not A, B)  | refuse the push: the ref moved            |
        // | absent     | B (creation)        | B             | already there: up to date, not sent       |
        // | absent     | B (creation)        | C             | refuse: the ref was created elsewhere     |
        // | A          | absent (deletion)   | absent        | already deleted (also prune): satisfied   |
        // | A          | absent (deletion)   | C (not A)     | refuse: the deletion target moved         |
        // | A          | A (no-op)           | C (not A)     | not re-validated: left alone, reported    |
        // |            |                     |               | not pushed with C; the other refs proceed |
        //
        // Remote refs no command names are not looked at. A refusal sends nothing for any ref;
        // receive-pack's own compare-and-swap still guards the time after this handshake.
        let now = advertised_ids(fresh.handshake.as_ref());
        let mut moved = Vec::new();
        let mut settled = Vec::new();
        for (index, command) in plan.commands.iter().enumerate() {
            let found = now.get(&command.refname).copied().unwrap_or(hash_kind.null());
            if found == command.old_oid {
                continue;
            }
            if command.old_oid == command.new_oid && !plan.atomic {
                // Nothing was to change here, so nothing was judged: do not fail the others
                // (an atomic push cannot leave one ref behind, so it refuses below instead).
                let reason = format!(
                    "not pushed: it needed no update when judged, but moved to {} while the before-send callback ran",
                    MovedRef::id(&found)
                );
                settled.push((index, Err(reason.into()), found));
            } else if found == command.new_oid {
                settled.push((index, Ok(()), command.new_oid));
            } else {
                moved.push(MovedRef {
                    refname: command.refname.clone(),
                    expected: command.old_oid,
                    found,
                });
            }
        }
        if !moved.is_empty() {
            return Err(Error::RemoteMoved { moved });
        }
        plan.settle(settled).deliver(fresh)
    }
}

/// The updates a push resolved and authorized, independent of the connection that carries them.
struct Plan<'repo, 'a> {
    repo: &'repo crate::Repository,
    commands: Vec<gix_protocol::send_pack::Command>,
    /// Parallel to `commands`: (local_name, remote_name) for populating RefStatus.
    name_pairs: Vec<(BString, BString)>,
    dry_run: bool,
    /// Ask the remote for all-or-nothing application (`atomic` capability).
    atomic: bool,
    should_interrupt: &'a AtomicBool,
    /// Updates taken out of the request after re-validation, with their reported status.
    already: Vec<crate::remote::push::RefStatus>,
}

impl Plan<'_, '_> {
    /// Take the commands at the given ascending indices out of the request and report each with
    /// its result and the id the remote showed.
    fn settle(mut self, settled: Vec<(usize, Result<(), BString>, gix_hash::ObjectId)>) -> Self {
        let mut taken = Vec::with_capacity(settled.len());
        for (index, result, found) in settled.into_iter().rev() {
            let command = self.commands.remove(index);
            let (local, remote) = self.name_pairs.remove(index);
            taken.push(crate::remote::push::RefStatus {
                local,
                remote,
                result,
                old_oid: found,
                new_oid: command.new_oid,
            });
        }
        taken.reverse();
        self.already.extend(taken);
        self
    }

    /// Pack against `connection`'s advertisement and send there; after a failed send,
    /// verify on a fresh connection whether the updates arrived.
    fn deliver<T: Transport>(self, mut connection: Connection<'_, '_, '_, T>) -> Result<Outcome, Error> {
        let repo = self.repo;
        let hash_kind = repo.object_hash();
        let null_oid = hash_kind.null();
        let handshake = connection.handshake.as_ref();
        if self.commands.is_empty() {
            // Re-validation found every update already on the remote.
            return Ok(Outcome {
                status: self.already,
                notes: Vec::new(),
            });
        }

        // ── Step 5: Build the Request ─────────────────────────────────────────
        let (agent_key, agent_val) = repo.config.user_agent_tuple();
        let agent_cap: BString = match agent_val {
            Some(v) => format!("{agent_key}={v}").into(),
            None => BString::from(agent_key.as_bytes()),
        };
        let mut capabilities: Vec<BString> = vec![
            b"report-status".as_slice().into(),
            b"side-band-64k".as_slice().into(),
            b"ofs-delta".as_slice().into(),
            agent_cap,
        ];
        // Echo the negotiated object format when the server advertises it.
        // Without this, a SHA-256 receive-pack rejects the command list and
        // closes the connection before it can report a ref status.
        if handshake.is_some_and(|h| h.capabilities.contains("object-format")) {
            capabilities.push(format!("object-format={hash_kind}").into());
        }
        if self.atomic {
            capabilities.push(b"atomic".as_slice().into());
        }

        let request = gix_protocol::send_pack::Request {
            commands: self.commands.clone(),
            capabilities,
        };

        // ── Step 6: Enumerate objects to pack ────────────────────────────────
        //
        // APPROACH: use `objects_unthreaded` with `TreeAdditionsComparedToAncestor`
        // to walk commits reachable from new_oid but not from old_oid, and expand
        // each commit to the tree + blob objects it introduces.  Entries are produced
        // by `Entry::from_data` which re-compresses objects as base entries.
        //
        // The hides come from the advertisement of the connection that carries the
        // pack, so a reconnect after the hook packs against what the remote has now.
        //
        // NOTE: This is correct but not maximally efficient — re-compression is
        // more expensive than pack-copying.  A follow-up task should switch to
        // `iter_from_counts` + `InOrderIter` for pack-copy mode.  See Task 7.1 plan.
        let entries = build_pack_entries(
            repo,
            &self.commands,
            advertised_ids(handshake).into_values(),
            self.should_interrupt,
        )?;

        // ── Step 7: Dry run early return ─────────────────────────────────────
        // Lookup table: remote refname → (old_oid, new_oid) for status rendering.
        let cmd_oids: std::collections::HashMap<BString, (gix_hash::ObjectId, gix_hash::ObjectId)> = self
            .commands
            .iter()
            .map(|c| (c.refname.clone(), (c.old_oid, c.new_oid)))
            .collect();

        if self.dry_run {
            let status = self
                .name_pairs
                .into_iter()
                .map(|(local, remote)| {
                    let (old_oid, new_oid) = cmd_oids.get(&remote).copied().unwrap_or((null_oid, null_oid));
                    crate::remote::push::RefStatus {
                        local,
                        remote,
                        result: Ok(()),
                        old_oid,
                        new_oid,
                    }
                })
                .collect();
            return Ok(Outcome {
                status,
                notes: Vec::new(),
            });
        }

        // Only the push upload goes without a deadline; see `crate::remote::push::reqwest_backend()`.
        #[cfg(all(
            feature = "blocking-http-transport-reqwest",
            not(feature = "blocking-http-transport-curl")
        ))]
        if let Some(http) = connection
            .transport_options
            .as_ref()
            .and_then(|options| options.downcast_ref::<gix_protocol::transport::client::blocking_io::http::Options>())
        {
            let mut upload = http.clone();
            upload.backend = Some(crate::remote::push::reqwest_backend(http, true));
            connection
                .transport
                .inner
                .configure(&upload)
                .map_err(Error::ConfigureTransport)?;
        }

        // ── Step 8: Call send_pack ────────────────────────────────────────────
        let protocol_outcome = match gix_protocol::send_pack::send_pack(
            connection.transport_mut(),
            request,
            entries,
            gix_protocol::send_pack::Options {
                // Git's `http.postBuffer`: the largest body sent whole, with its error path intact.
                max_buffered_body: repo
                    .config_snapshot()
                    .integer("http.postBuffer")
                    .and_then(|size| usize::try_from(size).ok())
                    .filter(|size| *size > 0),
                ..Default::default()
            },
            hash_kind,
        ) {
            Ok(outcome) => outcome,
            Err(source) => {
                // The request may have reached the remote even though its answer did not
                // reach us. Ask a fresh connection once; never resend.
                let Connection {
                    remote,
                    authenticate,
                    transport_options,
                    transport,
                    ..
                } = connection;
                drop(transport);
                let delivery = match reconnect(remote, authenticate, transport_options) {
                    Ok(fresh) => Delivery::judge(&self.commands, &advertised_ids(fresh.handshake.as_ref()), null_oid),
                    Err(err) => Some(Delivery::Unverified(Box::new(err))),
                };
                if let Some(delivery) = delivery {
                    return Err(Error::SendFailed { delivery, source });
                }
                // Every ref shows its new id on a fresh handshake: the remote is in the
                // requested state, only the report was lost.
                let note = format!(
                    "the server's report was lost ({}). A fresh advertisement shows every ref this push named at its requested id; it cannot show whether this push applied them, whether server-side hooks ran, or what the server printed.",
                    error_chain(&source)
                );
                let mut status: Vec<_> = self
                    .commands
                    .iter()
                    .zip(self.name_pairs)
                    .map(|(command, (local, remote))| crate::remote::push::RefStatus {
                        local,
                        remote,
                        result: Ok(()),
                        old_oid: command.old_oid,
                        new_oid: command.new_oid,
                    })
                    .collect();
                status.extend(self.already);
                return Ok(Outcome {
                    status,
                    notes: vec![note],
                });
            }
        };

        // ── Step 9: Map to porcelain Outcome ─────────────────────────────────
        let mut status: Vec<_> = protocol_outcome
            .report
            .refs
            .into_iter()
            .map(|r| {
                let local = self
                    .name_pairs
                    .iter()
                    .find(|(_, remote)| remote == &r.refname)
                    .map(|(local, _)| local.clone())
                    .unwrap_or_default();
                let (old_oid, new_oid) = cmd_oids.get(&r.refname).copied().unwrap_or((null_oid, null_oid));
                crate::remote::push::RefStatus {
                    local,
                    remote: r.refname,
                    result: r.result,
                    old_oid,
                    new_oid,
                }
            })
            .collect();
        status.extend(self.already);
        Ok(Outcome {
            status,
            notes: Vec::new(),
        })
    }
}

/// Every message in `err`'s source chain, joined with `: `.
pub(super) fn error_chain(err: &(dyn std::error::Error + 'static)) -> String {
    let mut out = err.to_string();
    let mut next = err.source();
    while let Some(err) = next {
        out.push_str(": ");
        out.push_str(&err.to_string());
        next = err.source();
    }
    out
}

/// Connect to `remote` again for pushing and perform the `receive-pack` handshake,
/// reusing the credentials callback and transport options of the connection it replaces.
///
/// Per transport this is a new `git-receive-pack` process (file), a new `ssh` process and
/// session (ssh), a new TCP connection (git://), or a new HTTP client with its own
/// connection pool (http, https).
fn reconnect<'remote, 'auth, 'repo>(
    remote: &'remote crate::Remote<'repo>,
    authenticate: Option<AuthenticateFn<'auth>>,
    transport_options: Option<Box<dyn std::any::Any>>,
) -> Result<Connection<'remote, 'auth, 'repo, Box<dyn Transport + Send>>, Error> {
    let Connection { transport, trace, .. } = remote.connect(Direction::Push).map_err(Error::Connect)?;
    Connection {
        remote,
        authenticate,
        transport_options,
        transport,
        handshake: None,
        trace,
    }
    .prepare_push(crate::progress::Discard)
    .map(|prepare| prepare.connection)
}

fn is_authentication_failure(err: &gix_error::Error) -> bool {
    err.is_unauthenticated()
        || err
            .downcast_any_ref::<gix_transport::client::AuthenticationRequired>()
            .is_some()
}

/// Map every advertised remote ref name to the id it points at.
fn advertised_ids(
    handshake: Option<&gix_protocol::Handshake>,
) -> std::collections::HashMap<BString, gix_hash::ObjectId> {
    use gix_protocol::handshake::Ref;
    handshake
        .and_then(|h| h.refs.as_deref())
        .unwrap_or(&[])
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
}

/// Collect `gix_pack::data::output::Entry` items for all objects reachable from
/// non-delete commands' `new_oid` but not already present via advertised commits.
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
    advertised: impl IntoIterator<Item = gix_hash::ObjectId>,
    should_interrupt: &AtomicBool,
) -> Result<Vec<gix_pack::data::output::Entry>, Error> {
    use gix_pack::data::output::{Entry, count::objects::ObjectExpansion};
    use gix_traverse::commit::Simple;

    let mut tips: Vec<gix_hash::ObjectId> = commands
        .iter()
        .filter(|c| !c.is_delete() && !c.new_oid.is_null())
        .map(|c| c.new_oid)
        .collect();

    if tips.is_empty() {
        // Pure deletion push — no objects needed in the pack.
        return Ok(Vec::new());
    }

    // Every advertised commit is a common boundary, including refs other than
    // the destination. Unknown remote history cannot prune the local walk.
    let mut hides = std::collections::HashSet::new();
    for id in advertised {
        if repo
            .try_find_header(id)
            .map_err(|error| Error::Resolve(Box::new(error)))?
            .is_some_and(|header| header.kind() == gix_object::Kind::Commit)
        {
            hides.insert(id);
        }
    }
    tips.retain(|id| !hides.contains(id));
    if tips.is_empty() {
        return Ok(Vec::new());
    }

    // Walk commits reachable from tips but hidden behind hides.
    // Simple includes visible tips itself. Re-inserting hidden tips would
    // rebuild trees which the receiver already has.
    let commit_ids: Vec<gix_hash::ObjectId> =
        Simple::new(tips.iter().copied(), crate::object::bounded::Source::deferred(repo))
            .hide(hides)
            .map_err(Error::Walk)?
            .map(|info| info.map(|info| info.id))
            .collect::<Result<_, _>>()
            .map_err(Error::Walk)?;

    if commit_ids.is_empty() {
        return Ok(Vec::new());
    }

    let source = crate::object::bounded::Source::deferred(repo);

    let progress_discard = crate::progress::Discard;
    let (counts, _) = gix_pack::data::output::count::objects_unthreaded_with_source(
        &source,
        &mut commit_ids.iter().map(|id| Ok::<_, gix_error::Error>(*id)),
        &progress_discard,
        should_interrupt,
        ObjectExpansion::TreeAdditionsComparedToAncestor,
    )
    .map_err(Error::Pack)?;

    // Base-entry encoding needs bytes, not a pack location. Reuse bounded
    // operation-owned storage and release each input before reading its successor.
    let mut buf = gix_object::find::Buffer::default();
    let mut entries = Vec::with_capacity(counts.len());
    for count in &counts {
        let obj = source
            .load(&count.id, &mut buf)
            .map_err(|source| Error::ReadObject { id: count.id, source })?;
        let entry = Entry::from_data(count, &obj, gix_zlib::Compression::DEFAULT).map_err(Error::Pack)?;
        entries.push(entry);
        buf.clear();
    }
    Ok(entries)
}
