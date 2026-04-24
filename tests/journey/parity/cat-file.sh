# Must be sourced into tests/parity.sh or tests/journey.sh — see tests/parity.sh.
#
# Parity scaffold for `git cat-file` ↔ `gix cat` (with `cat-file` alias to be
# added in iter 2 when the first row closes).
#
# One `title` + `it` block per flag derived from
# vendor/git/Documentation/git-cat-file.adoc and vendor/git/builtin/cat-file.c
# (cmd_cat_file + options[]). Flags enumerated from the options[] array
# (~19 entries), the four synopsis forms (<type> <object>, query modes,
# textconv/filters, batch family), and the BATCH OUTPUT atom table
# (objectname, objecttype, objectsize, objectsize:disk, deltabase, rest,
# objectmode).
#
# Verdict modes (comment above each block):
#   bytes  — scriptable output consumed by tooling: the query modes
#            (-e/-p/-t/-s) emit well-defined bytes that callers grep;
#            batch output is line-oriented and scripted; --filter output
#            uses a defined grammar. Most cat-file rows want byte parity.
#   effect — UX-level parity (exit-code match + optional prose check).
#            Used for usage/error paths and help, and for flags whose
#            observable side-effect is accept-or-reject rather than
#            byte-exact output (e.g. deprecated --allow-unknown-type).
#
# Coverage on gix's current Clap surface (src/plumbing/options/mod.rs):
#   Subcommands::Cat { revspec: String }
# That is the entire surface — one positional, zero flags. Every flag
# below will fail its first parity attempt by tripping Clap's
# UnknownArgument path or, more fundamentally, by the subcommand name
# itself: `gix cat-file` is not registered (only `gix cat`). The
# first-closable row (typically --help) therefore also carries the
# scaffold work: add `visible_alias = "cat-file"` to the Cat variant
# in src/plumbing/options/mod.rs so `gix cat-file …` parses.
#
# Closing a semantics row will generally require:
#   (1) add the flag to src/plumbing/options/mod.rs::Subcommands::Cat (or
#       promote Cat to Cat(cat::Platform) once the flag count justifies
#       the refactor),
#   (2) widen gitoxide_core::repository::cat::function::cat's signature
#       and/or add new entrypoints (batch readers, object-info emit,
#       type/size queries),
#   (3) implement semantics in gitoxide-core/src/repository/cat.rs using
#       gix::Object / gix::object::Header / gix::Repository::find* / the
#       `gix_odb` streaming API for blobs, `gix::filter` for --filters,
#       `gix_traverse` helpers for --batch-all-objects.
#
# Hash coverage: every row that opens a repository is `sha1-only` because
# gix-config rejects `extensions.objectFormat=sha256`
# (gix/src/config/tree/sections/extensions.rs try_into_object_format,
# sha1-only validator). Rows that short-circuit before repo load
# (--help, unknown-flag outside a repo, outside-of-repo) are `dual`.
# Rows flip to `dual` once that validator accepts sha256.
#
# parity-defaults:
#   hash=sha1-only "gix cannot load sha256 repos: extensions.objectFormat=sha256 rejected (gix/src/config/tree/sections/extensions.rs)"
#   mode=bytes

title "gix cat-file"

# --- meta / help --------------------------------------------------------

# mode=effect — clap --help short-circuits before repo load, exits 0.
# git's --help delegates to `man git-cat-file`; gix returns clap's
# auto-generated help. Message text diverges wildly; only the exit-code
# match is asserted. This row also demands the `visible_alias = "cat-file"`
# wiring on the Cat subcommand — without it `gix cat-file --help` parses
# as an unknown subcommand (exit 2) rather than reaching Clap's help
# short-circuit.
# hash=dual
# mode=effect
title "gix cat-file --help"
only_for_hash dual && (sandbox
  it "matches git behavior" && {
    expect_parity effect -- cat-file --help
  }
)

# --- argument-parsing error paths --------------------------------------

# mode=effect — unknown flag: git exits 129 (usage_msg_opt in
# vendor/git/parse-options.c). gix's Clap layer maps UnknownArgument to
# 129 via src/plumbing/main.rs. Tested inside a repo so the arg-parse
# path runs after repo setup.
# hash=sha1-only
# mode=effect
title "gix cat-file --bogus-flag"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    expect_parity effect -- cat-file --bogus-flag
  }
)

# mode=effect — `git cat-file` outside any repo dies 128 with
# "fatal: not a git repository (or any of the parent directories): .git".
# gix's plumbing repository() closure remaps the
# gix_discover::upwards::Error::NoGitRepository* variants to git's exact
# wording + exit 128 (see status.sh's analogous row).
# hash=dual
# mode=effect
title "gix cat-file (outside a repository)"
only_for_hash dual && (sandbox
  it "matches git behavior" && {
    expect_parity effect -- cat-file -p HEAD
  }
)

# mode=effect — bare `git cat-file` with no args and no mode flag prints
# usage to stderr and exits 129 (parse_options with empty argv after
# opts). Arg-parse error path; in-repo to exercise config load first.
# hash=sha1-only
# mode=effect
title "gix cat-file (no arguments)"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    expect_parity effect -- cat-file
  }
)

# --- query modes: -e/-p/-t/-s + <type> <object> -----------------------

# mode=effect — `-e <obj>` tests object existence; exits 0 if present,
# non-zero otherwise. No stdout output on success; stderr only on malformed
# inputs (per git-cat-file(1) DESCRIPTION: "exit with non-zero status if
# <object> is of an invalid format, emit error"). Effect mode: exit-code
# parity, no byte content to compare.
# hash=sha1-only
# mode=effect
title "gix cat-file -e (existing object)"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    expect_parity effect -- cat-file -e HEAD
  }
)

# mode=effect — `-e <missing-sha>` (well-formed but unknown oid) exits
# non-zero. git emits "fatal: Not a valid object name <sha>" and exits
# 128 (via die() in cat_one_file). gix must mirror.
# hash=sha1-only
# mode=effect
title "gix cat-file -e (missing object)"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior with a well-formed but absent oid" && {
    expect_parity effect -- cat-file -e 0000000000000000000000000000000000000000
  }
  it "matches git behavior with an unresolvable ref name (exit 128)" && {
    expect_parity effect -- cat-file -e nonexistent-ref
  }
)

# mode=bytes — `-p <blob>` pretty-prints blob contents: raw bytes to
# stdout. For a blob this is identical to `<type> <object>` with
# type=blob. Scripts consume this; byte-exact required.
# hash=sha1-only
title "gix cat-file -p (blob)"
only_for_hash sha1-only && (small-repo-in-sandbox
  echo blob-content > a && git add a && git commit -q -m "populate a"
  it "matches git behavior" && {
    expect_parity bytes -- cat-file -p HEAD:a
  }
)

# mode=bytes — `-p <tree>` pretty-prints tree entries. git's format is
# equivalent to `git ls-tree <tree>` (via cmd_ls_tree in cat-file.c).
# gix's current cat already dispatches tree → display_tree (pretty).
# hash=sha1-only
title "gix cat-file -p (tree)"
only_for_hash sha1-only && (small-repo-in-sandbox
  mkdir sub && echo nested > sub/c && git add sub && git commit -q -m "add subtree"
  it "matches git behavior (root tree with blob + subtree entries)" && {
    expect_parity bytes -- cat-file -p HEAD^{tree}
  }
)

# mode=bytes — `-p <commit>` pretty-prints commit header + message
# (tree/parent/author/committer/blank line/message). Byte-exact.
# hash=sha1-only
title "gix cat-file -p (commit)"
only_for_hash sha1-only && (small-repo-in-sandbox
  # TODO — expect_parity bytes -- cat-file -p HEAD
  it "matches git behavior" && { :; }
)

# mode=bytes — `-p <tag>` pretty-prints annotated-tag header + message.
# `annotated` tag exists in small-repo-in-sandbox via `git tag annotated
# -m "tag message"`.
# hash=sha1-only
title "gix cat-file -p (annotated tag)"
only_for_hash sha1-only && (small-repo-in-sandbox
  # TODO — expect_parity bytes -- cat-file -p annotated
  it "matches git behavior" && { :; }
)

# mode=bytes — `-t <obj>` prints the object's type name: "blob", "tree",
# "commit", or "tag". One line, trailing newline.
# hash=sha1-only
title "gix cat-file -t"
only_for_hash sha1-only && (small-repo-in-sandbox
  # TODO — expect_parity bytes -- cat-file -t HEAD
  it "matches git behavior on a commit (type = commit)" && { :; }
  # TODO — expect_parity bytes -- cat-file -t HEAD^{tree}
  it "matches git behavior on a tree (type = tree)" && { :; }
  # TODO — expect_parity bytes -- cat-file -t HEAD:a
  it "matches git behavior on a blob (type = blob)" && { :; }
  # TODO — expect_parity bytes -- cat-file -t annotated
  it "matches git behavior on an annotated tag (type = tag)" && { :; }
)

# mode=bytes — `-s <obj>` prints the object's size in bytes (decimal,
# trailing newline). Source: odb_read_object_info_extended → oi.sizep.
# hash=sha1-only
title "gix cat-file -s"
only_for_hash sha1-only && (small-repo-in-sandbox
  # TODO — expect_parity bytes -- cat-file -s HEAD
  it "matches git behavior on a commit" && { :; }
  # TODO — expect_parity bytes -- cat-file -s HEAD:a
  it "matches git behavior on a blob" && { :; }
)

# mode=bytes — positional `<type> <object>` form: raw object bytes
# (uncompressed) with the given type as a type hint / assertion. If the
# object's real type does not match and cannot be dereferenced to the
# requested type, git dies 128. Otherwise stdout is the raw bytes.
# hash=sha1-only
title "gix cat-file <type> <object>"
only_for_hash sha1-only && (small-repo-in-sandbox
  # TODO — expect_parity bytes -- cat-file blob HEAD:a
  it "matches git behavior: blob <obj>" && { :; }
  # TODO — expect_parity bytes -- cat-file commit HEAD
  it "matches git behavior: commit <obj>" && { :; }
  # TODO — expect_parity bytes -- cat-file tree HEAD^{tree}
  it "matches git behavior: tree <obj>" && { :; }
)

# mode=effect — positional `<type> <object>` with a type mismatch (e.g.
# `blob HEAD` where HEAD is a commit, and the commit does not deref to
# a blob). git dies 128 with "fatal: <sha> is not a valid 'blob' object".
# hash=sha1-only
# mode=effect
title "gix cat-file <type> <object> (type mismatch)"
only_for_hash sha1-only && (small-repo-in-sandbox
  # TODO — expect_parity effect -- cat-file blob HEAD
  it "matches git behavior" && { :; }
)

# --- mailmap ------------------------------------------------------------

# mode=effect — `--mailmap` / `--no-mailmap` are aliases of
# `--use-mailmap` / `--no-use-mailmap`. Toggles mailmap ident rewriting
# for commit/tag `-s`/`-p` output. On a fixture without a .mailmap file
# the flag is a no-op content-wise; exit-code parity holds.
# hash=sha1-only
# mode=effect
title "gix cat-file --mailmap / --no-mailmap"
only_for_hash sha1-only && (small-repo-in-sandbox
  # TODO — expect_parity effect -- cat-file --mailmap -p HEAD
  it "matches git behavior with --mailmap -p HEAD" && { :; }
  # TODO — expect_parity effect -- cat-file --no-mailmap -p HEAD
  it "matches git behavior with --no-mailmap -p HEAD" && { :; }
)

# mode=effect — `--use-mailmap` / `--no-use-mailmap` (canonical spelling).
# Same semantics as --mailmap; separate row because Clap aliasing needs
# both entries reachable.
# hash=sha1-only
# mode=effect
title "gix cat-file --use-mailmap / --no-use-mailmap"
only_for_hash sha1-only && (small-repo-in-sandbox
  # TODO — expect_parity effect -- cat-file --use-mailmap -p HEAD
  it "matches git behavior with --use-mailmap -p HEAD" && { :; }
  # TODO — expect_parity effect -- cat-file --no-use-mailmap -p HEAD
  it "matches git behavior with --no-use-mailmap -p HEAD" && { :; }
)

# mode=bytes — `--use-mailmap -s` with an actual .mailmap file present:
# size reflects post-rewrite object (longer or shorter than raw).
# Exercises the mailmap replacement path in cat_one_file.
# hash=sha1-only
title "gix cat-file --use-mailmap -s (with .mailmap)"
only_for_hash sha1-only && (small-repo-in-sandbox
  printf 'Mapped Name <mapped@example.com> Sebastian Thiel <git@example.com>\n' > .mailmap
  git add .mailmap && git commit -q -m "add mailmap"
  # TODO — expect_parity bytes -- cat-file --use-mailmap -s HEAD
  it "matches git behavior" && { :; }
)

# --- textconv / filters ------------------------------------------------

# mode=bytes — `--textconv <rev:path>`: runs the textconv filter
# configured for <path> on the blob at <rev:path>. With no textconv
# filter configured the output is the raw blob. Exercises path resolution.
# hash=sha1-only
title "gix cat-file --textconv"
only_for_hash sha1-only && (small-repo-in-sandbox
  # TODO — expect_parity bytes -- cat-file --textconv HEAD:a
  it "matches git behavior" && { :; }
)

# mode=bytes — `--filters <rev:path>`: runs smudge filters / EOL
# conversions / checkout filters on the blob. With no filters configured
# output is raw blob. Exercises gix::filter::Pipeline wiring.
# hash=sha1-only
title "gix cat-file --filters"
only_for_hash sha1-only && (small-repo-in-sandbox
  # TODO — expect_parity bytes -- cat-file --filters HEAD:a
  it "matches git behavior" && { :; }
)

# mode=bytes — `--path=<path> --textconv <rev>`: alternative spelling
# for textconv when you have only the blob SHA (not a tree-ish:path).
# Same output as `--textconv <rev>:<path>`.
# hash=sha1-only
title "gix cat-file --path=<path> --textconv"
only_for_hash sha1-only && (small-repo-in-sandbox
  # TODO — expect_parity bytes -- cat-file --path=a --textconv HEAD:a
  it "matches git behavior" && { :; }
)

# mode=bytes — `--path=<path> --filters <rev>`: same as `--path=<path>
# --textconv` but invokes the smudge-filter path.
# hash=sha1-only
title "gix cat-file --path=<path> --filters"
only_for_hash sha1-only && (small-repo-in-sandbox
  # TODO — expect_parity bytes -- cat-file --path=a --filters HEAD:a
  it "matches git behavior" && { :; }
)

# mode=effect — `--path=<path>` without `--textconv`/`--filters` is a
# usage error: git dies 129 with "fatal: '--path=<path|tree-ish>' needs
# '--filters' or '--textconv'".
# hash=sha1-only
# mode=effect
title "gix cat-file --path=<path> (without --textconv/--filters)"
only_for_hash sha1-only && (small-repo-in-sandbox
  # TODO — expect_parity effect -- cat-file --path=a HEAD:a
  it "matches git behavior" && { :; }
)

# --- batch family ------------------------------------------------------

# mode=bytes — `--batch`: reads object names from stdin, emits
# `<oid> SP <type> SP <size> LF<contents>LF` per line. Default format.
# hash=sha1-only
title "gix cat-file --batch"
only_for_hash sha1-only && (small-repo-in-sandbox
  # TODO — expect_parity bytes -- cat-file --batch  (stdin fed via heredoc)
  it "matches git behavior" && { :; }
)

# mode=bytes — `--batch=<format>`: custom format string. Exercises
# %(objectname), %(objecttype), %(objectsize) expansion + contents trailer.
# hash=sha1-only
title "gix cat-file --batch=<format>"
only_for_hash sha1-only && (small-repo-in-sandbox
  # TODO — expect_parity bytes -- cat-file --batch='%(objecttype) %(objectsize)'
  it "matches git behavior" && { :; }
)

# mode=bytes — `--batch-check`: like --batch but without the <contents>
# trailer. One `<oid> SP <type> SP <size> LF` per input line.
# hash=sha1-only
title "gix cat-file --batch-check"
only_for_hash sha1-only && (small-repo-in-sandbox
  # TODO — expect_parity bytes -- cat-file --batch-check
  it "matches git behavior" && { :; }
)

# mode=bytes — `--batch-check=<format>`: custom format, no contents.
# hash=sha1-only
title "gix cat-file --batch-check=<format>"
only_for_hash sha1-only && (small-repo-in-sandbox
  # TODO — expect_parity bytes -- cat-file --batch-check='%(objectname) %(objecttype)'
  it "matches git behavior" && { :; }
)

# mode=bytes — `--batch-command`: reads `contents <obj>` / `info <obj>` /
# `flush` commands from stdin, dispatches each to --batch or --batch-check
# equivalent. Default format per command.
# hash=sha1-only
title "gix cat-file --batch-command"
only_for_hash sha1-only && (small-repo-in-sandbox
  # TODO — expect_parity bytes -- cat-file --batch-command  (stdin: "info HEAD\ncontents HEAD:a\n")
  it "matches git behavior" && { :; }
)

# mode=bytes — `--batch-command=<format>`: custom format applied to each
# command's info/contents output.
# hash=sha1-only
title "gix cat-file --batch-command=<format>"
only_for_hash sha1-only && (small-repo-in-sandbox
  # TODO — expect_parity bytes -- cat-file --batch-command='%(objecttype)'
  it "matches git behavior" && { :; }
)

# mode=bytes — `--batch-all-objects`: ignore stdin, iterate every object
# in the odb (loose + packed + alternates). Combined with --batch-check
# for scriptable output. Requires --batch/--batch-check. Bitmap-aware on
# git's side; gix enumerates via gix_odb::Cache::iter.
# hash=sha1-only
title "gix cat-file --batch-all-objects"
only_for_hash sha1-only && (small-repo-in-sandbox
  # TODO — expect_parity bytes -- cat-file --batch-all-objects --batch-check
  it "matches git behavior (sorted hash order)" && { :; }
)

# mode=effect — `--batch-all-objects` without `--batch[-check]` is a
# usage error: git dies 129 "'--batch-all-objects' requires a batch mode".
# hash=sha1-only
# mode=effect
title "gix cat-file --batch-all-objects (without batch)"
only_for_hash sha1-only && (small-repo-in-sandbox
  # TODO — expect_parity effect -- cat-file --batch-all-objects
  it "matches git behavior" && { :; }
)

# mode=effect — `--buffer`: toggles stdio buffering on batch output.
# Default is line-flushed for interactive use; --buffer switches to
# normal stdio buffering. Observable effect: output timing, not
# content. Pairs with batch mode only (outside batch git errors 129).
# hash=sha1-only
# mode=effect
title "gix cat-file --buffer"
only_for_hash sha1-only && (small-repo-in-sandbox
  # TODO — expect_parity effect -- cat-file --buffer --batch-check
  it "matches git behavior with --buffer --batch-check" && { :; }
)

# mode=effect — `--buffer` without batch: usage error 129.
# hash=sha1-only
# mode=effect
title "gix cat-file --buffer (without batch)"
only_for_hash sha1-only && (small-repo-in-sandbox
  # TODO — expect_parity effect -- cat-file --buffer
  it "matches git behavior" && { :; }
)

# mode=effect — `--unordered`: with --batch-all-objects, visit objects
# in pack-storage order rather than hash-sorted. Observable effect is
# line ordering; the set of lines is identical. Effect-mode parity
# (exit-code match) chosen over bytes because order is spec-loose.
# hash=sha1-only
# mode=effect
title "gix cat-file --unordered"
only_for_hash sha1-only && (small-repo-in-sandbox
  # TODO — expect_parity effect -- cat-file --batch-all-objects --batch-check --unordered
  it "matches git behavior" && { :; }
)

# mode=bytes — `--follow-symlinks`: with --batch/--batch-check, resolve
# in-tree symlinks when requesting `tree-ish:path`. Produces specialized
# status lines (`symlink <size>`, `dangling <size>`, `loop <size>`,
# `notdir <size>`) per the BATCH OUTPUT section of git-cat-file(1).
# hash=sha1-only
title "gix cat-file --follow-symlinks"
only_for_hash sha1-only && (small-repo-in-sandbox
  ln -s a link-to-a && git add link-to-a && git commit -q -m "add symlink"
  # TODO — expect_parity bytes -- cat-file --batch --follow-symlinks  (stdin: "HEAD:link-to-a\n")
  it "matches git behavior" && { :; }
)

# mode=effect — `--follow-symlinks` without batch: usage error 129.
# hash=sha1-only
# mode=effect
title "gix cat-file --follow-symlinks (without batch)"
only_for_hash sha1-only && (small-repo-in-sandbox
  # TODO — expect_parity effect -- cat-file --follow-symlinks
  it "matches git behavior" && { :; }
)

# mode=bytes — `-Z`: NUL-delimits input AND output (replaces LF with NUL
# in both directions). Recommended for scripting when object names may
# contain LF. Paired with any batch mode.
# hash=sha1-only
title "gix cat-file -Z (NUL in+out)"
only_for_hash sha1-only && (small-repo-in-sandbox
  # TODO — expect_parity bytes -- cat-file --batch-check -Z  (stdin: "HEAD\0HEAD:a\0")
  it "matches git behavior" && { :; }
)

# mode=bytes — `-z`: NUL-delimits INPUT only (output remains LF).
# Deprecated in favor of `-Z` because output can be ambiguous.
# hash=sha1-only
title "gix cat-file -z (NUL input only)"
only_for_hash sha1-only && (small-repo-in-sandbox
  # TODO — expect_parity bytes -- cat-file --batch-check -z  (stdin: "HEAD\0HEAD:a\0")
  it "matches git behavior" && { :; }
)

# mode=effect — `-Z` without batch mode: usage error 129.
# hash=sha1-only
# mode=effect
title "gix cat-file -Z (without batch)"
only_for_hash sha1-only && (small-repo-in-sandbox
  # TODO — expect_parity effect -- cat-file -Z
  it "matches git behavior" && { :; }
)

# --- --filter (batch-only) ---------------------------------------------

# mode=bytes — `--filter=blob:none`: omit blobs from batch output.
# Excluded blobs reported with `<input> excluded LF`. Batch-only.
# hash=sha1-only
title "gix cat-file --filter=blob:none"
only_for_hash sha1-only && (small-repo-in-sandbox
  # TODO — expect_parity bytes -- cat-file --batch-check --batch-all-objects --filter=blob:none
  it "matches git behavior" && { :; }
)

# mode=bytes — `--filter=blob:limit=<n>`: omit blobs larger than <n>
# bytes. Supports k/m/g suffixes.
# hash=sha1-only
title "gix cat-file --filter=blob:limit=<n>"
only_for_hash sha1-only && (small-repo-in-sandbox
  # TODO — expect_parity bytes -- cat-file --batch-check --batch-all-objects --filter=blob:limit=0
  it "matches git behavior with limit=0" && { :; }
  # TODO — expect_parity bytes -- cat-file --batch-check --batch-all-objects --filter=blob:limit=1k
  it "matches git behavior with limit=1k" && { :; }
)

# mode=bytes — `--filter=object:type=<t>`: keep only objects of the
# requested type (blob|tree|commit|tag). Others excluded per filter rules.
# hash=sha1-only
title "gix cat-file --filter=object:type=<type>"
only_for_hash sha1-only && (small-repo-in-sandbox
  # TODO — expect_parity bytes -- cat-file --batch-check --batch-all-objects --filter=object:type=commit
  it "matches git behavior with type=commit" && { :; }
  # TODO — expect_parity bytes -- cat-file --batch-check --batch-all-objects --filter=object:type=blob
  it "matches git behavior with type=blob" && { :; }
)

# mode=effect — `--filter=<spec>` outside batch mode is a usage error:
# git exits 129 "objects filter only supported in batch mode" via
# usage() in cmd_cat_file.
# hash=sha1-only
# mode=effect
title "gix cat-file --filter (without batch)"
only_for_hash sha1-only && (small-repo-in-sandbox
  # TODO — expect_parity effect -- cat-file --filter=blob:none
  it "matches git behavior" && { :; }
)

# mode=bytes — `--no-filter` disables any previously-specified filter.
# Accepted; exits 0 when paired with a batch mode.
# hash=sha1-only
title "gix cat-file --no-filter"
only_for_hash sha1-only && (small-repo-in-sandbox
  # TODO — expect_parity bytes -- cat-file --batch-check --batch-all-objects --no-filter
  it "matches git behavior" && { :; }
)

# --- batch format atoms (--batch-check=<format>) -----------------------

# mode=bytes — `%(objectname)`: full-length hex oid.
# hash=sha1-only
title "gix cat-file --batch-check='%(objectname)'"
only_for_hash sha1-only && (small-repo-in-sandbox
  # TODO — expect_parity bytes -- cat-file --batch-check='%(objectname)'  (stdin: "HEAD\n")
  it "matches git behavior" && { :; }
)

# mode=bytes — `%(objecttype)`: type name.
# hash=sha1-only
title "gix cat-file --batch-check='%(objecttype)'"
only_for_hash sha1-only && (small-repo-in-sandbox
  # TODO — expect_parity bytes -- cat-file --batch-check='%(objecttype)'  (stdin: "HEAD\n")
  it "matches git behavior" && { :; }
)

# mode=bytes — `%(objectsize)`: size in bytes.
# hash=sha1-only
title "gix cat-file --batch-check='%(objectsize)'"
only_for_hash sha1-only && (small-repo-in-sandbox
  # TODO — expect_parity bytes -- cat-file --batch-check='%(objectsize)'  (stdin: "HEAD\n")
  it "matches git behavior" && { :; }
)

# mode=bytes — `%(objectsize:disk)`: on-disk size (packed or loose). Per
# CAVEATS, identity of the copy measured is undefined when multiple
# copies exist, so this may diverge fixture-to-fixture; pinning to a
# freshly-init'd loose-only repo keeps it stable.
# hash=sha1-only
title "gix cat-file --batch-check='%(objectsize:disk)'"
only_for_hash sha1-only && (small-repo-in-sandbox
  # TODO — expect_parity bytes -- cat-file --batch-check='%(objectsize:disk)'  (stdin: "HEAD\n")
  it "matches git behavior" && { :; }
)

# mode=bytes — `%(deltabase)`: full-length hex oid of delta base, or
# null-oid (40/64 zeros) if not stored as delta. Loose-only fixture ⇒
# expect null-oid.
# hash=sha1-only
title "gix cat-file --batch-check='%(deltabase)'"
only_for_hash sha1-only && (small-repo-in-sandbox
  # TODO — expect_parity bytes -- cat-file --batch-check='%(deltabase)'  (stdin: "HEAD\n")
  it "matches git behavior" && { :; }
)

# mode=bytes — `%(rest)`: splits input on first whitespace; characters
# after that split are emitted in place of %(rest). Must set
# split_on_whitespace on expand_data.
# hash=sha1-only
title "gix cat-file --batch-check='%(rest)'"
only_for_hash sha1-only && (small-repo-in-sandbox
  # TODO — expect_parity bytes -- cat-file --batch-check='%(objectname) %(rest)'  (stdin: "HEAD extra stuff\n")
  it "matches git behavior" && { :; }
)

# mode=bytes — `%(objectmode)`: 6-digit octal mode when object came
# from a tree/index entry that has mode info, else empty string. In
# non-tree contexts expand to "".
# hash=sha1-only
title "gix cat-file --batch-check='%(objectmode)'"
only_for_hash sha1-only && (small-repo-in-sandbox
  # TODO — expect_parity bytes -- cat-file --batch-check='%(objectmode) %(objectname)'  (stdin: "HEAD:a\n")
  it "matches git behavior" && { :; }
)

# --- batch input error paths -------------------------------------------

# mode=bytes — missing object on stdin: batch prints `<input> missing LF`
# and keeps reading. Exit 0 at EOF.
# hash=sha1-only
title "gix cat-file --batch-check (missing object on stdin)"
only_for_hash sha1-only && (small-repo-in-sandbox
  # TODO — expect_parity bytes -- cat-file --batch-check  (stdin: "notanoid\n")
  it "matches git behavior" && { :; }
)

# mode=bytes — ambiguous short sha on stdin (requires fabricated
# collision or intentional short prefix): batch prints `<input>
# ambiguous LF`. Hard to stage deterministically from a fresh fixture
# without crafted objects; covered by a short-sha prefix that matches
# multiple objects in the seeded repo.
# hash=sha1-only
title "gix cat-file --batch-check (ambiguous short sha)"
only_for_hash sha1-only && (small-repo-in-sandbox
  # TODO — construct two objects sharing a 1-byte prefix; feed that prefix
  # TODO — expect_parity bytes -- cat-file --batch-check  (stdin: "<ambiguous-prefix>\n")
  it "matches git behavior" && { :; }
)

# --- historical / hidden -----------------------------------------------

# mode=effect — `--allow-unknown-type`: historical option, no-op per the
# option table's N_("historical option -- no-op") help text. Must be
# accepted by Clap without error; exits 0 alongside a valid mode flag.
# hash=sha1-only
# mode=effect
title "gix cat-file --allow-unknown-type"
only_for_hash sha1-only && (small-repo-in-sandbox
  # TODO — expect_parity effect -- cat-file --allow-unknown-type -t HEAD
  it "matches git behavior" && { :; }
)

# --- submodule / dangling / notdir / loop (follow-symlinks statuses) ---

# mode=bytes — submodule entry in a tree where the commit is not in the
# local odb: batch emits `<oid> submodule LF` (see S_IFGITLINK branch
# in batch_object_write). Requires a fixture with a gitlink tree entry.
# hash=sha1-only
title "gix cat-file --batch (submodule entry)"
only_for_hash sha1-only && (small-repo-in-sandbox
  # TODO — add submodule fixture (gitlink tree entry whose commit is absent locally)
  # TODO — expect_parity bytes -- cat-file --batch  (stdin: "HEAD:sub\n")
  it "matches git behavior" && { :; }
)

# mode=bytes — `--follow-symlinks` on a link that points outside the
# tree root: emits `symlink <size> LF<target>LF` with the external
# path (absolute or `..`-relative). Fixture: link to /etc/passwd.
# hash=sha1-only
title "gix cat-file --batch --follow-symlinks (link outside tree)"
only_for_hash sha1-only && (small-repo-in-sandbox
  ln -s /etc/passwd alink && git add alink && git commit -q -m "alink"
  # TODO — expect_parity bytes -- cat-file --batch --follow-symlinks  (stdin: "HEAD:alink\n")
  it "matches git behavior" && { :; }
)

# mode=bytes — `--follow-symlinks` on a broken symlink (dangling):
# emits `dangling <size> LF<target>LF`.
# hash=sha1-only
title "gix cat-file --batch --follow-symlinks (dangling)"
only_for_hash sha1-only && (small-repo-in-sandbox
  ln -s missing-target dlink && git add dlink && git commit -q -m "dlink"
  # TODO — expect_parity bytes -- cat-file --batch --follow-symlinks  (stdin: "HEAD:dlink\n")
  it "matches git behavior" && { :; }
)

# --- combined flags ----------------------------------------------------

# mode=effect — conflicting mode flags (e.g. `-e -p`): git exits 129
# "options '-e' and '-p' are mutually exclusive" via OPT_CMDMODE.
# hash=sha1-only
# mode=effect
title "gix cat-file -e -p (mutually exclusive modes)"
only_for_hash sha1-only && (small-repo-in-sandbox
  # TODO — expect_parity effect -- cat-file -e -p HEAD
  it "matches git behavior" && { :; }
)

# mode=effect — mode flag combined with batch (e.g. `-p --batch`): git
# exits 129 "'-p' is incompatible with batch mode".
# hash=sha1-only
# mode=effect
title "gix cat-file -p --batch (mode + batch)"
only_for_hash sha1-only && (small-repo-in-sandbox
  # TODO — expect_parity effect -- cat-file -p --batch
  it "matches git behavior" && { :; }
)

# mode=effect — two batch modes at once (e.g. `--batch --batch-check`):
# git exits 128 "only one batch option may be specified" via the
# batch_option_callback error() path.
# hash=sha1-only
# mode=effect
title "gix cat-file --batch --batch-check (conflicting batches)"
only_for_hash sha1-only && (small-repo-in-sandbox
  # TODO — expect_parity effect -- cat-file --batch --batch-check
  it "matches git behavior" && { :; }
)

# End-of-file sentinel: when every row is `only_for_hash sha1-only` and the
# active hash is sha256, the last statement returns 1 (skip), which would
# propagate out of `source` and trip `set -e` in tests/parity.sh. A trailing
# `:` normalizes the exit code so a fully-skipped file still returns 0.
:
