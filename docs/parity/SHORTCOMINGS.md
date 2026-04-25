# Parity Shortcomings Ledger

Auto-generated from `tests/journey/parity/*.sh` by `etc/parity/shortcomings.sh`.
**Do not edit by hand** — re-run the generator.

Two row classes:
- **deferred** — `shortcoming "<reason>"`: row closed as a legitimate deferral; reason describes the gap.
- **compat** — `compat_effect "<reason>"`: row green at effect mode (exit-code parity); byte-output parity is a known follow-up.

## add

| Class | Section | Reason | Source |
|---|---|---|---|
| compat | `gix add a` | deferred until add driver lands | [add.sh:144](../../tests/journey/parity/add.sh#L144) |
| compat | `gix add new-file` | deferred until add driver lands | [add.sh:154](../../tests/journey/parity/add.sh#L154) |
| compat | `gix add .` | deferred until add driver lands | [add.sh:164](../../tests/journey/parity/add.sh#L164) |
| compat | `gix add -- a` | deferred until add driver lands | [add.sh:173](../../tests/journey/parity/add.sh#L173) |
| compat | `gix add missing-file` | deferred until add driver lands | [add.sh:184](../../tests/journey/parity/add.sh#L184) |
| compat | `gix add -n a` | deferred until add driver lands | [add.sh:196](../../tests/journey/parity/add.sh#L196) |
| compat | `gix add --dry-run a` | deferred until add driver lands | [add.sh:205](../../tests/journey/parity/add.sh#L205) |
| compat | `gix add -v a` | deferred until add driver lands | [add.sh:215](../../tests/journey/parity/add.sh#L215) |
| compat | `gix add --verbose a` | deferred until add driver lands | [add.sh:224](../../tests/journey/parity/add.sh#L224) |
| compat | `gix add -f a` | deferred until add driver lands | [add.sh:236](../../tests/journey/parity/add.sh#L236) |
| compat | `gix add --force a` | deferred until add driver lands | [add.sh:245](../../tests/journey/parity/add.sh#L245) |
| compat | `gix add -u` | deferred until add driver lands | [add.sh:255](../../tests/journey/parity/add.sh#L255) |
| compat | `gix add --update` | deferred until add driver lands | [add.sh:264](../../tests/journey/parity/add.sh#L264) |
| compat | `gix add -A` | deferred until add driver lands | [add.sh:274](../../tests/journey/parity/add.sh#L274) |
| compat | `gix add --all` | deferred until add driver lands | [add.sh:283](../../tests/journey/parity/add.sh#L283) |
| compat | `gix add --no-all` | deferred until add driver lands | [add.sh:293](../../tests/journey/parity/add.sh#L293) |
| compat | `gix add --ignore-removal` | deferred until add driver lands | [add.sh:302](../../tests/journey/parity/add.sh#L302) |
| compat | `gix add --no-ignore-removal` | deferred until add driver lands | [add.sh:312](../../tests/journey/parity/add.sh#L312) |
| compat | `gix add --sparse a` | deferred until add driver lands | [add.sh:322](../../tests/journey/parity/add.sh#L322) |
| compat | `gix add -N new-file` | deferred until add driver lands | [add.sh:335](../../tests/journey/parity/add.sh#L335) |
| compat | `gix add --intent-to-add new-file` | deferred until add driver lands | [add.sh:345](../../tests/journey/parity/add.sh#L345) |
| compat | `gix add --renormalize` | deferred until add driver lands | [add.sh:357](../../tests/journey/parity/add.sh#L357) |
| compat | `gix add --refresh a` | deferred until add driver lands | [add.sh:369](../../tests/journey/parity/add.sh#L369) |
| compat | `gix add --ignore-errors a` | deferred until add driver lands | [add.sh:379](../../tests/journey/parity/add.sh#L379) |
| compat | `gix add --chmod=+x a` | deferred until add driver lands | [add.sh:391](../../tests/journey/parity/add.sh#L391) |
| compat | `gix add --chmod=-x a` | deferred until add driver lands | [add.sh:400](../../tests/journey/parity/add.sh#L400) |
| compat | `gix add --pathspec-from-file=spec.txt` | deferred until add driver lands | [add.sh:413](../../tests/journey/parity/add.sh#L413) |
| compat | `gix add --pathspec-from-file=spec.txt --pathspec-file-nul` | deferred until add driver lands | [add.sh:426](../../tests/journey/parity/add.sh#L426) |
| compat | `gix add -i` | deferred until add driver lands | [add.sh:440](../../tests/journey/parity/add.sh#L440) |
| compat | `gix add --interactive` | deferred until add driver lands | [add.sh:449](../../tests/journey/parity/add.sh#L449) |
| compat | `gix add -p` | deferred until add driver lands | [add.sh:459](../../tests/journey/parity/add.sh#L459) |
| compat | `gix add --patch` | deferred until add driver lands | [add.sh:468](../../tests/journey/parity/add.sh#L468) |
| compat | `gix add -e a` | deferred until add driver lands | [add.sh:485](../../tests/journey/parity/add.sh#L485) |
| compat | `gix add --edit a` | deferred until add driver lands | [add.sh:495](../../tests/journey/parity/add.sh#L495) |
| deferred | `gix add --auto-advance --patch` | system git 2.47.3 lacks --auto-advance; vendor/git v2.54.0 has it | [add.sh:509](../../tests/journey/parity/add.sh#L509) |
| deferred | `gix add -U 3 --patch` | system git 2.47.3 lacks -U for add; vendor/git v2.54.0 has it | [add.sh:521](../../tests/journey/parity/add.sh#L521) |
| deferred | `gix add --unified=3 --patch` | system git 2.47.3 lacks --unified for add; vendor/git v2.54.0 has it | [add.sh:530](../../tests/journey/parity/add.sh#L530) |
| deferred | `gix add --inter-hunk-context=2 --patch` | system git 2.47.3 lacks --inter-hunk-context for add; vendor/git v2.54.0 has it | [add.sh:541](../../tests/journey/parity/add.sh#L541) |
| compat | `gix add --dry-run --ignore-missing missing-file` | deferred until add driver lands | [add.sh:554](../../tests/journey/parity/add.sh#L554) |
| deferred | `gix add --unified=3` | system git 2.47.3 lacks --unified for add; vendor/git v2.54.0 has it | [add.sh:666](../../tests/journey/parity/add.sh#L666) |
| deferred | `gix add --inter-hunk-context=2` | system git 2.47.3 lacks --inter-hunk-context for add; vendor/git v2.54.0 has it | [add.sh:678](../../tests/journey/parity/add.sh#L678) |
| deferred | `gix add --no-auto-advance` | system git 2.47.3 lacks --no-auto-advance; vendor/git v2.54.0 has it | [add.sh:690](../../tests/journey/parity/add.sh#L690) |

## blame

| Class | Section | Reason | Source |
|---|---|---|---|
| compat | `gix blame <file> (default format, populated repo)` | blame default-format author/date renderer deferred | [blame.sh:120](../../tests/journey/parity/blame.sh#L120) |
| compat | `gix blame HEAD <file>` | blame default-format renderer deferred (rev <file>) | [blame.sh:147](../../tests/journey/parity/blame.sh#L147) |
| compat | `gix blame -- <file>` | blame default-format renderer deferred (-- separator) | [blame.sh:159](../../tests/journey/parity/blame.sh#L159) |
| compat | `gix blame -p / --porcelain` | blame --porcelain machine-consumption renderer deferred | [blame.sh:175](../../tests/journey/parity/blame.sh#L175) |
| compat | `gix blame --line-porcelain` | blame --line-porcelain machine-consumption renderer deferred | [blame.sh:186](../../tests/journey/parity/blame.sh#L186) |
| compat | `gix blame --incremental` | blame --incremental machine-consumption renderer deferred | [blame.sh:198](../../tests/journey/parity/blame.sh#L198) |
| compat | `gix blame -c` | blame -c annotate-compat renderer deferred | [blame.sh:208](../../tests/journey/parity/blame.sh#L208) |
| compat | `gix blame -t` | blame -t raw-timestamp renderer deferred | [blame.sh:218](../../tests/journey/parity/blame.sh#L218) |
| compat | `gix blame -l` | blame -l long-hash renderer deferred | [blame.sh:228](../../tests/journey/parity/blame.sh#L228) |
| compat | `gix blame -s (suppress author+timestamp)` | blame -s author-suppression renderer deferred | [blame.sh:242](../../tests/journey/parity/blame.sh#L242) |
| compat | `gix blame -e / --show-email` | blame -e show-email renderer deferred | [blame.sh:252](../../tests/journey/parity/blame.sh#L252) |
| compat | `gix blame -f / --show-name` | blame -f show-name renderer deferred | [blame.sh:262](../../tests/journey/parity/blame.sh#L262) |
| compat | `gix blame -n / --show-number` | blame -n show-number renderer deferred | [blame.sh:272](../../tests/journey/parity/blame.sh#L272) |
| compat | `gix blame -L <start>,<end>` | blame default-format renderer deferred (range form) | [blame.sh:287](../../tests/journey/parity/blame.sh#L287) |
| deferred | `gix blame -L <start>, (open-ended)` | open-ended -L <start>, / -L ,<end> not parsed by AsRange (src/shared.rs) | [blame.sh:313](../../tests/journey/parity/blame.sh#L313) |
| deferred | `gix blame -L :<funcname>` | -L :<funcname> requires userdiff/funcname-pattern infra (not yet in gix-diff) | [blame.sh:324](../../tests/journey/parity/blame.sh#L324) |
| deferred | `gix blame -L /<regex>/,<end>` | -L /<regex>/,... requires regex-anchored range parsing (AsRange + blob scan) — not yet wired | [blame.sh:335](../../tests/journey/parity/blame.sh#L335) |
| compat | `gix blame -L A,B -L C,D (multiple)` | blame default-format renderer deferred (multi-range) | [blame.sh:344](../../tests/journey/parity/blame.sh#L344) |
| compat | `gix blame --show-stats` | blame --show-stats renderer deferred | [blame.sh:359](../../tests/journey/parity/blame.sh#L359) |
| compat | `gix blame --score-debug` | blame --score-debug renderer deferred | [blame.sh:369](../../tests/journey/parity/blame.sh#L369) |
| compat | `gix blame --abbrev=<n>` | blame --abbrev hash-width tunable deferred | [blame.sh:384](../../tests/journey/parity/blame.sh#L384) |
| compat | `gix blame --date=<format>` | blame --date format renderer deferred | [blame.sh:397](../../tests/journey/parity/blame.sh#L397) |
| compat | `gix blame --since=<date>` | blame default-format renderer deferred (--since) | [blame.sh:412](../../tests/journey/parity/blame.sh#L412) |
| compat | `gix blame --first-parent` | blame --first-parent walker integration deferred | [blame.sh:435](../../tests/journey/parity/blame.sh#L435) |
| compat | `gix blame -b (blank boundary)` | blame -b boundary blanking deferred | [blame.sh:448](../../tests/journey/parity/blame.sh#L448) |
| compat | `gix blame --root` | blame --root boundary policy deferred | [blame.sh:459](../../tests/journey/parity/blame.sh#L459) |
| compat | `gix blame -M` | blame -M within-file move detection deferred | [blame.sh:473](../../tests/journey/parity/blame.sh#L473) |
| compat | `gix blame -M<num>` | blame -M=<num> threshold tunable deferred | [blame.sh:486](../../tests/journey/parity/blame.sh#L486) |
| compat | `gix blame -C` | blame -C cross-file copy detection deferred | [blame.sh:499](../../tests/journey/parity/blame.sh#L499) |
| compat | `gix blame -C -C (creation-commit)` | blame -C -C creation-commit copy detection deferred | [blame.sh:509](../../tests/journey/parity/blame.sh#L509) |
| compat | `gix blame -C -C -C (any-commit)` | blame -C -C -C any-commit copy detection deferred | [blame.sh:519](../../tests/journey/parity/blame.sh#L519) |
| compat | `gix blame --ignore-rev <rev>` | blame --ignore-rev attribution-rewrite deferred | [blame.sh:532](../../tests/journey/parity/blame.sh#L532) |
| compat | `gix blame --ignore-revs-file <file>` | blame --ignore-revs-file deferred | [blame.sh:544](../../tests/journey/parity/blame.sh#L544) |
| compat | `gix blame -w` | blame -w whitespace-ignore threading deferred | [blame.sh:558](../../tests/journey/parity/blame.sh#L558) |
| compat | `gix blame --diff-algorithm=<algo>` | blame --diff-algorithm threading deferred | [blame.sh:569](../../tests/journey/parity/blame.sh#L569) |
| compat | `gix blame --minimal` | blame --minimal alias deferred | [blame.sh:579](../../tests/journey/parity/blame.sh#L579) |
| compat | `gix blame --color-lines` | blame --color-lines deferred | [blame.sh:591](../../tests/journey/parity/blame.sh#L591) |
| compat | `gix blame --color-by-age` | blame --color-by-age deferred | [blame.sh:601](../../tests/journey/parity/blame.sh#L601) |
| compat | `gix blame --progress` | blame --progress reporting deferred | [blame.sh:615](../../tests/journey/parity/blame.sh#L615) |
| compat | `gix blame --no-progress` | blame --no-progress deferred | [blame.sh:625](../../tests/journey/parity/blame.sh#L625) |
| compat | `gix blame --contents <file>` | blame --contents alternate-final-image deferred | [blame.sh:650](../../tests/journey/parity/blame.sh#L650) |
| compat | `gix blame --encoding=<encoding>` | blame --encoding output transcoding deferred | [blame.sh:660](../../tests/journey/parity/blame.sh#L660) |
| compat | `gix blame -S <revs-file>` | blame -S graft-revs-file deferred | [blame.sh:672](../../tests/journey/parity/blame.sh#L672) |

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

## diff

| Class | Section | Reason | Source |
|---|---|---|---|
| compat | `gix diff (no args, dirty working tree)` | diff worktree-vs-index patch output deferred until renderer lands | [diff.sh:124](../../tests/journey/parity/diff.sh#L124) |
| compat | `gix diff <commit>` | diff worktree-vs-<commit> patch output deferred until renderer lands | [diff.sh:142](../../tests/journey/parity/diff.sh#L142) |
| compat | `gix diff <commit> <commit>` | diff tree-vs-tree patch output deferred until renderer lands | [diff.sh:157](../../tests/journey/parity/diff.sh#L157) |
| compat | `gix diff A..B` | diff A..B tree-vs-tree patch output deferred until renderer lands | [diff.sh:185](../../tests/journey/parity/diff.sh#L185) |
| compat | `gix diff A...B` | diff A...B symmetric tree-vs-tree patch output deferred until renderer lands | [diff.sh:198](../../tests/journey/parity/diff.sh#L198) |
| compat | `gix diff <blob> <blob>` | diff blob-vs-blob patch output deferred until renderer lands | [diff.sh:220](../../tests/journey/parity/diff.sh#L220) |
| compat | `gix diff -- <path>` | diff path-filter (-- <path>) filtering not yet implemented | [diff.sh:235](../../tests/journey/parity/diff.sh#L235) |
| compat | `gix diff --cached` | diff --cached index-vs-HEAD patch output deferred until renderer lands | [diff.sh:250](../../tests/journey/parity/diff.sh#L250) |
| compat | `gix diff --staged` | diff --staged alias of --cached, same patch output deferral | [diff.sh:261](../../tests/journey/parity/diff.sh#L261) |
| compat | `gix diff --cached <commit>` | diff --cached <commit> patch output deferred until renderer lands | [diff.sh:275](../../tests/journey/parity/diff.sh#L275) |
| compat | `gix diff --merge-base <commit>` | diff --merge-base <commit> resolution + patch output deferred until renderer lands | [diff.sh:287](../../tests/journey/parity/diff.sh#L287) |
| compat | `gix diff --merge-base A B` | diff --merge-base A B substitution + patch output deferred until renderer lands | [diff.sh:299](../../tests/journey/parity/diff.sh#L299) |
| compat | `gix diff --cached --merge-base` | diff --cached --merge-base index-vs-merge-base patch output deferred until renderer lands | [diff.sh:310](../../tests/journey/parity/diff.sh#L310) |
| compat | `gix diff --no-index <path-a> <path-b>` | diff --no-index patch output deferred until renderer lands | [diff.sh:326](../../tests/journey/parity/diff.sh#L326) |
| compat | `gix diff -p / -u / --patch` | diff -p/-u/--patch patch output deferred until renderer lands | [diff.sh:340](../../tests/journey/parity/diff.sh#L340) |
| compat | `gix diff -s / --no-patch` | diff -s/--no-patch suppression interaction deferred until renderer lands | [diff.sh:352](../../tests/journey/parity/diff.sh#L352) |
| compat | `gix diff --raw` | diff --raw scriptable raw format deferred until renderer lands | [diff.sh:364](../../tests/journey/parity/diff.sh#L364) |
| compat | `gix diff --patch-with-raw` | diff --patch-with-raw composite output deferred until renderer lands | [diff.sh:375](../../tests/journey/parity/diff.sh#L375) |
| compat | `gix diff -t` | diff -t tree-entry recursion deferred until renderer lands | [diff.sh:386](../../tests/journey/parity/diff.sh#L386) |
| compat | `gix diff --name-only` | diff --name-only changed-path enumeration deferred until renderer lands | [diff.sh:398](../../tests/journey/parity/diff.sh#L398) |
| compat | `gix diff --name-status` | diff --name-status status-letter+path enumeration deferred until renderer lands | [diff.sh:408](../../tests/journey/parity/diff.sh#L408) |
| compat | `gix diff --stat` | diff --stat file-by-file layout deferred until renderer lands | [diff.sh:419](../../tests/journey/parity/diff.sh#L419) |
| compat | `gix diff --compact-summary` | diff --compact-summary condensed layout deferred until renderer lands | [diff.sh:429](../../tests/journey/parity/diff.sh#L429) |
| compat | `gix diff --shortstat` | diff --shortstat one-line summary deferred until renderer lands | [diff.sh:439](../../tests/journey/parity/diff.sh#L439) |
| compat | `gix diff --numstat` | diff --numstat tab-separated stat deferred until renderer lands | [diff.sh:449](../../tests/journey/parity/diff.sh#L449) |
| compat | `gix diff --dirstat` | diff --dirstat per-directory percentage layout deferred until renderer lands | [diff.sh:460](../../tests/journey/parity/diff.sh#L460) |
| compat | `gix diff --cumulative` | diff --cumulative dirstat synonym deferred until renderer lands | [diff.sh:470](../../tests/journey/parity/diff.sh#L470) |
| compat | `gix diff --dirstat-by-file` | diff --dirstat-by-file dirstat synonym deferred until renderer lands | [diff.sh:480](../../tests/journey/parity/diff.sh#L480) |
| compat | `gix diff --summary` | diff --summary mode-change/rename summary deferred until renderer lands | [diff.sh:490](../../tests/journey/parity/diff.sh#L490) |
| compat | `gix diff --patch-with-stat` | diff --patch-with-stat composite output deferred until renderer lands | [diff.sh:500](../../tests/journey/parity/diff.sh#L500) |
| compat | `gix diff -z` | diff -z NUL-termination deferred until renderer lands | [diff.sh:511](../../tests/journey/parity/diff.sh#L511) |
| compat | `gix diff -U / --unified` | diff -U/--unified context-line count deferred until renderer lands | [diff.sh:523](../../tests/journey/parity/diff.sh#L523) |
| compat | `gix diff --output` | diff --output file-redirect deferred until renderer lands | [diff.sh:533](../../tests/journey/parity/diff.sh#L533) |
| compat | `gix diff --output-indicator-{new,old,context}` | diff --output-indicator-* per-line marker override deferred until renderer lands | [diff.sh:544](../../tests/journey/parity/diff.sh#L544) |
| compat | `gix diff --abbrev` | diff --abbrev hash-abbreviation width deferred until renderer lands | [diff.sh:554](../../tests/journey/parity/diff.sh#L554) |
| compat | `gix diff --binary` | diff --binary base85 binary patch deferred until renderer lands | [diff.sh:565](../../tests/journey/parity/diff.sh#L565) |
| compat | `gix diff --full-index` | diff --full-index full SHA emission deferred until renderer lands | [diff.sh:575](../../tests/journey/parity/diff.sh#L575) |
| compat | `gix diff --line-prefix` | diff --line-prefix per-line prefix deferred until renderer lands | [diff.sh:586](../../tests/journey/parity/diff.sh#L586) |
| compat | `gix diff --src-prefix / --dst-prefix` | diff --src-prefix/--dst-prefix patch-header overrides deferred until renderer lands | [diff.sh:597](../../tests/journey/parity/diff.sh#L597) |
| compat | `gix diff --no-prefix` | diff --no-prefix prefix-suppression deferred until renderer lands | [diff.sh:607](../../tests/journey/parity/diff.sh#L607) |
| compat | `gix diff --default-prefix` | diff --default-prefix deferred until renderer lands | [diff.sh:617](../../tests/journey/parity/diff.sh#L617) |
| compat | `gix diff --color` | diff --color=always deferred until renderer lands | [diff.sh:629](../../tests/journey/parity/diff.sh#L629) |
| compat | `gix diff --no-color` | diff --no-color deferred until renderer lands | [diff.sh:638](../../tests/journey/parity/diff.sh#L638) |
| compat | `gix diff --color-moved` | diff --color-moved=zebra deferred until renderer lands | [diff.sh:648](../../tests/journey/parity/diff.sh#L648) |
| compat | `gix diff --no-color-moved` | diff --no-color-moved deferred until renderer lands | [diff.sh:657](../../tests/journey/parity/diff.sh#L657) |
| compat | `gix diff --color-moved-ws` | diff --color-moved-ws=ignore-all-space deferred until renderer lands | [diff.sh:668](../../tests/journey/parity/diff.sh#L668) |
| compat | `gix diff --no-color-moved-ws` | diff --no-color-moved-ws deferred until renderer lands | [diff.sh:677](../../tests/journey/parity/diff.sh#L677) |
| compat | `gix diff --word-diff` | diff --word-diff=plain deferred until renderer lands | [diff.sh:686](../../tests/journey/parity/diff.sh#L686) |
| compat | `gix diff --word-diff-regex` | diff --word-diff-regex='\\w+' deferred until renderer lands | [diff.sh:696](../../tests/journey/parity/diff.sh#L696) |
| compat | `gix diff --color-words` | diff --color-words deferred until renderer lands | [diff.sh:706](../../tests/journey/parity/diff.sh#L706) |
| compat | `gix diff --minimal` | diff --minimal deferred until renderer lands | [diff.sh:717](../../tests/journey/parity/diff.sh#L717) |
| compat | `gix diff --patience` | diff --patience deferred until renderer lands | [diff.sh:726](../../tests/journey/parity/diff.sh#L726) |
| compat | `gix diff --histogram` | diff --histogram deferred until renderer lands | [diff.sh:735](../../tests/journey/parity/diff.sh#L735) |
| compat | `gix diff --anchored` | diff --anchored=foo deferred until renderer lands | [diff.sh:745](../../tests/journey/parity/diff.sh#L745) |
| compat | `gix diff --diff-algorithm` | diff --diff-algorithm=histogram deferred until renderer lands | [diff.sh:754](../../tests/journey/parity/diff.sh#L754) |
| compat | `gix diff --indent-heuristic` | diff --indent-heuristic deferred until renderer lands | [diff.sh:764](../../tests/journey/parity/diff.sh#L764) |
| compat | `gix diff --no-indent-heuristic` | diff --no-indent-heuristic deferred until renderer lands | [diff.sh:773](../../tests/journey/parity/diff.sh#L773) |
| compat | `gix diff -a / --text` | diff -a deferred until renderer lands | [diff.sh:785](../../tests/journey/parity/diff.sh#L785) |
| compat | `gix diff --ignore-cr-at-eol` | diff --ignore-cr-at-eol deferred until renderer lands | [diff.sh:794](../../tests/journey/parity/diff.sh#L794) |
| compat | `gix diff --ignore-space-at-eol` | diff --ignore-space-at-eol deferred until renderer lands | [diff.sh:804](../../tests/journey/parity/diff.sh#L804) |
| compat | `gix diff -b / --ignore-space-change` | diff -b deferred until renderer lands | [diff.sh:814](../../tests/journey/parity/diff.sh#L814) |
| compat | `gix diff -w / --ignore-all-space` | diff -w deferred until renderer lands | [diff.sh:823](../../tests/journey/parity/diff.sh#L823) |
| compat | `gix diff --ignore-blank-lines` | diff --ignore-blank-lines deferred until renderer lands | [diff.sh:833](../../tests/journey/parity/diff.sh#L833) |
| compat | `gix diff --ignore-matching-lines` | diff --ignore-matching-lines='^#' deferred until renderer lands | [diff.sh:843](../../tests/journey/parity/diff.sh#L843) |
| compat | `gix diff --ws-error-highlight` | diff --ws-error-highlight=all deferred until renderer lands | [diff.sh:853](../../tests/journey/parity/diff.sh#L853) |
| compat | `gix diff --check` | diff --check deferred until renderer lands | [diff.sh:863](../../tests/journey/parity/diff.sh#L863) |
| compat | `gix diff --inter-hunk-context` | diff --inter-hunk-context=3 deferred until renderer lands | [diff.sh:873](../../tests/journey/parity/diff.sh#L873) |
| compat | `gix diff -W / --function-context` | diff -W deferred until renderer lands | [diff.sh:883](../../tests/journey/parity/diff.sh#L883) |
| compat | `gix diff --no-renames` | diff --no-renames deferred until renderer lands | [diff.sh:894](../../tests/journey/parity/diff.sh#L894) |
| compat | `gix diff --rename-empty / --no-rename-empty` | diff --no-rename-empty deferred until renderer lands | [diff.sh:904](../../tests/journey/parity/diff.sh#L904) |
| compat | `gix diff -B / --break-rewrites` | diff -B50 deferred until renderer lands | [diff.sh:915](../../tests/journey/parity/diff.sh#L915) |
| compat | `gix diff -M / --find-renames` | diff -M deferred until renderer lands | [diff.sh:925](../../tests/journey/parity/diff.sh#L925) |
| compat | `gix diff -C / --find-copies` | diff -C deferred until renderer lands | [diff.sh:934](../../tests/journey/parity/diff.sh#L934) |
| compat | `gix diff --find-copies-harder` | diff --find-copies-harder deferred until renderer lands | [diff.sh:943](../../tests/journey/parity/diff.sh#L943) |
| compat | `gix diff --diff-filter` | diff --diff-filter=AM deferred until renderer lands | [diff.sh:953](../../tests/journey/parity/diff.sh#L953) |
| compat | `gix diff -D / --irreversible-delete` | diff -D deferred until renderer lands | [diff.sh:963](../../tests/journey/parity/diff.sh#L963) |
| compat | `gix diff -S` | diff -Sfoo deferred until renderer lands | [diff.sh:975](../../tests/journey/parity/diff.sh#L975) |
| compat | `gix diff -G` | diff -Gfoo deferred until renderer lands | [diff.sh:985](../../tests/journey/parity/diff.sh#L985) |
| compat | `gix diff --find-object` | diff --find-object deferred until renderer lands | [diff.sh:995](../../tests/journey/parity/diff.sh#L995) |
| compat | `gix diff --pickaxe-all` | diff --pickaxe-all deferred until renderer lands | [diff.sh:1005](../../tests/journey/parity/diff.sh#L1005) |
| compat | `gix diff --pickaxe-regex` | diff --pickaxe-regex deferred until renderer lands | [diff.sh:1014](../../tests/journey/parity/diff.sh#L1014) |
| compat | `gix diff -R` | diff -R deferred until renderer lands | [diff.sh:1025](../../tests/journey/parity/diff.sh#L1025) |
| compat | `gix diff --relative` | diff --relative deferred until renderer lands | [diff.sh:1035](../../tests/journey/parity/diff.sh#L1035) |
| compat | `gix diff --no-relative` | diff --no-relative deferred until renderer lands | [diff.sh:1044](../../tests/journey/parity/diff.sh#L1044) |
| compat | `gix diff --skip-to` | diff --skip-to deferred until renderer lands | [diff.sh:1053](../../tests/journey/parity/diff.sh#L1053) |
| compat | `gix diff --rotate-to` | diff --rotate-to deferred until renderer lands | [diff.sh:1063](../../tests/journey/parity/diff.sh#L1063) |
| compat | `gix diff <path>` | diff with trailing pathspec deferred until renderer lands | [diff.sh:1073](../../tests/journey/parity/diff.sh#L1073) |
| compat | `gix diff --submodule` | diff --submodule=log deferred until renderer lands | [diff.sh:1085](../../tests/journey/parity/diff.sh#L1085) |
| compat | `gix diff --ignore-submodules` | diff --ignore-submodules=all deferred until renderer lands | [diff.sh:1094](../../tests/journey/parity/diff.sh#L1094) |
| compat | `gix diff --ita-invisible-in-index` | diff --ita-invisible-in-index deferred until renderer lands | [diff.sh:1104](../../tests/journey/parity/diff.sh#L1104) |
| compat | `gix diff --textconv / --no-textconv` | diff --textconv deferred until renderer lands | [diff.sh:1114](../../tests/journey/parity/diff.sh#L1114) |
| compat | `gix diff --ext-diff / --no-ext-diff` | diff --ext-diff deferred until renderer lands | [diff.sh:1124](../../tests/journey/parity/diff.sh#L1124) |
| compat | `gix diff --exit-code` | diff --exit-code semantic-parity (exit 1 on diff) deferred | [diff.sh:1136](../../tests/journey/parity/diff.sh#L1136) |
| compat | `gix diff --quiet` | diff --quiet semantic-parity (exit 1 on diff) deferred | [diff.sh:1145](../../tests/journey/parity/diff.sh#L1145) |
| compat | `gix diff -1 / --base` | diff -1 deferred until renderer lands | [diff.sh:1157](../../tests/journey/parity/diff.sh#L1157) |
| compat | `gix diff -2 / --ours` | diff -2 deferred until renderer lands | [diff.sh:1166](../../tests/journey/parity/diff.sh#L1166) |
| compat | `gix diff -3 / --theirs` | diff -3 deferred until renderer lands | [diff.sh:1175](../../tests/journey/parity/diff.sh#L1175) |
| compat | `gix diff -0` | diff -0 deferred until renderer lands | [diff.sh:1185](../../tests/journey/parity/diff.sh#L1185) |
| deferred | `gix diff <merge> <merge>^@` | combined-diff (gix-rev lacks ^@ revision syntax for parent-set expansion; renderer also unimplemented) | [diff.sh:1197](../../tests/journey/parity/diff.sh#L1197) |
| deferred | `gix diff --combined-all-paths` | combined-diff per-parent path emission requires combined-diff renderer (not yet implemented) | [diff.sh:1205](../../tests/journey/parity/diff.sh#L1205) |

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
| compat | `gix log --oneline` | gix log --oneline pretty-format divergence deferred — clap accepted | [log.sh:428](../../tests/journey/parity/log.sh#L428) |
| compat | `gix log --pretty=oneline` | gix log --pretty=oneline format emission deferred — clap accepted | [log.sh:439](../../tests/journey/parity/log.sh#L439) |
| compat | `gix log --pretty=short` | gix log --pretty=short format emission deferred — clap accepted | [log.sh:448](../../tests/journey/parity/log.sh#L448) |
| compat | `gix log --pretty=medium` | gix log --pretty=medium format emission deferred — clap accepted | [log.sh:457](../../tests/journey/parity/log.sh#L457) |
| compat | `gix log --pretty=full` | gix log --pretty=full format emission deferred — clap accepted | [log.sh:466](../../tests/journey/parity/log.sh#L466) |
| compat | `gix log --pretty=fuller` | gix log --pretty=fuller format emission deferred — clap accepted | [log.sh:475](../../tests/journey/parity/log.sh#L475) |
| compat | `gix log --pretty=raw` | gix log --pretty=raw format emission deferred — clap accepted | [log.sh:485](../../tests/journey/parity/log.sh#L485) |
| compat | `gix log --pretty=reference` | gix log --pretty=reference format emission deferred — clap accepted | [log.sh:494](../../tests/journey/parity/log.sh#L494) |
| compat | `gix log --format=%H` | gix log --format=<fmt> custom formatter deferred — clap accepted | [log.sh:505](../../tests/journey/parity/log.sh#L505) |
| compat | `gix log --format='%h %s'` | gix log --format=<fmt> custom formatter deferred — clap accepted | [log.sh:515](../../tests/journey/parity/log.sh#L515) |
| compat | `gix log --abbrev-commit` | gix log --abbrev-commit hash-width control deferred — clap accepted | [log.sh:525](../../tests/journey/parity/log.sh#L525) |
| compat | `gix log --no-abbrev-commit` | gix log --no-abbrev-commit hash-width control deferred — clap accepted | [log.sh:535](../../tests/journey/parity/log.sh#L535) |
| compat | `gix log --abbrev=<n>` | gix log --abbrev=<n> hash-width control deferred — clap accepted | [log.sh:545](../../tests/journey/parity/log.sh#L545) |
| compat | `gix log --reverse` | gix log --reverse output-order reversal deferred — flag accepted | [log.sh:556](../../tests/journey/parity/log.sh#L556) |
| compat | `gix log --topo-order` | gix log --topo-order already-default for gix's topo walker — flag accepted | [log.sh:565](../../tests/journey/parity/log.sh#L565) |
| compat | `gix log --date-order` | gix log --date-order commit-date ordering deferred — flag accepted | [log.sh:574](../../tests/journey/parity/log.sh#L574) |
| compat | `gix log --author-date-order` | gix log --author-date-order author-date ordering deferred — flag accepted | [log.sh:583](../../tests/journey/parity/log.sh#L583) |
| compat | `gix log --first-parent` | gix log --first-parent merge-parent selection deferred — flag accepted | [log.sh:592](../../tests/journey/parity/log.sh#L592) |
| compat | `gix log --decorate` | gix log --decorate ref-name emission deferred — clap accepted | [log.sh:626](../../tests/journey/parity/log.sh#L626) |
| compat | `gix log --decorate=short` | gix log --decorate=short ref-name emission deferred — clap accepted | [log.sh:635](../../tests/journey/parity/log.sh#L635) |
| compat | `gix log --decorate=full` | gix log --decorate=full ref-name emission deferred — clap accepted | [log.sh:644](../../tests/journey/parity/log.sh#L644) |
| compat | `gix log --decorate=no` | gix log --decorate=no suppressed (never emitted decorations anyway) — clap accepted | [log.sh:653](../../tests/journey/parity/log.sh#L653) |
| compat | `gix log --no-decorate` | gix log --no-decorate suppressed (never emitted decorations anyway) — clap accepted | [log.sh:662](../../tests/journey/parity/log.sh#L662) |
| compat | `gix log --decorate-refs=<pattern>` | gix log --decorate-refs filter deferred — clap accepted | [log.sh:672](../../tests/journey/parity/log.sh#L672) |
| compat | `gix log --decorate-refs-exclude=<pattern>` | gix log --decorate-refs-exclude filter deferred — clap accepted | [log.sh:682](../../tests/journey/parity/log.sh#L682) |
| compat | `gix log --clear-decorations` | gix log --clear-decorations decoration-filter reset deferred — clap accepted | [log.sh:692](../../tests/journey/parity/log.sh#L692) |
| compat | `gix log --source` | gix log --source ref-prefix emission deferred — clap accepted | [log.sh:702](../../tests/journey/parity/log.sh#L702) |
| compat | `gix log --graph` | gix log --graph ASCII-art commit graph deferred — clap accepted | [log.sh:713](../../tests/journey/parity/log.sh#L713) |
| compat | `gix log -p` | gix log -p per-commit diff emission deferred — clap accepted | [log.sh:724](../../tests/journey/parity/log.sh#L724) |
| compat | `gix log -s` | gix log -s --no-patch diff suppression (vs -p) deferred — clap accepted | [log.sh:733](../../tests/journey/parity/log.sh#L733) |
| compat | `gix log --stat` | gix log --stat diffstat emission deferred — clap accepted | [log.sh:742](../../tests/journey/parity/log.sh#L742) |
| compat | `gix log --shortstat` | gix log --shortstat summary emission deferred — clap accepted | [log.sh:751](../../tests/journey/parity/log.sh#L751) |
| compat | `gix log --numstat` | gix log --numstat machine-readable diffstat deferred — clap accepted | [log.sh:760](../../tests/journey/parity/log.sh#L760) |
| compat | `gix log --name-only` | gix log --name-only path listing deferred — clap accepted | [log.sh:769](../../tests/journey/parity/log.sh#L769) |
| compat | `gix log --name-status` | gix log --name-status status-letter path listing deferred — clap accepted | [log.sh:778](../../tests/journey/parity/log.sh#L778) |
| compat | `gix log --raw` | gix log --raw diff emission deferred — clap accepted | [log.sh:787](../../tests/journey/parity/log.sh#L787) |
| compat | `gix log -M` | gix log -M/--find-renames rename detection deferred — clap accepted | [log.sh:796](../../tests/journey/parity/log.sh#L796) |
| compat | `gix log --follow <file>` | gix log --follow rename-following deferred — flag accepted, positional treated as pathspec hint | [log.sh:811](../../tests/journey/parity/log.sh#L811) |
| compat | `gix log --full-diff -- <path>` | gix log --full-diff diff-scope override deferred — clap accepted | [log.sh:821](../../tests/journey/parity/log.sh#L821) |
| compat | `gix log -L <start>,<end>:<file>` | gix log -L line-range traversal deferred — flag accepted, range parser and file-lookup unwired | [log.sh:834](../../tests/journey/parity/log.sh#L834) |
| compat | `gix log --date=relative` | gix log --date=relative date-format emission deferred — clap accepted | [log.sh:845](../../tests/journey/parity/log.sh#L845) |
| compat | `gix log --date=iso` | gix log --date=iso date-format emission deferred — clap accepted | [log.sh:854](../../tests/journey/parity/log.sh#L854) |
| compat | `gix log --date=short` | gix log --date=short date-format emission deferred — clap accepted | [log.sh:863](../../tests/journey/parity/log.sh#L863) |
| compat | `gix log --date=raw` | gix log --date=raw date-format emission deferred — clap accepted | [log.sh:872](../../tests/journey/parity/log.sh#L872) |
| compat | `gix log --date=unix` | gix log --date=unix date-format emission deferred — clap accepted | [log.sh:881](../../tests/journey/parity/log.sh#L881) |
| compat | `gix log --date=format:<strftime>` | gix log --date=format:<strftime> strftime emission deferred — clap accepted | [log.sh:890](../../tests/journey/parity/log.sh#L890) |
| compat | `gix log -m` | gix log -m per-parent merge diff deferred — clap accepted | [log.sh:901](../../tests/journey/parity/log.sh#L901) |
| compat | `gix log -c` | gix log -c combined merge diff deferred — clap accepted | [log.sh:910](../../tests/journey/parity/log.sh#L910) |
| compat | `gix log --cc` | gix log --cc dense combined merge diff deferred — clap accepted | [log.sh:919](../../tests/journey/parity/log.sh#L919) |
| compat | `gix log --diff-merges=off` | gix log --diff-merges=off merge-diff control deferred — clap accepted | [log.sh:928](../../tests/journey/parity/log.sh#L928) |
| compat | `gix log --diff-merges=first-parent` | gix log --diff-merges=first-parent merge-diff control deferred — clap accepted | [log.sh:937](../../tests/journey/parity/log.sh#L937) |
| compat | `gix log --mailmap` | gix log --mailmap author/committer rewriting deferred — clap accepted | [log.sh:948](../../tests/journey/parity/log.sh#L948) |
| compat | `gix log --no-mailmap` | gix log --no-mailmap .mailmap bypass deferred — clap accepted | [log.sh:957](../../tests/journey/parity/log.sh#L957) |
| compat | `gix log --log-size` | gix log --log-size message-length header deferred — clap accepted | [log.sh:966](../../tests/journey/parity/log.sh#L966) |
| compat | `gix log --notes` | gix log --notes refs/notes emission deferred — clap accepted | [log.sh:975](../../tests/journey/parity/log.sh#L975) |
| compat | `gix log --no-notes` | gix log --no-notes refs/notes suppression deferred — clap accepted | [log.sh:985](../../tests/journey/parity/log.sh#L985) |
| compat | `gix log --show-signature` | gix log --show-signature GPG verification deferred — clap accepted | [log.sh:994](../../tests/journey/parity/log.sh#L994) |
| compat | `gix log --color=always` | gix log --color=always ANSI color emission deferred — clap accepted | [log.sh:1005](../../tests/journey/parity/log.sh#L1005) |
| compat | `gix log --no-color` | gix log --no-color color-suppression deferred (never emitted colors anyway) — clap accepted | [log.sh:1014](../../tests/journey/parity/log.sh#L1014) |
| compat | `gix log --boundary` | gix log --boundary + --not boundary-marker emission deferred — clap accepted | [log.sh:1025](../../tests/journey/parity/log.sh#L1025) |
| compat | `gix log --ancestry-path` | gix log --ancestry-path ancestry filter deferred — clap accepted | [log.sh:1034](../../tests/journey/parity/log.sh#L1034) |
| compat | `gix log -G<regex>` | gix log -G pickaxe line-add/remove regex deferred — flag accepted | [log.sh:1045](../../tests/journey/parity/log.sh#L1045) |
| compat | `gix log -S<string>` | gix log -S pickaxe occurrence-count deferred — flag accepted | [log.sh:1054](../../tests/journey/parity/log.sh#L1054) |
| compat | `gix log --pickaxe-regex` | gix log --pickaxe-regex -S-as-regex mode deferred — flag accepted | [log.sh:1063](../../tests/journey/parity/log.sh#L1063) |
| compat | `gix log --pickaxe-all` | gix log --pickaxe-all merge-inclusion in pickaxe deferred — flag accepted | [log.sh:1072](../../tests/journey/parity/log.sh#L1072) |
| compat | `gix log --cherry` | gix log --cherry patch-equivalence detection deferred — flag accepted | [log.sh:1083](../../tests/journey/parity/log.sh#L1083) |
| compat | `gix log --cherry-mark` | gix log --cherry-mark equivalence-class annotation deferred — flag accepted | [log.sh:1092](../../tests/journey/parity/log.sh#L1092) |
| compat | `gix log --cherry-pick` | gix log --cherry-pick equivalence-class filter deferred — flag accepted | [log.sh:1101](../../tests/journey/parity/log.sh#L1101) |
| compat | `gix log --left-only` | gix log --left-only symmetric-diff side filter deferred — flag accepted | [log.sh:1110](../../tests/journey/parity/log.sh#L1110) |
| compat | `gix log --right-only` | gix log --right-only symmetric-diff side filter deferred — flag accepted | [log.sh:1119](../../tests/journey/parity/log.sh#L1119) |
| compat | `gix log --left-right` | gix log --left-right symmetric-diff side annotation deferred — flag accepted | [log.sh:1128](../../tests/journey/parity/log.sh#L1128) |
| compat | `gix log -g / --walk-reflogs` | gix log --walk-reflogs reflog traversal mode deferred — flag accepted | [log.sh:1139](../../tests/journey/parity/log.sh#L1139) |
| compat | `gix log --grep-reflog=<pattern>` | gix log --grep-reflog reflog message filter deferred — flag accepted | [log.sh:1148](../../tests/journey/parity/log.sh#L1148) |
| compat | `gix log --simplify-by-decoration` | gix log --simplify-by-decoration history simplification deferred — flag accepted | [log.sh:1159](../../tests/journey/parity/log.sh#L1159) |
| compat | `gix log --simplify-merges` | gix log --simplify-merges merge-simplification deferred — flag accepted | [log.sh:1168](../../tests/journey/parity/log.sh#L1168) |
| compat | `gix log --full-history` | gix log --full-history disables history simplification — flag accepted, simplification never applied in gix | [log.sh:1177](../../tests/journey/parity/log.sh#L1177) |
| compat | `gix log --dense` | gix log --dense alias for --full-history — flag accepted | [log.sh:1186](../../tests/journey/parity/log.sh#L1186) |
| compat | `gix log --sparse` | gix log --sparse sparse-history mode deferred — flag accepted | [log.sh:1195](../../tests/journey/parity/log.sh#L1195) |
| compat | `gix log --no-walk` | gix log --no-walk traversal suppression deferred — flag accepted | [log.sh:1204](../../tests/journey/parity/log.sh#L1204) |
| compat | `gix log --do-walk` | gix log --do-walk --no-walk override deferred — flag accepted | [log.sh:1213](../../tests/journey/parity/log.sh#L1213) |
| compat | `gix log --in-commit-order` | gix log --in-commit-order emission-order override deferred — flag accepted | [log.sh:1222](../../tests/journey/parity/log.sh#L1222) |
| compat | `gix log --exclude=<pattern>` | gix log --exclude ref-category exclusion deferred — flag accepted | [log.sh:1233](../../tests/journey/parity/log.sh#L1233) |
| compat | `gix log --glob=<pattern>` | gix log --glob glob ref-selection deferred — flag accepted | [log.sh:1242](../../tests/journey/parity/log.sh#L1242) |
| compat | `gix log --alternate-refs` | gix log --alternate-refs alternates traversal deferred — flag accepted | [log.sh:1251](../../tests/journey/parity/log.sh#L1251) |
| compat | `gix log --parents` | gix log --parents parent-id emission deferred — flag accepted | [log.sh:1262](../../tests/journey/parity/log.sh#L1262) |
| compat | `gix log --children` | gix log --children child-id emission deferred — flag accepted | [log.sh:1271](../../tests/journey/parity/log.sh#L1271) |
| compat | `gix log --show-pulls` | gix log --show-pulls merge rejoin detection deferred — flag accepted | [log.sh:1280](../../tests/journey/parity/log.sh#L1280) |
| compat | `gix log --show-linear-break` | gix log --show-linear-break linear-break marker deferred — flag accepted | [log.sh:1289](../../tests/journey/parity/log.sh#L1289) |
| compat | `gix log -z` | gix log -z NUL-terminator emission deferred — flag accepted | [log.sh:1298](../../tests/journey/parity/log.sh#L1298) |
| compat | `gix log --count` | gix log --count count-only suppression deferred — flag accepted | [log.sh:1307](../../tests/journey/parity/log.sh#L1307) |
| compat | `gix log --submodule=<mode>` | gix log --submodule diff rendering mode deferred — flag accepted | [log.sh:1318](../../tests/journey/parity/log.sh#L1318) |
| compat | `gix log --unified=<n>` | gix log --unified=<n> unified-context width control deferred — flag accepted | [log.sh:1334](../../tests/journey/parity/log.sh#L1334) |
| compat | `gix log --summary` | gix log --summary extended-header summary emission deferred — flag accepted | [log.sh:1344](../../tests/journey/parity/log.sh#L1344) |
| compat | `gix log --compact-summary` | gix log --compact-summary compact-summary emission deferred — flag accepted | [log.sh:1354](../../tests/journey/parity/log.sh#L1354) |
| compat | `gix log --minimal` | gix log --minimal Myers minimal-diff variant deferred — flag accepted | [log.sh:1364](../../tests/journey/parity/log.sh#L1364) |
| compat | `gix log --patience` | gix log --patience patience diff algorithm deferred — flag accepted | [log.sh:1374](../../tests/journey/parity/log.sh#L1374) |
| compat | `gix log --histogram` | gix log --histogram histogram diff algorithm deferred — flag accepted | [log.sh:1384](../../tests/journey/parity/log.sh#L1384) |
| compat | `gix log --diff-filter=<filter>` | gix log --diff-filter=<filter> status-letter filter deferred — flag accepted | [log.sh:1394](../../tests/journey/parity/log.sh#L1394) |
| compat | `gix log --find-object=<oid>` | gix log --find-object=<oid> object-touched filter deferred — flag accepted | [log.sh:1404](../../tests/journey/parity/log.sh#L1404) |
| compat | `gix log --find-copies-harder` | gix log --find-copies-harder aggressive copy detection deferred — flag accepted | [log.sh:1414](../../tests/journey/parity/log.sh#L1414) |
| compat | `gix log --exit-code` | gix log --exit-code diff-presence exit-code bit deferred — empty range used for exit-0 parity — flag accepted | [log.sh:1424](../../tests/journey/parity/log.sh#L1424) |
| compat | `gix log --check` | gix log --check whitespace/conflict-marker check deferred — flag accepted | [log.sh:1434](../../tests/journey/parity/log.sh#L1434) |
| compat | `gix log --binary` | gix log --binary binary patch emission deferred — flag accepted | [log.sh:1444](../../tests/journey/parity/log.sh#L1444) |
| compat | `gix log --full-index` | gix log --full-index full-index hash emission deferred — flag accepted | [log.sh:1454](../../tests/journey/parity/log.sh#L1454) |
| compat | `gix log --remerge-diff` | gix log --remerge-diff remerge-diff rendering deferred — flag accepted | [log.sh:1464](../../tests/journey/parity/log.sh#L1464) |
| compat | `gix log --dirstat` | gix log --dirstat dirstat emission deferred — flag accepted | [log.sh:1474](../../tests/journey/parity/log.sh#L1474) |
| compat | `gix log --ext-diff` | gix log --ext-diff external-diff program dispatch deferred — flag accepted | [log.sh:1484](../../tests/journey/parity/log.sh#L1484) |
| compat | `gix log --no-ext-diff` | gix log --no-ext-diff external-diff suppression deferred — flag accepted | [log.sh:1494](../../tests/journey/parity/log.sh#L1494) |
| compat | `gix log --textconv` | gix log --textconv textconv filter application deferred — flag accepted | [log.sh:1504](../../tests/journey/parity/log.sh#L1504) |
| compat | `gix log --no-textconv` | gix log --no-textconv textconv filter suppression deferred — flag accepted | [log.sh:1514](../../tests/journey/parity/log.sh#L1514) |
| compat | `gix log --text` | gix log --text treat-all-as-text mode deferred — flag accepted | [log.sh:1524](../../tests/journey/parity/log.sh#L1524) |
| compat | `gix log --patch-with-raw` | gix log --patch-with-raw patch+raw composition deferred — flag accepted | [log.sh:1534](../../tests/journey/parity/log.sh#L1534) |
| compat | `gix log --patch-with-stat` | gix log --patch-with-stat patch+stat composition deferred — flag accepted | [log.sh:1544](../../tests/journey/parity/log.sh#L1544) |
| compat | `gix log --color-moved` | gix log --color-moved moved-line highlighting deferred — flag accepted | [log.sh:1554](../../tests/journey/parity/log.sh#L1554) |
| compat | `gix log --word-diff` | gix log --word-diff word-level diff rendering deferred — flag accepted | [log.sh:1564](../../tests/journey/parity/log.sh#L1564) |
| compat | `gix log --word-diff-regex=<regex>` | gix log --word-diff-regex=<regex> word-boundary regex deferred — flag accepted | [log.sh:1574](../../tests/journey/parity/log.sh#L1574) |
| compat | `gix log --ws-error-highlight=<kind>` | gix log --ws-error-highlight=<kind> whitespace-error highlighting deferred — flag accepted | [log.sh:1584](../../tests/journey/parity/log.sh#L1584) |
| compat | `gix log --function-context` | gix log --function-context function-context hunk expansion deferred — flag accepted | [log.sh:1594](../../tests/journey/parity/log.sh#L1594) |
| compat | `gix log --inter-hunk-context=<lines>` | gix log --inter-hunk-context=<lines> inter-hunk merge-context control deferred — flag accepted | [log.sh:1604](../../tests/journey/parity/log.sh#L1604) |
| compat | `gix log --indent-heuristic` | gix log --indent-heuristic indent-heuristic hunk tuning deferred — flag accepted | [log.sh:1614](../../tests/journey/parity/log.sh#L1614) |
| compat | `gix log --no-indent-heuristic` | gix log --no-indent-heuristic indent-heuristic suppression deferred — flag accepted | [log.sh:1624](../../tests/journey/parity/log.sh#L1624) |
| compat | `gix log --irreversible-delete` | gix log --irreversible-delete deletion-only hunk reordering deferred — flag accepted | [log.sh:1634](../../tests/journey/parity/log.sh#L1634) |
| compat | `gix log --no-renames` | gix log --no-renames rename-detection suppression deferred — flag accepted | [log.sh:1644](../../tests/journey/parity/log.sh#L1644) |
| compat | `gix log --rename-empty` | gix log --rename-empty empty-file rename semantics deferred — flag accepted | [log.sh:1654](../../tests/journey/parity/log.sh#L1654) |
| compat | `gix log --no-rename-empty` | gix log --no-rename-empty --rename-empty negation deferred — flag accepted | [log.sh:1664](../../tests/journey/parity/log.sh#L1664) |
| compat | `gix log --ignore-all-space` | gix log --ignore-all-space whitespace-ignoring diff mode deferred — flag accepted | [log.sh:1674](../../tests/journey/parity/log.sh#L1674) |
| compat | `gix log --ignore-blank-lines` | gix log --ignore-blank-lines blank-line ignore mode deferred — flag accepted | [log.sh:1684](../../tests/journey/parity/log.sh#L1684) |
| compat | `gix log --ignore-cr-at-eol` | gix log --ignore-cr-at-eol CR-at-EOL ignore deferred — flag accepted | [log.sh:1694](../../tests/journey/parity/log.sh#L1694) |
| compat | `gix log --ignore-matching-lines=<regex>` | gix log --ignore-matching-lines=<regex> regex-match line-ignore deferred — flag accepted | [log.sh:1704](../../tests/journey/parity/log.sh#L1704) |
| compat | `gix log --ignore-space-at-eol` | gix log --ignore-space-at-eol EOL-whitespace ignore deferred — flag accepted | [log.sh:1714](../../tests/journey/parity/log.sh#L1714) |
| compat | `gix log --ignore-space-change` | gix log --ignore-space-change whitespace-amount ignore deferred — flag accepted | [log.sh:1724](../../tests/journey/parity/log.sh#L1724) |
| compat | `gix log --src-prefix=<prefix>` | gix log --src-prefix=<prefix> source-prefix override deferred — flag accepted | [log.sh:1734](../../tests/journey/parity/log.sh#L1734) |
| compat | `gix log --dst-prefix=<prefix>` | gix log --dst-prefix=<prefix> destination-prefix override deferred — flag accepted | [log.sh:1744](../../tests/journey/parity/log.sh#L1744) |
| compat | `gix log --no-prefix` | gix log --no-prefix a/ b/ prefix suppression deferred — flag accepted | [log.sh:1754](../../tests/journey/parity/log.sh#L1754) |
| compat | `gix log --relative` | gix log --relative pathname-relativization deferred — flag accepted | [log.sh:1764](../../tests/journey/parity/log.sh#L1764) |
| compat | `gix log --no-relative` | gix log --no-relative --relative negation deferred — flag accepted | [log.sh:1774](../../tests/journey/parity/log.sh#L1774) |
| compat | `gix log --output=<file>` | gix log --output=<file> diff-output redirection deferred — flag accepted | [log.sh:1784](../../tests/journey/parity/log.sh#L1784) |
| compat | `gix log --reflog` | gix log --reflog HEAD reflog inclusion as tips deferred — flag accepted | [log.sh:1794](../../tests/journey/parity/log.sh#L1794) |
| compat | `gix log --stdin` | gix log --stdin stdin rev-argument reader deferred — empty stdin keeps exit 0 — flag accepted | [log.sh:1804](../../tests/journey/parity/log.sh#L1804) |
| compat | `gix log --ignore-missing` | gix log --ignore-missing missing-object tolerance deferred — flag accepted | [log.sh:1814](../../tests/journey/parity/log.sh#L1814) |
| deferred | `gix log --merge` | --merge precondition check not yet implemented — git exits 128 when no merge pseudoref exists, gix walks HEAD | [log.sh:1827](../../tests/journey/parity/log.sh#L1827) |
| compat | `gix log --since-as-filter=<time>` | gix log --since-as-filter=<time> --since-as-filter predicate deferred — flag accepted | [log.sh:1836](../../tests/journey/parity/log.sh#L1836) |
| compat | `gix log --exclude-first-parent-only` | gix log --exclude-first-parent-only first-parent exclude semantics deferred — flag accepted | [log.sh:1846](../../tests/journey/parity/log.sh#L1846) |
| compat | `gix log --remove-empty` | gix log --remove-empty empty-commit removal deferred — flag accepted | [log.sh:1856](../../tests/journey/parity/log.sh#L1856) |
| compat | `gix log --single-worktree` | gix log --single-worktree single-worktree ref scoping deferred — flag accepted | [log.sh:1866](../../tests/journey/parity/log.sh#L1866) |
| compat | `gix log --encoding=<enc>` | gix log --encoding=<enc> commit-message encoding conversion deferred — flag accepted | [log.sh:1876](../../tests/journey/parity/log.sh#L1876) |
| compat | `gix log --expand-tabs[=<n>]` | gix log --expand-tabs[=<n>] tab-expansion deferred — flag accepted | [log.sh:1886](../../tests/journey/parity/log.sh#L1886) |
| compat | `gix log --no-expand-tabs` | gix log --no-expand-tabs --expand-tabs negation deferred — flag accepted | [log.sh:1896](../../tests/journey/parity/log.sh#L1896) |
| compat | `gix log --basic-regexp` | gix log --basic-regexp POSIX basic-regex mode for --grep deferred — flag accepted | [log.sh:1913](../../tests/journey/parity/log.sh#L1913) |
| compat | `gix log -P / --perl-regexp` | gix log -P / --perl-regexp Perl-regex mode for --grep deferred — flag accepted | [log.sh:1923](../../tests/journey/parity/log.sh#L1923) |
| compat | `gix log --exclude-hidden=<section>` | gix log --exclude-hidden=<section> hidden-refs filter deferred — flag accepted | [log.sh:1933](../../tests/journey/parity/log.sh#L1933) |
| compat | `gix log --bisect` | gix log --bisect bisect-output emission deferred — flag accepted | [log.sh:1943](../../tests/journey/parity/log.sh#L1943) |
| compat | `gix log --relative-date` | gix log --relative-date relative-date shorthand deferred (equivalent to --date=relative) — flag accepted | [log.sh:1953](../../tests/journey/parity/log.sh#L1953) |
| compat | `gix log --dd` | gix log --dd --diff-merges=dd alias deferred — flag accepted | [log.sh:1963](../../tests/journey/parity/log.sh#L1963) |
| compat | `gix log --no-diff-merges` | gix log --no-diff-merges --diff-merges suppression deferred — flag accepted | [log.sh:1973](../../tests/journey/parity/log.sh#L1973) |
| compat | `gix log --combined-all-paths` | gix log --combined-all-paths combined-diff per-parent path emission deferred — flag accepted | [log.sh:1983](../../tests/journey/parity/log.sh#L1983) |
| compat | `gix log --output-indicator-new=<char>` | gix log --output-indicator-new=<char> custom add-indicator character deferred — flag accepted | [log.sh:1993](../../tests/journey/parity/log.sh#L1993) |
| compat | `gix log --output-indicator-old=<char>` | gix log --output-indicator-old=<char> custom remove-indicator character deferred — flag accepted | [log.sh:2003](../../tests/journey/parity/log.sh#L2003) |
| compat | `gix log --output-indicator-context=<char>` | gix log --output-indicator-context=<char> custom context-indicator character deferred — flag accepted | [log.sh:2013](../../tests/journey/parity/log.sh#L2013) |
| compat | `gix log -t (show tree objects)` | gix log -t (show tree objects) tree-object diff emission deferred — flag accepted | [log.sh:2023](../../tests/journey/parity/log.sh#L2023) |
| compat | `gix log --anchored=<text>` | gix log --anchored=<text> anchored-diff algorithm deferred — flag accepted | [log.sh:2033](../../tests/journey/parity/log.sh#L2033) |
| compat | `gix log --cumulative` | gix log --cumulative cumulative-dirstat emission deferred — flag accepted | [log.sh:2043](../../tests/journey/parity/log.sh#L2043) |
| compat | `gix log --dirstat-by-file` | gix log --dirstat-by-file dirstat per-file counting mode deferred — flag accepted | [log.sh:2053](../../tests/journey/parity/log.sh#L2053) |
| compat | `gix log --no-color-moved` | gix log --no-color-moved --color-moved negation deferred — flag accepted | [log.sh:2063](../../tests/journey/parity/log.sh#L2063) |
| compat | `gix log --color-moved-ws=<mode>` | gix log --color-moved-ws=<mode> moved-line whitespace mode deferred — flag accepted | [log.sh:2073](../../tests/journey/parity/log.sh#L2073) |
| compat | `gix log --no-color-moved-ws` | gix log --no-color-moved-ws --color-moved-ws negation deferred — flag accepted | [log.sh:2083](../../tests/journey/parity/log.sh#L2083) |
| compat | `gix log --color-words` | gix log --color-words word-level coloring deferred — flag accepted | [log.sh:2093](../../tests/journey/parity/log.sh#L2093) |
| compat | `gix log -B / --break-rewrites` | gix log -B / --break-rewrites break-rewrites detection deferred — flag accepted | [log.sh:2103](../../tests/journey/parity/log.sh#L2103) |
| compat | `gix log -C / --find-copies` | gix log -C / --find-copies copy detection deferred — flag accepted | [log.sh:2113](../../tests/journey/parity/log.sh#L2113) |
| compat | `gix log -l<num>` | gix log -l<num> rename-detection scan cap deferred — flag accepted | [log.sh:2123](../../tests/journey/parity/log.sh#L2123) |
| compat | `gix log -O / --orderfile=<file>` | gix log -O / --orderfile=<file> path-ordering file deferred — flag accepted | [log.sh:2133](../../tests/journey/parity/log.sh#L2133) |
| compat | `gix log --skip-to=<path>` | gix log --skip-to=<path> diff skip-to-path deferred — flag accepted | [log.sh:2143](../../tests/journey/parity/log.sh#L2143) |
| compat | `gix log --rotate-to=<path>` | gix log --rotate-to=<path> diff rotate-to-path deferred — flag accepted | [log.sh:2153](../../tests/journey/parity/log.sh#L2153) |
| compat | `gix log -R (reverse diff inputs)` | gix log -R (reverse diff inputs) old/new side swap deferred — flag accepted | [log.sh:2163](../../tests/journey/parity/log.sh#L2163) |
| compat | `gix log --ignore-submodules[=<when>]` | gix log --ignore-submodules[=<when>] submodule-diff ignore mode deferred — flag accepted | [log.sh:2173](../../tests/journey/parity/log.sh#L2173) |
| compat | `gix log --default-prefix` | gix log --default-prefix default-prefix restore deferred — flag accepted | [log.sh:2183](../../tests/journey/parity/log.sh#L2183) |
| compat | `gix log --line-prefix=<prefix>` | gix log --line-prefix=<prefix> per-line prefix emission deferred — flag accepted | [log.sh:2193](../../tests/journey/parity/log.sh#L2193) |
| compat | `gix log --ita-invisible-in-index` | gix log --ita-invisible-in-index intent-to-add invisibility deferred — flag accepted | [log.sh:2203](../../tests/journey/parity/log.sh#L2203) |
| compat | `gix log --show-notes-by-default` | gix log --show-notes-by-default notes-by-default emission deferred — flag accepted | [log.sh:2219](../../tests/journey/parity/log.sh#L2219) |
| compat | `gix log --show-notes` | gix log --show-notes deprecated --show-notes alias deferred — flag accepted | [log.sh:2229](../../tests/journey/parity/log.sh#L2229) |
| compat | `gix log --standard-notes` | gix log --standard-notes deprecated --standard-notes alias deferred — flag accepted | [log.sh:2239](../../tests/journey/parity/log.sh#L2239) |
| compat | `gix log --no-standard-notes` | gix log --no-standard-notes deprecated --no-standard-notes alias deferred — flag accepted | [log.sh:2249](../../tests/journey/parity/log.sh#L2249) |
| compat | `gix log --no-use-mailmap` | gix log --no-use-mailmap --no-use-mailmap (alias of --no-mailmap) deferred — flag accepted | [log.sh:2259](../../tests/journey/parity/log.sh#L2259) |

## merge

| Class | Section | Reason | Source |
|---|---|---|---|
| compat | `gix merge <commit> (already up to date)` | deferred until merge driver lands | [merge.sh:127](../../tests/journey/parity/merge.sh#L127) |
| compat | `gix merge <commit> (fast-forward)` | deferred until merge driver lands | [merge.sh:142](../../tests/journey/parity/merge.sh#L142) |
| compat | `gix merge <commit> (3-way merge, clean)` | deferred until merge driver lands | [merge.sh:161](../../tests/journey/parity/merge.sh#L161) |
| compat | `gix merge <commit> <commit> (octopus)` | deferred until merge driver lands | [merge.sh:174](../../tests/journey/parity/merge.sh#L174) |
| compat | `gix merge -n` | deferred until merge driver lands | [merge.sh:203](../../tests/journey/parity/merge.sh#L203) |
| compat | `gix merge --stat` | deferred until merge driver lands | [merge.sh:213](../../tests/journey/parity/merge.sh#L213) |
| compat | `gix merge --no-stat` | deferred until merge driver lands | [merge.sh:222](../../tests/journey/parity/merge.sh#L222) |
| compat | `gix merge --summary` | deferred until merge driver lands | [merge.sh:232](../../tests/journey/parity/merge.sh#L232) |
| compat | `gix merge --no-summary` | deferred until merge driver lands | [merge.sh:241](../../tests/journey/parity/merge.sh#L241) |
| deferred | `gix merge --compact-summary` | system git 2.47.3 lacks --compact-summary; vendor/git v2.54.0 has it | [merge.sh:255](../../tests/journey/parity/merge.sh#L255) |
| compat | `gix merge --log` | deferred until merge driver lands | [merge.sh:269](../../tests/journey/parity/merge.sh#L269) |
| compat | `gix merge --log=<n>` | deferred until merge driver lands | [merge.sh:278](../../tests/journey/parity/merge.sh#L278) |
| compat | `gix merge --no-log` | deferred until merge driver lands | [merge.sh:287](../../tests/journey/parity/merge.sh#L287) |
| compat | `gix merge --squash` | deferred until merge driver lands | [merge.sh:301](../../tests/journey/parity/merge.sh#L301) |
| compat | `gix merge --no-squash` | deferred until merge driver lands | [merge.sh:311](../../tests/journey/parity/merge.sh#L311) |
| compat | `gix merge --commit` | deferred until merge driver lands | [merge.sh:321](../../tests/journey/parity/merge.sh#L321) |
| compat | `gix merge --no-commit` | deferred until merge driver lands | [merge.sh:331](../../tests/journey/parity/merge.sh#L331) |
| compat | `gix merge --edit` | deferred until merge driver lands | [merge.sh:342](../../tests/journey/parity/merge.sh#L342) |
| compat | `gix merge --no-edit` | deferred until merge driver lands | [merge.sh:352](../../tests/journey/parity/merge.sh#L352) |
| compat | `gix merge --cleanup=<mode>` | deferred until merge driver lands | [merge.sh:363](../../tests/journey/parity/merge.sh#L363) |
| compat | `gix merge --ff` | deferred until merge driver lands | [merge.sh:376](../../tests/journey/parity/merge.sh#L376) |
| compat | `gix merge --no-ff` | deferred until merge driver lands | [merge.sh:386](../../tests/journey/parity/merge.sh#L386) |
| compat | `gix merge --ff-only` | deferred until merge driver lands | [merge.sh:398](../../tests/journey/parity/merge.sh#L398) |
| compat | `gix merge --rerere-autoupdate` | deferred until merge driver lands | [merge.sh:410](../../tests/journey/parity/merge.sh#L410) |
| compat | `gix merge --no-rerere-autoupdate` | deferred until merge driver lands | [merge.sh:419](../../tests/journey/parity/merge.sh#L419) |
| compat | `gix merge --verify-signatures` | deferred until merge driver lands | [merge.sh:433](../../tests/journey/parity/merge.sh#L433) |
| compat | `gix merge --no-verify-signatures` | deferred until merge driver lands | [merge.sh:442](../../tests/journey/parity/merge.sh#L442) |
| compat | `gix merge -s <strategy>` | deferred until merge driver lands | [merge.sh:456](../../tests/journey/parity/merge.sh#L456) |
| compat | `gix merge --strategy=<strategy>` | deferred until merge driver lands | [merge.sh:465](../../tests/journey/parity/merge.sh#L465) |
| compat | `gix merge -X <option>` | deferred until merge driver lands | [merge.sh:476](../../tests/journey/parity/merge.sh#L476) |
| compat | `gix merge --strategy-option=<option>` | deferred until merge driver lands | [merge.sh:485](../../tests/journey/parity/merge.sh#L485) |
| compat | `gix merge -m <msg>` | deferred until merge driver lands | [merge.sh:498](../../tests/journey/parity/merge.sh#L498) |
| compat | `gix merge --message=<msg>` | deferred until merge driver lands | [merge.sh:507](../../tests/journey/parity/merge.sh#L507) |
| compat | `gix merge -F <file>` | deferred until merge driver lands | [merge.sh:521](../../tests/journey/parity/merge.sh#L521) |
| compat | `gix merge --file=<file>` | deferred until merge driver lands | [merge.sh:531](../../tests/journey/parity/merge.sh#L531) |
| compat | `gix merge --into-name <branch>` | deferred until merge driver lands | [merge.sh:541](../../tests/journey/parity/merge.sh#L541) |
| compat | `gix merge -v` | deferred until merge driver lands | [merge.sh:553](../../tests/journey/parity/merge.sh#L553) |
| compat | `gix merge -q` | deferred until merge driver lands | [merge.sh:563](../../tests/journey/parity/merge.sh#L563) |
| compat | `gix merge --allow-unrelated-histories` | deferred until merge driver lands | [merge.sh:616](../../tests/journey/parity/merge.sh#L616) |
| compat | `gix merge --no-allow-unrelated-histories` | deferred until merge driver lands | [merge.sh:625](../../tests/journey/parity/merge.sh#L625) |
| compat | `gix merge --progress` | deferred until merge driver lands | [merge.sh:635](../../tests/journey/parity/merge.sh#L635) |
| compat | `gix merge --no-progress` | deferred until merge driver lands | [merge.sh:644](../../tests/journey/parity/merge.sh#L644) |
| compat | `gix merge -S` | deferred until merge driver lands | [merge.sh:659](../../tests/journey/parity/merge.sh#L659) |
| compat | `gix merge --gpg-sign` | deferred until merge driver lands | [merge.sh:668](../../tests/journey/parity/merge.sh#L668) |
| compat | `gix merge --no-gpg-sign` | deferred until merge driver lands | [merge.sh:678](../../tests/journey/parity/merge.sh#L678) |
| compat | `gix merge --autostash` | deferred until merge driver lands | [merge.sh:690](../../tests/journey/parity/merge.sh#L690) |
| compat | `gix merge --no-autostash` | deferred until merge driver lands | [merge.sh:699](../../tests/journey/parity/merge.sh#L699) |
| compat | `gix merge --overwrite-ignore` | deferred until merge driver lands | [merge.sh:711](../../tests/journey/parity/merge.sh#L711) |
| compat | `gix merge --no-overwrite-ignore` | deferred until merge driver lands | [merge.sh:721](../../tests/journey/parity/merge.sh#L721) |
| compat | `gix merge --signoff` | deferred until merge driver lands | [merge.sh:733](../../tests/journey/parity/merge.sh#L733) |
| compat | `gix merge --no-signoff` | deferred until merge driver lands | [merge.sh:743](../../tests/journey/parity/merge.sh#L743) |
| compat | `gix merge --verify` | deferred until merge driver lands | [merge.sh:755](../../tests/journey/parity/merge.sh#L755) |
| compat | `gix merge --no-verify` | deferred until merge driver lands | [merge.sh:765](../../tests/journey/parity/merge.sh#L765) |

## pull

| Class | Section | Reason | Source |
|---|---|---|---|
| compat | `gix pull <remote> (already up to date)` | deferred until pull driver lands | [pull.sh:138](../../tests/journey/parity/pull.sh#L138) |
| compat | `gix pull <remote> <refspec>` | deferred until pull driver lands | [pull.sh:149](../../tests/journey/parity/pull.sh#L149) |
| deferred | `gix pull <remote> <bad-revspec>` | bad-revspec exit-1 emerges from the fetch step; deferred until pull driver wires fetch composition | [pull.sh:162](../../tests/journey/parity/pull.sh#L162) |
| deferred | `gix pull --compact-summary` | system git 2.47.3 lacks --compact-summary; vendor/git v2.54.0 has it | [pull.sh:343](../../tests/journey/parity/pull.sh#L343) |
| deferred | `gix pull -a` | git fetch --append precedes merge-candidates check; deferred until pull driver wires fetch composition | [pull.sh:611](../../tests/journey/parity/pull.sh#L611) |
| deferred | `gix pull --append` | git fetch --append precedes merge-candidates check; deferred until pull driver wires fetch composition | [pull.sh:616](../../tests/journey/parity/pull.sh#L616) |

## rebase

| Class | Section | Reason | Source |
|---|---|---|---|
| compat | `gix rebase <upstream> (already up to date)` | deferred until rebase driver lands | [rebase.sh:135](../../tests/journey/parity/rebase.sh#L135) |
| compat | `gix rebase <upstream> (fast-forward)` | deferred until rebase driver lands | [rebase.sh:149](../../tests/journey/parity/rebase.sh#L149) |
| compat | `gix rebase <upstream> <branch>` | deferred until rebase driver lands | [rebase.sh:161](../../tests/journey/parity/rebase.sh#L161) |
| compat | `gix rebase --onto` | deferred until rebase driver lands | [rebase.sh:184](../../tests/journey/parity/rebase.sh#L184) |
| compat | `gix rebase --keep-base` | deferred until rebase driver lands | [rebase.sh:192](../../tests/journey/parity/rebase.sh#L192) |
| compat | `gix rebase --root` | deferred until rebase driver lands | [rebase.sh:200](../../tests/journey/parity/rebase.sh#L200) |
| compat | `gix rebase --apply` | deferred until rebase driver lands | [rebase.sh:268](../../tests/journey/parity/rebase.sh#L268) |
| compat | `gix rebase --merge` | deferred until rebase driver lands | [rebase.sh:276](../../tests/journey/parity/rebase.sh#L276) |
| compat | `gix rebase -m` | deferred until rebase driver lands | [rebase.sh:284](../../tests/journey/parity/rebase.sh#L284) |
| compat | `gix rebase --interactive` | deferred until rebase driver lands | [rebase.sh:292](../../tests/journey/parity/rebase.sh#L292) |
| compat | `gix rebase -i` | deferred until rebase driver lands | [rebase.sh:300](../../tests/journey/parity/rebase.sh#L300) |
| compat | `gix rebase --empty=drop` | deferred until rebase driver lands | [rebase.sh:310](../../tests/journey/parity/rebase.sh#L310) |
| compat | `gix rebase --empty=keep` | deferred until rebase driver lands | [rebase.sh:318](../../tests/journey/parity/rebase.sh#L318) |
| compat | `gix rebase --empty=stop` | deferred until rebase driver lands | [rebase.sh:326](../../tests/journey/parity/rebase.sh#L326) |
| compat | `gix rebase --keep-empty` | deferred until rebase driver lands | [rebase.sh:334](../../tests/journey/parity/rebase.sh#L334) |
| compat | `gix rebase --no-keep-empty` | deferred until rebase driver lands | [rebase.sh:342](../../tests/journey/parity/rebase.sh#L342) |
| compat | `gix rebase --reapply-cherry-picks` | deferred until rebase driver lands | [rebase.sh:350](../../tests/journey/parity/rebase.sh#L350) |
| compat | `gix rebase --no-reapply-cherry-picks` | deferred until rebase driver lands | [rebase.sh:358](../../tests/journey/parity/rebase.sh#L358) |
| compat | `gix rebase --allow-empty-message` | deferred until rebase driver lands | [rebase.sh:366](../../tests/journey/parity/rebase.sh#L366) |
| compat | `gix rebase --strategy=ort` | deferred until rebase driver lands | [rebase.sh:376](../../tests/journey/parity/rebase.sh#L376) |
| compat | `gix rebase -s ort` | deferred until rebase driver lands | [rebase.sh:384](../../tests/journey/parity/rebase.sh#L384) |
| compat | `gix rebase --strategy-option=ours` | deferred until rebase driver lands | [rebase.sh:392](../../tests/journey/parity/rebase.sh#L392) |
| compat | `gix rebase -X ours` | deferred until rebase driver lands | [rebase.sh:400](../../tests/journey/parity/rebase.sh#L400) |
| compat | `gix rebase --rebase-merges` | deferred until rebase driver lands | [rebase.sh:408](../../tests/journey/parity/rebase.sh#L408) |
| compat | `gix rebase --rebase-merges=rebase-cousins` | deferred until rebase driver lands | [rebase.sh:416](../../tests/journey/parity/rebase.sh#L416) |
| compat | `gix rebase --rebase-merges=no-rebase-cousins` | deferred until rebase driver lands | [rebase.sh:424](../../tests/journey/parity/rebase.sh#L424) |
| compat | `gix rebase --no-rebase-merges` | deferred until rebase driver lands | [rebase.sh:432](../../tests/journey/parity/rebase.sh#L432) |
| compat | `gix rebase -r` | deferred until rebase driver lands | [rebase.sh:440](../../tests/journey/parity/rebase.sh#L440) |
| compat | `gix rebase --force-rebase` | deferred until rebase driver lands | [rebase.sh:450](../../tests/journey/parity/rebase.sh#L450) |
| compat | `gix rebase -f` | deferred until rebase driver lands | [rebase.sh:458](../../tests/journey/parity/rebase.sh#L458) |
| compat | `gix rebase --no-ff` | deferred until rebase driver lands | [rebase.sh:466](../../tests/journey/parity/rebase.sh#L466) |
| compat | `gix rebase --fork-point` | deferred until rebase driver lands | [rebase.sh:474](../../tests/journey/parity/rebase.sh#L474) |
| compat | `gix rebase --no-fork-point` | deferred until rebase driver lands | [rebase.sh:482](../../tests/journey/parity/rebase.sh#L482) |
| compat | `gix rebase --exec` | deferred until rebase driver lands | [rebase.sh:492](../../tests/journey/parity/rebase.sh#L492) |
| compat | `gix rebase -x` | deferred until rebase driver lands | [rebase.sh:500](../../tests/journey/parity/rebase.sh#L500) |
| compat | `gix rebase --autosquash` | deferred until rebase driver lands | [rebase.sh:508](../../tests/journey/parity/rebase.sh#L508) |
| compat | `gix rebase --no-autosquash` | deferred until rebase driver lands | [rebase.sh:516](../../tests/journey/parity/rebase.sh#L516) |
| compat | `gix rebase --reschedule-failed-exec` | deferred until rebase driver lands | [rebase.sh:524](../../tests/journey/parity/rebase.sh#L524) |
| compat | `gix rebase --no-reschedule-failed-exec` | deferred until rebase driver lands | [rebase.sh:532](../../tests/journey/parity/rebase.sh#L532) |
| compat | `gix rebase --update-refs` | deferred until rebase driver lands | [rebase.sh:540](../../tests/journey/parity/rebase.sh#L540) |
| compat | `gix rebase --no-update-refs` | deferred until rebase driver lands | [rebase.sh:548](../../tests/journey/parity/rebase.sh#L548) |
| compat | `gix rebase --quiet` | deferred until rebase driver lands | [rebase.sh:558](../../tests/journey/parity/rebase.sh#L558) |
| compat | `gix rebase -q` | deferred until rebase driver lands | [rebase.sh:566](../../tests/journey/parity/rebase.sh#L566) |
| compat | `gix rebase --verbose` | deferred until rebase driver lands | [rebase.sh:574](../../tests/journey/parity/rebase.sh#L574) |
| compat | `gix rebase -v` | deferred until rebase driver lands | [rebase.sh:582](../../tests/journey/parity/rebase.sh#L582) |
| compat | `gix rebase --stat` | deferred until rebase driver lands | [rebase.sh:590](../../tests/journey/parity/rebase.sh#L590) |
| compat | `gix rebase --no-stat` | deferred until rebase driver lands | [rebase.sh:598](../../tests/journey/parity/rebase.sh#L598) |
| compat | `gix rebase -n` | deferred until rebase driver lands | [rebase.sh:606](../../tests/journey/parity/rebase.sh#L606) |
| compat | `gix rebase --no-verify` | deferred until rebase driver lands | [rebase.sh:616](../../tests/journey/parity/rebase.sh#L616) |
| compat | `gix rebase --verify` | deferred until rebase driver lands | [rebase.sh:624](../../tests/journey/parity/rebase.sh#L624) |
| compat | `gix rebase -C` | deferred until rebase driver lands | [rebase.sh:634](../../tests/journey/parity/rebase.sh#L634) |
| compat | `gix rebase --ignore-whitespace` | deferred until rebase driver lands | [rebase.sh:642](../../tests/journey/parity/rebase.sh#L642) |
| compat | `gix rebase --whitespace=fix` | deferred until rebase driver lands | [rebase.sh:650](../../tests/journey/parity/rebase.sh#L650) |
| deferred | `gix rebase --trailer` | system git 2.47.3 lacks --trailer; vendor/git v2.54.0 has it | [rebase.sh:665](../../tests/journey/parity/rebase.sh#L665) |
| compat | `gix rebase --signoff` | deferred until rebase driver lands | [rebase.sh:673](../../tests/journey/parity/rebase.sh#L673) |
| compat | `gix rebase --committer-date-is-author-date` | deferred until rebase driver lands | [rebase.sh:681](../../tests/journey/parity/rebase.sh#L681) |
| compat | `gix rebase --reset-author-date` | deferred until rebase driver lands | [rebase.sh:689](../../tests/journey/parity/rebase.sh#L689) |
| compat | `gix rebase --ignore-date` | deferred until rebase driver lands | [rebase.sh:697](../../tests/journey/parity/rebase.sh#L697) |
| compat | `gix rebase --rerere-autoupdate` | deferred until rebase driver lands | [rebase.sh:707](../../tests/journey/parity/rebase.sh#L707) |
| compat | `gix rebase --no-rerere-autoupdate` | deferred until rebase driver lands | [rebase.sh:715](../../tests/journey/parity/rebase.sh#L715) |
| compat | `gix rebase --autostash` | deferred until rebase driver lands | [rebase.sh:725](../../tests/journey/parity/rebase.sh#L725) |
| compat | `gix rebase --no-autostash` | deferred until rebase driver lands | [rebase.sh:733](../../tests/journey/parity/rebase.sh#L733) |
| compat | `gix rebase --gpg-sign` | deferred until rebase driver lands | [rebase.sh:743](../../tests/journey/parity/rebase.sh#L743) |
| compat | `gix rebase --gpg-sign=keyid` | deferred until rebase driver lands | [rebase.sh:751](../../tests/journey/parity/rebase.sh#L751) |
| compat | `gix rebase -S` | deferred until rebase driver lands | [rebase.sh:759](../../tests/journey/parity/rebase.sh#L759) |
| compat | `gix rebase --no-gpg-sign` | deferred until rebase driver lands | [rebase.sh:767](../../tests/journey/parity/rebase.sh#L767) |

## reset

| Class | Section | Reason | Source |
|---|---|---|---|
| compat | `gix reset (default mixed, no args)` | deferred until reset driver lands | [reset.sh:131](../../tests/journey/parity/reset.sh#L131) |
| compat | `gix reset HEAD` | deferred until reset driver lands | [reset.sh:140](../../tests/journey/parity/reset.sh#L140) |
| compat | `gix reset HEAD~1` | deferred until reset driver lands | [reset.sh:149](../../tests/journey/parity/reset.sh#L149) |
| compat | `gix reset (unborn HEAD)` | deferred until reset driver lands | [reset.sh:162](../../tests/journey/parity/reset.sh#L162) |
| compat | `gix reset --mixed HEAD~1` | deferred until reset driver lands | [reset.sh:187](../../tests/journey/parity/reset.sh#L187) |
| compat | `gix reset --soft HEAD~1` | deferred until reset driver lands | [reset.sh:198](../../tests/journey/parity/reset.sh#L198) |
| compat | `gix reset --hard HEAD~1` | deferred until reset driver lands | [reset.sh:211](../../tests/journey/parity/reset.sh#L211) |
| compat | `gix reset --merge HEAD~1` | deferred until reset driver lands | [reset.sh:223](../../tests/journey/parity/reset.sh#L223) |
| compat | `gix reset --keep HEAD~1` | deferred until reset driver lands | [reset.sh:234](../../tests/journey/parity/reset.sh#L234) |
| compat | `gix reset --soft --hard HEAD~1` | deferred until reset driver lands | [reset.sh:249](../../tests/journey/parity/reset.sh#L249) |
| compat | `gix reset --mixed -- a` | deferred until reset driver lands | [reset.sh:273](../../tests/journey/parity/reset.sh#L273) |
| compat | `gix reset -q HEAD~1` | deferred until reset driver lands | [reset.sh:285](../../tests/journey/parity/reset.sh#L285) |
| compat | `gix reset --quiet HEAD~1` | deferred until reset driver lands | [reset.sh:294](../../tests/journey/parity/reset.sh#L294) |
| compat | `gix reset --refresh HEAD` | deferred until reset driver lands | [reset.sh:306](../../tests/journey/parity/reset.sh#L306) |
| compat | `gix reset --no-refresh HEAD` | deferred until reset driver lands | [reset.sh:317](../../tests/journey/parity/reset.sh#L317) |
| compat | `gix reset --recurse-submodules HEAD` | deferred until reset driver lands | [reset.sh:330](../../tests/journey/parity/reset.sh#L330) |
| compat | `gix reset --recurse-submodules=yes HEAD` | deferred until reset driver lands | [reset.sh:342](../../tests/journey/parity/reset.sh#L342) |
| compat | `gix reset --no-recurse-submodules HEAD` | deferred until reset driver lands | [reset.sh:351](../../tests/journey/parity/reset.sh#L351) |
| compat | `gix reset --patch` | deferred until reset driver lands | [reset.sh:365](../../tests/journey/parity/reset.sh#L365) |
| compat | `gix reset -p` | deferred until reset driver lands | [reset.sh:374](../../tests/journey/parity/reset.sh#L374) |
| deferred | `gix reset --auto-advance --patch` | system git 2.47.3 lacks --auto-advance; vendor/git v2.54.0 has it | [reset.sh:387](../../tests/journey/parity/reset.sh#L387) |
| deferred | `gix reset --no-auto-advance --patch` | system git 2.47.3 lacks --auto-advance; vendor/git v2.54.0 has it | [reset.sh:399](../../tests/journey/parity/reset.sh#L399) |
| deferred | `gix reset --unified=3 --patch` | system git 2.47.3 lacks --unified for reset; vendor/git v2.54.0 has it | [reset.sh:410](../../tests/journey/parity/reset.sh#L410) |
| deferred | `gix reset -U 5 --patch` | system git 2.47.3 lacks -U for reset; vendor/git v2.54.0 has it | [reset.sh:419](../../tests/journey/parity/reset.sh#L419) |
| deferred | `gix reset --inter-hunk-context=2 --patch` | system git 2.47.3 lacks --inter-hunk-context for reset; vendor/git v2.54.0 has it | [reset.sh:430](../../tests/journey/parity/reset.sh#L430) |
| compat | `gix reset -N` | deferred until reset driver lands | [reset.sh:443](../../tests/journey/parity/reset.sh#L443) |
| compat | `gix reset --intent-to-add` | deferred until reset driver lands | [reset.sh:452](../../tests/journey/parity/reset.sh#L452) |
| compat | `gix reset --pathspec-from-file=spec.txt` | deferred until reset driver lands | [reset.sh:466](../../tests/journey/parity/reset.sh#L466) |
| compat | `gix reset --pathspec-from-file=spec.txt --pathspec-file-nul` | deferred until reset driver lands | [reset.sh:478](../../tests/journey/parity/reset.sh#L478) |
| compat | `gix reset HEAD -- a` | deferred until reset driver lands | [reset.sh:502](../../tests/journey/parity/reset.sh#L502) |
| compat | `gix reset HEAD a` | deferred until reset driver lands | [reset.sh:514](../../tests/journey/parity/reset.sh#L514) |
| compat | `gix reset -- a` | deferred until reset driver lands | [reset.sh:523](../../tests/journey/parity/reset.sh#L523) |

## show

| Class | Section | Reason | Source |
|---|---|---|---|
| compat | `gix show (default HEAD)` | deferred until show driver lands | [show.sh:117](../../tests/journey/parity/show.sh#L117) |
| compat | `gix show <ref>` | deferred until show driver lands | [show.sh:139](../../tests/journey/parity/show.sh#L139) |
| compat | `gix show <sha>` | deferred until show driver lands | [show.sh:150](../../tests/journey/parity/show.sh#L150) |
| compat | `gix show <tag>` | deferred until show driver lands | [show.sh:173](../../tests/journey/parity/show.sh#L173) |
| compat | `gix show <tree>` | deferred until show driver lands | [show.sh:184](../../tests/journey/parity/show.sh#L184) |
| compat | `gix show <blob>` | deferred until show driver lands | [show.sh:194](../../tests/journey/parity/show.sh#L194) |
| compat | `gix show <obj1> <obj2>` | deferred until show driver lands | [show.sh:205](../../tests/journey/parity/show.sh#L205) |
| compat | `gix show --pretty` | deferred until show driver lands | [show.sh:215](../../tests/journey/parity/show.sh#L215) |
| compat | `gix show --pretty=oneline` | deferred until show driver lands | [show.sh:223](../../tests/journey/parity/show.sh#L223) |
| compat | `gix show --format=%H` | deferred until show driver lands | [show.sh:231](../../tests/journey/parity/show.sh#L231) |
| compat | `gix show --abbrev-commit` | deferred until show driver lands | [show.sh:239](../../tests/journey/parity/show.sh#L239) |
| compat | `gix show --no-abbrev-commit` | deferred until show driver lands | [show.sh:247](../../tests/journey/parity/show.sh#L247) |
| compat | `gix show --oneline` | deferred until show driver lands | [show.sh:255](../../tests/journey/parity/show.sh#L255) |
| compat | `gix show --encoding` | deferred until show driver lands | [show.sh:263](../../tests/journey/parity/show.sh#L263) |
| compat | `gix show --expand-tabs` | deferred until show driver lands | [show.sh:271](../../tests/journey/parity/show.sh#L271) |
| compat | `gix show --no-expand-tabs` | deferred until show driver lands | [show.sh:279](../../tests/journey/parity/show.sh#L279) |
| compat | `gix show --notes` | deferred until show driver lands | [show.sh:287](../../tests/journey/parity/show.sh#L287) |
| compat | `gix show --no-notes` | deferred until show driver lands | [show.sh:295](../../tests/journey/parity/show.sh#L295) |
| compat | `gix show --show-notes-by-default` | deferred until show driver lands | [show.sh:303](../../tests/journey/parity/show.sh#L303) |
| compat | `gix show --show-signature` | deferred until show driver lands | [show.sh:311](../../tests/journey/parity/show.sh#L311) |
| compat | `gix show -p` | deferred until show driver lands | [show.sh:321](../../tests/journey/parity/show.sh#L321) |
| compat | `gix show --patch` | deferred until show driver lands | [show.sh:329](../../tests/journey/parity/show.sh#L329) |
| compat | `gix show -s` | deferred until show driver lands | [show.sh:337](../../tests/journey/parity/show.sh#L337) |
| compat | `gix show --no-patch` | deferred until show driver lands | [show.sh:345](../../tests/journey/parity/show.sh#L345) |
| compat | `gix show -U<n>` | deferred until show driver lands | [show.sh:353](../../tests/journey/parity/show.sh#L353) |
| compat | `gix show --unified=<n>` | deferred until show driver lands | [show.sh:361](../../tests/journey/parity/show.sh#L361) |
| compat | `gix show --output` | deferred until show driver lands | [show.sh:369](../../tests/journey/parity/show.sh#L369) |
| compat | `gix show --output-indicator-new` | deferred until show driver lands | [show.sh:377](../../tests/journey/parity/show.sh#L377) |
| compat | `gix show --output-indicator-old` | deferred until show driver lands | [show.sh:385](../../tests/journey/parity/show.sh#L385) |
| compat | `gix show --output-indicator-context` | deferred until show driver lands | [show.sh:393](../../tests/journey/parity/show.sh#L393) |
| compat | `gix show --raw` | deferred until show driver lands | [show.sh:401](../../tests/journey/parity/show.sh#L401) |
| compat | `gix show --patch-with-raw` | deferred until show driver lands | [show.sh:409](../../tests/journey/parity/show.sh#L409) |
| compat | `gix show --indent-heuristic` | deferred until show driver lands | [show.sh:417](../../tests/journey/parity/show.sh#L417) |
| compat | `gix show --no-indent-heuristic` | deferred until show driver lands | [show.sh:425](../../tests/journey/parity/show.sh#L425) |
| compat | `gix show --minimal` | deferred until show driver lands | [show.sh:433](../../tests/journey/parity/show.sh#L433) |
| compat | `gix show --patience` | deferred until show driver lands | [show.sh:441](../../tests/journey/parity/show.sh#L441) |
| compat | `gix show --histogram` | deferred until show driver lands | [show.sh:449](../../tests/journey/parity/show.sh#L449) |
| compat | `gix show --anchored` | deferred until show driver lands | [show.sh:457](../../tests/journey/parity/show.sh#L457) |
| compat | `gix show --diff-algorithm` | deferred until show driver lands | [show.sh:465](../../tests/journey/parity/show.sh#L465) |
| compat | `gix show --stat` | deferred until show driver lands | [show.sh:473](../../tests/journey/parity/show.sh#L473) |
| compat | `gix show --numstat` | deferred until show driver lands | [show.sh:481](../../tests/journey/parity/show.sh#L481) |
| compat | `gix show --shortstat` | deferred until show driver lands | [show.sh:489](../../tests/journey/parity/show.sh#L489) |
| compat | `gix show --compact-summary` | deferred until show driver lands | [show.sh:497](../../tests/journey/parity/show.sh#L497) |
| compat | `gix show --dirstat` | deferred until show driver lands | [show.sh:505](../../tests/journey/parity/show.sh#L505) |
| compat | `gix show --cumulative` | deferred until show driver lands | [show.sh:513](../../tests/journey/parity/show.sh#L513) |
| compat | `gix show --summary` | deferred until show driver lands | [show.sh:521](../../tests/journey/parity/show.sh#L521) |
| compat | `gix show --patch-with-stat` | deferred until show driver lands | [show.sh:529](../../tests/journey/parity/show.sh#L529) |
| compat | `gix show -z` | deferred until show driver lands | [show.sh:537](../../tests/journey/parity/show.sh#L537) |
| compat | `gix show --name-only` | deferred until show driver lands | [show.sh:545](../../tests/journey/parity/show.sh#L545) |
| compat | `gix show --name-status` | deferred until show driver lands | [show.sh:553](../../tests/journey/parity/show.sh#L553) |
| compat | `gix show --submodule` | deferred until show driver lands | [show.sh:561](../../tests/journey/parity/show.sh#L561) |
| compat | `gix show --color` | deferred until show driver lands | [show.sh:569](../../tests/journey/parity/show.sh#L569) |
| compat | `gix show --no-color` | deferred until show driver lands | [show.sh:577](../../tests/journey/parity/show.sh#L577) |
| compat | `gix show --color-moved` | deferred until show driver lands | [show.sh:585](../../tests/journey/parity/show.sh#L585) |
| compat | `gix show --no-color-moved` | deferred until show driver lands | [show.sh:593](../../tests/journey/parity/show.sh#L593) |
| compat | `gix show --color-moved-ws` | deferred until show driver lands | [show.sh:601](../../tests/journey/parity/show.sh#L601) |
| compat | `gix show --no-color-moved-ws` | deferred until show driver lands | [show.sh:609](../../tests/journey/parity/show.sh#L609) |
| compat | `gix show --word-diff` | deferred until show driver lands | [show.sh:617](../../tests/journey/parity/show.sh#L617) |
| compat | `gix show --word-diff-regex` | deferred until show driver lands | [show.sh:625](../../tests/journey/parity/show.sh#L625) |
| compat | `gix show --color-words` | deferred until show driver lands | [show.sh:633](../../tests/journey/parity/show.sh#L633) |
| compat | `gix show --no-renames` | deferred until show driver lands | [show.sh:641](../../tests/journey/parity/show.sh#L641) |
| compat | `gix show --rename-empty` | deferred until show driver lands | [show.sh:649](../../tests/journey/parity/show.sh#L649) |
| compat | `gix show --no-rename-empty` | deferred until show driver lands | [show.sh:657](../../tests/journey/parity/show.sh#L657) |
| compat | `gix show --check` | deferred until show driver lands | [show.sh:665](../../tests/journey/parity/show.sh#L665) |
| compat | `gix show --ws-error-highlight` | deferred until show driver lands | [show.sh:673](../../tests/journey/parity/show.sh#L673) |
| compat | `gix show --full-index` | deferred until show driver lands | [show.sh:681](../../tests/journey/parity/show.sh#L681) |
| compat | `gix show --binary` | deferred until show driver lands | [show.sh:689](../../tests/journey/parity/show.sh#L689) |
| compat | `gix show --abbrev` | deferred until show driver lands | [show.sh:697](../../tests/journey/parity/show.sh#L697) |
| compat | `gix show -B` | deferred until show driver lands | [show.sh:705](../../tests/journey/parity/show.sh#L705) |
| compat | `gix show -M` | deferred until show driver lands | [show.sh:713](../../tests/journey/parity/show.sh#L713) |
| compat | `gix show --find-renames` | deferred until show driver lands | [show.sh:721](../../tests/journey/parity/show.sh#L721) |
| compat | `gix show -C` | deferred until show driver lands | [show.sh:729](../../tests/journey/parity/show.sh#L729) |
| compat | `gix show --find-copies` | deferred until show driver lands | [show.sh:737](../../tests/journey/parity/show.sh#L737) |
| compat | `gix show --find-copies-harder` | deferred until show driver lands | [show.sh:745](../../tests/journey/parity/show.sh#L745) |
| compat | `gix show -D` | deferred until show driver lands | [show.sh:753](../../tests/journey/parity/show.sh#L753) |
| compat | `gix show --irreversible-delete` | deferred until show driver lands | [show.sh:761](../../tests/journey/parity/show.sh#L761) |
| compat | `gix show -l` | deferred until show driver lands | [show.sh:769](../../tests/journey/parity/show.sh#L769) |
| compat | `gix show --diff-filter` | deferred until show driver lands | [show.sh:777](../../tests/journey/parity/show.sh#L777) |
| compat | `gix show -S` | deferred until show driver lands | [show.sh:785](../../tests/journey/parity/show.sh#L785) |
| compat | `gix show -G` | deferred until show driver lands | [show.sh:793](../../tests/journey/parity/show.sh#L793) |
| compat | `gix show --find-object` | deferred until show driver lands | [show.sh:802](../../tests/journey/parity/show.sh#L802) |
| compat | `gix show --pickaxe-all` | deferred until show driver lands | [show.sh:810](../../tests/journey/parity/show.sh#L810) |
| compat | `gix show --pickaxe-regex` | deferred until show driver lands | [show.sh:818](../../tests/journey/parity/show.sh#L818) |
| compat | `gix show -O` | deferred until show driver lands | [show.sh:827](../../tests/journey/parity/show.sh#L827) |
| compat | `gix show --skip-to` | deferred until show driver lands | [show.sh:835](../../tests/journey/parity/show.sh#L835) |
| compat | `gix show --rotate-to` | deferred until show driver lands | [show.sh:843](../../tests/journey/parity/show.sh#L843) |
| compat | `gix show -R` | deferred until show driver lands | [show.sh:851](../../tests/journey/parity/show.sh#L851) |
| compat | `gix show --relative` | deferred until show driver lands | [show.sh:859](../../tests/journey/parity/show.sh#L859) |
| compat | `gix show --no-relative` | deferred until show driver lands | [show.sh:867](../../tests/journey/parity/show.sh#L867) |
| compat | `gix show -a` | deferred until show driver lands | [show.sh:875](../../tests/journey/parity/show.sh#L875) |
| compat | `gix show --text` | deferred until show driver lands | [show.sh:883](../../tests/journey/parity/show.sh#L883) |
| compat | `gix show --ignore-cr-at-eol` | deferred until show driver lands | [show.sh:891](../../tests/journey/parity/show.sh#L891) |
| compat | `gix show --ignore-space-at-eol` | deferred until show driver lands | [show.sh:899](../../tests/journey/parity/show.sh#L899) |
| compat | `gix show -b` | deferred until show driver lands | [show.sh:907](../../tests/journey/parity/show.sh#L907) |
| compat | `gix show --ignore-space-change` | deferred until show driver lands | [show.sh:915](../../tests/journey/parity/show.sh#L915) |
| compat | `gix show -w` | deferred until show driver lands | [show.sh:923](../../tests/journey/parity/show.sh#L923) |
| compat | `gix show --ignore-all-space` | deferred until show driver lands | [show.sh:931](../../tests/journey/parity/show.sh#L931) |
| compat | `gix show --ignore-blank-lines` | deferred until show driver lands | [show.sh:939](../../tests/journey/parity/show.sh#L939) |
| compat | `gix show --ignore-matching-lines` | deferred until show driver lands | [show.sh:947](../../tests/journey/parity/show.sh#L947) |
| compat | `gix show -I` | deferred until show driver lands | [show.sh:955](../../tests/journey/parity/show.sh#L955) |
| compat | `gix show --inter-hunk-context` | deferred until show driver lands | [show.sh:963](../../tests/journey/parity/show.sh#L963) |
| compat | `gix show -W` | deferred until show driver lands | [show.sh:971](../../tests/journey/parity/show.sh#L971) |
| compat | `gix show --function-context` | deferred until show driver lands | [show.sh:979](../../tests/journey/parity/show.sh#L979) |
| compat | `gix show --exit-code` | deferred until show driver lands | [show.sh:989](../../tests/journey/parity/show.sh#L989) |
| compat | `gix show --quiet` | deferred until show driver lands | [show.sh:997](../../tests/journey/parity/show.sh#L997) |
| compat | `gix show --ext-diff` | deferred until show driver lands | [show.sh:1005](../../tests/journey/parity/show.sh#L1005) |
| compat | `gix show --no-ext-diff` | deferred until show driver lands | [show.sh:1013](../../tests/journey/parity/show.sh#L1013) |
| compat | `gix show --textconv` | deferred until show driver lands | [show.sh:1021](../../tests/journey/parity/show.sh#L1021) |
| compat | `gix show --no-textconv` | deferred until show driver lands | [show.sh:1029](../../tests/journey/parity/show.sh#L1029) |
| compat | `gix show --ignore-submodules` | deferred until show driver lands | [show.sh:1037](../../tests/journey/parity/show.sh#L1037) |
| compat | `gix show --src-prefix` | deferred until show driver lands | [show.sh:1045](../../tests/journey/parity/show.sh#L1045) |
| compat | `gix show --dst-prefix` | deferred until show driver lands | [show.sh:1053](../../tests/journey/parity/show.sh#L1053) |
| compat | `gix show --no-prefix` | deferred until show driver lands | [show.sh:1061](../../tests/journey/parity/show.sh#L1061) |
| compat | `gix show --default-prefix` | deferred until show driver lands | [show.sh:1069](../../tests/journey/parity/show.sh#L1069) |
| compat | `gix show --line-prefix` | deferred until show driver lands | [show.sh:1077](../../tests/journey/parity/show.sh#L1077) |
| compat | `gix show --ita-invisible-in-index` | deferred until show driver lands | [show.sh:1085](../../tests/journey/parity/show.sh#L1085) |
| compat | `gix show --diff-merges` | deferred until show driver lands | [show.sh:1095](../../tests/journey/parity/show.sh#L1095) |
| compat | `gix show --no-diff-merges` | deferred until show driver lands | [show.sh:1103](../../tests/journey/parity/show.sh#L1103) |
| compat | `gix show -c` | deferred until show driver lands | [show.sh:1111](../../tests/journey/parity/show.sh#L1111) |
| compat | `gix show --cc` | deferred until show driver lands | [show.sh:1119](../../tests/journey/parity/show.sh#L1119) |
| compat | `gix show -m` | deferred until show driver lands | [show.sh:1127](../../tests/journey/parity/show.sh#L1127) |
| compat | `gix show --combined-all-paths` | deferred until show driver lands | [show.sh:1136](../../tests/journey/parity/show.sh#L1136) |
| compat | `gix show --remerge-diff` | deferred until show driver lands | [show.sh:1144](../../tests/journey/parity/show.sh#L1144) |
| compat | `gix show -t` | deferred until show driver lands | [show.sh:1152](../../tests/journey/parity/show.sh#L1152) |
| compat | `gix show --dd` | deferred until show driver lands | [show.sh:1160](../../tests/journey/parity/show.sh#L1160) |

## tag

| Class | Section | Reason | Source |
|---|---|---|---|
| compat | `gix tag --sort` | tag --sort=<key> interpreter deferred (key-based sort, descending/version) | [tag.sh:270](../../tests/journey/parity/tag.sh#L270) |
| compat | `gix tag --sort` | tag --sort=<key> interpreter deferred (key-based sort, descending/version) | [tag.sh:273](../../tests/journey/parity/tag.sh#L273) |
| compat | `gix tag --sort` | tag --sort=<key> interpreter deferred (key-based sort, descending/version) | [tag.sh:276](../../tests/journey/parity/tag.sh#L276) |
| compat | `gix tag --column / --no-column` | tag --column packing deferred; Clap accepts, one-per-line output | [tag.sh:306](../../tests/journey/parity/tag.sh#L306) |
| compat | `gix tag --column / --no-column` | tag --column=always packing deferred; Clap accepts, one-per-line output | [tag.sh:309](../../tests/journey/parity/tag.sh#L309) |
| deferred | `gix tag -F / --file` | tag -F - stdin row blocked on expect_parity_reset PARITY_STDIN plumbing | [tag.sh:662](../../tests/journey/parity/tag.sh#L662) |

