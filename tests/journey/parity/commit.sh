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
title "gix commit --bogus-flag"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    expect_parity effect -- commit --bogus-flag
  }
)

# mode=bytes — `git commit` outside any repo dies 128 with the standard
# "fatal: not a git repository" stanza. gix's plumbing repository() closure
# maps the RepositoryOpen error to the same exit-code.
# hash=dual
title "gix commit (outside a repository)"
only_for_hash dual && (sandbox
  it "TODO: matches git behavior" && {
    : # expect_parity bytes -- commit -m msg
  }
)

# --- bare invocation (no staged content) -------------------------------

# mode=effect — `git commit` with nothing staged exits 1 and prints the
# canonical "no changes added to commit" status block. gix should match
# the exit code; bytes parity on the prose is compat-deferred (status
# rendering is wide).
title "gix commit (nothing to commit)"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "TODO: matches git behavior" && {
    : # expect_parity effect -- commit -m msg
  }
)

# --- message sources ---------------------------------------------------

# mode=effect — `-m <msg>` is the canonical happy path: stage content,
# `gix commit -m "msg"`, observe a new commit on HEAD with that message
# as both subject and body's only paragraph. Multiple `-m` concatenate
# as separate paragraphs.
title "gix commit -m / --message"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "TODO: matches git behavior — single -m" && {
    : # expect_parity effect -- commit --allow-empty -m subject
  }
  it "TODO: matches git behavior — multiple -m paragraphs" && {
    : # expect_parity effect -- commit --allow-empty -m subject -m body
  }
)

# mode=effect — `-F <file>` reads message from file; `-F -` reads from
# stdin. Stdin variant requires expect_parity_reset stdin plumbing.
title "gix commit -F / --file"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "TODO: matches git behavior — file" && {
    : # echo "msg" >.msg && expect_parity effect -- commit --allow-empty -F .msg
  }
  it "TODO: matches git behavior — stdin" && {
    : # PARITY_STDIN="msg" expect_parity effect -- commit --allow-empty -F -
  }
)

# mode=effect — `-t <file>` / `--template=<file>` pre-fills editor. Under
# EDITOR=true (no edit), git aborts with "Aborting commit due to empty
# commit message"; same path expected for gix once -t lands.
title "gix commit -t / --template"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "TODO: matches git behavior" && {
    : # echo "tmpl" >.tmpl && EDITOR=true expect_parity effect -- commit --allow-empty -t .tmpl
  }
)

# mode=effect — `-C <commit>` reuses message + authorship + timestamp
# from another commit; `-c <commit>` reedits via editor; `--squash=<commit>`
# constructs a "squash! <subject>" message; `--fixup=<commit>` constructs
# "fixup! <subject>" (or "amend!" / "amend! reword:" prefixes for
# amend/reword variants).
title "gix commit -C / --reuse-message"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "TODO: matches git behavior" && {
    : # expect_parity effect -- commit --allow-empty -C HEAD
  }
)

title "gix commit -c / --reedit-message"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "TODO: matches git behavior" && {
    : # EDITOR=true expect_parity effect -- commit --allow-empty -c HEAD
  }
)

title "gix commit --squash"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "TODO: matches git behavior" && {
    : # expect_parity effect -- commit --allow-empty --squash=HEAD
  }
)

title "gix commit --fixup"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "TODO: matches git behavior — plain" && {
    : # expect_parity effect -- commit --allow-empty --fixup=HEAD
  }
  it "TODO: matches git behavior — amend:" && {
    : # EDITOR=true expect_parity effect -- commit --allow-empty --fixup=amend:HEAD
  }
  it "TODO: matches git behavior — reword:" && {
    : # EDITOR=true expect_parity effect -- commit --allow-empty --fixup=reword:HEAD
  }
)

# --- staging modes -----------------------------------------------------

# mode=effect — `-a` / `--all` pre-stages modified+deleted tracked files
# before composing the index→tree.
title "gix commit -a / --all"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "TODO: matches git behavior" && {
    : # echo more >> a && expect_parity effect -- commit -a -m bump
  }
)

# mode=effect — `-p` / `--patch` enters interactive hunk-selection. Gix
# has no interactive UI yet; iteration must decide deferred vs minimal
# stub (e.g. accept-then-error like rebase does).
title "gix commit -p / --patch"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "TODO: matches git behavior" && {
    : # expect_parity effect -- commit -p
  }
)

# mode=effect — `-i` / `--include` stages listed pathspecs in addition
# to already-staged content before composing the commit.
title "gix commit -i / --include"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "TODO: matches git behavior" && {
    : # touch new && expect_parity effect -- commit -i -m incl new
  }
)

# mode=effect — `-o` / `--only` is the default when pathspecs are given:
# commit only the listed paths, ignoring the rest of the index.
title "gix commit -o / --only"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "TODO: matches git behavior" && {
    : # echo more >> a && expect_parity effect -- commit -o -m only-a a
  }
)

# mode=effect — pathspec args without -i/-o default to --only.
title "gix commit <pathspec>"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "TODO: matches git behavior" && {
    : # echo more >> a && expect_parity effect -- commit -m sub a
  }
)

# mode=effect — `--pathspec-from-file=<file>` reads pathspec lines from a
# file (`-` = stdin). `--pathspec-file-nul` switches to NUL separators.
title "gix commit --pathspec-from-file"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "TODO: matches git behavior — file" && {
    : # echo a > .ps && echo more >> a && expect_parity effect -- commit --pathspec-from-file=.ps -m sub
  }
)

title "gix commit --pathspec-file-nul"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "TODO: matches git behavior" && {
    : # printf 'a\0' > .ps && echo more >> a && expect_parity effect -- commit --pathspec-from-file=.ps --pathspec-file-nul -m sub
  }
)

# --- amend / rewrite ---------------------------------------------------

# mode=effect — `--amend` replaces tip with a new commit; --no-edit
# preserves the original message.
title "gix commit --amend"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "TODO: matches git behavior — --no-edit" && {
    : # expect_parity effect -- commit --amend --no-edit
  }
  it "TODO: matches git behavior — new -m" && {
    : # expect_parity effect -- commit --amend -m new-msg
  }
)

# mode=effect — `--reset-author` together with -C/-c/--amend declares the
# resulting commit's authorship belongs to the committer.
title "gix commit --reset-author"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "TODO: matches git behavior" && {
    : # expect_parity effect -- commit --amend --no-edit --reset-author
  }
)

# mode=effect — `--no-post-rewrite` bypasses the post-rewrite hook on
# amend/rebase. Effectively a no-op until hooks land.
title "gix commit --no-post-rewrite"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "TODO: matches git behavior" && {
    : # expect_parity effect -- commit --amend --no-edit --no-post-rewrite
  }
)

# --- safety bypasses ---------------------------------------------------

# mode=effect — `--allow-empty` permits same-tree-as-parent commits.
title "gix commit --allow-empty"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "TODO: matches git behavior" && {
    : # expect_parity effect -- commit --allow-empty -m e
  }
)

# mode=effect — `--allow-empty-message` permits an empty commit message.
title "gix commit --allow-empty-message"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "TODO: matches git behavior" && {
    : # expect_parity effect -- commit --allow-empty --allow-empty-message -m ''
  }
)

# mode=effect — `-n` / `--no-verify` bypasses pre-commit + commit-msg hooks.
# `--verify` is the default. Both should accept and exit identically until
# hooks land.
title "gix commit -n / --no-verify"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "TODO: matches git behavior — --no-verify" && {
    : # expect_parity effect -- commit --allow-empty --no-verify -m m
  }
  it "TODO: matches git behavior — --verify" && {
    : # expect_parity effect -- commit --allow-empty --verify -m m
  }
)

# --- author / date overrides -------------------------------------------

# mode=effect — `--author=<author>` overrides committer-as-author. Pattern
# matching for partial-author lookup (`git rev-list -i --author=<pat>`)
# is a separate, larger feature.
title "gix commit --author"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "TODO: matches git behavior — explicit ident" && {
    : # expect_parity effect -- commit --allow-empty --author='A U <a@u>' -m m
  }
)

# mode=effect — `--date=<date>` overrides author date. Accepts
# RFC2822 / ISO8601 / git-internal forms (see date-formats.adoc).
title "gix commit --date"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "TODO: matches git behavior" && {
    : # expect_parity effect -- commit --allow-empty --date='2020-01-01' -m m
  }
)

# --- trailers / signoff ------------------------------------------------

# mode=effect — `-s` / `--signoff` appends `Signed-off-by:` trailer using
# committer identity. `--no-signoff` countermands an earlier --signoff.
title "gix commit -s / --signoff"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "TODO: matches git behavior — --signoff" && {
    : # expect_parity effect -- commit --allow-empty -s -m m
  }
  it "TODO: matches git behavior — --no-signoff" && {
    : # expect_parity effect -- commit --allow-empty --no-signoff -m m
  }
)

# mode=effect — `--trailer <token>=<value>` appends one or more
# RFC2822-style trailers; multiple --trailer accumulate. Implementation
# composes via gix-trailer / interpret-trailers semantics.
title "gix commit --trailer"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "TODO: matches git behavior" && {
    : # expect_parity effect -- commit --allow-empty --trailer 'Helped-by: x <x@x>' -m m
  }
)

# --- cleanup -----------------------------------------------------------

# mode=effect — `--cleanup=<mode>` ∈ {strip,whitespace,verbatim,scissors,default}.
# Bogus mode → exit 128 + "Invalid cleanup mode" prose.
title "gix commit --cleanup"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "TODO: matches git behavior — strip" && {
    : # expect_parity effect -- commit --allow-empty --cleanup=strip -m '  m  '
  }
  it "TODO: matches git behavior — whitespace" && {
    : # expect_parity effect -- commit --allow-empty --cleanup=whitespace -m m
  }
  it "TODO: matches git behavior — verbatim" && {
    : # expect_parity effect -- commit --allow-empty --cleanup=verbatim -m m
  }
  it "TODO: matches git behavior — scissors" && {
    : # expect_parity effect -- commit --allow-empty --cleanup=scissors -m m
  }
  it "TODO: matches git behavior — default" && {
    : # expect_parity effect -- commit --allow-empty --cleanup=default -m m
  }
  it "TODO: matches git behavior — bogus" && {
    : # expect_parity effect -- commit --allow-empty --cleanup=bogus -m m
  }
)

# --- editor toggles ----------------------------------------------------

# mode=effect — `-e` / `--edit` forces editor pass on `-m`/`-F`/`-C` paths.
# Under EDITOR=true editor is a no-op; commit proceeds with original text.
title "gix commit -e / --edit"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "TODO: matches git behavior" && {
    : # EDITOR=true expect_parity effect -- commit --allow-empty -e -m m
  }
)

# mode=effect — `--no-edit` skips the editor (default for `-m`/`-F`).
title "gix commit --no-edit"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "TODO: matches git behavior" && {
    : # expect_parity effect -- commit --allow-empty --no-edit -m m
  }
)

# --- dry-run / status-only renderers -----------------------------------

# mode=bytes — `--dry-run` lists what would be committed without creating
# a commit. Output mirrors git status long-format.
title "gix commit --dry-run"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "TODO: matches git behavior" && {
    : # echo more >> a && git add a && expect_parity bytes -- commit --dry-run
  }
)

# mode=bytes — `--short` implies --dry-run, output is short-format status.
title "gix commit --short"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "TODO: matches git behavior" && {
    : # expect_parity bytes -- commit --short
  }
)

# mode=bytes — `--branch` adds branch+tracking header (only meaningful in
# --short / --porcelain).
title "gix commit --branch"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "TODO: matches git behavior" && {
    : # expect_parity bytes -- commit --short --branch
  }
)

# mode=bytes — `--porcelain` implies --dry-run with porcelain v1 format.
title "gix commit --porcelain"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "TODO: matches git behavior" && {
    : # expect_parity bytes -- commit --porcelain
  }
)

# mode=bytes — `--long` implies --dry-run with default git-status long format.
title "gix commit --long"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "TODO: matches git behavior" && {
    : # expect_parity bytes -- commit --long
  }
)

# mode=bytes — `-z` / `--null` switches short/porcelain output to NUL
# terminators. Implies --porcelain when neither --short nor --porcelain
# is given.
title "gix commit -z / --null"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "TODO: matches git behavior" && {
    : # expect_parity bytes -- commit --short -z
  }
)

# --- status block / verbosity -----------------------------------------

# mode=effect — `--status` / `--no-status` toggle inclusion of git-status
# output in the editor template. Observable only through editor capture
# or via captured COMMIT_EDITMSG; deferred to bytes parity once editor
# template renderer lands.
title "gix commit --status / --no-status"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "TODO: matches git behavior — --status" && {
    : # EDITOR=true expect_parity effect -- commit --allow-empty --status -m m
  }
  it "TODO: matches git behavior — --no-status" && {
    : # EDITOR=true expect_parity effect -- commit --allow-empty --no-status -m m
  }
)

# mode=effect — `-v` / `--verbose` adds unified diff to editor template.
# `-vv` adds the worktree diff on top. Bytes-mode parity is far off;
# effect-mode parity holds (commit succeeds, exit 0).
title "gix commit -v / --verbose"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "TODO: matches git behavior — -v" && {
    : # expect_parity effect -- commit --allow-empty -v -m m
  }
  it "TODO: matches git behavior — -vv" && {
    : # expect_parity effect -- commit --allow-empty -vv -m m
  }
)

# mode=effect — `-q` / `--quiet` suppresses the post-commit summary line.
title "gix commit -q / --quiet"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "TODO: matches git behavior" && {
    : # expect_parity effect -- commit --allow-empty -q -m m
  }
)

# --- untracked-files mode (git-status passthrough) ---------------------

# mode=bytes — `-u[<mode>]` / `--untracked-files[=<mode>]` controls the
# untracked-files block in --dry-run / status output. mode ∈ {no,normal,all}.
title "gix commit -u / --untracked-files"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "TODO: matches git behavior — -u (default all)" && {
    : # expect_parity bytes -- commit --short -u
  }
  it "TODO: matches git behavior — --untracked-files=no" && {
    : # expect_parity bytes -- commit --short --untracked-files=no
  }
  it "TODO: matches git behavior — --untracked-files=normal" && {
    : # expect_parity bytes -- commit --short --untracked-files=normal
  }
  it "TODO: matches git behavior — --untracked-files=all" && {
    : # expect_parity bytes -- commit --short --untracked-files=all
  }
)

# --- signing -----------------------------------------------------------

# mode=effect — `-S` / `--gpg-sign[=<keyid>]` requests gpg signing;
# `--no-gpg-sign` countermands `commit.gpgSign` config + earlier --gpg-sign.
# gix has no GPG backend wired, so signing-on rows expect to emit a
# git-compat error + exit 128 (mirroring the `tag -s` shortcoming).
title "gix commit -S / --gpg-sign / --no-gpg-sign"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "TODO: matches git behavior — -S" && {
    : # expect_parity effect -- commit --allow-empty -S -m m
  }
  it "TODO: matches git behavior — --no-gpg-sign" && {
    : # expect_parity effect -- commit --allow-empty --no-gpg-sign -m m
  }
)

# --- terminator --------------------------------------------------------

# mode=effect — `--` separates options from pathspec.
title "gix commit -- <pathspec>"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "TODO: matches git behavior" && {
    : # echo more >> a && expect_parity effect -- commit -m m -- a
  }
)
