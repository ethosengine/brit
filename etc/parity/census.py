#!/usr/bin/env python3
"""Pinned Git documentation census. Inventory keys are local anchors, never protocol CIDs.

Definition groups are units, not claims about complete behavior. Adjacent definition
labels are aliases of one group; repeated includes merge occurrences by source path
and normalized labels. Shared groups retain the same ID across commands. Option names
are syntactic aliases, including both forms of --[no-]foo. Conditional branches are
retained with their guards rather than being silently treated as universally available.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import posixpath
import re
import subprocess
import sys

SCHEMA_VERSION = 1
REFERENCE_PATH = "docs/parity/git-reference.json"
JSON_PATH = "docs/parity/git-census.json"
MARKDOWN_PATH = "docs/parity/GIT-CENSUS.md"
NONCOMMAND = {"guide", "userinterfaces", "developerinterfaces"}
DIRECTIVE = re.compile(r"^(include|ifdef|ifndef|ifeval|endif)::(.*?)\[(.*)\]\s*$")
DEFINITION = re.compile(r"^\s*(\S.*?)\s*(:{2,4}|;;)(?:\s+(.*))?$")
OPTION = re.compile(r"(?:^|(?<=[\s,|]))(--(?:\[no-\])?[A-Za-z0-9][A-Za-z0-9_-]*|-[A-Za-z0-9?])")


class CensusError(ValueError):
    pass


class PinnedTree:
    """Read object bytes only, with inherited Git routing disabled."""
    def __init__(self, git_dir: Path, revision: str):
        self.git_dir = git_dir
        self.revision = revision
        self.cache = {}
        if not re.fullmatch(r"[0-9a-f]{40}|[0-9a-f]{64}", revision):
            raise CensusError("reference revision must be a full immutable object ID")
        actual = self.git("rev-parse", "--verify", f"{revision}^{{commit}}").decode().strip()
        if actual != revision:
            raise CensusError("reference must name the commit itself, not a tag object")
        self.paths = set(self.git("ls-tree", "-r", "--name-only", "-z", revision).decode().split("\0"))

    def git(self, *args):
        env = {k: v for k, v in os.environ.items() if not k.startswith("GIT_")}
        proc = subprocess.run(["git", "--no-replace-objects", "-C", str(self.git_dir), *args], env=env,
                              stdout=subprocess.PIPE, stderr=subprocess.PIPE, check=False)
        if proc.returncode:
            raise CensusError(proc.stderr.decode(errors="replace").strip())
        return proc.stdout

    def read(self, path):
        if path not in self.paths:
            return None
        if path not in self.cache:
            try:
                self.cache[path] = self.git("show", f"{self.revision}:{path}").decode("utf-8")
            except UnicodeDecodeError as exc:
                raise CensusError(f"{path}: reference documentation is not UTF-8") from exc
        return self.cache[path]


def option_names(labels):
    names = set()
    for label in labels:
        # Only a definition beginning with '-' declares options. Prose mentioning
        # another command's option remains a definition, not an extra option claim.
        clean = label.replace("`", "").replace("*", "").strip().strip("_\'")
        spelling = clean
        clean = re.sub(r"<[^>]*>", "", clean)
        if not clean.startswith("-"):
            continue
        if spelling == "--":
            names.add("--")
        for name in OPTION.findall(clean):
            if name.startswith("--[no-]"):
                names.update(("--" + name[7:], "--no-" + name[7:]))
            else:
                names.add(name)
    return sorted(names)


class Extractor:
    def __init__(self, tree):
        self.tree = tree
        self.diagnostics = []

    def diagnostic(self, code, path, line, detail):
        self.diagnostics.append({"code": code, "path": path, "line": line, "detail": detail})

    def expanded(self, path, stack=(), inherited=(), via=()):
        if path in stack:
            self.diagnostic("include-cycle", path, 0, " -> ".join((*stack, path)))
            return
        text = self.tree.read(path)
        if text is None:
            self.diagnostic("missing-document", path, 0, "not present in the pinned tree")
            return
        conditions = list(inherited)
        floor = len(conditions)
        literal = None
        lines = text.splitlines()
        for number, line in enumerate(lines, 1):
            stripped = line.strip()
            source = {"path": path, "line": number, "conditions": list(conditions), "via": list(via)}
            if literal:
                yield line, source
                if line == literal:
                    literal = None
                continue
            previous = lines[number - 2] if number > 1 else ""
            heading = bool(re.fullmatch(r"-{4,}", stripped)) and bool(re.match(r"^[A-Za-z0-9]", previous)) and not previous.endswith(".") and "::" not in previous
            if line == stripped and re.fullmatch(r"(-{4,}|\.{4,}|/{4,}|\+{4,})", stripped) and not heading:
                literal = stripped
                yield line, source
                continue
            match = DIRECTIVE.match(stripped)
            if match:
                kind, target, body = match.groups()
                if kind == "include":
                    if body and not re.fullmatch(r"leveloffset=[+-]?\d+", body):
                        self.diagnostic("unsupported-include-attributes", path, number, line)
                        continue
                    if "{" in target or ":" in target or target.startswith("/"):
                        self.diagnostic("unresolved-include-target", path, number, target)
                        continue
                    child = posixpath.normpath(posixpath.join(posixpath.dirname(path), target))
                    if child.startswith("../") or child == ".." or not child.startswith("Documentation/"):
                        self.diagnostic("include-outside-documentation", path, number, target)
                        continue
                    if child not in self.tree.paths:
                        self.diagnostic("missing-include", path, number, child)
                        continue
                    provenance = {"path": path, "line": number, "target": child}
                    yield from self.expanded(child, (*stack, path), tuple(conditions), (*via, provenance))
                elif kind == "endif":
                    if len(conditions) == floor:
                        self.diagnostic("unmatched-endif", path, number, line)
                    else:
                        previous = conditions.pop()
                        if target and target != previous["expression"]:
                            self.diagnostic("mismatched-endif", path, number, line)
                else:
                    expression = body if kind == "ifeval" else target
                    guard = {"kind": kind, "expression": expression, "path": path, "line": number}
                    self.diagnostic("conditional-unresolved", path, number, f"{kind}::{expression}")
                    if body and kind != "ifeval":
                        yield body, {"path": path, "line": number, "conditions": [*conditions, guard], "via": list(via)}
                    else:
                        conditions.append(guard)
                continue
            if re.match(r"^(include|ifdef|ifndef|ifeval|endif)::", line.strip()):
                self.diagnostic("unsupported-directive", path, number, line)
                continue
            yield line, {"path": path, "line": number, "conditions": list(conditions), "via": list(via)}
        if literal:
            self.diagnostic("unclosed-literal-block", path, len(lines), literal)
        if len(conditions) != floor:
            self.diagnostic("unclosed-conditional", path, len(text.splitlines()), str(conditions[floor:]))

    def terms(self, path):
        rows = list(self.expanded(path))
        found = {}
        pending = []
        section = ""
        section_source = None
        block = None

        def flush():
            if not pending:
                return
            labels = list(dict.fromkeys(label for label, _ in pending))
            primary_path = pending[0][1]["path"]
            canonical = json.dumps([primary_path, [" ".join(label.split()) for label in labels]], separators=(",", ":"))
            identifier = "git:term:" + hashlib.sha256(canonical.encode()).hexdigest()[:24]
            names = option_names(labels)
            clean = labels[0].replace("`", "").replace("*", "").strip().strip("_\'")
            if names or clean.startswith("-"):
                kind = "option"
            elif re.search(r"COMMAND|SUBCOMMAND|ACTION", section, re.I) and re.match(r"[a-z][a-z0-9-]*(?:\s|$)", clean):
                kind = "nested-verb"
            else:
                kind = "definition"
            if kind == "option" and not names:
                self.diagnostic("parameterized-option", primary_path, pending[0][1]["line"], " | ".join(labels))
            term = found.setdefault(identifier, {"id": identifier, "kind": kind, "labels": labels,
                                                 "option_names": names, "sources": [], "sections": []})
            if section not in term["sections"]:
                term["sections"].append(section)
            for _, source in pending:
                if source not in term["sources"]:
                    term["sources"].append(source)
            pending.clear()

        for index, (line, source) in enumerate(rows):
            stripped = line.strip()
            if block:
                if line == block:
                    block = None
                continue
            if line == stripped and re.fullmatch(r"(-{4,}|\.{4,}|/{4,}|\+{4,})", stripped):
                previous = rows[index - 1][0] if index else ""
                heading = bool(re.fullmatch(r"-{4,}", stripped)) and bool(re.match(r"^[A-Za-z0-9]", previous)) and not previous.endswith(".") and "::" not in previous
                if not heading:
                    flush()
                    block = stripped
                    continue
            if stripped.startswith("//"):
                continue
            if line == stripped and index + 1 < len(rows) and re.fullmatch(r"[=~^-]{3,}", rows[index + 1][0].strip()) and re.match(r"^[A-Za-z0-9]", stripped) and not stripped.endswith("."):
                flush()
                section = stripped
                section_source = {"path": source["path"], "line": source["line"]}
                continue
            if re.match(r"^={1,6} +\S", line):
                flush()
                section = stripped.lstrip("= ")
                section_source = {"path": source["path"], "line": source["line"]}
                continue
            source = {**source, "section": section, "section_source": section_source}
            match = DEFINITION.match(line)
            if match:
                label, _, inline = match.groups()
                # Group adjacent aliases only within one source/condition context.
                if pending and (source["path"] != pending[-1][1]["path"] or source["conditions"] != pending[-1][1]["conditions"]):
                    flush()
                pending.append((label.strip(), source))
                if inline:
                    flush()
            else:
                flush()
        flush()
        for term in found.values():
            term["applicability"] = "conditional-unresolved" if any(s["conditions"] for s in term["sources"]) else "documented-unconditional"
        return sorted(found.values(), key=lambda term: term["id"])


def build_census(root, git_dir, reference):
    root = Path(root)
    git_dir = Path(git_dir)
    if not git_dir.is_absolute():
        git_dir = root / git_dir
    if isinstance(reference, str):
        reference = {"revision": reference}
    tree = PinnedTree(git_dir, reference["revision"])
    extractor = Extractor(tree)
    command_list = tree.read("command-list.txt")
    if command_list is None:
        raise CensusError("pinned tree has no command-list.txt")
    commands, interfaces = [], []
    declared = set()
    for number, line in enumerate(command_list.splitlines(), 1):
        if not line.strip() or line.lstrip().startswith("#"):
            continue
        columns = line.split()
        git_name, categories = columns[0], columns[1:]
        if git_name in declared:
            raise CensusError(f"command-list.txt:{number}: duplicate declaration {git_name}")
        declared.add(git_name)
        if not categories:
            extractor.diagnostic("missing-category", "command-list.txt", number, git_name)
        is_interface = bool(NONCOMMAND.intersection(categories))
        name = git_name.removeprefix("git-")
        row = {"id": f"git:{'interface' if is_interface else 'command'}:{name}", "name": name,
               "git_name": git_name, "categories": categories,
               "declaration": {"path": "command-list.txt", "line": number}}
        attach_documentation(row, tree, extractor)
        (interfaces if is_interface else commands).append(row)
    if "git" not in declared:
        row = {"id": "git:interface:global", "name": "git", "git_name": "git", "categories": ["global"],
               "declaration": {"path": "Documentation/git.adoc", "line": 1}}
        attach_documentation(row, tree, extractor)
        interfaces.append(row)
    commands.sort(key=lambda row: row["id"])
    interfaces.sort(key=lambda row: row["id"])
    diagnostics = {json.dumps(d, sort_keys=True): d for d in extractor.diagnostics}
    groups = [term for row in commands for term in row["terms"]]
    return {"schemaVersion": SCHEMA_VERSION,
            "source": {**reference, "command_list": "command-list.txt"},
            "method": {"id": "pinned-git-definition-groups-v1", "conditionalPolicy": "retain all guarded branches with unresolved applicability",
                       "aliasPolicy": "adjacent definition labels form one group; --[no-] expands both names; repeated include occurrences merge",
                       "limits": "Documentation syntax inventory, not exhaustive semantics or behavior. Definitions in literal/comment blocks are excluded; unresolved includes remain diagnostics."},
            "counts": {"commandCount": len(commands), "interfaceCount": len(interfaces),
                       "commandTermCount": len(groups), "commandOptionGroupCount": sum(term["kind"] == "option" for term in groups),
                       "commandOptionAliasOccurrences": sum(len(term["option_names"]) for term in groups),
                       "uniqueSharedTermCount": len({term["id"] for term in groups}),
                       "diagnosticCount": len(diagnostics)},
            "commands": commands, "interfaces": interfaces,
            "diagnostics": [diagnostics[key] for key in sorted(diagnostics)]}


def attach_documentation(row, tree, extractor):
    candidates = [f"Documentation/{row['git_name']}.{suffix}" for suffix in ("adoc", "txt")]
    path = next((p for p in candidates if p in tree.paths), candidates[0])
    row["documentation"] = path
    row["terms"] = extractor.terms(path)
    row["termCount"] = len(row["terms"])
    row["optionGroupCount"] = sum(term["kind"] == "option" for term in row["terms"])


def render_markdown(census):
    counts = census["counts"]
    lines = ["# Pinned Git source census", "", "Generated by `etc/parity/census.py`; edit the extractor or pinned reference, then regenerate.", "",
             f"Reference: `{census['source'].get('label', '')}` at `{census['source']['revision']}`.", "",
             f"{counts['commandCount']} declared commands; {counts['commandOptionGroupCount']} command option definition groups; "
             f"{counts['commandOptionAliasOccurrences']} command option alias occurrences; {counts['uniqueSharedTermCount']} unique shared definition groups; "
             f"{counts['interfaceCount']} separately inventoried interfaces; {counts['diagnosticCount']} extraction diagnostics.", "",
             "These are separate denominators. They establish documented syntax, not behavioral parity. "
             "All guarded branches remain conditional; unresolved includes remain visible. Native-only capabilities and governance decisions belong in the comparison overlay.", "",
             "Adjacent definition labels count as one alias group. Repeated includes preserve occurrences without duplicating a group. "
             "Shared IDs derive from source path and normalized labels; source line movement does not change identity. Non-option definitions and nested verbs remain inventoried.", ""]
    for title, rows in (("Commands", census["commands"]), ("Interfaces and global options", census["interfaces"])):
        lines.extend([f"## {title}", "", "| Name | Categories | Terms | Option groups | Source |", "|---|---|---:|---:|---|"])
        for row in rows:
            lines.append(f"| `{row['name']}` | {', '.join(row['categories'])} | {row['termCount']} | {row['optionGroupCount']} | `{row['documentation']}` |")
        lines.append("")
    lines.extend(["## Extraction diagnostics", "", "The JSON records each source line, conditional guard and include chain. These gaps cannot count as complete extraction.", ""])
    for diagnostic in census["diagnostics"]:
        detail = diagnostic["detail"].replace("\n", " ").replace("`", "'")
        lines.append(f"- `{diagnostic['code']}` at `{diagnostic['path']}:{diagnostic['line']}`: {detail}")
    return "\n".join(lines) + "\n"


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=Path(__file__).resolve().parents[2])
    parser.add_argument("--git-dir", default="vendor/git")
    parser.add_argument("--reference", default=REFERENCE_PATH)
    parser.add_argument("--check", action="store_true", help="compare generated bytes without writing")
    args = parser.parse_args(argv)
    try:
        reference = json.loads((args.root / args.reference).read_text())
        census = build_census(args.root, args.git_dir, reference)
        outputs = {JSON_PATH: json.dumps(census, indent=2, ensure_ascii=False) + "\n", MARKDOWN_PATH: render_markdown(census)}
        drift = []
        for rel, content in outputs.items():
            path = args.root / rel
            if args.check:
                if not path.is_file() or path.read_bytes() != content.encode():
                    drift.append(rel)
            else:
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_text(content)
        if drift:
            print("STALE: " + ", ".join(drift), file=sys.stderr)
            return 1
        print(json.dumps(census["counts"], sort_keys=True))
        return 0
    except (CensusError, OSError, KeyError, json.JSONDecodeError) as exc:
        print(f"census: {exc}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
