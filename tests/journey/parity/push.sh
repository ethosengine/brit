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

title "gix push"

# --- error paths (pre-transport validation) --------------------------------

# mode=effect — mirrors die() in vendor/git/builtin/push.c around line 631.
# Exit code 128; message text is close-to-git but not byte-exact.
title "gix push (no configured push destination)"
(sandbox
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
title "gix push '' (bad repository: empty name)"
(sandbox
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
title "gix push (conflicting ref-selection flags)"
(sandbox
  git init -q
  git config commit.gpgsign false
  git config tag.gpgsign false
  git -c user.email=x@x -c user.name=x commit --allow-empty -qm init
  git remote add origin /tmp/parity-unused
  it "matches git: --all + --mirror" && {
    expect_parity effect -- push --all --mirror origin
  }
)

# mode=effect — mirrors `if (deleterefs && argc < 2) die()` at push.c line
# ~559. --delete requires at least one refspec; if none, exit 128 before
# remote resolution.
title "gix push --delete (without any refs)"
(sandbox
  git init -q
  git config commit.gpgsign false
  git config tag.gpgsign false
  git -c user.email=x@x -c user.name=x commit --allow-empty -qm init
  git remote add origin /tmp/parity-unused
  it "matches git: --delete with a remote but no refspecs" && {
    expect_parity effect -- push --delete origin
  }
)

# --- positional & repository selection -------------------------------------

# mode=effect
title "gix push <repository>"
(small-repo-in-sandbox
  it "matches git behavior" && {
    # TODO: bare-repo-with-remotes upstream.git; expect_parity effect -- push upstream.git main
    true
  }
)

# mode=effect
title "gix push <repository> <refspec>..."
(small-repo-in-sandbox
  it "matches git behavior" && {
    # TODO: expect_parity effect -- push upstream.git refs/heads/main:refs/heads/main
    true
  }
)

# mode=effect
title "gix push --repo=<repository>"
(small-repo-in-sandbox
  it "matches git behavior" && {
    # TODO: expect_parity effect -- push --repo=upstream.git
    true
  }
)

# --- branch / ref selection ------------------------------------------------

# mode=effect
title "gix push --all"
(small-repo-in-sandbox
  it "matches git behavior" && {
    # TODO: expect_parity effect -- push --all origin
    true
  }
)

# mode=effect — alias of --all
title "gix push --branches"
(small-repo-in-sandbox
  it "matches git behavior" && {
    # TODO: expect_parity effect -- push --branches origin
    true
  }
)

# mode=effect
title "gix push --mirror"
(small-repo-in-sandbox
  it "matches git behavior" && {
    # TODO: expect_parity effect -- push --mirror origin
    true
  }
)

# mode=effect
title "gix push --tags"
(small-repo-in-sandbox
  it "matches git behavior" && {
    # TODO: expect_parity effect -- push --tags origin
    true
  }
)

# mode=effect
title "gix push --follow-tags"
(small-repo-in-sandbox
  it "matches git behavior" && {
    # TODO: expect_parity effect -- push --follow-tags origin main
    true
  }
)

# mode=effect
title "gix push -d / --delete"
(small-repo-in-sandbox
  it "matches git behavior" && {
    # TODO: expect_parity effect -- push --delete origin feature
    true
  }
)

# mode=effect
title "gix push --prune"
(small-repo-in-sandbox
  it "matches git behavior" && {
    # TODO: expect_parity effect -- push --prune origin refs/heads/*:refs/heads/*
    true
  }
)

# --- dry-run / reporting ---------------------------------------------------

# mode=effect
title "gix push -n / --dry-run"
(small-repo-in-sandbox
  it "matches git behavior" && {
    # TODO: expect_parity effect -- push --dry-run origin main
    true
  }
)

# mode=bytes — machine-readable; byte-exact
title "gix push --porcelain"
(small-repo-in-sandbox
  it "matches git behavior" && {
    # TODO: expect_parity bytes -- push --porcelain origin main
    true
  }
)

# mode=effect
title "gix push --progress / --no-progress"
(small-repo-in-sandbox
  it "matches git behavior" && {
    # TODO: expect_parity effect -- push --no-progress origin main
    true
  }
)

# mode=effect — inherited from OPT__VERBOSITY
title "gix push -v / --verbose"
(small-repo-in-sandbox
  it "matches git behavior" && {
    # TODO: expect_parity effect -- push -v origin main
    true
  }
)

# mode=effect — inherited from OPT__VERBOSITY
title "gix push -q / --quiet"
(small-repo-in-sandbox
  it "matches git behavior" && {
    # TODO: expect_parity effect -- push -q origin main
    true
  }
)

# --- force / safety --------------------------------------------------------

# mode=effect
title "gix push -f / --force"
(small-repo-in-sandbox
  it "matches git behavior" && {
    # TODO: expect_parity effect -- push --force origin main
    true
  }
)

# mode=effect — no-arg, with-refname, with-refname:expect
title "gix push --force-with-lease"
(small-repo-in-sandbox
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
title "gix push --force-if-includes"
(small-repo-in-sandbox
  it "matches git behavior" && {
    # TODO: expect_parity effect -- push --force-with-lease --force-if-includes origin main
    true
  }
)

# mode=effect
title "gix push --atomic"
(small-repo-in-sandbox
  it "matches git behavior" && {
    # TODO: expect_parity effect -- push --atomic origin main
    true
  }
)

# --- upstream / tracking ---------------------------------------------------

# mode=effect
title "gix push -u / --set-upstream"
(small-repo-in-sandbox
  it "matches git behavior" && {
    # TODO: expect_parity effect -- push --set-upstream origin main
    true
  }
)

# --- transport-level options ----------------------------------------------

# mode=effect
title "gix push --thin / --no-thin"
(small-repo-in-sandbox
  it "matches git behavior (--no-thin)" && {
    # TODO: expect_parity effect -- push --no-thin origin main
    true
  }
)

# mode=effect
title "gix push --receive-pack=<program>"
(small-repo-in-sandbox
  it "matches git behavior" && {
    # TODO: expect_parity effect -- push --receive-pack=git-receive-pack origin main
    true
  }
)

# mode=effect — alias of --receive-pack
title "gix push --exec=<program>"
(small-repo-in-sandbox
  it "matches git behavior" && {
    # TODO: expect_parity effect -- push --exec=git-receive-pack origin main
    true
  }
)

# mode=effect
title "gix push -o / --push-option=<option>"
(small-repo-in-sandbox
  it "matches git behavior" && {
    # TODO: expect_parity effect -- push --push-option=foo --push-option=bar origin main
    true
  }
)

# mode=effect — IPv4 transport family
title "gix push -4"
(small-repo-in-sandbox
  it "matches git behavior" && {
    # TODO: expect_parity effect -- push -4 origin main
    true
  }
)

# mode=effect — IPv6 transport family
title "gix push -6"
(small-repo-in-sandbox
  it "matches git behavior" && {
    # TODO: expect_parity effect -- push -6 origin main
    true
  }
)

# --- hooks / signing / submodules ----------------------------------------

# mode=effect
title "gix push --no-verify"
(small-repo-in-sandbox
  it "matches git behavior" && {
    # TODO: expect_parity effect -- push --no-verify origin main
    true
  }
)

# mode=effect — GPG signing; may be deferred if GPG toolchain absent
title "gix push --signed"
(small-repo-in-sandbox
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
title "gix push --recurse-submodules=<mode>"
(small-repo-in-sandbox
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
