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
| compat | `gix log --not <ref>` | gix log --not multi-revspec state-flipper deferred — first revspec honored, --not flag accepted | [log.sh:226](../../tests/journey/parity/log.sh#L226) |
| compat | `gix log -- <path>` | gix log -- <path> pathspec filtering deferred — flag accepted, full traversal emitted | [log.sh:239](../../tests/journey/parity/log.sh#L239) |
| compat | `gix log <ref> -- <path>` | gix log <ref> -- <path> pathspec filter on revspec deferred — clap accepted | [log.sh:249](../../tests/journey/parity/log.sh#L249) |
| compat | `gix log -- <missing-path>` | gix log -- <missing-path> pathspec filter deferred — empty match exit-0 parity holds | [log.sh:260](../../tests/journey/parity/log.sh#L260) |
| compat | `gix log --since=<time>` | gix log --since filter deferred — flag accepted, no date predicate applied | [log.sh:313](../../tests/journey/parity/log.sh#L313) |
| compat | `gix log --until=<time>` | gix log --until filter deferred — flag accepted, no date predicate applied | [log.sh:322](../../tests/journey/parity/log.sh#L322) |
| compat | `gix log --author=<pattern>` | gix log --author filter deferred — flag accepted, no regex applied to authors | [log.sh:331](../../tests/journey/parity/log.sh#L331) |
| compat | `gix log --committer=<pattern>` | gix log --committer filter deferred — flag accepted, no regex applied to committers | [log.sh:340](../../tests/journey/parity/log.sh#L340) |
| compat | `gix log --grep=<pattern>` | gix log --grep filter deferred — flag accepted, no regex applied to messages | [log.sh:349](../../tests/journey/parity/log.sh#L349) |
| compat | `gix log -i --grep=<pattern>` | gix log -i --grep case-insensitive match deferred — flag accepted | [log.sh:358](../../tests/journey/parity/log.sh#L358) |
| compat | `gix log --invert-grep --grep=<pattern>` | gix log --invert-grep filter deferred — flag accepted | [log.sh:367](../../tests/journey/parity/log.sh#L367) |
| compat | `gix log --all-match --grep=<p1> --grep=<p2>` | gix log --all-match multi-grep AND-semantics deferred — flag accepted | [log.sh:376](../../tests/journey/parity/log.sh#L376) |
| compat | `gix log -E --grep=<regex>` | gix log -E POSIX extended regex deferred — flag accepted | [log.sh:385](../../tests/journey/parity/log.sh#L385) |
| compat | `gix log -F --grep=<literal>` | gix log -F literal-string match deferred — flag accepted | [log.sh:394](../../tests/journey/parity/log.sh#L394) |
| compat | `gix log --oneline` | gix log --oneline pretty-format divergence deferred — clap accepted | [log.sh:427](../../tests/journey/parity/log.sh#L427) |
| compat | `gix log --pretty=oneline` | gix log --pretty=oneline format emission deferred — clap accepted | [log.sh:436](../../tests/journey/parity/log.sh#L436) |
| compat | `gix log --pretty=short` | gix log --pretty=short format emission deferred — clap accepted | [log.sh:445](../../tests/journey/parity/log.sh#L445) |
| compat | `gix log --pretty=medium` | gix log --pretty=medium format emission deferred — clap accepted | [log.sh:454](../../tests/journey/parity/log.sh#L454) |
| compat | `gix log --pretty=full` | gix log --pretty=full format emission deferred — clap accepted | [log.sh:463](../../tests/journey/parity/log.sh#L463) |
| compat | `gix log --pretty=fuller` | gix log --pretty=fuller format emission deferred — clap accepted | [log.sh:472](../../tests/journey/parity/log.sh#L472) |
| compat | `gix log --pretty=raw` | gix log --pretty=raw format emission deferred — clap accepted | [log.sh:482](../../tests/journey/parity/log.sh#L482) |
| compat | `gix log --pretty=reference` | gix log --pretty=reference format emission deferred — clap accepted | [log.sh:491](../../tests/journey/parity/log.sh#L491) |
| compat | `gix log --format=%H` | gix log --format=<fmt> custom formatter deferred — clap accepted | [log.sh:501](../../tests/journey/parity/log.sh#L501) |
| compat | `gix log --format='%h %s'` | gix log --format=<fmt> custom formatter deferred — clap accepted | [log.sh:510](../../tests/journey/parity/log.sh#L510) |
| compat | `gix log --abbrev-commit` | gix log --abbrev-commit hash-width control deferred — clap accepted | [log.sh:520](../../tests/journey/parity/log.sh#L520) |
| compat | `gix log --no-abbrev-commit` | gix log --no-abbrev-commit hash-width control deferred — clap accepted | [log.sh:530](../../tests/journey/parity/log.sh#L530) |
| compat | `gix log --abbrev=<n>` | gix log --abbrev=<n> hash-width control deferred — clap accepted | [log.sh:540](../../tests/journey/parity/log.sh#L540) |
| compat | `gix log --reverse` | gix log --reverse output-order reversal deferred — flag accepted | [log.sh:551](../../tests/journey/parity/log.sh#L551) |
| compat | `gix log --topo-order` | gix log --topo-order already-default for gix's topo walker — flag accepted | [log.sh:560](../../tests/journey/parity/log.sh#L560) |
| compat | `gix log --date-order` | gix log --date-order commit-date ordering deferred — flag accepted | [log.sh:569](../../tests/journey/parity/log.sh#L569) |
| compat | `gix log --author-date-order` | gix log --author-date-order author-date ordering deferred — flag accepted | [log.sh:578](../../tests/journey/parity/log.sh#L578) |
| compat | `gix log --first-parent` | gix log --first-parent merge-parent selection deferred — flag accepted | [log.sh:587](../../tests/journey/parity/log.sh#L587) |
| compat | `gix log --decorate` | gix log --decorate ref-name emission deferred — clap accepted | [log.sh:621](../../tests/journey/parity/log.sh#L621) |
| compat | `gix log --decorate=short` | gix log --decorate=short ref-name emission deferred — clap accepted | [log.sh:630](../../tests/journey/parity/log.sh#L630) |
| compat | `gix log --decorate=full` | gix log --decorate=full ref-name emission deferred — clap accepted | [log.sh:639](../../tests/journey/parity/log.sh#L639) |
| compat | `gix log --decorate=no` | gix log --decorate=no suppressed (never emitted decorations anyway) — clap accepted | [log.sh:648](../../tests/journey/parity/log.sh#L648) |
| compat | `gix log --no-decorate` | gix log --no-decorate suppressed (never emitted decorations anyway) — clap accepted | [log.sh:657](../../tests/journey/parity/log.sh#L657) |
| compat | `gix log --decorate-refs=<pattern>` | gix log --decorate-refs filter deferred — clap accepted | [log.sh:667](../../tests/journey/parity/log.sh#L667) |
| compat | `gix log --decorate-refs-exclude=<pattern>` | gix log --decorate-refs-exclude filter deferred — clap accepted | [log.sh:677](../../tests/journey/parity/log.sh#L677) |
| compat | `gix log --clear-decorations` | gix log --clear-decorations decoration-filter reset deferred — clap accepted | [log.sh:687](../../tests/journey/parity/log.sh#L687) |
| compat | `gix log --source` | gix log --source ref-prefix emission deferred — clap accepted | [log.sh:697](../../tests/journey/parity/log.sh#L697) |
| compat | `gix log --graph` | gix log --graph ASCII-art commit graph deferred — clap accepted | [log.sh:708](../../tests/journey/parity/log.sh#L708) |
| compat | `gix log -p` | gix log -p per-commit diff emission deferred — clap accepted | [log.sh:719](../../tests/journey/parity/log.sh#L719) |
| compat | `gix log -s` | gix log -s --no-patch diff suppression (vs -p) deferred — clap accepted | [log.sh:728](../../tests/journey/parity/log.sh#L728) |
| compat | `gix log --stat` | gix log --stat diffstat emission deferred — clap accepted | [log.sh:737](../../tests/journey/parity/log.sh#L737) |
| compat | `gix log --shortstat` | gix log --shortstat summary emission deferred — clap accepted | [log.sh:746](../../tests/journey/parity/log.sh#L746) |
| compat | `gix log --numstat` | gix log --numstat machine-readable diffstat deferred — clap accepted | [log.sh:755](../../tests/journey/parity/log.sh#L755) |
| compat | `gix log --name-only` | gix log --name-only path listing deferred — clap accepted | [log.sh:764](../../tests/journey/parity/log.sh#L764) |
| compat | `gix log --name-status` | gix log --name-status status-letter path listing deferred — clap accepted | [log.sh:773](../../tests/journey/parity/log.sh#L773) |
| compat | `gix log --raw` | gix log --raw diff emission deferred — clap accepted | [log.sh:782](../../tests/journey/parity/log.sh#L782) |
| compat | `gix log -M` | gix log -M/--find-renames rename detection deferred — clap accepted | [log.sh:791](../../tests/journey/parity/log.sh#L791) |
| compat | `gix log --follow <file>` | gix log --follow rename-following deferred — flag accepted, positional treated as pathspec hint | [log.sh:806](../../tests/journey/parity/log.sh#L806) |
| compat | `gix log --full-diff -- <path>` | gix log --full-diff diff-scope override deferred — clap accepted | [log.sh:816](../../tests/journey/parity/log.sh#L816) |
| compat | `gix log -L <start>,<end>:<file>` | gix log -L line-range traversal deferred — flag accepted, range parser and file-lookup unwired | [log.sh:829](../../tests/journey/parity/log.sh#L829) |
| compat | `gix log --date=relative` | gix log --date=relative date-format emission deferred — clap accepted | [log.sh:840](../../tests/journey/parity/log.sh#L840) |
| compat | `gix log --date=iso` | gix log --date=iso date-format emission deferred — clap accepted | [log.sh:849](../../tests/journey/parity/log.sh#L849) |
| compat | `gix log --date=short` | gix log --date=short date-format emission deferred — clap accepted | [log.sh:858](../../tests/journey/parity/log.sh#L858) |
| compat | `gix log --date=raw` | gix log --date=raw date-format emission deferred — clap accepted | [log.sh:867](../../tests/journey/parity/log.sh#L867) |
| compat | `gix log --date=unix` | gix log --date=unix date-format emission deferred — clap accepted | [log.sh:876](../../tests/journey/parity/log.sh#L876) |
| compat | `gix log --date=format:<strftime>` | gix log --date=format:<strftime> strftime emission deferred — clap accepted | [log.sh:885](../../tests/journey/parity/log.sh#L885) |
| compat | `gix log -m` | gix log -m per-parent merge diff deferred — clap accepted | [log.sh:896](../../tests/journey/parity/log.sh#L896) |
| compat | `gix log -c` | gix log -c combined merge diff deferred — clap accepted | [log.sh:905](../../tests/journey/parity/log.sh#L905) |
| compat | `gix log --cc` | gix log --cc dense combined merge diff deferred — clap accepted | [log.sh:914](../../tests/journey/parity/log.sh#L914) |
| compat | `gix log --diff-merges=off` | gix log --diff-merges=off merge-diff control deferred — clap accepted | [log.sh:923](../../tests/journey/parity/log.sh#L923) |
| compat | `gix log --diff-merges=first-parent` | gix log --diff-merges=first-parent merge-diff control deferred — clap accepted | [log.sh:932](../../tests/journey/parity/log.sh#L932) |
| compat | `gix log --mailmap` | gix log --mailmap author/committer rewriting deferred — clap accepted | [log.sh:943](../../tests/journey/parity/log.sh#L943) |
| compat | `gix log --no-mailmap` | gix log --no-mailmap .mailmap bypass deferred — clap accepted | [log.sh:952](../../tests/journey/parity/log.sh#L952) |
| compat | `gix log --log-size` | gix log --log-size message-length header deferred — clap accepted | [log.sh:961](../../tests/journey/parity/log.sh#L961) |
| compat | `gix log --notes` | gix log --notes refs/notes emission deferred — clap accepted | [log.sh:970](../../tests/journey/parity/log.sh#L970) |
| compat | `gix log --no-notes` | gix log --no-notes refs/notes suppression deferred — clap accepted | [log.sh:980](../../tests/journey/parity/log.sh#L980) |
| compat | `gix log --show-signature` | gix log --show-signature GPG verification deferred — clap accepted | [log.sh:989](../../tests/journey/parity/log.sh#L989) |
| compat | `gix log --color=always` | gix log --color=always ANSI color emission deferred — clap accepted | [log.sh:1000](../../tests/journey/parity/log.sh#L1000) |
| compat | `gix log --no-color` | gix log --no-color color-suppression deferred (never emitted colors anyway) — clap accepted | [log.sh:1009](../../tests/journey/parity/log.sh#L1009) |
| compat | `gix log --boundary` | gix log --boundary + --not boundary-marker emission deferred — clap accepted | [log.sh:1020](../../tests/journey/parity/log.sh#L1020) |
| compat | `gix log --ancestry-path` | gix log --ancestry-path ancestry filter deferred — clap accepted | [log.sh:1029](../../tests/journey/parity/log.sh#L1029) |

