# Must be sourced into tests/parity.sh or tests/journey.sh — see tests/parity.sh.
#
# Parity scaffold for `git clone` ↔ `gix clone`.
#
# One `title` + `it` block per flag derived from vendor/git/builtin/clone.c
# (cmd_clone::builtin_clone_options[]) and
# vendor/git/Documentation/git-clone.adoc. Every `it` body starts as a
# TODO: placeholder — iteration N of the parity loop picks the next
# TODO, converts it to a real `expect_parity` assertion, and removes the
# TODO marker.
#
# Verdict modes (comment above each block):
#   bytes  — scriptable output; byte-exact match required
#   effect — exit-code + UX; output diff reported but not fatal
#
# Every row is `# hash=sha1-only` — `gix clone` cannot open sha256 remotes
# yet (gix/src/clone/fetch/mod.rs:278 still `unimplemented!()`s hash-change
# reconfiguration, shared with fetch). Once that lands, rows become `dual`.

# hash=sha1-only "gix cannot open sha256 remotes, see gix/src/clone/fetch/mod.rs unimplemented!()"
title "gix clone"

# --- meta / help ----------------------------------------------------------

# mode=effect — `git clone --help` delegates to man git-clone (exit 0 when
# man is available); gix returns Clap's auto-generated help (exit 0).
# Message text diverges wildly and is NOT asserted.
# hash=sha1-only "gix cannot open sha256 remotes, see gix/src/clone/fetch/mod.rs unimplemented!()"
title "gix clone --help"
only_for_hash sha1-only && (sandbox
  it "matches git: --help exits 0" && {
    expect_parity effect -- clone --help
  }
)

# --- positional / usage error paths --------------------------------------

# mode=effect — mirrors `if (argc == 0) usage_msg_opt("You must specify a
# repository to clone.")`. Exit 129.
# hash=sha1-only "gix cannot open sha256 remotes, see gix/src/clone/fetch/mod.rs unimplemented!()"
title "gix clone (no repository)"
only_for_hash sha1-only && (sandbox
  it "matches git: bare 'clone' dies 129 (usage)" && {
    expect_parity effect -- clone
  }
)

# mode=effect — mirrors `if (argc > 2) usage_msg_opt("Too many arguments.")`.
# Exit 129. Usage wording differs; only exit code is asserted.
# hash=sha1-only "gix cannot open sha256 remotes, see gix/src/clone/fetch/mod.rs unimplemented!()"
title "gix clone <repo> <dir> <extra> (too many args)"
only_for_hash sha1-only && (sandbox
  it "matches git: three positionals dies 129 (usage)" && {
    expect_parity effect -- clone /nonexistent.git foo bar
  }
)

# mode=effect — mirrors `die("repository '%s' does not exist")` after
# get_repo_path returns NULL and the name has no ':' (so it's not a URL).
# Exit 128.
# hash=sha1-only "gix cannot open sha256 remotes, see gix/src/clone/fetch/mod.rs unimplemented!()"
title "gix clone <nonexistent-local-path>"
only_for_hash sha1-only && (sandbox
  it "matches git: colon-less nonexistent path dies 128" && {
    expect_parity effect -- clone /nonexistent-path-for-parity
  }
)

# mode=effect — mirrors `die("destination path '%s' already exists and is
# not an empty directory.")` when dir arg points at a non-empty dir.
# Exit 128.
# hash=sha1-only "gix cannot open sha256 remotes, see gix/src/clone/fetch/mod.rs unimplemented!()"
title "gix clone <repo> <existing-non-empty-dir>"
only_for_hash sha1-only && (sandbox
  git init -q --bare upstream.git
  mkdir target
  touch target/foo
  it "matches git: destination exists + non-empty dies 128" && {
    expect_parity effect -- clone upstream.git target
  }
)

# --- happy paths ---------------------------------------------------------

# mode=effect — vanilla clone from a local bare remote into an auto-derived
# directory (humanish name). Golden path — the fetch+checkout round-trip.
# hash=sha1-only "gix cannot open sha256 remotes, see gix/src/clone/fetch/mod.rs unimplemented!()"
title "gix clone <bare-repo> (auto directory)"
only_for_hash sha1-only && (sandbox
  # Auto-directory derivation creates `upstream/` from `upstream.git`.
  # expect_parity runs git then gix in the same cwd, so the second call
  # would collide with the first's target dir. Split into two
  # single-binary it-blocks, each in its own subdir with a symlink back
  # to the shared bare upstream — same effect-mode contract
  # (exit-code parity) without the stateful-fixture collision.
  git-init-hash-aware -q --bare upstream.git
  mkdir g-side && (cd g-side && ln -s ../upstream.git .)
  mkdir x-side && (cd x-side && ln -s ../upstream.git .)
  it "matches git: vanilla clone into auto humanish dir exits 0" && {
    (cd g-side && expect_run 0 git clone upstream.git)
  }
  it "matches gix: vanilla clone into auto humanish dir exits 0" && {
    (cd x-side && expect_run 0 "$exe_plumbing" clone upstream.git)
  }
)

# mode=effect — explicit destination directory argument.
# hash=sha1-only "gix cannot open sha256 remotes, see gix/src/clone/fetch/mod.rs unimplemented!()"
title "gix clone <repo> <dir>"
only_for_hash sha1-only && (sandbox
  # Explicit destination positional — same stateful-fixture split as the
  # auto-directory row, but here the target is the explicit `my-clone`
  # name passed on the CLI (not derived from the URL).
  git-init-hash-aware -q --bare upstream.git
  mkdir g-side && (cd g-side && ln -s ../upstream.git .)
  mkdir x-side && (cd x-side && ln -s ../upstream.git .)
  it "matches git: explicit <dir> positional exits 0" && {
    (cd g-side && expect_run 0 git clone upstream.git my-clone)
  }
  it "matches gix: explicit <dir> positional exits 0" && {
    (cd x-side && expect_run 0 "$exe_plumbing" clone upstream.git my-clone)
  }
)

# --- verbosity / progress -------------------------------------------------

# mode=effect — OPT__VERBOSITY maps -v → option_verbosity++.
# hash=sha1-only "gix cannot open sha256 remotes, see gix/src/clone/fetch/mod.rs unimplemented!()"
title "gix clone -v / --verbose"
only_for_hash sha1-only && (sandbox
  git-init-hash-aware -q --bare upstream.git
  mkdir g-short g-long x-short x-long
  for d in g-short g-long x-short x-long; do
    (cd "$d" && ln -s ../upstream.git .)
  done
  it "matches git: -v exits 0" && {
    (cd g-short && expect_run 0 git clone -v upstream.git)
  }
  it "matches gix: -v exits 0" && {
    (cd x-short && expect_run 0 "$exe_plumbing" clone -v upstream.git)
  }
  it "matches git: --verbose exits 0" && {
    (cd g-long && expect_run 0 git clone --verbose upstream.git)
  }
  it "matches gix: --verbose exits 0" && {
    (cd x-long && expect_run 0 "$exe_plumbing" clone --verbose upstream.git)
  }
)

# mode=effect — OPT__VERBOSITY maps -q → option_verbosity--; clone prints
# no "Cloning into..." banner.
# hash=sha1-only "gix cannot open sha256 remotes, see gix/src/clone/fetch/mod.rs unimplemented!()"
title "gix clone -q / --quiet"
only_for_hash sha1-only && (sandbox
  it "matches git: -q exits 0" && {
    # TODO: expect_parity effect -- clone -q upstream.git
    true
  }
  it "matches git: --quiet exits 0" && {
    # TODO: expect_parity effect -- clone --quiet upstream.git
    true
  }
)

# mode=effect — OPT_BOOL `progress` forces progress even without a TTY.
# Effect mode — progress lines diverge; only exit code is asserted.
# hash=sha1-only "gix cannot open sha256 remotes, see gix/src/clone/fetch/mod.rs unimplemented!()"
title "gix clone --progress"
only_for_hash sha1-only && (sandbox
  it "matches git behavior" && {
    # TODO: expect_parity effect -- clone --progress upstream.git
    true
  }
)

# --- structural flags (bare / mirror / sparse / no-checkout) --------------

# mode=effect — `--bare` → option_bare=1 → option_no_checkout=1; remote-
# tracking refs land directly under refs/heads/ rather than refs/remotes/.
# hash=sha1-only "gix cannot open sha256 remotes, see gix/src/clone/fetch/mod.rs unimplemented!()"
title "gix clone --bare"
only_for_hash sha1-only && (sandbox
  it "matches git behavior" && {
    # TODO: expect_parity effect -- clone --bare upstream.git
    true
  }
)

# mode=effect — `--mirror` implies `--bare` AND uses refspec `+refs/*:refs/*`,
# plus sets `remote.<name>.mirror=true`.
# hash=sha1-only "gix cannot open sha256 remotes, see gix/src/clone/fetch/mod.rs unimplemented!()"
title "gix clone --mirror"
only_for_hash sha1-only && (sandbox
  it "matches git behavior" && {
    # TODO: expect_parity effect -- clone --mirror upstream.git
    true
  }
)

# mode=effect — `-n` / `--no-checkout` suppresses the worktree checkout
# step. Equivalent to the first half of `--bare` without the bareness.
# hash=sha1-only "gix cannot open sha256 remotes, see gix/src/clone/fetch/mod.rs unimplemented!()"
title "gix clone -n / --no-checkout"
only_for_hash sha1-only && (sandbox
  it "matches git: -n exits 0" && {
    # TODO: expect_parity effect -- clone -n upstream.git
    true
  }
  it "matches git: --no-checkout exits 0" && {
    # TODO: expect_parity effect -- clone --no-checkout upstream.git
    true
  }
)

# mode=effect — `--sparse` writes a sparse-checkout pattern of just the
# toplevel directory. Needs sparse-checkout support in gix.
# hash=sha1-only "gix cannot open sha256 remotes, see gix/src/clone/fetch/mod.rs unimplemented!()"
title "gix clone --sparse"
only_for_hash sha1-only && (sandbox
  it "matches git behavior" && {
    # TODO: expect_parity effect -- clone --sparse upstream.git
    true
  }
)

# --- origin & branch selection --------------------------------------------

# mode=effect — `-o <name>` / `--origin=<name>` replaces the default remote
# name `origin`. Overrides `clone.defaultRemoteName` config.
# hash=sha1-only "gix cannot open sha256 remotes, see gix/src/clone/fetch/mod.rs unimplemented!()"
title "gix clone -o / --origin=<name>"
only_for_hash sha1-only && (sandbox
  it "matches git: -o upstream exits 0" && {
    # TODO: expect_parity effect -- clone -o upstream upstream.git
    true
  }
  it "matches git: --origin=upstream exits 0" && {
    # TODO: expect_parity effect -- clone --origin=upstream upstream.git
    true
  }
)

# mode=effect — `-b <name>` / `--branch=<name>` points HEAD at <name>
# instead of the remote default. Accepts tag names too (detaches).
# hash=sha1-only "gix cannot open sha256 remotes, see gix/src/clone/fetch/mod.rs unimplemented!()"
title "gix clone -b / --branch=<name>"
only_for_hash sha1-only && (sandbox
  it "matches git: -b main exits 0" && {
    # TODO: expect_parity effect -- clone -b main upstream.git
    true
  }
  it "matches git: --branch=main exits 0" && {
    # TODO: expect_parity effect -- clone --branch=main upstream.git
    true
  }
)

# mode=effect — `--revision=<rev>` detaches HEAD at <rev>; no local branch
# is created; incompatible with --branch and --mirror.
# hash=sha1-only "gix cannot open sha256 remotes, see gix/src/clone/fetch/mod.rs unimplemented!()"
title "gix clone --revision=<rev>"
only_for_hash sha1-only && (sandbox
  it "matches git behavior" && {
    # TODO: expect_parity effect -- clone --revision=refs/heads/main upstream.git
    true
  }
)

# mode=effect — `--revision` + `--branch` conflict → die 128.
# hash=sha1-only "gix cannot open sha256 remotes, see gix/src/clone/fetch/mod.rs unimplemented!()"
title "gix clone --revision=<rev> --branch=<name> (conflict)"
only_for_hash sha1-only && (sandbox
  it "matches git: conflict dies 128" && {
    # TODO: expect_parity effect -- clone --revision=main --branch=main upstream.git
    true
  }
)

# mode=effect — `--revision` + `--mirror` conflict → die 128.
# hash=sha1-only "gix cannot open sha256 remotes, see gix/src/clone/fetch/mod.rs unimplemented!()"
title "gix clone --revision=<rev> --mirror (conflict)"
only_for_hash sha1-only && (sandbox
  it "matches git: conflict dies 128" && {
    # TODO: expect_parity effect -- clone --revision=main --mirror upstream.git
    true
  }
)

# --- local-optimization flags ---------------------------------------------

# mode=effect — `-l` / `--local` forces the local hardlink optimization.
# No-op when repo is already local (default for path args). `--no-local`
# forces the regular Git transport.
# hash=sha1-only "gix cannot open sha256 remotes, see gix/src/clone/fetch/mod.rs unimplemented!()"
title "gix clone -l / --local / --no-local"
only_for_hash sha1-only && (sandbox
  it "matches git: -l exits 0 on local path" && {
    # TODO: expect_parity effect -- clone -l upstream.git
    true
  }
  it "matches git: --local exits 0" && {
    # TODO: expect_parity effect -- clone --local upstream.git
    true
  }
  it "matches git: --no-local exits 0" && {
    # TODO: expect_parity effect -- clone --no-local upstream.git
    true
  }
)

# mode=effect — `--no-hardlinks` forces copy of .git/objects instead of
# hardlinking (no-op if not a local clone).
# hash=sha1-only "gix cannot open sha256 remotes, see gix/src/clone/fetch/mod.rs unimplemented!()"
title "gix clone --no-hardlinks"
only_for_hash sha1-only && (sandbox
  it "matches git behavior" && {
    # TODO: expect_parity effect -- clone --no-hardlinks upstream.git
    true
  }
)

# mode=effect — `-s` / `--shared` sets objects/info/alternates to share
# with the source. NOTE: dangerous in practice; effect-mode exits 0.
# hash=sha1-only "gix cannot open sha256 remotes, see gix/src/clone/fetch/mod.rs unimplemented!()"
title "gix clone -s / --shared"
only_for_hash sha1-only && (sandbox
  it "matches git: -s exits 0" && {
    # TODO: expect_parity effect -- clone -s upstream.git
    true
  }
  it "matches git: --shared exits 0" && {
    # TODO: expect_parity effect -- clone --shared upstream.git
    true
  }
)

# mode=effect — `--reject-shallow` / `--no-reject-shallow` fails the clone
# if the source is shallow (overrides `clone.rejectShallow` config).
# hash=sha1-only "gix cannot open sha256 remotes, see gix/src/clone/fetch/mod.rs unimplemented!()"
title "gix clone --reject-shallow / --no-reject-shallow"
only_for_hash sha1-only && (sandbox
  it "matches git: --reject-shallow on non-shallow upstream exits 0" && {
    # TODO: expect_parity effect -- clone --reject-shallow upstream.git
    true
  }
  it "matches git: --no-reject-shallow exits 0" && {
    # TODO: expect_parity effect -- clone --no-reject-shallow upstream.git
    true
  }
)

# --- reference repositories -----------------------------------------------

# mode=effect — `--reference=<repo>` adds <repo>/objects as an alternate.
# Requires the alternate to exist; aborts if it doesn't.
# hash=sha1-only "gix cannot open sha256 remotes, see gix/src/clone/fetch/mod.rs unimplemented!()"
title "gix clone --reference=<repo>"
only_for_hash sha1-only && (sandbox
  it "matches git behavior" && {
    # TODO: expect_parity effect -- clone --reference=reference.git upstream.git
    true
  }
)

# mode=effect — `--reference-if-able=<repo>` is `--reference` but with a
# warning (not a fatal error) when the alternate doesn't exist.
# hash=sha1-only "gix cannot open sha256 remotes, see gix/src/clone/fetch/mod.rs unimplemented!()"
title "gix clone --reference-if-able=<repo>"
only_for_hash sha1-only && (sandbox
  it "matches git: missing alternate warns then exits 0" && {
    # TODO: expect_parity effect -- clone --reference-if-able=/nonexistent upstream.git
    true
  }
)

# mode=effect — `--dissociate` copies borrowed objects locally after the
# clone and stops using alternates. Requires `--reference`.
# hash=sha1-only "gix cannot open sha256 remotes, see gix/src/clone/fetch/mod.rs unimplemented!()"
title "gix clone --dissociate --reference=<repo>"
only_for_hash sha1-only && (sandbox
  it "matches git behavior" && {
    # TODO: expect_parity effect -- clone --dissociate --reference=reference.git upstream.git
    true
  }
)

# --- shallow clone --------------------------------------------------------

# mode=effect — `--depth=<n>` truncates history to <n> commits. Implies
# `--single-branch` unless `--no-single-branch` overrides.
# hash=sha1-only "gix cannot open sha256 remotes, see gix/src/clone/fetch/mod.rs unimplemented!()"
title "gix clone --depth=<n>"
only_for_hash sha1-only && (sandbox
  it "matches git behavior" && {
    # TODO: expect_parity effect -- clone --depth=1 upstream.git
    true
  }
)

# mode=effect — `--depth` with a non-positive integer → die 128 with
# "depth %s is not a positive number".
# hash=sha1-only "gix cannot open sha256 remotes, see gix/src/clone/fetch/mod.rs unimplemented!()"
title "gix clone --depth=0 (non-positive)"
only_for_hash sha1-only && (sandbox
  it "matches git: --depth=0 dies 128" && {
    # TODO: expect_parity effect -- clone --depth=0 upstream.git
    true
  }
  it "matches git: --depth=-1 dies 128" && {
    # TODO: expect_parity effect -- clone --depth=-1 upstream.git
    true
  }
)

# mode=effect — `--shallow-since=<time>` creates a shallow clone with
# history after <time>. Implies `--single-branch`.
# hash=sha1-only "gix cannot open sha256 remotes, see gix/src/clone/fetch/mod.rs unimplemented!()"
title "gix clone --shallow-since=<time>"
only_for_hash sha1-only && (sandbox
  it "matches git behavior" && {
    # TODO: expect_parity effect -- clone --shallow-since=2020-01-01 upstream.git
    true
  }
)

# mode=effect — `--shallow-exclude=<ref>` creates a shallow clone excluding
# history reachable from <ref>. Multi-valued.
# hash=sha1-only "gix cannot open sha256 remotes, see gix/src/clone/fetch/mod.rs unimplemented!()"
title "gix clone --shallow-exclude=<ref>"
only_for_hash sha1-only && (sandbox
  it "matches git behavior" && {
    # TODO: expect_parity effect -- clone --shallow-exclude=refs/heads/main upstream.git
    true
  }
)

# mode=effect — `--single-branch` clones only the branch HEAD resolves to
# (or --branch). `--no-single-branch` explicitly cancels --depth's
# implicit single-branch.
# hash=sha1-only "gix cannot open sha256 remotes, see gix/src/clone/fetch/mod.rs unimplemented!()"
title "gix clone --single-branch / --no-single-branch"
only_for_hash sha1-only && (sandbox
  it "matches git: --single-branch exits 0" && {
    # TODO: expect_parity effect -- clone --single-branch upstream.git
    true
  }
  it "matches git: --no-single-branch exits 0" && {
    # TODO: expect_parity effect -- clone --no-single-branch upstream.git
    true
  }
)

# --- tags ----------------------------------------------------------------

# mode=effect — `--tags` / `--no-tags` toggles whether remote tags follow.
# `--no-tags` sets `remote.<name>.tagOpt=--no-tags` in the new repo.
# hash=sha1-only "gix cannot open sha256 remotes, see gix/src/clone/fetch/mod.rs unimplemented!()"
title "gix clone --tags / --no-tags"
only_for_hash sha1-only && (sandbox
  it "matches git: --tags exits 0" && {
    # TODO: expect_parity effect -- clone --tags upstream.git
    true
  }
  it "matches git: --no-tags exits 0" && {
    # TODO: expect_parity effect -- clone --no-tags upstream.git
    true
  }
)

# --- submodules ----------------------------------------------------------

# mode=effect — `--recurse-submodules[=<pathspec>]` initializes submodules
# after the clone. Multi-valued; `--recursive` is an alias.
# hash=sha1-only "gix cannot open sha256 remotes, see gix/src/clone/fetch/mod.rs unimplemented!()"
title "gix clone --recurse-submodules / --recursive"
only_for_hash sha1-only && (sandbox
  it "matches git: --recurse-submodules exits 0" && {
    # TODO: expect_parity effect -- clone --recurse-submodules upstream.git
    true
  }
  it "matches git: --recursive exits 0" && {
    # TODO: expect_parity effect -- clone --recursive upstream.git
    true
  }
  it "matches git: --recurse-submodules=path exits 0" && {
    # TODO: expect_parity effect -- clone --recurse-submodules=lib upstream.git
    true
  }
)

# mode=effect — `--shallow-submodules` / `--no-shallow-submodules` makes
# cloned submodules shallow (depth 1).
# hash=sha1-only "gix cannot open sha256 remotes, see gix/src/clone/fetch/mod.rs unimplemented!()"
title "gix clone --shallow-submodules / --no-shallow-submodules"
only_for_hash sha1-only && (sandbox
  it "matches git: --shallow-submodules exits 0" && {
    # TODO: expect_parity effect -- clone --shallow-submodules upstream.git
    true
  }
  it "matches git: --no-shallow-submodules exits 0" && {
    # TODO: expect_parity effect -- clone --no-shallow-submodules upstream.git
    true
  }
)

# mode=effect — `--remote-submodules` / `--no-remote-submodules` uses each
# submodule's remote-tracking HEAD, not the superproject's SHA-1.
# hash=sha1-only "gix cannot open sha256 remotes, see gix/src/clone/fetch/mod.rs unimplemented!()"
title "gix clone --remote-submodules / --no-remote-submodules"
only_for_hash sha1-only && (sandbox
  it "matches git: --remote-submodules exits 0" && {
    # TODO: expect_parity effect -- clone --remote-submodules upstream.git
    true
  }
  it "matches git: --no-remote-submodules exits 0" && {
    # TODO: expect_parity effect -- clone --no-remote-submodules upstream.git
    true
  }
)

# mode=effect — `--also-filter-submodules` requires `--filter` and
# `--recurse-submodules`; applies the partial-clone filter to submodules.
# hash=sha1-only "gix cannot open sha256 remotes, see gix/src/clone/fetch/mod.rs unimplemented!()"
title "gix clone --also-filter-submodules"
only_for_hash sha1-only && (sandbox
  it "matches git behavior" && {
    # TODO: expect_parity effect -- clone --filter=blob:none --recurse-submodules --also-filter-submodules upstream.git
    true
  }
)

# --- transport / networking ----------------------------------------------

# mode=effect — `-u` / `--upload-pack=<path>` overrides the remote
# upload-pack binary (ssh only; silently used/ignored elsewhere).
# hash=sha1-only "gix cannot open sha256 remotes, see gix/src/clone/fetch/mod.rs unimplemented!()"
title "gix clone -u / --upload-pack=<path>"
only_for_hash sha1-only && (sandbox
  it "matches git: -u exits 0 on file:// transport" && {
    # TODO: expect_parity effect -- clone -u /custom-upload-pack upstream.git
    true
  }
  it "matches git: --upload-pack exits 0" && {
    # TODO: expect_parity effect -- clone --upload-pack=/custom upstream.git
    true
  }
)

# mode=effect — `--server-option=<opt>` sends protocol-v2 server options.
# Multi-valued, preserves order. No-op on file:// transport.
# hash=sha1-only "gix cannot open sha256 remotes, see gix/src/clone/fetch/mod.rs unimplemented!()"
title "gix clone --server-option=<opt>"
only_for_hash sha1-only && (sandbox
  it "matches git behavior" && {
    # TODO: expect_parity effect -- clone --server-option=foo=bar upstream.git
    true
  }
)

# mode=effect — `-4` / `-6` / `--ipv4` / `--ipv6` restricts address family.
# No-op on file:// transport; exit-code parity only.
# hash=sha1-only "gix cannot open sha256 remotes, see gix/src/clone/fetch/mod.rs unimplemented!()"
title "gix clone -4 / -6 / --ipv4 / --ipv6"
only_for_hash sha1-only && (sandbox
  it "matches git: -4 exits 0" && {
    # TODO: expect_parity effect -- clone -4 upstream.git
    true
  }
  it "matches git: -6 exits 0" && {
    # TODO: expect_parity effect -- clone -6 upstream.git
    true
  }
  it "matches git: --ipv4 exits 0" && {
    # TODO: expect_parity effect -- clone --ipv4 upstream.git
    true
  }
  it "matches git: --ipv6 exits 0" && {
    # TODO: expect_parity effect -- clone --ipv6 upstream.git
    true
  }
)

# mode=effect — `-j` / `--jobs=<n>` sets submodule-fetch parallelism.
# No effect without submodules; defaults to submodule.fetchJobs config.
# hash=sha1-only "gix cannot open sha256 remotes, see gix/src/clone/fetch/mod.rs unimplemented!()"
title "gix clone -j / --jobs=<n>"
only_for_hash sha1-only && (sandbox
  it "matches git: -j 4 exits 0" && {
    # TODO: expect_parity effect -- clone -j 4 upstream.git
    true
  }
  it "matches git: --jobs=4 exits 0" && {
    # TODO: expect_parity effect -- clone --jobs=4 upstream.git
    true
  }
)

# --- repository structure ------------------------------------------------

# mode=effect — `--template=<dir>` overrides the template-directory used
# during init. Empty-template exits 0.
# hash=sha1-only "gix cannot open sha256 remotes, see gix/src/clone/fetch/mod.rs unimplemented!()"
title "gix clone --template=<dir>"
only_for_hash sha1-only && (sandbox
  it "matches git behavior" && {
    # TODO: expect_parity effect -- clone --template=./empty-template upstream.git
    true
  }
)

# mode=effect — `--separate-git-dir=<dir>` places .git at <dir> with a
# gitfile link back from the worktree.
# hash=sha1-only "gix cannot open sha256 remotes, see gix/src/clone/fetch/mod.rs unimplemented!()"
title "gix clone --separate-git-dir=<dir>"
only_for_hash sha1-only && (sandbox
  it "matches git behavior" && {
    # TODO: expect_parity effect -- clone --separate-git-dir=./real-git upstream.git
    true
  }
)

# mode=effect — `--bare --separate-git-dir` → die 128 with
# "options '--bare' and '--separate-git-dir' cannot be used together".
# hash=sha1-only "gix cannot open sha256 remotes, see gix/src/clone/fetch/mod.rs unimplemented!()"
title "gix clone --bare --separate-git-dir=<dir> (conflict)"
only_for_hash sha1-only && (sandbox
  it "matches git: conflict dies 128" && {
    # TODO: expect_parity effect -- clone --bare --separate-git-dir=./real-git upstream.git
    true
  }
)

# mode=effect — `--ref-format=<fmt>` picks the ref storage backend
# (files|reftable). Unknown values die 128.
# hash=sha1-only "gix cannot open sha256 remotes, see gix/src/clone/fetch/mod.rs unimplemented!()"
title "gix clone --ref-format=<fmt>"
only_for_hash sha1-only && (sandbox
  it "matches git: --ref-format=files exits 0" && {
    # TODO: expect_parity effect -- clone --ref-format=files upstream.git
    true
  }
  it "matches git: --ref-format=bogus dies 128" && {
    # TODO: expect_parity effect -- clone --ref-format=bogus upstream.git
    true
  }
)

# mode=effect — `-c <key>=<value>` / `--config=<key>=<value>` seeds the
# new repo's config before the initial fetch. Multi-valued.
# hash=sha1-only "gix cannot open sha256 remotes, see gix/src/clone/fetch/mod.rs unimplemented!()"
title "gix clone -c / --config=<key=value>"
only_for_hash sha1-only && (sandbox
  it "matches git: -c core.eol=lf exits 0" && {
    # TODO: expect_parity effect -- clone -c core.eol=lf upstream.git
    true
  }
  it "matches git: --config=core.eol=lf exits 0" && {
    # TODO: expect_parity effect -- clone --config=core.eol=lf upstream.git
    true
  }
)

# --- partial clone / bundle ----------------------------------------------

# mode=effect — `--filter=<spec>` uses partial clone and asks the server
# for a filtered subset. Requires promisor remote + protocol v2.
# hash=sha1-only "gix cannot open sha256 remotes, see gix/src/clone/fetch/mod.rs unimplemented!()"
title "gix clone --filter=<spec>"
only_for_hash sha1-only && (sandbox
  it "matches git behavior" && {
    # TODO: expect_parity effect -- clone --filter=blob:none upstream.git
    true
  }
)

# mode=effect — `--bundle-uri=<uri>` fetches a bundle before the real
# fetch. Incompatible with --depth / --shallow-since / --shallow-exclude.
# hash=sha1-only "gix cannot open sha256 remotes, see gix/src/clone/fetch/mod.rs unimplemented!()"
title "gix clone --bundle-uri=<uri>"
only_for_hash sha1-only && (sandbox
  it "matches git behavior" && {
    # TODO: expect_parity effect -- clone --bundle-uri=file:///tmp/bundle upstream.git
    true
  }
)

# mode=effect — `--bundle-uri` + `--depth` → die 128 (mirrors `if
# (bundle_uri && deepen) die(...)` in cmd_clone).
# hash=sha1-only "gix cannot open sha256 remotes, see gix/src/clone/fetch/mod.rs unimplemented!()"
title "gix clone --bundle-uri=<uri> --depth=<n> (conflict)"
only_for_hash sha1-only && (sandbox
  it "matches git: conflict dies 128" && {
    # TODO: expect_parity effect -- clone --bundle-uri=file:///tmp/bundle --depth=1 upstream.git
    true
  }
)
