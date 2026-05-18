# Phase D3 — SQLite Connection Pool

## Goal

Replace repeated per-request SQLite connection opens in long-lived API / daemon runtime paths with a small shared connection pool, while preserving CLI one-shot storage behavior and existing SQLite contracts.

This is a production hardening slice for TianJi's local daemon/API mode. The API currently stores only a `sqlite_path` in `AppState` and calls storage functions that open a new `rusqlite::Connection` for each request. D3 should introduce a bounded pool for the long-lived runtime so repeated API reads reuse initialized SQLite connections.

## Scope

In scope:
- `src/storage.rs`
- `src/api.rs`
- `src/daemon.rs`
- tests in the same files or existing test modules

Out of scope:
- Rewriting all CLI storage APIs to pool-based APIs
- Changing database schema
- Changing HTTP route contracts or response envelopes
- Adding async SQLite or external service dependencies
- Persisting secrets or changing config file format unless strictly necessary

## Current behavior

Long-lived API route handlers call storage helpers by path:
- `list_runs(&state.sqlite_path, ...)`
- `get_run_summary(&state.sqlite_path, ...)`
- `get_latest_run_id(&state.sqlite_path)`
- `compare_runs(&state.sqlite_path, ...)`

Those helpers open their own SQLite connection internally. That is acceptable for CLI one-shot commands but wasteful in daemon/API paths.

## Requirements

1. Add a small SQLite connection pool abstraction.
   - The pool must be owned by long-lived runtime state.
   - Use `rusqlite::Connection`; no async SQLite rewrite.
   - The pool must be bounded. Default size can be small, e.g. 4.
   - Connection checkout must be safe across concurrent API requests.
   - Returned connections must be reusable after use.

2. Preserve SQLite initialization contracts for every pooled connection.
   - `PRAGMA foreign_keys = ON`
   - `PRAGMA journal_mode = WAL`
   - schema initialized via existing `initialize_schema`
   - Do not weaken read-limit contracts.

3. Keep CLI one-shot storage simple.
   - Existing public path-based helpers may remain for CLI usage.
   - Prefer adding connection-based helper variants where needed by API rather than forcing all CLI call sites through a pool.

4. Use pool only in long-lived runtime paths.
   - `api::AppState` should hold a cloneable pool handle, not only `sqlite_path`.
   - `api::serve_api` should create or receive the pool and install it into router state.
   - `daemon::serve` should construct/reuse the pool for API serving.
   - Worker job write path can stay one-shot unless a clean shared design falls out naturally; do not risk changing job semantics.

5. Preserve API behavior.
   - Routes, status codes, JSON envelopes, and fixtures must not change except for internal implementation.
   - Existing deterministic ordering must remain.

6. Add tests.
   - Pool initializes schema and pragmas on checked-out connections.
   - Pool reuses returned connections and is bounded.
   - API state can serve read endpoints through pooled storage.
   - Existing valid API/storage tests continue to pass.

7. Verification commands.
   - `cargo fmt`
   - `cargo test --quiet`
   - `cargo clippy -- -D warnings`

## Suggested design

Implement in `src/storage.rs`:

```rust
#[derive(Clone)]
pub struct SqlitePool { ... }

impl SqlitePool {
    pub fn new(path: impl Into<PathBuf>, max_connections: usize) -> Result<Self, TianJiError>;
    pub fn get(&self) -> Result<PooledConnection<'_>, TianJiError>;
}
```

A simple implementation is enough:
- `Arc<Mutex<Vec<Connection>>>`
- path stored once
- max size stored once
- checkout pops an idle connection, or opens a new one if under max, or waits/blocks briefly on a `Condvar`
- `Drop` on `PooledConnection` returns the connection to the pool

If implementing a blocking wait is too large, a simpler bounded eager pool is acceptable:
- Open `max_connections` during `new`
- `get()` waits on `Condvar` until a connection is available

Add connection-based helper variants where API needs them, for example:
- `list_runs_with_conn(conn: &Connection, ...)`
- `get_run_summary_with_conn(conn: &Connection, ...)`
- `get_latest_run_id_with_conn(conn: &Connection)`
- `compare_runs_with_conn(conn: &Connection, ...)`

The existing path-based helpers should delegate to the connection-based variants after opening/initializing a one-shot connection.

## Acceptance criteria

- API route handlers no longer open SQLite by path for every read request.
- Long-lived API state stores and clones a pool handle.
- Pooled connections have `foreign_keys=ON` and WAL/schema initialized.
- Existing CLI path-based storage helpers continue to work.
- Full test and clippy suite pass.
