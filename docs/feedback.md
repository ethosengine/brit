# Brit feedback

Brit publishes its claim declarations through `brit feedback claims` (one ID per
line) and `brit feedback claims --json`. Release archives ship the JSON as
`claims.json` and record its SHA-256 as `claims_sha256` in `BUILD-INFO.txt`.
The JSON object has a `claims` array and a `cures` array.

A claim ID has the form `<producer>/<claim>[:<param>]`, for example
`brit/capability-absent:cherry-pick.range`. IDs are compiled into Brit and are
append-only. A parameter can name a canonical verb or form, such as
`cherry-pick.range`, or a declared cause, such as `timeout`. Input values such
as paths, revisions, and remotes never enter an ID.

Each declaration contains these fields.

| Field | Meaning |
| --- | --- |
| `id` | The published claim ID. |
| `asserts`, `contradictedBy` | One sentence each describing the claim and what would contradict it. |
| `validityHorizon` | An ISO 8601 duration, currently `P14D` (14 days). |
| `supersedes` | Earlier claim IDs superseded by this declaration. Those IDs stay in the table. |
| `clears` | Clearing conditions drawn from `release-current:<tool>`, `release-at-least:<tool>@<ver>`, `path:<p>`, `bound:<measure@v>`, and `observed`. |
| `polarity` | `negative` or `positive`. |
| `severity` | `trace`, `debug`, `info`, `warn`, `error`, or `fatal`. |
| `weight` | A small nonnegative integer. |
| `ceiling` | `device`, `steward`, `collective`, or `commons`. Brit uses `device` or `steward`. |
| `retireWhen` | An optional retirement condition, serialized as `null` for every current row. |
| `cure` | Optional short action text, omitted when absent. |

Each reviewed entry in `cures` has `claim` (a declared ID), `cite` (the commit,
landing, or note with the root-cause analysis), and `summary` (one plain-text
sentence). Brit currently ships an empty list. When a mapped form completes
successfully and its claim is on the list, Brit also captures that claim with
outcome `passed`. The parsed `cherry-pick` actions map to the published
`cherry-pick.single`, `.range`, `.list`, `.continue`, `.abort`, `.skip`, or
`.status` capability claim.

## Local capture and markers

Capture is enabled by `EPR_FEEDBACK_ROOT` or a `.epr-meta/feedback-policy.json`
in the working directory or an ancestor. Brit passes `EPR_FEEDBACK_CLAIM` and
`EPR_FEEDBACK_EMITTER` to `epr feedback capture` in the child environment. The
argument list is unchanged from earlier releases. Its `--operation` word comes from the
closed vocabulary every installed recorder accepts (`status`, `log`, `help`, `version`,
`surface`, `parse`, `bridge`, `guard`, `build`, `snapshot`, `context`, `integrate`, `init`,
`clone`, `fetch`, `push`, `pull`, `add`, `commit`, `diff`, `branch`, `switch`, `merge`,
`rebase`, `restore`, `tag`, `remote`, `submodule`, `other`), and any other command is sent
as `other`. The claim names the command exactly. The
emitter is `EPR_BIN_RECORD` when set, otherwise
`unrecorded:<version>+<sha12>`. When the build SHA is `unavailable`, the
fallback is `unrecorded:<version>+exe.<sha12>` using SHA-256 over the running
executable, cached by canonical path, size, and mtime.

Brit uses the first nonblank session value from `ELOHIM_SESSION_ID`,
`CLAUDE_SESSION_ID`, `CLAUDE_CODE_SESSION_ID`, `CODEX_THREAD_ID`, and
`CODEX_SESSION_ID`. A nonblank `EPR_FEEDBACK_INVOCATION` further scopes the
session. The key hashes the trimmed session value and, when present, a NUL
separator followed by the trimmed invocation. Brit prefixes that hash with
`session-` or `invocation-`. With neither variable it uses `day-<UTC epoch day>`.

A successful capture creates a marker in
`$XDG_CACHE_HOME/brit/feedback-markers/` (fallback
`~/.cache/brit/feedback-markers/`). The marker keys on session, emitter, and
claim. Command markers include the outcome, and a cure's `passed` observation
has its own marker even if a symptom was already captured. Brit skips the
recorder when the marker exists.

## Spool v1

An unavailable recorder is recorded under `$XDG_STATE_HOME/brit/feedback-spool/`
(fallback `~/.local/state/brit/feedback-spool/`). The filename is the first 32
hex characters of SHA-256 over the session key and recorder identity, followed
by `.json`. Each hash input is framed by its byte length as a little-endian
`u64`. The inputs are the session key, the canonical path's native encoded bytes,
the size as a little-endian `u64`, and mtime as little-endian signed Unix
nanoseconds (`i128`, empty when unavailable). If the executable is missing,
the inputs after the session key are replaced by the bytes of `missing`.
When the recorder is missing, times out or cannot be spawned, Brit skips further
spawns for that session and recorder identity while its record exists. A changed
path, size, or mtime is eligible for another attempt.

A recorder that *refuses* a record is not unavailable: it may accept others. Its
record is keyed on the session key and the claim ID (`<session key>\0<claim id>`
in place of the session key above), so only that claim is not retried in the
session, and other claims are still captured.

Each file is one JSON object, at most 1024 bytes, written to a temporary file
in the directory and published atomically without overwriting an existing record.

| Key | Value |
| --- | --- |
| `v` | The integer `1`. |
| `claim` | `epr/recorder-unavailable:<cause>`. |
| `cause` | `not-found`, `timeout`, `refused`, or `spawn-failed`. |
| `at` | RFC 3339 UTC timestamp at seconds precision, ending in `Z`. |
| `emitter` | The emitter passed to the recorder. |
| `session` | The lowercase hex SHA-256 of the session key. |
| `recorder` | An object with `path` (resolved path or `null`), `size` (bytes or `null`), and `mtime` (Unix seconds or `null`). |
| `stderr` | The recorder's first stderr line, truncated to 256 bytes at a UTF-8 character boundary, or `null`. |

`stderr` and `recorder.path` are open fields that never cross devices. To fit
the size limit, Brit may shorten or omit `stderr` and set `recorder.path` to
`null`. Brit leaves spool files for EPR to drain. When the spool already has
256 entries, a recorder failure adds neither a spool record nor a stderr notice.
A local cache marker still suppresses repeat spawns for that session and recorder
identity. With space available, a visible gap notice is emitted at most once
per session.

Che workspaces wipe `$HOME` at restart, and `XDG_STATE_HOME` may be unset.
Spool files can therefore disappear on restart. That loses notices, never
command correctness.
