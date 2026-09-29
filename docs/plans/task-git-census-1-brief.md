---
gap: plans__2026-09-28-git-feature-census#1
actor: agent:implementer@gpt-6
status: active
serves: dev-system-equilibrium
---
# Git census task 1

Implement only etc/parity/census.py and its focused tests. Read pinned Git objects from vendor/git at 94f057755b7941b321fd11fec1b2e3ca5313a4e0 (v2.54.0), not mutable working tree. Own explicit reference metadata docs/parity/git-reference.json if needed. Export Python function build_census(root, git_dir, reference) returning schema version, source revision, commands (name/category/doc/source/option or definition terms), noncommand interfaces, extraction diagnostics. Stable IDs and exact path/line provenance; recursive include handling with cycles/path traversal/missing refs explicit, conditional handling cannot pretend universal applicability; option alias dedup/counting documented. Preserve all terms not just options to avoid false denominator; distinguish declared commands, docs, option definitions, nested verbs. Deterministic output to docs/parity/git-census.json and GIT-CENSUS.md, CLI --check no writes. Share schema with parent early. Parent owns wrapper+Brit overlay+CI+doc integration. Do not touch existing commands.md historical claims. Test with small synthetic pinned git repositories and real pin. No cargo needed.

Governed plan: elohim/brit/docs/plans/2026-09-28-git-feature-census.md. Brit base 1017e94646275d167b21425100c2655a18e84826. All earlier dirty changes are preserved. Use native flow --root elohim/brit for commitment verbs, parent path report allowed absolute if resolver accepts otherwise write Brit-local report and notify parent. Independent review required.
