# Must be sourced into tests/parity.sh or tests/journey.sh — see tests/parity.sh.
#
# Parity scaffold for `git pull` ↔ `gix pull`.
#
# One `title` + `it` block per flag derived from
# vendor/git/Documentation/git-pull.adoc + the inherited
# include::merge-options.adoc / fetch-options.adoc surface, plus
# vendor/git/builtin/pull.c (cmd_pull, options[] at builtin/pull.c:870..1009).
#
# Verdict modes (comment above each block):
#   bytes  — scriptable output consumed by tooling: bare-no-upstream
#            "There is no tracking information for the current branch."
#            stanza + 1, --dry-run silent exit-0 short-circuit.
#   effect — UX-level parity (exit-code match + optional prose check).
#            For -e (both binaries exit 129 with different wording),
#            --unshallow (both exit 1 but different wording).
#
# Coverage on gix's current Clap surface (src/plumbing/options/pull.rs):
#   gix pull [OPTIONS] [<repository> [<refspec>...]]
# The porcelain entry point at gitoxide-core/src/repository/pull.rs::porcelain
# implements three byte-exact gates today:
#   * --dry-run short-circuit (vendor/git/builtin/pull.c:1086 mirror) —
#     silent exit-0 even when no upstream is configured.
#   * bare-no-upstream stanza (8 lines + exit 1) — split into "merge
#     with" / "rebase against" variants based on --rebase[=...] /
#     --no-rebase, with the suggestion line's remote-name placeholder
#     ("<remote>" vs "origin") matching git's quirk where opt_rebase
#     pre-loads remote_state via get_rebase_fork_point.
#   * happy-path placeholder note + exit 0 for synopsis forms with a
#     positional <repository>, deferred until the real pull driver
#     composes fetch + merge/rebase.
# Closing this command end-to-end requires (1) implementing the pull
# driver (compose fetch + merge/rebase, FETCH_HEAD integration), (2)
# wiring per-flag passthrough into the fetch + merge sub-invocations
# so flag-bearing rows can flip from "bare-no-upstream stanza" parity
# to actual fetch+integrate bytes parity, (3) wiring the bad-revspec
# fetch-step error path so the exit-1 shape matches git.
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
#   mode=bytes

# Local helper: bare upstream + non-bare clone whose `origin` points
# at it AND whose `main` branch is configured to track `origin/main`.
# Used as the common synopsis fixture for rows with a positional
# <repository> — both binaries open the repo, gix's porcelain emits
# its stub note + exit 0, git fetches the empty round-trip + says
# "Already up to date." + exit 0.
function pull-fixture-with-tracking() {
  git init -q --bare upstream.git
  git init -q clone
  (cd clone
    git remote add origin "$(cd .. && pwd)/upstream.git"
    git config commit.gpgsign false
    git -c user.email=x@x -c user.name=x commit --allow-empty -qm init
    git checkout -b main 2>/dev/null || :
    git push -q origin main
    git branch --set-upstream-to=origin/main
  ) &>/dev/null
  cd clone
}

title "gix pull"

# --- meta / help --------------------------------------------------------

# mode=effect — clap --help short-circuits before repo load, exits 0.
# git's --help delegates to `man git-pull` (exit 0 when man is
# available); gix returns clap's auto-generated help. Message text
# diverges; only the exit-code match is asserted.
# hash=dual
# mode=effect
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
# mode=effect
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
# mode=effect
title "gix pull (outside a repository)"
only_for_hash dual && (sandbox
  it "matches git behavior" && {
    expect_parity effect -- pull
  }
)

# mode=bytes — bare `git pull` with no upstream configured: git emits
# the 8-line "There is no tracking information for the current branch."
# stanza on stderr + exits 1 (vendor/git/builtin/pull.c::cmd_pull
# → die path when `branch.<name>.merge` is unset). Exit is 1, not 128.
# Implementation in gitoxide_core::repository::pull::porcelain via
# repo.head_ref()?.remote_ref_name(Direction::Fetch) detection.
title "gix pull (bare, no upstream)"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    expect_parity bytes -- pull
  }
)

# --- synopsis: <repository> / <repository> <refspec> --------------------

# mode=effect — `git pull <remote>` against a configured local
# upstream that is already up to date: git fetches, then merge says
# "Already up to date." + exit 0. gix's placeholder emits a stub note
# and exits 0; `compat_effect` deferral until the pull driver lands.
# mode=effect
title "gix pull <remote> (already up to date)"
only_for_hash sha1-only && (sandbox
  pull-fixture-with-tracking
  it "matches git behavior" && {
    compat_effect "deferred until pull driver lands" -- pull origin
  }
)

# mode=effect — `git pull <remote> <refspec>`: git fetches the named
# branch + integrates. gix's placeholder exits 0; compat_effect.
# mode=effect
title "gix pull <remote> <refspec>"
only_for_hash sha1-only && (sandbox
  pull-fixture-with-tracking
  it "matches git behavior" && {
    compat_effect "deferred until pull driver lands" -- pull origin main
  }
)

# mode=effect — bad revspec under `git pull <remote> <bad-ref>`: git's
# fetch step fails with "fatal: couldn't find remote ref nonexistent-ref"
# + exit 1. gix's stub exits 0 because the fetch step is not yet
# composed in. Exit-code divergence is a `shortcoming` not a compat
# row (per the parity prompt's compat_effect rules).
# mode=effect
title "gix pull <remote> <bad-revspec>"
only_for_hash sha1-only && (sandbox
  pull-fixture-with-tracking
  shortcoming "bad-revspec exit-1 emerges from the fetch step; deferred until pull driver wires fetch composition"
)

# --- shared verbosity / progress ---------------------------------------

# mode=bytes — flag-only rows below all hit the bare-no-upstream gate
# in both binaries (no positional <repository> + no upstream
# configured), so the 8-line stanza is the entire output and bytes
# parity holds. Once the pull driver lands these rows flip to
# fixture-driven happy-path bytes/effect parity.

title "gix pull -v"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    expect_parity bytes -- pull -v
  }
)

title "gix pull --verbose"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    expect_parity bytes -- pull --verbose
  }
)

title "gix pull -q"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    expect_parity bytes -- pull -q
  }
)

title "gix pull --quiet"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    expect_parity bytes -- pull --quiet
  }
)

title "gix pull --progress"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    expect_parity bytes -- pull --progress
  }
)

title "gix pull --no-progress"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    expect_parity bytes -- pull --no-progress
  }
)

title "gix pull --recurse-submodules"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    expect_parity bytes -- pull --recurse-submodules
  }
)

title "gix pull --recurse-submodules=on-demand"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    expect_parity bytes -- pull --recurse-submodules=on-demand
  }
)

title "gix pull --recurse-submodules=no"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    expect_parity bytes -- pull --recurse-submodules=no
  }
)

title "gix pull --no-recurse-submodules"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    expect_parity bytes -- pull --no-recurse-submodules
  }
)

# --- merging: -r/--rebase[=VALUE] / --no-rebase ------------------------

# Rebase variants emit "rebase against" instead of "merge with" and
# substitute "origin" for "<remote>" in the suggestion line — see
# vendor/git/builtin/pull.c::die_no_merge_candidates and the
# get_rebase_fork_point side-effect that pre-loads remote_state.
# gix replicates the variant choice via rebase_active() in the
# porcelain.

title "gix pull -r"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    expect_parity bytes -- pull -r
  }
)

title "gix pull --rebase"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    expect_parity bytes -- pull --rebase
  }
)

title "gix pull --rebase=true"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    expect_parity bytes -- pull --rebase=true
  }
)

title "gix pull --rebase=false"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    expect_parity bytes -- pull --rebase=false
  }
)

title "gix pull --rebase=merges"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    expect_parity bytes -- pull --rebase=merges
  }
)

title "gix pull --rebase=interactive"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    expect_parity bytes -- pull --rebase=interactive
  }
)

title "gix pull --no-rebase"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    expect_parity bytes -- pull --no-rebase
  }
)

# --- merging: -n/--no-stat / --stat / --summary / --compact-summary ----

title "gix pull -n"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    expect_parity bytes -- pull -n
  }
)

title "gix pull --no-stat"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    expect_parity bytes -- pull --no-stat
  }
)

title "gix pull --stat"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    expect_parity bytes -- pull --stat
  }
)

title "gix pull --summary"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    expect_parity bytes -- pull --summary
  }
)

title "gix pull --no-summary"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    expect_parity bytes -- pull --no-summary
  }
)

# `--compact-summary` is a vendor/git v2.54.0 flag absent from the
# test runtime's system git (2.47.3). The version skew is a hard
# system constraint, lifted when the runtime upgrades.
title "gix pull --compact-summary"
only_for_hash sha1-only && (small-repo-in-sandbox
  shortcoming "system git 2.47.3 lacks --compact-summary; vendor/git v2.54.0 has it"
)

# --- merging: --log[=n] / --no-log -------------------------------------

title "gix pull --log"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    expect_parity bytes -- pull --log
  }
)

title "gix pull --log=<n>"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    expect_parity bytes -- pull --log=3
  }
)

title "gix pull --no-log"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    expect_parity bytes -- pull --no-log
  }
)

# --- merging: --signoff / --no-signoff ---------------------------------

title "gix pull --signoff"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    expect_parity bytes -- pull --signoff
  }
)

title "gix pull --no-signoff"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    expect_parity bytes -- pull --no-signoff
  }
)

# --- merging: --squash / --no-squash / --commit / --no-commit ----------

title "gix pull --squash"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    expect_parity bytes -- pull --squash
  }
)

title "gix pull --no-squash"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    expect_parity bytes -- pull --no-squash
  }
)

title "gix pull --commit"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    expect_parity bytes -- pull --commit
  }
)

title "gix pull --no-commit"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    expect_parity bytes -- pull --no-commit
  }
)

# --- merging: --edit / --no-edit / --cleanup ---------------------------

# `git pull` does NOT take `-e` short for `--edit` (see
# vendor/git/builtin/pull.c OPT_PASSTHRU(0, "edit", ...)). gix's
# Platform omits the short for parity. Both binaries exit 129 with
# different "unknown switch" / "unexpected argument" wording.
# mode=effect
title "gix pull -e"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    expect_parity effect -- pull -e
  }
)

title "gix pull --edit"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    expect_parity bytes -- pull --edit
  }
)

title "gix pull --no-edit"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    expect_parity bytes -- pull --no-edit
  }
)

title "gix pull --cleanup=strip"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    expect_parity bytes -- pull --cleanup=strip
  }
)

# --- merging: --ff / --no-ff / --ff-only -------------------------------

title "gix pull --ff"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    expect_parity bytes -- pull --ff
  }
)

title "gix pull --no-ff"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    expect_parity bytes -- pull --no-ff
  }
)

title "gix pull --ff-only"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    expect_parity bytes -- pull --ff-only
  }
)

# --- merging: --verify / --no-verify -----------------------------------

title "gix pull --verify"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    expect_parity bytes -- pull --verify
  }
)

title "gix pull --no-verify"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    expect_parity bytes -- pull --no-verify
  }
)

# --- merging: --verify-signatures / --no-verify-signatures -------------

title "gix pull --verify-signatures"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    expect_parity bytes -- pull --verify-signatures
  }
)

title "gix pull --no-verify-signatures"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    expect_parity bytes -- pull --no-verify-signatures
  }
)

# --- merging: --autostash / --no-autostash -----------------------------

title "gix pull --autostash"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    expect_parity bytes -- pull --autostash
  }
)

title "gix pull --no-autostash"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    expect_parity bytes -- pull --no-autostash
  }
)

# --- merging: -s/--strategy / -X/--strategy-option ---------------------

title "gix pull -s ort"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    expect_parity bytes -- pull -s ort
  }
)

title "gix pull --strategy=ort"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    expect_parity bytes -- pull --strategy=ort
  }
)

title "gix pull -X theirs"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    expect_parity bytes -- pull -X theirs
  }
)

title "gix pull --strategy-option=theirs"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    expect_parity bytes -- pull --strategy-option=theirs
  }
)

# --- merging: -S/--gpg-sign / --no-gpg-sign ----------------------------

title "gix pull -S"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    expect_parity bytes -- pull -S
  }
)

title "gix pull --gpg-sign"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    expect_parity bytes -- pull --gpg-sign
  }
)

title "gix pull --gpg-sign=<key-id>"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    expect_parity bytes -- pull --gpg-sign=ABCDEF12
  }
)

title "gix pull --no-gpg-sign"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    expect_parity bytes -- pull --no-gpg-sign
  }
)

# --- merging: --allow-unrelated-histories ------------------------------

title "gix pull --allow-unrelated-histories"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    expect_parity bytes -- pull --allow-unrelated-histories
  }
)

# --- fetching: --all ---------------------------------------------------

title "gix pull --all"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    expect_parity bytes -- pull --all
  }
)

# --- fetching: -a/--append ---------------------------------------------

# `-a`/`--append` triggers run_fetch's `--append` mode, which opens
# `.git/FETCH_HEAD` for appending BEFORE the merge-candidates check.
# In a fresh repo with no FETCH_HEAD this errors 128 with "fatal:
# could not open '.git/FETCH_HEAD' for reading: No such file or
# directory". gix's stub hits the bare-no-upstream gate at exit 1
# before any fetch dance, so exit codes diverge until the pull driver
# wires the fetch step.

title "gix pull -a"
only_for_hash sha1-only && (small-repo-in-sandbox
  shortcoming "git fetch --append precedes merge-candidates check; deferred until pull driver wires fetch composition"
)

title "gix pull --append"
only_for_hash sha1-only && (small-repo-in-sandbox
  shortcoming "git fetch --append precedes merge-candidates check; deferred until pull driver wires fetch composition"
)

# --- fetching: --upload-pack -------------------------------------------

title "gix pull --upload-pack=<path>"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    expect_parity bytes -- pull --upload-pack=/usr/bin/git-upload-pack
  }
)

# --- fetching: -f/--force ----------------------------------------------

title "gix pull -f"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    expect_parity bytes -- pull -f
  }
)

title "gix pull --force"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    expect_parity bytes -- pull --force
  }
)

# --- fetching: -t/--tags / -p/--prune ----------------------------------

title "gix pull -t"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    expect_parity bytes -- pull -t
  }
)

title "gix pull --tags"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    expect_parity bytes -- pull --tags
  }
)

title "gix pull -p"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    expect_parity bytes -- pull -p
  }
)

title "gix pull --prune"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    expect_parity bytes -- pull --prune
  }
)

# --- fetching: -j/--jobs / --dry-run -----------------------------------

title "gix pull --jobs=4"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    expect_parity bytes -- pull --jobs=4
  }
)

# `--dry-run` short-circuits the merge-candidates check in cmd_pull
# (vendor/git/builtin/pull.c:1086 `if (opt_dry_run) return 0`), so
# git silently exits 0 even with no upstream. gix's porcelain mirrors
# the short-circuit before the bare-no-upstream gate fires.
title "gix pull --dry-run"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    expect_parity bytes -- pull --dry-run
  }
)

# --- fetching: -k/--keep -----------------------------------------------

title "gix pull -k"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    expect_parity bytes -- pull -k
  }
)

title "gix pull --keep"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    expect_parity bytes -- pull --keep
  }
)

# --- fetching: shallow flags -------------------------------------------

title "gix pull --depth=1"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    expect_parity bytes -- pull --depth=1
  }
)

title "gix pull --shallow-since=<date>"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    expect_parity bytes -- pull --shallow-since=2024-01-01
  }
)

title "gix pull --shallow-exclude=<ref>"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    expect_parity bytes -- pull --shallow-exclude=v1.0
  }
)

title "gix pull --deepen=1"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    expect_parity bytes -- pull --deepen=1
  }
)

# `--unshallow` on a complete repo: git emits "fatal: --unshallow on
# a complete repository does not make sense" + exit 1. gix's stub
# emits the no-upstream stanza + exit 1. Bytes diverge, exit codes
# match → effect-mode parity.
# mode=effect
title "gix pull --unshallow"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    expect_parity effect -- pull --unshallow
  }
)

title "gix pull --update-shallow"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    expect_parity bytes -- pull --update-shallow
  }
)

# --- fetching: --refmap ------------------------------------------------

title "gix pull --refmap=<refspec>"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    expect_parity bytes -- pull --refmap=refs/heads/*:refs/remotes/origin/*
  }
)

# --- fetching: -o/--server-option --------------------------------------

title "gix pull -o <opt>"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    expect_parity bytes -- pull -o feature=on
  }
)

title "gix pull --server-option=<opt>"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    expect_parity bytes -- pull --server-option=feature=on
  }
)

# --- fetching: -4/-6 ipv family ----------------------------------------

title "gix pull -4"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    expect_parity bytes -- pull -4
  }
)

title "gix pull --ipv4"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    expect_parity bytes -- pull --ipv4
  }
)

title "gix pull -6"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    expect_parity bytes -- pull -6
  }
)

title "gix pull --ipv6"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    expect_parity bytes -- pull --ipv6
  }
)

# --- fetching: --negotiation-tip ---------------------------------------

title "gix pull --negotiation-tip=<rev>"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    expect_parity bytes -- pull --negotiation-tip=HEAD
  }
)

# --- fetching: --show-forced-updates / --no-show-forced-updates --------

title "gix pull --show-forced-updates"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    expect_parity bytes -- pull --show-forced-updates
  }
)

title "gix pull --no-show-forced-updates"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    expect_parity bytes -- pull --no-show-forced-updates
  }
)

# --- fetching: --set-upstream ------------------------------------------

title "gix pull --set-upstream"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    expect_parity bytes -- pull --set-upstream
  }
)
