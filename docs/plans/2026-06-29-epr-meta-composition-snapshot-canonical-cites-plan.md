---
id: epr-meta-composition-snapshot-plan
---

# EPR-Meta Composition Snapshot — Canonical Cites & Parity Slice — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Give brit a generic, content-addressed **cite verdict engine** + the `EprMeta`/`NodeSeed` composition envelope + a `brit epr-meta status` verb, proven at **semantic parity** with the parent monorepo's cite oracle on a controlled fixture corpus — the first usable slice of the Snapshot discipline.

**Architecture:** Lift `EprMeta`/`NodeSeed` and a new `InterfaceRef` edge + frontmatter/cite/verdict engine into the **generic** `brit-epr::engine` (the elohim covenant vocabulary stays behind feature `elohim-protocol`). A doc's drift is a `sha256:hex16` **fingerprint** (non-address) over its frontmatter-excluded canonical body; "current" is the live filesystem (no head pointer — that is Layer-2). The parity harness imports the parent `_lib.cite_graph` and compares verdict labels.

**Tech Stack:** Rust (workspace edition), `cid 0.11`, `multihash-codetable 0.2` (sha2), `serde_ipld_dagcbor 0.6`, `hex 0.4`, `serde 1`, `thiserror 2`; Python 3 (the parent oracle, subprocess-invoked in one test).

## Global Constraints

- **Build env (this container, verbatim):** `export RUSTFLAGS=""` (the inherited WASM `getrandom` flag breaks native linking) and `export CARGO_TARGET_DIR=/tmp/brit-target` (the `/projects` volume trips cargo fingerprint ENOENT). Use **plain `cargo test`**, NOT `cargo nextest` (nextest is unavailable in this container despite the justfile).
- **Run tests from** `/projects/elohim/elohim/brit`.
- **Crate attrs:** `brit-epr` is `#![deny(missing_docs, rust_2018_idioms)]`, `#![forbid(unsafe_code)]` — every new `pub` item needs a `///` doc comment. Workspace clippy is `pedantic = warn`.
- **CID format (verbatim):** CIDv1, dag-cbor `0x71` (nodes) / raw `0x55` (blobs), sha2-256 `0x12`, base32 (`bafyrei…`/`bafkrei…`).
- **Drift fingerprint (verbatim):** `"sha256:" + hex(sha256(canonical_body))[..16]` — a **non-address fingerprint**, never a CID; matches the oracle's recipe (`cite_graph.fingerprint`).
- **Engine boundary:** new composition types live in `brit-epr::engine` (generic, multiformats only); disabling feature `elohim-protocol` must still build. The covenant vocabulary stays gated.
- **deny.toml** gates `[sources]` to crates.io + the two Nexus registries — add no new dep that resolves elsewhere. This plan adds **no** new dependency.
- **Commits:** end every commit message with `Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>`. Branch: `brit-dev` (commit-only; the operator integrates). Selective-add only the files you changed (the tree carries another session's WIP).
- **Source of truth:** `docs/specs/2026-06-29-epr-meta-composition-snapshot-canonical-cites-design.md`.

---

## Sub-slice A — foundations + conformance

### Task A1: Derive `Ord` on `BritCid`

**Files:** Modify `brit-epr/src/engine/cid.rs:21`.

**Interfaces:**
- Produces: `BritCid: PartialOrd + Ord` so `Vec<BritCid>` and `Vec<InterfaceRef>` can `.sort()` deterministically (inner `cid::Cid` is `Ord`).

- [ ] **Step 1: Write the failing test** — append to the `#[cfg(test)] mod tests` in `cid.rs`:
```rust
#[test]
fn brit_cids_sort_deterministically() {
    let mut v = vec![BritCid::compute(&[2]), BritCid::compute(&[1])];
    v.sort();
    assert!(v[0] <= v[1]);
}
```
- [ ] **Step 2: Run to verify it fails** — `RUSTFLAGS="" CARGO_TARGET_DIR=/tmp/brit-target cargo test -p brit-epr cid::tests::brit_cids_sort` → FAIL: `BritCid: Ord is not satisfied`.
- [ ] **Step 3: Add the derives** — `cid.rs:21`, change the derive to:
```rust
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
```
- [ ] **Step 4: Run to verify it passes** — same command → PASS.
- [ ] **Step 5: Commit**
```bash
git add brit-epr/src/engine/cid.rs
git commit -m "feat(brit-epr): derive Ord on BritCid for deterministic node ordering

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

### Task A2: Frontmatter splitter + canonical body + drift fingerprint (generic engine)

**Files:** Create `brit-epr/src/engine/frontmatter.rs`; Modify `brit-epr/src/engine/mod.rs` (declare + re-export).

**Interfaces:**
- Produces:
  - `pub fn split_frontmatter(content: &str) -> (Option<&str>, &str)` — returns `(frontmatter_yaml, body)`; `None` frontmatter when the content does not open with a `---` line.
  - `pub fn canonical_body(content: &str) -> String` — the frontmatter-excluded body, `.trim()`-ed (the bytes the drift fingerprint hashes).
  - `pub fn drift_fingerprint(content: &str) -> String` — `"sha256:" + hex(sha256(canonical_body))[..16]`.

- [ ] **Step 1: Write the failing test** — in `frontmatter.rs`:
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_yaml_frontmatter() {
        let c = "---\nid: foo\n---\nbody text\n";
        let (fm, body) = split_frontmatter(c);
        assert_eq!(fm, Some("id: foo\n"));
        assert_eq!(body, "body text\n");
    }

    #[test]
    fn no_frontmatter_is_whole_body() {
        let c = "# Title\nno frontmatter here\n";
        let (fm, body) = split_frontmatter(c);
        assert_eq!(fm, None);
        assert_eq!(body, c);
    }

    #[test]
    fn canonical_body_excludes_frontmatter_and_trims() {
        let c = "---\nid: foo\n---\n\n  body  \n\n";
        assert_eq!(canonical_body(c), "body");
    }

    #[test]
    fn drift_is_sha256_prefixed_16_hex() {
        let f = drift_fingerprint("---\nid: x\n---\nhello\n");
        assert!(f.starts_with("sha256:"));
        assert_eq!(f.len(), "sha256:".len() + 16);
    }

    #[test]
    fn frontmatter_edit_does_not_change_drift() {
        let a = "---\nid: foo\n---\nstable body\n";
        let b = "---\nid: foo\ncites:\n  - x | d | sha256:0000000000000000\n---\nstable body\n";
        assert_eq!(drift_fingerprint(a), drift_fingerprint(b));
    }
}
```
- [ ] **Step 2: Run to verify it fails** — `RUSTFLAGS="" CARGO_TARGET_DIR=/tmp/brit-target cargo test -p brit-epr frontmatter::tests` → FAIL: module not found.
- [ ] **Step 3: Implement `frontmatter.rs`**
```rust
//! Frontmatter splitting + the canonical-body drift fingerprint.
//!
//! Generic engine surface: a doc's drift identity is a `sha256:hex16`
//! fingerprint (NOT a content address) over its frontmatter-excluded,
//! trimmed body — byte-matching the parent cite oracle's recipe so a
//! metadata/cites edit never trips staleness.

use multihash_codetable::{Code, MultihashDigest};

/// Split a document into `(frontmatter_yaml, body)`. Frontmatter is the block
/// between a leading `---` line and the next `---` line. Returns `None`
/// frontmatter when the content does not open with `---`.
pub fn split_frontmatter(content: &str) -> (Option<&str>, &str) {
    let rest = match content.strip_prefix("---\n") {
        Some(r) => r,
        None => return (None, content),
    };
    // Find the closing delimiter line `---\n` (or trailing `---`).
    if let Some(end) = rest.find("\n---\n") {
        let fm = &rest[..=end]; // include the trailing newline of the fm block
        let body = &rest[end + "\n---\n".len()..];
        (Some(fm), body)
    } else if let Some(stripped) = rest.strip_suffix("\n---") {
        (Some(stripped), "")
    } else {
        // Unterminated frontmatter: treat the whole thing as body (no fm).
        (None, content)
    }
}

/// The frontmatter-excluded, trimmed body — the bytes the drift fingerprint
/// hashes.
pub fn canonical_body(content: &str) -> String {
    let (_, body) = split_frontmatter(content);
    body.trim().to_string()
}

/// `"sha256:" + first-16-hex of sha256(canonical_body)` — a non-address
/// fingerprint that byte-matches the parent oracle.
pub fn drift_fingerprint(content: &str) -> String {
    let body = canonical_body(content);
    let mh = Code::Sha2_256.digest(body.as_bytes());
    let hex = hex::encode(mh.digest());
    format!("sha256:{}", &hex[..16])
}
```
- [ ] **Step 4: Wire into `engine/mod.rs`** — add `pub mod frontmatter;` to the module list and `pub use frontmatter::{canonical_body, drift_fingerprint, split_frontmatter};` to the re-exports.
- [ ] **Step 5: Run to verify it passes** — `RUSTFLAGS="" CARGO_TARGET_DIR=/tmp/brit-target cargo test -p brit-epr frontmatter::tests` → PASS.
- [ ] **Step 6: Commit**
```bash
git add brit-epr/src/engine/frontmatter.rs brit-epr/src/engine/mod.rs
git commit -m "feat(brit-epr): frontmatter split + canonical-body drift fingerprint

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

### Task A3: Body-extraction + fingerprint conformance vs the oracle (the first parity gate)

**Files:** Create `brit-epr/tests/canonical_body_conformance.rs`; Create `brit-epr/tests/fixtures/conformance/` (the input docs + expected fingerprints, generated from the oracle).

**Interfaces:**
- Consumes: `brit_epr::engine::frontmatter::drift_fingerprint` (Task A2), the parent oracle `cite_graph.fingerprint` (read-only, subprocess).
- The oracle path is `/projects/elohim/.claude/scripts` (importable as `_lib.cite_graph`); skip the test when absent (standalone brit checkout).

- [ ] **Step 1: Generate the expected vectors from the oracle.** Create `brit-epr/tests/fixtures/conformance/gen.py` (run once, by hand, to write `expected.json`):
```python
#!/usr/bin/env python3
import json, os, sys
sys.path.insert(0, "/projects/elohim/.claude/scripts")
from _lib.cite_graph import fingerprint  # the parent recipe
CASES = {
  "plain.md": "# Title\n\nbody line one\n\nbody line two\n",
  "with_fm.md": "---\nid: sample\ncites:\n  - x | d | sha256:0000000000000000\n---\n# Title\n\nthe real body\n",
  "trailing_ws.md": "---\nid: t\n---\n\n   spaced body   \n\n",
  "no_trailing_newline.md": "---\nid: n\n---\nno newline body",
  "unicode.md": "---\nid: u\n---\ncafé — built\n",
}
out = {}
for name, text in CASES.items():
    path = os.path.join(os.path.dirname(__file__), name)
    with open(path, "w") as f: f.write(text)
    # the oracle fingerprints a file's frontmatter-excluded body:
    from _lib.frontmatter import parse_file
    body = parse_file(path).body
    out[name] = fingerprint(body)   # "sha256:hex16"
with open(os.path.join(os.path.dirname(__file__), "expected.json"), "w") as f:
    json.dump(out, f, indent=2, sort_keys=True)
print(json.dumps(out, indent=2))
```
Run: `python3 brit-epr/tests/fixtures/conformance/gen.py` — commit the generated `*.md` + `expected.json` as **test data** (the portable spec).
- [ ] **Step 2: Write the failing test** — `brit-epr/tests/canonical_body_conformance.rs`:
```rust
//! brit's canonical-body fingerprint MUST reproduce the parent oracle's
//! `cite_graph.fingerprint` byte-for-byte. The vectors are the portable spec.
use std::fs;
use std::path::Path;

#[test]
fn drift_fingerprint_matches_oracle_vectors() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/conformance");
    let expected: std::collections::BTreeMap<String, String> =
        serde_json::from_str(&fs::read_to_string(dir.join("expected.json")).unwrap()).unwrap();
    for (name, want) in &expected {
        let content = fs::read_to_string(dir.join(name)).unwrap();
        let got = brit_epr::engine::frontmatter::drift_fingerprint(&content);
        assert_eq!(&got, want, "drift mismatch for {name}");
    }
}
```
- [ ] **Step 3: Run to verify** — `RUSTFLAGS="" CARGO_TARGET_DIR=/tmp/brit-target cargo test -p brit-epr --test canonical_body_conformance`. If a vector fails, the divergence is the frontmatter boundary or Python `str.strip()` vs Rust `trim()` on an exotic char — read `/projects/elohim/.claude/scripts/_lib/frontmatter.py` and adjust `split_frontmatter`/`canonical_body` until all vectors pass. Re-run until green.
- [ ] **Step 4: Commit**
```bash
git add brit-epr/tests/canonical_body_conformance.rs brit-epr/tests/fixtures/conformance/
git commit -m "test(brit-epr): canonical-body drift fingerprint conformance vs parent oracle

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Sub-slice B — the verdict engine (generic)

### Task B1: `InterfaceRef` + cite-line parse/serialize

**Files:** Create `brit-epr/src/engine/interface_ref.rs`; Modify `brit-epr/src/engine/mod.rs`.

**Interfaces:**
- Produces:
  - `EdgeKind { DocCite, Content, SchemaVersion, Capability, Contract, Legacy, External }` and `EdgeRole { Import, Export }` (both serde `rename_all = "kebab-case"`, `Ord`).
  - `InterfaceRef { kind: EdgeKind, role: EdgeRole, ref_: String, cid: Option<BritCid>, drift: Option<String>, desc: Option<String> }` (serde `rename_all = "camelCase"`, field `ref_` → `"ref"`).
  - `pub fn parse_cite_line(line: &str) -> InterfaceRef` — the pipe-delimited grammar `slug | desc | fingerprint [| status: …] [| path: …]` → a `DocCite` import (a no-pipe segment is a legacy path-string → `Legacy`).

- [ ] **Step 1: Write the failing test** — in `interface_ref.rs`:
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_envelope_cite() {
        let r = parse_cite_line("constitution | the law | sha256:1eb96af782012fc6 | path: a/b.md");
        assert_eq!(r.kind, EdgeKind::DocCite);
        assert_eq!(r.role, EdgeRole::Import);
        assert_eq!(r.ref_, "constitution");
        assert_eq!(r.desc.as_deref(), Some("the law"));
        assert_eq!(r.drift.as_deref(), Some("sha256:1eb96af782012fc6"));
    }

    #[test]
    fn no_pipe_segment_is_legacy() {
        let r = parse_cite_line("genesis/docs/foo.md");
        assert_eq!(r.kind, EdgeKind::Legacy);
        assert_eq!(r.ref_, "genesis/docs/foo.md");
        assert!(r.drift.is_none());
    }
}
```
- [ ] **Step 2: Run to verify it fails** — `cargo test -p brit-epr interface_ref::tests` (with the env) → FAIL.
- [ ] **Step 3: Implement `interface_ref.rs`** — the type + `parse_cite_line` (segments split on `" | "`; classify positionally: a `sha256:`/`bafy` token → `drift`; `status:`/`path:` keyed; first remaining → `desc`):
```rust
//! `InterfaceRef` — one typed import/export edge of the composition envelope.
//! Generic: `kind: doc-cite` is the only populated kind in this slice.

use serde::{Deserialize, Serialize};
use crate::engine::cid::BritCid;

/// The typed interface kind an edge carries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum EdgeKind {
    /// A citation between docs (this slice).
    DocCite,
    /// (deferred) addressable content.
    Content,
    /// (deferred) a schema version.
    SchemaVersion,
    /// (deferred) a capability.
    Capability,
    /// (deferred) a constitutional governance contract.
    Contract,
    /// A legacy path-string cite to an id-bearing target.
    Legacy,
    /// A cross-repo target outside this snapshot.
    External,
}

/// Whether an edge is an import (a need) or an export (a provision).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum EdgeRole {
    /// This node needs the target.
    Import,
    /// This node provides the target.
    Export,
}

/// One typed import/export edge.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InterfaceRef {
    /// The typed interface kind.
    pub kind: EdgeKind,
    /// Import or export.
    pub role: EdgeRole,
    /// Stable identity — the target's `id`/slug (move-survivable).
    #[serde(rename = "ref")]
    pub ref_: String,
    /// The addressable content version, when one exists.
    pub cid: Option<BritCid>,
    /// `sha256:hex16` non-address drift fingerprint (doc-cite).
    pub drift: Option<String>,
    /// Directional relationship hint (imports only).
    pub desc: Option<String>,
}

fn is_fingerprint(s: &str) -> bool {
    s.starts_with("sha256:") || s.starts_with("bafy")
}

/// Parse one frontmatter `cites:` line into a `DocCite` import (or `Legacy`).
pub fn parse_cite_line(line: &str) -> InterfaceRef {
    let line = line.split(" # ").next().unwrap_or(line).trim();
    let segments: Vec<&str> = line.split(" | ").map(str::trim).collect();
    if segments.len() == 1 {
        return InterfaceRef {
            kind: EdgeKind::Legacy, role: EdgeRole::Import,
            ref_: segments[0].to_string(), cid: None, drift: None, desc: None,
        };
    }
    let ref_ = segments[0].to_string();
    let (mut drift, mut desc) = (None, None);
    for seg in &segments[1..] {
        if is_fingerprint(seg) {
            drift = Some((*seg).to_string());
        } else if seg.starts_with("status:") || seg.starts_with("path:") {
            // health/locator hints — not load-bearing for identity.
        } else if desc.is_none() {
            desc = Some((*seg).to_string());
        }
    }
    InterfaceRef {
        kind: EdgeKind::DocCite, role: EdgeRole::Import, ref_, cid: None, drift, desc,
    }
}
```
- [ ] **Step 4: Wire into `engine/mod.rs`** — `pub mod interface_ref;` + `pub use interface_ref::{EdgeKind, EdgeRole, InterfaceRef, parse_cite_line};`.
- [ ] **Step 5: Run to verify it passes** — `cargo test -p brit-epr interface_ref::tests` → PASS.
- [ ] **Step 6: Commit**
```bash
git add brit-epr/src/engine/interface_ref.rs brit-epr/src/engine/mod.rs
git commit -m "feat(brit-epr): InterfaceRef edge + cite-line parser (generic engine)

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

### Task B2: Frontmatter `id`/`cites` extraction + the slug index

**Files:** Create `brit-epr/src/engine/cite.rs`; Modify `brit-epr/src/engine/mod.rs`.

**Interfaces:**
- Consumes: `split_frontmatter` (A2), `parse_cite_line` (B1).
- Produces:
  - `pub fn extract_id(frontmatter: &str) -> Option<String>` — the `id:` scalar.
  - `pub fn extract_cites(frontmatter: &str) -> Vec<InterfaceRef>` — the `cites:` list as import edges.
  - `pub struct SlugIndex(BTreeMap<String, PathBuf>)` with `pub fn build(roots: &[PathBuf]) -> std::io::Result<SlugIndex>` (walk `*.md` under each root, map `id:` → path) and `pub fn resolve(&self, slug: &str) -> Option<&Path>`.

- [ ] **Step 1: Write the failing test** — in `cite.rs`:
```rust
#[cfg(test)]
mod tests {
    use super::*;

    const FM: &str = "id: my-doc\ncites:\n  - alpha | a desc | sha256:00aa11bb22cc33dd\n  - beta | b | sha256:44ee55ff66007788\n";

    #[test]
    fn extracts_id() {
        assert_eq!(extract_id(FM).as_deref(), Some("my-doc"));
    }

    #[test]
    fn extracts_two_cites() {
        let cites = extract_cites(FM);
        assert_eq!(cites.len(), 2);
        assert_eq!(cites[0].ref_, "alpha");
        assert_eq!(cites[1].drift.as_deref(), Some("sha256:44ee55ff66007788"));
    }

    #[test]
    fn slug_index_resolves_ids() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("a.md"), "---\nid: doc-a\n---\nbody\n").unwrap();
        let idx = SlugIndex::build(&[tmp.path().to_path_buf()]).unwrap();
        assert!(idx.resolve("doc-a").is_some());
        assert!(idx.resolve("missing").is_none());
    }
}
```
- [ ] **Step 2: Run to verify it fails** — `cargo test -p brit-epr cite::tests` (env) → FAIL.
- [ ] **Step 3: Implement `cite.rs`** — minimal YAML reads (no dep): `id:` is a line `id: <value>`; `cites:` is a block of `  - <line>` items until the next non-indented key:
```rust
//! `id`/`cites` frontmatter extraction + the move-survivable slug index.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::engine::frontmatter::split_frontmatter;
use crate::engine::interface_ref::{parse_cite_line, InterfaceRef};

/// Read the `id:` scalar from a frontmatter block.
pub fn extract_id(frontmatter: &str) -> Option<String> {
    frontmatter.lines().find_map(|l| {
        l.strip_prefix("id:").map(|v| v.trim().to_string())
    }).filter(|s| !s.is_empty())
}

/// Read the `cites:` list (lines `  - …`) as import edges.
pub fn extract_cites(frontmatter: &str) -> Vec<InterfaceRef> {
    let mut out = Vec::new();
    let mut in_cites = false;
    for line in frontmatter.lines() {
        if line.trim_end() == "cites:" {
            in_cites = true;
            continue;
        }
        if in_cites {
            let t = line.trim_start();
            if let Some(item) = t.strip_prefix("- ") {
                out.push(parse_cite_line(item));
            } else if !line.starts_with(char::is_whitespace) && !line.trim().is_empty() {
                break; // next top-level key ends the block
            }
        }
    }
    out
}

/// Slug → path map over a doc corpus. The slug (a doc's `id:`) is identity;
/// the path is a disposable cache, so a move self-heals on rebuild.
pub struct SlugIndex(BTreeMap<String, PathBuf>);

impl SlugIndex {
    /// Walk every `*.md` under `roots`, mapping `id:` → path.
    pub fn build(roots: &[PathBuf]) -> std::io::Result<Self> {
        let mut map = BTreeMap::new();
        for root in roots {
            walk(root, &mut |path, content| {
                if let (Some(fm), _) = split_frontmatter(content) {
                    if let Some(id) = extract_id(fm) {
                        map.entry(id).or_insert_with(|| path.to_path_buf());
                    }
                }
            })?;
        }
        Ok(Self(map))
    }

    /// Resolve a slug to its current path, if any.
    pub fn resolve(&self, slug: &str) -> Option<&Path> {
        self.0.get(slug).map(PathBuf::as_path)
    }
}

fn walk(dir: &Path, f: &mut dyn FnMut(&Path, &str)) -> std::io::Result<()> {
    if !dir.exists() { return Ok(()); }
    for entry in std::fs::read_dir(dir)? {
        let path = entry?.path();
        if path.is_dir() {
            walk(&path, f)?;
        } else if path.extension().is_some_and(|e| e == "md") {
            if let Ok(content) = std::fs::read_to_string(&path) {
                f(&path, &content);
            }
        }
    }
    Ok(())
}
```
- [ ] **Step 4: Wire into `engine/mod.rs`** — `pub mod cite;` + `pub use cite::{extract_cites, extract_id, SlugIndex};`. Add `tempfile` to `brit-epr`'s `[dev-dependencies]` is already present (confirmed).
- [ ] **Step 5: Run to verify it passes** — `cargo test -p brit-epr cite::tests` → PASS.
- [ ] **Step 6: Commit**
```bash
git add brit-epr/src/engine/cite.rs brit-epr/src/engine/mod.rs
git commit -m "feat(brit-epr): id/cites frontmatter extraction + slug index

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

### Task B3: The verdict engine (`envelope_verdict`-equivalent)

**Files:** Create `brit-epr/src/engine/verdict.rs`; Modify `brit-epr/src/engine/mod.rs`.

**Interfaces:**
- Consumes: `SlugIndex` (B2), `drift_fingerprint` (A2), `InterfaceRef` (B1).
- Produces:
  - `enum Verdict { Ok, Held, Stale, Dead }` (serde `rename_all = "kebab-case"`).
  - `pub fn verdict(edge: &InterfaceRef, idx: &SlugIndex) -> Verdict` — precedence **`dead > held > stale > ok`**: not in index → `Dead`; path contains a `held/` segment → `Held`; recomputed target drift ≠ `edge.drift` → `Stale`; else `Ok`. A `Legacy` edge is always `Ok`.

- [ ] **Step 1: Write the failing test** — in `verdict.rs`:
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::interface_ref::{EdgeKind, EdgeRole, InterfaceRef};
    use crate::engine::{cite::SlugIndex, frontmatter::drift_fingerprint};

    fn cite(reff: &str, drift: Option<&str>) -> InterfaceRef {
        InterfaceRef { kind: EdgeKind::DocCite, role: EdgeRole::Import,
            ref_: reff.into(), cid: None, drift: drift.map(String::from), desc: None }
    }

    #[test]
    fn ok_held_stale_dead() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("t.md"), "---\nid: target\n---\nbody\n").unwrap();
        std::fs::create_dir_all(tmp.path().join("held")).unwrap();
        std::fs::write(tmp.path().join("held/h.md"), "---\nid: held-doc\n---\nb\n").unwrap();
        let idx = SlugIndex::build(&[tmp.path().to_path_buf()]).unwrap();
        let target_fp = drift_fingerprint("---\nid: target\n---\nbody\n");

        assert_eq!(verdict(&cite("target", Some(&target_fp)), &idx), Verdict::Ok);
        assert_eq!(verdict(&cite("target", Some("sha256:0000000000000000")), &idx), Verdict::Stale);
        assert_eq!(verdict(&cite("held-doc", Some("sha256:0000000000000000")), &idx), Verdict::Held);
        assert_eq!(verdict(&cite("nope", Some("sha256:0000000000000000")), &idx), Verdict::Dead);
    }
}
```
- [ ] **Step 2: Run to verify it fails** — `cargo test -p brit-epr verdict::tests` (env) → FAIL.
- [ ] **Step 3: Implement `verdict.rs`**
```rust
//! The cite verdict engine — `envelope_verdict`-equivalent, parity with the
//! parent oracle. "Current" is the live filesystem (no head pointer; that is
//! Layer-2). Precedence: dead > held > stale > ok.

use serde::{Deserialize, Serialize};

use crate::engine::cite::SlugIndex;
use crate::engine::frontmatter::drift_fingerprint;
use crate::engine::interface_ref::{EdgeKind, InterfaceRef};

/// The health of one cite edge.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Verdict {
    /// Resolves, in the live tree, body unchanged.
    Ok,
    /// Resolves but the target is sequestered under `held/`.
    Held,
    /// Resolves but the target body drifted since the pin.
    Stale,
    /// The slug resolves nowhere.
    Dead,
}

/// Compute the verdict for one edge against the slug index + filesystem.
pub fn verdict(edge: &InterfaceRef, idx: &SlugIndex) -> Verdict {
    if edge.kind == EdgeKind::Legacy {
        return Verdict::Ok;
    }
    let Some(path) = idx.resolve(&edge.ref_) else {
        return Verdict::Dead;
    };
    if path.components().any(|c| c.as_os_str() == "held") {
        return Verdict::Held;
    }
    let current = std::fs::read_to_string(path).unwrap_or_default();
    if edge.drift.as_deref() != Some(drift_fingerprint(&current).as_str()) {
        return Verdict::Stale;
    }
    Verdict::Ok
}
```
- [ ] **Step 4: Wire into `engine/mod.rs`** — `pub mod verdict;` + `pub use verdict::{verdict, Verdict};`.
- [ ] **Step 5: Run to verify it passes** — `cargo test -p brit-epr verdict::tests` → PASS.
- [ ] **Step 6: Full gate + commit**
```bash
RUSTFLAGS="" CARGO_TARGET_DIR=/tmp/brit-target cargo clippy -p brit-epr --all-targets -- -D warnings -A unknown-lints --no-deps
git add brit-epr/src/engine/verdict.rs brit-epr/src/engine/mod.rs
git commit -m "feat(brit-epr): cite verdict engine (dead>held>stale>ok, filesystem-current)

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Sub-slice C — composition envelope, verbs, parity, dogfood, hook

### Task C1: Lift `EprMeta`/`NodeSeed` into the generic engine + extend with imports/exports

**Files:** Create `brit-epr/src/engine/meta.rs` (moved from `elohim/meta.rs`); Create `brit-epr/src/engine/node_seed.rs` (moved from `elohim/node_seed.rs`); Modify `brit-epr/src/engine/mod.rs`, `brit-epr/src/elohim/mod.rs`, `brit-epr/src/lib.rs`.

**Interfaces:**
- Produces (generic, unconditional):
  - `MetaEntry { path: String, cid: BritCid }` (unchanged).
  - `EprMeta { epr_meta_version: u32, subtree: String, entries: Vec<MetaEntry>, imports: Vec<InterfaceRef>, exports: Vec<InterfaceRef> }` — `imports`/`exports` carry `#[serde(default)]`, NO `skip_serializing_if` (stable encoding, Decision A). `content_type() == "brit.epr-meta"`.
  - `NodeSeed { epr_meta_version, repo, epr_metas: Vec<BritCid>, sub_seeds: Vec<BritCid>, imports, exports }` — same serde discipline. `content_type() == "brit.node-seed"`.

- [ ] **Step 1: Move the files** — `git mv brit-epr/src/elohim/meta.rs brit-epr/src/engine/meta.rs` and `git mv brit-epr/src/elohim/node_seed.rs brit-epr/src/engine/node_seed.rs`. In each moved file change `use crate::engine::cid::BritCid;`/`use crate::engine::content_node::ContentNode;` to the now-sibling paths (`use super::cid::BritCid; use super::content_node::ContentNode;` or keep `crate::engine::…`). Their inline `#[cfg(test)] mod tests` move with them.
- [ ] **Step 2: Write the failing test** — add to `engine/meta.rs` tests:
```rust
#[test]
fn epr_meta_carries_imports_exports_with_stable_default() {
    use crate::engine::interface_ref::{EdgeKind, EdgeRole, InterfaceRef};
    let m = EprMeta {
        epr_meta_version: 1, subtree: "docs".into(), entries: vec![],
        imports: vec![InterfaceRef { kind: EdgeKind::DocCite, role: EdgeRole::Import,
            ref_: "x".into(), cid: None, drift: Some("sha256:0000000000000000".into()), desc: None }],
        exports: vec![],
    };
    assert_eq!(m.content_type(), "brit.epr-meta");
    // round-trips through dag-cbor:
    let bytes = m.canonical_bytes().unwrap();
    let back: EprMeta = serde_ipld_dagcbor::from_slice(&bytes).unwrap();
    assert_eq!(m, back);
}
```
- [ ] **Step 3: Run to verify it fails** — `cargo test -p brit-epr meta::tests::epr_meta_carries` (env) → FAIL (fields absent).
- [ ] **Step 4: Extend the structs** — in `engine/meta.rs`, add to `EprMeta`:
```rust
    /// Typed import edges (cites). Stable empty encoding (Decision A).
    #[serde(default)]
    pub imports: Vec<crate::engine::interface_ref::InterfaceRef>,
    /// Typed export edges (provided ids). Stable empty encoding.
    #[serde(default)]
    pub exports: Vec<crate::engine::interface_ref::InterfaceRef>,
```
and in `engine/node_seed.rs`, add `#[serde(default)] pub sub_seeds: Vec<BritCid>`, `#[serde(default)] pub imports: …`, `#[serde(default)] pub exports: …`. Update the moved tests' struct literals to include the new fields (`..` is not allowed on a struct literal — list them).
- [ ] **Step 5: Re-home the re-exports** — `engine/mod.rs`: add `pub mod meta; pub mod node_seed;` + `pub use meta::{EprMeta, MetaEntry}; pub use node_seed::NodeSeed;`. `elohim/mod.rs`: remove `pub mod meta; pub mod node_seed;` and their re-export lines. `lib.rs`: move `EprMeta, MetaEntry, NodeSeed` from the `#[cfg(feature = "elohim-protocol")]` re-export block (lib.rs:29-33) into the unconditional `pub use engine::{…}` block (lib.rs:34-37).
- [ ] **Step 6: Run the whole brit-epr + brit-build-ref suites** — `RUSTFLAGS="" CARGO_TARGET_DIR=/tmp/brit-target cargo test -p brit-epr -p brit-build-ref`. The existing `meta_seal`/`meta_verify` tests must stay green (the symbol `brit_epr::EprMeta` is unchanged; only its module moved). Confirm with `--no-default-features`: `RUSTFLAGS="" CARGO_TARGET_DIR=/tmp/brit-target cargo build -p brit-epr --no-default-features` builds (EprMeta is now generic).
- [ ] **Step 7: Commit**
```bash
git add brit-epr/src/engine/meta.rs brit-epr/src/engine/node_seed.rs brit-epr/src/engine/mod.rs brit-epr/src/elohim/mod.rs brit-epr/src/lib.rs
git commit -m "refactor(brit-epr): lift EprMeta/NodeSeed into generic engine + imports/exports/sub_seeds

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

### Task C2: `seal` projects cites into `EprMeta.imports`/`exports`

**Files:** Modify `brit-build-ref/src/meta_cmd.rs:11` (`seal`).

**Interfaces:**
- Consumes: `split_frontmatter`, `extract_id`, `extract_cites`, `drift_fingerprint`, `EdgeKind`/`EdgeRole`/`InterfaceRef` (engine).
- Produces: `seal` now, for every `*.md` immediate file, projects its `id:` into an `Export` edge (drift = its own canonical-body fingerprint) and its `cites:` into `Import` edges; both sorted by `(role, kind, ref)`; stored on the `EprMeta`.

- [ ] **Step 1: Write the failing test** — `brit-build-ref/tests/meta_seal_cites.rs`:
```rust
use std::process::Command;

#[test]
fn seal_projects_cites_into_eprmeta() {
    let tmp = tempfile::tempdir().unwrap();
    Command::new("git").arg("init").arg(tmp.path()).output().unwrap();
    let docs = tmp.path().join("docs");
    std::fs::create_dir_all(&docs).unwrap();
    std::fs::write(docs.join("a.md"),
        "---\nid: doc-a\ncites:\n  - doc-b | needs b | sha256:0011223344556677\n---\n# A\nbody\n").unwrap();

    let out = Command::new(env!("CARGO_BIN_EXE_brit-build-ref"))
        .arg("--repo").arg(tmp.path())
        .arg("meta").arg("seal").arg("--dir").arg(&docs)
        .output().unwrap();
    assert!(out.status.success(), "stderr: {}", String::from_utf8_lossy(&out.stderr));
    let cid = String::from_utf8(out.stdout).unwrap();
    // re-read the stored node and assert it carries the edges:
    let node_path = tmp.path().join(".git/brit/objects").join(cid.trim());
    let bytes = std::fs::read(node_path).unwrap();
    let meta: brit_epr::EprMeta = serde_ipld_dagcbor::from_slice(&bytes).unwrap();
    assert!(meta.exports.iter().any(|e| e.ref_ == "doc-a"));
    assert!(meta.imports.iter().any(|e| e.ref_ == "doc-b"));
}
```
- [ ] **Step 2: Run to verify it fails** — `cargo test -p brit-build-ref --test meta_seal_cites` (env) → FAIL (edges empty).
- [ ] **Step 3: Extend `seal`** — after building `entries`, before constructing `EprMeta`, project edges from each `*.md`:
```rust
    use brit_epr::engine::{cite, frontmatter, interface_ref::{EdgeKind, EdgeRole, InterfaceRef}};
    let mut imports = Vec::new();
    let mut exports = Vec::new();
    for path in &files {
        if path.extension().is_none_or(|e| e != "md") { continue; }
        let content = std::fs::read_to_string(path)?;
        let (fm, _) = frontmatter::split_frontmatter(&content);
        if let Some(fm) = fm {
            if let Some(id) = cite::extract_id(fm) {
                exports.push(InterfaceRef { kind: EdgeKind::DocCite, role: EdgeRole::Export,
                    ref_: id, cid: None, drift: Some(frontmatter::drift_fingerprint(&content)), desc: None });
            }
            for mut c in cite::extract_cites(fm) { c.role = EdgeRole::Import; imports.push(c); }
        }
    }
    imports.sort();
    exports.sort();
    let meta = EprMeta { epr_meta_version: 1, subtree, entries, imports, exports };
```
(`InterfaceRef: Ord` from B1 makes `.sort()` the `(role, kind, ref, …)` field order.)
- [ ] **Step 4: Run to verify it passes** — `cargo test -p brit-build-ref --test meta_seal_cites` → PASS.
- [ ] **Step 5: Commit**
```bash
git add brit-build-ref/src/meta_cmd.rs brit-build-ref/tests/meta_seal_cites.rs
git commit -m "feat(brit-build-ref): seal projects cites into EprMeta imports/exports

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

### Task C3: `brit epr-meta status` verb

**Files:** Modify `brit-build-ref/src/meta_cmd.rs` (add `status`); Modify `brit-build-ref/src/main.rs` (add `Status` to `MetaCmd` + dispatch).

**Interfaces:**
- Consumes: `SlugIndex`, `verdict`, `split_frontmatter`, `extract_cites` (engine).
- Produces: `pub fn status(repo: &Path, dir: &Path) -> anyhow::Result<()>` — builds a slug index over `dir`, and for each `*.md` prints `"<verdict> <slug>: <doc> -> <cited-ref>"` per cite; exits 0 (advisory).

- [ ] **Step 1: Write the failing test** — `brit-build-ref/tests/meta_status.rs`:
```rust
use std::process::Command;

#[test]
fn status_reports_dead_and_ok() {
    let tmp = tempfile::tempdir().unwrap();
    Command::new("git").arg("init").arg(tmp.path()).output().unwrap();
    let d = tmp.path().join("docs");
    std::fs::create_dir_all(&d).unwrap();
    std::fs::write(d.join("b.md"), "---\nid: doc-b\n---\nB body\n").unwrap();
    // a.md cites doc-b (ok-able once we pin its real drift) and a missing slug (dead):
    let bfp = {
        let c = std::fs::read_to_string(d.join("b.md")).unwrap();
        brit_epr::engine::frontmatter::drift_fingerprint(&c)
    };
    std::fs::write(d.join("a.md"), format!(
        "---\nid: doc-a\ncites:\n  - doc-b | b | {bfp}\n  - ghost | g | sha256:0000000000000000\n---\nA\n")).unwrap();

    let out = Command::new(env!("CARGO_BIN_EXE_brit-build-ref"))
        .arg("--repo").arg(tmp.path())
        .arg("meta").arg("status").arg("--dir").arg(&d)
        .output().unwrap();
    let s = String::from_utf8(out.stdout).unwrap();
    assert!(s.contains("dead") && s.contains("ghost"), "got: {s}");
    assert!(s.contains("ok") && s.contains("doc-b"), "got: {s}");
}
```
- [ ] **Step 2: Run to verify it fails** — `cargo test -p brit-build-ref --test meta_status` (env) → FAIL (no `status` subcommand).
- [ ] **Step 3: Implement `status`** in `meta_cmd.rs`:
```rust
use brit_epr::engine::{cite::SlugIndex, frontmatter, verdict::verdict};

/// Print the cite verdict of every doc-cite under `dir` (advisory; exit 0).
pub fn status(_repo: &Path, dir: &Path) -> anyhow::Result<()> {
    let idx = SlugIndex::build(&[dir.to_path_buf()])?;
    let mut files: Vec<_> = walk_md(dir)?;
    files.sort();
    for path in &files {
        let content = std::fs::read_to_string(path)?;
        let (fm, _) = frontmatter::split_frontmatter(&content);
        let Some(fm) = fm else { continue };
        let slug = brit_epr::engine::cite::extract_id(fm).unwrap_or_default();
        for edge in brit_epr::engine::cite::extract_cites(fm) {
            let v = format!("{:?}", verdict(&edge, &idx)).to_lowercase();
            println!("{v} {slug}: {} -> {}", path.display(), edge.ref_);
        }
    }
    Ok(())
}

fn walk_md(dir: &Path) -> anyhow::Result<Vec<std::path::PathBuf>> {
    let mut out = Vec::new();
    for e in std::fs::read_dir(dir)? {
        let p = e?.path();
        if p.is_dir() { out.extend(walk_md(&p)?); }
        else if p.extension().is_some_and(|x| x == "md") { out.push(p); }
    }
    Ok(out)
}
```
- [ ] **Step 4: Wire the verb** — `main.rs`: add to `MetaCmd` a `Status { #[arg(long)] dir: PathBuf }` variant, and to the Meta dispatch arm `MetaCmd::Status { dir } => meta_cmd::status(&repo, &dir),`.
- [ ] **Step 5: Run to verify it passes** — `cargo test -p brit-build-ref --test meta_status` → PASS.
- [ ] **Step 6: Commit**
```bash
git add brit-build-ref/src/meta_cmd.rs brit-build-ref/src/main.rs brit-build-ref/tests/meta_status.rs
git commit -m "feat(brit-build-ref): brit epr-meta status — advisory cite verdict report

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

### Task C4: Fixture corpus + the parity harness (golden gate)

**Files:** Create `brit-epr/tests/fixtures/cite_corpus/**` (the controlled corpus: `ok`, `stale`, `dead`, `held/` cases); Create `brit-epr/tests/fixtures/cite_corpus/oracle.py` (the parent-`_lib` driver); Create `brit-epr/tests/cite_parity.rs`.

**Interfaces:**
- Consumes: `SlugIndex` + `verdict` (engine); the parent `_lib.cite_graph` (subprocess, skip when absent).

- [ ] **Step 1: Build the fixture corpus.** Under `brit-epr/tests/fixtures/cite_corpus/`: `target.md` (`id: target`), `held/sequestered.md` (`id: held-doc`), and `citer.md` whose `cites:` carries one `ok` (pinned to `target`'s real drift — compute it once with a throwaway `drift_fingerprint` and paste), one `stale` (pinned to `target` with a wrong `sha256:0000…`), one `held` (cites `held-doc`), one `dead` (cites `ghost`). Every doc has a matching `id:` so the oracle's slug index resolves it.
- [ ] **Step 2: Write the oracle driver** `oracle.py`:
```python
#!/usr/bin/env python3
"""Emit {citer_id|cited_ref: verdict} using the PARENT cite engine. Skips (exit 3) if absent."""
import json, os, sys
ORACLE = "/projects/elohim/.claude/scripts"
if not os.path.isdir(ORACLE): sys.exit(3)
sys.path.insert(0, ORACLE)
from _lib.cite_graph import build_slug_index, parse_cite, envelope_verdict
from _lib.frontmatter import parse_file
root = sys.argv[1]
idx = build_slug_index([root])
out = {}
for dirpath, _, files in os.walk(root):
    for fn in files:
        if not fn.endswith(".md"): continue
        p = os.path.join(dirpath, fn)
        fmeta = parse_file(p)
        for raw in (fmeta.get("cites") or []):
            cite = parse_cite(raw)
            out[f"{fmeta.get('id')}|{cite.ref}"] = envelope_verdict(cite, idx, root)
print(json.dumps(out, sort_keys=True))
```
(Adjust the `parse_cite`/`envelope_verdict` call arity to the real signatures in `/projects/elohim/.claude/scripts/_lib/cite_graph.py` — read them; the harness is correct only when it calls the oracle exactly.)
- [ ] **Step 3: Write the parity test** `cite_parity.rs`:
```rust
//! Verdict-label parity with the parent cite oracle on the fixture corpus.
//! Skips when the oracle (parent monorepo) is not on disk.
use std::collections::BTreeMap;
use std::path::Path;
use std::process::Command;

#[test]
fn brit_verdicts_match_oracle() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/cite_corpus");
    let py = Command::new("python3").arg(dir.join("oracle.py")).arg(&dir).output().unwrap();
    if py.status.code() == Some(3) { eprintln!("oracle absent; skipping parity"); return; }
    assert!(py.status.success(), "oracle: {}", String::from_utf8_lossy(&py.stderr));
    let oracle: BTreeMap<String, String> = serde_json::from_slice(&py.stdout).unwrap();

    let idx = brit_epr::engine::cite::SlugIndex::build(&[dir.clone()]).unwrap();
    let mut brit = BTreeMap::new();
    for e in std::fs::read_dir(&dir).unwrap() {
        let p = e.unwrap().path();
        if p.extension().is_none_or(|x| x != "md") { continue; }
        let c = std::fs::read_to_string(&p).unwrap();
        let (fm, _) = brit_epr::engine::frontmatter::split_frontmatter(&c);
        let Some(fm) = fm else { continue };
        let id = brit_epr::engine::cite::extract_id(fm).unwrap_or_default();
        for edge in brit_epr::engine::cite::extract_cites(fm) {
            let v = format!("{:?}", brit_epr::engine::verdict::verdict(&edge, &idx)).to_lowercase();
            brit.insert(format!("{id}|{}", edge.ref_), v);
        }
    }
    assert_eq!(brit, oracle, "verdict-label divergence brit (left) vs oracle (right)");
}
```
- [ ] **Step 4: Run** — `RUSTFLAGS="" CARGO_TARGET_DIR=/tmp/brit-target cargo test -p brit-epr --test cite_parity`. Resolve any divergence by aligning `verdict`/`oracle.py` (e.g. the oracle's `held` path-segment check, or a `legacy` line) until equal. (In this container the oracle is present, so the test runs, not skips.)
- [ ] **Step 5: Commit**
```bash
git add brit-epr/tests/cite_parity.rs brit-epr/tests/fixtures/cite_corpus/
git commit -m "test(brit-epr): verdict-label parity with parent cite oracle (fixture corpus)

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

### Task C5: Dogfood real brit docs + update the memory index

**Files:** Modify ~3 brit docs (prepend `---` frontmatter); Modify `brit/.claude/memory/MEMORY.md`.

**Interfaces:** none (content + index).

- [ ] **Step 1: Author frontmatter** on the epr-meta docs — prepend to `docs/specs/2026-06-29-epr-meta-composition-snapshot-canonical-cites-design.md`, its master `docs/specs/2026-06-29-canonical-epr-meta-git-bridge-design.md`, and this plan a YAML block, e.g.:
```
---
id: epr-meta-composition-snapshot
cites:
  - canonical-epr-meta-git-bridge | the master design this refines | sha256:<run `brit epr-meta status` to fill, or leave for a propagate pass>
---
```
Keep the existing `**Date:**`/`**Status:**` bold lines in the body. (Cross-repo `Consumes:` of the parent oracle stays prose — it is an `external` edge by design, not a doc-cite.)
- [ ] **Step 2: Run the verb on the real corpus** — `RUSTFLAGS="" CARGO_TARGET_DIR=/tmp/brit-target cargo run -p brit-build-ref -- --repo . meta status --dir docs/specs`. Confirm it prints verdicts (organic dogfood; cross-repo cites read `dead`/`legacy`, which is expected and documented).
- [ ] **Step 3: Update the memory index** — add to `brit/.claude/memory/MEMORY.md` under `## project`:
```
- [EPR-meta composition snapshot — canonical cites & parity slice](../../docs/specs/2026-06-29-epr-meta-composition-snapshot-canonical-cites-design.md) — the Snapshot discipline: generic cite verdict engine + EprMeta/NodeSeed import/export envelope + `brit epr-meta status`, proven at parity with the parent cite oracle; governance (floor/ceiling, Dunbar-graduated stewardship, Mishpat::Commitment) is Layer-2/DHT, deferred.
```
- [ ] **Step 4: Commit**
```bash
git add docs/specs/2026-06-29-epr-meta-composition-snapshot-canonical-cites-design.md docs/specs/2026-06-29-canonical-epr-meta-git-bridge-design.md docs/plans/2026-06-29-epr-meta-composition-snapshot-canonical-cites-plan.md brit/.claude/memory/MEMORY.md
git commit -m "docs(brit): dogfood canonical cites on the epr-meta docs + memory index

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

### Task C6: Thin advisory post-hook (the felt deliverable)

**Files:** Create `brit/.claude/hooks/epr-meta-status-signal.sh`; Create `brit/.claude/settings.json`.

**Interfaces:** the hook shells `brit epr-meta status` on the edited file's directory and emits an advisory (never-blocking) line.

- [ ] **Step 1: Write the hook script** `brit/.claude/hooks/epr-meta-status-signal.sh`:
```bash
#!/usr/bin/env bash
# Advisory: on edit of a brit *.md, report any non-ok cite verdicts. Never blocks.
set -euo pipefail
file="${CLAUDE_FILE_PATH:-}"
case "$file" in *.md) ;; *) exit 0 ;; esac
dir="$(dirname "$file")"
out="$(cd "${CLAUDE_PROJECT_DIR:-.}" && cargo run -q -p brit-build-ref -- --repo . meta status --dir "$dir" 2>/dev/null | grep -Ev '^ok ' || true)"
[ -n "$out" ] && printf 'epr-meta drift/debt:\n%s\n' "$out"
exit 0
```
`chmod +x` it.
- [ ] **Step 2: Register the hook** `brit/.claude/settings.json`:
```json
{
  "hooks": {
    "PostToolUse": [
      { "matcher": "Edit|Write",
        "hooks": [ { "type": "command", "command": "$CLAUDE_PROJECT_DIR/.claude/hooks/epr-meta-status-signal.sh" } ] }
    ]
  }
}
```
- [ ] **Step 3: Manual verification** — edit a fixture `.md` with a dead cite and confirm the script prints the advisory: `CLAUDE_FILE_PATH=brit-epr/tests/fixtures/cite_corpus/citer.md CLAUDE_PROJECT_DIR=. bash brit/.claude/hooks/epr-meta-status-signal.sh`. (Auto-fire depends on whether the harness loads a submodule's `.claude/settings.json`; the script is the deliverable — wire-ready and manually verifiable regardless.)
- [ ] **Step 4: Commit**
```bash
git add brit/.claude/hooks/epr-meta-status-signal.sh brit/.claude/settings.json
git commit -m "feat(brit): advisory PostToolUse hook surfacing epr-meta cite drift

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Self-Review

**Spec coverage** (against `2026-06-29-epr-meta-composition-snapshot-canonical-cites-design.md`):
- §2 oracle imported, verdict-label parity → C4 ✅. §3.1 `InterfaceRef` (cid `Option`, drift fingerprint, kinds incl. legacy/external) → B1 ✅. §3.2 `EprMeta` content-pure (no `parent`) + `sub_seeds` recursion-close → C1 ✅. §3.3 generic-engine placement → C1 (lift) ✅. §4 Layer-1 / "current = filesystem" → B3 ✅. §5 sub-slices A/B/C → all tasks ✅. §7.1 body-extraction conformance (first gate) → A3 ✅. §7.2 serde stability (`#[serde(default)]`, no skip) → C1 ✅. §7.3 ordering + `BritCid: Ord` → A1 + C2 ✅. Done-criteria 1–7 → A3, C4, C1, B3/C3, C1(`--no-default-features`), C6, C5 ✅.
- **Deferred (NOT in this plan, by design):** the version-DAG/`parent` commit node, signed head records, governance `kind: contract` enforcement + tag-off, `NodeSeed` consume-check/`lock`, functional kinds, git-bridge lift, recompose. Each is its own later spec+plan (spec §6).

**Placeholder scan:** the only intentional "fill at runtime" is the real drift `sha256:` values in the C4/C5 fixtures (computed by running `drift_fingerprint`, not guessed) and the C4 oracle-call arity (read the real `cite_graph.py` signatures) — both are explicit instructions, not vague TODOs.

**Type consistency:** `BritCid` (+`Ord`), `InterfaceRef{kind,role,ref_,cid,drift,desc}`, `EdgeKind`/`EdgeRole`, `Verdict{Ok,Held,Stale,Dead}`, `SlugIndex::{build,resolve}`, `split_frontmatter`/`canonical_body`/`drift_fingerprint`, `extract_id`/`extract_cites`, `verdict()`, `meta_cmd::{seal,verify,status}` are used identically across tasks.

**Open risk to flag at execution:** the C4 oracle driver must match the *real* `parse_cite`/`envelope_verdict`/`build_slug_index` signatures in `/projects/elohim/.claude/scripts/_lib/cite_graph.py` (the deep-read summary may lag the code) — read them before trusting the harness. If the oracle path is absent the parity test skips (not fails).

## Execution Handoff

Plan saved to `docs/plans/2026-06-29-epr-meta-composition-snapshot-canonical-cites-plan.md`. Two execution options:

1. **Subagent-Driven (recommended)** — a fresh subagent per task, two-stage review between tasks.
2. **Inline Execution** — execute tasks in this session with checkpoints.
