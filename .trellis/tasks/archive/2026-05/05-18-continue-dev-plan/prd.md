# Phase D1: Storage History Integration Coverage

## Goal

Add focused integration coverage for the Phase D1 dev-plan item: verify the persisted storage read path works end-to-end from `persist_run` through `get_run_summary` into `compare_runs`.

## What I Already Know

- Root `plan.md` is the authoritative development plan.
- `plan.md` marks Phase A and Phase B complete.
- Recent archived tasks and commits indicate Phase C1-C4 are complete.
- The next unclaimed `plan.md` item is Phase D1: integration test coverage for `persist_run -> get_run_summary -> compare_runs` using in-memory or temporary SQLite, parallel to existing worldline persistence tests.
- Current tests live in `src/lib.rs`; there is no `tests/**/*.rs` harness.
- Existing tests cover `persist_run`, `get_run_summary`, and `compare_runs` individually, but not one explicit end-to-end assertion that compare output is derived from persisted detail payloads across two runs.

## Requirements

- Add one focused test covering the complete storage-history flow:
  - persist two fixture runs into a temporary SQLite database,
  - read both runs with `get_run_summary`,
  - compare them with `compare_runs`,
  - assert compare side summaries and diff fields match the persisted run-detail payloads.
- Keep behavior unchanged; this task is test coverage only unless a real bug is discovered.
- Follow existing test helper patterns in `src/lib.rs` for temporary database paths and cleanup.

## Acceptance Criteria

- [ ] A test explicitly covers `persist_run -> get_run_summary -> compare_runs` as one flow.
- [ ] Test assertions prove compare sides are built from persisted detail fields, including run counts, scenario summary fields, top scored event identity, and diff fields.
- [ ] No production behavior changes unless needed to fix a failing discovered contract.
- [ ] `cargo fmt --check`, `cargo test`, and `cargo clippy -- -D warnings` pass.

## Definition of Done

- Tests added/updated where appropriate.
- Lint / typecheck / test suite green.
- Specs reviewed for whether any new project knowledge should be recorded.

## Out of Scope

- Adding new storage features.
- Refactoring the storage layer.
- Moving tests to a separate `tests/` integration harness unless necessary.
- External alert delivery or later Phase D feature work.

## Technical Approach

Add a minimal in-crate test near the existing Storage + History integration tests in `src/lib.rs`, reusing `run_fixture_path`, `get_run_summary`, `compare_runs`, `ScoredEventFilters::default`, and `EventGroupFilters::default`.

## Decision (ADR-lite)

Context: The development plan identifies Phase D1 as missing integration coverage, while current code already has broad unit-level and subsystem-level storage tests.

Decision: Use the narrow D1 scope: one focused end-to-end in-crate test that proves persisted run summaries feed compare output correctly.

Consequences: This keeps risk low and avoids introducing a separate integration-test harness unless future Phase D work needs it.

## Technical Notes

- `plan.md` Phase D1: "Integration test coverage — persist_run -> get_run_summary -> compare_runs full flow; in-memory SQLite, parallel to existing worldline persistence tests".
- Existing relevant code: `src/storage.rs`, `src/lib.rs` storage test module.
- Existing temp SQLite helper uses `/tmp/tianji_test_<id>.sqlite3` and explicit cleanup.
