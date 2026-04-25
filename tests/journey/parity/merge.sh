# Must be sourced into tests/parity.sh or tests/journey.sh — see tests/parity.sh.
#
# Parity scaffold for `git merge` ↔ `gix merge`.
#
# One `title` + `it` block per flag derived from
# vendor/git/Documentation/git-merge.adoc + the inherited
# include::merge-options.adoc / rerere-options.adoc / signoff-option.adoc
# surface, plus vendor/git/builtin/merge.c
# (cmd_merge, builtin_merge_options at builtin/merge.c:261..339). Every
# `it` body starts as a TODO placeholder — iteration N of the ralph
# loop picks the next TODO, converts it to a real `expect_parity` (or
# `compat_effect`) assertion, and removes the TODO marker.
#
# Verdict modes (comment above each block):
#   bytes  — scriptable output consumed by tooling: error stanzas
#            around bad revspecs, "Already up to date." short-circuit,
#            "Already up to date" for fast-forward equality, the exact
#            "fatal: There is no merge to abort" / "There is no merge
#            in progress (MERGE_HEAD missing)." wordings around the
#            in-progress transitions (--abort / --quit / --continue).
#   effect — UX-level parity (exit-code match + optional prose check).
#            Default for the human-rendered flags whose pretty
#            rendering is not yet implemented in gix's merge entry
#            point. Most rows close as `compat_effect` until the merge
#            driver lands.
#
# Coverage on gix's current Clap surface (src/plumbing/options/mod.rs::merge):
#   gix merge [OPTIONS] [<commit>...]
#     subcommand escape hatches: file / tree / commit (plumbing-only,
#     mirror builtin/merge-file.c / builtin/merge-tree.c — out of
#     scope for the porcelain `git merge` parity surface).
# Today the porcelain driver is a placeholder
# (gitoxide-core/src/repository/merge/porcelain.rs::porcelain) that
# emits a stub note on stderr and exits 0. The flag surface is
# clap-wired so `gix merge <flag> ...` does not trip UnknownArgument;
# every flag-bearing row therefore closes as `compat_effect "<reason>"`
# under the shared deferral phrase "deferred until merge driver lands"
# until the real driver implements the semantic. Closing this command
# requires (1) implementing the merge driver in
# gitoxide-core/src/repository/merge/porcelain.rs (fast-forward / 3-way
# / octopus paths, AUTO_MERGE / MERGE_HEAD / MERGE_MSG ref writes,
# conflict-marker emission), (2) wiring the in-progress transitions
# (--abort / --quit / --continue) to gix-ref + gix-status precondition
# checks for MERGE_HEAD presence, (3) translating C-side invariants in
# vendor/git/builtin/merge.c (option_commit / option_edit / fast_forward
# / squash interlocks; ff vs no-ff vs ff-only state machine).
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

title "gix merge"

# --- meta / help --------------------------------------------------------

# mode=effect — clap --help short-circuits before repo load, exits 0.
# git's --help delegates to `man git-merge` (exit 0 when man is
# available); gix returns clap's auto-generated help. Message text
# diverges; only the exit-code match is asserted.
# hash=dual
title "gix merge --help"
only_for_hash dual && (sandbox
  it "matches git behavior" && {
    expect_parity effect -- merge --help
  }
)

# mode=effect — unknown flag: git exits 129 (usage_msg_opt in
# parse-options.c). gix's Clap layer maps UnknownArgument to 129 via
# src/plumbing/main.rs. Tested inside a repo because git outside a
# repo dies 128 before reaching arg-parse, while clap in gix always
# runs first.
# hash=sha1-only
title "gix merge --bogus-flag"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    expect_parity effect -- merge --bogus-flag
  }
)

# mode=bytes — bare `git merge` with no args and no upstream
# configured: git emits the verbatim line "fatal: No remote for the
# current branch." + exit 128 (vendor/git/builtin/merge.c::cmd_merge
# default-to-upstream path → die_if_checked_out / die_for_remote_other).
# gix's porcelain placeholder gates on commits.is_empty() and emits
# the same fatal wording + exits 128; bytes parity holds even though
# the underlying driver is still a stub.
# hash=sha1-only
title "gix merge (bare, no upstream)"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    expect_parity bytes -- merge
  }
)

# mode=effect — outside any repo: git dies 128 with "fatal: not a git
# repository (or any of the parent directories): .git". gix's plumbing
# repository() closure already remaps gix_discover::upwards::Error::
# NoGitRepository* variants to git's exact wording + exit 128.
# hash=dual
title "gix merge (outside a repository)"
only_for_hash dual && (sandbox
  it "matches git behavior" && {
    expect_parity effect -- merge HEAD
  }
)

# --- synopsis: <commit> form --------------------------------------------

# mode=effect — `git merge <commit>` where <commit> is already an
# ancestor of HEAD: git emits "Already up to date." on stdout and
# exits 0. gix's placeholder emits a stub note (different bytes) and
# exits 0; exit-code parity holds. Close as `compat_effect` under
# the shared deferral phrase.
# hash=sha1-only
title "gix merge <commit> (already up to date)"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until merge driver lands" -- merge dev
  }
)

# mode=effect — `git merge <commit>` where <commit> is a descendant of
# HEAD: fast-forward; git updates HEAD and emits "Updating <a>..<b>"
# + diffstat, exit 0. gix's placeholder emits a stub note (different
# bytes) and exits 0; exit-code parity holds. Close as
# `compat_effect` under the shared deferral phrase. (`git checkout
# dev` first so dev sits behind main and merging main fast-forwards.)
# hash=sha1-only
title "gix merge <commit> (fast-forward)"
only_for_hash sha1-only && (small-repo-in-sandbox
  git checkout -q dev
  it "matches git behavior" && {
    compat_effect "deferred until merge driver lands" -- merge main
  }
)

# mode=effect — `git merge <commit>` where neither side is an ancestor
# of the other: 3-way merge. Clean merge produces a merge-commit
# (under GIT_MERGE_AUTOEDIT=no, set by tests/helpers.sh::set-static-
# git-environment) + "Merge made by the 'ort' strategy." stanza,
# exit 0. gix's placeholder emits a stub note and exits 0;
# exit-code parity holds. Close as `compat_effect`.
# hash=sha1-only
title "gix merge <commit> (3-way merge, clean)"
only_for_hash sha1-only && (small-repo-in-sandbox
  # Build a divergent history: dev edits a, main edits b — neither is
  # an ancestor of the other.
  git checkout -q dev
  echo "dev-line" >> a && git commit -qam "dev-edit"
  git checkout -q main
  it "matches git behavior" && {
    compat_effect "deferred until merge driver lands" -- merge dev
  }
)

# mode=effect — octopus merge: `git merge <commit-1> <commit-2> ...`.
# git uses the `octopus` strategy by default for >1 head. Even when
# both extras are ancestors, git emits "Already up to date." and
# exits 0. gix's placeholder accepts the multi-positional form and
# exits 0. Close as `compat_effect`.
# hash=sha1-only
title "gix merge <commit> <commit> (octopus)"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until merge driver lands" -- merge dev annotated
  }
)

# mode=bytes — bad revspec: git emits the verbatim line
# "merge: <ref> - not something we can merge" + exit 1
# (vendor/git/builtin/merge.c::collect_parents → get_oid_mb die path).
# gix's porcelain placeholder rev-parses each positional via
# Repository::rev_parse_single before the (placeholder) merge driver
# runs and emits the same wording + exit 1 on the first
# unresolvable ref.
# hash=sha1-only
title "gix merge <bad-revspec>"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    expect_parity bytes -- merge nonexistent-ref
  }
)

# --- diffstat: -n / --stat / --no-stat / --summary / --compact-summary --

# mode=effect — `-n` is a SET_INT alias for --no-stat (suppress the
# trailing diffstat). dev is an ancestor of HEAD, so both binaries
# emit "Already up to date." / placeholder note + exit 0;
# exit-code parity holds. compat_effect.
# hash=sha1-only
title "gix merge -n"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until merge driver lands" -- merge -n dev
  }
)

# mode=effect — `--stat` enables the trailing diffstat; default
# behavior unless overridden by `merge.stat=false` config.
# hash=sha1-only
title "gix merge --stat"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until merge driver lands" -- merge --stat dev
  }
)

# mode=effect — `--no-stat` is the canonical long form of -n.
# hash=sha1-only
title "gix merge --no-stat"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until merge driver lands" -- merge --no-stat dev
  }
)

# mode=effect — `--summary` is a deprecated synonym for --stat; git
# still accepts it without warning.
# hash=sha1-only
title "gix merge --summary"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until merge driver lands" -- merge --summary dev
  }
)

# mode=effect — `--no-summary` is a deprecated synonym for --no-stat.
# hash=sha1-only
title "gix merge --no-summary"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until merge driver lands" -- merge --no-summary dev
  }
)

# mode=effect — `--compact-summary` is a vendor/git v2.54.0 flag (see
# vendor/git/builtin/merge.c::builtin_merge_options) that the test
# runtime's system git (2.47.3) does not recognize. The version skew
# is a hard system constraint of the parity loop's test harness, not
# a closeable gix gap. Once the test runtime upgrades to a git that
# carries --compact-summary, this row flips to compat_effect under
# the shared "deferred until merge driver lands" phrase.
# hash=sha1-only
title "gix merge --compact-summary"
only_for_hash sha1-only && (small-repo-in-sandbox
  shortcoming "system git 2.47.3 lacks --compact-summary; vendor/git v2.54.0 has it"
)

# --- shortlog: --log[=<n>] / --no-log -----------------------------------

# mode=effect — `--log` (bare) populates the merge message with up to
# DEFAULT_MERGE_LOG_LEN one-line shortlog entries. `require_equals = true`
# in the Clap shape so `--log dev` parses as `--log` (default) +
# positional `dev`, mirroring git's PARSE_OPT_OPTARG semantics where
# the optarg must be stuck with `=`.
# hash=sha1-only
title "gix merge --log"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until merge driver lands" -- merge --log dev
  }
)

# mode=effect — `--log=<n>` caps shortlog inclusion at N entries.
# hash=sha1-only
title "gix merge --log=<n>"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until merge driver lands" -- merge --log=3 dev
  }
)

# mode=effect — `--no-log` suppresses the shortlog (default).
# hash=sha1-only
title "gix merge --no-log"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until merge driver lands" -- merge --no-log dev
  }
)

# --- squash / commit / edit / cleanup -----------------------------------

# mode=effect — `--squash` produces an index/working-tree state as if
# a real merge happened, but does NOT make a commit, move HEAD, or
# write MERGE_HEAD. Implies --no-commit; --commit is rejected when
# combined with --squash.
# hash=sha1-only
title "gix merge --squash"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until merge driver lands" -- merge --squash dev
  }
)

# mode=effect — `--no-squash` is the default; explicitly counter
# manding any `merge.squash` config.
# hash=sha1-only
title "gix merge --no-squash"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until merge driver lands" -- merge --no-squash dev
  }
)

# mode=effect — `--commit` is the default; explicitly countermanding
# any prior --no-commit.
# hash=sha1-only
title "gix merge --commit"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until merge driver lands" -- merge --commit dev
  }
)

# mode=effect — `--no-commit` performs the merge but stops just before
# creating the merge commit, leaving the user a chance to inspect.
# hash=sha1-only
title "gix merge --no-commit"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until merge driver lands" -- merge --no-commit dev
  }
)

# mode=effect — `-e` / `--edit` invokes EDITOR before committing the
# merge. tests/helpers.sh::set-static-git-environment sets
# GIT_EDITOR=true so EDITOR invocation is a no-op for parity.
# hash=sha1-only
title "gix merge --edit"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until merge driver lands" -- merge --edit dev
  }
)

# mode=effect — `--no-edit` (or GIT_MERGE_AUTOEDIT=no) accepts the
# auto-generated merge commit message without invoking EDITOR.
# hash=sha1-only
title "gix merge --no-edit"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until merge driver lands" -- merge --no-edit dev
  }
)

# mode=effect — `--cleanup=<mode>` controls how the merge commit
# message is cleaned up. Modes mirror git-commit: verbatim, whitespace,
# strip, scissors. `scissors` appends scissors-line to MERGE_MSG.
# hash=sha1-only
title "gix merge --cleanup=<mode>"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until merge driver lands" -- merge --cleanup=strip dev
  }
)

# --- fast-forward: --ff / --no-ff / --ff-only ---------------------------

# mode=effect — `--ff` is the default: when possible resolve the merge
# as a fast-forward (no merge commit). Explicitly countermanding
# `--no-ff` config.
# hash=sha1-only
title "gix merge --ff"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until merge driver lands" -- merge --ff dev
  }
)

# mode=effect — `--no-ff` always creates a merge commit, even when the
# merge could resolve as a fast-forward. Most-cited use case.
# hash=sha1-only
title "gix merge --no-ff"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until merge driver lands" -- merge --no-ff dev
  }
)

# mode=effect — `--ff-only` requires a fast-forward; if not possible,
# refuse the merge and exit non-zero. dev is an ancestor of HEAD so
# the merge resolves as Already-up-to-date (ff-only succeeds vacuously);
# both binaries exit 0.
# hash=sha1-only
title "gix merge --ff-only"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until merge driver lands" -- merge --ff-only dev
  }
)

# --- rerere: --rerere-autoupdate / --no-rerere-autoupdate ---------------

# mode=effect — `--rerere-autoupdate` allows rerere to update the
# index with reused resolution. Default unset.
# hash=sha1-only
title "gix merge --rerere-autoupdate"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until merge driver lands" -- merge --rerere-autoupdate dev
  }
)

# mode=effect — `--no-rerere-autoupdate` countermands rerere updates.
# hash=sha1-only
title "gix merge --no-rerere-autoupdate"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until merge driver lands" -- merge --no-rerere-autoupdate dev
  }
)

# --- signature verification: --verify-signatures ------------------------

# mode=effect — `--verify-signatures` aborts the merge unless the tip
# commit of the side branch is signed with a valid key. Without GPG
# tooling wired into gix, this row closes as compat_effect; the
# precondition gate lands when the merge driver does.
# hash=sha1-only
title "gix merge --verify-signatures"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until merge driver lands" -- merge --verify-signatures dev
  }
)

# mode=effect — `--no-verify-signatures` is the default.
# hash=sha1-only
title "gix merge --no-verify-signatures"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until merge driver lands" -- merge --no-verify-signatures dev
  }
)

# --- strategy: -s / -X --------------------------------------------------

# mode=effect — `-s <strategy>` / `--strategy=<strategy>` selects the
# merge strategy (ort, recursive, octopus, resolve, ours, subtree).
# Can be supplied multiple times; the implementations are tried in
# order. Closing as compat_effect.
# hash=sha1-only
title "gix merge -s <strategy>"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until merge driver lands" -- merge -s ort dev
  }
)

# mode=effect — `--strategy=<strategy>` is the long form of -s.
# hash=sha1-only
title "gix merge --strategy=<strategy>"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until merge driver lands" -- merge --strategy=ort dev
  }
)

# mode=effect — `-X <option>` / `--strategy-option=<option>` passes
# strategy-specific options through (e.g. -X ours, -X theirs,
# -X ignore-space-change, -X subtree=<path>).
# hash=sha1-only
title "gix merge -X <option>"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until merge driver lands" -- merge -X ours dev
  }
)

# mode=effect — `--strategy-option=<option>` is the long form of -X.
# hash=sha1-only
title "gix merge --strategy-option=<option>"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until merge driver lands" -- merge --strategy-option=theirs dev
  }
)

# --- message / file / into-name ----------------------------------------

# mode=effect — `-m <msg>` / `--message=<msg>` sets the commit message
# for the merge commit. Multiple -m become multi-paragraph (joined
# with blank lines). Implies `have_message` so EDITOR is not invoked.
# hash=sha1-only
title "gix merge -m <msg>"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until merge driver lands" -- merge -m "merge dev" dev
  }
)

# mode=effect — `--message=<msg>` is the long form of -m.
# hash=sha1-only
title "gix merge --message=<msg>"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until merge driver lands" -- merge --message="merge dev" dev
  }
)

# mode=effect — `-F <file>` reads the commit message from a file.
# git emits "error: could not read file '<f>'" + exit 129 when the
# file does not exist; the test pre-creates `merge-msg.txt` so the
# happy path is exercised. Both binaries exit 0 (Already up to date
# / placeholder note).
# hash=sha1-only
title "gix merge -F <file>"
only_for_hash sha1-only && (small-repo-in-sandbox
  echo "merge dev into main" > merge-msg.txt
  it "matches git behavior" && {
    compat_effect "deferred until merge driver lands" -- merge -F merge-msg.txt dev
  }
)

# mode=effect — `--file=<file>` is the long form of -F.
# hash=sha1-only
title "gix merge --file=<file>"
only_for_hash sha1-only && (small-repo-in-sandbox
  echo "merge dev into main" > merge-msg.txt
  it "matches git behavior" && {
    compat_effect "deferred until merge driver lands" -- merge --file=merge-msg.txt dev
  }
)

# mode=effect — `--into-name <branch>` prepares the default merge
# message as if merging into _<branch>_ instead of the real target.
# hash=sha1-only
title "gix merge --into-name <branch>"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until merge driver lands" -- merge --into-name release dev
  }
)

# --- verbosity: -v / -q -------------------------------------------------

# mode=effect — `-v` / `--verbose` increases verbosity (multi-count).
# OPT__VERBOSITY in C.
# hash=sha1-only
title "gix merge -v"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until merge driver lands" -- merge -v dev
  }
)

# mode=effect — `-q` / `--quiet` decreases verbosity. Implies
# --no-progress.
# hash=sha1-only
title "gix merge -q"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until merge driver lands" -- merge -q dev
  }
)

# --- in-progress transitions: --abort / --quit / --continue -------------

# mode=bytes — `--abort` (no merge in progress): git emits the
# verbatim line "fatal: There is no merge to abort (MERGE_HEAD
# missing)." + exit 128. gix's porcelain placeholder gates on
# Transitions.abort and emits the same wording verbatim before the
# bare-no-commits / revspec gates run. Bytes parity holds.
# hash=sha1-only
title "gix merge --abort (no merge in progress)"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    expect_parity bytes -- merge --abort
  }
)

# mode=bytes — `--quit` (no merge in progress): git silently exits 0
# (just removes the (absent) MERGE_HEAD ref). gix's porcelain
# placeholder gates on Transitions.quit and returns Ok(()) directly,
# bypassing the bare-no-commits gate. Bytes parity holds (both emit
# nothing).
# hash=sha1-only
title "gix merge --quit (no merge in progress)"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    expect_parity bytes -- merge --quit
  }
)

# mode=bytes — `--continue` (no merge in progress): git emits the
# verbatim line "fatal: There is no merge in progress (MERGE_HEAD
# missing)." + exit 128. gix's porcelain placeholder gates on
# Transitions.continue_ and emits the same wording verbatim.
# Bytes parity holds.
# hash=sha1-only
title "gix merge --continue (no merge in progress)"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    expect_parity bytes -- merge --continue
  }
)

# --- histories / progress -----------------------------------------------

# mode=effect — `--allow-unrelated-histories` overrides the safety
# check that refuses to merge histories without a common ancestor.
# hash=sha1-only
title "gix merge --allow-unrelated-histories"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until merge driver lands" -- merge --allow-unrelated-histories dev
  }
)

# mode=effect — `--no-allow-unrelated-histories` is the default.
# hash=sha1-only
title "gix merge --no-allow-unrelated-histories"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until merge driver lands" -- merge --no-allow-unrelated-histories dev
  }
)

# mode=effect — `--progress` forces progress reporting on (default is
# auto-detect via stderr-is-tty).
# hash=sha1-only
title "gix merge --progress"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until merge driver lands" -- merge --progress dev
  }
)

# mode=effect — `--no-progress` forces progress reporting off.
# hash=sha1-only
title "gix merge --no-progress"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until merge driver lands" -- merge --no-progress dev
  }
)

# --- GPG signing: -S / --gpg-sign / --no-gpg-sign -----------------------

# mode=effect — `-S[<key-id>]` GPG-signs the resulting merge commit.
# `require_equals = true` in the Clap shape so `-S dev` parses as
# `-S` (no key-id, default-missing-value) + positional `dev`,
# mirroring git's PARSE_OPT_OPTARG semantics where the optarg must
# be stuck without a space.
# hash=sha1-only
title "gix merge -S"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until merge driver lands" -- merge -S dev
  }
)

# mode=effect — `--gpg-sign` (long form), no key-id.
# hash=sha1-only
title "gix merge --gpg-sign"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until merge driver lands" -- merge --gpg-sign dev
  }
)

# mode=effect — `--no-gpg-sign` countermands `commit.gpgSign` config
# and earlier --gpg-sign on the command line.
# hash=sha1-only
title "gix merge --no-gpg-sign"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until merge driver lands" -- merge --no-gpg-sign dev
  }
)

# --- autostash ----------------------------------------------------------

# mode=effect — `--autostash` automatically stashes uncommitted
# changes before the merge and reapplies them after.
# hash=sha1-only
title "gix merge --autostash"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until merge driver lands" -- merge --autostash dev
  }
)

# mode=effect — `--no-autostash` is the default.
# hash=sha1-only
title "gix merge --no-autostash"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until merge driver lands" -- merge --no-autostash dev
  }
)

# --- overwrite-ignore ---------------------------------------------------

# mode=effect — `--overwrite-ignore` is the default: silently overwrite
# ignored files in the merge result.
# hash=sha1-only
title "gix merge --overwrite-ignore"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until merge driver lands" -- merge --overwrite-ignore dev
  }
)

# mode=effect — `--no-overwrite-ignore` aborts the merge instead of
# overwriting ignored files.
# hash=sha1-only
title "gix merge --no-overwrite-ignore"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until merge driver lands" -- merge --no-overwrite-ignore dev
  }
)

# --- signoff -----------------------------------------------------------

# mode=effect — `--signoff` adds a Signed-off-by trailer by the
# committer at the end of the commit log message.
# hash=sha1-only
title "gix merge --signoff"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until merge driver lands" -- merge --signoff dev
  }
)

# mode=effect — `--no-signoff` countermands `--signoff` on the command
# line.
# hash=sha1-only
title "gix merge --no-signoff"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until merge driver lands" -- merge --no-signoff dev
  }
)

# --- pre-merge / commit-msg hooks: --verify / --no-verify ---------------

# mode=effect — `--verify` is the default: pre-merge and commit-msg
# hooks run before the merge commit is made.
# hash=sha1-only
title "gix merge --verify"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until merge driver lands" -- merge --verify dev
  }
)

# mode=effect — `--no-verify` bypasses the pre-merge-commit and
# commit-msg hooks.
# hash=sha1-only
title "gix merge --no-verify"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until merge driver lands" -- merge --no-verify dev
  }
)

# End-of-file sentinel: when every row is `only_for_hash sha1-only` and the
# active hash is sha256, the last statement returns 1 (skip), which would
# propagate out of `source` and trip `set -e` in tests/parity.sh. A trailing
# `:` normalizes the exit code so a fully-skipped file still returns 0.
:
