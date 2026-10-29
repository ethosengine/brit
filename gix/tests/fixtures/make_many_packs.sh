#!/usr/bin/env bash
set -eu -o pipefail

# Create one commit, tree, and blob plus 38 standalone blobs, then put each object
# into its own pack. The 41 indices exceed the reduced-trust default of 32 slots.
git init -q
git checkout -q -b main
echo head >file
git add file
git commit -q -m "packed head"

git rev-list --objects --no-object-names HEAD >object-ids
for number in $(seq 1 38); do
  printf 'standalone object %s\n' "$number" | git hash-object -w --stdin >>object-ids
done

while read -r object_id; do
  printf '%s\n' "$object_id" | git pack-objects .git/objects/pack/pack >/dev/null
done <object-ids
git prune-packed
rm object-ids
