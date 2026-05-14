# Fix Known Bugs B3/B5/B8/B9

## Goal

Fix the next batch of known bugs from plan.md §12 to improve daemon observability, error fidelity, API safety, and code hygiene.

## Bugs

### B3 — daemon error info lost (TianJiError→String) [HIGH]

**File**: `src/daemon.rs:449–456`
**Problem**: `run_pipeline_for_job` returns `Result<(), String>` and converts `TianJiError` to a formatted string via `format!("TianJiError: {e}")`, losing the structured variant (`Usage`/`Input`/`Io`/`Json`/`Storage`). Downstream, `set_job_failed` further flattens it.
**Fix**: Change return type to `Result<(), TianJiError>`, use `?` operator directly (TianJiError implements `From` for sub-errors). The `set_job_failed` call still formats to string for storage, but the intermediate conversion preserves variant info.

### B5 — daemon child stdout/stderr discarded [HIGH]

**File**: `src/main.rs:464–465`
**Problem**: `Command::new` sets both `stdout` and `stderr` to `Stdio::null()`, silently discarding all daemon output. If the daemon crashes or panics, diagnostic info is irretrievably lost.
**Fix**: Redirect child stdout/stderr to a log file next to the socket path (`<socket-path>.log`). Use `Stdio::from(File)` to attach log file handles. Create the log file before spawning.

### B8 — API limit param unbounded [HIGH]

**File**: `src/api.rs:113–139`
**Problem**: `RunsQuery.limit` is `Option<i64>`. The `i64→usize` cast is unsafe on 32-bit targets (silent wrap). Absurd values are silently clamped to `MAX_RUNS_LIMIT=200` with no feedback.
**Fix**: Change type to `Option<u32>`. Return HTTP 400 if `limit > MAX_RUNS_LIMIT` instead of silently clamping. This is a safer API contract.

### B9 — Utility functions duplicated 3x [HIGH]

**Files**: `src/scoring.rs:508`, `src/grouping.rs:546`, `src/storage.rs:1513` (round2); `src/grouping.rs:528`, `src/storage.rs:1497` (days_since_epoch)
**Problem**: `round2` is identically copy-pasted across 3 modules. `days_since_epoch` duplicated in 2 modules. Minor inconsistency: `storage.rs` uses `.expect()`, others use `.unwrap()`.
**Fix**: Create `src/utils.rs` with `pub fn round2()` and `pub fn days_since_epoch()`. Delete private duplicates, import from `utils`. Standardize on `.expect()`.

## Scope

- Fix B3: change `run_pipeline_for_job` return type to `Result<(), TianJiError>`
- Fix B5: redirect daemon child stdout/stderr to `<socket-path>.log`
- Fix B8: change `RunsQuery.limit` to `Option<u32>`, return 400 on overflow
- Fix B9: extract `round2` and `days_since_epoch` to `src/utils.rs`
- Update plan.md §12 bug table
- Run `cargo test`, `cargo fmt --check`, `cargo clippy -- -D warnings`

## Acceptance Criteria

1. `run_pipeline_for_job` returns `Result<(), TianJiError>` — no `.map_err(|e| format!(...))`
2. Daemon child stdout/stderr go to a log file, not `/dev/null`
3. `RunsQuery.limit` is `Option<u32>`; values > `MAX_RUNS_LIMIT` return 400
4. `round2` and `days_since_epoch` defined once in `src/utils.rs`, no duplicates
5. All existing tests pass
6. `cargo fmt --check` + `cargo clippy -- -D warnings` clean
7. plan.md §12 bug table updated

## Out of Scope

- B4 (include_str! Python coupling) — different scope, separate task
- B10 (backtrack string matching) — needs design discussion
- M3C schedule, Crucix Phase 2, TUI, Hongmeng, Nuwa
