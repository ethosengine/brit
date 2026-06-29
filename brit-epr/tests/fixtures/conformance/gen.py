#!/usr/bin/env python3
"""Regenerate conformance vectors from the parent cite oracle (run by hand)."""
import json, os, sys
sys.path.insert(0, "/projects/elohim/.claude/scripts")
from _lib.cite_graph import fingerprint  # PATH-based: parse_file -> body -> strip -> "sha256:hex16"

CASES = {
    "plain.md": "# Title\n\nbody line one\n\nbody line two\n",
    "with_fm.md": "---\nid: sample\ncites:\n  - x | d | sha256:0000000000000000\n---\n# Title\n\nthe real body\n",
    "trailing_ws.md": "---\nid: t\n---\n\n   spaced body   \n\n",
    "no_trailing_newline.md": "---\nid: n\n---\nno newline body",
    "unicode.md": "---\nid: u\n---\ncafé — built\n",
}
d = os.path.dirname(os.path.abspath(__file__))
out = {}
for name, text in CASES.items():
    path = os.path.join(d, name)
    with open(path, "w", encoding="utf-8") as f:
        f.write(text)
    out[name] = fingerprint(path)  # the real oracle, path-based
with open(os.path.join(d, "expected.json"), "w", encoding="utf-8") as f:
    json.dump(out, f, indent=2, sort_keys=True)
    f.write("\n")
print(json.dumps(out, indent=2))
