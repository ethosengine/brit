# Must be sourced into tests/parity.sh or tests/journey.sh — see tests/parity.sh.
#
# Parity scaffold for `git pull` ↔ `gix pull`.
#
# One `title` + `it` block per flag derived from
# vendor/git/Documentation/git-pull.adoc + the inherited
# include::merge-options.adoc / fetch-options.adoc surface, plus
# vendor/git/builtin/pull.c (cmd_pull, options[] at builtin/pull.c:870..1009).
# Every `it` body starts as a TODO placeholder — iteration N of the ralph
# loop picks the next TODO, converts it to a real `expect_parity` (or
# `compat_effect`) assertion, and removes the TODO marker.
#
# Verdict modes (comment above each block):
#   bytes  — scriptable output consumed by tooling: bare-no-upstream
#            "There is no tracking information for the current branch."
#            stanza + 128, bad-revspec wording around fetch-resolve
#            failure paths, --help exit-code contract on the meta row.
#   effect — UX-level parity (exit-code match + optional prose check).
#            Default for the human-rendered flags whose pretty
#            rendering is not yet implemented in gix's pull entry
#            point. Most rows close as `compat_effect` until the pull
#            driver lands.
#
# Coverage on gix's current Clap surface (src/plumbing/options/pull.rs):
#   gix pull [OPTIONS] [<repository> [<refspec>...]]
# Today the porcelain driver is a placeholder
# (gitoxide-core/src/repository/pull.rs::porcelain) that emits a stub
# note on stderr and exits 0. The flag surface is clap-wired so
# `gix pull <flag> ...` does not trip UnknownArgument; every
# flag-bearing row therefore closes as `compat_effect "<reason>"`
# under the shared deferral phrase "deferred until pull driver lands"
# until the real driver implements the semantic. Closing this command
# requires (1) implementing the pull driver in
# gitoxide-core/src/repository/pull.rs (compose fetch + merge/rebase,
# resolve FETCH_HEAD entries, drive integration), (2) wiring the
# bare-no-upstream gate (translate `branch.<name>.remote` /
# `branch.<name>.merge` lookup failure into git's verbatim 8-line
# stanza at exit 128), (3) translating C-side invariants in
# vendor/git/builtin/pull.c (parse_config_rebase / config_get_rebase /
# config_get_ff / opt_autostash interlocks; --rebase[=value] enum
# parsing; --ff vs --no-ff vs --ff-only state machine inherited from
# merge.c).
#
# Hash coverage: `dual` rows never open a repo (--help, outside-of-repo,
# --bogus-flag pre-repo dispatch). Every row that opens a repository
# is `sha1-only` because gix-config rejects
# `extensions.objectFormat=sha256`
# (gix/src/config/tree/sections/extensions.rs try_into_object_format,
# sha1-only validator), blocking every sha256 fixture at open. Rows
# flip to `dual` once that validator accepts sha256.
#
# parity-defaults:
#   hash=sha1-only "gix cannot load sha256 repos: extensions.objectFormat=sha256 rejected (gix/src/config/tree/sections/extensions.rs)"
#   mode=effect

title "gix pull"

# --- meta / help --------------------------------------------------------

# mode=effect — clap --help short-circuits before repo load, exits 0.
# git's --help delegates to `man git-pull` (exit 0 when man is
# available); gix returns clap's auto-generated help. Message text
# diverges; only the exit-code match is asserted.
# hash=dual
title "gix pull --help"
only_for_hash dual && (sandbox
  it "matches git behavior" && {
    expect_parity effect -- pull --help
  }
)

# mode=effect — unknown flag: git exits 129 (usage_msg_opt in
# parse-options.c). gix's Clap layer maps UnknownArgument to 129 via
# src/plumbing/main.rs. Tested inside a repo because git outside a
# repo dies 128 before reaching arg-parse, while clap in gix always
# runs first.
# hash=sha1-only
title "gix pull --bogus-flag"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    expect_parity effect -- pull --bogus-flag
  }
)

# mode=effect — outside any repo: git dies 128 with "fatal: not a git
# repository (or any of the parent directories): .git". gix's plumbing
# repository() closure already remaps gix_discover::upwards::Error::
# NoGitRepository* variants to git's exact wording + exit 128.
# hash=dual
title "gix pull (outside a repository)"
only_for_hash dual && (sandbox
  it "matches git behavior" && {
    expect_parity effect -- pull
  }
)

# mode=bytes — bare `git pull` with no upstream configured: git emits
# the 8-line "There is no tracking information for the current branch."
# stanza + exit 128 (vendor/git/builtin/pull.c::cmd_pull → die path
# when `branch.<name>.remote` / `branch.<name>.merge` are unset).
# gix's porcelain placeholder must mirror byte-exact wording.
# hash=sha1-only
title "gix pull (bare, no upstream)"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "TODO" && {
    : # TODO expect_parity bytes -- pull
  }
)

# --- synopsis: <repository> / <repository> <refspec> --------------------

# mode=effect — `git pull <remote>` against a configured local
# upstream that is already up to date: git fetches, then merge says
# "Already up to date." + exit 0. gix's placeholder emits a stub note
# and exits 0; `compat_effect` deferral until pull driver lands.
# hash=sha1-only
title "gix pull <remote> (already up to date)"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "TODO" && {
    : # TODO compat_effect "deferred until pull driver lands" -- pull origin
  }
)

# mode=effect — `git pull <remote> <refspec>`: git fetches the named
# branch + integrates. gix's placeholder exits 0; compat_effect.
# hash=sha1-only
title "gix pull <remote> <refspec>"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "TODO" && {
    : # TODO compat_effect "deferred until pull driver lands" -- pull origin main
  }
)

# mode=bytes — bad revspec under `git pull <remote> <bad-ref>`: git
# fetch fails with the canonical "couldn't find remote ref" stanza +
# exit 128. gix's placeholder must mirror bytes once the fetch step
# is wired in.
# hash=sha1-only
title "gix pull <remote> <bad-revspec>"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "TODO" && {
    : # TODO expect_parity bytes -- pull origin nonexistent-ref
  }
)

# --- shared verbosity / progress ---------------------------------------

# mode=effect — `-v`/`--verbose`: passes through to fetch + merge.
# hash=sha1-only
title "gix pull -v"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "TODO" && {
    : # TODO compat_effect "deferred until pull driver lands" -- pull -v
  }
)

# mode=effect — `--verbose` long form.
# hash=sha1-only
title "gix pull --verbose"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "TODO" && {
    : # TODO compat_effect "deferred until pull driver lands" -- pull --verbose
  }
)

# mode=effect — `-q`/`--quiet`: squelches reporting on both sub-commands.
# hash=sha1-only
title "gix pull -q"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "TODO" && {
    : # TODO compat_effect "deferred until pull driver lands" -- pull -q
  }
)

# mode=effect — `--quiet` long form.
# hash=sha1-only
title "gix pull --quiet"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "TODO" && {
    : # TODO compat_effect "deferred until pull driver lands" -- pull --quiet
  }
)

# mode=effect — `--progress`: forces progress output on stderr even
# when not a TTY.
# hash=sha1-only
title "gix pull --progress"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "TODO" && {
    : # TODO compat_effect "deferred until pull driver lands" -- pull --progress
  }
)

# mode=effect — `--no-progress`: suppresses progress output.
# hash=sha1-only
title "gix pull --no-progress"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "TODO" && {
    : # TODO compat_effect "deferred until pull driver lands" -- pull --no-progress
  }
)

# mode=effect — `--recurse-submodules` (bare): defaults to "yes".
# hash=sha1-only
title "gix pull --recurse-submodules"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "TODO" && {
    : # TODO compat_effect "deferred until pull driver lands" -- pull --recurse-submodules
  }
)

# mode=effect — `--recurse-submodules=on-demand`: only fetch submodules
# whose superproject commits update them.
# hash=sha1-only
title "gix pull --recurse-submodules=on-demand"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "TODO" && {
    : # TODO compat_effect "deferred until pull driver lands" -- pull --recurse-submodules=on-demand
  }
)

# mode=effect — `--recurse-submodules=no`: equivalent to
# `--no-recurse-submodules`.
# hash=sha1-only
title "gix pull --recurse-submodules=no"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "TODO" && {
    : # TODO compat_effect "deferred until pull driver lands" -- pull --recurse-submodules=no
  }
)

# mode=effect — `--no-recurse-submodules`: long-form negation.
# hash=sha1-only
title "gix pull --no-recurse-submodules"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "TODO" && {
    : # TODO compat_effect "deferred until pull driver lands" -- pull --no-recurse-submodules
  }
)

# --- merging: -r/--rebase[=VALUE] / --no-rebase ------------------------

# mode=effect — `-r` (bare): rebase=true, rebase the current branch
# onto upstream after fetching. `require_equals = true` so `-r dev`
# parses as `-r` + positional `dev`, mirroring git's PARSE_OPT_OPTARG.
# hash=sha1-only
title "gix pull -r"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "TODO" && {
    : # TODO compat_effect "deferred until pull driver lands" -- pull -r
  }
)

# mode=effect — `--rebase` (bare): same as `-r`.
# hash=sha1-only
title "gix pull --rebase"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "TODO" && {
    : # TODO compat_effect "deferred until pull driver lands" -- pull --rebase
  }
)

# mode=effect — `--rebase=true`: explicit form.
# hash=sha1-only
title "gix pull --rebase=true"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "TODO" && {
    : # TODO compat_effect "deferred until pull driver lands" -- pull --rebase=true
  }
)

# mode=effect — `--rebase=false`: integrate via merge instead of rebase.
# hash=sha1-only
title "gix pull --rebase=false"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "TODO" && {
    : # TODO compat_effect "deferred until pull driver lands" -- pull --rebase=false
  }
)

# mode=effect — `--rebase=merges`: rebase --rebase-merges, preserving
# local merge commits.
# hash=sha1-only
title "gix pull --rebase=merges"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "TODO" && {
    : # TODO compat_effect "deferred until pull driver lands" -- pull --rebase=merges
  }
)

# mode=effect — `--rebase=interactive`: interactive rebase mode.
# hash=sha1-only
title "gix pull --rebase=interactive"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "TODO" && {
    : # TODO compat_effect "deferred until pull driver lands" -- pull --rebase=interactive
  }
)

# mode=effect — `--no-rebase`: shorthand for --rebase=false.
# hash=sha1-only
title "gix pull --no-rebase"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "TODO" && {
    : # TODO compat_effect "deferred until pull driver lands" -- pull --no-rebase
  }
)

# --- merging: -n/--no-stat / --stat / --summary / --compact-summary ----

# mode=effect — `-n`: SET_INT alias for --no-stat (suppress trailing
# diffstat).
# hash=sha1-only
title "gix pull -n"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "TODO" && {
    : # TODO compat_effect "deferred until pull driver lands" -- pull -n
  }
)

# mode=effect — `--no-stat`: canonical long form.
# hash=sha1-only
title "gix pull --no-stat"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "TODO" && {
    : # TODO compat_effect "deferred until pull driver lands" -- pull --no-stat
  }
)

# mode=effect — `--stat`: enable trailing diffstat.
# hash=sha1-only
title "gix pull --stat"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "TODO" && {
    : # TODO compat_effect "deferred until pull driver lands" -- pull --stat
  }
)

# mode=effect — `--summary`: deprecated synonym for --stat.
# hash=sha1-only
title "gix pull --summary"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "TODO" && {
    : # TODO compat_effect "deferred until pull driver lands" -- pull --summary
  }
)

# mode=effect — `--no-summary`: deprecated synonym for --no-stat.
# hash=sha1-only
title "gix pull --no-summary"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "TODO" && {
    : # TODO compat_effect "deferred until pull driver lands" -- pull --no-summary
  }
)

# mode=effect — `--compact-summary`: vendor/git v2.54.0 flag absent
# from system git 2.47.3 — version skew is a hard system constraint.
# hash=sha1-only
title "gix pull --compact-summary"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "TODO" && {
    : # TODO shortcoming "system git 2.47.3 lacks --compact-summary; vendor/git v2.54.0 has it"
  }
)

# --- merging: --log[=n] / --no-log -------------------------------------

# mode=effect — `--log` (bare): include up to DEFAULT_MERGE_LOG_LEN
# one-line shortlog entries in the merge message.
# hash=sha1-only
title "gix pull --log"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "TODO" && {
    : # TODO compat_effect "deferred until pull driver lands" -- pull --log
  }
)

# mode=effect — `--log=<n>`: cap shortlog inclusion at N entries.
# hash=sha1-only
title "gix pull --log=<n>"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "TODO" && {
    : # TODO compat_effect "deferred until pull driver lands" -- pull --log=3
  }
)

# mode=effect — `--no-log`: suppress shortlog (default).
# hash=sha1-only
title "gix pull --no-log"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "TODO" && {
    : # TODO compat_effect "deferred until pull driver lands" -- pull --no-log
  }
)

# --- merging: --signoff / --no-signoff ---------------------------------

# mode=effect — `--signoff`: append "Signed-off-by: <ident>" trailer.
# hash=sha1-only
title "gix pull --signoff"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "TODO" && {
    : # TODO compat_effect "deferred until pull driver lands" -- pull --signoff
  }
)

# mode=effect — `--no-signoff`: explicit countermand of merge.signoff.
# hash=sha1-only
title "gix pull --no-signoff"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "TODO" && {
    : # TODO compat_effect "deferred until pull driver lands" -- pull --no-signoff
  }
)

# --- merging: --squash / --no-squash / --commit / --no-commit ----------

# mode=effect — `--squash`: produce working-tree state but no commit.
# hash=sha1-only
title "gix pull --squash"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "TODO" && {
    : # TODO compat_effect "deferred until pull driver lands" -- pull --squash
  }
)

# mode=effect — `--no-squash`: default; countermand merge.squash config.
# hash=sha1-only
title "gix pull --no-squash"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "TODO" && {
    : # TODO compat_effect "deferred until pull driver lands" -- pull --no-squash
  }
)

# mode=effect — `--commit`: force a merge commit even on fast-forward.
# hash=sha1-only
title "gix pull --commit"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "TODO" && {
    : # TODO compat_effect "deferred until pull driver lands" -- pull --commit
  }
)

# mode=effect — `--no-commit`: leave MERGE_HEAD; require explicit commit.
# hash=sha1-only
title "gix pull --no-commit"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "TODO" && {
    : # TODO compat_effect "deferred until pull driver lands" -- pull --no-commit
  }
)

# --- merging: --edit / --no-edit / --cleanup ---------------------------

# mode=effect — `-e`/`--edit`: open editor on merge message.
# hash=sha1-only
title "gix pull -e"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "TODO" && {
    : # TODO compat_effect "deferred until pull driver lands" -- pull -e
  }
)

# mode=effect — `--edit` long form.
# hash=sha1-only
title "gix pull --edit"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "TODO" && {
    : # TODO compat_effect "deferred until pull driver lands" -- pull --edit
  }
)

# mode=effect — `--no-edit`: skip editor.
# hash=sha1-only
title "gix pull --no-edit"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "TODO" && {
    : # TODO compat_effect "deferred until pull driver lands" -- pull --no-edit
  }
)

# mode=effect — `--cleanup=<mode>`: control message-cleanup.
# hash=sha1-only
title "gix pull --cleanup=strip"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "TODO" && {
    : # TODO compat_effect "deferred until pull driver lands" -- pull --cleanup=strip
  }
)

# --- merging: --ff / --no-ff / --ff-only -------------------------------

# mode=effect — `--ff`: allow fast-forward.
# hash=sha1-only
title "gix pull --ff"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "TODO" && {
    : # TODO compat_effect "deferred until pull driver lands" -- pull --ff
  }
)

# mode=effect — `--no-ff`: forbid fast-forward; always merge commit.
# hash=sha1-only
title "gix pull --no-ff"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "TODO" && {
    : # TODO compat_effect "deferred until pull driver lands" -- pull --no-ff
  }
)

# mode=effect — `--ff-only`: refuse non-fast-forward integration.
# hash=sha1-only
title "gix pull --ff-only"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "TODO" && {
    : # TODO compat_effect "deferred until pull driver lands" -- pull --ff-only
  }
)

# --- merging: --verify / --no-verify -----------------------------------

# mode=effect — `--verify`: run hooks (default).
# hash=sha1-only
title "gix pull --verify"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "TODO" && {
    : # TODO compat_effect "deferred until pull driver lands" -- pull --verify
  }
)

# mode=effect — `--no-verify`: bypass commit-msg / pre-merge-commit hooks.
# hash=sha1-only
title "gix pull --no-verify"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "TODO" && {
    : # TODO compat_effect "deferred until pull driver lands" -- pull --no-verify
  }
)

# --- merging: --verify-signatures / --no-verify-signatures -------------

# mode=effect — `--verify-signatures`: abort on bad/missing GPG sig.
# hash=sha1-only
title "gix pull --verify-signatures"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "TODO" && {
    : # TODO compat_effect "deferred until pull driver lands" -- pull --verify-signatures
  }
)

# mode=effect — `--no-verify-signatures`: skip signature verification.
# hash=sha1-only
title "gix pull --no-verify-signatures"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "TODO" && {
    : # TODO compat_effect "deferred until pull driver lands" -- pull --no-verify-signatures
  }
)

# --- merging: --autostash / --no-autostash -----------------------------

# mode=effect — `--autostash`: stash + reapply local changes around
# integration.
# hash=sha1-only
title "gix pull --autostash"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "TODO" && {
    : # TODO compat_effect "deferred until pull driver lands" -- pull --autostash
  }
)

# mode=effect — `--no-autostash`: countermand pull.autostash config.
# hash=sha1-only
title "gix pull --no-autostash"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "TODO" && {
    : # TODO compat_effect "deferred until pull driver lands" -- pull --no-autostash
  }
)

# --- merging: -s/--strategy / -X/--strategy-option ---------------------

# mode=effect — `-s <strategy>`: select named merge strategy.
# hash=sha1-only
title "gix pull -s ort"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "TODO" && {
    : # TODO compat_effect "deferred until pull driver lands" -- pull -s ort
  }
)

# mode=effect — `--strategy=<strategy>` long form.
# hash=sha1-only
title "gix pull --strategy=ort"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "TODO" && {
    : # TODO compat_effect "deferred until pull driver lands" -- pull --strategy=ort
  }
)

# mode=effect — `-X <option>`: pass strategy-specific option.
# hash=sha1-only
title "gix pull -X theirs"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "TODO" && {
    : # TODO compat_effect "deferred until pull driver lands" -- pull -X theirs
  }
)

# mode=effect — `--strategy-option=<key=value>` long form.
# hash=sha1-only
title "gix pull --strategy-option=theirs"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "TODO" && {
    : # TODO compat_effect "deferred until pull driver lands" -- pull --strategy-option=theirs
  }
)

# --- merging: -S/--gpg-sign / --no-gpg-sign ----------------------------

# mode=effect — `-S` (bare): GPG-sign the merge commit with the default
# key. `require_equals = true` so `-S dev` parses as `-S` + positional.
# hash=sha1-only
title "gix pull -S"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "TODO" && {
    : # TODO compat_effect "deferred until pull driver lands" -- pull -S
  }
)

# mode=effect — `--gpg-sign` long form.
# hash=sha1-only
title "gix pull --gpg-sign"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "TODO" && {
    : # TODO compat_effect "deferred until pull driver lands" -- pull --gpg-sign
  }
)

# mode=effect — `--gpg-sign=<key-id>`: use named key.
# hash=sha1-only
title "gix pull --gpg-sign=<key-id>"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "TODO" && {
    : # TODO compat_effect "deferred until pull driver lands" -- pull --gpg-sign=ABCDEF12
  }
)

# mode=effect — `--no-gpg-sign`: override commit.gpgSign config.
# hash=sha1-only
title "gix pull --no-gpg-sign"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "TODO" && {
    : # TODO compat_effect "deferred until pull driver lands" -- pull --no-gpg-sign
  }
)

# --- merging: --allow-unrelated-histories ------------------------------

# mode=effect — `--allow-unrelated-histories`: permit merging two
# histories with no common ancestor.
# hash=sha1-only
title "gix pull --allow-unrelated-histories"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "TODO" && {
    : # TODO compat_effect "deferred until pull driver lands" -- pull --allow-unrelated-histories
  }
)

# --- fetching: --all ---------------------------------------------------

# mode=effect — `--all`: fetch from all configured remotes.
# hash=sha1-only
title "gix pull --all"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "TODO" && {
    : # TODO compat_effect "deferred until pull driver lands" -- pull --all
  }
)

# --- fetching: -a/--append ---------------------------------------------

# mode=effect — `-a`/`--append`: append fetched refs to FETCH_HEAD
# instead of overwriting.
# hash=sha1-only
title "gix pull -a"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "TODO" && {
    : # TODO compat_effect "deferred until pull driver lands" -- pull -a
  }
)

# mode=effect — `--append` long form.
# hash=sha1-only
title "gix pull --append"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "TODO" && {
    : # TODO compat_effect "deferred until pull driver lands" -- pull --append
  }
)

# --- fetching: --upload-pack -------------------------------------------

# mode=effect — `--upload-pack=<path>`: override remote upload-pack
# program.
# hash=sha1-only
title "gix pull --upload-pack=<path>"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "TODO" && {
    : # TODO compat_effect "deferred until pull driver lands" -- pull --upload-pack=/usr/bin/git-upload-pack
  }
)

# --- fetching: -f/--force ----------------------------------------------

# mode=effect — `-f`/`--force`: allow non-fast-forward ref updates
# during fetch.
# hash=sha1-only
title "gix pull -f"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "TODO" && {
    : # TODO compat_effect "deferred until pull driver lands" -- pull -f
  }
)

# mode=effect — `--force` long form.
# hash=sha1-only
title "gix pull --force"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "TODO" && {
    : # TODO compat_effect "deferred until pull driver lands" -- pull --force
  }
)

# --- fetching: -t/--tags / -p/--prune ----------------------------------

# mode=effect — `-t`/`--tags`: fetch all tags in addition.
# hash=sha1-only
title "gix pull -t"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "TODO" && {
    : # TODO compat_effect "deferred until pull driver lands" -- pull -t
  }
)

# mode=effect — `--tags` long form.
# hash=sha1-only
title "gix pull --tags"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "TODO" && {
    : # TODO compat_effect "deferred until pull driver lands" -- pull --tags
  }
)

# mode=effect — `-p`/`--prune`: remove remote-tracking refs no longer
# on the remote.
# hash=sha1-only
title "gix pull -p"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "TODO" && {
    : # TODO compat_effect "deferred until pull driver lands" -- pull -p
  }
)

# mode=effect — `--prune` long form.
# hash=sha1-only
title "gix pull --prune"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "TODO" && {
    : # TODO compat_effect "deferred until pull driver lands" -- pull --prune
  }
)

# --- fetching: -j/--jobs / --dry-run -----------------------------------

# mode=effect — `-j`/`--jobs=<n>`: parallel submodule/multi-remote
# fetches.
# hash=sha1-only
title "gix pull --jobs=4"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "TODO" && {
    : # TODO compat_effect "deferred until pull driver lands" -- pull --jobs=4
  }
)

# mode=effect — `--dry-run`: show what would be fetched without doing it.
# hash=sha1-only
title "gix pull --dry-run"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "TODO" && {
    : # TODO compat_effect "deferred until pull driver lands" -- pull --dry-run
  }
)

# --- fetching: -k/--keep -----------------------------------------------

# mode=effect — `-k`/`--keep`: keep downloaded pack rather than
# exploding/discarding.
# hash=sha1-only
title "gix pull -k"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "TODO" && {
    : # TODO compat_effect "deferred until pull driver lands" -- pull -k
  }
)

# mode=effect — `--keep` long form.
# hash=sha1-only
title "gix pull --keep"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "TODO" && {
    : # TODO compat_effect "deferred until pull driver lands" -- pull --keep
  }
)

# --- fetching: shallow flags -------------------------------------------

# mode=effect — `--depth=<n>`: limit fetched history.
# hash=sha1-only
title "gix pull --depth=1"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "TODO" && {
    : # TODO compat_effect "deferred until pull driver lands" -- pull --depth=1
  }
)

# mode=effect — `--shallow-since=<date>`: cut off history past date.
# hash=sha1-only
title "gix pull --shallow-since=<date>"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "TODO" && {
    : # TODO compat_effect "deferred until pull driver lands" -- pull --shallow-since=2024-01-01
  }
)

# mode=effect — `--shallow-exclude=<ref>`: cut history at named ref.
# hash=sha1-only
title "gix pull --shallow-exclude=<ref>"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "TODO" && {
    : # TODO compat_effect "deferred until pull driver lands" -- pull --shallow-exclude=v1.0
  }
)

# mode=effect — `--deepen=<n>`: extend shallow boundary by N commits.
# hash=sha1-only
title "gix pull --deepen=1"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "TODO" && {
    : # TODO compat_effect "deferred until pull driver lands" -- pull --deepen=1
  }
)

# mode=effect — `--unshallow`: remove shallow boundary.
# hash=sha1-only
title "gix pull --unshallow"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "TODO" && {
    : # TODO compat_effect "deferred until pull driver lands" -- pull --unshallow
  }
)

# mode=effect — `--update-shallow`: accept refs that update shallow file.
# hash=sha1-only
title "gix pull --update-shallow"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "TODO" && {
    : # TODO compat_effect "deferred until pull driver lands" -- pull --update-shallow
  }
)

# --- fetching: --refmap ------------------------------------------------

# mode=effect — `--refmap=<refspec>`: override configured refmap.
# hash=sha1-only
title "gix pull --refmap=<refspec>"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "TODO" && {
    : # TODO compat_effect "deferred until pull driver lands" -- pull --refmap=refs/heads/*:refs/remotes/origin/*
  }
)

# --- fetching: -o/--server-option --------------------------------------

# mode=effect — `-o`/`--server-option=<opt>`: protocol v2 server option.
# hash=sha1-only
title "gix pull -o <opt>"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "TODO" && {
    : # TODO compat_effect "deferred until pull driver lands" -- pull -o feature=on
  }
)

# mode=effect — `--server-option=<opt>` long form.
# hash=sha1-only
title "gix pull --server-option=<opt>"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "TODO" && {
    : # TODO compat_effect "deferred until pull driver lands" -- pull --server-option=feature=on
  }
)

# --- fetching: -4/-6 ipv family ----------------------------------------

# mode=effect — `-4`/`--ipv4`: force IPv4.
# hash=sha1-only
title "gix pull -4"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "TODO" && {
    : # TODO compat_effect "deferred until pull driver lands" -- pull -4
  }
)

# mode=effect — `--ipv4` long form.
# hash=sha1-only
title "gix pull --ipv4"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "TODO" && {
    : # TODO compat_effect "deferred until pull driver lands" -- pull --ipv4
  }
)

# mode=effect — `-6`/`--ipv6`: force IPv6.
# hash=sha1-only
title "gix pull -6"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "TODO" && {
    : # TODO compat_effect "deferred until pull driver lands" -- pull -6
  }
)

# mode=effect — `--ipv6` long form.
# hash=sha1-only
title "gix pull --ipv6"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "TODO" && {
    : # TODO compat_effect "deferred until pull driver lands" -- pull --ipv6
  }
)

# --- fetching: --negotiation-tip ---------------------------------------

# mode=effect — `--negotiation-tip=<rev>`: narrow negotiation.
# hash=sha1-only
title "gix pull --negotiation-tip=<rev>"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "TODO" && {
    : # TODO compat_effect "deferred until pull driver lands" -- pull --negotiation-tip=HEAD
  }
)

# --- fetching: --show-forced-updates / --no-show-forced-updates --------

# mode=effect — `--show-forced-updates`: force the forced-update check.
# hash=sha1-only
title "gix pull --show-forced-updates"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "TODO" && {
    : # TODO compat_effect "deferred until pull driver lands" -- pull --show-forced-updates
  }
)

# mode=effect — `--no-show-forced-updates`: skip the check (perf).
# hash=sha1-only
title "gix pull --no-show-forced-updates"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "TODO" && {
    : # TODO compat_effect "deferred until pull driver lands" -- pull --no-show-forced-updates
  }
)

# --- fetching: --set-upstream ------------------------------------------

# mode=effect — `--set-upstream`: configure upstream tracking on
# integrated branch.
# hash=sha1-only
title "gix pull --set-upstream"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "TODO" && {
    : # TODO compat_effect "deferred until pull driver lands" -- pull --set-upstream
  }
)
