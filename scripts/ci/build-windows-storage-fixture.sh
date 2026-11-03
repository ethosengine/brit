#!/usr/bin/env bash
# Disposable cross-platform test snapshot; never a crate publication source.
set -euo pipefail
if [[ $# != 3 ]]; then
  echo "Usage: $0 build|seal|verify <elohim-source-root> <new-fixture-root>" >&2
  exit 2
fi
python3 - "$@" <<'PY'
import hashlib
import json
import re
import shutil
import sys
import tomllib
from pathlib import Path

mode, source_arg, fixture_arg = sys.argv[1:]
if mode not in {"build", "seal", "verify"}:
    raise SystemExit("Expected build, seal, or verify")
source, fixture = Path(source_arg).resolve(), Path(fixture_arg).resolve()
workspace = "elohim/eprfs"
members = ["eprfs-core", "eprfs-serve", "eprfs-storage"]
copied = {}
for crate in [*(f"{workspace}/{name}" for name in members), "crates/seam-contracts"]:
    for name in ["Cargo.toml", "README.md", "Cargo.lock", "build.rs"]:
        path = source / crate / name
        if path.is_file():
            copied[f"{crate}/{name}"] = f"{crate}/{name}"
    for name in ["src", "tests"]:
        for path in sorted((source / crate / name).rglob("*")):
            if path.is_symlink():
                raise SystemExit(f"Refusing symlink in source fixture: {path}")
            if path.is_file():
                relative = path.relative_to(source).as_posix()
                copied[relative] = relative
for name in ["eprfs-local", "eprfs-agent"]:
    copied[f"{workspace}/{name}/Cargo.toml"] = f"{workspace}/{name}/Cargo.toml"
for name in ["fixtures/tree-vectors.json", "Cargo.toml", "Cargo.lock"]:
    destination = f"{workspace}/{name}" if name.startswith("fixtures/") else f"source-workspace/{name}"
    copied[destination] = f"{workspace}/{name}"

original = (source / workspace / "Cargo.toml").read_text()
tables = re.split(r"(?m)^(?=\[)", original)
inherited = "".join(part for part in tables if part.startswith((
    "[workspace.package]", "[workspace.dependencies]", "[profile.")))
reduced = '[workspace]\nresolver = "2"\nmembers = ' + json.dumps(members) + '\n\n' + inherited
parsed = tomllib.loads(reduced)
original_parsed = tomllib.loads(original)
for table in ["package", "dependencies"]:
    assert parsed["workspace"][table] == original_parsed["workspace"][table]
assert parsed.get("profile") == original_parsed.get("profile")
generated = {
    f"{workspace}/Cargo.toml": reduced.encode(),
    ".gitattributes": b"* -text\n",
}

def digest(data):
    return hashlib.sha256(data).hexdigest()

if mode == "build":
    if fixture.exists():
        raise SystemExit("Fixture already exists; use a new directory, or seal/verify the existing snapshot")
    fixture.mkdir(parents=True)
    for destination, origin in copied.items():
        target = fixture / destination
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(source / origin, target)
    for destination, data in generated.items():
        (fixture / destination).write_bytes(data)
    shutil.copyfile(source / workspace / "Cargo.lock", fixture / workspace / "Cargo.lock")

expected = set(copied) | set(generated) | {f"{workspace}/Cargo.lock"}
actual = {p.relative_to(fixture).as_posix() for p in fixture.rglob("*") if p.is_file()}
if actual - {"candidate.json"} != expected:
    raise SystemExit(f"Unexpected fixture inventory: {sorted((actual - {'candidate.json'}) ^ expected)}")
for destination, origin in copied.items():
    if (fixture / destination).read_bytes() != (source / origin).read_bytes():
        raise SystemExit(f"Source bytes changed or fixture differs: {origin}")
for destination, data in generated.items():
    if (fixture / destination).read_bytes() != data:
        raise SystemExit(f"Derived file does not reproduce: {destination}")

receipt = {
    "schema": "eprfs-windows-source-fixture/v1",
    "storageVersion": tomllib.loads((fixture / workspace / "eprfs-storage/Cargo.toml").read_text())["package"]["version"],
    "rustToolchain": "1.96.1",
    "purpose": "Disposable production-source Windows tests; no publication or acceptance",
    "workspaceChanges": ["Only core, serve and storage members", "Unrelated iroh patch omitted"],
    "files": [{"path": name, "sha256": digest((fixture / name).read_bytes()),
               "sourcePath": copied.get(name),
               "derived": None if name in copied else ("Cargo-resolved fixture lock" if name.endswith("Cargo.lock") else "Fixture harness")}
              for name in sorted(expected)],
}
receipt_bytes = (json.dumps(receipt, indent=2) + "\n").encode()
if mode == "verify":
    if (fixture / "candidate.json").read_bytes() != receipt_bytes:
        raise SystemExit("Fixture hashes/provenance differ from candidate.json")
else:
    (fixture / "candidate.json").write_bytes(receipt_bytes)
print(f"{mode}: {len(expected)} files verified; candidate.json sha256={digest(receipt_bytes)}")
PY
