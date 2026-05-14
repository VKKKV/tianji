# PRD: Fix Known Bugs B1 + B6

## Context

plan.md §12 lists 10 known bugs. Priority fix order: B1 → B2 → B6 → B7.
Research confirms **B2 and B7 are already fixed** (no HashMap in src/, all SQL has LIMIT).
Remaining: **B1 (zombie process)** and **B6 (regex recompilation)**.

## Bugs

### B1 — Zombie process leak (CRITICAL)

**File**: `src/main.rs:475–509`
**Problem**: `handle_daemon_start` spawns a child process via `Command::spawn()`. On the error paths, `terminate_child()` correctly calls `child.wait()`. But on the success path (after socket + API readiness checks pass), the `Child` handle is dropped without calling `wait()`. In Rust, dropping a `Child` without waiting does NOT reap the OS process — the child becomes a zombie until the parent exits.

**Fix**: After success checks pass, deliberately detach from the child by calling `std::mem::forget(child)`. This leaks the handle intentionally, which is correct for a daemon: the child is meant to outlive the parent CLI process, and the OS will reap it when it eventually exits. Alternative: `child.wait()` would block, which is wrong. `try_wait()` would return `Ok(None)` for a running process but still leak if not called again.

### B6 — Regex recompiled every call (HIGH)

**File**: `src/normalize.rs:182–190`
**Problem**: `match_patterns()` has a fast path for `ACTOR_PATTERNS` and `REGION_PATTERNS` (pointer-equality check → use pre-compiled `LazyLock` regex maps). The fallthrough branch compiles regex via `Regex::new(pattern)` on every call.

**Current callers**: All callers pass `ACTOR_PATTERNS` or `REGION_PATTERNS`, so the fallthrough branch is dead code in practice. But it exists and would be a performance trap if future code used it.

**Fix**: Remove the fallthrough `Regex::new` branch. Replace it with either:
- (a) A panic or unreachable!() to catch misuse, OR
- (b) Refactor `match_patterns` to accept `&[(&'static str, Regex)]` and eliminate the pattern-string branch entirely.

Option (b) is cleaner: change the function signature so it only accepts pre-compiled regex slices, making the bug impossible.

## Scope

- Fix B1: Add `std::mem::forget(child)` on the daemon start success path in `src/main.rs`
- Fix B6: Refactor `match_patterns` to accept pre-compiled regex slices, remove `Regex::new` fallthrough in `src/normalize.rs`
- Update plan.md §12 "已知问题" table: mark B2, B7 as "已修复", B1 and B6 as "修复中"
- Run `cargo test`, `cargo fmt --check`, `cargo clippy -- -D warnings`

## Out of Scope

- B3–B5, B8–B10 (remaining known bugs — deferred to future tasks)
- Crucix Delta Engine Phase 2 (separate task)
- Any module restructuring or refactoring beyond the minimal fix

## Acceptance Criteria

1. `handle_daemon_start` success path does not leak a zombie `Child` handle
2. `match_patterns` no longer contains any `Regex::new` call
3. All existing tests pass (`cargo test`)
4. `cargo fmt --check` clean
5. `cargo clippy -- -D warnings` clean
6. plan.md §12 bug table updated
