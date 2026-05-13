# Milestone 2: Storage + History Parity

**Created**: 2026-05-13
**Assignee**: kita
**Priority**: P1

## Goal

Port the Python storage/history layer to Rust so that `cargo run -- run --fixture ...` persists a run to SQLite and `cargo run -- history/show/compare` produces field-for-field compatible JSON output with the Python oracle.

## What I Already Know

* Rust Milestone 1A+1B complete — pipeline produces parity `RunArtifact` with Python.
* Python storage layer spans 4 modules: `storage_write.py`, `storage_views.py`, `storage_filters.py`, `storage_compare.py`.
* Python CLI exposes 3 history subcommands: `history`, `history-show`, `history-compare`, all outputting JSON.
* Frozen contract fixtures exist at `tests/fixtures/contracts/` defining exact field vocabulary for list/detail/compare payloads.
* Python opens a new SQLite connection per call (no pooling). Schema is 6 tables, idempotent init, one migration column (`canonical_source_item_id`).
* Current Rust code is flat `src/*.rs`; storage module will be a new `src/storage.rs`.

## Requirements

### 2A — SQLite Persistence (Write Path)

* `persist_run()` writes a full run (run row + canonical source items + raw items + normalized events + scored events + intervention candidates) into SQLite in a single transaction.
* Schema: 6 tables matching Python exactly (`runs`, `source_items`, `raw_items`, `normalized_events`, `scored_events`, `intervention_candidates`), same column names/types/constraints.
* `PRAGMA foreign_keys = ON` set on every connection.
* Idempotent `initialize_schema()` — creates tables if not exist, runs `ensure_column` migrations.
* Canonical source item deduplication via `UNIQUE(entry_identity_hash, content_hash)` upsert.
* `tianji run --fixture <path>` auto-persists to SQLite (default path or `--sqlite-path`).

### 2B — History List (Read Path)

* `cargo run -- history --sqlite-path <path>` — lists runs with 18-key list-item vocabulary matching `history_list_item_v1.json` contract fixture.
* Filters: `--limit`, `--mode`, `--dominant-field`, `--risk-level`, `--since`, `--until`, `--min/max-top-impact-score`, `--min/max-top-field-attraction`, `--min/max-top-divergence-score`, `--top-group-dominant-field`, `--min/max-event-group-count`.
* Filter-before-limit semantics (Python parity).
* Top scored event batch-fetched per run for list-item enrichment.

### 2C — History Show (Read Path)

* `cargo run -- history-show --sqlite-path <path> --run-id <id>` — single-run detail with 8-key vocabulary matching `history_detail_v1.json` contract.
* Navigation: `--latest`, `--previous`, `--next` relative to a given run_id.
* Scored event projection: `--dominant-field`, `--min/max-impact-score`, `--min/max-field-attraction`, `--min/max-divergence-score`, `--limit-scored-events`.
* Intervention projection: `--only-matching-interventions` (filter to visible scored event IDs).
* Event group projection: `--group-dominant-field`, `--limit-event-groups`.

### 2D — History Compare (Read Path)

* `cargo run -- history-compare --sqlite-path <path> --left-run-id <id> --right-run-id <id>` — pair comparison with 5-key top-level vocabulary matching `history_compare_v1.json` contract.
* Presets: `--latest-pair`, `--run-id <id> --against-latest`, `--run-id <id> --against-previous`.
* Same projection lenses as history-show, applied symmetrically.
* Diff computation: score deltas, field changes, evidence chain link diff, intervention event ID diffs.
* Mixing comparison modes rejected with error.

## Acceptance Criteria

* [ ] `cargo run -- run --fixture tests/fixtures/sample_feed.xml --sqlite-path <path>` persists all 6 tables with data matching Python oracle.
* [ ] `cargo run -- history --sqlite-path <path>` output matches Python `history` output field-by-field.
* [ ] `cargo run -- history-show --sqlite-path <path> --run-id 1` output matches Python `history-show` output field-by-field.
* [ ] `cargo run -- history-compare --sqlite-path <path> --latest-pair` output matches Python `history-compare` output field-by-field.
* [ ] All filters and projection lenses work identically to Python.
* [ ] `cargo test` passes, `cargo fmt --check` clean, `cargo clippy -- -D warnings` clean.
* [ ] SQLite schema is bit-for-bit compatible (same DDL semantics) with Python.

## Definition of Done

* Tests added/updated (unit + integration with fixture SQLite)
* Lint / typecheck / CI green
* Contract fixture verification (JSON output matches Python vocabulary)
* Python code preserved (not deleted)

## Out of Scope

* HTTP API / daemon (Milestone 3+)
* TUI (Milestone 4)
* Connection pooling or WAL mode optimization (future, post-parity)
* `tianji watch` / `tianji predict` / `tianji backtrack` commands (later milestones)
* Refactoring flat `src/*.rs` into `cangjie/`/`fuxi/` sub-modules (separate task if desired)

## Technical Notes

### Files Inspected

* `tianji/storage_write.py` — 6-table schema, persist_run, ensure_canonical_source_items
* `tianji/storage_views.py` — list_runs, get_run_summary, get_latest_run_id/pair, top-scored-event batch
* `tianji/storage_filters.py` — post-fetch filters for scored events, interventions, event groups, run list
* `tianji/storage_compare.py` — compare_runs, build_compare_side/diff, evidence chain diff
* `tianji/cli.py` — history/history-show/history-show subcommand flags and validation
* `tianji/cli_validation.py` — score range validation, compare mode resolution
* `tests/test_history_list.py` — 18-key contract, filter-before-limit, edge cases
* `tests/test_history_show.py` — 8-key contract, projection lenses, navigation
* `tests/test_history_compare.py` — 5-key contract, diff vocabulary, presets, mode rejection
* `tests/fixtures/contracts/history_list_item_v1.json` — frozen field list (20 fields incl. computed)
* `tests/fixtures/contracts/history_detail_v1.json` — frozen detail structure
* `tests/fixtures/contracts/history_compare_v1.json` — frozen compare structure
* `.trellis/spec/backend/contracts/local-api-contract.md` — API vocabulary mirrors CLI/storage
* `src/lib.rs` — current Rust pipeline (357 lines)
* `src/models.rs` — current Rust data structures (119 lines)

### Key Constraints

* Python storage opens new connection per call — Rust can do the same for parity, or use `rusqlite::Connection` directly.
* No SQLite indexes beyond PKs and the `source_items` UNIQUE — match this for parity.
* Scored events sorted by `divergence_score DESC, id ASC` — must match exactly.
* `format_evidence_chain_link` produces deterministic string with sorted components — Rust must reproduce the same format.
* All CLI history output is JSON with `indent=2`, `ensure_ascii=False` — Rust uses `serde_json::to_string_pretty` with Unicode.
* Current `tianji run` does NOT persist — this milestone adds `--sqlite-path` flag and auto-persist logic.
* Event group computation already exists in `src/grouping.rs` — storage views need to read from persisted scored events and reconstruct groups (or persist groups separately).

## Decisions (ADR-lite)

| # | Decision | Rationale |
|---|---|---|
| D1 | Event groups: recompute on read (no 7th table) | scored_events is source of truth; event_groups is derived. LiveStore event-sourcing principle: never persist computed values. TianJi scored_events are immutable post-write, so recompute is always current. Per-run cost is O(1) on 3-10 scored_events. |
| D2 | `--sqlite-path` explicit-only (no default) | Matches Python CLI parity. No implicit file creation. |
| D3 | Full clap subcommands in this milestone | Deliver a usable CLI at milestone end, not a lib-only slice. |
| D4 | `run --sqlite-path` optional (persist only when specified) | Matches Python behavior: `run` without `--sqlite-path` outputs JSON only. |
| D5 | Flat subcommands: `history`, `history-show`, `history-compare` | Matches Python CLI shape exactly. |
| D6 | Only add `src/storage.rs`, no module refactor | Minimize scope; flat `src/*.rs` works fine for current size. |
