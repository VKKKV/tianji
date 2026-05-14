# Crucix Delta Engine Phase 2 — Daemon Auto-Delta + AlertTier

## Goal

Wire the existing but unused `classify_delta_tier()` into the daemon worker loop so that every daemon-triggered run automatically produces a delta classification, and expose the tier + delta summary in daemon job status and API responses.

## Decision (ADR-lite)

**Context**: `run_fixture_path()` internally computes delta via `update_delta_memory_for_latest_run()` but discards the result. The daemon never sees `DeltaReport` or `AlertTier`.

**Decision**: Approach A — change `run_fixture_path()` to return `RunResult { artifact, delta, alert_tier }`. Delta is already computed; returning it has zero extra cost. All 3 callers (CLI run, daemon worker, tests) updated to unpack `RunResult`.

**Consequences**: Public API signature changes. `set_job_succeeded()` signature expands to accept delta+tier. But the delta was already being computed and thrown away — this just surfaces it.

## Requirements

1. `run_fixture_path()` returns `RunResult { artifact: RunArtifact, delta: Option<DeltaReport>, alert_tier: Option<AlertTier> }` instead of `RunArtifact`
2. `update_delta_memory_for_latest_run()` returns `Option<DeltaReport>` instead of `()`
3. `classify_delta_tier()` called on the returned delta
4. Daemon worker loop passes delta+tier to `set_job_succeeded()`
5. `JobRecord` gains `delta_tier: Option<AlertTier>` and `delta_summary: Option<DeltaSummary>`
6. `job_status` response includes `delta_tier` and `delta_summary` fields
7. Signal marking: after tier classification, `HotMemory::mark_alerted()` for each delta signal
8. Alert suppression: before emitting notification, check `HotMemory::is_signal_suppressed()`
9. API endpoint `GET /api/v1/delta/latest` returns latest `DeltaReport` + `AlertTier` from hot memory
10. Fix NB1: `DeltaConfig.numeric_thresholds` type `BTreeMap<String, u64>` → `BTreeMap<String, f64>` (numeric thresholds are percentages like 20.0)
11. Fix NB4: `collect_string_array` duplicated in `delta.rs:479` and `delta_memory.rs:356` — move to `src/utils.rs`
12. CLI `tianji delta` output now includes `alert_tier` field in JSON output

## Caller Updates

| Location | Current | Changed |
|----------|---------|---------|
| `main.rs` CLI run | `let artifact = run_fixture_path(...)?; artifact_json(&artifact)` | `let result = run_fixture_path(...)?; artifact_json(&result.artifact)` |
| `daemon.rs` worker | `run_fixture_path(...)?; set_job_succeeded(job_id, run_id)` | `let result = run_fixture_path(...)?; set_job_succeeded(job_id, run_id, result.delta, result.alert_tier)` |
| `lib.rs` tests | `let artifact = run_fixture_path(SAMPLE_FIXTURE, None).expect(...)` | `let result = run_fixture_path(...).expect(...); let artifact = result.artifact;` |

## Not a Bug: NB2 fixture epoch timestamps

`development-plan.md` explicitly mandates: "Pruning in the persistence path must use the current run's persisted `generated_at`, not wall-clock time, so fixture runs stay deterministic." Fixture runs have `generated_at` = epoch, which means `prune_stale_signals_at_timestamp()` is a no-op for fixtures (all signals at "now=epoch", nothing expires). This is correct — fixtures are deterministic replays, not real-time monitoring. Daemon runs with real data use real timestamps, pruning works normally. Tests use temp DBs that reset hot memory each time, so no accumulation occurs.

## Acceptance Criteria

- [ ] `run_fixture_path()` returns `Result<RunResult, TianJiError>`
- [ ] `update_delta_memory_for_latest_run()` returns `Option<DeltaReport>`
- [ ] `classify_delta_tier()` called on every delta produced by daemon runs
- [ ] `JobRecord` has `delta_tier: Option<AlertTier>` and `delta_summary: Option<DeltaSummary>`
- [ ] `job_status` response includes delta fields
- [ ] `GET /api/v1/delta/latest` endpoint works
- [ ] Signal marking (`mark_alerted`) happens after tier classification
- [ ] `DeltaConfig.numeric_thresholds` is `BTreeMap<String, f64>`
- [ ] `collect_string_array` deduplicated to `src/utils.rs`
- [ ] CLI `tianji delta` output includes `alert_tier`
- [ ] All 80+ existing tests pass + new tests
- [ ] `cargo fmt --check` + `cargo clippy -- -D warnings` clean

## Out of Scope

- Telegram/Discord webhook notifications (Crucix Phase 3)
- Cold archive rotation
- Schema-backed delta tables in SQLite
- M3C schedule
- Changing prune to use wall-clock time (NB2 is not a bug)

## Technical Notes

- `src/delta.rs`: `compute_delta()`, `compute_delta_with_metrics()`, `collect_string_array` (duplicate)
- `src/delta_memory.rs`: `classify_delta_tier()`, `AlertTier`, `HotMemory`, `AlertDecayModel`, `DeltaConfig`, `collect_string_array` (duplicate)
- `src/daemon.rs`: `worker_loop()`, `run_pipeline_for_job()`, `JobRecord`, `DaemonState`
- `src/lib.rs`: `run_fixture_path()`, `update_delta_memory_for_latest_run()`
- `src/api.rs`: axum routes — need new `/api/v1/delta/latest`
- `src/main.rs`: CLI `tianji delta` handler
- `plan-crucix.md` §10 Phase 2: items 6-8
- `development-plan.md` Milestone 3.5 §7 "Wrong vs Correct" — confirms prune must use `generated_at`
