"""Resumable, dependency-ordered publisher for the brit package set.

The package set comes from Cargo metadata, including optional and build path
dependencies. Dev dependencies are not needed to install the binary and are
not propagated to consumers. All local edges are validated before any upload.
Individual packages are then packaged and published in dependency order so
Cargo can resolve newly published predecessors while generating Cargo.lock.
The sequence is not atomic: earlier packages can be present if a later one
fails. Re-running is safe only when an existing version's payload is equivalent.
"""

from __future__ import annotations

import argparse
import hashlib
import io
import json
import os
import subprocess
import sys
import tarfile
import time
import tomllib
from pathlib import Path
from urllib.error import HTTPError, URLError
from urllib.parse import urlsplit
from urllib.request import urlopen


class PublishError(Exception):
    pass


ROOTS = ("brit-cli", "brit-build-ref")
MAX_ARCHIVE_BYTES = 128 * 1024 * 1024
MAX_UNPACKED_BYTES = 512 * 1024 * 1024


def sparse_path(name: str) -> str:
    if len(name) == 1:
        return f"1/{name}"
    if len(name) == 2:
        return f"2/{name}"
    if len(name) == 3:
        return f"3/{name[0]}/{name}"
    return f"{name[:2]}/{name[2:4]}/{name}"


def publication_order(metadata: dict, registry: str, roots: tuple[str, ...] = ROOTS) -> list[dict]:
    """Return the complete local normal/optional/build closure, dependencies first."""
    packages = metadata["packages"]
    by_path = {str(Path(p["manifest_path"]).parent): p for p in packages}
    by_name = {p["name"]: p for p in packages}
    absent = [name for name in roots if name not in by_name]
    if absent:
        raise PublishError(f"publish roots absent from Cargo metadata: {', '.join(absent)}")

    done: set[str] = set()
    visiting: set[str] = set()
    ordered: list[dict] = []
    missing_tags: list[str] = []

    def visit(package: dict) -> None:
        name = package["name"]
        if name in done:
            return
        if name in visiting:
            raise PublishError(f"cycle in local publish closure at {name}")
        if package.get("publish") == [] or (
            package.get("publish") is not None and "elohim" not in package["publish"]
        ):
            raise PublishError(f"{name} is not permitted to publish to elohim")
        visiting.add(name)
        for edge in package["dependencies"]:
            if edge.get("path") is None or edge.get("kind") == "dev":
                continue
            target = by_path.get(str(Path(edge["path"])))
            if target is None:
                raise PublishError(f"{name} has unresolved local dependency {edge['name']}")
            if edge.get("registry") != registry:
                missing_tags.append(f"{name} -> {target['name']}")
            visit(target)
        visiting.remove(name)
        done.add(name)
        ordered.append(package)

    for root in roots:
        visit(by_name[root])
    if missing_tags:
        sample = ", ".join(missing_tags[:8])
        raise PublishError(
            f"{len(missing_tags)} local dependency edges lack registry=elohim "
            f"({sample}); refusing publication before packaging/upload"
        )
    return ordered


def index_record(index_url: str, name: str, version: str) -> dict | None:
    url = index_url.rstrip("/") + "/" + sparse_path(name)
    try:
        with urlopen(url, timeout=15) as response:
            body = response.read(8 * 1024 * 1024 + 1)
    except HTTPError as error:
        if error.code == 404:
            return None
        raise PublishError(f"registry index HTTP {error.code} for {name}") from error
    except (OSError, URLError) as error:
        raise PublishError(f"registry index unavailable for {name} ({type(error).__name__})") from error
    if len(body) > 8 * 1024 * 1024:
        raise PublishError(f"registry index response too large for {name}")
    try:
        rows = [json.loads(line) for line in body.splitlines() if line]
    except (ValueError, UnicodeDecodeError) as error:
        raise PublishError(f"invalid registry index response for {name}") from error
    matches = [row for row in rows if row.get("vers") == version]
    if len(matches) > 1:
        raise PublishError(f"duplicate registry index records for {name} {version}")
    if matches and (
        matches[0].get("name") != name
        or not isinstance(matches[0].get("cksum"), str)
        or len(matches[0]["cksum"]) != 64
        or any(c not in "0123456789abcdef" for c in matches[0]["cksum"])
        or matches[0].get("yanked", False)
    ):
        raise PublishError(f"invalid registry index record for {name} {version}")
    return matches[0] if matches else None


def archive_files(data: bytes, name: str, version: str) -> dict[str, tuple[str, str, int]]:
    """Map safe entries to (kind, content SHA-256, full permission bits)."""
    if len(data) > MAX_ARCHIVE_BYTES:
        raise PublishError(f"unsafe archive for {name} {version}: compressed size limit")
    prefix = f"{name}-{version}/"
    files: dict[str, tuple[str, str, int]] = {}
    seen: set[str] = set()
    total = 0
    try:
        with tarfile.open(fileobj=io.BytesIO(data), mode="r:gz") as archive:
            for member in archive:
                path = member.name
                if member.isdir() and path in (f"{name}-{version}", f"{name}-{version}/"):
                    if "." in seen:
                        raise PublishError(f"unsafe archive for {name} {version}: duplicate root")
                    seen.add(".")
                    files["."] = ("directory", "", member.mode & 0o7777)
                    continue
                if not path.startswith(prefix):
                    raise PublishError(f"unsafe archive for {name} {version}: outside package root")
                relative = path[len(prefix):]
                if member.isdir() and relative.endswith("/"):
                    relative = relative[:-1]
                parts = relative.split("/")
                if (
                    not relative
                    or relative.startswith("/")
                    or "\\" in relative
                    or any(part in ("", ".", "..") for part in parts)
                    or relative in seen
                ):
                    raise PublishError(f"unsafe archive for {name} {version}: invalid or duplicate path")
                seen.add(relative)
                if member.isdir():
                    files[relative] = ("directory", "", member.mode & 0o7777)
                    continue
                if not member.isfile():
                    raise PublishError(f"unsafe archive for {name} {version}: non-regular entry")
                total += member.size
                if total > MAX_UNPACKED_BYTES:
                    raise PublishError(f"unsafe archive for {name} {version}: unpacked size limit")
                stream = archive.extractfile(member)
                if stream is None:
                    raise PublishError(f"unsafe archive for {name} {version}: unreadable entry")
                contents = stream.read(MAX_UNPACKED_BYTES + 1)
                if len(contents) != member.size:
                    raise PublishError(f"unsafe archive for {name} {version}: truncated entry")
                if relative != ".cargo_vcs_info.json":
                    files[relative] = ("file", hashlib.sha256(contents).hexdigest(), member.mode & 0o7777)
    except (tarfile.TarError, EOFError, OSError) as error:
        raise PublishError(f"unsafe archive for {name} {version}: unreadable tarball") from error
    if "Cargo.toml" not in files:
        raise PublishError(f"unsafe archive for {name} {version}: missing Cargo.toml")
    return files


def payloads_equivalent(local: bytes, remote: bytes, name: str, version: str) -> bool:
    """Ignore only Cargo's checkout-location receipt, never lock/source/manifests."""
    return archive_files(local, name, version) == archive_files(remote, name, version)


def remote_archive(index_url: str, name: str, version: str, checksum: str) -> bytes:
    # This publisher is specifically for Nexus cargo-internal, whose config.json
    # advertises this crates endpoint. Never pass credentials in the URL.
    url = index_url.rstrip("/") + f"/crates/{name}/{version}/download"
    try:
        with urlopen(url, timeout=30) as response:
            data = response.read(MAX_ARCHIVE_BYTES + 1)
    except HTTPError as error:
        raise PublishError(f"registry archive HTTP {error.code} for {name} {version}") from error
    except (OSError, URLError) as error:
        raise PublishError(f"registry archive unavailable for {name} {version} ({type(error).__name__})") from error
    if len(data) > MAX_ARCHIVE_BYTES or hashlib.sha256(data).hexdigest() != checksum:
        raise PublishError(f"registry archive checksum mismatch for {name} {version}")
    return data


def existing_decision(index_url: str, name: str, version: str, local: bytes) -> str:
    record = index_record(index_url, name, version)
    if record is None:
        return "publish"
    remote = remote_archive(index_url, name, version, record["cksum"])
    checksum = hashlib.sha256(local).hexdigest()
    if checksum == record["cksum"]:
        return "skip-exact"
    if payloads_equivalent(local, remote, name, version):
        return "skip-equivalent"
    raise PublishError(
        f"immutable version collision for {name} {version}: local crate SHA-256 "
        f"{checksum}, registry {record['cksum']}; bump the package version"
    )


def decide(index_url: str, name: str, version: str, checksum: str) -> str:
    record = index_record(index_url, name, version)
    if record is None:
        return "publish"
    if record["cksum"] != checksum:
        raise PublishError(
            f"immutable version collision for {name} {version}: local crate SHA-256 "
            f"{checksum}, registry {record['cksum']}; bump the package version"
        )
    return "skip"


def verify_conflict(index_url: str, name: str, version: str, local: bytes) -> None:
    """A failed upload succeeds only for a verified, equivalent existing payload."""
    if existing_decision(index_url, name, version, local) == "publish":
        raise PublishError(f"upload failed and {name} {version} is absent from the registry index")


def cargo_metadata(repo: Path, env: dict[str, str]) -> dict:
    result = subprocess.run(
        ["cargo", "metadata", "--locked", "--no-deps", "--format-version", "1"],
        cwd=repo,
        env=env,
        check=True,
        capture_output=True,
        text=True,
    )
    return json.loads(result.stdout)


def package_crate(repo: Path, package: dict, env: dict[str, str]) -> tuple[Path, str]:
    name, version = package["name"], package["version"]
    subprocess.run(
        ["cargo", "package", "--locked", "--registry", "elohim", "--no-verify", "-p", name],
        cwd=repo,
        env=env,
        check=True,
    )
    archive = Path(env["CARGO_TARGET_DIR"]) / "package" / f"{name}-{version}.crate"
    if not archive.is_file():
        raise PublishError(f"Cargo did not create {archive}")
    checksum = hashlib.sha256(archive.read_bytes()).hexdigest()
    return archive, checksum


def validate_registry_url(registry: str) -> str:
    if not registry.startswith("sparse+https://"):
        raise PublishError("elohim registry must use a sparse HTTPS index")
    index_url = registry.removeprefix("sparse+")
    parsed = urlsplit(index_url)
    if parsed.username or parsed.password or parsed.query or parsed.fragment or not parsed.hostname:
        raise PublishError("elohim index URL must have no credentials, query, or fragment")
    return index_url


def assert_download_endpoint(index_url: str) -> None:
    """Pin this Nexus publisher's download URL to the registry's own config."""
    try:
        with urlopen(index_url.rstrip("/") + "/config.json", timeout=15) as response:
            data = response.read(64 * 1024 + 1)
    except (HTTPError, OSError, URLError) as error:
        raise PublishError(f"registry download config unavailable ({type(error).__name__})") from error
    if len(data) > 64 * 1024:
        raise PublishError("registry download config too large")
    try:
        config = json.loads(data)
    except (ValueError, UnicodeDecodeError) as error:
        raise PublishError("invalid registry download config") from error
    expected = index_url.rstrip("/") + "/crates"
    if config.get("dl") != expected:
        raise PublishError("Nexus download endpoint differs from the verified publisher contract")


def audit_index(index_url: str, ordered: list[dict]) -> tuple[int, int]:
    """Read-only count of exact current-version index records, never uploads."""
    present = sum(
        index_record(index_url, package["name"], package["version"]) is not None
        for package in ordered
    )
    absent = len(ordered) - present
    print(f"INDEX AUDIT: {present}/{len(ordered)} present; {absent}/{len(ordered)} absent")
    return present, absent


def run(repo: Path, publish: bool, audit: bool = False) -> None:
    config = tomllib.loads((repo / ".cargo/config.toml").read_text())
    registry = os.environ.get("CARGO_REGISTRIES_ELOHIM_INDEX", config["registries"]["elohim"]["index"])
    index_url = validate_registry_url(registry)
    env = os.environ.copy()
    env["RUSTFLAGS"] = ""
    env["RUSTC_WRAPPER"] = ""
    env["CARGO_REGISTRY_GLOBAL_CREDENTIAL_PROVIDERS"] = "cargo:token"
    if not env.get("CARGO_TARGET_DIR"):
        raise PublishError("set CARGO_TARGET_DIR to an explicit external native Cargo pool slot")

    ordered = publication_order(cargo_metadata(repo, env), registry)
    print(f"Validated {len(ordered)} local crates and all registry-routed edges", flush=True)
    if audit:
        audit_index(index_url, ordered)
        return
    assert_download_endpoint(index_url)
    if publish and not env.get("CARGO_REGISTRIES_ELOHIM_TOKEN"):
        raise PublishError("write credential missing; no crates uploaded")
    print("Publication is dependency-ordered and non-atomic; rerun safely after a failure.", flush=True)
    for package in ordered:
        name, version = package["name"], package["version"]
        archive, checksum = package_crate(repo, package, env)
        local = archive.read_bytes()
        decision = existing_decision(index_url, name, version, local)
        print(f"{decision}: {name} {version}", flush=True)
        if decision.startswith("skip") or not publish:
            continue
        if hashlib.sha256(archive.read_bytes()).hexdigest() != checksum:
            raise PublishError(f"packaged bytes changed before upload for {name} {version}")
        result = subprocess.run(
            ["cargo", "publish", "--locked", "--registry", "elohim", "--no-verify", "-p", name],
            cwd=repo,
            env=env,
            capture_output=True,
            text=True,
        )
        if result.returncode != 0:
            # Cargo may time out after a successful upload or report 409 after
            # a race. Neither is success until the full payload is verified.
            verify_conflict(index_url, name, version, local)
            print(f"verified existing payload after failed upload: {name} {version}", flush=True)
            continue
        if hashlib.sha256(archive.read_bytes()).hexdigest() != checksum:
            raise PublishError(f"Cargo repackaged different bytes for {name} {version}")
        for _attempt in range(12):
            if decide(index_url, name, version, checksum) == "skip":
                remote_archive(index_url, name, version, checksum)
                break
            time.sleep(5)
        else:
            raise PublishError(f"{name} {version} did not appear in the registry index")
        print(f"verified: {name} {version} sha256={checksum}", flush=True)
    if not publish:
        print("CHECK ONLY: no crates uploaded; absent dependencies may prevent later packaging")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    mode = parser.add_mutually_exclusive_group(required=True)
    mode.add_argument("--check", action="store_true", help="preflight without uploading")
    mode.add_argument("--publish", action="store_true", help="upload after complete preflight")
    mode.add_argument("--audit-index", action="store_true", help="count exact Nexus versions without packaging")
    args = parser.parse_args()
    repo = Path(__file__).resolve().parents[2]
    try:
        run(repo, args.publish, audit=args.audit_index)
    except (PublishError, subprocess.CalledProcessError) as error:
        print(f"ERROR: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
