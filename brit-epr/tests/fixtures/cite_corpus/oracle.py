#!/usr/bin/env python3
"""Emit {citer_id|cited_ref: verdict} using the PARENT cite engine. Skips (exit 3) if absent.

Note: Frontmatter.get(key) returns default ("") for list values, so we access
cites via fmeta.fields.get("cites", []) to get the raw list directly.
"""
import json, os, sys
ORACLE = "/projects/elohim/.claude/scripts"
if not os.path.isdir(ORACLE):
    sys.exit(3)
sys.path.insert(0, ORACLE)
from _lib.cite_graph import build_slug_index, parse_cite, envelope_verdict
from _lib.frontmatter import parse_file
root = sys.argv[1]
idx = build_slug_index([root])
out = {}
for dirpath, _dirs, files in os.walk(root):
    for fn in files:
        if not fn.endswith(".md"):
            continue
        fmeta = parse_file(os.path.join(dirpath, fn))
        cid = fmeta.get("id")
        for raw in fmeta.fields.get("cites", []):
            cite = parse_cite(raw)
            out[f"{cid}|{cite['ref']}"] = envelope_verdict(cite, idx)
print(json.dumps(out, sort_keys=True))
