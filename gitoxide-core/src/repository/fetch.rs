use gix::bstr::BString;

use crate::OutputFormat;

pub struct Options {
    pub format: OutputFormat,
    pub dry_run: bool,
    pub remote: Option<String>,
    /// If non-empty, override all ref-specs otherwise configured in the remote
    pub ref_specs: Vec<BString>,
    pub shallow: gix::remote::fetch::Shallow,
    pub handshake_info: bool,
    pub negotiation_info: bool,
    pub open_negotiation_graph: Option<std::path::PathBuf>,
}

pub const PROGRESS_RANGE: std::ops::RangeInclusive<u8> = 1..=3;

/// A single advertised branch selected for a durable pull. No local ref has been updated.
pub(crate) struct PullFetchPlan {
    pub target_oid: gix::ObjectId,
    pub tracking: Option<(BString, bool)>,
}

/// Newly received object files. The caller owns synchronization and eventual keep-file cleanup.
pub(crate) struct PullFetchOutcome {
    pub plan: PullFetchPlan,
    pub object_paths: Vec<std::path::PathBuf>,
    pub keep_path: Option<std::path::PathBuf>,
    pub keep_identity: Option<(u64, u64)>,
}

/// Resolve the configured mapping before connecting, without guessing a tracking destination for URLs.
pub(crate) fn pull_tracking(
    repo: &gix::Repository,
    remote_name: &str,
    branch: &gix::bstr::BStr,
) -> anyhow::Result<Option<(BString, bool)>> {
    use anyhow::ensure;
    let remote = crate::repository::remote::by_name_or_url(repo, Some(remote_name))?;
    if remote.name().is_none() {
        return Ok(None);
    }
    let specs = remote.refspecs(gix::remote::Direction::Fetch);
    let matches = gix::refspec::MatchGroup::from_fetch_specs(specs.iter().map(|s| s.to_ref())).match_lhs(
        std::iter::once(gix::refspec::match_group::Item {
            full_ref_name: branch,
            target: None,
            object: None,
        }),
    );
    let mut destinations = Vec::new();
    for mapping in &matches.mappings {
        // Literal object refspecs are emitted independently of input items by the native
        // matcher. They are not tracking mappings for the explicitly selected branch.
        if !matches!(&mapping.lhs, gix::refspec::match_group::SourceRef::FullName(name) if name.as_ref() == branch) {
            continue;
        }
        if let Some(name) = &mapping.rhs {
            ensure!(
                name.starts_with(b"refs/remotes/"),
                "pull tracking destination must be under refs/remotes/"
            );
            let _: gix::refs::FullName = name.as_ref().try_into()?;
            destinations.push((
                name.as_ref().to_owned(),
                specs[mapping.spec_index].allow_non_fast_forward(),
            ));
        }
    }
    ensure!(destinations.len() <= 1, "pull refuses ambiguous fetch mappings");
    Ok(destinations.pop())
}

/// Git's opportunistic remote-tracking update (git 1.8.4, `get_ref_map` in `builtin/fetch.c`):
/// a command-line refspec that names a branch but no destination still moves the
/// remote-tracking ref the remote's configured fetch refspecs map that branch to.
///
/// Return one exact refspec per such mapping, carrying the configured spec's force flag, so
/// the fetch updates only the named branch's tracking ref and leaves every other tracking ref
/// alone. Refspecs with a destination, globs, negations and object ids contribute nothing,
/// and neither does a URL remote (`named` false), which has no configured refspecs.
/// Unparsable refspecs are skipped here; replacing the remote's refspecs reports them.
///
/// A short name resolves as Git resolves it: to the first of its expansions, in rev-parse
/// order, that the remote `advertised`. Without the advertisement every expansion is a
/// candidate, which the caller must narrow once the advertisement is known.
pub(crate) fn opportunistic_tracking_specs(
    configured: &[gix::refspec::RefSpec],
    named: bool,
    command_line: &[BString],
    advertised: Option<&std::collections::BTreeSet<BString>>,
) -> Vec<gix::refspec::RefSpec> {
    use gix::bstr::{ByteSlice, ByteVec};
    if !named {
        return Vec::new();
    }
    let parsed: Vec<_> = command_line
        .iter()
        .filter_map(|spec| gix::refspec::parse(spec.as_bstr(), gix::refspec::parse::Operation::Fetch).ok())
        .collect();
    let explicit_destinations: Vec<_> = parsed.iter().filter_map(|spec| spec.destination()).collect();
    let mut candidates = Vec::new();
    for spec in &parsed {
        // Only a positive, destination-less refspec; it can never be a pattern.
        let gix::refspec::Instruction::Fetch(gix::refspec::instruction::Fetch::Only { src: source }) =
            spec.instruction()
        else {
            continue;
        };
        if gix::ObjectId::from_hex(source).is_ok() {
            continue;
        }
        let expansions: Vec<BString> = if source.starts_with(b"refs/") {
            vec![source.into()]
        } else {
            // Git's `ref_rev_parse_rules`, in order.
            [
                ("", ""),
                ("refs/", ""),
                ("refs/tags/", ""),
                ("refs/heads/", ""),
                ("refs/remotes/", ""),
                ("refs/remotes/", "/HEAD"),
            ]
            .into_iter()
            .map(|(base, suffix)| {
                let mut name = BString::from(base);
                name.push_str(source);
                name.push_str(suffix);
                name
            })
            .collect()
        };
        match advertised {
            Some(advertised) => candidates.extend(expansions.into_iter().find(|name| advertised.contains(name))),
            None => candidates.extend(expansions),
        }
    }
    let matches = gix::refspec::MatchGroup::from_fetch_specs(configured.iter().map(gix::refspec::RefSpec::to_ref))
        .match_lhs(candidates.iter().map(|name| gix::refspec::match_group::Item {
            full_ref_name: name.as_bstr(),
            target: None,
            object: None,
        }));
    let mut specs: Vec<gix::refspec::RefSpec> = Vec::new();
    for mapping in &matches.mappings {
        let (gix::refspec::match_group::SourceRef::FullName(source), Some(destination)) = (&mapping.lhs, &mapping.rhs)
        else {
            continue;
        };
        if !destination.starts_with(b"refs/") || explicit_destinations.contains(&destination.as_ref()) {
            continue;
        }
        let mut text = BString::from(if configured[mapping.spec_index].allow_non_fast_forward() {
            "+"
        } else {
            ""
        });
        text.push_str(source.as_ref());
        text.push_byte(b':');
        text.push_str(destination.as_ref());
        if let Ok(spec) = gix::refspec::parse(text.as_bstr(), gix::refspec::parse::Operation::Fetch) {
            let spec = spec.to_owned();
            if !specs.contains(&spec) {
                specs.push(spec);
            }
        }
    }
    specs
}

/// Whether a first `fetch` positional names a repository rather than a ref: a URL
/// (`scheme://...` or scp-like `user@host:path`), an explicit path (`/`, `./`, `../`, `~/`,
/// `.` or `..`), or a relative path to an existing repository directory. Refspecs never take
/// these forms, except a bare relative name that happens to be a repository directory here.
pub(crate) fn names_a_repository(candidate: &gix::bstr::BStr) -> bool {
    let Ok(text) = std::str::from_utf8(candidate) else {
        return false;
    };
    if text.contains("://") {
        return true;
    }
    if let Some((user_host, _path)) = text.split_once(':')
        && let Some((user, host)) = user_host.split_once('@')
        && !user.is_empty()
        && !host.is_empty()
        && !user_host.contains('/')
    {
        return true;
    }
    if matches!(text, "." | "..") || ["/", "./", "../", "~/"].iter().any(|prefix| text.starts_with(prefix)) {
        return true;
    }
    let path = std::path::Path::new(text);
    path.is_dir() && (path.join(".git").exists() || (path.join("HEAD").is_file() && path.join("objects").is_dir()))
}

/// The command-line refspecs that name one ref (not a pattern, negation or object id) yet
/// matched nothing the remote advertised. Git refuses these: "couldn't find remote ref".
pub(crate) fn unmatched_refspecs(
    command_line: &[gix::refspec::RefSpec],
    map: &gix::remote::fetch::RefMap,
) -> Vec<BString> {
    command_line
        .iter()
        .enumerate()
        .filter_map(|(index, spec)| {
            let source = match spec.to_ref().instruction() {
                gix::refspec::Instruction::Fetch(
                    gix::refspec::instruction::Fetch::Only { src }
                    | gix::refspec::instruction::Fetch::AndUpdate { src, .. },
                ) => src,
                _ => return None,
            };
            if source.contains(&b'*') || gix::ObjectId::from_hex(source).is_ok() {
                return None;
            }
            let matched = map
                .mappings
                .iter()
                .any(|mapping| mapping.spec_index == gix::remote::fetch::refmap::SpecIndex::ExplicitInRemote(index));
            (!matched).then(|| source.to_owned())
        })
        .collect()
}

/// Receive only the selected branch, leaving every ref and FETCH_HEAD for the caller's journal.
/// The callback runs after advertisement and before receiving any pack bytes.
pub(crate) fn pull_receive(
    repo: &gix::Repository,
    remote_name: &str,
    branch: &gix::bstr::BStr,
    pin: impl FnOnce(&PullFetchPlan) -> anyhow::Result<()>,
) -> anyhow::Result<PullFetchOutcome> {
    use anyhow::{Context, ensure};
    let mut isolated = repo.clone();
    isolated
        .config_snapshot_mut()
        .set_value(&gix::config::tree::Clone::REJECT_SHALLOW, "true")?;
    let repo = &isolated;
    let tracking = pull_tracking(repo, remote_name, branch)?;
    let mut remote = crate::repository::remote::by_name_or_url(repo, Some(remote_name))?;
    remote.replace_refspecs([branch], gix::remote::Direction::Fetch)?;
    remote = remote.with_fetch_tags(gix::remote::fetch::Tags::None);
    let prepared = remote
        .connect(gix::remote::Direction::Fetch)?
        .prepare_fetch(gix::progress::Discard, Default::default())?;
    let mappings = &prepared.ref_map().mappings;
    ensure!(
        mappings.len() == 1,
        "pull requires exactly one advertised remote branch"
    );
    ensure!(
        mappings[0].local.is_none(),
        "pull fetch unexpectedly selected a local destination"
    );
    let target_oid = mappings[0]
        .remote
        .as_id()
        .context("remote branch is unborn")?
        .to_owned();
    let plan = PullFetchPlan { target_oid, tracking };
    pin(&plan)?;
    let received = prepared.receive(gix::progress::Discard, &gix::interrupt::IS_INTERRUPTED)?;
    let (object_paths, keep_path, keep_identity) = match received.status {
        gix::remote::fetch::Status::Change {
            write_pack_bundle,
            update_refs,
            ..
        } => {
            ensure!(
                update_refs.edits.is_empty(),
                "source-only pull fetch unexpectedly updated refs"
            );
            (
                write_pack_bundle
                    .data_path
                    .into_iter()
                    .chain(write_pack_bundle.index_path)
                    .collect(),
                write_pack_bundle.keep_path,
                write_pack_bundle.keep_identity,
            )
        }
        gix::remote::fetch::Status::NoPackReceived { update_refs, .. } => {
            ensure!(
                update_refs.edits.is_empty(),
                "source-only pull fetch unexpectedly updated refs"
            );
            (Vec::new(), None, None)
        }
    };
    Ok(PullFetchOutcome {
        plan,
        object_paths,
        keep_path,
        keep_identity,
    })
}

pub(crate) mod function {
    use gix::{
        Result,
        error::{OptionExt, ResultExt, bail, validation},
        prelude::ObjectIdExt,
        refs::{
            Target,
            transaction::{PreviousValue, RefEdit},
        },
        refspec::match_group::validate::Fix,
        remote::fetch::{Status, refs::update::TypeChange},
    };
    use layout::{
        backends::svg::SVGWriter,
        core::{base::Orientation, geometry::Point, style::StyleAttr},
        std_shapes::shapes::{Arrow, Element, ShapeKind},
    };

    use super::Options;
    use crate::{OutputFormat, repository::notes_refspec};

    pub fn fetch<P>(
        repo: gix::Repository,
        mut progress: P,
        mut out: impl std::io::Write,
        err: impl std::io::Write,
        Options {
            format,
            dry_run,
            remote,
            handshake_info,
            negotiation_info,
            open_negotiation_graph,
            shallow,
            ref_specs,
        }: Options,
    ) -> Result<()>
    where
        P: gix::NestedProgress,
        P::SubProgress: 'static,
    {
        if format != OutputFormat::Human {
            bail!(gix::error::unsupported("JSON output isn't yet supported for fetching."));
        }

        use gix::bstr::ByteSlice;
        // Git's spelling `fetch <repository> <refspec>...`: without `--remote`, a first
        // positional naming a configured remote, or a URL or path to a repository, selects it.
        // With `--remote`, every positional is a refspec.
        let explicit_remote = remote.is_some();
        let (remote, ref_specs) = match remote {
            Some(remote) => (Some(remote), ref_specs),
            None => match ref_specs.split_first() {
                Some((first, rest))
                    if repo.remote_names().iter().any(|name| name.as_bstr() == first.as_bstr())
                        || super::names_a_repository(first.as_bstr()) =>
                {
                    (Some(first.to_str_lossy().into_owned()), rest.to_vec())
                }
                _ => (None, ref_specs),
            },
        };
        let first_positional_is_refspec = !explicit_remote && remote.is_none() && !ref_specs.is_empty();
        let remote_selector = remote.as_deref();
        let mut remote = crate::repository::remote::by_name_or_url(&repo, remote_selector)?;
        // Read the configured mapping before the command line replaces it.
        let configured = remote.refspecs(gix::remote::Direction::Fetch).to_vec();
        let configured_specs;
        let effective_specs = if ref_specs.is_empty() {
            configured_specs = configured
                .iter()
                .map(|spec| spec.to_ref().to_bstring())
                .collect::<Vec<_>>();
            &configured_specs
        } else {
            &ref_specs
        };
        if let Some(name) = notes_refspec::single(effective_specs)? {
            if !matches!(shallow, gix::remote::fetch::Shallow::NoChange) {
                bail!(validation(
                    "notes fetch requires complete history and accepts no depth options"
                ));
            }
            let res = fetch_notes(&repo, remote_selector, name.as_ref(), dry_run, &mut progress)?;
            if handshake_info {
                writeln!(out, "Handshake Information\n\t{:?}", res.handshake).or_error()?;
            }
            let negotiate = match &res.status {
                Status::Change { negotiate, .. } => Some(negotiate),
                Status::NoPackReceived { negotiate, .. } => negotiate.as_ref(),
            };
            if negotiation_info {
                print_negotiate_info(&mut out, negotiate).or_error()?;
            }
            if let Some((negotiate, path)) = negotiate.zip(open_negotiation_graph) {
                render_graph(&repo, &negotiate.graph, &path, progress)?;
            }
            if dry_run {
                writeln!(out, "DRY-RUN: {name} can be fetched without changing the repository.").or_error()?;
            } else {
                writeln!(out, "{name} -> {name}").or_error()?;
            }
            return Ok(());
        }
        let named = remote.name().is_some();
        let remote_display = remote.name().map_or_else(
            || {
                remote
                    .url(gix::remote::Direction::Fetch)
                    .map_or_else(|| "the remote".to_owned(), |url| url.to_bstring().to_string())
            },
            |name| name.as_bstr().to_string(),
        );
        let tracking = |advertised: Option<&std::collections::BTreeSet<gix::bstr::BString>>| {
            super::opportunistic_tracking_specs(&configured, named, &ref_specs, advertised)
        };
        let provisional = if ref_specs.is_empty() {
            Vec::new()
        } else {
            remote.replace_refspecs(ref_specs.iter(), gix::remote::Direction::Fetch)?;
            remote = remote.with_fetch_tags(gix::remote::fetch::Tags::None);
            tracking(None)
        };
        let options = |extra_refspecs| gix::remote::ref_map::Options {
            extra_refspecs,
            ..Default::default()
        };
        // Every advertisement a fetch proceeds on must hold each ref the command line names.
        let ensure_matched = |map: &gix::remote::fetch::RefMap| -> Result<()> {
            let specs = remote.refspecs(gix::remote::Direction::Fetch);
            let unmatched = super::unmatched_refspecs(specs, map);
            if unmatched.is_empty() {
                return Ok(());
            }
            let first_source = specs
                .first()
                .and_then(|spec| spec.to_ref().source().map(ToOwned::to_owned));
            if first_positional_is_refspec && unmatched.first() == first_source.as_ref() {
                bail!(gix::error::validation(format!(
                    "'{}' is neither a configured remote, a URL or path to a repository, nor a ref on {remote_display}",
                    unmatched[0].to_str_lossy()
                )));
            }
            bail!(gix::error::validation(format!(
                "couldn't find remote ref {}",
                unmatched
                    .iter()
                    .map(|name| name.to_str_lossy().into_owned())
                    .collect::<Vec<_>>()
                    .join(", ")
            )));
        };
        let mut prepared = remote
            .connect(gix::remote::Direction::Fetch)?
            .prepare_fetch(&mut progress, options(provisional.clone()))?;
        if !ref_specs.is_empty() {
            ensure_matched(prepared.ref_map())?;
            // A short name that more than one advertised ref expands to resolves to the best
            // one only; reconnect without the other candidates' tracking updates.
            let advertised: std::collections::BTreeSet<gix::bstr::BString> = prepared
                .ref_map()
                .remote_refs
                .iter()
                .map(|r| r.unpack().0.to_owned())
                .collect();
            let needed = tracking(Some(&advertised));
            let stray = prepared.ref_map().mappings.iter().any(|mapping| {
                mapping
                    .spec_index
                    .implicit_index()
                    .is_some_and(|index| !needed.contains(&provisional[index]))
            });
            if stray {
                drop(prepared);
                prepared = remote
                    .connect(gix::remote::Direction::Fetch)?
                    .prepare_fetch(&mut progress, options(needed))?;
                // The remote may have changed between the two advertisements.
                ensure_matched(prepared.ref_map())?;
            }
        }
        if prepared.ref_map().mappings.iter().any(|mapping| {
            mapping
                .local
                .as_ref()
                .is_some_and(|name| notes_refspec::is_notes(name.as_bstr()))
        }) {
            bail!(validation(notes_refspec::ONE_REF_FORM));
        }
        let res: gix::remote::fetch::Outcome = prepared
            .with_dry_run(dry_run)
            .with_shallow(shallow)
            .receive(&mut progress, &gix::interrupt::IS_INTERRUPTED)?;

        if handshake_info {
            writeln!(out, "Handshake Information").or_error()?;
            writeln!(out, "\t{:?}", res.handshake).or_error()?;
        }

        let ref_specs = remote.refspecs(gix::remote::Direction::Fetch);
        let rejected = rejected_updates(&res)?;
        match res.status {
            Status::NoPackReceived {
                update_refs,
                negotiate,
                dry_run: _,
            } => {
                let negotiate_default = Default::default();
                print_updates(
                    &repo,
                    negotiate.as_ref().unwrap_or(&negotiate_default),
                    update_refs,
                    ref_specs,
                    res.ref_map,
                    &mut out,
                    err,
                )?;
                if negotiation_info {
                    print_negotiate_info(&mut out, negotiate.as_ref()).or_error()?;
                }
                if let Some((negotiate, path)) = negotiate.as_ref().zip(open_negotiation_graph) {
                    render_graph(&repo, &negotiate.graph, &path, progress)?;
                }
                Ok::<_, gix::Error>(())
            }
            Status::Change {
                update_refs,
                write_pack_bundle,
                negotiate,
            } => {
                print_updates(&repo, &negotiate, update_refs, ref_specs, res.ref_map, &mut out, err)?;
                if let Some(data_path) = write_pack_bundle.data_path {
                    writeln!(out, "pack  file: \"{}\"", data_path.display()).ok();
                }
                if let Some(index_path) = write_pack_bundle.index_path {
                    writeln!(out, "index file: \"{}\"", index_path.display()).ok();
                }
                if negotiation_info {
                    print_negotiate_info(&mut out, Some(&negotiate)).or_error()?;
                }
                if let Some(path) = open_negotiation_graph {
                    render_graph(&repo, &negotiate.graph, &path, progress)?;
                }
                Ok(())
            }
        }?;
        if dry_run {
            writeln!(out, "DRY-RUN: No ref was updated and no pack was received.").ok();
        }
        if !rejected.is_empty() {
            bail!(gix::error::validation(format!(
                "rejected ref update(s): {}",
                rejected.join(", ")
            )));
        }
        Ok(())
    }

    fn fetch_notes<P: gix::NestedProgress>(
        repo: &gix::Repository,
        remote_selector: Option<&str>,
        name: &gix::refs::FullNameRef,
        dry_run: bool,
        progress: P,
    ) -> Result<gix::remote::fetch::Outcome>
    where
        P::SubProgress: 'static,
    {
        use std::fs;

        use gix::{bstr::ByteVec, object::Kind, remote::fetch::refmap::Source};

        let local = repo
            .try_find_reference(name.as_bstr())?
            .filter(|reference| reference.name() == name);
        let old_commit_id = local
            .as_ref()
            .map(|reference| {
                reference
                    .try_id()
                    .ok_or_raise(|| validation(format!("notes ref {name} must be a direct reference")))
            })
            .transpose()?;

        // Keep received packs out of the live object store until ancestry has been verified.
        // The original store remains available for negotiation and thin-pack bases.
        let objects_dir = repo.objects.store_ref().path();
        let quarantine = tempfile::tempdir().or_error()?;
        let info = quarantine.path().join("info");
        fs::create_dir(&info).or_error()?;
        fs::create_dir(quarantine.path().join("pack")).or_error()?;
        let absolute = std::path::absolute(objects_dir).or_error()?;
        let path = gix::path::os_str_into_bstr(absolute.as_os_str())?;
        let mut alternate = gix::quote::ansi_c::quote(path).into_owned();
        alternate.push_byte(b'\n');
        fs::write(info.join("alternates"), alternate).or_error()?;
        let mut isolated = repo.clone();
        isolated.objects = gix::odb::at_opts(
            quarantine.path().to_owned(),
            repo.object_hash(),
            None,
            gix::odb::store::init::Options {
                alloc_limit_bytes: repo.allocation_limit(),
                ..Default::default()
            },
        )
        .or_error()?
        .into();
        isolated
            .config_snapshot_mut()
            .set_value(&gix::config::tree::Clone::REJECT_SHALLOW, "true")?;
        let mut remote = crate::repository::remote::by_name_or_url(&isolated, remote_selector)?;
        remote.replace_refspecs([name.as_bstr()], gix::remote::Direction::Fetch)?;
        let remote = remote.with_fetch_tags(gix::remote::fetch::Tags::None);
        let prepared = remote
            .connect(gix::remote::Direction::Fetch)?
            .prepare_fetch(gix::progress::Discard, Default::default())?;
        let new_commit_id = match prepared.ref_map().mappings.as_slice() {
            [mapping] if mapping.local.is_none() => match &mapping.remote {
                Source::Ref(reference) if reference.unpack().0 == name.as_bstr() => {
                    reference.unpack().1.map(ToOwned::to_owned)
                }
                _ => None,
            },
            _ => None,
        }
        .ok_or_raise(|| validation(format!("couldn't find remote ref {name}")))?;
        let res = prepared.receive(progress, &gix::interrupt::IS_INTERRUPTED)?;
        if isolated.find_header(new_commit_id)?.kind() != Kind::Commit {
            bail!(validation(format!("notes ref {name} must point to a commit")));
        }
        if let Some(old_commit_id) = old_commit_id {
            if old_commit_id == new_commit_id {
                return Ok(res);
            }
            if !notes_refspec::is_fast_forward(&isolated, old_commit_id.detach(), new_commit_id)? {
                bail!(validation(format!("refusing non-fast-forward update of {name}")));
            }
        }
        if dry_run {
            return Ok(res);
        }
        let expected = old_commit_id.map_or(PreviousValue::MustNotExist, |id| {
            PreviousValue::MustExistAndMatch(Target::Object(id.detach()))
        });
        // Check again under Git's ref lock before promoting any objects.
        let config = repo.config_snapshot();
        let file_timeout = gix::config::tree::Core::FILES_REF_LOCK_TIMEOUT
            .try_into_lock_timeout(config.try_integer("core.filesRefLockTimeout"))?
            .unwrap_or(gix::lock::acquire::Fail::AfterDurationWithBackoff(
                std::time::Duration::from_millis(100),
            ));
        let packed_timeout = gix::config::tree::Core::PACKED_REFS_TIMEOUT
            .try_into_lock_timeout(config.try_integer("core.packedRefsTimeout"))?
            .unwrap_or(gix::lock::acquire::Fail::AfterDurationWithBackoff(
                std::time::Duration::from_secs(1),
            ));
        let committer = repo.committer().transpose()?;
        let edit = RefEdit::update(name.to_owned(), new_commit_id, expected, "fetch: update notes");
        let transaction = repo.refs.transaction().prepare([edit], file_timeout, packed_timeout)?;
        let mut keep = None;
        if let Status::Change { write_pack_bundle, .. } = &res.status {
            let pack_dir = objects_dir.join("pack");
            fs::create_dir_all(&pack_dir).or_error()?;
            if let Some(file_name) = write_pack_bundle
                .keep_path
                .as_deref()
                .and_then(std::path::Path::file_name)
            {
                match gix::tempfile::mark_at(
                    pack_dir.join(file_name),
                    gix::tempfile::ContainingDirectory::Exists,
                    gix::tempfile::AutoRemove::Tempfile,
                ) {
                    Ok(marker) => keep = Some(marker),
                    Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => {}
                    Err(err) => return Err(err).or_error(),
                }
            }
            for path in write_pack_bundle.data_path.iter().chain(&write_pack_bundle.index_path) {
                let file_name = path
                    .file_name()
                    .ok_or_raise(|| validation("received pack path has no file name"))?;
                let mut file = tempfile::NamedTempFile::new_in(&pack_dir).or_error()?;
                std::io::copy(&mut fs::File::open(path).or_error()?, file.as_file_mut()).or_error()?;
                if let Err(err) = file.persist_noclobber(pack_dir.join(file_name))
                    && err.error.kind() != std::io::ErrorKind::AlreadyExists
                {
                    return Err(err.error).or_error();
                }
            }
        }
        transaction.commit(committer)?;
        drop(keep);
        Ok(res)
    }

    /// Every local ref whose update was refused, as `<ref> (<reason>)`. Git exits 1 for these.
    ///
    /// Updates pair with ref-map mappings by position; a count mismatch would name the wrong
    /// refs, so it is an error rather than a silent truncation.
    fn rejected_updates(res: &gix::remote::fetch::Outcome) -> Result<Vec<String>> {
        use gix::remote::fetch::refs::update::Mode;
        let update_refs = match &res.status {
            Status::NoPackReceived { update_refs, .. } | Status::Change { update_refs, .. } => update_refs,
        };
        if update_refs.updates.len() != res.ref_map.mappings.len() {
            bail!(gix::error::message!(
                "cannot attribute ref updates: {} update outcome(s) for {} ref mapping(s)",
                update_refs.updates.len(),
                res.ref_map.mappings.len()
            ));
        }
        Ok(update_refs
            .updates
            .iter()
            .zip(&res.ref_map.mappings)
            .filter(|(update, _)| {
                matches!(
                    update.mode,
                    Mode::RejectedSourceObjectNotFound { .. }
                        | Mode::RejectedTagUpdate
                        | Mode::RejectedNonFastForward
                        | Mode::RejectedToReplaceWithUnborn
                        | Mode::RejectedCurrentlyCheckedOut { .. }
                )
            })
            .map(|(update, mapping)| {
                let name = mapping
                    .local
                    .as_ref()
                    .map_or_else(|| "(no local ref)".to_owned(), ToString::to_string);
                format!("{name} ({})", update.mode)
            })
            .collect())
    }

    fn render_graph(
        repo: &gix::Repository,
        graph: &gix::negotiate::IdMap,
        path: &std::path::Path,
        mut progress: impl gix::Progress,
    ) -> Result<()> {
        progress.init(Some(graph.len()), gix::progress::count("commits"));
        progress.set_name("building graph".into());

        let mut map = gix::hashtable::HashMap::default();
        let mut vg = layout::topo::layout::VisualGraph::new(Orientation::TopToBottom);

        for (id, commit) in graph.iter().inspect(|_| progress.inc()) {
            let source = match map.get(id) {
                Some(handle) => *handle,
                None => {
                    let handle = vg.add_node(new_node(id.attach(repo), commit.data.flags));
                    map.insert(*id, handle);
                    handle
                }
            };

            for parent_id in &commit.parents {
                let dest = match map.get(parent_id) {
                    Some(handle) => *handle,
                    None => {
                        let flags = match graph.get(parent_id) {
                            Some(c) => c.data.flags,
                            None => continue,
                        };
                        let dest = vg.add_node(new_node(parent_id.attach(repo), flags));
                        map.insert(*parent_id, dest);
                        dest
                    }
                };
                let arrow = Arrow::simple("");
                vg.add_edge(arrow, source, dest);
            }
        }

        let start = std::time::Instant::now();
        progress.set_name("layout graph".into());
        progress.info(format!("writing {}…", path.display()));
        let mut svg = SVGWriter::new();
        vg.do_it(false, false, false, &mut svg);
        std::fs::write(path, svg.finalize().as_bytes()).or_error()?;
        open::that(path).or_error()?;
        progress.show_throughput(start);

        return Ok(());

        fn new_node(id: gix::Id<'_>, flags: gix::negotiate::Flags) -> Element {
            let pt = Point::new(250., 50.);
            let name = format!("{}\n\n{flags:?}", id.shorten_or_id());
            let shape = ShapeKind::new_box(name.as_str());
            let style = StyleAttr::simple();
            Element::create(shape, style, Orientation::LeftToRight, pt)
        }
    }

    fn print_negotiate_info(
        mut out: impl std::io::Write,
        negotiate: Option<&gix::remote::fetch::outcome::Negotiate>,
    ) -> std::io::Result<()> {
        writeln!(out, "Negotiation Phase Information")?;
        match negotiate {
            Some(negotiate) => {
                writeln!(out, "\t{:?}", negotiate.rounds)?;
                writeln!(out, "\tnum commits traversed in graph: {}", negotiate.graph.len())
            }
            None => writeln!(out, "\tno negotiation performed"),
        }
    }

    pub(crate) fn print_updates(
        repo: &gix::Repository,
        negotiate: &gix::remote::fetch::outcome::Negotiate,
        update_refs: gix::remote::fetch::refs::update::Outcome,
        refspecs: &[gix::refspec::RefSpec],
        mut map: gix::remote::fetch::RefMap,
        mut out: impl std::io::Write,
        mut err: impl std::io::Write,
    ) -> Result<()> {
        let mut last_spec_index = gix::remote::fetch::refmap::SpecIndex::ExplicitInRemote(usize::MAX);
        let mut updates = update_refs
            .iter_mapping_updates(&map.mappings, refspecs, &map.extra_refspecs)
            .filter_map(|(update, mapping, spec, edit)| spec.map(|spec| (update, mapping, spec, edit)))
            .collect::<Vec<_>>();
        updates.sort_by_key(|t| t.2);
        let mut skipped_due_to_implicit_tag = None;
        fn consume_skipped_tags(skipped: &mut Option<usize>, out: &mut impl std::io::Write) -> std::io::Result<()> {
            match skipped.take() {
                Some(skipped) if skipped != 0 => {
                    writeln!(
                        out,
                        "\tskipped {skipped} tags known to the remote without bearing on this commit-graph. Use `gix remote ref-map` to list them."
                    )?;
                }
                _ => {}
            }
            Ok(())
        }
        for (update, mapping, spec, edit) in updates {
            if mapping.spec_index != last_spec_index {
                last_spec_index = mapping.spec_index;
                consume_skipped_tags(&mut skipped_due_to_implicit_tag, &mut out).or_error()?;
                spec.to_ref().write_to(&mut out).or_error()?;
                let is_implicit = mapping.spec_index.implicit_index().is_some();
                if is_implicit {
                    write!(&mut out, " (implicit").or_error()?;
                    if spec.to_ref()
                        == gix::remote::fetch::Tags::Included
                            .to_refspec()
                            .expect("always yields refspec")
                    {
                        skipped_due_to_implicit_tag = Some(0);
                        write!(&mut out, ", due to auto-tag").or_error()?;
                    }
                    write!(&mut out, ")").or_error()?;
                }
                writeln!(out).or_error()?;
            }

            match skipped_due_to_implicit_tag.as_mut() {
                Some(num_skipped) if matches!(update.mode, gix::remote::fetch::refs::update::Mode::NoChangeNeeded) => {
                    *num_skipped += 1;
                    continue;
                }
                _ => {}
            }

            write!(out, "\t").or_error()?;
            match &mapping.remote {
                gix::remote::fetch::refmap::Source::ObjectId(id) => {
                    write!(out, "{}", id.attach(repo).shorten_or_id()).or_error()?;
                }
                gix::remote::fetch::refmap::Source::Ref(r) => {
                    crate::repository::remote::refs::print_ref(&mut out, r).or_error()?;
                }
            }
            let mode_and_type = update.type_change.map_or_else(
                || format!("{}", update.mode),
                |type_change| {
                    format!(
                        "{} ({})",
                        update.mode,
                        match type_change {
                            TypeChange::DirectToSymbolic => {
                                "direct ref overwrites symbolic"
                            }
                            TypeChange::SymbolicToDirect => {
                                "symbolic ref overwrites direct"
                            }
                        }
                    )
                },
            );
            match edit {
                Some(edit) => {
                    writeln!(out, " -> {} [{mode_and_type}]", edit.name)
                }
                None => writeln!(out, " [{mode_and_type}]"),
            }
            .or_error()?;
        }
        consume_skipped_tags(&mut skipped_due_to_implicit_tag, &mut out).or_error()?;
        if !map.fixes.is_empty() {
            writeln!(
                err,
                "The following destination refs were removed as they didn't start with 'ref/'"
            )
            .or_error()?;
            map.fixes.sort_by(|l, r| match (l, r) {
                (
                    Fix::MappingWithPartialDestinationRemoved { spec: l, .. },
                    Fix::MappingWithPartialDestinationRemoved { spec: r, .. },
                ) => l.cmp(r),
            });
            let mut prev_spec = None;
            for fix in &map.fixes {
                match fix {
                    Fix::MappingWithPartialDestinationRemoved { name, spec } => {
                        if prev_spec.is_some_and(|prev_spec| prev_spec != spec) {
                            prev_spec = spec.into();
                            spec.to_ref().write_to(&mut err).or_error()?;
                            writeln!(err).or_error()?;
                        }
                        writeln!(err, "\t{name}").or_error()?;
                    }
                }
            }
        }
        // Several refspecs may map the same tip, so count tips, not mappings.
        let mapped: std::collections::BTreeSet<_> = map
            .mappings
            .iter()
            .filter_map(|mapping| match &mapping.remote {
                gix::remote::fetch::refmap::Source::Ref(r) => Some(r.unpack().0),
                gix::remote::fetch::refmap::Source::ObjectId(_) => None,
            })
            .collect();
        let filtered = map
            .remote_refs
            .iter()
            .filter(|r| !mapped.contains(r.unpack().0))
            .count();
        if filtered != 0 {
            writeln!(
                err,
                "server sent {} tips, {filtered} were filtered due to {} refspec(s).",
                map.remote_refs.len(),
                refspecs.len()
            )
            .or_error()?;
        }
        match negotiate.rounds.len() {
            0 => writeln!(err, "no negotiation was necessary").or_error()?,
            1 => {}
            rounds => writeln!(err, "needed {rounds} rounds of pack-negotiation").or_error()?,
        }
        Ok(())
    }
}
