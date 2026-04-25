# Must be sourced into tests/parity.sh or tests/journey.sh — see tests/parity.sh.
#
# Parity scaffold for `git rebase` ↔ `gix rebase`.
#
# One `title` + `it` block per flag derived from
# vendor/git/Documentation/git-rebase.adoc +
# vendor/git/builtin/rebase.c (cmd_rebase, builtin_rebase_options[]
# at vendor/git/builtin/rebase.c:1120..1247). Every `it` body starts
# as a TODO placeholder — iteration N of the ralph loop picks the
# next TODO, converts it to a real `expect_parity` (or `compat_effect`)
# assertion, and removes the TODO marker.
#
# Verdict modes (comment above each block):
#   bytes  — scriptable output consumed by tooling: error stanzas
#            around bad revspecs, "Current branch <name> is up to
#            date." short-circuit, the verbatim "There is no tracking
#            information for the current branch." stanza printed to
#            stdout (NOT stderr — see vendor/git/builtin/rebase.c:1058
#            error_on_missing_default_upstream calls printf, not
#            fprintf(stderr)) when no upstream is configured, the
#            "fatal: No rebase in progress?" wording around the
#            in-progress transitions (--abort / --quit / --continue
#            / --skip / --edit-todo / --show-current-patch).
#   effect — UX-level parity (exit-code match + optional prose check).
#            Default for the human-rendered flags whose pretty
#            rendering is not yet implemented in gix's rebase entry
#            point. Most rows close as `compat_effect` until the
#            rebase driver lands.
#
# Coverage on gix's current Clap surface (src/plumbing/options/rebase.rs):
#   gix rebase [OPTIONS] [<upstream> [<branch>]]
# Today the porcelain driver is a placeholder
# (gitoxide-core/src/repository/rebase.rs::porcelain) that emits a
# stub note on stderr and exits 0, *except* for the bare-no-upstream
# branch path where it emits git's verbatim 9-line stanza on stdout
# and exits 1 (matching error_on_missing_default_upstream). The flag
# surface is clap-wired so `gix rebase <flag> ...` does not trip
# UnknownArgument; every flag-bearing row therefore closes as
# `compat_effect "<reason>"` under the shared deferral phrase
# "deferred until rebase driver lands" until the real driver
# implements the semantic. Closing this command requires (1)
# implementing the rebase driver in
# gitoxide-core/src/repository/rebase.rs (revision-walk + cherry-pick
# replay; apply vs. merge backend; --interactive todo-list
# emission/edit/replay; --rebase-merges branching-structure preservation),
# (2) wiring the in-progress transitions (--abort / --quit /
# --continue / --skip / --edit-todo / --show-current-patch) to
# gix-ref + gix-status precondition checks for `.git/rebase-merge/` /
# `.git/rebase-apply/` state directories, (3) translating C-side
# invariants in vendor/git/builtin/rebase.c (apply vs. merge backend
# selection state machine, can_fast_forward shortcut at L894,
# REBASE_FORCE / REBASE_VERBOSE / REBASE_DIFFSTAT bit interlocks,
# fork-point / keep-base / onto compatibility checks at the
# INCOMPATIBLE OPTIONS surface).
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

# --- meta / help --------------------------------------------------------

# mode=effect — clap --help short-circuits before repo load, exits 0.
# git's --help delegates to `man git-rebase` (exit 0 when man is
# available); gix returns clap's auto-generated help. Message text
# diverges; only the exit-code match is asserted.
# hash=dual
title "gix rebase --help"
only_for_hash dual && (sandbox
  it "matches git behavior" && {
    expect_parity effect -- rebase --help
  }
)

# mode=effect — unknown flag: git exits 129 (usage_msg_opt in
# parse-options.c). gix's Clap layer maps UnknownArgument to 129 via
# src/plumbing/main.rs. Tested inside a repo because git outside a
# repo dies 128 before reaching arg-parse, while clap in gix always
# runs first.
# hash=sha1-only
title "gix rebase --bogus-flag"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    expect_parity effect -- rebase --bogus-flag
  }
)

# mode=bytes — bare `git rebase` with no args and no upstream
# configured: git emits the verbatim 9-line "There is no tracking
# information for the current branch." stanza on **stdout** (not
# stderr — see vendor/git/builtin/rebase.c:1058
# error_on_missing_default_upstream calls printf) + exit 1. gix's
# porcelain placeholder gates on (upstream.is_none() &&
# branch.is_none() && opts.onto.is_none() && !opts.root) and emits
# the same stanza + exits 1; bytes parity holds even though the
# underlying driver is still a stub.
# hash=sha1-only
title "gix rebase (bare, no upstream)"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    expect_parity bytes -- rebase
  }
)

# mode=effect — outside any repo: git dies 128 with "fatal: not a git
# repository (or any of the parent directories): .git". gix's plumbing
# repository() closure already remaps gix_discover::upwards::Error::
# NoGitRepository* variants to git's exact wording + exit 128.
# hash=dual
title "gix rebase (outside a repository)"
only_for_hash dual && (sandbox
  it "matches git behavior" && {
    expect_parity effect -- rebase main
  }
)

# --- synopsis: <upstream> form ------------------------------------------

# mode=effect — `git rebase <upstream>` where <upstream> resolves to
# HEAD itself (or an ancestor where HEAD is already at the tip): git
# emits "Current branch <name> is up to date." + exits 0. gix's
# placeholder emits a stub note (different bytes) + exits 0;
# exit-code parity holds. Close as `compat_effect`.
# hash=sha1-only
title "gix rebase <upstream> (already up to date)"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until rebase driver lands" -- rebase main
  }
)

# mode=effect — `git rebase <upstream>` where <upstream> is a
# descendant of HEAD: fast-forward; git updates HEAD and emits
# "Successfully rebased and updated refs/heads/<name>." + exit 0.
# gix's placeholder emits a stub note + exit 0; exit-code parity
# holds. Close as `compat_effect`.
# hash=sha1-only
title "gix rebase <upstream> (fast-forward)"
only_for_hash sha1-only && (small-repo-in-sandbox
  git checkout -q dev
  it "matches git behavior" && {
    compat_effect "deferred until rebase driver lands" -- rebase main
  }
)

# mode=effect — `git rebase <upstream> <branch>`: implicit checkout
# of <branch> before rebasing. Documented at
# Documentation/git-rebase.adoc:33 ("`git rebase master topic` is a
# shortcut for `git checkout topic && git rebase master`").
# hash=sha1-only
title "gix rebase <upstream> <branch>"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until rebase driver lands" -- rebase main dev
  }
)

# mode=bytes — bad revspec: git emits the verbatim line
# "fatal: invalid upstream '<ref>'" + exit 128. gix's porcelain
# rev-parses the upstream positional via `repo.rev_parse_single`
# before the placeholder happy path; on parse failure it emits
# git's verbatim wording and exits 128.
# hash=sha1-only
title "gix rebase <bad-revspec>"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    expect_parity bytes -- rebase nonexistent-branch
  }
)

# --- starting-point control ---------------------------------------------

# hash=sha1-only
title "gix rebase --onto"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until rebase driver lands" -- rebase --onto main main dev
  }
)

# hash=sha1-only
title "gix rebase --keep-base"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until rebase driver lands" -- rebase --keep-base main
  }
)

# hash=sha1-only
title "gix rebase --root"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until rebase driver lands" -- rebase --root
  }
)

# --- in-progress transitions (cmdmode group) ----------------------------

# mode=bytes — git emits "fatal: No rebase in progress?" + exit 128
# when no .git/rebase-merge/ or .git/rebase-apply/ state dir exists.
# Mirrors vendor/git/builtin/rebase.c get_replay_opts gate.
# hash=sha1-only
title "gix rebase --continue"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    expect_parity bytes -- rebase --continue
  }
)

# mode=bytes
# hash=sha1-only
title "gix rebase --skip"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    expect_parity bytes -- rebase --skip
  }
)

# mode=bytes
# hash=sha1-only
title "gix rebase --abort"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    expect_parity bytes -- rebase --abort
  }
)

# mode=bytes
# hash=sha1-only
title "gix rebase --quit"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    expect_parity bytes -- rebase --quit
  }
)

# mode=bytes
# hash=sha1-only
title "gix rebase --edit-todo"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    expect_parity bytes -- rebase --edit-todo
  }
)

# mode=bytes
# hash=sha1-only
title "gix rebase --show-current-patch"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    expect_parity bytes -- rebase --show-current-patch
  }
)

# --- backend selection --------------------------------------------------

# hash=sha1-only
title "gix rebase --apply"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until rebase driver lands" -- rebase --apply main
  }
)

# hash=sha1-only
title "gix rebase --merge"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until rebase driver lands" -- rebase --merge main
  }
)

# hash=sha1-only
title "gix rebase -m"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until rebase driver lands" -- rebase -m main
  }
)

# hash=sha1-only
title "gix rebase --interactive"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until rebase driver lands" -- rebase --interactive main
  }
)

# hash=sha1-only
title "gix rebase -i"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until rebase driver lands" -- rebase -i main
  }
)

# --- empty / cherry-pick handling ---------------------------------------

# hash=sha1-only
title "gix rebase --empty=drop"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until rebase driver lands" -- rebase --empty=drop main
  }
)

# hash=sha1-only
title "gix rebase --empty=keep"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until rebase driver lands" -- rebase --empty=keep main
  }
)

# hash=sha1-only
title "gix rebase --empty=stop"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until rebase driver lands" -- rebase --empty=stop main
  }
)

# hash=sha1-only
title "gix rebase --keep-empty"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until rebase driver lands" -- rebase --keep-empty main
  }
)

# hash=sha1-only
title "gix rebase --no-keep-empty"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until rebase driver lands" -- rebase --no-keep-empty main
  }
)

# hash=sha1-only
title "gix rebase --reapply-cherry-picks"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until rebase driver lands" -- rebase --reapply-cherry-picks main
  }
)

# hash=sha1-only
title "gix rebase --no-reapply-cherry-picks"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until rebase driver lands" -- rebase --no-reapply-cherry-picks main
  }
)

# hash=sha1-only
title "gix rebase --allow-empty-message"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until rebase driver lands" -- rebase --allow-empty-message main
  }
)

# --- strategy / merge-driver tuning -------------------------------------

# hash=sha1-only
title "gix rebase --strategy=ort"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until rebase driver lands" -- rebase --strategy=ort main
  }
)

# hash=sha1-only
title "gix rebase -s ort"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until rebase driver lands" -- rebase -s ort main
  }
)

# hash=sha1-only
title "gix rebase --strategy-option=ours"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until rebase driver lands" -- rebase --strategy-option=ours main
  }
)

# hash=sha1-only
title "gix rebase -X ours"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until rebase driver lands" -- rebase -X ours main
  }
)

# hash=sha1-only
title "gix rebase --rebase-merges"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until rebase driver lands" -- rebase --rebase-merges main
  }
)

# hash=sha1-only
title "gix rebase --rebase-merges=rebase-cousins"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until rebase driver lands" -- rebase --rebase-merges=rebase-cousins main
  }
)

# hash=sha1-only
title "gix rebase --rebase-merges=no-rebase-cousins"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until rebase driver lands" -- rebase --rebase-merges=no-rebase-cousins main
  }
)

# hash=sha1-only
title "gix rebase --no-rebase-merges"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until rebase driver lands" -- rebase --no-rebase-merges main
  }
)

# hash=sha1-only
title "gix rebase -r"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until rebase driver lands" -- rebase -r main
  }
)

# --- force / fork-point -------------------------------------------------

# hash=sha1-only
title "gix rebase --force-rebase"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until rebase driver lands" -- rebase --force-rebase main
  }
)

# hash=sha1-only
title "gix rebase -f"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until rebase driver lands" -- rebase -f main
  }
)

# hash=sha1-only
title "gix rebase --no-ff"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until rebase driver lands" -- rebase --no-ff main
  }
)

# hash=sha1-only
title "gix rebase --fork-point"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until rebase driver lands" -- rebase --fork-point main
  }
)

# hash=sha1-only
title "gix rebase --no-fork-point"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until rebase driver lands" -- rebase --no-fork-point main
  }
)

# --- interactive companions ---------------------------------------------

# hash=sha1-only
title "gix rebase --exec"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until rebase driver lands" -- rebase --exec "true" main
  }
)

# hash=sha1-only
title "gix rebase -x"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until rebase driver lands" -- rebase -x "true" main
  }
)

# hash=sha1-only
title "gix rebase --autosquash"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until rebase driver lands" -- rebase --autosquash main
  }
)

# hash=sha1-only
title "gix rebase --no-autosquash"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until rebase driver lands" -- rebase --no-autosquash main
  }
)

# hash=sha1-only
title "gix rebase --reschedule-failed-exec"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until rebase driver lands" -- rebase --reschedule-failed-exec main
  }
)

# hash=sha1-only
title "gix rebase --no-reschedule-failed-exec"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until rebase driver lands" -- rebase --no-reschedule-failed-exec main
  }
)

# hash=sha1-only
title "gix rebase --update-refs"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until rebase driver lands" -- rebase --update-refs main
  }
)

# hash=sha1-only
title "gix rebase --no-update-refs"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until rebase driver lands" -- rebase --no-update-refs main
  }
)

# --- verbosity / diffstat -----------------------------------------------

# hash=sha1-only
title "gix rebase --quiet"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until rebase driver lands" -- rebase --quiet main
  }
)

# hash=sha1-only
title "gix rebase -q"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until rebase driver lands" -- rebase -q main
  }
)

# hash=sha1-only
title "gix rebase --verbose"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until rebase driver lands" -- rebase --verbose main
  }
)

# hash=sha1-only
title "gix rebase -v"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until rebase driver lands" -- rebase -v main
  }
)

# hash=sha1-only
title "gix rebase --stat"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until rebase driver lands" -- rebase --stat main
  }
)

# hash=sha1-only
title "gix rebase --no-stat"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until rebase driver lands" -- rebase --no-stat main
  }
)

# hash=sha1-only
title "gix rebase -n"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until rebase driver lands" -- rebase -n main
  }
)

# --- hooks --------------------------------------------------------------

# hash=sha1-only
title "gix rebase --no-verify"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until rebase driver lands" -- rebase --no-verify main
  }
)

# hash=sha1-only
title "gix rebase --verify"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until rebase driver lands" -- rebase --verify main
  }
)

# --- apply-backend passthroughs -----------------------------------------

# hash=sha1-only
title "gix rebase -C"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until rebase driver lands" -- rebase -C 5 main
  }
)

# hash=sha1-only
title "gix rebase --ignore-whitespace"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until rebase driver lands" -- rebase --ignore-whitespace main
  }
)

# hash=sha1-only
title "gix rebase --whitespace=fix"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until rebase driver lands" -- rebase --whitespace=fix main
  }
)

# --- trailers / signoff / authorship ------------------------------------

# mode=effect — `--trailer` was added to git-rebase upstream but is
# absent from system git 2.47.3 (vendor/git v2.54.0 has it). System
# git emits "error: unknown option `trailer'" + exit 129; gix's clap
# accepts the flag and exits 0. Hard system-version constraint —
# closes as `shortcoming` until the test runtime upgrades.
# hash=sha1-only
title "gix rebase --trailer"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    shortcoming "system git 2.47.3 lacks --trailer; vendor/git v2.54.0 has it"
  }
)

# hash=sha1-only
title "gix rebase --signoff"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until rebase driver lands" -- rebase --signoff main
  }
)

# hash=sha1-only
title "gix rebase --committer-date-is-author-date"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until rebase driver lands" -- rebase --committer-date-is-author-date main
  }
)

# hash=sha1-only
title "gix rebase --reset-author-date"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until rebase driver lands" -- rebase --reset-author-date main
  }
)

# hash=sha1-only
title "gix rebase --ignore-date"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until rebase driver lands" -- rebase --ignore-date main
  }
)

# --- rerere -------------------------------------------------------------

# hash=sha1-only
title "gix rebase --rerere-autoupdate"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until rebase driver lands" -- rebase --rerere-autoupdate main
  }
)

# hash=sha1-only
title "gix rebase --no-rerere-autoupdate"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until rebase driver lands" -- rebase --no-rerere-autoupdate main
  }
)

# --- autostash ----------------------------------------------------------

# hash=sha1-only
title "gix rebase --autostash"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until rebase driver lands" -- rebase --autostash main
  }
)

# hash=sha1-only
title "gix rebase --no-autostash"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until rebase driver lands" -- rebase --no-autostash main
  }
)

# --- GPG signing --------------------------------------------------------

# hash=sha1-only
title "gix rebase --gpg-sign"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until rebase driver lands" -- rebase --gpg-sign main
  }
)

# hash=sha1-only
title "gix rebase --gpg-sign=keyid"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until rebase driver lands" -- rebase --gpg-sign=KEYID main
  }
)

# hash=sha1-only
title "gix rebase -S"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until rebase driver lands" -- rebase -S main
  }
)

# hash=sha1-only
title "gix rebase --no-gpg-sign"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    compat_effect "deferred until rebase driver lands" -- rebase --no-gpg-sign main
  }
)
