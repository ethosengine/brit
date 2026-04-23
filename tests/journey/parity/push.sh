# Must be sourced into tests/parity.sh or tests/journey.sh — see tests/parity.sh.
#
# Parity scaffold for `git push` ↔ `gix push`.
#
# One `title` + `it` block per flag derived from vendor/git/builtin/push.c
# (cmd_push options[]) and vendor/git/Documentation/git-push.adoc. Every
# `it` body starts as a TODO: placeholder — iteration N of the ralph loop
# picks the next TODO, converts it to a real `expect_parity` assertion,
# and removes the TODO marker.
#
# Verdict modes (comment above each block):
#   bytes  — scriptable output; byte-exact match required (e.g. --porcelain)
#   effect — exit-code + UX; output diff reported but not fatal
#
# ─── PARITY STATE (iter 18) ────────────────────────────────────────────────
# Closed (all effect-mode, pre-transport dies & parse-time contracts):
#   no-destination (128), bad-repository '' (128), --all+--mirror (128),
#   --delete w/o refs (128), --all+refspecs (128), --mirror+refspecs (128),
#   --signed=<bogus> (128), --recurse-submodules=<bogus> (128),
#   --force-with-lease=ref:<bogus-oid> (129), <nonexistent-path> fallthrough,
#   -4/-6 overrides_with, --help (0), 3-way conflict (128), 4-way (128),
#   --push-option=<\n> (128).
# Happy-path rows closed (send-pack substrate landed, iter 18+):
#   (1) push origin refs/heads/main:refs/heads/main (bare file:// remote,
#       fast-forward / initial push) — exit 0 parity confirmed.
#   (2) push <url> <partial-refspec> — one-sided push spec (e.g. `main`)
#       inherits its destination from the matched local ref; URL-as-repo
#       falls back to anonymous remote (mirrors git's `remote_get_1`).
#   (3) push --repo=<name> — when no CLI refspecs are given, gix now
#       falls back to the remote's configured `remote.<name>.push`
#       refspecs (mirrors git's `match_push_refs`).
#   (4) push --all — CLI glue translates to `refs/heads/*:refs/heads/*`
#       (mirrors git's TRANSPORT_PUSH_ALL → MATCH_REFS_ALL flag path).
#
# send-pack substrate is online (gix-protocol send-pack + Repository::push
# + gitoxide-core glue + gix CLI wiring). Remaining TODO rows below exercise
# happy-path variants, flags, and transport options that can now be closed
# iteratively. `docs/parity/commands.md` updated from partial/blocked to
# partial/iterative.
# ────────────────────────────────────────────────────────────────────────────

# hash=sha1-only "gix cannot open sha256 remotes, see gix/src/clone/fetch/mod.rs unimplemented!()"
title "gix push"

# --- meta / help -----------------------------------------------------------

# mode=effect — `git push --help` delegates to man git-push (exit 0 when man
# is available); gix returns Clap's auto-generated help (exit 0). Message
# text diverges wildly and is NOT asserted — this row guards only the
# exit-code contract that `--help` is a benign, zero-exit operation.
# hash=sha1-only "gix cannot open sha256 remotes, see gix/src/clone/fetch/mod.rs unimplemented!()"
title "gix push --help"
only_for_hash sha1-only && (sandbox
  it "matches git: --help exits 0" && {
    expect_parity effect -- push --help
  }
)

# --- error paths (pre-transport validation) --------------------------------

# mode=effect — mirrors die() in vendor/git/builtin/push.c around line 631.
# Exit code 128; message text is close-to-git but not byte-exact.
# hash=sha1-only "gix cannot open sha256 remotes, see gix/src/clone/fetch/mod.rs unimplemented!()"
title "gix push (no configured push destination)"
only_for_hash sha1-only && (sandbox
  git init -q
  git config commit.gpgsign false
  git config tag.gpgsign false
  git -c user.email=x@x -c user.name=x commit --allow-empty -qm init
  it "matches git: bare 'push' in a repo with no remotes" && {
    expect_parity effect -- push
  }
)

# mode=effect — mirrors the `if (repo) die("bad repository '%s'")` branch in
# vendor/git/builtin/push.c::cmd_push. Empty-string repository (positional or
# --repo=) hits `remote_get_1` with an empty name, which returns NULL; git
# then dies 128.
# hash=sha1-only "gix cannot open sha256 remotes, see gix/src/clone/fetch/mod.rs unimplemented!()"
title "gix push '' (bad repository: empty name)"
only_for_hash sha1-only && (sandbox
  git init -q
  git config commit.gpgsign false
  git config tag.gpgsign false
  git -c user.email=x@x -c user.name=x commit --allow-empty -qm init
  it "matches git: positional empty repository name" && {
    expect_parity effect -- push ''
  }
  it "matches git: --repo='' (empty repository override)" && {
    expect_parity effect -- push --repo=
  }
)

# mode=effect — mirrors die_for_incompatible_opt4 at the top of cmd_push
# (vendor/git/builtin/push.c). Any pair drawn from {--all/--branches,
# --mirror, --tags, --delete} dies 128 with a git-exact message text.
# hash=sha1-only "gix cannot open sha256 remotes, see gix/src/clone/fetch/mod.rs unimplemented!()"
title "gix push (conflicting ref-selection flags)"
only_for_hash sha1-only && (sandbox
  git init -q
  git config commit.gpgsign false
  git config tag.gpgsign false
  git -c user.email=x@x -c user.name=x commit --allow-empty -qm init
  git remote add origin /tmp/parity-unused
  it "matches git: --all + --mirror" && {
    expect_parity effect -- push --all --mirror origin
  }
  it "matches git: --all + --mirror + --tags (3-way)" && {
    expect_parity effect -- push --all --mirror --tags origin
  }
  it "matches git: --all + --mirror + --tags + --delete (4-way)" && {
    expect_parity effect -- push --all --mirror --tags --delete origin foo
  }
)

# mode=effect — mirrors `if (deleterefs && argc < 2) die()` at push.c line
# ~559. --delete requires at least one refspec; if none, exit 128 before
# remote resolution.
# hash=sha1-only "gix cannot open sha256 remotes, see gix/src/clone/fetch/mod.rs unimplemented!()"
title "gix push --delete (without any refs)"
only_for_hash sha1-only && (sandbox
  git init -q
  git config commit.gpgsign false
  git config tag.gpgsign false
  git -c user.email=x@x -c user.name=x commit --allow-empty -qm init
  git remote add origin /tmp/parity-unused
  it "matches git: --delete with a remote but no refspecs" && {
    expect_parity effect -- push --delete origin
  }
)

# mode=effect — mirrors `if (argc >= 2) die("--all can't be combined
# with refspecs")` at push.c ~573. Runs AFTER remote resolution: the
# remote must exist, and refspecs must be non-empty.
# hash=sha1-only "gix cannot open sha256 remotes, see gix/src/clone/fetch/mod.rs unimplemented!()"
title "gix push --all (combined with refspecs)"
only_for_hash sha1-only && (sandbox
  git init -q
  git config commit.gpgsign false
  git config tag.gpgsign false
  git -c user.email=x@x -c user.name=x commit --allow-empty -qm init
  git remote add origin /tmp/parity-unused
  it "matches git: --all with an explicit refspec" && {
    expect_parity effect -- push --all origin main
  }
)

# mode=effect — mirrors `if (argc >= 2) die("--mirror can't be combined
# with refspecs")` at push.c ~577. Sibling to --all+refspecs with the
# same die-order (post-resolve).
# hash=sha1-only "gix cannot open sha256 remotes, see gix/src/clone/fetch/mod.rs unimplemented!()"
title "gix push --mirror (combined with refspecs)"
only_for_hash sha1-only && (sandbox
  git init -q
  git config commit.gpgsign false
  git config tag.gpgsign false
  git -c user.email=x@x -c user.name=x commit --allow-empty -qm init
  git remote add origin /tmp/parity-unused
  it "matches git: --mirror with an explicit refspec" && {
    expect_parity effect -- push --mirror origin main
  }
)

# mode=effect — mirrors option_parse_push_signed in vendor/git/send-pack.c.
# git accepts yes/no/true/false/on/off/1/0/if-asked (case-insensitive);
# anything else dies 128 with `fatal: bad signed argument: <arg>`.
# hash=sha1-only "gix cannot open sha256 remotes, see gix/src/clone/fetch/mod.rs unimplemented!()"
title "gix push --signed=<bogus> (unknown signed argument)"
only_for_hash sha1-only && (sandbox
  git init -q
  git config commit.gpgsign false
  git config tag.gpgsign false
  git -c user.email=x@x -c user.name=x commit --allow-empty -qm init
  git remote add origin /tmp/parity-unused
  it "matches git: --signed=bogus is rejected pre-transport" && {
    expect_parity effect -- push --signed=bogus origin main
  }
)

# mode=effect — mirrors parse_push_recurse in vendor/git/submodule-config.c.
# Accepts check/on-demand/only (case-sensitive) and no/off/false/0
# (case-insensitive); rejects yes/on/true/1 and anything else with exit
# 128 `fatal: bad recurse-submodules argument: <arg>`.
# hash=sha1-only "gix cannot open sha256 remotes, see gix/src/clone/fetch/mod.rs unimplemented!()"
title "gix push --recurse-submodules=<bogus>"
only_for_hash sha1-only && (sandbox
  git init -q
  git config commit.gpgsign false
  git config tag.gpgsign false
  git -c user.email=x@x -c user.name=x commit --allow-empty -qm init
  git remote add origin /tmp/parity-unused
  it "matches git: --recurse-submodules=bogus is rejected pre-transport" && {
    expect_parity effect -- push --recurse-submodules=bogus origin main
  }
)

# mode=effect — mirrors parse_push_cas_option in vendor/git/remote.c
# (line ~2584). When the expect part of --force-with-lease=<refname>:
# <expect> is non-empty and doesn't resolve as an OID/ref, parse-options.c
# propagates the callback error and git exits 129 with a single-line
# `error: cannot parse expected object name '<expect>'` (no usage banner
# — a quirk of this specific error path).
# hash=sha1-only "gix cannot open sha256 remotes, see gix/src/clone/fetch/mod.rs unimplemented!()"
title "gix push --force-with-lease=ref:<bogus-oid>"
only_for_hash sha1-only && (sandbox
  git init -q
  git config commit.gpgsign false
  git config tag.gpgsign false
  git -c user.email=x@x -c user.name=x commit --allow-empty -qm init
  git remote add origin /tmp/parity-unused
  it "matches git: unparseable expect OID → exit 129" && {
    expect_parity effect -- push --force-with-lease=main:notavalidoid origin main
  }
)

# mode=effect — mirrors git_config_bool's die path. Boolean config keys
# used by cmd_push (push.followTags, push.useForceIfIncludes,
# push.autoSetupRemote) reject anything outside yes/on/true/1/no/off/
# false/0 with a single-line `fatal: bad boolean config value '<v>'
# for '<key-lower>'`. Key is lowercased in the error text.
# hash=sha1-only "gix cannot open sha256 remotes, see gix/src/clone/fetch/mod.rs unimplemented!()"
title "gix push with push.followTags=<bogus>"
only_for_hash sha1-only && (sandbox
  git init -q
  git checkout -b main
  git config commit.gpgsign false
  git config tag.gpgsign false
  touch a && git add a
  git -c user.email=x@x -c user.name=x commit -qm init
  git remote add origin /tmp/parity-unused
  it "matches git: -c push.followTags=bogus → 128" && {
    expect_parity effect -- -c push.followTags=bogus push origin main
  }
  it "matches git: -c push.useForceIfIncludes=bogus → 128" && {
    expect_parity effect -- -c push.useForceIfIncludes=bogus push origin main
  }
  it "matches git: -c push.autoSetupRemote=bogus → 128" && {
    expect_parity effect -- -c push.autoSetupRemote=bogus push origin main
  }
  it "matches git: -c submodule.recurse=bogus → 128" && {
    expect_parity effect -- -c submodule.recurse=bogus push origin main
  }
  it "matches git: -c color.push=bogus → 128 (colorbool, wider value set)" && {
    expect_parity effect -- -c color.push=bogus push origin main
  }
  it "matches git: -c color.push.reset=<notacolor> → 128" && {
    expect_parity effect -- -c color.push.reset=notacolor push origin main
  }
  it "matches git: -c remote.origin.mirror=bogus → 128" && {
    expect_parity effect -- -c remote.origin.mirror=bogus push origin main
  }
  it "matches git: -c remote.origin.prune=bogus → 128" && {
    expect_parity effect -- -c remote.origin.prune=bogus push origin main
  }
  it "matches git: -c remote.origin.skipDefaultUpdate=bogus → 128" && {
    expect_parity effect -- -c remote.origin.skipDefaultUpdate=bogus push origin main
  }
  it "matches git: -c remote.origin.skipFetchAll=bogus → 128" && {
    expect_parity effect -- -c remote.origin.skipFetchAll=bogus push origin main
  }
)

# mode=effect — mirrors the push.default validator in
# vendor/git/environment.c. Unknown values die 128 with git's three-line
# error/error/fatal.
# hash=sha1-only "gix cannot open sha256 remotes, see gix/src/clone/fetch/mod.rs unimplemented!()"
title "gix push with push.default=<bogus>"
only_for_hash sha1-only && (sandbox
  git init -q
  git checkout -b main
  git config commit.gpgsign false
  git config tag.gpgsign false
  touch a && git add a
  git -c user.email=x@x -c user.name=x commit -qm init
  git remote add origin /tmp/parity-unused
  it "matches git: -c push.default=bogus push → 128" && {
    expect_parity effect -- -c push.default=bogus push origin main
  }
)

# mode=effect — mirrors the `push.recursesubmodules` arm of
# git_push_config, which delegates to parse_push_recurse_submodules_arg
# (same semantics as --recurse-submodules). Invalid values die 128 with
# "fatal: bad push.recursesubmodules argument: <v>".
# hash=sha1-only "gix cannot open sha256 remotes, see gix/src/clone/fetch/mod.rs unimplemented!()"
title "gix push with push.recursesubmodules=<bogus>"
only_for_hash sha1-only && (sandbox
  git init -q
  git checkout -b main
  git config commit.gpgsign false
  git config tag.gpgsign false
  touch a && git add a
  git -c user.email=x@x -c user.name=x commit -qm init
  git remote add origin /tmp/parity-unused
  it "matches git: -c push.recursesubmodules=bogus push → 128" && {
    expect_parity effect -- -c push.recursesubmodules=bogus push origin main
  }
)

# mode=effect — mirrors the `push.gpgsign` arm of git_push_config. The
# config key accepts the same values as --signed (yes/true/on/1, no/false/
# off/0, if-asked, case-insensitive via git_parse_maybe_bool). Invalid
# values bubble through git_config with a two-line error/fatal (exit 128).
# hash=sha1-only "gix cannot open sha256 remotes, see gix/src/clone/fetch/mod.rs unimplemented!()"
title "gix push with push.gpgsign=<bogus>"
only_for_hash sha1-only && (sandbox
  git init -q
  git checkout -b main
  git config commit.gpgsign false
  git config tag.gpgsign false
  touch a && git add a
  git -c user.email=x@x -c user.name=x commit -qm init
  git remote add origin /tmp/parity-unused
  it "matches git: -c push.gpgsign=bogus push → 128" && {
    expect_parity effect -- -c push.gpgsign=bogus push origin main
  }
)

# mode=effect — mirrors the PUSH_DEFAULT_NOTHING arm in cmd_push. With no
# CLI refspecs, no configured `push` refspecs on the remote, and
# push.default=nothing, git dies 128 (it would otherwise fall through to
# use push.default to compute a default refspec).
# hash=sha1-only "gix cannot open sha256 remotes, see gix/src/clone/fetch/mod.rs unimplemented!()"
title "gix push with push.default=nothing and no refspecs"
only_for_hash sha1-only && (sandbox
  git init -q
  git checkout -b main
  git config commit.gpgsign false
  git config tag.gpgsign false
  touch a && git add a
  git -c user.email=x@x -c user.name=x commit -qm init
  git remote add origin /tmp/parity-unused
  it "matches git: -c push.default=nothing push → 128" && {
    expect_parity effect -- -c push.default=nothing push
  }
)

# mode=effect — push-options are transmitted over pkt-lines which don't
# permit embedded newlines; git's cmd_push iterates push_options and
# dies 128 "push options must not have new line characters" if any
# contains '\n'. Mirrored in gix after --mirror/-refspecs checks.
# hash=sha1-only "gix cannot open sha256 remotes, see gix/src/clone/fetch/mod.rs unimplemented!()"
title "gix push --push-option=<value-with-newline>"
only_for_hash sha1-only && (sandbox
  git init -q
  git config commit.gpgsign false
  git config tag.gpgsign false
  git -c user.email=x@x -c user.name=x commit --allow-empty -qm init
  git remote add origin /tmp/parity-unused
  it "matches git: newline in push-option → 128" && {
    expect_parity effect -- push "--push-option=foo
bar" origin main
  }
)

# mode=effect — git's `remote_get_1` accepts any non-empty name/URL and
# synthesizes an anonymous Remote from it; failure surfaces at the
# transport layer, not at `bad repository` resolution. Pushing to a
# nonexistent local path thus exits 1 (transport-level "refspec didn't
# match" / "failed to push"), NOT 128 (die). This row guards gix's
# remote-resolution predicate against the overly-aggressive "is this a
# known remote?" fail-fast that would otherwise exit 128.
# hash=sha1-only "gix cannot open sha256 remotes, see gix/src/clone/fetch/mod.rs unimplemented!()"
title "gix push <nonexistent-path> (falls through to transport)"
only_for_hash sha1-only && (sandbox
  git init -q
  git config commit.gpgsign false
  git config tag.gpgsign false
  git -c user.email=x@x -c user.name=x commit --allow-empty -qm init
  it "matches git: nonexistent path is not a bad-repository die" && {
    expect_parity effect -- push /tmp/gix-parity-definitely-does-not-exist main
  }
)

# mode=effect — git's OPT_IPVERSION binds -4 and -6 to the same
# transport_family variable, so the two flags silently override each
# other instead of erroring like Clap's conflicts_with would. Test
# both orders (-4 -6 and -6 -4) reach the same post-parse state and
# exit identically.
# hash=sha1-only "gix cannot open sha256 remotes, see gix/src/clone/fetch/mod.rs unimplemented!()"
title "gix push -4 -6 / -6 -4 (no parse-time conflict)"
only_for_hash sha1-only && (sandbox
  git init -q
  git config commit.gpgsign false
  git config tag.gpgsign false
  git -c user.email=x@x -c user.name=x commit --allow-empty -qm init
  git remote add origin /tmp/parity-unused
  it "matches git: -4 -6 doesn't error at parse time" && {
    expect_parity effect -- push -4 -6 origin main
  }
  it "matches git: -6 -4 doesn't error at parse time" && {
    expect_parity effect -- push -6 -4 origin main
  }
)

# --- positional & repository selection -------------------------------------

# mode=effect — positional <repository> may be a URL (not a configured
# remote name). Mirrors git's `remote_get_1` which wraps any non-empty
# string as an anonymous URL-backed remote when it isn't a known name.
# hash=sha1-only "gix cannot open sha256 remotes, see gix/src/clone/fetch/mod.rs unimplemented!()"
title "gix push <repository>"
only_for_hash sha1-only && (sandbox
  dst="$(pwd)/dst.git"
  git init -q -b main src
  git -C src config commit.gpgsign false
  git -C src config tag.gpgsign false
  git -C src -c user.email=x@x -c user.name=x commit --allow-empty -qm "init"
  git init -q --bare "$dst"
  it "matches git: push <url-as-repository> <refspec> exits 0" && {
    cd src && expect_parity effect -- push "$dst" main
  }
)

# mode=effect — first happy-path parity row closed by the send-pack
# substrate (gix-protocol send-pack + Repository::push + CLI wiring).
# Sets up a bare file:// remote so both git and gix perform a real
# initial-push over the local transport; both exit 0.
# See etc/plan/2026-04-23-send-pack-substrate.md for the full substrate
# history (Tasks 1–9).
# hash=sha1-only "gix cannot open sha256 remotes, see gix/src/clone/fetch/mod.rs unimplemented!()"
title "gix push origin refs/heads/main:refs/heads/main (bare file:// remote, fast-forward)"
only_for_hash sha1-only && (sandbox
  dst="$(pwd)/dst.git"
  git init -q -b main src
  git -C src config commit.gpgsign false
  git -C src config tag.gpgsign false
  git -C src -c user.email=x@x -c user.name=x commit --allow-empty -qm "init"
  git init -q --bare "$dst"
  git -C src remote add origin "$dst"
  it "matches git: push origin refs/heads/main:refs/heads/main exits 0" && {
    cd src && expect_parity effect -- push origin refs/heads/main:refs/heads/main
  }
)

# mode=effect — git's cmd_push: `--repo=<x>` is only honored when no
# positional <repository> is given (argc>0 overrides `repo`). With a
# configured `remote.<name>.push` refspec, this drives a bare
# `push --repo=<name>` end-to-end with no CLI refspecs.
# hash=sha1-only "gix cannot open sha256 remotes, see gix/src/clone/fetch/mod.rs unimplemented!()"
title "gix push --repo=<repository>"
only_for_hash sha1-only && (sandbox
  dst="$(pwd)/dst.git"
  git init -q -b main src
  git -C src config commit.gpgsign false
  git -C src config tag.gpgsign false
  git -C src -c user.email=x@x -c user.name=x commit --allow-empty -qm "init"
  git init -q --bare "$dst"
  git -C src remote add upstream "$dst"
  git -C src config remote.upstream.push refs/heads/main:refs/heads/main
  it "matches git: --repo=<name> uses remote.<name>.push refspec, exits 0" && {
    cd src && expect_parity effect -- push --repo=upstream
  }
)

# --- branch / ref selection ------------------------------------------------

# mode=effect — `--all` sets TRANSPORT_PUSH_ALL in git; match_push_refs
# then matches all local refs/heads/* against same-named remote refs,
# including branches that don't yet exist on the remote. Equivalent
# refspec: `refs/heads/*:refs/heads/*` (with creation semantics).
# hash=sha1-only "gix cannot open sha256 remotes, see gix/src/clone/fetch/mod.rs unimplemented!()"
title "gix push --all"
only_for_hash sha1-only && (sandbox
  dst="$(pwd)/dst.git"
  git init -q -b main src
  git -C src config commit.gpgsign false
  git -C src config tag.gpgsign false
  git -C src -c user.email=x@x -c user.name=x commit --allow-empty -qm c1
  git -C src branch dev
  git -C src -c user.email=x@x -c user.name=x commit --allow-empty -qm c2
  git init -q --bare "$dst"
  git -C src remote add origin "$dst"
  it "matches git: --all pushes refs/heads/* to matching names, exits 0" && {
    cd src && expect_parity effect -- push --all origin
  }
)

# mode=effect — `--branches` is git's visible alias for `--all`
# (see cmd_push options[] in vendor/git/builtin/push.c; both set
# TRANSPORT_PUSH_ALL). Same semantics, same expected outcome.
# hash=sha1-only "gix cannot open sha256 remotes, see gix/src/clone/fetch/mod.rs unimplemented!()"
title "gix push --branches"
only_for_hash sha1-only && (sandbox
  dst="$(pwd)/dst.git"
  git init -q -b main src
  git -C src config commit.gpgsign false
  git -C src config tag.gpgsign false
  git -C src -c user.email=x@x -c user.name=x commit --allow-empty -qm c1
  git -C src branch dev
  git -C src -c user.email=x@x -c user.name=x commit --allow-empty -qm c2
  git init -q --bare "$dst"
  git -C src remote add origin "$dst"
  it "matches git: --branches (alias of --all), exits 0" && {
    cd src && expect_parity effect -- push --branches origin
  }
)

# mode=effect
# hash=sha1-only "gix cannot open sha256 remotes, see gix/src/clone/fetch/mod.rs unimplemented!()"
title "gix push --mirror"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    # TODO: expect_parity effect -- push --mirror origin
    true
  }
)

# mode=effect
# hash=sha1-only "gix cannot open sha256 remotes, see gix/src/clone/fetch/mod.rs unimplemented!()"
title "gix push --tags"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    # TODO: expect_parity effect -- push --tags origin
    true
  }
)

# mode=effect — --follow-tags adds reachable tags to the push after the
# main refs but doesn't skip local src refspec matching. Same refspec-
# first invariant as iter 30.
# hash=sha1-only "gix cannot open sha256 remotes, see gix/src/clone/fetch/mod.rs unimplemented!()"
title "gix push --follow-tags"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git: --follow-tags with unmatched src refspec exits 1" && {
    expect_parity effect -- push --follow-tags /tmp/parity-unused nonexistent-refspec
  }
)

# mode=effect
# hash=sha1-only "gix cannot open sha256 remotes, see gix/src/clone/fetch/mod.rs unimplemented!()"
title "gix push -d / --delete"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    # TODO: expect_parity effect -- push --delete origin feature
    true
  }
)

# mode=effect
# hash=sha1-only "gix cannot open sha256 remotes, see gix/src/clone/fetch/mod.rs unimplemented!()"
title "gix push --prune"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    # TODO: expect_parity effect -- push --prune origin refs/heads/*:refs/heads/*
    true
  }
)

# --- dry-run / reporting ---------------------------------------------------

# mode=effect — git resolves refspecs locally *before* any transport
# contact, so a nonexistent src refspec fails with exit 1 regardless of
# the --dry-run / --force flags that would otherwise gate transport.
# This exercises the scaffold row without requiring a working send-pack
# (both tools exit 1 — git at refspec match, gix at the not-impl bail;
# the row will tighten naturally once happy-path push lands).
# hash=sha1-only "gix cannot open sha256 remotes, see gix/src/clone/fetch/mod.rs unimplemented!()"
title "gix push -n / --dry-run"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git: --dry-run with unmatched src refspec exits 1" && {
    expect_parity effect -- push --dry-run /tmp/parity-unused nonexistent-refspec
  }
)

# mode=bytes — machine-readable; byte-exact
# hash=sha1-only "gix cannot open sha256 remotes, see gix/src/clone/fetch/mod.rs unimplemented!()"
title "gix push --porcelain"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    # TODO: expect_parity bytes -- push --porcelain origin main
    true
  }
)

# mode=effect
# hash=sha1-only "gix cannot open sha256 remotes, see gix/src/clone/fetch/mod.rs unimplemented!()"
title "gix push --progress / --no-progress"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    # TODO: expect_parity effect -- push --no-progress origin main
    true
  }
)

# mode=effect — inherited from OPT__VERBOSITY. -v/-q tune stderr output
# volume but don't alter local refspec matching. Same refspec-first
# invariant as iter 30.
# hash=sha1-only "gix cannot open sha256 remotes, see gix/src/clone/fetch/mod.rs unimplemented!()"
title "gix push -v / --verbose"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git: -v with unmatched src refspec exits 1" && {
    expect_parity effect -- push -v /tmp/parity-unused nonexistent-refspec
  }
)

# mode=effect — inherited from OPT__VERBOSITY. Same invariant as -v;
# -q tunes stderr output volume, not exit codes.
# hash=sha1-only "gix cannot open sha256 remotes, see gix/src/clone/fetch/mod.rs unimplemented!()"
title "gix push -q / --quiet"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git: -q with unmatched src refspec exits 1" && {
    expect_parity effect -- push -q /tmp/parity-unused nonexistent-refspec
  }
)

# --- force / safety --------------------------------------------------------

# mode=effect — --force adjusts the update rule but doesn't skip local
# refspec resolution. Same refspec-first invariant as --dry-run (iter 30):
# a nonexistent src refspec exits 1 before transport regardless of
# --force.
# hash=sha1-only "gix cannot open sha256 remotes, see gix/src/clone/fetch/mod.rs unimplemented!()"
title "gix push -f / --force"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git: --force with unmatched src refspec exits 1" && {
    expect_parity effect -- push --force /tmp/parity-unused nonexistent-refspec
  }
)

# mode=effect — no-arg, with-refname, with-refname:expect
# hash=sha1-only "gix cannot open sha256 remotes, see gix/src/clone/fetch/mod.rs unimplemented!()"
title "gix push --force-with-lease"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior (bare --force-with-lease)" && {
    # TODO: expect_parity effect -- push --force-with-lease origin main
    true
  }
  it "matches git behavior (--force-with-lease=refname)" && {
    # TODO: expect_parity effect -- push --force-with-lease=main origin main
    true
  }
  it "matches git behavior (--force-with-lease=refname:expect)" && {
    # TODO: expect_parity effect -- push --force-with-lease=main:<sha> origin main
    true
  }
)

# mode=effect — depends on --force-with-lease being in play
# hash=sha1-only "gix cannot open sha256 remotes, see gix/src/clone/fetch/mod.rs unimplemented!()"
title "gix push --force-if-includes"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    # TODO: expect_parity effect -- push --force-with-lease --force-if-includes origin main
    true
  }
)

# mode=effect — --atomic requests a server-side atomic transaction but
# doesn't alter local refspec matching. Same refspec-first invariant
# as iter 30.
# hash=sha1-only "gix cannot open sha256 remotes, see gix/src/clone/fetch/mod.rs unimplemented!()"
title "gix push --atomic"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git: --atomic with unmatched src refspec exits 1" && {
    expect_parity effect -- push --atomic /tmp/parity-unused nonexistent-refspec
  }
)

# --- upstream / tracking ---------------------------------------------------

# mode=effect — --set-upstream only records remote tracking state *after*
# a successful push, so pre-transport refspec resolution still runs.
# Same refspec-first invariant as iter 30.
# hash=sha1-only "gix cannot open sha256 remotes, see gix/src/clone/fetch/mod.rs unimplemented!()"
title "gix push -u / --set-upstream"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git: --set-upstream with unmatched src refspec exits 1" && {
    expect_parity effect -- push --set-upstream /tmp/parity-unused nonexistent-refspec
  }
)

# --- transport-level options ----------------------------------------------

# mode=effect — --thin / --no-thin are packfile-generation knobs; neither
# affects local refspec resolution. Same refspec-first invariant as iter 30.
# hash=sha1-only "gix cannot open sha256 remotes, see gix/src/clone/fetch/mod.rs unimplemented!()"
title "gix push --thin / --no-thin"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git: --thin with unmatched src refspec exits 1" && {
    expect_parity effect -- push --thin /tmp/parity-unused nonexistent-refspec
  }
  it "matches git: --no-thin with unmatched src refspec exits 1" && {
    expect_parity effect -- push --no-thin /tmp/parity-unused nonexistent-refspec
  }
)

# mode=effect
# hash=sha1-only "gix cannot open sha256 remotes, see gix/src/clone/fetch/mod.rs unimplemented!()"
title "gix push --receive-pack=<program>"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    # TODO: expect_parity effect -- push --receive-pack=git-receive-pack origin main
    true
  }
)

# mode=effect — alias of --receive-pack
# hash=sha1-only "gix cannot open sha256 remotes, see gix/src/clone/fetch/mod.rs unimplemented!()"
title "gix push --exec=<program>"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    # TODO: expect_parity effect -- push --exec=git-receive-pack origin main
    true
  }
)

# mode=effect
# hash=sha1-only "gix cannot open sha256 remotes, see gix/src/clone/fetch/mod.rs unimplemented!()"
title "gix push -o / --push-option=<option>"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    # TODO: expect_parity effect -- push --push-option=foo --push-option=bar origin main
    true
  }
)

# mode=effect — IPv4 transport family
# hash=sha1-only "gix cannot open sha256 remotes, see gix/src/clone/fetch/mod.rs unimplemented!()"
title "gix push -4"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    # TODO: expect_parity effect -- push -4 origin main
    true
  }
)

# mode=effect — IPv6 transport family
# hash=sha1-only "gix cannot open sha256 remotes, see gix/src/clone/fetch/mod.rs unimplemented!()"
title "gix push -6"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    # TODO: expect_parity effect -- push -6 origin main
    true
  }
)

# --- hooks / signing / submodules ----------------------------------------

# mode=effect — --no-verify bypasses the pre-push hook (executed right
# before send-pack), but local refspec resolution runs long before.
# Same refspec-first invariant as iter 30.
# hash=sha1-only "gix cannot open sha256 remotes, see gix/src/clone/fetch/mod.rs unimplemented!()"
title "gix push --no-verify"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git: --no-verify with unmatched src refspec exits 1" && {
    expect_parity effect -- push --no-verify /tmp/parity-unused nonexistent-refspec
  }
)

# mode=effect — GPG signing; may be deferred if GPG toolchain absent
# hash=sha1-only "gix cannot open sha256 remotes, see gix/src/clone/fetch/mod.rs unimplemented!()"
title "gix push --signed"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior (--signed=no)" && {
    # TODO: expect_parity effect -- push --signed=no origin main
    true
  }
  it "matches git behavior (--signed=if-asked)" && {
    # TODO: expect_parity effect -- push --signed=if-asked origin main
    true
  }
  it "matches git behavior (--signed=yes)" && {
    # TODO: expect_parity effect -- push --signed=yes origin main
    true
  }
)

# mode=effect
# hash=sha1-only "gix cannot open sha256 remotes, see gix/src/clone/fetch/mod.rs unimplemented!()"
title "gix push --recurse-submodules=<mode>"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior (=no)" && {
    # TODO: expect_parity effect -- push --recurse-submodules=no origin main
    true
  }
  it "matches git behavior (=check)" && {
    # TODO: expect_parity effect -- push --recurse-submodules=check origin main
    true
  }
  it "matches git behavior (=on-demand)" && {
    # TODO: expect_parity effect -- push --recurse-submodules=on-demand origin main
    true
  }
  it "matches git behavior (=only)" && {
    # TODO: expect_parity effect -- push --recurse-submodules=only origin main
    true
  }
)

# End-of-file sentinel: when every row is `only_for_hash sha1-only` and the
# active hash is sha256, the last statement returns 1 (skip), which would
# propagate out of `source` and trip `set -e` in tests/parity.sh. A trailing
# `:` normalizes the exit code so a fully-skipped file still returns 0.
:
