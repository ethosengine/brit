---
gap: plans__2026-09-28-git-atomic-census#1
actor: agent:implementer@gpt-6
status: active
serves: brit-git-compatibility
---
# Atomic documentation graph

Own etc/parity/atoms.py and test_atoms.py plus census.py/test_census.py only if
needed to expose truthful documentation context. Derive from existing census; no
new authoritative catalog. Export graph(census) API plus CLI command inspection,
JSON and readable tree. Keep command-context group IDs distinct from shared term
IDs, alias/name atoms separate from capability groups. Include section context,
source/guards and non-option/nested-verb candidates; never infer executable scope
from headings. Do not duplicate shared groups in totals because repeated includes
or section edges reference them. Any source extractor edits require regenerating
existing artifacts with all tests green. Coordinate API with parent comparator.
No Cargo, commits or pushes. Fresh generated JSON may be large; avoid an additional
checked-in graph if existing source can derive it at runtime. Independent reviewer
is /root/brit. Report and exactly one terminal fulfil in Brit flow root.
