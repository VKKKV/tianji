# PRD — Phase D3: SQLite Connection Pool

> Priority: D3 | Spec: `.trellis/spec/backend/phase-d3-sqlite-connection-pool.md`

## Goal

Implement a bounded SQLite connection pool for TianJi's long-lived daemon/API runtime so repeated HTTP read requests reuse initialized `rusqlite::Connection`s instead of opening a new connection per request.

## Background

TianJi's CLI one-shot paths can continue opening SQLite connections per command. The daemon/API runtime is long-lived and currently keeps only `sqlite_path` in `api::AppState`; route handlers call path-based storage helpers that open fresh connections internally.

This task hardens production behavior without changing public API contracts.

## Requirements

1. Add a bounded SQLite pool abstraction in `src/storage.rs`.
   - Use `rusqlite::Connection`.
   - Use std synchronization primitives; no async SQLite rewrite.
   - Default pool size should be small and deterministic, e.g. 4.
   - Checked-out connections must return to the pool on drop.
   - Pool handle must be cloneable for `axum` state.

2. Preserve connection initialization on every pooled connection.
   - `PRAGMA foreign_keys = ON`
   - `PRAGMA journal_mode = WAL`
   - `initialize_schema(...)`

3. Keep existing CLI-oriented path APIs working.
   - Existing callers of `list_runs`, `get_run_summary`, `get_latest_run_id`, `compare_runs`, etc. should keep compiling.
   - Add connection-based variants if needed, and make existing path APIs delegate to them.

4. Update long-lived runtime paths.
   - `src/api.rs`: `AppState` should store a pool handle rather than only `sqlite_path` for read queries.
   - HTTP handlers should checkout from the pool and call connection-based storage helpers.
   - `serve_api` should create/use the pool and attach it to router state.
   - `src/daemon.rs`: construct/reuse the pool for API serving.
   - Do not refactor the worker write path unless necessary.

5. Preserve behavior.
   - No route/path/status/envelope changes.
   - Existing deterministic ordering and read limits remain unchanged.
   - Existing tests continue to pass.

6. Tests.
   - Pool initializes schema and pragmas.
   - Pool is bounded and reuses returned connections.
   - API can serve at least one persisted-run read endpoint through pooled state.
   - Existing storage/API tests pass.

## Suggested files

- `src/storage.rs`
- `src/api.rs`
- `src/daemon.rs`

Do not modify unrelated modules.

## Verification

Run:

```bash
cargo fmt
cargo test --quiet
cargo clippy -- -D warnings
```

## Non-goals

- Do not introduce Diesel/sqlx/r2d2 unless already present. A small local pool is enough.
- Do not rewrite all storage APIs around the pool.
- Do not change schema or API response payloads.
- Do not commit from OpenCode; Hermes will inspect and commit.
