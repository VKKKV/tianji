# J1 SQLite retention policy

## Purpose

Start Phase J operational reliability with a small, deterministic, operator-safe storage maintenance slice: prune old persisted runs while preserving the most recent N runs and cleaning unreferenced canonical source items.

## Scope

Implement retention policy v1 for the local SQLite read model.

In scope:
- Add storage-layer retention API for `runs` history.
- Keep the latest N runs by run id descending.
- Delete older runs inside one transaction.
- Rely on existing `ON DELETE CASCADE` for run-scoped tables.
- Clean orphan `source_items` no longer referenced by `raw_items` or `normalized_events`.
- Add a CLI command for operators.
- Return a JSON-friendly report.
- Add tests for storage behavior and CLI behavior.
- Update README and plan for Phase J1.

Out of scope:
- Time-based TTL.
- Daemon scheduling/automatic retention.
- Remote backup/restore.
- Source health history persistence.
- Schema rewrites or destructive migrations.

## Proposed CLI

```bash
tianji maintenance retain --sqlite-path <PATH> --keep-last-runs <N>
```

Output should be JSON, consistent with existing operator commands. Minimal fields:

```json
{
  "schema_version": "tianji.retention-report.v1",
  "sqlite_path": "runs/tianji.sqlite3",
  "keep_last_runs": 2,
  "runs_before": 3,
  "runs_after": 2,
  "deleted_runs": 1,
  "deleted_source_items": 0
}
```

## Storage contract

- `keep_last_runs` may be zero; this deletes all runs and then removes orphan `source_items`.
- If `keep_last_runs` is greater than or equal to current run count, no runs are deleted.
- Use an initialized connection from `src/storage.rs`; keep one-shot CLI storage behavior path-based.
- Do not silently ignore SQLite errors.
- Use parameterized SQL.
- Do all deletion and orphan cleanup inside one transaction.

Suggested API names:

```rust
pub struct RetentionReport { ... }
pub fn apply_retention_policy(sqlite_path: &str, keep_last_runs: usize) -> Result<RetentionReport, TianJiError>
```

## Acceptance criteria

1. Three persisted runs pruned with `keep_last_runs=2` leave only the latest two run ids.
2. Run-scoped rows in `raw_items`, `normalized_events`, `scored_events`, and `intervention_candidates` are cascaded.
3. Orphan `source_items` are cleaned after deleting all runs with `keep_last_runs=0`.
4. `keep_last_runs` larger than current count is a no-op with accurate report counts.
5. CLI parses and executes `maintenance retain` and emits the report.
6. README documents the operator maintenance command.
7. `plan.md` records Phase J1 complete/in progress consistently.

## Verification commands

```bash
cargo test retention
cargo test maintenance
cargo test
cargo fmt --check
cargo clippy -- -D warnings
cargo run -- run --fixture tests/fixtures/sample_feed.xml --sqlite-path /tmp/tianji-j1.sqlite3
cargo run -- maintenance retain --sqlite-path /tmp/tianji-j1.sqlite3 --keep-last-runs 1
```
