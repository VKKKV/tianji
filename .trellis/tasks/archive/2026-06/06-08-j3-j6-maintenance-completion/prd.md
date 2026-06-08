# J3-J6 maintenance completion

## Purpose

Complete the remaining Phase J operational reliability candidates after J1 retention and J2 health/readiness: SQLite backup/export plus additional operator maintenance commands.

## Scope

Add a complete local maintenance toolbox:

1. `tianji maintenance check --sqlite-path <PATH>`
   - Read-only SQLite diagnostics.
   - Reject missing database instead of creating one.
   - Report `tianji.maintenance-check-report.v1` JSON.
   - Include quick_check, foreign_key_check violation count, table counts, latest_run_id, file sizes, page_count, freelist_count, journal_mode.

2. `tianji maintenance backup --sqlite-path <PATH> --output <PATH> [--overwrite]`
   - Create an online-safe SQLite backup/exported DB for operators before destructive actions.
   - Reject missing source.
   - Reject existing output unless `--overwrite` is set.
   - Must not use naive `.sqlite3` file copy when WAL can be present.
   - Prefer `VACUUM INTO` or SQLite-native backup API.
   - Report `tianji.backup-report.v1` JSON with source/output paths, sizes, run count.

3. `tianji maintenance export --sqlite-path <PATH> --output <PATH> [--format json|jsonl] [--include-details] [--overwrite]`
   - Export run history to portable JSON/JSONL.
   - Default summaries use `list_runs` ordering.
   - `--include-details` includes `get_run_summary` for each run.
   - Reject missing source and existing output unless `--overwrite`.
   - Report `tianji.export-report.v1` JSON with output path, format, run_count, bytes.

4. `tianji maintenance compact --sqlite-path <PATH> [--vacuum]`
   - Run WAL checkpoint truncate and optionally VACUUM.
   - Reject missing source.
   - Report `tianji.compact-report.v1` JSON with before/after file sizes, freelist/page counts, checkpoint result.

## Documentation

- README operator quickstart should document suggested sequence:
  1. maintenance check
  2. maintenance backup
  3. maintenance export optional
  4. maintenance retain
  5. maintenance compact
  6. maintenance check
- plan.md should mark Phase J operational reliability complete and remove backup/export from later candidates.
- Update test counts and Rust line counts with real command output before commit.

## Acceptance criteria

- All commands parse and execute in CLI tests.
- Storage tests cover missing source rejection, existing output rejection, seeded DB backup/export/check/compact behavior.
- Backup DB can be opened and queried with existing `list_runs`.
- JSON export parses and contains expected run count.
- JSONL export has one record per run plus metadata, or clearly documented deterministic record count.
- Compact preserves readable history.
- No network/LLM/secrets.

## Verification commands

```bash
cargo test maintenance
cargo test backup
cargo test export
cargo test compact
cargo test check
cargo test
cargo fmt --check
cargo clippy -- -D warnings
git diff --check
```
