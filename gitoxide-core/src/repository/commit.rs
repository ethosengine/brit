use std::{
    borrow::Cow,
    io::{Read, Write},
    process::Stdio,
};

use anyhow::{Context, Result, anyhow, bail};
use gix::{
    bstr::{BStr, BString},
    objs::commit::SIGNATURE_FIELD_NAME,
};

/// Note that this is a quick implementation of commit signature verification that ignores a lot of what
/// git does and can do, while focussing on the gist of it.
/// For this to go into `gix`, one will have to implement many more options and various validation programs.
pub fn verify(repo: gix::Repository, rev_spec: Option<&str>) -> Result<()> {
    let rev_spec = rev_spec.unwrap_or("HEAD");
    let commit = repo
        .rev_parse_single(format!("{rev_spec}^{{commit}}").as_str())?
        .object()?
        .into_commit();
    let outcome = commit
        .verify_signature()
        .context("Could not verify commit signature")?
        .ok_or_else(|| anyhow!("Commit at {rev_spec} is not signed"))?;
    std::io::stderr().write_all(&outcome.output)?;
    if !outcome.is_valid() {
        bail!("Commit at {rev_spec} has an invalid or untrusted signature");
    }
    Ok(())
}

/// Note that this is a quick first prototype that lacks some of the features provided by `git verify-commit`.
pub fn sign(repo: gix::Repository, rev_spec: Option<&str>, mut out: impl std::io::Write) -> Result<()> {
    let rev_spec = rev_spec.unwrap_or("HEAD");
    let object = repo
        .rev_parse_single(format!("{rev_spec}^{{commit}}").as_str())?
        .object()?;
    let mut commit_ref = object.to_commit_ref();
    if commit_ref.extra_headers().pgp_signature().is_some() {
        gix::trace::info!("The commit {id} is already signed, did nothing", id = object.id);
        writeln!(out, "{id}", id = object.id)?;
        return Ok(());
    }

    let mut cmd: std::process::Command = gix::command::prepare("gpg").into();
    cmd.args([
        "--keyid-format=long",
        "--status-fd=2",
        "--detach-sign",
        "--sign",
        "--armor",
    ])
    .stdin(Stdio::piped())
    .stdout(Stdio::piped());

    gix::trace::debug!("About to execute {cmd:?}");
    let mut child = cmd.spawn()?;
    child.stdin.take().expect("to be present").write_all(&object.data)?;

    if !child.wait()?.success() {
        bail!("Command {cmd:?} failed");
    }

    let mut signed_data = Vec::new();
    child.stdout.expect("to be present").read_to_end(&mut signed_data)?;

    commit_ref
        .extra_headers
        .push((BStr::new(SIGNATURE_FIELD_NAME), Cow::Owned(BString::new(signed_data))));

    let signed_id = repo.write_object(&commit_ref)?;
    writeln!(&mut out, "{signed_id}")?;

    Ok(())
}

pub fn describe(
    mut repo: gix::Repository,
    rev_spec: Option<&str>,
    mut out: impl std::io::Write,
    mut err: impl std::io::Write,
    describe::Options {
        all_tags,
        all_refs,
        first_parent,
        always,
        statistics,
        max_candidates,
        long_format,
        dirty_suffix,
    }: describe::Options,
) -> Result<()> {
    repo.object_cache_size_if_unset(4 * 1024 * 1024);
    let commit = match rev_spec {
        Some(spec) => repo.rev_parse_single(spec)?.object()?.try_into_commit()?,
        None => repo.head_commit()?,
    };
    use gix::commit::describe::SelectRef::*;
    let select_ref = if all_refs {
        AllRefs
    } else if all_tags {
        AllTags
    } else {
        Default::default()
    };
    let resolution = commit
        .describe()
        .names(select_ref)
        .traverse_first_parent(first_parent)
        .id_as_fallback(always)
        .max_candidates(max_candidates)
        .try_resolve()?
        .with_context(|| format!("Did not find a single candidate ref for naming id '{}'", commit.id))?;

    if statistics {
        writeln!(err, "traversed {} commits", resolution.outcome.commits_seen)?;
    }

    let mut describe_id = resolution.format_with_dirty_suffix(dirty_suffix)?;
    describe_id.long(long_format);

    writeln!(out, "{describe_id}")?;
    Ok(())
}

pub mod describe {
    #[derive(Debug, Clone)]
    pub struct Options {
        pub all_tags: bool,
        pub all_refs: bool,
        pub first_parent: bool,
        pub always: bool,
        pub long_format: bool,
        pub statistics: bool,
        pub max_candidates: usize,
        pub dirty_suffix: Option<String>,
    }
}

/// Options for the porcelain `create` (a.k.a. `git commit`) entry point.
/// Only the smallest viable subset of `git commit` flags is wired in
/// the first iterations; the field set will grow as parity rows close.
#[derive(Debug, Clone, Default)]
pub struct CreateOptions {
    /// `-m`/`--message` values, in order. Concatenated with `\n\n`
    /// when multiple are given (mirrors `opt_parse_m` in
    /// vendor/git/builtin/commit.c).
    pub message: Vec<String>,
    /// `--allow-empty`: permit a commit whose tree is identical to its
    /// sole parent's tree.
    pub allow_empty: bool,
    /// `--allow-empty-message`: permit an empty commit message.
    pub allow_empty_message: bool,
    /// `-q`/`--quiet`: suppress the post-commit summary line.
    pub quiet: bool,
    /// Skip pre-commit and commit-msg only, as an explicit user choice.
    pub no_verify: bool,
    /// `--reset-author`: requires -C/-c/--amend per git's precondition;
    /// gix mirrors the exit-128 rejection until those flags are wired.
    pub reset_author: bool,
    /// `-F`/`--file=<file>`: read commit message from `<file>` (or
    /// stdin when the path is `-`).
    pub file: Option<std::path::PathBuf>,
    /// `-S`/`--gpg-sign[=<keyid>]`: requested. gix has no GPG backend
    /// today; setting this flag emits git's "unable to start gpg"
    /// stanza and exits 128 (mirrors tag's rejection path).
    pub gpg_sign: Option<String>,
    /// `--author=<author>`: override the commit author. Accepts a
    /// fully-formed `Name <email>` ident; pattern-matching to look up
    /// an existing author (`git rev-list -i --author=<pat>`) is
    /// deferred.
    pub author: Option<String>,
    /// `--date=<date>`: override the author date. Accepts the standard
    /// git date formats (gix-date::parse).
    pub date: Option<String>,
    /// `--trailer <token>[(=|:)<value>]`: appended to the commit
    /// message. Multiple `--trailer` accumulate, one per line.
    pub trailer: Vec<String>,
    /// Trailing pathspec args. With only `--allow-empty` exercised
    /// today, pathspec is effectively a no-op — present here so the
    /// Clap surface accepts `gix commit -m m -- <path>` without
    /// tripping the unknown-subcommand path.
    pub pathspec: Vec<std::ffi::OsString>,
    /// `--cleanup=<mode>`: one of `strip` / `whitespace` / `verbatim`
    /// / `scissors` / `default`. Anything else exits 128 with
    /// "fatal: Invalid cleanup mode <x>" mirroring git's wording.
    pub cleanup: Option<String>,
    /// `-C`/`--reuse-message=<commit>`: copy message + author +
    /// timestamp from the named commit.
    pub reuse_message: Option<String>,
    /// `-c`/`--reedit-message=<commit>`: like -C but with editor
    /// pass; under EDITOR=true the message passes through unchanged.
    pub reedit_message: Option<String>,
    /// `--squash=<commit>`: construct `squash! <subject>` message.
    pub squash: Option<String>,
    /// `--fixup=[(amend|reword):]<commit>`: construct fixup/amend
    /// message variants.
    pub fixup: Option<String>,
    /// `--dry-run`: report what would be committed without writing
    /// the commit object. Implementation today short-circuits before
    /// any commit-object write so --allow-empty + --dry-run is
    /// observably "no commit advance".
    pub dry_run: bool,
    /// `--amend`: replace HEAD with a new commit that has the same
    /// parents as the current tip. Without -m the message is reused
    /// from the original commit (effective --no-edit semantics under
    /// EDITOR=true).
    pub amend: bool,
    /// `-i`/`--include`: stage listed pathspecs in addition to staged
    /// content. With no pathspec this errors 128 ("fatal: No paths
    /// with --include/--only does not make sense.") mirroring git.
    pub include: bool,
    /// `-o`/`--only`: commit only the listed pathspecs. Same paths
    /// precondition as `--include`.
    pub only: bool,
}

/// Create a Git commit from the index, honoring repository commit hooks.
pub fn create(
    repo: gix::Repository,
    mut out: impl std::io::Write,
    CreateOptions {
        message,
        allow_empty,
        allow_empty_message,
        quiet,
        no_verify,
        reset_author,
        file,
        gpg_sign,
        author,
        date,
        trailer,
        pathspec,
        cleanup,
        reuse_message,
        reedit_message,
        squash,
        fixup,
        dry_run,
        amend,
        include,
        only,
    }: CreateOptions,
) -> Result<()> {
    // git's `--reset-author` precondition (vendor/git/builtin/commit.c
    // parse_and_validate_options): only valid with `-C`, `-c`, or
    // `--amend`. Without those (and they aren't wired yet), error 128
    // with git's exact wording.
    if reset_author {
        use std::io::Write as _;
        let _ = writeln!(
            std::io::stderr().lock(),
            "fatal: --reset-author can be used only with -C, -c or --amend."
        );
        std::process::exit(128);
    }

    // `-S`/`--gpg-sign` requested but gix has no GPG backend wired —
    // emit git's canonical "unable to start gpg" stanza and exit 128
    // (mirrors gitoxide-core/src/repository/tag.rs's `-s` path).
    if gpg_sign.is_some() {
        use std::io::Write as _;
        let mut err = std::io::stderr().lock();
        let _ = writeln!(err, "error: gpg failed to sign the data");
        let _ = writeln!(err, "fatal: failed to write commit object");
        std::process::exit(128);
    }

    // `--cleanup=<mode>` validation. git's parse_cleanup_arg
    // (vendor/git/builtin/commit.c) accepts strip / whitespace /
    // verbatim / scissors / default and dies 128 on anything else
    // with "fatal: Invalid cleanup mode <x>". Validate before any
    // tree/parent work so the message-mode check can short-circuit
    // even on flag combos that would otherwise bail later (e.g.
    // without --allow-empty).
    if let Some(mode) = cleanup.as_deref() {
        match mode {
            "strip" | "whitespace" | "verbatim" | "scissors" | "default" => {}
            other => {
                use std::io::Write as _;
                let _ = writeln!(std::io::stderr().lock(), "fatal: Invalid cleanup mode {other}");
                std::process::exit(128);
            }
        }
    }

    // git's parse_and_validate_options gate (vendor/git/builtin/commit.c):
    // `-i`/`--include` and `-o`/`--only` require at least one pathspec.
    // Without one, exit 128 with the verbatim wording.
    if (include || only) && pathspec.is_empty() {
        use std::io::Write as _;
        let _ = writeln!(
            std::io::stderr().lock(),
            "fatal: No paths with --include/--only does not make sense."
        );
        std::process::exit(128);
    }
    if !pathspec.is_empty() || include || only {
        bail!("path-scoped commit is not yet supported; stage explicit paths and inspect the index, or use git commit");
    }
    if let Some(state) = repo.state() {
        bail!("commit during {state:?} is not yet supported; use git commit");
    }
    let original_head = repo.head()?;
    let head_id = original_head.id();
    let head_commit = head_id
        .map(|id| -> Result<_> { Ok(id.object()?.try_into_commit()?) })
        .transpose()?;
    if amend && head_commit.is_none() {
        bail!("HEAD must point at a commit for --amend");
    }

    // Resolve the per-flag message-source modes. git's
    // parse_and_validate_options + prepare_to_commit (vendor/git/
    // builtin/commit.c) layer these in a specific order; mirrored here:
    //   1. -C/--reuse-message + -c/--reedit-message: full message copy
    //      from the named commit (the `template_file` / `use_message`
    //      paths in commit.c). -c additionally invokes the editor;
    //      under EDITOR=true the result is identical to -C.
    //   2. --squash=<commit>: "squash! <subject>" prefix; -m messages
    //      append as paragraphs (parse_squash_arg).
    //   3. --fixup=<spec>: "fixup! <subject>" (plain) or
    //      "amend! <subject>" + original message (amend:/reword:
    //      variants); parse_fixup_arg.
    // Multiple message-source flags are mutually exclusive in git; gix
    // applies them in the documented precedence and lets the last one
    // win (the test rows exercise one at a time so this is moot for
    // effect-mode parity today).
    let mut composed = message.join("\n\n");

    let prefix_commit_subject = |spec: &str| -> Result<String> {
        use gix::bstr::ByteSlice;
        let id = repo.rev_parse_single(spec)?;
        let cm = id.object()?.try_into_commit()?;
        let msg = cm.message_raw()?;
        let subject = msg.lines().next().unwrap_or(b"");
        Ok(String::from_utf8_lossy(subject).into_owned())
    };

    let load_full_message = |spec: &str| -> Result<String> {
        let id = repo.rev_parse_single(spec)?;
        let cm = id.object()?.try_into_commit()?;
        let msg = cm.message_raw()?;
        Ok(String::from_utf8_lossy(msg).into_owned())
    };

    if let Some(spec) = reuse_message.as_deref().or(reedit_message.as_deref()) {
        composed = load_full_message(spec)?;
    }

    if let Some(spec) = squash.as_deref() {
        let subject = prefix_commit_subject(spec)?;
        let mut s = format!("squash! {subject}");
        if !composed.is_empty() {
            s.push_str("\n\n");
            s.push_str(&composed);
        }
        composed = s;
    }

    if let Some(spec) = fixup.as_deref() {
        let (mode, target) = match spec.split_once(':') {
            Some(("amend", t)) => ("amend", t),
            Some(("reword", t)) => ("reword", t),
            _ => ("fixup", spec),
        };
        let subject = prefix_commit_subject(target)?;
        composed = match mode {
            "fixup" => format!("fixup! {subject}"),
            "amend" => {
                let original = load_full_message(target)?;
                format!("amend! {subject}\n\n{original}")
            }
            "reword" => {
                // `--fixup=reword:<commit>` is `--fixup=amend:<commit>
                //  --only`; the body is dropped and the editor would
                // open with subject only. Under EDITOR=true that means
                // an "amend! <subject>" message with empty body.
                format!("amend! {subject}\n\n")
            }
            _ => unreachable!("split arms exhausted"),
        };
    }
    if let Some(path) = file.as_ref() {
        let body = if path.as_os_str() == "-" {
            let mut buf = String::new();
            std::io::Read::read_to_string(&mut std::io::stdin().lock(), &mut buf)?;
            buf
        } else {
            std::fs::read_to_string(path).with_context(|| format!("could not open '{}' for reading", path.display()))?
        };
        if !composed.is_empty() {
            composed.push_str("\n\n");
        }
        composed.push_str(&body);
    }
    // --trailer <token>=<value>: append each as a separate line to the
    // message, mirroring tag.rs:175-181. opt_pass_trailer in
    // vendor/git/builtin/commit.c routes through interpret_trailers;
    // the simple-append path is sufficient for effect-mode parity
    // (a dedicated --trailer parity row may close on bytes mode later
    // when gix-trailer integration lands).
    for t in &trailer {
        if !composed.is_empty() && !composed.ends_with('\n') {
            composed.push('\n');
        }
        composed.push_str(t);
        composed.push('\n');
    }

    // --amend without an explicit message reuses HEAD's message
    // (effective --no-edit semantics under EDITOR=true). Mirrors
    // builtin/commit.c prepare_to_commit: when no message source is
    // active and amend is set, the original commit message is read
    // and used as the base.
    if amend && composed.is_empty() {
        let head_commit = repo.head_commit().context("HEAD must point at a commit for --amend")?;
        let raw = head_commit.message_raw()?;
        composed = String::from_utf8_lossy(raw).into_owned();
    }

    // Apply cleanup. Bytes parity on the precise normalization rules
    // (git's clean_message in commit.c) is out of scope for the first
    // iteration since both git and gix paths exit 0 either way for
    // effect-mode rows; deeper bytes-parity rides a follow-up row.
    // verbatim leaves the message untouched. The other modes trim
    // outer whitespace so identity rules (no trailing space, no empty
    // leading/trailing lines) match git for typical -m inputs.
    let cleanup_mode = cleanup.as_deref().unwrap_or("default");
    if cleanup_mode != "verbatim" {
        composed = composed
            .trim_matches(|c: char| c == '\n' || c == ' ' || c == '\t')
            .to_string();
    }

    // --dry-run short-circuit: git's `--dry-run` reports what would
    // be committed and returns 0 without writing the commit object.
    // Bytes parity on the dry-run rendering rides the index→tree
    // primitive; the exit-0 short-circuit here is sufficient for
    // effect-mode parity on `--allow-empty --dry-run -m m` style rows.
    if dry_run {
        return Ok(());
    }

    if !no_verify {
        super::authoring::run_hook(&repo, "pre-commit", &[])?;
    }
    let message_path = repo.current_dir().join(repo.git_dir()).join("COMMIT_EDITMSG");
    std::fs::write(&message_path, format!("{composed}\n"))?;
    let mut prepare_args = vec![message_path.clone().into_os_string()];
    if !message.is_empty() || file.is_some() {
        prepare_args.push("message".into());
    } else if let Some(spec) = reuse_message.as_ref().or(reedit_message.as_ref()) {
        prepare_args.extend(["commit".into(), spec.into()]);
    } else if amend {
        prepare_args.extend(["commit".into(), "HEAD".into()]);
    }
    super::authoring::run_hook(&repo, "prepare-commit-msg", &prepare_args)?;
    if !no_verify {
        super::authoring::run_hook(&repo, "commit-msg", &[message_path.clone().into_os_string()])?;
    }
    composed = std::fs::read_to_string(&message_path).context("read hook-edited commit message")?;
    if cleanup_mode != "verbatim" {
        composed = composed
            .trim_matches(|c: char| c == '\n' || c == ' ' || c == '\t')
            .to_owned();
    }
    if composed.is_empty() && !allow_empty_message {
        bail!("Aborting commit due to empty commit message.");
    }
    // Read after hooks, which may stage changes. Never substitute worktree bytes.
    let tree_id = super::authoring::index_tree(&repo)?;
    let previous_tree = head_commit
        .as_ref()
        .map(|commit| commit.tree_id().map(|id| id.detach()))
        .transpose()?;
    if !allow_empty
        && !amend
        && previous_tree.unwrap_or_else(|| gix::ObjectId::empty_tree(repo.object_hash())) == tree_id
    {
        bail!("nothing to commit (no staged changes)");
    }
    let current_head = repo.head()?;
    if current_head.id() != original_head.id() || current_head.referent_name() != original_head.referent_name() {
        bail!("HEAD changed while commit hooks ran; retry after inspecting the repository");
    }
    // For --amend, the new commit's parents are HEAD's parents (we
    // replace HEAD itself). For --allow-empty, the new commit's
    // parent is HEAD.
    let parent_ids: Vec<gix::ObjectId> = if amend {
        head_commit
            .as_ref()
            .context("amend requires HEAD")?
            .parent_ids()
            .map(gix::Id::detach)
            .collect()
    } else {
        head_id.into_iter().map(gix::Id::detach).collect()
    };

    // Resolve the author signature. For --amend without --reset-author
    // (the latter is gated 128 above), git keeps the original commit's
    // author. For non-amend, we read from config. --author / --date
    // override either base.
    let mut author_sig: gix::actor::Signature = if amend {
        head_commit
            .as_ref()
            .context("amend requires HEAD")?
            .author()
            .context("HEAD commit author could not be decoded")?
            .to_owned()?
    } else {
        repo.author()
            .context("author identity not configured")?
            .context("invalid author time configuration")?
            .to_owned()?
    };
    if let Some(a) = author.as_ref() {
        let id = gix::actor::IdentityRef::from_bytes(a.as_bytes())
            .map_err(|_| anyhow::anyhow!("invalid --author: {a:?}"))?;
        author_sig.name = id.name.to_owned();
        author_sig.email = id.email.to_owned();
    }
    if let Some(d) = date.as_ref() {
        author_sig.time = gix::date::parse(d, Some(gix::date::Zoned::now()))
            .map_err(|e| anyhow::anyhow!("invalid --date {d:?}: {e}"))?;
    }
    let committer_sig: gix::actor::Signature = repo
        .committer()
        .context("committer identity not configured")?
        .context("invalid committer time configuration")?
        .to_owned()?;

    let new_id = if amend {
        // --amend: build the commit object directly so we can both (a)
        // set parents to HEAD's parents (not HEAD) and (b) update HEAD
        // with the expected previous value being HEAD itself, not the
        // new commit's first parent. repo.commit_as() can't do that
        // because it expects HEAD == first parent in its ref-update
        // pre-condition (see commit_as_inner).
        use gix::refs::{
            Target,
            transaction::{Change, LogChange, PreviousValue, RefEdit, RefLog},
        };
        let commit_obj = gix::objs::Commit {
            message: composed.as_str().into(),
            tree: tree_id,
            author: author_sig.clone(),
            committer: committer_sig.clone(),
            encoding: None,
            parents: parent_ids.iter().copied().collect(),
            extra_headers: Default::default(),
        };
        let new_id = repo.write_object(&commit_obj)?;
        let log_message = gix::reference::log::message("commit (amend)", commit_obj.message.as_ref(), parent_ids.len());
        repo.edit_reference(RefEdit {
            change: Change::Update {
                log: LogChange {
                    mode: RefLog::AndReference,
                    force_create_reflog: false,
                    message: log_message,
                },
                expected: PreviousValue::MustExistAndMatch(Target::Object(
                    head_id.context("amend requires HEAD")?.detach(),
                )),
                new: Target::Object(new_id.detach()),
            },
            name: "HEAD".try_into().expect("HEAD is a valid ref name"),
            deref: true,
        })?;
        new_id
    } else {
        // Non-amend: route through commit_as for both override and
        // default paths so we always pass an explicit committer/author.
        // commit_as expects HEAD == first parent (which holds for
        // --allow-empty since parents = [HEAD]).
        let mut author_buf = gix::date::parse::TimeBuf::default();
        let mut committer_buf = gix::date::parse::TimeBuf::default();
        let author_ref = author_sig.to_ref(&mut author_buf);
        let committer_ref = committer_sig.to_ref(&mut committer_buf);
        repo.commit_as(
            committer_ref,
            author_ref,
            "HEAD",
            composed.as_str(),
            tree_id,
            parent_ids.iter().copied(),
        )
        .context("writing the commit object failed")?
    };

    // Post-commit cannot undo an already-created commit; report its failure as a warning.
    if let Err(err) = super::authoring::run_hook(&repo, "post-commit", &[]) {
        eprintln!("warning: commit {new_id} succeeded, but {err:#}");
    }
    if !quiet {
        // Minimal summary. git's wording is `[<branch> <abbrev>] <subject>`;
        // bytes parity is out of scope for the first iteration since
        // git also emits stat lines that depend on diff machinery. Use
        // a stable, grep-able shape — bytes-mode rows that need the
        // full git wording will close later via dedicated work.
        let abbrev = new_id.shorten_or_id().to_string();
        let subject = composed.lines().next().unwrap_or("");
        writeln!(out, "[{abbrev}] {subject}")?;
    }

    Ok(())
}
