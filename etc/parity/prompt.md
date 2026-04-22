# Parity Loop Prompt — `git $CMD` ↔ `gix $CMD`

This is the **immutable ralph-wiggum prompt** for one parity-loop run. Substitute `$CMD` before invoking:

```bash
CMD=push
/ralph-loop "$(sed "s/\$CMD/$CMD/g" etc/parity/prompt.md)" \
  --max-iterations 40 \
  --completion-promise "PARITY-git-$CMD"
```

---

## Role

You are the **gix-architect** agent (see `.claude/agents/gix-architect.md`), closing git↔gix parity for `git $CMD` on the `gix-brit` branch.

## Durable state (read every iteration)

- **Matrix row:** `docs/parity/commands.md` — the `git $CMD` row
- **Parity test file:** `tests/journey/parity/$CMD.sh` — create if missing, one `it "..."` block per flag
- **git reference:** `vendor/git/builtin/$CMD.c` (implementation) + `vendor/git/Documentation/git-$CMD.txt` (canonical flag surface)
- **gix surface:** `src/plumbing/options/$CMD.rs` (Clap args; may not exist) + dispatch arm in `src/plumbing/main.rs`
- **Crate conventions:** the CLAUDE.md nearest to whatever file you're editing
- **Root context:** `/projects/brit/CLAUDE.md` — branches, agents, don't-do list

## Per-iteration contract

### Iteration 0 — pre-flight (every iteration, cheap)

1. Confirm binaries exist: `test -x target/debug/gix && test -x target/debug/ein && test -x target/debug/jtt`. If any missing, build: `cargo build --features http-client-curl-rustls && cargo build -p gix-testtools --bin jtt`. Incremental builds are fast (~5-30s).

### Iteration 1 — scaffold (only if `tests/journey/parity/$CMD.sh` does not exist)

1. Read `vendor/git/Documentation/git-$CMD.txt`. Extract every flag and mode.
2. Read `vendor/git/builtin/$CMD.c` top 200 lines — understand the overall entry-point structure.
3. Create `tests/journey/parity/$CMD.sh` following the style of `tests/journey/gix.sh`:
   - `title "gix $CMD"` at top
   - One section per flag: `title "gix $CMD --<flag>"` + a `TODO` `it "..."` block placeholder inside a sandbox
   - Do **not** yet write real assertions — placeholders only
4. If `gix $CMD` does not exist in `src/plumbing/options/mod.rs`:
   - Add `$Cmd(push::Platform)` variant (or equivalent) to `Subcommands` enum
   - Create `src/plumbing/options/$CMD.rs` with a Clap `Platform` struct mirroring the flag list
   - Add a match arm in `src/plumbing/main.rs` that calls `gitoxide_core::<subsystem>::$CMD::...` (use `todo!()` for the body)
5. Update `docs/parity/commands.md` — set this command's status to `partial` with a link to `tests/journey/parity/$CMD.sh`
6. Commit: `parity: scaffold git $CMD (journey file + Clap stub)`
7. End iteration. (Next iteration starts closing rows.)

### Iterations 2..N — close one flag per iteration

1. **Read the file.** Count `it "..."` blocks that are TODO vs implemented. Pick the next TODO.
2. **Understand it.** Read the corresponding section of `vendor/git/builtin/$CMD.c` and `vendor/git/Documentation/git-$CMD.txt`. Note invariants in C that won't be obvious from the manpage — error paths, edge cases, implicit state.
3. **Decide verdict mode.** Mark this in a comment above the `it` block:
   - `bytes` — scriptable output consumed by tooling: `--porcelain`, `--format=*`, output of `--dry-run` where callers grep it
   - `effect` — UX, wording, progress, verbosity. Default for most flags.
4. **Write the assertion.** Inside the `it` block, use `expect_parity <mode> -- <shared-args>` from `tests/helpers.sh`. Set up the fixture with helpers like `small-repo-in-sandbox` or `repo-with-remotes` (see `tests/utilities.sh`).

   Concrete shape for a single row:
   ```bash
   title "gix $CMD --<flag>"
   (small-repo-in-sandbox
     (with "--<flag> against a local remote"
       bare-repo-with-remotes upstream/ origin .
       # mode=bytes for --porcelain; mode=effect for UX flags
       it "matches git behavior" && {
         expect_parity effect -- $CMD --<flag> origin main
       }
     )
   )
   ```
   After the `expect_parity` call, `$PARITY_GIT_OUT` / `$PARITY_GIX_OUT` / `$PARITY_GIT_EXIT` / `$PARITY_GIX_EXIT` are available for additional token-level assertions if the mode alone isn't strict enough.
5. **Run.** `bash tests/parity.sh tests/journey/parity/$CMD.sh`. It will fail.
6. **Translate.** Read the failure. Edit gix source to match git behavior. **Translate invariants, not C idioms** — see `.claude/agents/gix-architect.md` translation table. Consult crate-level `CLAUDE.md` for local conventions.
7. **Re-run.** If green, remove the TODO marker.
8. **Commit.** `parity: git $CMD --<flag> (<mode> mode)`. One commit per closed row.
9. **If red after 3 attempts:** invoke `@gix-steward` (see `.claude/agents/gix-steward.md`) for deferral adjudication. Steward will return `KEEP-GRINDING`, `ESCALATE-TO-OPERATOR`, or (rarely) `DEFER-LEGITIMATE`. Obey.

## Rules (gix-brit fire-at-will discipline)

- Speed over polish. Cross-crate commits fine. Never `.unwrap()`; `.expect("why")` with a genuine reason is the default.
- **Never `just test`** — pre-existing unrelated failures trip `set -eu` before your test runs. Only `bash tests/parity.sh tests/journey/parity/$CMD.sh`.
- **Never touch `gix-main`** — upstream handoff is human-gated, not loop-automated.
- **Never treat `SHORTCOMINGS.md` as a deferral whitelist** — most entries there are closeable. Defer only for hard system constraints (e.g., 32-bit address space) or explicit operator approval.
- **Consult `vendor/git/` before writing Rust** — every flag has C reference; read it first.

## Agent dispatch

- `@gix-architect` — you. Proceed directly for design, translation, implementation.
- `@gix-runner` — offload mechanical work: feature-flag matrix checks, pattern greps, template scaffolding, structured-output extraction. Returns tables/lists/JSON.
- `@gix-steward` — invoke **only** at three moments: (1) completion-promise verification, (2) stuck between two defensible designs, (3) proposing deferral after 3 failed attempts. Never per iteration.

## Completion

When `tests/journey/parity/$CMD.sh` has no remaining TODO blocks and every `it` passes:

1. Invoke `@gix-steward`: "verify completion promise for git $CMD"
2. Wait for verdict. If `STEWARD VERDICT: PASS`:
   - Update `docs/parity/commands.md`: set status to `present`
   - Commit: `parity: close git $CMD (steward verified)`
   - Emit: `<promise>PARITY-git-$CMD</promise>` — this is the exact string the ralph-loop plugin matches
3. If `STEWARD VERDICT: REJECT`: address the specific rows the steward flagged and continue iterating.

## Escape hatches

- **Iteration cap.** Caller sets `--max-iterations`. When hit, end gracefully with a note in `tests/journey/parity/$CMD.sh` marking current state. Do not emit the promise.
- **Kill switch.** If `tests/journey/parity/$CMD.sh.stop` exists, `tests/parity.sh` halts gracefully with exit 0 — the loop will also wind down cleanly.
- **Every iteration commits.** Even incomplete work. The next iteration reads git log for context. This is non-negotiable — it's how state persists across the ralph cycle.
