"""Isolated contract tests for the Nexus publisher; never contacts Nexus."""

import json
import io
import contextlib
import tarfile
import tempfile
import threading
import unittest
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
from unittest import mock

import publish_brit


def package(name, version, path, dependencies=(), features=None):
    return {
        "name": name,
        "version": version,
        "manifest_path": str(path / "Cargo.toml"),
        "publish": None,
        "dependencies": list(dependencies),
        "features": features or {},
    }


def dependency(name, path, registry, kind=None, optional=False, rename=None):
    return {
        "name": name,
        "path": str(path),
        "registry": registry,
        "kind": kind,
        "optional": optional,
        "rename": rename,
    }


def crate_archive(name, version, files, executable=(), symlink=None, modes=None, directories=()):
    output = io.BytesIO()
    with tarfile.open(fileobj=output, mode="w:gz") as archive:
        for directory in directories:
            info = tarfile.TarInfo(f"{name}-{version}/{directory}/")
            info.type = tarfile.DIRTYPE
            info.mode = 0o755
            archive.addfile(info)
        for path, data in files.items():
            info = tarfile.TarInfo(f"{name}-{version}/{path}")
            info.size = len(data)
            info.mode = modes[path] if modes and path in modes else (0o755 if path in executable else 0o644)
            archive.addfile(info, io.BytesIO(data))
        if symlink:
            info = tarfile.TarInfo(f"{name}-{version}/{symlink}")
            info.type = tarfile.SYMTYPE
            info.linkname = "../outside"
            archive.addfile(info)
    return output.getvalue()


class FakeRegistry:
    def __init__(self):
        self.records = {}
        self.archives = {}
        self.status = 200

        outer = self

        class Handler(BaseHTTPRequestHandler):
            def do_GET(self):
                if outer.status != 200:
                    self.send_response(outer.status)
                    self.end_headers()
                    return
                if self.path in outer.archives:
                    self.send_response(200)
                    self.end_headers()
                    self.wfile.write(outer.archives[self.path])
                    return
                records = outer.records.get(self.path)
                if records is None:
                    self.send_response(404)
                    self.end_headers()
                    return
                body = b"\n".join(json.dumps(record).encode() for record in records)
                self.send_response(200)
                self.end_headers()
                self.wfile.write(body)

            def log_message(self, *_args):
                pass

        self.server = ThreadingHTTPServer(("127.0.0.1", 0), Handler)
        self.thread = threading.Thread(target=self.server.serve_forever, daemon=True)
        self.thread.start()
        self.url = f"http://127.0.0.1:{self.server.server_port}/"

    def close(self):
        self.server.shutdown()
        self.server.server_close()
        self.thread.join()


class ClosureTests(unittest.TestCase):
    def test_includes_optional_and_build_edges_in_dependency_order(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            registry = "sparse+https://example.invalid/"
            metadata = {"packages": [
                package("brit-cli", "0.1.2", root / "cli", [
                    dependency("gitoxide", root / "engine", registry),
                    dependency("optional", root / "optional", registry, optional=True),
                    dependency("build", root / "build", registry, kind="build"),
                    dependency("dev-only", root / "dev", None, kind="dev"),
                ]),
                package("gitoxide", "0.58.0", root / "engine"),
                package("optional", "0.1.0", root / "optional"),
                package("build", "0.1.0", root / "build"),
                package("dev-only", "0.1.0", root / "dev"),
                package("brit-build-ref", "0.1.1", root / "build-ref"),
            ]}
            order = publish_brit.publication_order(metadata, registry)
            self.assertEqual([p["name"] for p in order], ["gitoxide", "optional", "build", "brit-cli", "brit-build-ref"])
            cli_only = publish_brit.publication_order(metadata, registry, roots=("brit-cli",))
            self.assertEqual([p["name"] for p in cli_only], ["gitoxide", "optional", "build", "brit-cli"])

    def test_missing_registry_tag_refuses_before_publish(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            metadata = {"packages": [
                package("brit-cli", "0.1.2", root / "cli", [
                    dependency("gitoxide", root / "engine", None),
                ]),
                package("gitoxide", "0.58.0", root / "engine"),
                package("brit-build-ref", "0.1.1", root / "build-ref"),
            ]}
            with self.assertRaisesRegex(publish_brit.PublishError, "registry.*gitoxide"):
                publish_brit.publication_order(metadata, "sparse+https://example.invalid/")

    def test_explicit_version_dev_or_target_source_must_match_normal_source(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            crate = root / "gix"
            crate.mkdir()
            manifest = crate / "Cargo.toml"
            registry = "sparse+https://example.invalid/"
            normal = dependency("gix-hash", root / "gix-hash", registry)
            dev = dependency("gix-hash", root / "gix-hash", None, kind="dev")
            item = package("gix", "0.87.1", crate, [normal, dev])
            manifest.write_text(
                '[dependencies]\ngix-hash = { path = "../gix-hash", version = "^0.26.3", registry = "elohim" }\n'
                '[target.\'cfg(unix)\'.dev-dependencies]\n'
                'gix-hash = { path = "../gix-hash", version = "*" }\n'
            )
            with self.assertRaisesRegex(publish_brit.PublishError, "canonical source.*gix-hash"):
                publish_brit.validate_canonical_sources(item)
            manifest.write_text(
                '[dependencies]\ngix-hash = { path = "../gix-hash", version = "^0.26.3", registry = "elohim" }\n'
                '[dev-dependencies]\ngix-hash = { path = "../gix-hash" }\n'
            )
            publish_brit.validate_canonical_sources(item)
            manifest.write_text(
                '[dependencies]\ngix-hash = { path = "../gix-hash", version = "^0.26.3", registry = "elohim" }\n'
                '[dev-dependencies]\ngix-hash = { path = "../gix-hash", version = "^0.26.3", registry = "elohim" }\n'
            )
            publish_brit.validate_canonical_sources(item)
            manifest.write_text(
                '[dependencies]\ngix-hash = { path = "../gix-hash", version = "^0.26.3", registry = "elohim" }\n'
                '[dev-dependencies]\ngix-hash = { workspace = true }\n'
            )
            with self.assertRaisesRegex(publish_brit.PublishError, "workspace-inherited local dependency source"):
                publish_brit.validate_canonical_sources(item)

    def test_package_command_is_locked_and_uses_elohim_registry(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            archive = root / "target/package/brit-cli-0.1.2.crate"
            archive.parent.mkdir(parents=True)
            archive.write_bytes(b"test package")
            package_data = package("brit-cli", "0.1.2", root / "cli")
            with mock.patch.object(publish_brit.subprocess, "run") as cargo_run:
                found, checksum = publish_brit.package_crate(root, package_data, {"CARGO_TARGET_DIR": str(root / "target")})
            self.assertEqual(found, archive)
            self.assertEqual(len(checksum), 64)
            self.assertEqual(cargo_run.call_args.args[0], [
                "cargo", "package", "--locked", "--registry", "elohim", "--no-verify", "-p", "brit-cli",
            ])


class RegistryTests(unittest.TestCase):
    def setUp(self):
        self.registry = FakeRegistry()

    def tearDown(self):
        self.registry.close()

    def test_same_version_different_payload_is_collision_not_success(self):
        path = publish_brit.sparse_path("brit-cli")
        self.registry.records["/" + path] = [{
            "name": "brit-cli", "vers": "0.1.1", "cksum": "0" * 64,
        }]
        with self.assertRaisesRegex(publish_brit.PublishError, "immutable.*collision"):
            publish_brit.decide(self.registry.url, "brit-cli", "0.1.1", "1" * 64, package("brit-cli", "0.1.1", Path("/fake")))

    def test_index_alias_and_feature_contract_catches_nexus_name_loss(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            item = package("gix-diff", "0.67.1", root / "diff", [
                dependency("gix-imara-diff", root / "imara", "elohim", optional=True, rename="imara-diff"),
            ], features={"blob": ["dep:imara-diff"]})
            good = {"deps": [{"name": "imara-diff", "package": "gix-imara-diff", "kind": "normal", "optional": True}],
                    "features": {"blob": ["dep:imara-diff"]}}
            publish_brit.validate_index_contract(good, item)
            with self.assertRaisesRegex(publish_brit.PublishError, "invalid index features"):
                publish_brit.validate_index_contract(
                    {**good, "features": {"blob": ["dep:imara-diff", 42]}}, item
                )
            lost_alias = {"deps": [{"name": "gix-imara-diff", "kind": "normal", "optional": True}],
                          "features": {"blob": ["dep:imara-diff"]}}
            with self.assertRaisesRegex(publish_brit.PublishError, "index dependency.*imara-diff"):
                publish_brit.validate_index_contract(lost_alias, item)
            wrong_feature = {"deps": good["deps"], "features": {"blob": ["dep:gix-imara-diff"]}}
            with self.assertRaisesRegex(publish_brit.PublishError, "index feature.*gix-imara-diff"):
                publish_brit.validate_index_contract(wrong_feature, item)
            missing_feature = {"deps": good["deps"], "features": {}}
            with self.assertRaisesRegex(publish_brit.PublishError, "missing index feature.*blob"):
                publish_brit.validate_index_contract(missing_feature, item)
            missing_link = {"deps": good["deps"], "features": {"blob": []}}
            with self.assertRaisesRegex(publish_brit.PublishError, "missing index dependency feature link.*blob"):
                publish_brit.validate_index_contract(missing_link, item)
            not_optional = {"deps": [{**good["deps"][0], "optional": False}], "features": good["features"]}
            with self.assertRaisesRegex(publish_brit.PublishError, "index dependency.*imara-diff"):
                publish_brit.validate_index_contract(not_optional, item)

    def test_required_alias_is_checked_but_dev_alias_is_not_required(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            item = package("gitoxide-core", "0.61.1", root / "core", [
                dependency("gix-pack", root / "pack", "elohim", rename="gix-pack-for-configuration-only"),
                dependency("gix-archive", root / "archive", "elohim", kind="dev", rename="archive-test"),
            ])
            bad = {"deps": [{"name": "gix-pack", "kind": "normal"}], "features": {}}
            with self.assertRaisesRegex(publish_brit.PublishError, "index dependency.*gix-pack-for-configuration-only"):
                publish_brit.validate_index_contract(bad, item)
            good = {"deps": [{"name": "gix-pack-for-configuration-only", "package": "gix-pack", "kind": "normal"}],
                    "features": {}}
            publish_brit.validate_index_contract(good, item)

    def test_unrenamed_dependency_accepts_omitted_or_explicit_self_package(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            item = package("gix-diff", "0.67.2", root / "diff", [
                dependency("gix-imara-diff", root / "imara", "elohim", optional=True),
            ])
            for entry in (
                {"name": "gix-imara-diff", "optional": True},
                {"name": "gix-imara-diff", "package": "gix-imara-diff", "optional": True},
            ):
                with self.subTest(entry=entry):
                    publish_brit.validate_index_contract(
                        {"deps": [entry], "features": {"blob": ["dep:gix-imara-diff"]}}, item
                    )

    def test_implicit_optional_feature_may_be_omitted_but_explicit_one_may_not(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            manifest = root / "Cargo.toml"
            item = package("gix-error", "0.3.2", root, [
                dependency("document-features", root / "document-features", "elohim", optional=True),
            ], features={"document-features": ["dep:document-features"]})
            index = {"deps": [{"name": "document-features", "optional": True}], "features": {}}
            manifest.write_text('[package]\nname = "gix-error"\nversion = "0.3.2"\n')
            publish_brit.validate_index_contract(index, item)
            manifest.write_text('[features]\ndocument-features = ["dep:document-features"]\n')
            with self.assertRaisesRegex(publish_brit.PublishError, "missing index feature.*document-features"):
                publish_brit.validate_index_contract(index, item)

    def test_matching_archive_cannot_skip_a_broken_index_entry(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            item = package("gix-diff", "0.67.1", root / "diff", [
                dependency("gix-imara-diff", root / "imara", "elohim", optional=True, rename="imara-diff"),
            ])
            archive = crate_archive("gix-diff", "0.67.1", {"Cargo.toml": b"manifest"})
            checksum = publish_brit.hashlib.sha256(archive).hexdigest()
            self.registry.records["/" + publish_brit.sparse_path("gix-diff")] = [{
                "name": "gix-diff", "vers": "0.67.1", "cksum": checksum,
                "deps": [{"name": "gix-imara-diff", "kind": "normal", "optional": True}],
                "features": {"blob": ["dep:imara-diff"]},
            }]
            self.registry.archives["/crates/gix-diff/0.67.1/download"] = archive
            with self.assertRaisesRegex(publish_brit.PublishError, "index dependency.*imara-diff"):
                publish_brit.existing_decision(self.registry.url, "gix-diff", "0.67.1", archive, item)

    def test_successful_upload_with_broken_index_has_no_verified_receipt(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            (root / ".cargo").mkdir()
            (root / ".cargo/config.toml").write_text(
                '[registries.elohim]\nindex = "sparse+https://example.invalid/"\n'
            )
            item = package("brit-cli", "0.1.2", root / "cli", [{
                "name": "gix-imara-diff", "path": None, "registry": None,
                "kind": None, "optional": True, "rename": "imara-diff",
            }])
            metadata = {"packages": [item, package("brit-build-ref", "0.1.1", root / "build-ref")]}
            uploads = []

            def package_one(_repo, package_data, _env):
                archive = root / f"{package_data['name']}.crate"
                archive.write_bytes(package_data["name"].encode())
                return archive, publish_brit.hashlib.sha256(archive.read_bytes()).hexdigest()

            def publish_one(command, **_kwargs):
                name = command[-1]
                uploads.append(name)
                checksum = publish_brit.hashlib.sha256(name.encode()).hexdigest()
                self.registry.records["/" + publish_brit.sparse_path(name)] = [{
                    "name": name, "vers": "0.1.2", "cksum": checksum,
                    "deps": [{"name": "gix-imara-diff", "kind": "normal", "optional": True}],
                    "features": {"blob": ["dep:imara-diff"]},
                }]
                return mock.Mock(returncode=0)

            output = io.StringIO()
            with (
                mock.patch.object(publish_brit, "validate_registry_url", return_value=self.registry.url),
                mock.patch.object(publish_brit, "assert_download_endpoint"),
                mock.patch.object(publish_brit, "cargo_metadata", return_value=metadata),
                mock.patch.object(publish_brit, "package_crate", side_effect=package_one),
                mock.patch.object(publish_brit.subprocess, "run", side_effect=publish_one),
                mock.patch.dict("os.environ", {
                    "CARGO_TARGET_DIR": str(root / "target"),
                    "CARGO_REGISTRIES_ELOHIM_TOKEN": "test-token",
                }),
                contextlib.redirect_stdout(output),
            ):
                with self.assertRaisesRegex(publish_brit.PublishError, "index dependency.*imara-diff"):
                    publish_brit.run(root, publish=True)
            self.assertEqual(uploads, ["brit-cli"])
            self.assertNotIn("verified:", output.getvalue())

    def test_exact_checksum_skips_and_missing_version_publishes(self):
        path = publish_brit.sparse_path("brit-cli")
        self.registry.records["/" + path] = [{
            "name": "brit-cli", "vers": "0.1.1", "cksum": "1" * 64,
        }]
        self.assertEqual(publish_brit.decide(self.registry.url, "brit-cli", "0.1.1", "1" * 64, package("brit-cli", "0.1.1", Path("/fake"))), "skip")
        self.assertEqual(publish_brit.decide(self.registry.url, "brit-cli", "0.1.2", "2" * 64, package("brit-cli", "0.1.2", Path("/fake"))), "publish")

    def test_index_audit_counts_exact_versions_without_packaging(self):
        self.registry.records["/" + publish_brit.sparse_path("brit-cli")] = [{
            "name": "brit-cli", "vers": "0.1.1", "cksum": "1" * 64,
        }]
        ordered = [
            {"name": "brit-cli", "version": "0.1.1"},
            {"name": "gitoxide", "version": "0.58.0"},
        ]
        self.assertEqual(publish_brit.audit_index(self.registry.url, ordered), (1, 1))

    def test_auth_or_server_error_is_not_absence(self):
        for status in (401, 403, 500):
            self.registry.status = status
            with self.subTest(status=status), self.assertRaises(publish_brit.PublishError):
                publish_brit.decide(self.registry.url, "brit-cli", "0.1.2", "2" * 64, package("brit-cli", "0.1.2", Path("/fake")))

    def test_registry_url_rejects_embedded_credentials_without_echoing_them(self):
        with self.assertRaises(publish_brit.PublishError) as result:
            publish_brit.validate_registry_url("sparse+https://secret@example.invalid/index?token=secret")
        self.assertNotIn("secret", str(result.exception))

    def test_download_endpoint_must_match_nexus_config(self):
        self.registry.archives["/config.json"] = json.dumps({"dl": self.registry.url + "crates"}).encode()
        publish_brit.assert_download_endpoint(self.registry.url)
        self.registry.archives["/config.json"] = b'{"dl":"https://example.invalid/wrong"}'
        with self.assertRaisesRegex(publish_brit.PublishError, "download endpoint differs"):
            publish_brit.assert_download_endpoint(self.registry.url)

    def test_only_vcs_receipt_may_differ_in_existing_payload(self):
        base = {"Cargo.toml": b"[package]", "Cargo.toml.orig": b"[package]", "Cargo.lock": b"lock", "build.rs": b"fn main() {}", "src/main.rs": b"fn main() {}", ".cargo_vcs_info.json": b'{"sha1":"old"}'}
        old = crate_archive("brit-cli", "0.1.1", base)
        changed = dict(base, **{".cargo_vcs_info.json": b'{"sha1":"new"}'})
        new = crate_archive("brit-cli", "0.1.1", changed)
        self.assertTrue(publish_brit.payloads_equivalent(old, new, "brit-cli", "0.1.1"))
        for path in ("Cargo.toml", "Cargo.toml.orig", "Cargo.lock", "build.rs", "src/main.rs"):
            with self.subTest(path=path):
                altered = dict(changed, **{path: b"different"})
                self.assertFalse(publish_brit.payloads_equivalent(
                    old, crate_archive("brit-cli", "0.1.1", altered), "brit-cli", "0.1.1"
                ))

    def test_existing_version_download_is_checksum_verified_before_equivalence(self):
        old = crate_archive("brit-cli", "0.1.1", {
            "Cargo.toml": b"manifest", "Cargo.lock": b"lock", ".cargo_vcs_info.json": b"old",
        })
        new = crate_archive("brit-cli", "0.1.1", {
            "Cargo.toml": b"manifest", "Cargo.lock": b"lock", ".cargo_vcs_info.json": b"new",
        })
        checksum = publish_brit.hashlib.sha256(old).hexdigest()
        self.registry.records["/" + publish_brit.sparse_path("brit-cli")] = [{
            "name": "brit-cli", "vers": "0.1.1", "cksum": checksum,
        }]
        archive_path = "/crates/brit-cli/0.1.1/download"
        self.registry.archives[archive_path] = old
        self.assertEqual(publish_brit.existing_decision(self.registry.url, "brit-cli", "0.1.1", new, package("brit-cli", "0.1.1", Path("/fake"))), "skip-equivalent")
        self.registry.archives[archive_path] = b"tampered"
        with self.assertRaisesRegex(publish_brit.PublishError, "checksum mismatch"):
            publish_brit.existing_decision(self.registry.url, "brit-cli", "0.1.1", new, package("brit-cli", "0.1.1", Path("/fake")))

    def test_executable_mode_and_unsafe_archive_entries_refuse(self):
        files = {"Cargo.toml": b"manifest", "src/main.rs": b"main"}
        old = crate_archive("brit-cli", "0.1.1", files)
        executable = crate_archive("brit-cli", "0.1.1", files, executable=("src/main.rs",))
        self.assertFalse(publish_brit.payloads_equivalent(old, executable, "brit-cli", "0.1.1"))
        restricted = crate_archive("brit-cli", "0.1.1", files, modes={"src/main.rs": 0o600})
        self.assertFalse(publish_brit.payloads_equivalent(old, restricted, "brit-cli", "0.1.1"))
        extra_directory = crate_archive("brit-cli", "0.1.1", files, directories=("empty",))
        self.assertFalse(publish_brit.payloads_equivalent(old, extra_directory, "brit-cli", "0.1.1"))
        unsafe = crate_archive("brit-cli", "0.1.1", files, symlink="src/link")
        with self.assertRaisesRegex(publish_brit.PublishError, "unsafe archive"):
            publish_brit.payloads_equivalent(old, unsafe, "brit-cli", "0.1.1")

    def test_failed_upload_must_refresh_to_equivalent_verified_payload(self):
        path = publish_brit.sparse_path("brit-cli")
        self.registry.records["/" + path] = [{
            "name": "brit-cli", "vers": "0.1.2", "cksum": "2" * 64,
        }]
        remote = crate_archive("brit-cli", "0.1.2", {"Cargo.toml": b"manifest", "src/main.rs": b"main"})
        altered = crate_archive("brit-cli", "0.1.2", {"Cargo.toml": b"changed", "src/main.rs": b"main"})
        with mock.patch.object(publish_brit, "remote_archive", return_value=remote):
            publish_brit.verify_conflict(self.registry.url, "brit-cli", "0.1.2", remote, package("brit-cli", "0.1.2", Path("/fake")))
            with self.assertRaisesRegex(publish_brit.PublishError, "immutable.*collision"):
                publish_brit.verify_conflict(self.registry.url, "brit-cli", "0.1.2", altered, package("brit-cli", "0.1.2", Path("/fake")))

    def test_whole_graph_static_preflight_refuses_before_any_upload(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            (root / ".cargo").mkdir()
            (root / ".cargo/config.toml").write_text(
                f'[registries.elohim]\nindex = "sparse+{self.registry.url}"\n'
            )
            registry = "sparse+" + self.registry.url
            metadata = {"packages": [
                package("brit-cli", "0.1.2", root / "cli", [
                    dependency("gitoxide", root / "engine", None),
                ]),
                package("gitoxide", "0.58.0", root / "engine"),
                package("brit-build-ref", "0.1.1", root / "build-ref"),
            ]}
            with (
                mock.patch.object(publish_brit, "validate_registry_url", return_value=self.registry.url),
                mock.patch.object(publish_brit, "assert_download_endpoint"),
                mock.patch.object(publish_brit, "cargo_metadata", return_value=metadata),
                mock.patch.object(publish_brit, "package_crate") as packager,
                mock.patch.object(publish_brit.subprocess, "run") as cargo_run,
                mock.patch.dict("os.environ", {
                    "CARGO_TARGET_DIR": str(root / "target"),
                    "CARGO_REGISTRIES_ELOHIM_INDEX": registry,
                    "CARGO_REGISTRIES_ELOHIM_TOKEN": "test-token",
                }),
            ):
                with self.assertRaisesRegex(publish_brit.PublishError, "registry=elohim"):
                    publish_brit.run(root, publish=True)
                packager.assert_not_called()
                cargo_run.assert_not_called()

    def test_mixed_explicit_dependency_sources_refuse_before_any_package_or_upload(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            (root / ".cargo").mkdir()
            (root / ".cargo/config.toml").write_text(
                '[registries.elohim]\nindex = "sparse+https://example.invalid/"\n'
            )
            (root / "engine").mkdir()
            (root / "engine/Cargo.toml").write_text(
                '[dependencies]\ngix-hash = { path = "../hash", version = "^0.26.3", registry = "elohim" }\n'
                '[dev-dependencies]\ngix-hash = { path = "../hash", version = "^0.26.2" }\n'
            )
            registry = "sparse+https://example.invalid/"
            metadata = {"packages": [
                package("brit-cli", "0.1.2", root / "cli", [
                    dependency("gitoxide", root / "engine", registry),
                ]),
                package("gitoxide", "0.58.0", root / "engine", [
                    dependency("gix-hash", root / "hash", registry),
                    dependency("gix-hash", root / "hash", None, kind="dev"),
                ]),
                package("gix-hash", "0.26.3", root / "hash"),
                package("brit-build-ref", "0.1.1", root / "build-ref"),
            ]}
            with (
                mock.patch.object(publish_brit, "validate_registry_url", return_value=self.registry.url),
                mock.patch.object(publish_brit, "assert_download_endpoint"),
                mock.patch.object(publish_brit, "cargo_metadata", return_value=metadata),
                mock.patch.object(publish_brit, "package_crate") as packager,
                mock.patch.object(publish_brit.subprocess, "run") as cargo_run,
                mock.patch.dict("os.environ", {
                    "CARGO_TARGET_DIR": str(root / "target"),
                    "CARGO_REGISTRIES_ELOHIM_INDEX": registry,
                    "CARGO_REGISTRIES_ELOHIM_TOKEN": "test-token",
                }),
            ):
                with self.assertRaisesRegex(publish_brit.PublishError, "canonical source.*gix-hash"):
                    publish_brit.run(root, publish=True)
                packager.assert_not_called()
                cargo_run.assert_not_called()

    def test_publish_interleaves_package_and_upload_in_topological_order(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            (root / ".cargo").mkdir()
            (root / ".cargo/config.toml").write_text(
                '[registries.elohim]\nindex = "sparse+https://example.invalid/"\n'
            )
            registry = "sparse+https://example.invalid/"
            metadata = {"packages": [
                package("brit-cli", "0.1.2", root / "cli", [
                    dependency("gitoxide", root / "engine", registry),
                ]),
                package("gitoxide", "0.58.0", root / "engine"),
                package("brit-build-ref", "0.1.1", root / "build-ref"),
            ]}
            events = []

            def package_one(_repo, item, _env):
                events.append(("package", item["name"]))
                archive = root / f"{item['name']}.crate"
                archive.write_bytes(item["name"].encode())
                return archive, publish_brit.hashlib.sha256(archive.read_bytes()).hexdigest()

            def publish_one(command, **_kwargs):
                self.assertIn("--locked", command)
                events.append(("publish", command[-1]))
                return mock.Mock(returncode=0)

            with (
                mock.patch.object(publish_brit, "assert_download_endpoint"),
                mock.patch.object(publish_brit, "cargo_metadata", return_value=metadata),
                mock.patch.object(publish_brit, "package_crate", side_effect=package_one),
                mock.patch.object(publish_brit, "existing_decision", return_value="publish"),
                mock.patch.object(publish_brit, "decide", return_value="skip"),
                mock.patch.object(publish_brit, "remote_archive", return_value=b"remote"),
                mock.patch.object(publish_brit.subprocess, "run", side_effect=publish_one),
                mock.patch.dict("os.environ", {
                    "CARGO_TARGET_DIR": str(root / "target"),
                    "CARGO_REGISTRIES_ELOHIM_INDEX": registry,
                    "CARGO_REGISTRIES_ELOHIM_TOKEN": "test-token",
                }),
            ):
                publish_brit.run(root, publish=True)
            self.assertEqual(events, [
                ("package", "gitoxide"), ("publish", "gitoxide"),
                ("package", "brit-cli"), ("publish", "brit-cli"),
                ("package", "brit-build-ref"), ("publish", "brit-build-ref"),
            ])

    def test_check_mode_never_invokes_cargo_publish(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            (root / ".cargo").mkdir()
            (root / ".cargo/config.toml").write_text(
                '[registries.elohim]\nindex = "sparse+https://example.invalid/"\n'
            )
            metadata = {"packages": [
                package("brit-cli", "0.1.2", root / "cli"),
                package("brit-build-ref", "0.1.1", root / "build-ref"),
            ]}
            (root / "archive").write_bytes(b"archive")
            with (
                mock.patch.object(publish_brit, "assert_download_endpoint"),
                mock.patch.object(publish_brit, "cargo_metadata", return_value=metadata),
                mock.patch.object(publish_brit, "package_crate", return_value=(root / "archive", "1" * 64)),
                mock.patch.object(publish_brit, "existing_decision", return_value="publish"),
                mock.patch.object(publish_brit.subprocess, "run") as cargo_run,
                mock.patch.dict("os.environ", {
                    "CARGO_TARGET_DIR": str(root / "target"),
                    "CARGO_REGISTRIES_ELOHIM_INDEX": "sparse+https://example.invalid/",
                }),
            ):
                publish_brit.run(root, publish=False)
                cargo_run.assert_not_called()


if __name__ == "__main__":
    unittest.main()
