"""Isolated clean-consumer tests; fake Cargo never accesses a registry or builds."""

import json
import hashlib
import os
import tempfile
import unittest
from pathlib import Path
from unittest import mock

import verify_brit_consumer as consumer


INDEX = "sparse+https://nexus.example.invalid/repository/cargo-internal/"
SOURCE = INDEX  # Real brit/Cargo.lock records Nexus as sparse+https://... .
VERSION = "0.1.2"
EXPECTED = {("brit-cli", VERSION), ("gitoxide", "0.58.0"), ("gix", "0.87.1")}
EDGES = {
    ("brit-cli", VERSION): {("gitoxide", "0.58.0")},
    ("gitoxide", "0.58.0"): {("gix", "0.87.1")},
    ("gix", "0.87.1"): set(),
}
CLOSURE = consumer.PrivateClosure(EXPECTED, EDGES)
DUAL_CLOSURE = consumer.PrivateClosure(
    EXPECTED | {("gix-trace", "0.1.21")},
    {**EDGES, ("gix", "0.87.1"): {("gix-trace", "0.1.21")},
     ("gix-trace", "0.1.21"): set()},
)


def write_lock(path: Path, *, gitoxide_source: str = SOURCE, include_fork_gix: bool = True) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    gix = f'[[package]]\nname = "gix"\nversion = "0.87.1"\nsource = "{SOURCE}"\n' if include_fork_gix else ""
    path.write_text(
        'version = 4\n'
        '[[package]]\nname = "brit-cli"\nversion = "0.1.2"\n'
        'dependencies = ["gitoxide"]\n'
        f'[[package]]\nname = "gitoxide"\nversion = "0.58.0"\nsource = "{gitoxide_source}"\n'
        'dependencies = ["gix 0.87.1"]\n'
        + gix
        + '[[package]]\nname = "gix"\nversion = "0.81.0"\n'
          'source = "registry+https://github.com/rust-lang/crates.io-index"\n'
    )


def write_dual_source_lock(path: Path, *, include_private_trace: bool = True,
                           private_edge_source: str | None = SOURCE,
                           root_edge_source: str | None = None) -> None:
    """Reduced shape of the published 0.1.2 lock: both gix lines use trace 0.1.21."""
    path.parent.mkdir(parents=True, exist_ok=True)
    public = "registry+https://github.com/rust-lang/crates.io-index"
    private_trace = (
        f'[[package]]\nname = "gix-trace"\nversion = "0.1.21"\nsource = "{SOURCE}"\n'
        if include_private_trace else ""
    )
    trace_edge = (
        f"gix-trace 0.1.21 ({private_edge_source})" if private_edge_source is not None
        else "gix-trace 0.1.21"
    )
    root_edge = (
        f"gitoxide 0.58.0 ({root_edge_source})" if root_edge_source is not None
        else "gitoxide"
    )
    path.write_text(
        'version = 4\n'
        '[[package]]\nname = "brit-cli"\nversion = "0.1.2"\n'
        f'dependencies = ["{root_edge}", "gix 0.81.0"]\n'
        f'[[package]]\nname = "gitoxide"\nversion = "0.58.0"\nsource = "{SOURCE}"\n'
        'dependencies = ["gix 0.87.1"]\n'
        f'[[package]]\nname = "gix"\nversion = "0.87.1"\nsource = "{SOURCE}"\n'
        f'dependencies = ["{trace_edge}"]\n'
        f'[[package]]\nname = "gix"\nversion = "0.81.0"\nsource = "{public}"\n'
        f'dependencies = ["gix-trace 0.1.21 ({public})"]\n'
        + private_trace
        + f'[[package]]\nname = "gix-trace"\nversion = "0.1.21"\nsource = "{public}"\n'
        + (f'[[package]]\nname = "gitoxide"\nversion = "0.58.0"\nsource = "{public}"\n'
           if root_edge_source == public else "")
    )


class GraphAuditTests(unittest.TestCase):
    def test_consumer_closure_excludes_unrelated_publisher_root(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            metadata = {"packages": [
                {"name": "brit-cli", "version": VERSION, "manifest_path": str(root / "cli/Cargo.toml"),
                 "publish": None, "dependencies": [{"name": "gitoxide", "path": str(root / "engine"),
                                                      "registry": INDEX, "kind": None}]},
                {"name": "gitoxide", "version": "0.58.0", "manifest_path": str(root / "engine/Cargo.toml"),
                 "publish": None, "dependencies": []},
                {"name": "brit-build-ref", "version": "0.1.1", "manifest_path": str(root / "build/Cargo.toml"),
                 "publish": None, "dependencies": []},
            ]}
            with mock.patch.object(consumer.publish_brit, "cargo_metadata", return_value=metadata):
                closure = consumer.expected_closure(root, INDEX, {})
            self.assertEqual(closure.packages, {("brit-cli", VERSION), ("gitoxide", "0.58.0")})
            self.assertEqual(closure.edges[("brit-cli", VERSION)], {("gitoxide", "0.58.0")})

    def test_fork_versions_are_private_but_distinct_public_gix_is_allowed(self):
        with tempfile.TemporaryDirectory() as temporary:
            lock = Path(temporary) / "Cargo.lock"
            write_lock(lock)
            consumer.audit_lock(lock, VERSION, INDEX, CLOSURE)

    def test_published_dual_source_identity_keeps_private_fork(self):
        with tempfile.TemporaryDirectory() as temporary:
            lock = Path(temporary) / "Cargo.lock"
            write_dual_source_lock(lock)
            consumer.audit_lock(lock, VERSION, INDEX, DUAL_CLOSURE)

    def test_missing_private_copy_is_not_satisfied_by_public_same_version(self):
        with tempfile.TemporaryDirectory() as temporary:
            lock = Path(temporary) / "Cargo.lock"
            write_dual_source_lock(lock, include_private_trace=False)
            with self.assertRaisesRegex(consumer.ConsumerError, "omits expected fork gix-trace"):
                consumer.audit_lock(lock, VERSION, INDEX, DUAL_CLOSURE)

    def test_private_parent_must_not_point_at_public_same_version(self):
        with tempfile.TemporaryDirectory() as temporary:
            lock = Path(temporary) / "Cargo.lock"
            public = "registry+https://github.com/rust-lang/crates.io-index"
            write_dual_source_lock(lock, private_edge_source=public)
            with self.assertRaisesRegex(consumer.ConsumerError, "gix 0.87.1 -> gix-trace"):
                consumer.audit_lock(lock, VERSION, INDEX, DUAL_CLOSURE)

    def test_ambiguous_unqualified_edge_refuses_even_with_private_copy(self):
        with tempfile.TemporaryDirectory() as temporary:
            lock = Path(temporary) / "Cargo.lock"
            write_dual_source_lock(lock, private_edge_source=None)
            with self.assertRaisesRegex(consumer.ConsumerError, "ambiguous or missing dependency gix-trace"):
                consumer.audit_lock(lock, VERSION, INDEX, DUAL_CLOSURE)

    def test_duplicate_full_package_id_refuses(self):
        with tempfile.TemporaryDirectory() as temporary:
            lock = Path(temporary) / "Cargo.lock"
            write_dual_source_lock(lock)
            with lock.open("a") as output:
                output.write(f'[[package]]\nname = "gix-trace"\nversion = "0.1.21"\nsource = "{SOURCE}"\n')
            with self.assertRaisesRegex(consumer.ConsumerError, "repeats package gix-trace 0.1.21"):
                consumer.audit_lock(lock, VERSION, INDEX, DUAL_CLOSURE)

    def test_root_must_point_at_private_gitoxide_even_if_private_copy_is_present(self):
        with tempfile.TemporaryDirectory() as temporary:
            lock = Path(temporary) / "Cargo.lock"
            public = "registry+https://github.com/rust-lang/crates.io-index"
            write_dual_source_lock(lock, root_edge_source=public)
            with self.assertRaisesRegex(consumer.ConsumerError, "brit-cli 0.1.2 -> gitoxide"):
                consumer.audit_lock(lock, VERSION, INDEX, DUAL_CLOSURE)

    def test_root_gitoxide_cannot_fall_back_to_public_registry(self):
        with tempfile.TemporaryDirectory() as temporary:
            lock = Path(temporary) / "Cargo.lock"
            write_lock(lock, gitoxide_source="registry+https://github.com/rust-lang/crates.io-index")
            with self.assertRaisesRegex(consumer.ConsumerError, "omits expected fork gitoxide"):
                consumer.audit_lock(lock, VERSION, INDEX, CLOSURE)

    def test_missing_optional_or_build_fork_fails_closed(self):
        with tempfile.TemporaryDirectory() as temporary:
            lock = Path(temporary) / "Cargo.lock"
            write_lock(lock, include_fork_gix=False)
            with self.assertRaisesRegex(consumer.ConsumerError, "omits expected fork gix"):
                consumer.audit_lock(lock, VERSION, INDEX, CLOSURE)

    def test_install_receipt_requires_exact_registry_and_version(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            (root / ".crates2.json").write_text(json.dumps({"installs": {
                f"brit-cli {VERSION} (registry+https://github.com/rust-lang/crates.io-index)": {},
            }}))
            with self.assertRaisesRegex(consumer.ConsumerError, "did not originate"):
                consumer.audit_install_provenance(root, VERSION, INDEX)
            (root / ".crates2.json").write_text(json.dumps({"installs": {
                f"brit-cli {VERSION} ({SOURCE})": {},
            }}))
            consumer.audit_install_provenance(root, VERSION, INDEX)

    def test_cached_root_crate_must_match_sparse_index_checksum(self):
        with tempfile.TemporaryDirectory() as temporary:
            home = Path(temporary)
            cache = home / "registry/cache/fake" / f"brit-cli-{VERSION}.crate"
            cache.parent.mkdir(parents=True)
            cache.write_bytes(b"published root crate bytes")
            checksum = hashlib.sha256(cache.read_bytes()).hexdigest()
            with mock.patch.object(consumer.publish_brit, "index_record", return_value={"cksum": checksum}):
                consumer.audit_cached_package(home, VERSION, INDEX)
            with mock.patch.object(consumer.publish_brit, "index_record", return_value={"cksum": "0" * 64}):
                with self.assertRaisesRegex(consumer.ConsumerError, "checksum differs"):
                    consumer.audit_cached_package(home, VERSION, INDEX)


class ConsumerRunTests(unittest.TestCase):
    def test_fresh_home_exact_locked_install_and_one_binary_smoke(self):
        with tempfile.TemporaryDirectory() as temporary:
            calls = []

            def fake_run(command, **kwargs):
                calls.append((command, kwargs))
                if "install" in command:
                    env = kwargs["env"]
                    cache = Path(env["CARGO_HOME"]) / "registry/src/fake" / f"brit-cli-{VERSION}"
                    write_lock(cache / "Cargo.lock")
                    root = Path(command[command.index("--root") + 1])
                    (root / "bin").mkdir(parents=True)
                    (root / "bin/brit").write_text("fake executable")
                    cache_archive = Path(env["CARGO_HOME"]) / "registry/cache/fake" / f"brit-cli-{VERSION}.crate"
                    cache_archive.parent.mkdir(parents=True)
                    cache_archive.write_bytes(b"published root crate bytes")
                    (root / ".crates2.json").write_text(json.dumps({"installs": {
                        f"brit-cli {VERSION} ({SOURCE})": {},
                    }}))
                return mock.Mock(returncode=0)

            env = {
                "CARGO_TARGET_DIR": str(Path(temporary) / "publisher-target"),
                "CARGO_REGISTRIES_ELOHIM_TOKEN": "redacted-test-token",
                "CARGO_REGISTRIES_ELOHIM_INDEX": INDEX,
            }
            with mock.patch.dict(os.environ, env, clear=True), \
                 mock.patch.object(consumer, "expected_closure", return_value=CLOSURE), \
                 mock.patch.object(consumer.publish_brit, "index_record", return_value={
                     "cksum": hashlib.sha256(b"published root crate bytes").hexdigest(),
                 }), \
                 mock.patch.object(consumer.subprocess, "run", side_effect=fake_run):
                consumer.run(VERSION)

            install, options = calls[0]
            self.assertIn("--locked", install)
            self.assertEqual(install[install.index("--version") + 1], f"={VERSION}")
            self.assertEqual(install[install.index("--registry") + 1], "elohim")
            self.assertEqual(options["env"]["RUSTFLAGS"], "")
            self.assertNotEqual(options["env"]["CARGO_HOME"], os.environ.get("CARGO_HOME"))
            self.assertEqual(len(calls), 4)  # install plus three smoke commands

    def test_nexus_token_fallback_is_passed_only_as_cargo_token(self):
        for source_name in ("NEXUS_NPM_TOKEN", "NPM_TOKEN"):
            with self.subTest(source_name=source_name), tempfile.TemporaryDirectory() as temporary:
                env = {
                    "CARGO_TARGET_DIR": str(Path(temporary) / "publisher-target"),
                    source_name: "secret-test-value",
                    "CARGO_REGISTRIES_ELOHIM_INDEX": INDEX,
                }
                seen = {}

                def capture_closure(_repo, _index, metadata_env):
                    seen.update(metadata_env)
                    raise consumer.ConsumerError("stop before install")

                with mock.patch.dict(os.environ, env, clear=True), \
                     mock.patch.object(consumer, "expected_closure", side_effect=capture_closure):
                    with self.assertRaisesRegex(consumer.ConsumerError, "stop before install"):
                        consumer.run(VERSION)
                self.assertEqual(seen["CARGO_REGISTRIES_ELOHIM_TOKEN"], "Bearer secret-test-value")
                self.assertNotIn(source_name, seen)


if __name__ == "__main__":
    unittest.main()
