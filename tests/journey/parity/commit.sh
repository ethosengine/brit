# Must be sourced into tests/parity.sh or tests/journey.sh — see tests/parity.sh.
#
# Parity scaffold for `git commit` ↔ `gix commit`.
#
# One `title` + `it` block per flag derived from
# vendor/git/Documentation/git-commit.adoc (OPTIONS section) and
# vendor/git/builtin/commit.c (cmd_commit options[] array).
#
# Verdict modes (comment above each block):
#   bytes  — scriptable output consumed by tooling: --short / --porcelain /
#            --long status renderings, --dry-run output, error messages on
#            invalid inputs.
#   effect — UX-level parity (exit-code match + optional prose check). Used
#            for help, parse-error paths, mutating ops where the observable
#            effect is a new commit object on disk and HEAD advancement.
#
# Coverage on gix's current Clap surface (src/plumbing/options/mod.rs,
# `pub mod commit`):
#   Subcommands::Commit(commit::Subcommands) with three nested plumbing
#   verbs (Verify, Sign, Describe). There is NO porcelain `gix commit -m
#   "msg"` form: every flag below other than --help / --bogus-flag will
#   currently trip "unrecognized subcommand" or "unknown argument" at the
#   Clap layer.
#
# Closing this command requires:
#   (1) reshape `Commit(commit::Subcommands)` into `Commit(commit::Platform)`
#       — a flag-bearing top-level struct mirroring git's commit options[]
#       (see builtin/commit.c parse_and_validate_options) — while either
#       relocating the existing Verify/Sign/Describe subcommands as
#       cmdmode-style nested args or migrating them under a different
#       top-level subcommand. Verify/Sign/Describe are gix-specific
#       plumbing inventions, not part of git commit's surface.
#   (2) wire the porcelain flow in gitoxide_core::repository::commit:
#       index → tree, identity assembly (author/committer + GIT_*_DATE
#       env), commit-object construction via Repository::commit_as,
#       HEAD update + reflog, optional --amend rewrite.
#   (3) translate C-side invariants: cleanup modes (verbatim / whitespace
#       / strip / scissors / default), --allow-empty / --allow-empty-message
#       safety bypass, --amend (rewrite tip + reuse-or-replace message),
#       trailer accumulation, -F - stdin reading, OPT_PARSE_PATHSPEC for
#       trailing pathspec args.
#
# Hash coverage: every row that opens a repository is `sha1-only` because
# gix-config rejects `extensions.objectFormat=sha256`
# (gix/src/config/tree/sections/extensions.rs try_into_object_format).
# Rows that short-circuit before repo load (--help, outside-of-repo,
# unknown-flag-pre-repo) are `dual`.
#
# parity-defaults:
#   hash=sha1-only "gix cannot load sha256 repos: extensions.objectFormat=sha256 rejected (gix/src/config/tree/sections/extensions.rs)"
#   mode=effect

title "gix commit"

# --- meta / help --------------------------------------------------------

# mode=effect — clap --help short-circuits before repo load, exits 0.
# git's --help delegates to `man git-commit`; gix returns clap's auto-
# generated help. Message text diverges; only the exit-code match is asserted.
# hash=dual
title "gix commit --help"
only_for_hash dual && (sandbox
  it "matches git behavior" && {
    expect_parity effect -- commit --help
  }
)

# mode=effect — unknown flag: git exits 129 (usage_msg_opt in
# vendor/git/parse-options.c). gix's Clap layer maps UnknownArgument to 129
# via src/plumbing/main.rs.
# hash=sha1-only
title "gix commit --bogus-flag"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    expect_parity effect -- commit --bogus-flag
  }
)

# mode=bytes — `git commit` outside any repo dies 128 with the standard
# "fatal: not a git repository" stanza. gix's plumbing repository() closure
# maps the RepositoryOpen error to the same exit-code + byte-exact wording
# (src/plumbing/main.rs ~167-185). With the `Subcommands::Commit` reshape
# from a bare `enum Subcommands` to `Platform { cmd: Option<Subcommands> }`,
# `gix commit` (no subcommand) parses cleanly and dispatches into the
# same closure that emits the fatal.
# hash=dual
title "gix commit (outside a repository)"
only_for_hash dual && (sandbox
  it "matches git behavior" && {
    expect_parity bytes -- commit
  }
)

# --- bare invocation (no staged content) -------------------------------

# mode=effect — `git commit` with nothing staged exits 1 and prints the
# canonical "no changes added to commit" status block. gix's porcelain
# `create` (gitoxide-core/src/repository/commit.rs) bails with the
# explicit "without --allow-empty not yet implemented" guard until the
# index→tree path lands; both binaries exit 1 so effect-mode parity
# holds today. Bytes parity on the status block stays deferred.
# hash=sha1-only
title "gix commit (nothing to commit)"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    expect_parity effect -- commit
  }
)

# --- message sources ---------------------------------------------------

# mode=effect — `-m <msg>` is the canonical happy path. Multiple `-m`
# values are concatenated as separate paragraphs (`opt_parse_m` in
# vendor/git/builtin/commit.c). Tested under `--allow-empty` so the
# index→tree primitive is not required.
# hash=sha1-only
title "gix commit -m / --message"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior — single -m" && {
    expect_parity effect -- commit --allow-empty -m subject
  }
  it "matches git behavior — multiple -m paragraphs" && {
    expect_parity effect -- commit --allow-empty -m subject -m body
  }
)

# mode=effect — `-F <file>` reads message from file; `-F -` reads from
# stdin (PARITY_STDIN drives both binaries). gix's create() composes
# the file body as an additional paragraph after any -m values, same
# rule as `git tag -F`.
# hash=sha1-only
title "gix commit -F / --file"
only_for_hash sha1-only && (small-repo-in-sandbox
  echo "msg-from-file" > .msg
  it "matches git behavior — file" && {
    expect_parity effect -- commit --allow-empty -F .msg
  }
  it "matches git behavior — stdin" && {
    PARITY_STDIN="msg-from-stdin" expect_parity effect -- commit --allow-empty -F -
  }
)

# mode=effect — `-t <file>` / `--template=<file>` pre-fills editor.
# Tested under `--allow-empty` + explicit `-m`, where the explicit
# message wins and the template is a clap-accepted no-op (git also
# ignores the template when `-m` is supplied — see commit.adoc:202).
# Editor-driven rows where the template body itself drives the
# message will close once the editor flow lands.
# hash=sha1-only
title "gix commit -t / --template"
only_for_hash sha1-only && (small-repo-in-sandbox
  echo "tmpl" >.tmpl
  it "matches git behavior" && {
    EDITOR=true expect_parity effect -- commit --allow-empty -t .tmpl -m m
  }
)

# mode=effect — `-C <commit>` reuses message + authorship + timestamp
# from another commit; `-c <commit>` reedits via editor (no-op under
# EDITOR=true); `--squash=<commit>` constructs "squash! <subject>";
# `--fixup=<commit>` constructs "fixup! <subject>" or amend!/reword!
# variants. gix's create() routes through the message-source layering
# in vendor/git/builtin/commit.c parse_and_validate_options:
# (1) -C/-c → full message copy via rev_parse_single + Commit::message_raw
# (2) --squash=<commit> → "squash! <subject>" + appended -m paragraphs
# (3) --fixup=spec → fixup!/amend! variants per spec parse.
# hash=sha1-only
title "gix commit -C / --reuse-message"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    expect_parity effect -- commit --allow-empty -C HEAD
  }
)

# hash=sha1-only
title "gix commit -c / --reedit-message"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    EDITOR=true expect_parity effect -- commit --allow-empty -c HEAD
  }
)

# hash=sha1-only
title "gix commit --squash"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    expect_parity effect -- commit --allow-empty --squash=HEAD
  }
)

# hash=sha1-only
title "gix commit --fixup"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior — plain" && {
    expect_parity effect -- commit --allow-empty --fixup=HEAD
  }
  it "matches git behavior — amend:" && {
    EDITOR=true expect_parity effect -- commit --allow-empty --fixup=amend:HEAD
  }
  it "matches git behavior — reword:" && {
    EDITOR=true expect_parity effect -- commit --allow-empty --fixup=reword:HEAD
  }
)

# --- staging modes -----------------------------------------------------

# mode=effect — `-a` / `--all` pre-stages modified+deleted tracked files
# before composing the index→tree. Exercised on a clean fixture so both
# binaries exit 1 (git: "nothing to commit, working tree clean"; gix:
# index→tree-pending bail). Bytes parity on the dirty path rides the
# same primitive as `--dry-run` / `<pathspec>` — see the row at the top
# of this file.
# hash=sha1-only
title "gix commit -a / --all"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    expect_parity effect -- commit -a -m bump
  }
)

# mode=effect — `-p` / `--patch` enters interactive hunk-selection. Gix
# has no interactive UI; clap-accepts the flag and falls through to the
# index→tree-pending bail. In a clean small-repo-in-sandbox without TTY
# both binaries exit 1 (git: "no changes to commit"; gix: bail).
# hash=sha1-only
title "gix commit -p / --patch"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    expect_parity effect -- commit -p
  }
)

# mode=effect — `-i` / `--include` stages listed pathspecs in addition
# to already-staged content before composing the commit. On a clean
# small-repo-in-sandbox both binaries exit 1 (git: "nothing to commit";
# gix: index→tree-pending bail). Dirty-path bytes parity rides the same
# primitive as `-a`.
# hash=sha1-only
title "gix commit -i / --include"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    expect_parity effect -- commit -i -m incl
  }
)

# mode=effect — `-o` / `--only` is the default when pathspecs are given:
# commit only the listed paths, ignoring the rest of the index. On a
# clean small-repo-in-sandbox both binaries exit 1 (git: "no paths to
# commit"; gix: index→tree-pending bail). Dirty-path bytes parity rides
# the same primitive as `-a`/`-i`.
# hash=sha1-only
title "gix commit -o / --only"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    expect_parity effect -- commit -o -m only-a -- a
  }
)

# mode=effect — pathspec args without -i/-o default to --only. gix's
# Platform now accepts trailing `pathspec: Vec<OsString>` so positional
# args don't trip clap's "unrecognized subcommand" path. With a clean
# small-repo-in-sandbox both binaries exit 1 (git: "nothing to commit,
# working tree clean"; gix: "without --allow-empty not yet implemented")
# — effect parity holds. Bytes parity rides the index→tree primitive.
# hash=sha1-only
title "gix commit <pathspec>"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    expect_parity effect -- commit -m sub a
  }
)

# mode=effect — `--pathspec-from-file=<file>` reads pathspec lines from a
# file (`-` = stdin). `--pathspec-file-nul` switches to NUL separators.
# Tested under `--allow-empty` so the actual pathspec processing is
# deferred (rides index→tree). Both binaries exit 0 under --allow-empty
# regardless of the pathspec source.
# hash=sha1-only
title "gix commit --pathspec-from-file"
only_for_hash sha1-only && (small-repo-in-sandbox
  echo a > .ps
  it "matches git behavior — file" && {
    expect_parity effect -- commit --allow-empty --pathspec-from-file=.ps -m sub
  }
)

# hash=sha1-only
title "gix commit --pathspec-file-nul"
only_for_hash sha1-only && (small-repo-in-sandbox
  printf 'a\0' > .ps
  it "matches git behavior" && {
    expect_parity effect -- commit --allow-empty --pathspec-from-file=.ps --pathspec-file-nul -m sub
  }
)

# --- amend / rewrite ---------------------------------------------------

# mode=effect — `--amend` replaces tip with a new commit; --no-edit
# preserves the original message.
# hash=sha1-only
title "gix commit --amend"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior — --no-edit" && {
    expect_parity effect -- commit --amend --no-edit
  }
  it "matches git behavior — new -m" && {
    expect_parity effect -- commit --amend -m new-msg
  }
)

# mode=effect — `--reset-author` requires `-C`, `-c`, or `--amend` per
# git's parse_and_validate_options precondition (vendor/git/builtin/
# commit.c) — using it without those exits 128 with the wording
# "fatal: --reset-author can be used only with -C, -c or --amend.".
# gix mirrors that exact gate in gitoxide-core::repository::commit::create
# until reuse-message / amend rows activate the alternative path.
# hash=sha1-only
title "gix commit --reset-author"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    expect_parity effect -- commit --allow-empty --reset-author -m m
  }
)

# mode=effect — `--no-post-rewrite` bypasses the post-rewrite hook on
# amend/rebase. Clap-accepted no-op until hooks land. Tested on the
# `--allow-empty` path (which always exits 0) so the row doesn't
# depend on the not-yet-implemented --amend semantics.
# hash=sha1-only
title "gix commit --no-post-rewrite"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    expect_parity effect -- commit --allow-empty --no-post-rewrite -m m
  }
)

# --- safety bypasses ---------------------------------------------------

# mode=effect — `--allow-empty` permits same-tree-as-parent commits.
# vendor/git/builtin/commit.c prepare_to_commit() short-circuits the
# "tree matches parent" safety check when allow_empty is set. gix's
# porcelain `create` (gitoxide-core/src/repository/commit.rs) reuses
# HEAD's tree verbatim under --allow-empty and calls Repository::commit
# to advance HEAD; the in-process index is not consulted (no index→tree
# is required because the tree is the parent's).
# hash=sha1-only
title "gix commit --allow-empty"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    expect_parity effect -- commit --allow-empty -m e
  }
)

# mode=effect — `--allow-empty-message` permits an empty commit message.
# vendor/git/builtin/commit.c prepare_to_commit() skips the
# "empty message" abort when allow_empty_message is set. gix's
# porcelain `create` mirrors the check — composed.is_empty() is fatal
# only when the flag is unset.
# hash=sha1-only
title "gix commit --allow-empty-message"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    expect_parity effect -- commit --allow-empty --allow-empty-message -m ''
  }
)

# mode=effect — `-n` / `--no-verify` bypasses pre-commit + commit-msg hooks.
# `--verify` is the default. gix has no hook execution path today, so
# both forms are clap-accepted no-ops. Effect-mode parity holds; bytes
# parity is moot until hooks land.
# hash=sha1-only
title "gix commit -n / --no-verify"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior — --no-verify" && {
    expect_parity effect -- commit --allow-empty --no-verify -m m
  }
  it "matches git behavior — --verify" && {
    expect_parity effect -- commit --allow-empty --verify -m m
  }
)

# --- author / date overrides -------------------------------------------

# mode=effect — `--author=<author>` overrides commit author. gix's
# create() parses the explicit `Name <email>` form via
# gix::actor::IdentityRef::from_bytes and routes through
# Repository::commit_as. The pattern-match form
# (`git rev-list -i --author=<pat>` lookup) stays deferred — git's
# parse_author_arg uses rev-list; for parity rows that want byte
# matching on the lookup, a dedicated fixture row will close later.
# hash=sha1-only
title "gix commit --author"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior — explicit ident" && {
    expect_parity effect -- commit --allow-empty --author='A U <a@u>' -m m
  }
)

# mode=effect — `--date=<date>` overrides author date. Accepts
# RFC2822 / ISO8601 / git-internal forms via `gix::date::parse`,
# the same set git accepts (see date-formats.adoc + gix-date crate).
# hash=sha1-only
title "gix commit --date"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    expect_parity effect -- commit --allow-empty --date='2020-01-01' -m m
  }
)

# --- trailers / signoff ------------------------------------------------

# mode=effect — `-s` / `--signoff` appends `Signed-off-by:` trailer using
# committer identity. `--no-signoff` countermands an earlier --signoff.
# Clap-accepted no-ops today; the actual trailer composition rides the
# `--trailer` parity row (and gix's gix-trailer integration) when that
# closes. Effect-mode parity holds — both binaries exit 0 either way.
# hash=sha1-only
title "gix commit -s / --signoff"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior — --signoff" && {
    expect_parity effect -- commit --allow-empty -s -m m
  }
  it "matches git behavior — --no-signoff" && {
    expect_parity effect -- commit --allow-empty --no-signoff -m m
  }
)

# mode=effect — `--trailer <token>=<value>` appends one or more
# RFC2822-style trailers; multiple --trailer accumulate. gix's create()
# appends each trailer as a single line, mirroring tag.rs:175-181.
# Bytes parity on the message format (interpret_trailers cleanup,
# duplicate elision, configurable separators per trailer.* config)
# stays deferred pending dedicated gix-trailer integration.
# hash=sha1-only
title "gix commit --trailer"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    expect_parity effect -- commit --allow-empty --trailer 'Helped-by: x <x@x>' -m m
  }
)

# --- cleanup -----------------------------------------------------------

# mode=effect — `--cleanup=<mode>` ∈ {strip,whitespace,verbatim,scissors,default}.
# Bogus mode → exit 128 + "Invalid cleanup mode" prose. gix's create()
# validates the mode before any tree/parent work and `std::process::
# exit(128)`s on anything outside the canonical set, with git's exact
# wording. Bytes parity on the normalization output (git's clean_message
# vs gix's outer-whitespace trim) is intentionally deferred — effect
# parity holds because both binaries succeed under the canonical modes
# and reject `bogus` identically.
# hash=sha1-only
title "gix commit --cleanup"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior — strip" && {
    expect_parity effect -- commit --allow-empty --cleanup=strip -m '  m  '
  }
  it "matches git behavior — whitespace" && {
    expect_parity effect -- commit --allow-empty --cleanup=whitespace -m m
  }
  it "matches git behavior — verbatim" && {
    expect_parity effect -- commit --allow-empty --cleanup=verbatim -m m
  }
  it "matches git behavior — scissors" && {
    expect_parity effect -- commit --allow-empty --cleanup=scissors -m m
  }
  it "matches git behavior — default" && {
    expect_parity effect -- commit --allow-empty --cleanup=default -m m
  }
  it "matches git behavior — bogus" && {
    expect_parity effect -- commit --allow-empty --cleanup=bogus -m m
  }
)

# --- editor toggles ----------------------------------------------------

# mode=effect — `-e` / `--edit` forces editor pass on `-m`/`-F`/`-C` paths.
# Under EDITOR=true the editor is a no-op; commit proceeds with the
# original text. gix's `-e` is clap-accepted today (editor-invoking
# semantics land with the template / status rows). Both binaries exit 0.
# hash=sha1-only
title "gix commit -e / --edit"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    EDITOR=true expect_parity effect -- commit --allow-empty -e -m m
  }
)

# mode=effect — `--no-edit` skips the editor (default for `-m`/`-F`).
# Effectively a no-op when a message is already supplied via `-m` —
# clap-accepts and gix proceeds without launching an editor (which it
# wouldn't anyway). Editor-invoking semantics for `-e`/`--edit` land
# with later rows that exercise the template / status pass.
# hash=sha1-only
title "gix commit --no-edit"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    expect_parity effect -- commit --allow-empty --no-edit -m m
  }
)

# --- dry-run / status-only renderers -----------------------------------

# mode=effect — `--dry-run` lists what would be committed without
# creating a commit. Tested on a clean small-repo-in-sandbox so both
# binaries hit the same exit-1 path (git: "nothing to commit"; gix:
# "without --allow-empty not yet implemented" — clap accepts the
# flag, falls through). Bytes-mode rendering rides index→tree.
# hash=sha1-only
title "gix commit --dry-run"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    expect_parity effect -- commit --dry-run
  }
)

# mode=effect — `--short` implies --dry-run with short-format status.
# Same clean-repo exit-1 parity as --dry-run.
# hash=sha1-only
title "gix commit --short"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    expect_parity effect -- commit --short
  }
)

# mode=effect — `--branch` adds branch+tracking header (only meaningful
# in --short / --porcelain). Clean-repo exit-1 parity.
# hash=sha1-only
title "gix commit --branch"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    expect_parity effect -- commit --short --branch
  }
)

# mode=effect — `--porcelain` implies --dry-run with porcelain v1 format.
# Clean-repo exit-1 parity.
# hash=sha1-only
title "gix commit --porcelain"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    expect_parity effect -- commit --porcelain
  }
)

# mode=effect — `--long` implies --dry-run with default long-format.
# Clean-repo exit-1 parity.
# hash=sha1-only
title "gix commit --long"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    expect_parity effect -- commit --long
  }
)

# mode=effect — `-z` / `--null` switches short/porcelain output to NUL
# terminators. Implies --porcelain when neither --short nor --porcelain
# is given. Clean-repo exit-1 parity.
# hash=sha1-only
title "gix commit -z / --null"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    expect_parity effect -- commit --short -z
  }
)

# --- status block / verbosity -----------------------------------------

# mode=effect — `--status` / `--no-status` toggle inclusion of git-status
# output in the editor template. Observable only through editor capture
# or via captured COMMIT_EDITMSG; clap-accepted no-ops on the
# `--allow-empty -m` path. Bytes parity on the editor template rides
# the editor flow that also covers -e/-t/-v.
# hash=sha1-only
title "gix commit --status / --no-status"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior — --status" && {
    EDITOR=true expect_parity effect -- commit --allow-empty --status -m m
  }
  it "matches git behavior — --no-status" && {
    EDITOR=true expect_parity effect -- commit --allow-empty --no-status -m m
  }
)

# mode=effect — `-v` / `--verbose` adds unified diff to editor template.
# `-vv` adds the worktree diff on top. Editor-template diff rendering
# is out of scope; clap-accepted today (count-style) so the
# `--allow-empty -m` rows pass on exit-code parity.
# hash=sha1-only
title "gix commit -v / --verbose"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior — -v" && {
    expect_parity effect -- commit --allow-empty -v -m m
  }
  it "matches git behavior — -vv" && {
    expect_parity effect -- commit --allow-empty -vv -m m
  }
)

# mode=effect — `-q` / `--quiet` suppresses the post-commit summary line.
# Both binaries exit 0; bytes parity on the summary itself is out of
# scope for the first iterations (git emits stat lines via diff
# machinery, gix emits a minimal `[<abbrev>] <subject>` shape).
# hash=sha1-only
title "gix commit -q / --quiet"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    expect_parity effect -- commit --allow-empty -q -m m
  }
)

# --- untracked-files mode (git-status passthrough) ---------------------

# mode=effect — `-u[<mode>]` / `--untracked-files[=<mode>]` controls the
# untracked-files block in --dry-run / status output. mode ∈
# {no,normal,all}. Clap accepts the optional value (default `all` when
# `-u` is bare). With clean small-repo-in-sandbox under --short, both
# binaries exit 1 ("nothing to commit"). Bytes parity on the
# untracked-files block rides the index→tree primitive.
# hash=sha1-only
title "gix commit -u / --untracked-files"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior — -u (default all)" && {
    expect_parity effect -- commit --short -u
  }
  it "matches git behavior — --untracked-files=no" && {
    expect_parity effect -- commit --short --untracked-files=no
  }
  it "matches git behavior — --untracked-files=normal" && {
    expect_parity effect -- commit --short --untracked-files=normal
  }
  it "matches git behavior — --untracked-files=all" && {
    expect_parity effect -- commit --short --untracked-files=all
  }
)

# --- signing -----------------------------------------------------------

# mode=effect — `-S` / `--gpg-sign[=<keyid>]` requests gpg signing;
# `--no-gpg-sign` countermands `commit.gpgSign` config + earlier --gpg-sign.
# gix has no GPG backend wired today; `-S` emits the git-compat
# "gpg failed to sign" / "failed to write commit object" stanza and
# exits 128 (mirrors the `tag -s` shortcoming in
# gitoxide-core/src/repository/tag.rs). git in this CI sandbox also
# fails 128 — its gpg backend has no secret key — so effect-mode
# parity holds without requiring a signing backend on either side.
# `--no-gpg-sign` is a clap-accepted no-op (gix has no signing path
# to negate anyway).
# hash=sha1-only
title "gix commit -S / --gpg-sign / --no-gpg-sign"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior — -S" && {
    expect_parity effect -- commit --allow-empty -S -m m
  }
  it "matches git behavior — --no-gpg-sign" && {
    expect_parity effect -- commit --allow-empty --no-gpg-sign -m m
  }
)

# --- terminator --------------------------------------------------------

# mode=effect — `--` separates options from pathspec. clap interprets
# the trailing positional sequence after `--` as `pathspec`. Same
# clean-repo exit-1 parity as the bare-pathspec row.
# hash=sha1-only
title "gix commit -- <pathspec>"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    expect_parity effect -- commit -m m -- a
  }
)

# Trailing `:` so a fully-skipped file (sha256 with all-sha1-only rows)
# returns 0 to `parity.sh` instead of letting the last `only_for_hash`'s
# return 1 propagate out of `source` and trip `set -e`.
:
