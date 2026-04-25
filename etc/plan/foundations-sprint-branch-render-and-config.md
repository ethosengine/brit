# Foundations sprint — branch -v renderer + branch-section config writes

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Close the residual compat rows on `git branch` (5 in `branch.sh` for the `-v`/`-vv`/`--abbrev`/`--column` cluster + 4 for upstream/description writes) by landing two reusable foundations: a column-aligned for-each-ref-style renderer in `gitoxide-core::repository::branch::list`, and a branch-section read/write surface (`branch.<name>.{remote,merge,description}`) on `Repository` plus a `core.editor`-aware EDITOR helper.

**Architecture:**
1. **Renderer.** Extend `list::Options` with `verbose: u8`, `abbrev: Option<usize>`, `no_abbrev: bool`, `column: Option<String>`, `no_column: bool`. In `list()` compute `maxwidth` after filtering, render `{marker}{name:<maxwidth$} {short_sha} {subject}` for `-v`; insert `[<upstream>: ahead N, behind N]` between SHA and subject for `-vv` (resolve via existing `Repository::branch_remote_ref_name`/`branch_remote_tracking_ref_name`, ahead/behind via `gix::revision::Walk`). `--column=always` calls a small packing helper that lays bare-list rows into a width-fitting grid (uses `$COLUMNS` env or 80 default).
2. **Config writes.** Three methods on `Repository`: `set_branch_upstream(name, upstream)`, `unset_branch_upstream(name)`, `set_branch_description(name, value)`. Implement with `config_snapshot_mut().set_subsection_value(...)` then `.commit()`. The `--track` create-side call reuses `set_branch_upstream` after the ref is written.
3. **EDITOR helper.** New `gitoxide-core::shared::editor::edit_file(repo: &Repository, initial: &[u8]) -> anyhow::Result<Vec<u8>>` — resolves `$GIT_EDITOR` → `core.editor` config → `$VISUAL` → `$EDITOR` → `vi`, spawns via `gix::command::prepare()`, returns edited bytes. `--edit-description` uses it; reusable by future `commit -e`, `tag --edit`, etc.

**Tech stack:** Rust, gix-config (`SnapshotMut::set_subsection_value`), gix-revision (`Walk` for ahead/behind), gix-command (`prepare`), `anyhow` at gitoxide-core layer, journey-test bytes-mode `expect_parity` for verification.

**Anchor files (reference only — don't re-read in tasks):**
- `gitoxide-core/src/repository/branch.rs:434-628` — `list()` to extend.
- `src/plumbing/options/mod.rs:378-581` — branch `Platform` clap struct (flags already parsed).
- `src/plumbing/main.rs:895-1172` — branch dispatch (stub at line 988-994 to remove).
- `gix/src/repository/config/branch.rs:44-101, 223-239` — existing read APIs (consumers we must produce config for).
- `gix/src/config/snapshot/access.rs:144-165` — `SnapshotMut::set_subsection_value`.
- `gix/src/config/tree/sections/branch.rs:1-27` — `Branch::{REMOTE, MERGE, PUSH_REMOTE}` key constants; **add `DESCRIPTION` here**.
- `gitoxide-core/src/repository/tag.rs:212-220` — current EDITOR-stub pattern (placeholder we'll replace globally).
- `tests/journey/parity/branch.sh:165-215, 299-311, 489-540` — the 9 compat rows that flip in this sprint.
- `vendor/git/builtin/branch.c:386-443, 466-467` — `build_format()` + `calc_maxwidth()` reference.
- `vendor/git/builtin/branch.c:678-707, 963-995` — upstream/edit-description writes reference.
- `vendor/git/branch.c:91-142` — `install_branch_config_multiple_remotes` (the canonical write order: clear `branch.<n>.merge` then re-add).

---

## Deliverable 1 — branch -v renderer

### Task 1: Plumb `-v` / `--abbrev` / `--column` into `list::Options`

**Files:**
- Modify: `gitoxide-core/src/repository/branch.rs` (around line 334-382 — `list::Options` struct)
- Modify: `src/plumbing/main.rs` (around line 895-933, 1137-1172 — destructure + thread through)

- [ ] **Step 1: Extend `list::Options` with the five new fields.** Open `gitoxide-core/src/repository/branch.rs`, find the `Options` struct (line ~334), add:

```rust
pub struct Options {
    pub kind: Kind,
    pub patterns: Vec<BString>,
    pub contains: Option<OsString>,
    pub no_contains: Option<OsString>,
    pub merged: Option<OsString>,
    pub no_merged: Option<OsString>,
    pub points_at: Option<OsString>,
    pub format_string: Option<String>,
    pub sort: Vec<String>,
    pub omit_empty: bool,
    pub ignore_case: bool,
    /// `-v` count: 0 = bare, 1 = `-v` (sha + subject), 2 = `-vv` (+ upstream tracking).
    pub verbose: u8,
    /// `--abbrev=<n>`. None = use `core.abbrev` / 7 default.
    pub abbrev: Option<usize>,
    /// `--no-abbrev` — render full hash regardless of `--abbrev`.
    pub no_abbrev: bool,
    /// `--column[=<opts>]`. None = honor `column.branch` config / off.
    pub column: Option<String>,
    /// `--no-column` — explicit off, beats config.
    pub no_column: bool,
}
```

- [ ] **Step 2: Pass the new flags from CLI dispatch.** In `src/plumbing/main.rs:895`, replace the leading-underscore destructuring of `verbose`, `abbrev`, `no_abbrev`, `column`, `no_column` with real bindings:

```rust
let branch::Platform {
    list: _list_flag,
    remotes,
    all,
    ...
    verbose,           // was `verbose: _verbose`
    quiet: _quiet,
    ...
    abbrev,            // was `abbrev: _abbrev`
    no_abbrev,         // was `no_abbrev: _no_abbrev`
    ...
    column,            // was `column: _column`
    no_column,         // was `no_column: _no_column`
    ...
} = platform;
```

Then in the list-mode `core::repository::branch::list::Options { ... }` constructor (around line 1137-1170), add the five new fields. `verbose` is already `u8` (from `clap(action = ArgAction::Count)`). Pass them through as-is.

- [ ] **Step 3: Build it.** Run:

```bash
cargo check -p gitoxide-core -p gitoxide
```

Expected: clean. If `Options` is constructed elsewhere (search `Options { kind:`), update those call sites too.

- [ ] **Step 4: Commit.**

```bash
git add gitoxide-core/src/repository/branch.rs src/plumbing/main.rs
git commit -m "branch: thread -v/--abbrev/--column flags into list::Options"
```

---

### Task 2: Implement column-aligned `-v` rendering (name + SHA + subject)

**Files:**
- Modify: `gitoxide-core/src/repository/branch.rs:539-628` (the two render loops in `list()`)

- [ ] **Step 1: Refactor the two render loops to share a row collector.** Currently lines 539-577 (locals) and 580-625 (remotes) duplicate the keep+sort+emit dance. Refactor into a single phase: collect `Vec<Row>` where `Row` is:

```rust
struct Row {
    /// Display name as written by git: short for locals, `remotes/<full>` if --all+show_local for remotes,
    /// otherwise short for remotes.
    display_name: String,
    /// Tip ObjectId (None only if peeling fails — log + skip; mirrors git's `ref_array_item.objectname`).
    tip: Option<gix::ObjectId>,
    /// Whether this row is the current HEAD (only true for locals).
    is_head: bool,
    /// Full ref name for `--format` and upstream resolution: `refs/heads/<short>` or `refs/remotes/<short>`.
    full_name: String,
}
```

Keep the existing `kept: Vec<String>` collection pattern but upgrade to `Vec<Row>`. The existing `tip_of` helper (line 500-502) supplies the tip. Sort by `display_name`. Honor `sort_descending` (line 462).

- [ ] **Step 2: Compute `maxwidth` for `-v` formatting.** After both locals and remotes are collected, before the emit loop:

```rust
let maxwidth = if options.verbose >= 1 {
    rows.iter().map(|r| r.display_name.chars().count()).max().unwrap_or(0)
} else {
    0
};
```

(Use `chars().count()` not `.len()` — branch names can contain UTF-8 multibyte sequences and git's `align:N,left` counts display width. For full git-correct width including grapheme clusters we'd use `unicode-width`, but git itself counts bytes for ASCII names; matching `.chars().count()` is closer for ASCII and good enough for the parity rows.)

- [ ] **Step 3: Resolve abbrev width once.** Above the emit loop:

```rust
let hash_kind = repo.object_hash();
let abbrev_len = if options.no_abbrev {
    hash_kind.len_in_hex()
} else {
    options.abbrev.unwrap_or(7).min(hash_kind.len_in_hex())
};
```

- [ ] **Step 4: Add a subject extractor helper.** Above `pub fn list(...)`:

```rust
/// Extract the commit subject (first line of the message, trimmed).
/// Falls back to "" on non-commit tips or read errors — matches git's
/// `%(contents:subject)` which prints empty for non-commit refs.
fn commit_subject(repo: &gix::Repository, tip: gix::ObjectId) -> String {
    let Ok(commit) = repo.find_object(tip).and_then(|o| o.try_into_commit()) else {
        return String::new();
    };
    let Ok(message) = commit.message() else {
        return String::new();
    };
    message.summary().to_string()
}
```

- [ ] **Step 5: Replace the bare-name emit with verbose-aware rendering.** Replace both emit loops (locals + remotes) with one shared loop:

```rust
for row in &rows {
    if let Some(fmt) = options.format_string.as_deref() {
        // Existing --format path — use full_name + display_name.
        let line = expand_format(fmt, &row.full_name, &row.display_name);
        if !(options.omit_empty && line.is_empty()) {
            writeln!(out, "{line}")?;
        }
        continue;
    }
    let marker = if row.is_head { "* " } else { "  " };
    if options.verbose == 0 {
        writeln!(out, "{marker}{name}", name = row.display_name)?;
    } else {
        let short_sha = row.tip
            .map(|oid| oid.to_hex_with_len(abbrev_len).to_string())
            .unwrap_or_default();
        let subject = row.tip.map(|oid| commit_subject(&repo, oid)).unwrap_or_default();
        // -vv upstream insertion happens in Task 4.
        writeln!(
            out,
            "{marker}{name:<width$} {short_sha} {subject}",
            name = row.display_name,
            width = maxwidth,
        )?;
    }
}
```

- [ ] **Step 6: Flip the branch.sh `-v` row to bytes mode and run.** Edit `tests/journey/parity/branch.sh:172-180`:

```bash
title "gix branch --verbose"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    expect_parity bytes -- branch --verbose
  }
  it "matches git behavior — -vv" && {
    compat_effect "branch -vv upstream tracking rendering deferred" -- branch -vv
  }
)
```

Run:

```bash
bash tests/parity.sh tests/journey/parity/branch.sh 2>&1 | grep -E "(verbose|FAIL|PASS|compat)" | head -40
```

Expected: the `--verbose` row prints PASS in bytes mode (and shows no `[compat]` marker). `-vv` still compat-marked.

- [ ] **Step 7: Commit.**

```bash
git add gitoxide-core/src/repository/branch.rs tests/journey/parity/branch.sh
git commit -m "parity: git branch -v column-aligned name+sha+subject (bytes mode)"
```

---

### Task 3: Wire `--abbrev=N` / `--no-abbrev` (also covered by Task 2's `abbrev_len`)

**Files:**
- Modify: `tests/journey/parity/branch.sh:202-215` (flip both --abbrev rows to bytes)

- [ ] **Step 1: Verify Task 2's `abbrev_len` already honors `--abbrev`/`--no-abbrev`.** Read back the abbrev resolver from Task 2 Step 3 — it should already cover both flags. If `gix::ObjectId` doesn't expose `to_hex_with_len`, use `format!("{:.width$}", oid.to_hex(), width = abbrev_len)` (search `to_hex` in gix-hash for the actual API).

- [ ] **Step 2: Flip both --abbrev rows in branch.sh.** Replace `tests/journey/parity/branch.sh:207-215`:

```bash
title "gix branch --abbrev"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior — --abbrev=12" && {
    expect_parity bytes -- branch -v --abbrev=12
  }
  it "matches git behavior — --no-abbrev" && {
    expect_parity bytes -- branch -v --no-abbrev
  }
)
```

- [ ] **Step 3: Run and verify.**

```bash
bash tests/parity.sh tests/journey/parity/branch.sh 2>&1 | grep -E "(abbrev|FAIL)" | head -10
```

Expected: both abbrev rows green, no `[compat]` markers.

- [ ] **Step 4: Commit.**

```bash
git add tests/journey/parity/branch.sh
git commit -m "parity: git branch --abbrev=N / --no-abbrev with -v (bytes mode, 2 rows)"
```

---

### Task 4: Implement `-vv` upstream tracking annotation

**Files:**
- Modify: `gitoxide-core/src/repository/branch.rs` (add a `tracking_annotation` helper + use in the emit loop from Task 2)

- [ ] **Step 1: Add a tracking-annotation helper above `pub fn list`.**

```rust
/// Resolve the `-vv` upstream annotation for a local branch.
/// Returns `Some("[origin/main: ahead 1, behind 2]")` or `Some("[origin/main]")` (no divergence)
/// or `None` if no upstream is configured. Matches git's
/// `%(upstream:short)` + `%(upstream:track,nobracket)` format.
///
/// Remote branches return None — git prints no upstream column for them.
fn tracking_annotation(
    repo: &gix::Repository,
    full_ref_name: &str,
    tip: gix::ObjectId,
) -> Option<String> {
    use gix::refs::FullName;
    let full = FullName::try_from(full_ref_name).ok()?;
    if !full.as_ref().category().is_some_and(|c| c == gix::refs::Category::LocalBranch) {
        return None;
    }
    // Upstream short name via Repository::branch_remote_ref_name + remote-tracking resolution.
    let tracking = repo
        .branch_remote_tracking_ref_name(full.as_ref(), gix::remote::Direction::Fetch)?
        .ok()?;
    let tracking_short = tracking.shorten().to_string();
    let tracking_oid = repo.find_reference(tracking.as_ref()).ok()
        .and_then(|mut r| r.peel_to_id_in_place().ok())
        .map(|id| id.detach())?;
    // Ahead/behind via gix::revision::Walk (graph ancestor traversal).
    // ahead = commits in tip but not in tracking_oid
    // behind = commits in tracking_oid but not in tip
    let (ahead, behind) = ahead_behind(repo, tip, tracking_oid).unwrap_or((0, 0));
    Some(match (ahead, behind) {
        (0, 0) => format!("[{tracking_short}]"),
        (a, 0) => format!("[{tracking_short}: ahead {a}]"),
        (0, b) => format!("[{tracking_short}: behind {b}]"),
        (a, b) => format!("[{tracking_short}: ahead {a}, behind {b}]"),
    })
}

/// Compute (ahead, behind) for `tip` relative to `upstream`. Returns None on walk errors.
fn ahead_behind(
    repo: &gix::Repository,
    tip: gix::ObjectId,
    upstream: gix::ObjectId,
) -> Option<(usize, usize)> {
    // gix::revision::Walk supports left/right symmetric difference via two walks
    // and a HashSet diff. For small repos this is fine; for large ones a
    // merge-base walk is needed (deferred — the parity fixture is small).
    let to_set = |start: gix::ObjectId| -> Option<std::collections::HashSet<gix::ObjectId>> {
        let walk = repo.rev_walk([start]).all().ok()?;
        let mut set = std::collections::HashSet::new();
        for info in walk {
            let info = info.ok()?;
            set.insert(info.id);
        }
        Some(set)
    };
    let tip_set = to_set(tip)?;
    let up_set = to_set(upstream)?;
    let ahead = tip_set.difference(&up_set).count();
    let behind = up_set.difference(&tip_set).count();
    Some((ahead, behind))
}
```

(If `gix::revision::Walk` API differs — likely `repo.rev_walk(...).all()` — adjust at compile time. The intent: two ancestor sets, symmetric difference.)

- [ ] **Step 2: Insert tracking annotation between SHA and subject in the verbose emit.** Replace the `verbose >= 1` emit branch from Task 2 Step 5:

```rust
} else {
    let short_sha = row.tip
        .map(|oid| oid.to_hex_with_len(abbrev_len).to_string())
        .unwrap_or_default();
    let tracking = if options.verbose >= 2 {
        row.tip.and_then(|oid| tracking_annotation(&repo, &row.full_name, oid))
    } else {
        None
    };
    let subject = row.tip.map(|oid| commit_subject(&repo, oid)).unwrap_or_default();
    match tracking {
        Some(t) => writeln!(
            out,
            "{marker}{name:<width$} {short_sha} {t} {subject}",
            name = row.display_name, width = maxwidth,
        )?,
        None => writeln!(
            out,
            "{marker}{name:<width$} {short_sha} {subject}",
            name = row.display_name, width = maxwidth,
        )?,
    }
}
```

- [ ] **Step 3: Flip the -vv row to bytes mode.** Edit `tests/journey/parity/branch.sh:177-179`:

```bash
  it "matches git behavior — -vv" && {
    expect_parity bytes -- branch -vv
  }
```

- [ ] **Step 4: Run and verify.** The `small-repo-in-sandbox` fixture in this it-block has no upstream configured for any branch by default, so the `-vv` output should equal the `-v` output (no `[origin/...]` annotations). If git emits subtly different whitespace when no upstream exists, adjust the format string (git emits a single space after SHA, no extra; the trailing-space pattern from `align:N,left` matters).

```bash
bash tests/parity.sh tests/journey/parity/branch.sh 2>&1 | grep -E "(\-vv|FAIL)" | head -10
```

Expected: row green.

- [ ] **Step 5: Add a fixture variant with upstream configured (covers the actual `-vv` payload).** Add a new it-block immediately after the existing `-vv` it (still inside the same parens, sandbox already provides setup):

```bash
  it "matches git behavior — -vv with upstream" && {
    function _branch-vv-upstream-fixture() {
      git-init-hash-aware
      git checkout -b main >/dev/null 2>&1
      git config commit.gpgsign false
      git -c user.email=t@t -c user.name=t commit -q --allow-empty -m c1
      git remote add origin .
      git branch dev
      git update-ref refs/remotes/origin/main HEAD
      git branch --set-upstream-to=origin/main dev >/dev/null 2>&1
    }
    expect_parity_reset _branch-vv-upstream-fixture bytes -- branch -vv
  }
```

(Note: this it-block must be at the parens level that supports `expect_parity_reset` — see existing usages in branch.sh:362 for the pattern. Move it out of `small-repo-in-sandbox` into its own `(sandbox` block if needed.)

- [ ] **Step 6: Commit.**

```bash
git add gitoxide-core/src/repository/branch.rs tests/journey/parity/branch.sh
git commit -m "parity: git branch -vv upstream tracking annotation (bytes mode)"
```

---

### Task 5: Implement `--column=always` packing

**Files:**
- Create: `gitoxide-core/src/shared/columns.rs` (new file — small, reusable beyond branch)
- Modify: `gitoxide-core/src/shared.rs` or equivalent module index (add `pub mod columns;`)
- Modify: `gitoxide-core/src/repository/branch.rs` (call columns helper when `column == "always"` and `verbose == 0` and `format_string.is_none()`)
- Modify: `tests/journey/parity/branch.sh:303-311` (flip --column=always to bytes)

- [ ] **Step 1: Create the column-packing helper.** Write `gitoxide-core/src/shared/columns.rs`:

```rust
//! Column-packing helper matching git's `column.c` behavior for the
//! `column.ui = always` / `column=plain` defaults: lay out items into
//! a width-fitting grid filled column-major.

use std::io::Write;

/// Pack items into a column-major grid that fits within `terminal_width`.
/// Each column is padded to the widest item in that column + 2 spaces.
/// Empty input emits nothing. Single-item input emits the item + newline.
///
/// Matches git's `display_plain` / `display_columns` for `--column=column,plain`.
pub fn write_columns(
    out: &mut dyn Write,
    items: &[String],
    terminal_width: usize,
) -> std::io::Result<()> {
    if items.is_empty() {
        return Ok(());
    }
    let n = items.len();
    let pad = 2;

    // Determine number of columns: largest c such that, with column-major fill
    // (rows = ceil(n / c)), the sum of per-column widths fits terminal_width.
    let mut cols = 1usize;
    for c in (1..=n).rev() {
        let rows = (n + c - 1) / c;
        let mut total = 0usize;
        let mut fits = true;
        for col in 0..c {
            let start = col * rows;
            let end = (start + rows).min(n);
            let widest = items[start..end].iter().map(|s| s.chars().count()).max().unwrap_or(0);
            // No padding after last column.
            total += widest + if col + 1 == c { 0 } else { pad };
            if total > terminal_width {
                fits = false;
                break;
            }
        }
        if fits {
            cols = c;
            break;
        }
    }

    let rows = (n + cols - 1) / cols;
    // Precompute per-column widths.
    let widths: Vec<usize> = (0..cols).map(|col| {
        let start = col * rows;
        let end = (start + rows).min(n);
        items[start..end].iter().map(|s| s.chars().count()).max().unwrap_or(0)
    }).collect();

    for r in 0..rows {
        for c in 0..cols {
            let i = c * rows + r;
            if i >= n { continue; }
            let item = &items[i];
            let last = c + 1 == cols || (c + 1) * rows + r >= n;
            if last {
                writeln!(out, "{item}")?;
            } else {
                let pad_to = widths[c] + pad;
                let used = item.chars().count();
                write!(out, "{item}{:padding$}", "", padding = pad_to - used)?;
            }
        }
    }
    Ok(())
}
```

- [ ] **Step 2: Wire the module.** In `gitoxide-core/src/shared.rs` (or `lib.rs` if no `shared.rs`), add `pub mod columns;`. If a `shared/` directory doesn't exist yet, create `gitoxide-core/src/shared/mod.rs` with `pub mod columns;` and register in `lib.rs`.

- [ ] **Step 3: Wire `--column=always` in `list()`.** In `gitoxide-core/src/repository/branch.rs`, after rows are collected and sorted but before the emit loop, branch on column mode:

```rust
let column_always = options.column.as_deref().map(str::trim).unwrap_or("") == "always"
    || options.column.as_deref().map(|s| s.starts_with("always")).unwrap_or(false);
let column_off = options.no_column
    || options.column.as_deref().map(|s| s == "never").unwrap_or(false);

let use_columns = column_always
    && options.verbose == 0
    && options.format_string.is_none()
    && !column_off;

if use_columns {
    let lines: Vec<String> = rows.iter().map(|row| {
        let marker = if row.is_head { "* " } else { "  " };
        format!("{marker}{}", row.display_name)
    }).collect();
    let width = std::env::var("COLUMNS").ok().and_then(|s| s.parse::<usize>().ok()).unwrap_or(80);
    crate::shared::columns::write_columns(out, &lines, width)?;
    return Ok(());
}
```

- [ ] **Step 4: Flip the --column=always row to bytes.** Edit `tests/journey/parity/branch.sh:308-310`:

```bash
  it "matches git behavior — --column=always" && {
    COLUMNS=80 expect_parity bytes -- branch --column=always
  }
```

(Setting `COLUMNS=80` removes terminal-detection variance — both git and gix honor `$COLUMNS`.)

- [ ] **Step 5: Run and verify.**

```bash
bash tests/parity.sh tests/journey/parity/branch.sh 2>&1 | grep -E "(column|FAIL)" | head -10
```

Expected: --column row green. If git uses different padding (`column.c` adds the pad after the column, not between every cell), inspect both outputs in the failure message and adjust.

- [ ] **Step 6: Commit.**

```bash
git add gitoxide-core/src/shared/ gitoxide-core/src/lib.rs gitoxide-core/src/repository/branch.rs tests/journey/parity/branch.sh
git commit -m "parity: git branch --column=always packing (bytes mode)"
```

---

## Deliverable 2 — branch-section config writes + EDITOR helper

### Task 6: Add `Branch::DESCRIPTION` key + three `Repository` write methods

**Files:**
- Modify: `gix/src/config/tree/sections/branch.rs` (add DESCRIPTION key constant)
- Create: `gix/src/repository/config/branch_write.rs` (new file — keeps reads in `branch.rs`, writes in `branch_write.rs`)
- Modify: `gix/src/repository/config/mod.rs` (add `mod branch_write;`)

- [ ] **Step 1: Add `Branch::DESCRIPTION`.** Edit `gix/src/config/tree/sections/branch.rs`:

```rust
pub struct Branch {
    pub merge: keys::FullNameRef,
    pub remote: keys::RemoteName,
    pub push_remote: keys::RemoteName,
    /// The `branch.<name>.description` key — free-form prose set by `git branch --edit-description`.
    pub description: keys::Any,
}

impl Branch {
    pub const MERGE: keys::FullNameRef = ...;
    pub const REMOTE: keys::RemoteName = ...;
    pub const PUSH_REMOTE: keys::RemoteName = ...;
    pub const DESCRIPTION: keys::Any = keys::Any::new("description", &CONFIG_TREE_BRANCH);
}
```

(Match the style of existing keys in the file; use whichever `keys::*` flavor matches "free string, no validation" — likely `keys::Any` or `keys::String`. Inspect the file first.)

- [ ] **Step 2: Create `gix/src/repository/config/branch_write.rs` with three methods.**

```rust
use crate::bstr::BStr;
use crate::config::tree::sections::Branch;

/// Errors writing branch-section config.
#[derive(Debug, thiserror::Error)]
#[allow(missing_docs)]
pub enum Error {
    #[error("failed to mutate the in-memory config")]
    Set(#[from] crate::config::set::Error),
    #[error("failed to commit the config snapshot")]
    Commit(#[from] crate::config::overrides::Error),
    #[error("the upstream ref name is not a valid full ref name")]
    InvalidUpstream(#[source] gix_validate::reference::name::Error),
    #[error("the branch '{0}' has no upstream information")]
    NoUpstream(String),
}

impl crate::Repository {
    /// Set `branch.<short_name>.remote` and `branch.<short_name>.merge` to track `upstream_full_ref`.
    /// `upstream_full_ref` must be a fully-qualified ref name, e.g. `refs/remotes/origin/main` (remote)
    /// or `refs/heads/main` (local — git uses `.` as the remote name in that case).
    /// Mirrors `vendor/git/branch.c:install_branch_config_multiple_remotes`.
    pub fn set_branch_upstream(
        &mut self,
        short_name: &BStr,
        upstream_full_ref: &gix_ref::FullNameRef,
    ) -> Result<(), Error> {
        // Determine remote name: if the ref is under refs/remotes/<remote>/..., use <remote>;
        // if under refs/heads/..., use ".".
        let (remote_name, merge_ref): (&BStr, gix_ref::FullName) =
            if let Some(rest) = upstream_full_ref.as_bstr().strip_prefix(b"refs/remotes/") {
                let slash = rest.iter().position(|&b| b == b'/').unwrap_or(rest.len());
                let remote = &rest[..slash];
                let local_branch = &rest[slash + 1..];
                let merge: gix_ref::FullName = format!("refs/heads/{}", String::from_utf8_lossy(local_branch))
                    .try_into()
                    .map_err(Error::InvalidUpstream)?;
                (remote.into(), merge)
            } else {
                (b".".into(), upstream_full_ref.to_owned())
            };

        let mut snap = self.config_snapshot_mut();
        snap.set_subsection_value(&Branch::REMOTE, short_name, remote_name)?;
        snap.set_subsection_value(&Branch::MERGE, short_name, merge_ref.as_bstr())?;
        snap.commit()?;
        Ok(())
    }

    /// Clear `branch.<short_name>.{remote,merge}`. Returns Error::NoUpstream if neither key existed
    /// (mirrors git's exit-128 "Branch '<n>' has no upstream information").
    pub fn unset_branch_upstream(&mut self, short_name: &BStr) -> Result<(), Error> {
        let had_remote = self.config_snapshot().string_by("branch", Some(short_name), Branch::REMOTE.name).is_some();
        let had_merge = self.config_snapshot().string_by("branch", Some(short_name), Branch::MERGE.name).is_some();
        if !had_remote && !had_merge {
            return Err(Error::NoUpstream(short_name.to_string()));
        }
        let mut snap = self.config_snapshot_mut();
        snap.clear_subsection_value(&Branch::REMOTE, short_name);
        snap.clear_subsection_value(&Branch::MERGE, short_name);
        snap.commit()?;
        Ok(())
    }

    /// Set or clear `branch.<short_name>.description`. Empty bytes → clear.
    pub fn set_branch_description(&mut self, short_name: &BStr, value: &BStr) -> Result<(), Error> {
        let mut snap = self.config_snapshot_mut();
        if value.is_empty() {
            snap.clear_subsection_value(&Branch::DESCRIPTION, short_name);
        } else {
            snap.set_subsection_value(&Branch::DESCRIPTION, short_name, value)?;
        }
        snap.commit()?;
        Ok(())
    }
}
```

(If `SnapshotMut` does not have a `clear_subsection_value` — search `gix/src/config/snapshot/access.rs` — add it with the same signature returning `Result<Option<BString>, Error>` that calls `config.remove_raw_value_by(...)`. The lower-level `gix-config` `File::remove_raw_value` exists; just plumb it through.)

- [ ] **Step 3: Wire the module.** Edit `gix/src/repository/config/mod.rs`:

```rust
mod branch;
mod branch_write;
pub use branch_write::Error as BranchWriteError;
```

- [ ] **Step 4: Add a unit test in `gix/tests/...`.** Write `gix/tests/config/branch_write.rs` (or extend an existing config test file):

```rust
#[test]
fn set_branch_upstream_writes_remote_and_merge() -> gix_testtools::Result {
    let (mut repo, _tmp) = crate::repo_with_one_commit()?;
    let upstream: gix::refs::FullName = "refs/heads/main".try_into()?;
    repo.set_branch_upstream(b"dev".into(), upstream.as_ref())?;
    let snap = repo.config_snapshot();
    assert_eq!(snap.string_by("branch", Some("dev".into()), "remote").as_deref(), Some(b".".as_ref().into()));
    assert_eq!(snap.string_by("branch", Some("dev".into()), "merge").as_deref(), Some(b"refs/heads/main".as_ref().into()));
    Ok(())
}
```

(Adjust to match `gix/tests`'s actual test fixture conventions — search for an existing `config_snapshot_mut` test as template.)

- [ ] **Step 5: Build + test.**

```bash
cargo check -p gix
cargo test -p gix --test config -- branch_write 2>&1 | tail -20
```

Expected: clean check, the new test passes.

- [ ] **Step 6: Commit.**

```bash
git add gix/src/config/tree/sections/branch.rs gix/src/repository/config/ gix/tests/
git commit -m "gix: add Repository::{set,unset}_branch_upstream + set_branch_description"
```

---

### Task 7: Wire `-u` / `--set-upstream-to`

**Files:**
- Modify: `gitoxide-core/src/repository/branch.rs` (add `set_upstream_to(repo, branch, upstream)` function)
- Modify: `src/plumbing/main.rs:988-994` (replace stub with real dispatch)
- Modify: `tests/journey/parity/branch.sh:489-509` (flip both -u rows to bytes; remove `[compat]` echos)

- [ ] **Step 1: Implement in gitoxide-core.** Add to `gitoxide-core/src/repository/branch.rs`:

```rust
/// `git branch -u <upstream> [<branch>]`. <branch> defaults to the current branch when None.
/// Resolves <upstream> via DWIM (rev-parse style) to a full ref, then calls
/// Repository::set_branch_upstream and emits git's confirmation message.
pub fn set_upstream_to(
    mut repo: gix::Repository,
    branch_short: Option<&BStr>,
    upstream: &BStr,
    out: &mut dyn std::io::Write,
    err: &mut dyn std::io::Write,
) -> anyhow::Result<()> {
    use gix::bstr::ByteSlice;

    // Determine target branch: explicit arg or HEAD's short name. Detached HEAD → 128.
    let target = match branch_short {
        Some(b) => b.to_owned(),
        None => match repo.head_name()? {
            Some(name) => name.shorten().to_owned(),
            None => {
                writeln!(err, "fatal: HEAD not pointing to a branch")?;
                std::process::exit(128);
            }
        },
    };

    // DWIM: try refs/heads/<x>, refs/remotes/<x>, refs/<x> in that order (mirrors git's
    // dwim_ref). For first cut, accept anything rev_parse_single can resolve to a ref.
    let upstream_ref = match repo.find_reference(upstream.as_bstr()) {
        Ok(r) => r.name().to_owned(),
        Err(_) => {
            // Try refs/heads/<upstream> and refs/remotes/<upstream>.
            let candidates = [
                format!("refs/heads/{}", upstream.to_str_lossy()),
                format!("refs/remotes/{}", upstream.to_str_lossy()),
            ];
            let resolved = candidates.iter().find_map(|c| {
                repo.find_reference(c.as_bytes().as_bstr()).ok().map(|r| r.name().to_owned())
            });
            match resolved {
                Some(n) => n,
                None => {
                    writeln!(err, "error: the requested upstream branch '{}' does not exist", upstream.to_str_lossy())?;
                    std::process::exit(1);
                }
            }
        }
    };

    repo.set_branch_upstream(target.as_ref(), upstream_ref.as_ref())?;

    // git's success message:
    //   "branch '<short>' set up to track '<upstream-short>'."
    // Use upstream's short form (strip refs/heads/ or refs/remotes/).
    let upstream_short = upstream_ref.shorten().to_string();
    let target_short = target.to_str_lossy();
    writeln!(out, "branch '{target_short}' set up to track '{upstream_short}'.")?;
    Ok(())
}
```

(Verify the exact message format against `vendor/git/branch.c:install_branch_config_multiple_remotes` — search for `"set up to track"`. Match git's wording byte-for-byte; it varies by `branch.<n>.rebase` setting and remote-vs-local target.)

- [ ] **Step 2: Replace the stub in `src/plumbing/main.rs:988-994`.**

```rust
} else if is_set_upstream {
    let upstream = set_upstream_to.expect("guarded by is_set_upstream");
    // First positional, if any, is the target branch; else current.
    let target = args.into_iter().next();
    prepare_and_run(
        "branch-set-upstream-to",
        trace, verbose, progress, progress_keep_open,
        None,
        move |_progress, out, err| {
            let target_bstr = target.as_ref()
                .map(|s| gix::path::os_str_into_bstr(s))
                .transpose()?;
            core::repository::branch::set_upstream_to(
                repository(Mode::Lenient)?,
                target_bstr,
                upstream.as_bytes().as_bstr(),
                out, err,
            )
        },
    )
} else if is_unset_upstream || is_edit_description {
    Ok(())  // still stubbed — Tasks 8 + 11 close these
}
```

- [ ] **Step 3: Flip the -u rows to bytes mode + drop `[compat]` echos.** Edit `tests/journey/parity/branch.sh:489-509`:

```bash
title "gix branch --set-upstream-to"
only_for_hash sha1-only && (sandbox
  function _branch-up-fixture() {
    git-init-hash-aware
    git checkout -b main >/dev/null 2>&1
    git config commit.gpgsign false
    git -c user.email=t@t -c user.name=t commit -q --allow-empty -m c1
    git branch dev
  }
  it "matches git behavior — -u" && {
    expect_parity_reset _branch-up-fixture bytes -- branch -u main dev
  }
  it "matches git behavior — --set-upstream-to" && {
    expect_parity_reset _branch-up-fixture bytes -- branch --set-upstream-to=main dev
  }
)
```

- [ ] **Step 4: Run + verify.**

```bash
bash tests/parity.sh tests/journey/parity/branch.sh 2>&1 | grep -E "(set-upstream|FAIL)" | head -20
```

Expected: both rows green. Likely first run will fail on the success-message wording — read both outputs in the diff and adjust the format string in `set_upstream_to` to match git exactly. Note rebase-tracking config can change git's wording; `branch.autoSetupRebase` defaults to `never`, so the simple form should match.

- [ ] **Step 5: Commit.**

```bash
git add gitoxide-core/src/repository/branch.rs src/plumbing/main.rs tests/journey/parity/branch.sh
git commit -m "parity: git branch -u / --set-upstream-to (bytes mode, 2 rows)"
```

---

### Task 8: Wire `--unset-upstream`

**Files:**
- Modify: `gitoxide-core/src/repository/branch.rs` (add `unset_upstream(repo, branch)`)
- Modify: `src/plumbing/main.rs` (add the dispatch arm)
- Modify: `tests/journey/parity/branch.sh:511-529` (flip to bytes)

- [ ] **Step 1: Implement.**

```rust
pub fn unset_upstream(
    mut repo: gix::Repository,
    branch_short: Option<&BStr>,
    err: &mut dyn std::io::Write,
) -> anyhow::Result<()> {
    let target = match branch_short {
        Some(b) => b.to_owned(),
        None => match repo.head_name()? {
            Some(name) => name.shorten().to_owned(),
            None => {
                writeln!(err, "fatal: HEAD not pointing to a branch")?;
                std::process::exit(128);
            }
        },
    };
    match repo.unset_branch_upstream(target.as_ref()) {
        Ok(()) => Ok(()),
        Err(gix::config::BranchWriteError::NoUpstream(name)) => {
            writeln!(err, "fatal: Branch '{name}' has no upstream information")?;
            std::process::exit(128);
        }
        Err(e) => Err(e.into()),
    }
}
```

(Verify exact wording in git: search `vendor/git/builtin/branch.c` for `"has no upstream information"` and match byte-for-byte.)

- [ ] **Step 2: Dispatch in main.rs.** Replace `is_unset_upstream` branch:

```rust
} else if is_unset_upstream {
    let target = args.into_iter().next();
    prepare_and_run(
        "branch-unset-upstream",
        trace, verbose, progress, progress_keep_open, None,
        move |_progress, _out, err| {
            let target_bstr = target.as_ref()
                .map(|s| gix::path::os_str_into_bstr(s)).transpose()?;
            core::repository::branch::unset_upstream(
                repository(Mode::Lenient)?, target_bstr, err,
            )
        },
    )
}
```

- [ ] **Step 3: Flip to bytes.** Edit `tests/journey/parity/branch.sh:515-529`, replace the `compat` echo with `expect_parity_reset _branch-unset-fixture bytes` (the existing fixture is fine).

Add a no-upstream error case as a sibling it-block:

```bash
  it "matches git behavior — no upstream error" && {
    function _branch-no-upstream-fixture() {
      git-init-hash-aware
      git checkout -b main >/dev/null 2>&1
      git config commit.gpgsign false
      git -c user.email=t@t -c user.name=t commit -q --allow-empty -m c1
    }
    expect_parity_reset _branch-no-upstream-fixture bytes -- branch --unset-upstream main
  }
```

- [ ] **Step 4: Run + commit.**

```bash
bash tests/parity.sh tests/journey/parity/branch.sh 2>&1 | grep -E "(unset|FAIL)" | head -10
git add gitoxide-core/src/repository/branch.rs src/plumbing/main.rs tests/journey/parity/branch.sh
git commit -m "parity: git branch --unset-upstream (bytes mode)"
```

---

### Task 9: Wire `--track` / `--no-track` config write side

**Files:**
- Modify: `gitoxide-core/src/repository/branch.rs` (extend create path)
- Modify: `src/plumbing/main.rs` (un-stub `track`/`no_track` destructure, pass to create)
- Modify: `tests/journey/parity/branch.sh:423-448` (flip the --track row)

- [ ] **Step 1: Find the create function.** Search `gitoxide-core/src/repository/branch.rs` for `pub fn create`. Extend its signature with `track: Option<&BStr>, no_track: bool`.

- [ ] **Step 2: After the ref is written, write tracking config.** At the end of `create()` after the RefEdit succeeds:

```rust
// --no-track wins over --track and over branch.autoSetupMerge config.
// For first cut we don't honor branch.autoSetupMerge — git default is
// "true" which auto-sets-up tracking when start_point is a remote
// branch; gix can defer that until a separate row.
if !no_track {
    if let Some(upstream_arg) = track {
        // --track=<mode> resolves the start-point's full ref name then
        // calls set_branch_upstream. For --track without arg (modes:
        // direct/inherit), the start-point itself is the upstream.
        let upstream_resolved = repo.find_reference(upstream_arg.as_bstr()).ok()
            .map(|r| r.name().to_owned());
        if let Some(name) = upstream_resolved {
            repo.set_branch_upstream(new_branch_short.as_ref(), name.as_ref())?;
        }
    }
}
```

(Adjust to match actual variable names in `create()`. The `--track` value comes from clap as `Option<String>` with `default_missing_value = "direct"` — bare `--track` produces `Some("direct")`. The `direct` / `inherit` mode distinction is git's plumbing for whether to copy upstream from the start-point's existing upstream; for first cut, treat `Some(_)` as "set start-point as upstream".)

- [ ] **Step 3: Un-stub `track` / `no_track` in main.rs:913-914.** Pass them through to the create dispatch.

- [ ] **Step 4: Flip the --track row in branch.sh:423-448 to bytes.** The existing fixture sets up `git branch dev origin/main --track`-style assertions; verify gix now writes `branch.dev.{remote,merge}` matching git. Drop the `[compat]` echo.

- [ ] **Step 5: Run + commit.**

```bash
bash tests/parity.sh tests/journey/parity/branch.sh 2>&1 | grep -E "(track|FAIL)" | head -20
git add gitoxide-core/src/repository/branch.rs src/plumbing/main.rs tests/journey/parity/branch.sh
git commit -m "parity: git branch --track upstream-config write (bytes mode)"
```

---

### Task 10: Add `editor::edit_file` helper in gitoxide-core

**Files:**
- Create: `gitoxide-core/src/shared/editor.rs` (or `gitoxide-core/src/editor.rs` if no shared/ yet)
- Modify: module index for the new file

- [ ] **Step 1: Implement `edit_file`.** Write `gitoxide-core/src/shared/editor.rs`:

```rust
//! Shell out to `$GIT_EDITOR` / `core.editor` / `$VISUAL` / `$EDITOR` / `vi`,
//! presenting `initial` as the file contents and returning the post-edit bytes.
//! Mirrors git's `launch_editor()` in `editor.c`.

use anyhow::Context;
use std::io::{Read, Write};

/// Resolve the editor command following git's precedence:
/// `$GIT_EDITOR` → `core.editor` → `$VISUAL` → `$EDITOR` → `vi`.
fn resolve_editor(repo: &gix::Repository) -> String {
    if let Ok(v) = std::env::var("GIT_EDITOR") {
        if !v.is_empty() { return v; }
    }
    if let Some(v) = repo.config_snapshot().string("core.editor") {
        if !v.is_empty() { return v.to_string(); }
    }
    if let Ok(v) = std::env::var("VISUAL") {
        if !v.is_empty() { return v; }
    }
    if let Ok(v) = std::env::var("EDITOR") {
        if !v.is_empty() { return v; }
    }
    "vi".to_string()
}

/// Open a temp file populated with `initial`, spawn the editor, return edited bytes.
/// `prefix` is used in the temp file name (e.g. "BRANCH_DESCRIPTION").
pub fn edit_file(repo: &gix::Repository, initial: &[u8], prefix: &str) -> anyhow::Result<Vec<u8>> {
    let editor = resolve_editor(repo);

    // Create a temp file in $GIT_DIR/.<prefix>~<rand> so it lives next to the repo
    // (matches git's behavior of using `git_path()`).
    let git_dir = repo.git_dir().to_owned();
    let temp_name = format!(".{prefix}~{}", std::process::id());
    let temp_path = git_dir.join(&temp_name);
    std::fs::write(&temp_path, initial).with_context(|| format!("writing temp file {}", temp_path.display()))?;

    // Spawn $editor "$temp_path" via the shell (git does this — `core.editor` may contain args).
    let status = gix::command::prepare(&editor)
        .arg(&temp_path)
        .with_shell()
        .spawn()
        .with_context(|| format!("spawning editor: {editor}"))?
        .wait()
        .with_context(|| format!("waiting for editor: {editor}"))?;

    if !status.success() {
        let _ = std::fs::remove_file(&temp_path);
        anyhow::bail!("editor '{editor}' exited with {status}");
    }

    let edited = std::fs::read(&temp_path).with_context(|| format!("reading edited file {}", temp_path.display()))?;
    let _ = std::fs::remove_file(&temp_path);
    Ok(edited)
}
```

- [ ] **Step 2: Wire the module.** Add `pub mod editor;` to `gitoxide-core/src/shared/mod.rs` (created in Task 5) or extend the lib root.

- [ ] **Step 3: Build.**

```bash
cargo check -p gitoxide-core
```

Expected: clean. If `gix::command::prepare` is gated behind a feature (`command-line` or similar), enable it in `gitoxide-core/Cargo.toml`.

- [ ] **Step 4: Commit.**

```bash
git add gitoxide-core/src/shared/editor.rs gitoxide-core/src/shared/mod.rs
git commit -m "gitoxide-core: add editor::edit_file helper"
```

---

### Task 11: Wire `--edit-description` to editor + description writer

**Files:**
- Modify: `gitoxide-core/src/repository/branch.rs` (add `edit_description(repo, branch_short)`)
- Modify: `src/plumbing/main.rs` (split `is_edit_description` from the stub)
- Modify: `tests/journey/parity/branch.sh:531-540` (flip to bytes)

- [ ] **Step 1: Implement.**

```rust
pub fn edit_description(
    mut repo: gix::Repository,
    branch_short: Option<&BStr>,
    err: &mut dyn std::io::Write,
) -> anyhow::Result<()> {
    let target = match branch_short {
        Some(b) => b.to_owned(),
        None => match repo.head_name()? {
            Some(name) => name.shorten().to_owned(),
            None => {
                writeln!(err, "fatal: HEAD not pointing to a branch")?;
                std::process::exit(128);
            }
        },
    };
    let target_str = target.to_str_lossy();

    // Read existing description (if any) to seed the temp file.
    let initial: Vec<u8> = repo.config_snapshot()
        .string_by("branch", Some(target.as_ref()), "description")
        .map(|s| s.into_owned().into())
        .unwrap_or_default();

    let edited = crate::shared::editor::edit_file(&repo, &initial, "BRANCH_DESCRIPTION")?;

    // git strips trailing newlines and treats "all whitespace" as empty (clears the key).
    let trimmed: &[u8] = {
        let mut end = edited.len();
        while end > 0 && (edited[end - 1] == b'\n' || edited[end - 1] == b'\r' || edited[end - 1] == b' ') {
            end -= 1;
        }
        &edited[..end]
    };

    repo.set_branch_description(target.as_ref(), trimmed.into())?;
    Ok(())
}
```

- [ ] **Step 2: Dispatch in main.rs.** Split `is_edit_description` from the remaining stub:

```rust
} else if is_edit_description {
    let target = args.into_iter().next();
    prepare_and_run(
        "branch-edit-description",
        trace, verbose, progress, progress_keep_open, None,
        move |_progress, _out, err| {
            let target_bstr = target.as_ref()
                .map(|s| gix::path::os_str_into_bstr(s)).transpose()?;
            core::repository::branch::edit_description(
                repository(Mode::Lenient)?, target_bstr, err,
            )
        },
    )
}
```

- [ ] **Step 3: Flip the row in branch.sh:534-540.** With `EDITOR=true` (no-op editor), gix should now: read empty description, spawn `true /tmp/...BRANCH_DESCRIPTION`, read back empty bytes, clear the (already-empty) key. Net effect = no config change. git does the same. Bytes match.

```bash
title "gix branch --edit-description"
only_for_hash sha1-only && (small-repo-in-sandbox
  it "matches git behavior" && {
    EDITOR=true expect_parity bytes -- branch --edit-description
  }
)
```

Add a non-empty edit case:

```bash
  it "matches git behavior — content edit" && {
    function _edit-fixture() {
      git-init-hash-aware
      git checkout -b main >/dev/null 2>&1
      git config commit.gpgsign false
      git -c user.email=t@t -c user.name=t commit -q --allow-empty -m c1
    }
    # EDITOR script that writes a fixed string. Git invokes it with the temp file as $1.
    EDITOR='sh -c "echo my-description > \"$1\""' expect_parity_reset _edit-fixture bytes -- branch --edit-description
  }
```

- [ ] **Step 4: Run + commit.**

```bash
bash tests/parity.sh tests/journey/parity/branch.sh 2>&1 | grep -E "(edit-description|FAIL)" | head -10
git add gitoxide-core/src/repository/branch.rs src/plumbing/main.rs tests/journey/parity/branch.sh
git commit -m "parity: git branch --edit-description (bytes mode)"
```

---

## Closeout

### Task 12: Re-run full branch.sh + update commands.md notes

**Files:**
- Modify: `docs/parity/commands.md:36` (the `branch` row's Notes column)

- [ ] **Step 1: Run the full branch parity file.**

```bash
bash tests/parity.sh tests/journey/parity/branch.sh 2>&1 | tail -40
```

Expected: all green, no `[compat]` markers for the 9 rows touched by Tasks 2/3/4/5/7/8/9/11. If anything regressed (e.g. existing `--column=never` row broken by Task 5's column gate), fix in place before the commit.

- [ ] **Step 2: Run the full status parity file (Deliverable 1 should not regress it, but verify).**

```bash
bash tests/parity.sh tests/journey/parity/status.sh 2>&1 | tail -10
```

Expected: same green count as before this sprint. (The renderer is opt-in via `verbose >= 1`, so status's bare path is unaffected; status's deferred -v rows remain compat — they live in status.sh's prose comments and would close in a follow-up that wires verbose into status's renderer too. Leave that for a future sprint.)

- [ ] **Step 3: Update commands.md branch-row notes.** Edit `docs/parity/commands.md:36`. Strike through the now-closed compat-effect rows in the Notes column. The remaining compat rows after this sprint are: none in the `-v`/`-vv`/`--abbrev`/`--column=always`/`-u`/`--unset-upstream`/`--track`/`--edit-description` cluster. Update the line to reflect the new state (essentially: drop the "Compat-effect rows tracked in SHORTCOMINGS.md: …" sentence entirely or pare it to just `--create-reflog` if that's still deferred).

- [ ] **Step 4: Run shortcomings regenerator.**

```bash
bash etc/parity/shortcomings.sh
git diff docs/parity/SHORTCOMINGS.md | head -40
```

Expected: SHORTCOMINGS.md loses the closed branch rows.

- [ ] **Step 5: Commit closeout.**

```bash
git add docs/parity/commands.md docs/parity/SHORTCOMINGS.md
git commit -m "parity: close branch -v/-vv/--abbrev/--column/upstream/--edit-description cluster"
```

- [ ] **Step 6: Steward gate (manual).** Hand off to `@gix-steward` with the prompt:

> Verify completion of the foundations sprint: branch.sh has 0 compat_effect rows in `-v`/`-vv`/`--abbrev`/`--column=always`/`-u`/`--set-upstream-to`/`--unset-upstream`/`--track`/`--edit-description`. Cross-check against vendor/git/Documentation/git-branch.txt and confirm no flag from those clusters was silently dropped.

If steward returns PASS, the sprint is closed. If REJECT-WITH-ROW, address the called-out gap and re-gate.

---

## Self-review

**Spec coverage check:**
- ✅ `-v` rendering — Task 2
- ✅ `-vv` upstream tracking — Task 4
- ✅ `--abbrev=N` / `--no-abbrev` — Task 3 (covered by Task 2's resolver, flipped in Task 3)
- ✅ `--column=always` packing — Task 5
- ✅ `-u` / `--set-upstream-to` config write — Tasks 6 + 7
- ✅ `--unset-upstream` config clear — Task 8
- ✅ `--track` config-write side — Task 9
- ✅ `--edit-description` EDITOR + write — Tasks 10 + 11
- ✅ Pre-pay for `git push --set-upstream` — `Repository::set_branch_upstream` from Task 6 is the consumer.
- ✅ Pre-pay for `git commit -e` — `editor::edit_file` from Task 10 is reusable.
- ✅ Ledger sweep — Task 12

**Type consistency check:** All three new `Repository` methods use `&BStr` short-name + `&FullNameRef`/`&BStr` content; CLI bridges via `gix::path::os_str_into_bstr`. The `BranchWriteError` is exported from `gix::config::BranchWriteError` for downstream consumers in `gitoxide-core`.

**Status -v rows note:** Status's deferred `-v`/`--column` rows are *not* in scope for this sprint — they live as prose in `status.sh` (no `compat_effect` markers exist) and would need their own renderer wiring inside `gitoxide-core::repository::status`. The `shared::columns` helper from Task 5 is reusable when that sprint lands; this is the "pre-payment" the user described, not a same-sprint flip.

---

**Plan complete. Next: pick execution mode.**

- **Subagent-driven** (recommended for this size — 12 tasks, mostly independent, each task has clear verification): dispatch `gix-architect` per task, two-stage review.
- **Inline**: execute via `superpowers:executing-plans` with checkpoints between tasks.
