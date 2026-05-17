# Phase 5.2: Worldline SQLite Persistence

> Ref: plan.md §1 Current State (Phase 5.2 ☐)
> Status: in_progress

## Summary

Persist Worldline and Baseline to SQLite so worldline trees survive daemon
restarts and enable checkpoint/resume. Currently worldlines are in-memory only
with a static atomic counter for IDs; Baseline is a stub JSON file.

## Requirements

1. Add `worldlines` table to `initialize_schema` in storage.rs
   - `id INTEGER PRIMARY KEY` (WorldlineId)
   - `parent_id INTEGER` (nullable, for fork tree)
   - `worldline_json TEXT NOT NULL` (full Worldline JSON, causal_graph excluded)
   - `created_at TEXT NOT NULL` (ISO 8601)

2. Add `baselines` table
   - `id INTEGER PRIMARY KEY AUTOINCREMENT`
   - `baseline_json TEXT NOT NULL`
   - `locked_at TEXT NOT NULL`

3. Add write functions to storage.rs:
   - `save_worldline(conn, worldline) → Result<i64>`
   - `save_baseline(conn, baseline) → Result<()>`
   - `clear_baseline(conn) → Result<()>`

4. Add read functions:
   - `load_worldline(conn, id) → Result<Option<Worldline>>`
   - `load_latest_worldlines(conn, limit) → Result<Vec<Worldline>>`
   - `load_baseline(conn) → Result<Option<Baseline>>`

5. Update `fork_worldline` to accept optional `&Connection` for DB-assigned ID

6. Update `handle_baseline` in main.rs to use SQLite instead of JSON file

7. Wire `HongmengCheckpoint::save` into simulation loop via connection

## Files Changed

- `src/storage.rs` — schema + CRUD
- `src/nuwa/sandbox.rs` — fork_worldline with optional DB
- `src/main.rs` — handle_baseline rewrite
- `src/hongmeng/simulation.rs` — checkpoint save wiring
- `src/lib.rs` — re-export new public functions

## Verification

- `cargo build` zero error
- `cargo test` all pass
- `cargo clippy -- -D warnings` clean
