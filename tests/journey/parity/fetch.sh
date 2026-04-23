# Must be sourced into tests/parity.sh or tests/journey.sh — see tests/parity.sh.
#
# Parity scaffold for `git fetch` ↔ `gix fetch`.
#
# One `title` + `it` block per flag derived from
# vendor/git/builtin/fetch.c (`builtin_fetch_options`) and
# vendor/git/Documentation/fetch-options.adoc. Every `it` body starts as a
# TODO: placeholder — iteration N of the ralph loop picks the next TODO,
# converts it to a real `expect_parity` assertion, and removes the TODO
# marker.
#
# Verdict modes (comment above each block):
#   bytes  — scriptable output; byte-exact match required (e.g. --porcelain)
#   effect — exit-code + UX; output diff reported but not fatal
#
# ─── PARITY STATE (iter 1) ────────────────────────────────────────────────
# Scaffolding only. All rows below are TODO placeholders. `gix fetch` exists
# (src/plumbing/options/mod.rs::fetch, dispatched via
# gitoxide-core::repository::fetch), but its flag surface is gix-native and
# does not yet mirror git-fetch. Iterations 2..N will (a) expand the Clap
# surface in src/plumbing/options/fetch.rs to cover the git-fetch flags
# enumerated below, and (b) wire each flag's semantics through
# gix::Repository::fetch (or land the upstream invariants it needs in
# gix-protocol / gix-refspec / gix-negotiate, leaf-first).
#
# Every row is `# hash=sha1-only` — `gix fetch` cannot open sha256 remotes
# yet (gix/src/clone/fetch/mod.rs:278 still `unimplemented!()`s hash-change
# reconfiguration). Once that lands, rows become `dual`.
# ──────────────────────────────────────────────────────────────────────────

# hash=sha1-only "gix cannot open sha256 remotes, see gix/src/clone/fetch/mod.rs unimplemented!()"
title "gix fetch"

# --- meta / help ----------------------------------------------------------

# mode=effect — `git fetch --help` delegates to man git-fetch (exit 0 when man
# is available); gix returns Clap's auto-generated help (exit 0). Message
# text diverges wildly and is NOT asserted — this row guards only the
# exit-code contract that `--help` is a benign, zero-exit operation.
# hash=sha1-only "gix cannot open sha256 remotes, see gix/src/clone/fetch/mod.rs unimplemented!()"
title "gix fetch --help"
only_for_hash sha1-only && (sandbox
  it "matches git: --help exits 0" && {
    expect_parity effect -- fetch --help
  }
)

# --- error paths (pre-transport validation) -------------------------------

# mode=effect — mirrors the `if (!remote) die("No remote repository specified")`
# path at the bottom of cmd_fetch. When no remote is configured and no
# positional repository is given, both binaries die 128.
# hash=sha1-only "gix cannot open sha256 remotes, see gix/src/clone/fetch/mod.rs unimplemented!()"
title "gix fetch (no configured remote)"
only_for_hash sha1-only && (sandbox
  it "TODO: matches git: bare 'fetch' in a repo with no remotes" && {
    :  # expect_parity effect -- fetch
  }
)

# mode=effect — mirrors `remote_get` returning NULL for an empty or invalid
# repository name. Exit 128.
# hash=sha1-only "gix cannot open sha256 remotes, see gix/src/clone/fetch/mod.rs unimplemented!()"
title "gix fetch '' (bad repository: empty name)"
only_for_hash sha1-only && (sandbox
  it "TODO: matches git: positional empty repository name" && {
    :  # expect_parity effect -- fetch ''
  }
)

# mode=effect — mirrors `die("fetch --all does not take a repository argument")`
# in cmd_fetch after parse_options. Exit 128.
# hash=sha1-only "gix cannot open sha256 remotes, see gix/src/clone/fetch/mod.rs unimplemented!()"
title "gix fetch --all <repository> (conflict: repo arg)"
only_for_hash sha1-only && (sandbox
  it "TODO: matches git: --all with a positional repository" && {
    :  # expect_parity effect -- fetch --all origin
  }
)

# mode=effect — mirrors `die("fetch --all does not make sense with refspecs")`
# in cmd_fetch. Exit 128.
# hash=sha1-only "gix cannot open sha256 remotes, see gix/src/clone/fetch/mod.rs unimplemented!()"
title "gix fetch --all <repo> <refspec> (conflict: refspecs)"
only_for_hash sha1-only && (sandbox
  it "TODO: matches git: --all with refspecs" && {
    :  # expect_parity effect -- fetch --all origin main
  }
)

# mode=effect — mirrors `die("--negotiate-only needs one or more --negotiation-tip=*")`
# in cmd_fetch. Exit 128 before remote resolution.
# hash=sha1-only "gix cannot open sha256 remotes, see gix/src/clone/fetch/mod.rs unimplemented!()"
title "gix fetch --negotiate-only (without --negotiation-tip)"
only_for_hash sha1-only && (sandbox
  it "TODO: matches git: --negotiate-only without any tips" && {
    :  # expect_parity effect -- fetch --negotiate-only origin
  }
)

# mode=effect — mirrors `die("options '--negotiate-only' and '--recurse-submodules' cannot be used together")`
# when --negotiate-only is combined with --recurse-submodules=yes/on-demand.
# hash=sha1-only "gix cannot open sha256 remotes, see gix/src/clone/fetch/mod.rs unimplemented!()"
title "gix fetch --negotiate-only --recurse-submodules=yes"
only_for_hash sha1-only && (sandbox
  it "TODO: matches git: conflict dies 128" && {
    :  # expect_parity effect -- fetch --negotiate-only --recurse-submodules=yes origin
  }
)

# mode=effect — mirrors `die("options '--porcelain' and '--recurse-submodules' cannot be used together")`
# for non-off recurse-submodules values.
# hash=sha1-only "gix cannot open sha256 remotes, see gix/src/clone/fetch/mod.rs unimplemented!()"
title "gix fetch --porcelain --recurse-submodules=yes"
only_for_hash sha1-only && (sandbox
  it "TODO: matches git: conflict dies 128" && {
    :  # expect_parity effect -- fetch --porcelain --recurse-submodules=yes origin
  }
)

# mode=effect — mirrors `die("options '--deepen' and '--depth' cannot be used together")`
# in cmd_fetch post-parse validation.
# hash=sha1-only "gix cannot open sha256 remotes, see gix/src/clone/fetch/mod.rs unimplemented!()"
title "gix fetch --deepen N --depth M (conflict)"
only_for_hash sha1-only && (sandbox
  it "TODO: matches git: --deepen combined with --depth dies 128" && {
    :  # expect_parity effect -- fetch --deepen 1 --depth 2 origin
  }
)

# mode=effect — mirrors `die("options '--depth' and '--unshallow' cannot be used together")`
# in cmd_fetch post-parse validation.
# hash=sha1-only "gix cannot open sha256 remotes, see gix/src/clone/fetch/mod.rs unimplemented!()"
title "gix fetch --depth N --unshallow (conflict)"
only_for_hash sha1-only && (sandbox
  it "TODO: matches git: --depth combined with --unshallow dies 128" && {
    :  # expect_parity effect -- fetch --depth 1 --unshallow origin
  }
)

# mode=effect — mirrors `die("--unshallow on a complete repository does not make sense")`
# when the repo is not shallow.
# hash=sha1-only "gix cannot open sha256 remotes, see gix/src/clone/fetch/mod.rs unimplemented!()"
title "gix fetch --unshallow (on a complete repository)"
only_for_hash sha1-only && (sandbox
  it "TODO: matches git: --unshallow on non-shallow repo dies 128" && {
    :  # expect_parity effect -- fetch --unshallow origin
  }
)

# mode=effect — mirrors `die("negative depth in --deepen is not supported")`.
# hash=sha1-only "gix cannot open sha256 remotes, see gix/src/clone/fetch/mod.rs unimplemented!()"
title "gix fetch --deepen=-1 (negative)"
only_for_hash sha1-only && (sandbox
  it "TODO: matches git: --deepen negative dies 128" && {
    :  # expect_parity effect -- fetch --deepen=-1 origin
  }
)

# mode=effect — mirrors `die("depth %s is not a positive number")` when
# --depth is 0 or negative.
# hash=sha1-only "gix cannot open sha256 remotes, see gix/src/clone/fetch/mod.rs unimplemented!()"
title "gix fetch --depth=0 (non-positive)"
only_for_hash sha1-only && (sandbox
  it "TODO: matches git: --depth=0 dies 128" && {
    :  # expect_parity effect -- fetch --depth=0 origin
  }
)

# --- happy path: named-remote round trip ----------------------------------

# mode=effect — baseline happy-path. A named `origin` remote pointing at a
# bare file:// sibling, fetching all refs; exit 0 from both binaries.
# hash=sha1-only "gix cannot open sha256 remotes, see gix/src/clone/fetch/mod.rs unimplemented!()"
title "gix fetch origin (named remote, bare file:// upstream)"
only_for_hash sha1-only && (sandbox
  it "TODO: matches git: fetch from named origin" && {
    :  # expect_parity effect -- fetch origin
  }
)

# mode=effect — the zero-arg form: no positional, remote.<branch>.remote or
# origin is chosen implicitly.
# hash=sha1-only "gix cannot open sha256 remotes, see gix/src/clone/fetch/mod.rs unimplemented!()"
title "gix fetch (zero-arg, implicit origin)"
only_for_hash sha1-only && (sandbox
  it "TODO: matches git: bare 'fetch' uses implicit remote" && {
    :  # expect_parity effect -- fetch
  }
)

# mode=effect — URL-as-repository: anonymous remote (no stored config).
# hash=sha1-only "gix cannot open sha256 remotes, see gix/src/clone/fetch/mod.rs unimplemented!()"
title "gix fetch <url> (anonymous URL remote)"
only_for_hash sha1-only && (sandbox
  it "TODO: matches git: fetch from URL, no named remote" && {
    :  # expect_parity effect -- fetch file:///path/to/upstream
  }
)

# mode=effect — explicit refspec narrowing.
# hash=sha1-only "gix cannot open sha256 remotes, see gix/src/clone/fetch/mod.rs unimplemented!()"
title "gix fetch <remote> <refspec>"
only_for_hash sha1-only && (sandbox
  it "TODO: matches git: fetch origin refs/heads/main:refs/remotes/origin/main" && {
    :  # expect_parity effect -- fetch origin refs/heads/main:refs/remotes/origin/main
  }
)

# --- flags ----------------------------------------------------------------

# mode=effect — --all fetches all configured remotes.
# hash=sha1-only "gix cannot open sha256 remotes, see gix/src/clone/fetch/mod.rs unimplemented!()"
title "gix fetch --all"
only_for_hash sha1-only && (sandbox
  it "TODO: matches git: --all with multiple remotes" && {
    :  # expect_parity effect -- fetch --all
  }
)

# mode=effect — --append appends to .git/FETCH_HEAD rather than overwriting.
# hash=sha1-only "gix cannot open sha256 remotes, see gix/src/clone/fetch/mod.rs unimplemented!()"
title "gix fetch --append / -a"
only_for_hash sha1-only && (sandbox
  it "TODO: matches git: --append preserves FETCH_HEAD lines" && {
    :  # expect_parity effect -- fetch --append origin
  }
)

# mode=effect — --atomic asks the ref transaction to succeed or fail as a
# whole.
# hash=sha1-only "gix cannot open sha256 remotes, see gix/src/clone/fetch/mod.rs unimplemented!()"
title "gix fetch --atomic"
only_for_hash sha1-only && (sandbox
  it "TODO: matches git: --atomic updates refs as one transaction" && {
    :  # expect_parity effect -- fetch --atomic origin
  }
)

# mode=effect — --depth limits history depth from the tip of each remote
# branch.
# hash=sha1-only "gix cannot open sha256 remotes, see gix/src/clone/fetch/mod.rs unimplemented!()"
title "gix fetch --depth=<n>"
only_for_hash sha1-only && (sandbox
  it "TODO: matches git: --depth=1 yields a shallow clone" && {
    :  # expect_parity effect -- fetch --depth=1 origin
  }
)

# mode=effect — --deepen extends the shallow boundary by N commits.
# hash=sha1-only "gix cannot open sha256 remotes, see gix/src/clone/fetch/mod.rs unimplemented!()"
title "gix fetch --deepen=<n>"
only_for_hash sha1-only && (sandbox
  it "TODO: matches git: --deepen=1 extends a shallow clone" && {
    :  # expect_parity effect -- fetch --deepen=1 origin
  }
)

# mode=effect — --shallow-since cuts off history past a date.
# hash=sha1-only "gix cannot open sha256 remotes, see gix/src/clone/fetch/mod.rs unimplemented!()"
title "gix fetch --shallow-since=<date>"
only_for_hash sha1-only && (sandbox
  it "TODO: matches git: --shallow-since=<date> narrows history" && {
    :  # expect_parity effect -- fetch --shallow-since='2020-01-01' origin
  }
)

# mode=effect — --shallow-exclude excludes a named ref/tag from the history.
# hash=sha1-only "gix cannot open sha256 remotes, see gix/src/clone/fetch/mod.rs unimplemented!()"
title "gix fetch --shallow-exclude=<ref>"
only_for_hash sha1-only && (sandbox
  it "TODO: matches git: --shallow-exclude=v1 narrows history" && {
    :  # expect_parity effect -- fetch --shallow-exclude=v1 origin
  }
)

# mode=effect — --unshallow promotes a shallow clone to complete.
# hash=sha1-only "gix cannot open sha256 remotes, see gix/src/clone/fetch/mod.rs unimplemented!()"
title "gix fetch --unshallow"
only_for_hash sha1-only && (sandbox
  it "TODO: matches git: --unshallow fills in history" && {
    :  # expect_parity effect -- fetch --unshallow origin
  }
)

# mode=effect — --update-shallow accepts refs that require updating
# .git/shallow.
# hash=sha1-only "gix cannot open sha256 remotes, see gix/src/clone/fetch/mod.rs unimplemented!()"
title "gix fetch --update-shallow"
only_for_hash sha1-only && (sandbox
  it "TODO: matches git: --update-shallow accepts shallow-boundary refs" && {
    :  # expect_parity effect -- fetch --update-shallow origin
  }
)

# mode=effect — --negotiation-tip narrows the set of commits reported to the
# server during negotiation.
# hash=sha1-only "gix cannot open sha256 remotes, see gix/src/clone/fetch/mod.rs unimplemented!()"
title "gix fetch --negotiation-tip=<rev>"
only_for_hash sha1-only && (sandbox
  it "TODO: matches git: --negotiation-tip is accepted" && {
    :  # expect_parity effect -- fetch --negotiation-tip=HEAD origin
  }
)

# mode=effect — --negotiate-only (needs --negotiation-tip, no packfile).
# hash=sha1-only "gix cannot open sha256 remotes, see gix/src/clone/fetch/mod.rs unimplemented!()"
title "gix fetch --negotiate-only"
only_for_hash sha1-only && (sandbox
  it "TODO: matches git: --negotiate-only prints common ancestors" && {
    :  # expect_parity effect -- fetch --negotiate-only --negotiation-tip=HEAD origin
  }
)

# mode=effect — --dry-run shows what would be fetched without changing state.
# hash=sha1-only "gix cannot open sha256 remotes, see gix/src/clone/fetch/mod.rs unimplemented!()"
title "gix fetch --dry-run"
only_for_hash sha1-only && (sandbox
  it "TODO: matches git: --dry-run leaves FETCH_HEAD untouched" && {
    :  # expect_parity effect -- fetch --dry-run origin
  }
)

# mode=bytes — --porcelain is machine-readable; exact byte parity required.
# hash=sha1-only "gix cannot open sha256 remotes, see gix/src/clone/fetch/mod.rs unimplemented!()"
title "gix fetch --porcelain"
only_for_hash sha1-only && (sandbox
  it "TODO: matches git: --porcelain output is byte-exact" && {
    :  # expect_parity bytes -- fetch --porcelain origin
  }
)

# mode=effect — --filter=<spec> requests a partial-clone filter.
# hash=sha1-only "gix cannot open sha256 remotes, see gix/src/clone/fetch/mod.rs unimplemented!()"
title "gix fetch --filter=<spec>"
only_for_hash sha1-only && (sandbox
  it "TODO: matches git: --filter=blob:none narrows the fetched pack" && {
    :  # expect_parity effect -- fetch --filter=blob:none origin
  }
)

# mode=effect — --write-fetch-head is the default; --no-write-fetch-head
# suppresses FETCH_HEAD.
# hash=sha1-only "gix cannot open sha256 remotes, see gix/src/clone/fetch/mod.rs unimplemented!()"
title "gix fetch --no-write-fetch-head / --write-fetch-head"
only_for_hash sha1-only && (sandbox
  it "TODO: matches git: --no-write-fetch-head suppresses FETCH_HEAD" && {
    :  # expect_parity effect -- fetch --no-write-fetch-head origin
  }
  it "TODO: matches git: --write-fetch-head is a no-op default" && {
    :  # expect_parity effect -- fetch --write-fetch-head origin
  }
)

# mode=effect — --force / -f overrides the non-fast-forward guard when a
# refspec lacks a leading '+'.
# hash=sha1-only "gix cannot open sha256 remotes, see gix/src/clone/fetch/mod.rs unimplemented!()"
title "gix fetch --force / -f"
only_for_hash sha1-only && (sandbox
  it "TODO: matches git: --force allows non-ff ref update" && {
    :  # expect_parity effect -- fetch --force origin main:main
  }
)

# mode=effect — --keep retains the downloaded pack rather than exploding
# loose objects / discarding.
# hash=sha1-only "gix cannot open sha256 remotes, see gix/src/clone/fetch/mod.rs unimplemented!()"
title "gix fetch --keep / -k"
only_for_hash sha1-only && (sandbox
  it "TODO: matches git: --keep retains the pack" && {
    :  # expect_parity effect -- fetch --keep origin
  }
)

# mode=effect — --multiple lets multiple repositories/groups follow.
# hash=sha1-only "gix cannot open sha256 remotes, see gix/src/clone/fetch/mod.rs unimplemented!()"
title "gix fetch --multiple"
only_for_hash sha1-only && (sandbox
  it "TODO: matches git: --multiple accepts several remotes" && {
    :  # expect_parity effect -- fetch --multiple origin upstream
  }
)

# mode=effect — --auto-maintenance / --auto-gc (synonym) runs maintenance
# after fetch. Gix may no-op.
# hash=sha1-only "gix cannot open sha256 remotes, see gix/src/clone/fetch/mod.rs unimplemented!()"
title "gix fetch --auto-maintenance / --no-auto-maintenance / --auto-gc / --no-auto-gc"
only_for_hash sha1-only && (sandbox
  it "TODO: matches git: --auto-maintenance accepted" && {
    :  # expect_parity effect -- fetch --auto-maintenance origin
  }
  it "TODO: matches git: --no-auto-maintenance accepted" && {
    :  # expect_parity effect -- fetch --no-auto-maintenance origin
  }
  it "TODO: matches git: --auto-gc accepted (alias)" && {
    :  # expect_parity effect -- fetch --auto-gc origin
  }
  it "TODO: matches git: --no-auto-gc accepted (alias)" && {
    :  # expect_parity effect -- fetch --no-auto-gc origin
  }
)

# mode=effect — --write-commit-graph / --no-write-commit-graph writes a
# commit-graph file after fetch.
# hash=sha1-only "gix cannot open sha256 remotes, see gix/src/clone/fetch/mod.rs unimplemented!()"
title "gix fetch --write-commit-graph / --no-write-commit-graph"
only_for_hash sha1-only && (sandbox
  it "TODO: matches git: --write-commit-graph accepted" && {
    :  # expect_parity effect -- fetch --write-commit-graph origin
  }
  it "TODO: matches git: --no-write-commit-graph accepted" && {
    :  # expect_parity effect -- fetch --no-write-commit-graph origin
  }
)

# mode=effect — --prefetch rewrites the refspec into refs/prefetch/*.
# hash=sha1-only "gix cannot open sha256 remotes, see gix/src/clone/fetch/mod.rs unimplemented!()"
title "gix fetch --prefetch"
only_for_hash sha1-only && (sandbox
  it "TODO: matches git: --prefetch stores under refs/prefetch/" && {
    :  # expect_parity effect -- fetch --prefetch origin
  }
)

# mode=effect — --prune / -p removes stale remote-tracking refs.
# hash=sha1-only "gix cannot open sha256 remotes, see gix/src/clone/fetch/mod.rs unimplemented!()"
title "gix fetch --prune / -p"
only_for_hash sha1-only && (sandbox
  it "TODO: matches git: --prune drops stale remote-tracking refs" && {
    :  # expect_parity effect -- fetch --prune origin
  }
)

# mode=effect — --prune-tags / -P is shorthand for refs/tags/*:refs/tags/*
# + --prune.
# hash=sha1-only "gix cannot open sha256 remotes, see gix/src/clone/fetch/mod.rs unimplemented!()"
title "gix fetch --prune-tags / -P"
only_for_hash sha1-only && (sandbox
  it "TODO: matches git: --prune-tags drops stale tags" && {
    :  # expect_parity effect -- fetch --prune-tags origin
  }
)

# mode=effect — --tags / -t fetches refs/tags/* alongside other refs.
# hash=sha1-only "gix cannot open sha256 remotes, see gix/src/clone/fetch/mod.rs unimplemented!()"
title "gix fetch --tags / -t"
only_for_hash sha1-only && (sandbox
  it "TODO: matches git: --tags pulls every remote tag" && {
    :  # expect_parity effect -- fetch --tags origin
  }
)

# mode=effect — --no-tags / -n disables automatic tag following.
# hash=sha1-only "gix cannot open sha256 remotes, see gix/src/clone/fetch/mod.rs unimplemented!()"
title "gix fetch --no-tags / -n"
only_for_hash sha1-only && (sandbox
  it "TODO: matches git: --no-tags suppresses tag auto-follow" && {
    :  # expect_parity effect -- fetch --no-tags origin
  }
  it "TODO: matches git: -n is the short alias for --no-tags" && {
    :  # expect_parity effect -- fetch -n origin
  }
)

# mode=effect — --refetch ignores local objects and refetches a fresh copy.
# hash=sha1-only "gix cannot open sha256 remotes, see gix/src/clone/fetch/mod.rs unimplemented!()"
title "gix fetch --refetch"
only_for_hash sha1-only && (sandbox
  it "TODO: matches git: --refetch bypasses negotiation" && {
    :  # expect_parity effect -- fetch --refetch origin
  }
)

# mode=effect — --refmap overrides remote.<name>.fetch for cmdline-specified
# refs.
# hash=sha1-only "gix cannot open sha256 remotes, see gix/src/clone/fetch/mod.rs unimplemented!()"
title "gix fetch --refmap=<spec>"
only_for_hash sha1-only && (sandbox
  it "TODO: matches git: --refmap=<spec> overrides the default refmap" && {
    :  # expect_parity effect -- fetch --refmap=refs/heads/*:refs/heads/* origin main
  }
)

# mode=effect — --recurse-submodules controls recursive fetch of submodules.
# Accepts yes, on-demand, no. Bare form defaults to yes.
# hash=sha1-only "gix cannot open sha256 remotes, see gix/src/clone/fetch/mod.rs unimplemented!()"
title "gix fetch --recurse-submodules[=<mode>]"
only_for_hash sha1-only && (sandbox
  it "TODO: matches git: --recurse-submodules=yes" && {
    :  # expect_parity effect -- fetch --recurse-submodules=yes origin
  }
  it "TODO: matches git: --recurse-submodules=on-demand" && {
    :  # expect_parity effect -- fetch --recurse-submodules=on-demand origin
  }
  it "TODO: matches git: --recurse-submodules=no" && {
    :  # expect_parity effect -- fetch --recurse-submodules=no origin
  }
  it "TODO: matches git: bare --recurse-submodules defaults to yes" && {
    :  # expect_parity effect -- fetch --recurse-submodules origin
  }
)

# mode=effect — --no-recurse-submodules is an alias for --recurse-submodules=no.
# hash=sha1-only "gix cannot open sha256 remotes, see gix/src/clone/fetch/mod.rs unimplemented!()"
title "gix fetch --no-recurse-submodules"
only_for_hash sha1-only && (sandbox
  it "TODO: matches git: --no-recurse-submodules" && {
    :  # expect_parity effect -- fetch --no-recurse-submodules origin
  }
)

# mode=effect — --recurse-submodules rejects unknown values with
# `fatal: bad recurse-submodules argument`.
# hash=sha1-only "gix cannot open sha256 remotes, see gix/src/clone/fetch/mod.rs unimplemented!()"
title "gix fetch --recurse-submodules=<bogus>"
only_for_hash sha1-only && (sandbox
  it "TODO: matches git: --recurse-submodules=bogus dies 128" && {
    :  # expect_parity effect -- fetch --recurse-submodules=bogus origin
  }
)

# mode=effect — --jobs / -j parallelizes submodule and multi-remote fetches.
# hash=sha1-only "gix cannot open sha256 remotes, see gix/src/clone/fetch/mod.rs unimplemented!()"
title "gix fetch --jobs=<n> / -j"
only_for_hash sha1-only && (sandbox
  it "TODO: matches git: --jobs=2" && {
    :  # expect_parity effect -- fetch --jobs=2 origin
  }
  it "TODO: matches git: -j 2" && {
    :  # expect_parity effect -- fetch -j 2 origin
  }
)

# mode=effect — --set-upstream wires branch.<name>.{remote,merge}.
# hash=sha1-only "gix cannot open sha256 remotes, see gix/src/clone/fetch/mod.rs unimplemented!()"
title "gix fetch --set-upstream"
only_for_hash sha1-only && (sandbox
  it "TODO: matches git: --set-upstream sets tracking" && {
    :  # expect_parity effect -- fetch --set-upstream origin main
  }
)

# mode=effect — --update-head-ok is internal/pull-only but accepted.
# hash=sha1-only "gix cannot open sha256 remotes, see gix/src/clone/fetch/mod.rs unimplemented!()"
title "gix fetch --update-head-ok / -u"
only_for_hash sha1-only && (sandbox
  it "TODO: matches git: --update-head-ok is accepted" && {
    :  # expect_parity effect -- fetch --update-head-ok origin
  }
)

# mode=effect — --upload-pack overrides the remote's upload-pack program.
# hash=sha1-only "gix cannot open sha256 remotes, see gix/src/clone/fetch/mod.rs unimplemented!()"
title "gix fetch --upload-pack=<path>"
only_for_hash sha1-only && (sandbox
  it "TODO: matches git: --upload-pack=git-upload-pack is a no-op default" && {
    :  # expect_parity effect -- fetch --upload-pack=git-upload-pack origin
  }
)

# mode=effect — --quiet / -q silences the status output.
# hash=sha1-only "gix cannot open sha256 remotes, see gix/src/clone/fetch/mod.rs unimplemented!()"
title "gix fetch --quiet / -q"
only_for_hash sha1-only && (sandbox
  it "TODO: matches git: --quiet suppresses the ref-status table" && {
    :  # expect_parity effect -- fetch --quiet origin
  }
)

# mode=effect — --verbose / -v requests more output, including up-to-date
# refs.
# hash=sha1-only "gix cannot open sha256 remotes, see gix/src/clone/fetch/mod.rs unimplemented!()"
title "gix fetch --verbose / -v"
only_for_hash sha1-only && (sandbox
  it "TODO: matches git: --verbose includes up-to-date rows" && {
    :  # expect_parity effect -- fetch --verbose origin
  }
)

# mode=effect — --progress forces progress even if stderr is not a TTY.
# hash=sha1-only "gix cannot open sha256 remotes, see gix/src/clone/fetch/mod.rs unimplemented!()"
title "gix fetch --progress"
only_for_hash sha1-only && (sandbox
  it "TODO: matches git: --progress forces progress output" && {
    :  # expect_parity effect -- fetch --progress origin
  }
)

# mode=effect — --server-option / -o transmits protocol-v2 server options.
# hash=sha1-only "gix cannot open sha256 remotes, see gix/src/clone/fetch/mod.rs unimplemented!()"
title "gix fetch --server-option=<option> / -o"
only_for_hash sha1-only && (sandbox
  it "TODO: matches git: --server-option sent on v2" && {
    :  # expect_parity effect -- fetch --server-option=key=val origin
  }
)

# mode=effect — --show-forced-updates / --no-show-forced-updates controls
# the forced-update check.
# hash=sha1-only "gix cannot open sha256 remotes, see gix/src/clone/fetch/mod.rs unimplemented!()"
title "gix fetch --show-forced-updates / --no-show-forced-updates"
only_for_hash sha1-only && (sandbox
  it "TODO: matches git: --show-forced-updates is accepted" && {
    :  # expect_parity effect -- fetch --show-forced-updates origin
  }
  it "TODO: matches git: --no-show-forced-updates skips the check" && {
    :  # expect_parity effect -- fetch --no-show-forced-updates origin
  }
)

# mode=effect — -4 and -6 force IPv4 / IPv6 resolution at the transport layer.
# Over file:// both are no-ops — exit-code parity only.
# hash=sha1-only "gix cannot open sha256 remotes, see gix/src/clone/fetch/mod.rs unimplemented!()"
title "gix fetch -4 / --ipv4 and -6 / --ipv6"
only_for_hash sha1-only && (sandbox
  it "TODO: matches git: -4 accepted" && {
    :  # expect_parity effect -- fetch -4 origin
  }
  it "TODO: matches git: -6 accepted" && {
    :  # expect_parity effect -- fetch -6 origin
  }
  it "TODO: matches git: -4 -6 last-wins (IPv6)" && {
    :  # expect_parity effect -- fetch -4 -6 origin
  }
)

# mode=effect — --stdin lets refspecs come from stdin.
# hash=sha1-only "gix cannot open sha256 remotes, see gix/src/clone/fetch/mod.rs unimplemented!()"
title "gix fetch --stdin"
only_for_hash sha1-only && (sandbox
  it "TODO: matches git: --stdin reads refspecs from stdin" && {
    :  # expect_parity effect -- fetch --stdin origin
  }
)

# --- config-triggered error paths -----------------------------------------

# mode=effect — bad boolean config value for any fetch.* boolean. git dies
# with `fatal: bad boolean config value '<v>' for '<key-lower>'`.
# hash=sha1-only "gix cannot open sha256 remotes, see gix/src/clone/fetch/mod.rs unimplemented!()"
title "gix fetch with fetch.<bool>=<bogus>"
only_for_hash sha1-only && (sandbox
  it "TODO: matches git: -c fetch.prune=bogus → 128" && {
    :  # expect_parity effect -- -c fetch.prune=bogus fetch origin
  }
  it "TODO: matches git: -c fetch.pruneTags=bogus → 128" && {
    :  # expect_parity effect -- -c fetch.pruneTags=bogus fetch origin
  }
  it "TODO: matches git: -c fetch.writeCommitGraph=bogus → 128" && {
    :  # expect_parity effect -- -c fetch.writeCommitGraph=bogus fetch origin
  }
  it "TODO: matches git: -c fetch.showForcedUpdates=bogus → 128" && {
    :  # expect_parity effect -- -c fetch.showForcedUpdates=bogus fetch origin
  }
)

# mode=effect — bad recurse-submodules config value.
# hash=sha1-only "gix cannot open sha256 remotes, see gix/src/clone/fetch/mod.rs unimplemented!()"
title "gix fetch with fetch.recurseSubmodules=<bogus>"
only_for_hash sha1-only && (sandbox
  it "TODO: matches git: -c fetch.recurseSubmodules=bogus → 128" && {
    :  # expect_parity effect -- -c fetch.recurseSubmodules=bogus fetch origin
  }
)

# End-of-file sentinel: when every row is `only_for_hash sha1-only` and the
# active hash is sha256, the last statement returns 1 (skip), which would
# propagate out of `source` and trip `set -e` in tests/parity.sh. A trailing
# `:` normalizes the exit code so a fully-skipped file still returns 0.
:
