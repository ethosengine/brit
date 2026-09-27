---
epr-meta-version: 1
id: brit-cli-public-entry-point
purpose: >
  The one locally built public brit executable, currently composing Git and
  Rakia build commands without taking ownership of lower-layer semantics.
rules:
  - id: brit-cli-binary-name
    class: inject
    when: { write: "Cargo.toml", contains-any: ['name = "rakia"'] }
    route-to: { dest: "brit-cli/Cargo.toml [[bin]] name = brit" }
    why: >
      A separate `rakia` [[bin]] reintroduces the executable/package split.
      The approved migration makes one physical `brit` binary the public entry
      point; a compatibility `rakia` name may point to that same binary. Check
      executable journey tests and Git/build dispatch before claiming that
      invariant holds.
    retire-when: >
      when the executable manifest and journey tests prove the single `brit`
      binary and no separate `rakia` executable remains
---

# Public CLI seam

Dispatch parsed arguments through the root `src/plumbing/` Git dispatcher and
existing Rakia planning from one executable. Preserve the `gix` library and `ein`
compatibility where the approved plan requires them. The linked Git/build
command surface is source-built and locally tested; standalone installation,
publication and the record/prove/release journey are not yet qualified. This crate coordinates the operation; it does not
mint a second intent ledger, review policy, build scheduler or storage engine.
Use the Elohim workspace's `epr flow` for accountable work and its existing checks; a
standalone checkout integrates through a published interface, not a required
sibling directory. Source scope must
protect unrelated staged and dirty files, submodule pins and interruption
recovery. These are acceptance targets; the rule above only detects the current
legacy binary declaration.
