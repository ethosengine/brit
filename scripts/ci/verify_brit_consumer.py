"""Verify an exact published Brit package from an isolated Cargo consumer.

This does not publish. It requires the published binary package to carry a
Cargo.lock so the dependency sources resolved by ``cargo install --locked``
can be audited independently of the source checkout's lockfile.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import subprocess
import sys
import tempfile
import tomllib
from dataclasses import dataclass
from pathlib import Path
from urllib.parse import urlsplit

import publish_brit


class ConsumerError(Exception):
    pass


PackageIdentity = tuple[str, str]
PackageId = tuple[str, str, str | None]


@dataclass(frozen=True)
class PrivateClosure:
    packages: set[PackageIdentity]
    edges: dict[PackageIdentity, set[PackageIdentity]]


def registry_index() -> str:
    index = os.environ.get(
        "CARGO_REGISTRIES_ELOHIM_INDEX",
        "sparse+https://nexus.ethosengine.com/repository/cargo-internal/",
    )
    if not index.startswith("sparse+https://"):
        raise ConsumerError("elohim registry index must use sparse HTTPS")
    parsed = urlsplit(index.removeprefix("sparse+"))
    if parsed.username or parsed.password or parsed.query or parsed.fragment or not parsed.hostname:
        raise ConsumerError("elohim registry index must not contain credentials or URL parameters")
    return index


def expected_closure(repo: Path, index: str, env: dict[str, str]) -> PrivateClosure:
    metadata = publish_brit.cargo_metadata(repo, env)
    ordered = publish_brit.publication_order(metadata, index, roots=("brit-cli",))
    by_path = {str(Path(package["manifest_path"]).parent): package for package in ordered}
    packages = {(package["name"], package["version"]) for package in ordered}
    edges: dict[PackageIdentity, set[PackageIdentity]] = {}
    for package in ordered:
        parent = (package["name"], package["version"])
        edges[parent] = {
            (by_path[str(Path(dependency["path"]))]["name"],
             by_path[str(Path(dependency["path"]))]["version"])
            for dependency in package["dependencies"]
            if dependency.get("path") is not None and dependency.get("kind") != "dev"
        }
    return PrivateClosure(packages, edges)


def resolve_lock_dependency(reference: str, packages: dict[PackageId, dict]) -> PackageId:
    """Resolve one Cargo.lock dependency spelling to exactly one package ID."""
    match = re.fullmatch(r"([^ ()]+)(?: ([^ ()]+))?(?: \(([^()]+)\))?", reference)
    if match is None:
        raise ConsumerError("published lock has an invalid dependency reference")
    name, version, source = match.groups()
    matches = [identity for identity in packages
               if identity[0] == name
               and (version is None or identity[1] == version)
               and (source is None or identity[2] == source)]
    if len(matches) != 1:
        raise ConsumerError(f"published lock has ambiguous or missing dependency {name}")
    return matches[0]


def audit_lock(lock_path: Path, version: str, index: str, expected: PrivateClosure) -> None:
    packages = tomllib.loads(lock_path.read_text())["package"]
    # Cargo.lock records named sparse registries with the sparse+ source ID.
    # The repository's existing rakia/elohim-epr entries establish this shape.
    by_id: dict[PackageId, dict] = {}
    for package in packages:
        identity = (package["name"], package["version"], package.get("source"))
        if identity in by_id:
            raise ConsumerError(f"published lock repeats package {identity[0]} {identity[1]}")
        by_id[identity] = package
    root_id = ("brit-cli", version, None)
    if root_id not in by_id:
        raise ConsumerError("published lock lacks the exact brit-cli root package")
    missing = {identity for identity in expected.packages - {("brit-cli", version)}
               if (*identity, index) not in by_id}
    if missing:
        name, missing_version = sorted(missing)[0]
        raise ConsumerError(f"published lock omits expected fork {name} {missing_version}")
    roots = {item for item in expected.packages if item[0] == "gitoxide"}
    if len(roots) != 1 or (*next(iter(roots)), index) not in by_id:
        raise ConsumerError("published lock lacks the exact elohim gitoxide root dependency")
    for parent, children in expected.edges.items():
        parent_id = (*parent, None if parent == ("brit-cli", version) else index)
        dependencies = by_id[parent_id].get("dependencies", [])
        if not isinstance(dependencies, list) or any(not isinstance(item, str) for item in dependencies):
            raise ConsumerError(f"published lock has invalid dependencies for {parent[0]}")
        resolved = {resolve_lock_dependency(item, by_id) for item in dependencies}
        for child in children:
            if (*child, index) not in resolved:
                raise ConsumerError(
                    f"published lock does not bind {parent[0]} {parent[1]} -> "
                    f"{child[0]} {child[1]} to the elohim registry"
                )


def audit_install_provenance(install_root: Path, version: str, index: str) -> None:
    receipt = install_root / ".crates2.json"
    if not receipt.is_file():
        raise ConsumerError("Cargo install provenance receipt is absent")
    installs = json.loads(receipt.read_text())["installs"]
    # Cargo's install receipt may render a registry source using either source
    # display prefix; the endpoint must still be the exact elohim index.
    sources = (index, "registry+" + index.removeprefix("sparse+"))
    if not any(f"brit-cli {version} ({source})" in installs for source in sources):
        raise ConsumerError("installed brit-cli did not originate from exact elohim registry version")


def audit_cached_package(cargo_home: Path, version: str, index: str) -> None:
    archives = list((cargo_home / "registry" / "cache").glob(f"*/brit-cli-{version}.crate"))
    if len(archives) != 1:
        raise ConsumerError("clean Cargo home lacks one cached brit-cli crate archive")
    record = publish_brit.index_record(index.removeprefix("sparse+"), "brit-cli", version)
    if record is None:
        raise ConsumerError("installed brit-cli version is absent from elohim sparse index")
    digest = hashlib.sha256(archives[0].read_bytes()).hexdigest()
    if digest != record["cksum"]:
        raise ConsumerError("cached brit-cli crate checksum differs from elohim sparse index")


def run(version: str) -> None:
    if not re.fullmatch(r"[0-9]+\.[0-9]+\.[0-9]+", version):
        raise ConsumerError("version must be an exact major.minor.patch value")
    index = registry_index()
    repo = Path(__file__).resolve().parents[2]
    outer_target = os.environ.get("CARGO_TARGET_DIR")
    if not outer_target or not Path(outer_target).is_absolute():
        raise ConsumerError("CARGO_TARGET_DIR must name an absolute external Cargo pool slot")
    target_path = Path(outer_target).resolve()
    if target_path.is_relative_to(repo.resolve()):
        raise ConsumerError("Cargo target slot must be outside the source checkout")
    target_parent = target_path.parent
    if not target_parent.is_dir():
        raise ConsumerError("external Cargo target parent does not exist")
    token = os.environ.get("CARGO_REGISTRIES_ELOHIM_TOKEN")
    if not token:
        npm_token = os.environ.get("NEXUS_NPM_TOKEN") or os.environ.get("NPM_TOKEN")
        if npm_token:
            token = "Bearer " + npm_token
    if not token:
        raise ConsumerError("elohim registry read credential is missing")

    metadata_env = os.environ.copy()
    metadata_env["CARGO_REGISTRIES_ELOHIM_TOKEN"] = token
    metadata_env.pop("NEXUS_NPM_TOKEN", None)
    metadata_env.pop("NPM_TOKEN", None)
    metadata_env["RUSTFLAGS"] = ""
    metadata_env.pop("CARGO_ENCODED_RUSTFLAGS", None)
    expected = expected_closure(repo, index, metadata_env)
    if ("brit-cli", version) not in expected.packages:
        raise ConsumerError("requested version differs from the checked-out brit-cli manifest")

    with tempfile.TemporaryDirectory(prefix="brit-consumer-", dir=target_parent) as temporary:
        root = Path(temporary)
        env = os.environ.copy()
        env.update(
            CARGO_HOME=str(root / "cargo-home"),
            CARGO_TARGET_DIR=str(root / "target"),
            CARGO_REGISTRY_GLOBAL_CREDENTIAL_PROVIDERS="cargo:token",
            RUSTFLAGS="",
            RUSTC_WRAPPER="",
            CARGO_BUILD_JOBS="1",
            CARGO_REGISTRIES_ELOHIM_TOKEN=token,
        )
        env.pop("NEXUS_NPM_TOKEN", None)
        env.pop("NPM_TOKEN", None)
        env.pop("CARGO_ENCODED_RUSTFLAGS", None)
        install_root = root / "install"
        subprocess.run(
            [
                "cargo", "--config", f'registries.elohim.index="{index}"',
                "install", "--locked", "--registry", "elohim", "--version", f"={version}",
                "--no-default-features", "--features", "max-pure", "--bin", "brit",
                "--root", str(install_root), "brit-cli",
            ],
            cwd=root,
            env=env,
            check=True,
        )
        source_cache = root / "cargo-home" / "registry" / "src"
        locks = list(source_cache.glob(f"*/brit-cli-{version}/Cargo.lock"))
        if len(locks) != 1:
            raise ConsumerError("published brit-cli must supply one auditable Cargo.lock")
        audit_lock(locks[0], version, index, expected)
        audit_install_provenance(install_root, version, index)
        audit_cached_package(root / "cargo-home", version, index)
        bin_dir = install_root / "bin"
        expected_binary = "brit.exe" if os.name == "nt" else "brit"
        binaries = sorted(path.name for path in bin_dir.iterdir() if path.is_file())
        if binaries != [expected_binary]:
            raise ConsumerError("clean install did not produce only the brit executable")
        binary = bin_dir / expected_binary
        for arguments in (["--help"], ["build", "--help"], ["--version"]):
            subprocess.run([str(binary), *arguments], cwd=root, env=env, check=True, stdout=subprocess.DEVNULL)
    print(f"verified isolated elohim consumer: brit-cli {version}, one brit binary, locked fork sources")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("version")
    args = parser.parse_args()
    try:
        run(args.version)
    except (ConsumerError, publish_brit.PublishError, subprocess.CalledProcessError, OSError, KeyError, ValueError) as error:
        # Cargo handles its own credential redaction; never echo its environment
        # or a registry URL containing user-supplied credentials here.
        if isinstance(error, subprocess.CalledProcessError):
            print("ERROR: isolated Cargo command failed", file=sys.stderr)
        else:
            print(f"ERROR: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
