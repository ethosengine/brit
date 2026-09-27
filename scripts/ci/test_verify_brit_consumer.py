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


def write_lock(path: Path, *, gitoxide_source: str = SOURCE, include_fork_gix: bool = True) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    gix = f'[[package]]\nname = "gix"\nversion = "0.87.1"\nsource = "{SOURCE}"\n' if include_fork_gix else ""
    path.write_text(
        'version = 4\n'
        '[[package]]\nname = "brit-cli"\nversion = "0.1.2"\n'
        f'[[package]]\nname = "gitoxide"\nversion = "0.58.0"\nsource = "{gitoxide_source}"\n'
        + gix
        + '[[package]]\nname = "gix"\nversion = "0.81.0"\n'
          'source = "registry+https://github.com/rust-lang/crates.io-index"\n'
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
            self.assertEqual(closure, {("brit-cli", VERSION), ("gitoxide", "0.58.0")})

    def test_fork_versions_are_private_but_distinct_public_gix_is_allowed(self):
        with tempfile.TemporaryDirectory() as temporary:
            lock = Path(temporary) / "Cargo.lock"
            write_lock(lock)
            consumer.audit_lock(lock, VERSION, INDEX, EXPECTED)

    def test_root_gitoxide_cannot_fall_back_to_public_registry(self):
        with tempfile.TemporaryDirectory() as temporary:
            lock = Path(temporary) / "Cargo.lock"
            write_lock(lock, gitoxide_source="registry+https://github.com/rust-lang/crates.io-index")
            with self.assertRaisesRegex(consumer.ConsumerError, "gitoxide.*outside"):
                consumer.audit_lock(lock, VERSION, INDEX, EXPECTED)

    def test_missing_optional_or_build_fork_fails_closed(self):
        with tempfile.TemporaryDirectory() as temporary:
            lock = Path(temporary) / "Cargo.lock"
            write_lock(lock, include_fork_gix=False)
            with self.assertRaisesRegex(consumer.ConsumerError, "omits expected fork gix"):
                consumer.audit_lock(lock, VERSION, INDEX, EXPECTED)

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
                 mock.patch.object(consumer, "expected_closure", return_value=EXPECTED), \
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
