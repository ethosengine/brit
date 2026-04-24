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

## log

| Class | Section | Reason | Source |
|---|---|---|---|
| compat | `gix log --since=<time>` | gix log --since filter deferred — flag accepted, no date predicate applied | [log.sh:309](../../tests/journey/parity/log.sh#L309) |
| compat | `gix log --until=<time>` | gix log --until filter deferred — flag accepted, no date predicate applied | [log.sh:318](../../tests/journey/parity/log.sh#L318) |
| compat | `gix log --author=<pattern>` | gix log --author filter deferred — flag accepted, no regex applied to authors | [log.sh:327](../../tests/journey/parity/log.sh#L327) |
| compat | `gix log --committer=<pattern>` | gix log --committer filter deferred — flag accepted, no regex applied to committers | [log.sh:336](../../tests/journey/parity/log.sh#L336) |
| compat | `gix log --grep=<pattern>` | gix log --grep filter deferred — flag accepted, no regex applied to messages | [log.sh:345](../../tests/journey/parity/log.sh#L345) |
| compat | `gix log -i --grep=<pattern>` | gix log -i --grep case-insensitive match deferred — flag accepted | [log.sh:354](../../tests/journey/parity/log.sh#L354) |
| compat | `gix log --invert-grep --grep=<pattern>` | gix log --invert-grep filter deferred — flag accepted | [log.sh:363](../../tests/journey/parity/log.sh#L363) |
| compat | `gix log --all-match --grep=<p1> --grep=<p2>` | gix log --all-match multi-grep AND-semantics deferred — flag accepted | [log.sh:372](../../tests/journey/parity/log.sh#L372) |
| compat | `gix log -E --grep=<regex>` | gix log -E POSIX extended regex deferred — flag accepted | [log.sh:381](../../tests/journey/parity/log.sh#L381) |
| compat | `gix log -F --grep=<literal>` | gix log -F literal-string match deferred — flag accepted | [log.sh:390](../../tests/journey/parity/log.sh#L390) |
| compat | `gix log --reverse` | gix log --reverse output-order reversal deferred — flag accepted | [log.sh:534](../../tests/journey/parity/log.sh#L534) |
| compat | `gix log --topo-order` | gix log --topo-order already-default for gix's topo walker — flag accepted | [log.sh:543](../../tests/journey/parity/log.sh#L543) |
| compat | `gix log --date-order` | gix log --date-order commit-date ordering deferred — flag accepted | [log.sh:552](../../tests/journey/parity/log.sh#L552) |
| compat | `gix log --author-date-order` | gix log --author-date-order author-date ordering deferred — flag accepted | [log.sh:561](../../tests/journey/parity/log.sh#L561) |
| compat | `gix log --first-parent` | gix log --first-parent merge-parent selection deferred — flag accepted | [log.sh:570](../../tests/journey/parity/log.sh#L570) |

