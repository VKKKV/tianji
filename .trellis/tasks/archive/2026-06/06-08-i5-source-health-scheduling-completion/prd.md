# I5 source health scheduling completion

## Purpose

Complete remaining source-management candidates: live source polling metadata persistence, source health history, and operator scheduling integration.

## Scope

Add persistent source health history to the local SQLite database and wire the source registry CLI to optionally persist/read it.

In scope:
- Add SQLite table(s) for source health checks/history.
- Add storage APIs to persist `SourceRunReport` results with source id, kind, status, checked_at, counts, dominant_field, risk_level, error, and optional run id if available.
- Add storage APIs to read latest health per source.
- Extend `tianji sources` with optional `--sqlite-path <PATH>`.
- When `--sqlite-path` is present and `--run-fixtures` or `--fetch-live` is used, persist source run/check results.
- When `--sqlite-path` is present for listing, enrich source statuses with persisted latest `last_success`/`last_error` where available.
- Keep default `tianji sources --config <PATH>` validation-only and no network/no DB writes unless sqlite path is explicitly provided.
- Document the operator scheduling pattern: external scheduler/cron/systemd invokes `tianji sources --config ... --fetch-live --sqlite-path ...`; TianJi records health history but does not spawn a scheduler daemon in this slice.

Out of scope:
- Built-in cron daemon.
- External network tests.
- Secret/config persistence.

## Acceptance criteria

1. New source health history schema is initialized additively.
2. `sources --sqlite-path <DB>` listing can show persisted last_success / last_error without running/fetching sources.
3. `sources --run-fixtures --sqlite-path <DB>` persists health rows for enabled fixture runs and skipped disabled sources.
4. `sources --fetch-live --sqlite-path <DB>` persists health rows using injected/mock fetch tests only.
5. Default sources listing still performs no network I/O and no DB writes when `--sqlite-path` is absent.
6. README documents source health history and external scheduling integration.
7. plan.md marks source-management later candidates complete and updates real test/source metrics.

## Verification commands

```bash
cargo test source_health
cargo test source_registry
cargo test sources
cargo test
cargo fmt --check
cargo clippy -- -D warnings
git diff --check
```
