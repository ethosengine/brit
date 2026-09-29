#!/usr/bin/env python3
"""Small pinned source fixtures, plus a real-pin smoke test when vendor Git is present."""
import importlib.util
import json
import os
from pathlib import Path
import subprocess
import tempfile
import unittest

HERE = Path(__file__).resolve().parent
spec = importlib.util.spec_from_file_location("census", HERE / "census.py")
census = importlib.util.module_from_spec(spec)
spec.loader.exec_module(census)


class SourceFixture:
    def __init__(self):
        self.temp = tempfile.TemporaryDirectory()
        self.root = Path(self.temp.name)
        self.git("init", "-q")
        self.git("config", "user.email", "fixture@example.invalid")
        self.git("config", "user.name", "Fixture")
        self.write("command-list.txt", "git-demo mainporcelain worktree\ngitguide guide\n")
        self.write("Documentation/git-demo.adoc", "OPTIONS\n-------\n--demo::\n Description.\n")
        self.write("Documentation/gitguide.adoc", "topic::\n Topic.\n")
        self.write("Documentation/git.adoc", "OPTIONS\n-------\n--global::\n Global.\n")

    def git(self, *args):
        env = {k: v for k, v in os.environ.items() if not k.startswith("GIT_")}
        env.update(GIT_CONFIG_NOSYSTEM="1", GIT_CONFIG_GLOBAL=os.devnull)
        return subprocess.check_output(["git", "-c", "core.hooksPath=" + os.devnull, "-C", str(self.root), *args], env=env, stderr=subprocess.PIPE).decode().strip()

    def write(self, path, text):
        destination = self.root / path
        destination.parent.mkdir(parents=True, exist_ok=True)
        destination.write_text(text)

    def commit(self):
        self.git("add", "--", "command-list.txt", "Documentation")
        self.git("-c", "commit.gpgsign=false", "commit", "-qm", "fixture")
        return self.git("rev-parse", "HEAD")

    def build(self, revision=None):
        return census.build_census(self.root, ".", {"revision": revision or self.commit(), "label": "fixture"})

    def close(self):
        self.temp.cleanup()


class CensusTests(unittest.TestCase):
    def setUp(self):
        self.fixture = SourceFixture()
        self.addCleanup(self.fixture.close)

    def test_pinned_bytes_ignore_worktree_and_later_commits(self):
        revision = self.fixture.commit()
        first = self.fixture.build(revision)
        self.fixture.write("Documentation/git-demo.adoc", "--changed::\n Different.\n")
        self.assertEqual(first, self.fixture.build(revision))
        self.fixture.commit()
        self.assertEqual(first, self.fixture.build(revision))

    def test_replacement_refs_cannot_rewrite_the_claimed_reference(self):
        revision = self.fixture.commit()
        first = self.fixture.build(revision)
        self.fixture.write("Documentation/git-demo.adoc", "--substituted::\n Different.\n")
        replacement = self.fixture.commit()
        self.fixture.git("replace", revision, replacement)
        self.assertEqual(first, self.fixture.build(revision))

    def test_alias_groups_includes_and_source_locations(self):
        self.fixture.write("Documentation/git-demo.adoc", "OPTIONS\n-------\ninclude::shared.adoc[]\ninclude::shared.adoc[]\n")
        self.fixture.write("Documentation/shared.adoc", "-q::\n--quiet::\n Silent.\n\n--[no-]color::\n Color.\n")
        result = self.fixture.build()
        groups = result["commands"][0]["terms"]
        self.assertEqual(len(groups), 2)
        quiet = next(term for term in groups if "--quiet" in term["option_names"])
        self.assertEqual(quiet["labels"], ["-q", "--quiet"])
        self.assertEqual(quiet["option_names"], ["--quiet", "-q"])
        self.assertEqual({s["line"] for s in quiet["sources"]}, {1, 2})
        self.assertEqual({s["via"][0]["line"] for s in quiet["sources"]}, {3, 4})
        color = next(term for term in groups if "--color" in term["option_names"])
        self.assertEqual(color["option_names"], ["--color", "--no-color"])

    def test_option_names_do_not_parse_values_or_destroy_underscores(self):
        self.assertEqual(census.option_names(["--foo=<foo-bar>"]), ["--foo"])
        self.assertEqual(census.option_names(["`--show_linear_break[=<barrier>]`"]), ["--show_linear_break"])
        self.assertEqual(census.option_names(["--foo, --bar=<value-with-hyphens>"]), ["--bar", "--foo"])
        self.assertEqual(census.option_names(["-L<start>,<end>:<file>"]), ["-L"])
        self.assertEqual(census.option_names(["<path-with-hyphens>"]), [])
        self.assertEqual(census.option_names(["--<dynamic>"]), [])

    def test_definition_identity_survives_line_movement(self):
        first = self.fixture.build()["commands"][0]["terms"][0]
        self.fixture.write("Documentation/git-demo.adoc", "\n\nOPTIONS\n-------\n--demo::\n Description.\n")
        later = self.fixture.build()["commands"][0]["terms"][0]
        self.assertEqual(first["id"], later["id"])
        self.assertNotEqual(first["sources"][0]["line"], later["sources"][0]["line"])

    def test_cycles_traversal_missing_and_selective_includes_are_diagnostics(self):
        self.fixture.write("Documentation/git-demo.adoc", "include::a.adoc[]\ninclude::missing.adoc[]\ninclude::../../outside.adoc[]\ninclude::{generated}/x.adoc[]\ninclude::a.adoc[lines=2..3]\n")
        self.fixture.write("Documentation/a.adoc", "include::git-demo.adoc[]\n--included::\n Included.\n")
        result = self.fixture.build()
        codes = {item["code"] for item in result["diagnostics"]}
        self.assertTrue({"include-cycle", "missing-include", "include-outside-documentation", "unresolved-include-target", "unsupported-include-attributes"}.issubset(codes))
        self.assertEqual(result["commands"][0]["terms"][0]["option_names"], ["--included"])

    def test_guards_remain_explicit_and_never_claim_universal_applicability(self):
        self.fixture.write("Documentation/git-demo.adoc", "ifdef::platform[]\n--platform::\n Optional.\nendif::platform[]\nifndef::other[--inline::]\nifeval::[1 == 2]\n--evaluated::\n Optional.\nendif::[]\n--plain::\n Plain.\n")
        rows = self.fixture.build()["commands"][0]["terms"]
        plain = next(term for term in rows if "--plain" in term["option_names"])
        self.assertEqual(plain["applicability"], "documented-unconditional")
        for term in rows:
            if term is not plain:
                self.assertEqual(term["applicability"], "conditional-unresolved")
                self.assertTrue(term["sources"][0]["conditions"])

    def test_all_definition_terms_and_nested_options_survive_while_literal_examples_do_not(self):
        self.fixture.write("Documentation/git-demo.adoc", "COMMANDS\n--------\nrun::\n Run work.\n  --nested::\n    An option.\nsettings.value::\n Setting.\nkind;;\n Another definition delimiter.\n+\n----\n--example::\n----\n[source,shell]\n----\n--also-example::\n----\n////\n--comment::\n////\n--after::\n Real option.\n")
        terms = self.fixture.build()["commands"][0]["terms"]
        labels = {label for term in terms for label in term["labels"]}
        self.assertEqual(labels, {"run", "--nested", "settings.value", "kind", "--after"})
        self.assertEqual(next(term for term in terms if term["labels"] == ["run"])["kind"], "nested-verb")

    def test_directive_examples_inside_literal_blocks_are_not_expanded(self):
        self.fixture.write("Documentation/git-demo.adoc", "[source]\n----\ninclude::missing-example.adoc[]\nifdef::example[]\n--example::\n----\n   ....\n--real::\n Real.\n")
        result = self.fixture.build()
        self.assertEqual(result["diagnostics"], [])
        self.assertEqual(result["commands"][0]["terms"][0]["option_names"], ["--real"])

    def test_missing_docs_never_remove_commands_and_global_interface_is_separate(self):
        self.fixture.write("command-list.txt", "git-missing plumbingmanipulators\ngitguide guide\n")
        result = self.fixture.build()
        self.assertEqual(result["counts"]["commandCount"], 1)
        self.assertEqual(result["commands"][0]["name"], "missing")
        self.assertEqual(result["commands"][0]["terms"], [])
        self.assertEqual({row["name"] for row in result["interfaces"]}, {"gitguide", "git"})
        self.assertIn("missing-document", {row["code"] for row in result["diagnostics"]})

    def test_check_mode_is_nonwriting_and_deterministic(self):
        revision = self.fixture.commit()
        self.fixture.write(census.REFERENCE_PATH, json.dumps({"revision": revision}))
        args = ["--root", str(self.fixture.root), "--git-dir", "."]
        self.assertEqual(census.main([*args, "--check"]), 1)
        self.assertFalse((self.fixture.root / census.JSON_PATH).exists())
        self.assertEqual(census.main(args), 0)
        before = {rel: ((self.fixture.root / rel).read_bytes(), (self.fixture.root / rel).stat().st_mtime_ns) for rel in (census.JSON_PATH, census.MARKDOWN_PATH)}
        self.assertEqual(census.main([*args, "--check"]), 0)
        for rel, expected in before.items():
            self.assertEqual(((self.fixture.root / rel).read_bytes(), (self.fixture.root / rel).stat().st_mtime_ns), expected)

    def test_mutable_refs_and_duplicate_declarations_refuse(self):
        self.fixture.commit()
        with self.assertRaises(census.CensusError):
            self.fixture.build("HEAD")
        self.fixture.write("command-list.txt", "git-demo mainporcelain\ngit-demo plumbingmanipulators\n")
        with self.assertRaises(census.CensusError):
            self.fixture.build()


class RealPinTests(unittest.TestCase):
    def test_reference_inventory_contains_inherited_and_nested_options(self):
        root = HERE.parents[1]
        reference = json.loads((root / census.REFERENCE_PATH).read_text())
        if not (root / "vendor/git").exists():
            self.skipTest("pinned Git object checkout unavailable")
        result = census.build_census(root, "vendor/git", reference)
        self.assertEqual(result["counts"]["commandCount"], 159)
        for name, expected in (("log", {"--since", "--max-count", "--format"}), ("multi-pack-index", {"--preferred-pack", "--stdin-packs", "--refs-snapshot", "--incremental"})):
            command = next(row for row in result["commands"] if row["name"] == name)
            options = {option for term in command["terms"] for option in term["option_names"]}
            self.assertTrue(expected <= options, f"{name} misses {expected - options}")


if __name__ == "__main__":
    unittest.main()
