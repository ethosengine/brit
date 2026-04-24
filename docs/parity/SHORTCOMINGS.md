# Parity Shortcomings Ledger

Auto-generated from `tests/journey/parity/*.sh` by `etc/parity/shortcomings.sh`.
**Do not edit by hand** — re-run the generator.

Two row classes:
- **deferred** — `shortcoming "<reason>"`: row closed as a legitimate deferral; reason describes the gap.
- **compat** — `compat_effect "<reason>"`: row green at effect mode (exit-code parity); byte-output parity is a known follow-up.

## clone

| Class | Section | Reason | Source |
|---|---|---|---|
| deferred | `gix clone -b / --branch=<name>` | deferred: gix::clone::Fetch ref-not-found error maps to exit 1; git exits 128 | [clone.sh:312](../../tests/journey/parity/clone.sh#L312) |
| deferred | `gix clone --revision=<rev>` | deferred: --revision is vendor-only; system git 2.47 rejects it | [clone.sh:327](../../tests/journey/parity/clone.sh#L327) |
| deferred | `gix clone --revision=<rev> --branch=<name> (conflict)` | deferred: --revision is vendor-only; conflict row depends on it | [clone.sh:334](../../tests/journey/parity/clone.sh#L334) |
| deferred | `gix clone --revision=<rev> --mirror (conflict)` | deferred: --revision is vendor-only; conflict row depends on it | [clone.sh:341](../../tests/journey/parity/clone.sh#L341) |
| deferred | `gix clone --reference=<repo>` | deferred: gix doesn't write objects/info/alternates; --reference is parse-only | [clone.sh:457](../../tests/journey/parity/clone.sh#L457) |
| deferred | `gix clone --reference-if-able=<repo>` | deferred: parent --reference is not yet wired; --reference-if-able inherits the deferral | [clone.sh:470](../../tests/journey/parity/clone.sh#L470) |
| deferred | `gix clone --dissociate --reference=<repo>` | deferred: depends on --reference alternates wiring | [clone.sh:482](../../tests/journey/parity/clone.sh#L482) |
| deferred | `gix clone --depth=0 (non-positive)` | deferred: Clap's NonZeroU32 parser exits 2; git exits 128 with a fatal | [clone.sh:520](../../tests/journey/parity/clone.sh#L520) |
| deferred | `gix clone --shallow-since=<time>` | deferred: gix-protocol shallow-since decoder returns 'Could not decode server reply' | [clone.sh:533](../../tests/journey/parity/clone.sh#L533) |
| deferred | `gix clone --shallow-exclude=<ref>` | deferred: gix-protocol deepen-not opcode alignment (same gap as fetch.sh --shallow-exclude) | [clone.sh:545](../../tests/journey/parity/clone.sh#L545) |

## fetch

| Class | Section | Reason | Source |
|---|---|---|---|
| deferred | `gix fetch --shallow-exclude=<ref>` | --shallow-exclude semantic parity needs gix-protocol deepen-not alignment | [fetch.sh:497](../../tests/journey/parity/fetch.sh#L497) |
| deferred | `gix fetch --unshallow` | happy-path parity needs expect_parity to reset fixtures between the git and gix invocations for stateful ops | [fetch.sh:512](../../tests/journey/parity/fetch.sh#L512) |
| deferred | `gix fetch --negotiate-only` | --negotiate-only needs ack-only-capability enforcement in gix-protocol | [fetch.sh:546](../../tests/journey/parity/fetch.sh#L546) |
| deferred | `gix fetch --multiple` | --multiple needs Clap-level or dispatch-level remapping of positionals to remote-names | [fetch.sh:641](../../tests/journey/parity/fetch.sh#L641) |

