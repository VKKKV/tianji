# TianJi Development Plan

## Authority

Root `plan.md` is the authoritative architecture document for the TianJi Rust rewrite.
It defines the four subsystems (Cangjie, Fuxi, Hongmeng, Nuwa), the project structure,
the dependency list, the TUI design spec (§9), and the phased build order.

**Python oracle retired in Phase 6 (v0.2.0).** All Rust parity gates have passed.
The project is now a pure Rust binary.

## Migration Alignment

`plan.md` defines this build order:

| Phase | Scope | Status |
|-------|-------|--------|
| 1 | Worldline core + pipeline (Cangjie/Fuxi) | Milestone 1A+1B complete |
| 2 | Storage + History | Milestone 2 complete |
| 3 | Local Runtime (daemon + API + webui) | Milestone 3 complete |
| - | Hongmeng orchestration layer | Deferred |
| 3 | Nuwa simulation sandbox | Deferred |
| 4 | TUI (ratatui + Kanagawa Dark) | Milestone 4 complete |
| 5 | Daemon + Web UI | Absorbed by Milestone 3 |
| 6 | Cleanup + docs (Python retirement) | Phase 6 complete |

### Milestone 1A — Feed + Normalization Parity

**Complete.** Replaces the Milestone 0 scaffold with real deterministic
feed parsing, canonical hashing, and normalized event emission.

- RSS 2.0 and Atom 1.0 local fixture parsing ✅
- SHA-256 entry identity and content hashes ✅
- Deterministic normalization: keywords, actors, regions, field scores, event IDs ✅
- Normalized-event-shaped payloads emitted through the Rust artifact ✅

### Milestone 1B — Scoring + Grouping + Backtracking Parity

**Complete.** Rust one-shot output is deterministic and verified.

- `Im` / `Fa` scoring semantics and rationale vocabulary ✅
- Event grouping, causal/evidence summaries ✅
- Backtrack intervention candidates ✅
- Full `RunArtifact` field-for-field parity verified ✅
- 18 Rust tests pass, `cargo fmt --check` clean, `cargo clippy` clean ✅

### Milestone 2 — Storage + History Parity

**Complete.** Port the durable local read model with field-for-field parity.

- SQLite persistence: 6 tables (`runs`, `source_items`, `raw_items`, `normalized_events`, `scored_events`, `intervention_candidates`) ✅
- `PRAGMA foreign_keys = ON`, atomic transactions, canonical source item deduplication ✅
- Event groups recomputed on read (LiveStore principle: never persist derived values) ✅
- `history`: list/filter runs with 18-key list-item vocabulary, filter-before-limit ✅
- `history-show`: single-run detail with 8-key vocabulary, scored-event/intervention/event-group projection lenses ✅
- `history-compare`: pair comparison with 5-key vocabulary, diff computation, presets (--latest-pair, --against-latest, --against-previous) ✅
- CLI: clap subcommands (`run`, `history`, `history-show`, `history-compare`), `--sqlite-path` optional for `run` ✅
- 33 tests pass, `cargo fmt --check` clean, `cargo clippy -- -D warnings` clean ✅

#### Storage Read-Model Paging Contract

`list_runs(sqlite_path, limit, filters)` preserves the shipped history contract:
filters are applied before final limit truncation. To avoid loading an unbounded
SQL result set into memory, filtered reads page through `runs` in deterministic
`ORDER BY id DESC` order using bounded `LIMIT/OFFSET` queries, apply Rust-side
filters per page, and stop only after collecting `limit` matching rows or
exhausting the table. Do not add a hard max scanned-row cap, because that can
silently violate filter-before-limit semantics when a matching run exists beyond
the cap.

```rust
// Correct: bounded SQL pages, unbounded logical scan until enough matches or EOF.
while items.len() < limit {
    let rows = query_run_list_rows(connection, PAGE_SIZE, offset)?;
    if rows.is_empty() { break; }
    items.extend(filter_run_list_items(build_run_list_items(connection, &rows)?, filters));
    offset += rows.len();
}
items.truncate(limit);
```

Tests must cover a filtered match that appears beyond the first SQL page.

### Milestone 3 — Local Runtime Parity

**Complete.** Port the thin local runtime with behavior parity.

- Daemon core: in-memory job queue (4 states: queued/running/succeeded/failed), worker loop, subprocess model ✅
- UNIX socket control plane: JSON-lines protocol, `queue_run` / `job_status` actions ✅
- HTTP read API: 5 axum routes (meta, runs, runs/:id, runs/latest, compare), envelope matching frozen fixtures ✅
- Web UI: compile-time embedded static files, reverse proxy, /queue-run with 2s retry ✅
- CLI: `daemon start/stop/status/run/serve`, `webui`, PID file management ✅
- Loopback enforcement, schedule deferred (D5) ✅
- 52 tests pass, `cargo fmt --check` clean, `cargo clippy -- -D warnings` clean ✅

### Milestone 3.5 — Cross-Run Delta Engine

**Daemon auto-delta complete.** Adds a Crucix-inspired cross-run analysis layer on top
of the persisted SQLite read model without changing the six-table schema. Persisted
runs update hot memory, classify alert tiers, and expose delta summary through daemon
job status and the read API.

#### 1. Scope / Trigger

- Trigger: `tianji run --sqlite-path <path>` now updates a hot-memory JSON file
  after successful persistence, returns a `RunResult` containing the computed delta
  and alert tier, and `tianji delta` exposes manual run-pair diffing with tier output.
- Scope: compute structured deltas between two persisted runs, keep recent compact
  run snapshots, classify alert tiers, expose latest delta via API, and attach delta
  fields to daemon job status.
- Out of scope: external push notifications, cold archive rotation, and schema-backed
  delta tables.

#### 2. Signatures

Rust CLI commands:

```bash
tianji delta --sqlite-path <path> --latest-pair
tianji delta --sqlite-path <path> --left-run-id <id> --right-run-id <id>
```

Rust module boundaries:

```rust
pub fn compute_delta(
    current: &serde_json::Value,
    previous: Option<&serde_json::Value>,
) -> Option<DeltaReport>;

pub fn compact_run_data(run: &serde_json::Value) -> CompactRunData;

pub struct RunResult {
    pub artifact: RunArtifact,
    pub delta: Option<DeltaReport>,
    pub alert_tier: Option<AlertTier>,
}

pub fn run_fixture_path(
    path: impl AsRef<Path>,
    sqlite_path: Option<&str>,
) -> Result<RunResult, TianJiError>;

impl HotMemory {
    pub fn load(path: &Path) -> Self;
    pub fn save_atomic(&self, path: &Path) -> Result<(), TianJiError>;
    pub fn push_run(&mut self, compact: CompactRunData, delta: Option<DeltaReport>, max_runs: usize);
    pub fn prune_stale_signals_at_timestamp(&mut self, decay: &AlertDecayModel, now_rfc3339: &str);
}
```

Read API endpoint:

```text
GET /api/v1/delta/latest?sqlite_path=<path>
```

Hot-memory path:

```text
<sqlite-parent>/<sqlite-file-stem>.memory/hot.json
<sqlite-parent>/<sqlite-file-stem>.memory/hot.json.bak
```

#### 3. Contracts

- `compute_delta` returns `None` when no previous run is available.
- Numeric and count delta outputs are deterministic and sorted by stable metric definitions.
- New-signal identity uses persisted scored-event fields from `get_run_summary`, including
  `event_id`, `actors`, `regions`, and `keywords`.
- The first persisted run creates fresh hot memory with one entry and no delta.
- Later persisted runs load existing hot memory, push the newest compact run at index `0`,
  attach the computed delta, and retain at most the requested hot-run count.
- If a database has no previous persisted run, existing hot memory for the same path is reset
  to avoid stale cross-test or reused-path contamination.
- Pruning in the persistence path must use the current run's persisted `generated_at`, not
  wall-clock time, so fixture runs stay deterministic.
- Alert pruning must retain entries with malformed `last_alerted` timestamps rather than
  deleting them silently. This matches suppression behavior (`malformed timestamp` means
  "not suppressed") and avoids losing alert history during cleanup.
- `save_atomic` writes a temporary file, calls `sync_all()` on that temporary file, copies the
  previous hot file to `.bak` when present, calls `sync_all()` on the backup, renames the
  temporary file into place, and syncs the parent directory. A backup-write failure must leave
  the primary hot file untouched.
- API run-summary routes (`/api/v1/runs/:id`, `/api/v1/runs/latest`, and compare endpoints)
  must pass explicit bounded scored-event and event-group limits. CLI/history-show defaults
  remain unbounded unless the caller provides an explicit limit, and explicit limits are clamped
  to the storage maximum.
- `run_fixture_path` returns the computed `DeltaReport` and `AlertTier` in `RunResult`.
  CLI artifact output must serialize `RunResult.artifact`, preserving the shipped run JSON.
- Daemon successful job status includes `delta_tier` and `delta_summary` fields.
- Daemon persisted runs use `run_fixture_path_with_alert_marking(..., true)` so the same
  hot-memory load/update/save cycle both pushes the latest compact run and marks emitted
  delta signal keys as alerted. Do not reload and resave hot memory in the daemon worker
  solely to call `mark_alerted`.
- Alert marking in the daemon persistence path must use the persisted run `generated_at`
  timestamp, not wall-clock time, matching stale-signal pruning determinism.
- `GET /api/v1/delta/latest` loads hot memory for the SQLite path, returns the newest run's
  delta plus `classify_delta_tier(delta)`, and returns `null` fields when no delta exists.
- `DeltaConfig.numeric_thresholds` uses `f64` values because numeric thresholds are percentages.

#### 4. Validation & Error Matrix

| Condition | Behavior |
|-----------|----------|
| `delta --latest-pair` with fewer than two runs | Return `TianJiError::Usage` |
| `delta` mixes `--latest-pair` with explicit IDs | Return `TianJiError::Usage` |
| `delta` omits either explicit ID without `--latest-pair` | Return `TianJiError::Usage` |
| Explicit run ID not found | Return `TianJiError::Usage("Run not found: <id>")` |
| Hot-memory primary JSON is corrupt but `.bak` is valid | Load `.bak` |
| Hot-memory primary and backup are missing/corrupt | Return `HotMemory::default()` |
| Hot-memory save fails | Propagate `TianJiError` |
| Hot-memory backup write fails during save | Propagate `TianJiError`; keep primary intact |
| Alert entry has malformed `last_alerted` during pruning | Retain the entry |
| API run summary omits item limits | Apply API defaults |
| CLI/history run summary omits item limits | Return unbounded detail |
| Explicit run-summary limit exceeds storage max | Clamp to max |
| Latest delta requested with fewer than two persisted runs | Return successful envelope with `delta: null`, `alert_tier: null` |

#### 5. Good/Base/Bad Cases

- Good: two persisted fixture runs produce a hot memory file with two entries, newest first,
  the newest entry has `delta: Some(...)`, and `RunResult.alert_tier` matches `classify_delta_tier`.
- Base: one persisted fixture run produces a hot memory file with one entry, `delta: None`,
  and daemon/API delta fields are null.
- Bad: using `Utc::now()` or `SystemTime` in the run-persistence pruning path makes fixture
  tests and replayed runs nondeterministic.
- Bad: applying API default run-summary limits inside the storage layer would truncate CLI
  `history-show` output and break the explicit-detail contract.

#### 6. Tests Required

- Unit-test numeric, count, new-signal, and risk-direction delta behavior.
- Unit-test alert suppression and stale-signal pruning with injected timestamps.
- Unit-test hot-memory primary/backup load fallback and atomic save behavior.
- Regression-test backup-write failure leaves the primary hot-memory file readable.
- Unit-test malformed alert timestamps survive stale-signal pruning.
- Integration-test two persisted runs update hot memory in newest-first order.
- Unit-test run-summary limit behavior: omitted CLI limits stay unbounded, explicit limits clamp,
  and filtered scored-event queries do not pre-limit before applying predicates.
- Regression-test reused temp DB names reset stale hot memory on first run.
- Integration-test `RunResult` includes delta and alert tier for persisted run pairs.
- Test daemon job status includes `delta_tier` and `delta_summary` after a successful run.
- Regression-test daemon-style persisted runs can mark delta signal keys as alerted during
  the hot-memory update path without a second load/save cycle.
- Test `GET /api/v1/delta/latest` returns latest delta/tier and handles no-delta cases.
- Run `cargo fmt --check`, `cargo test`, and `cargo clippy -- -D warnings` after changes.

#### 7. Wrong vs Correct

#### Wrong

```rust
// Nondeterministic in fixture/replay paths.
memory.prune_stale_signals(&AlertDecayModel::default());
```

#### Correct

```rust
// Use persisted run time as the deterministic pruning reference.
memory.prune_stale_signals_at_timestamp(&AlertDecayModel::default(), generated_at);
```

#### Wrong

```rust
// Daemon worker path: this reloads and saves the hot-memory file a second time
// after `run_fixture_path` already updated it.
let result = run_fixture_path(fixture_path, Some(sqlite_path))?;
mark_delta_signals_alerted(sqlite_path, &result)?;
```

#### Correct

```rust
// Daemon worker path: update compact run data, delta, alert tier, pruning, and
// alerted-signal markers in one hot-memory update/save cycle.
let result = run_fixture_path_with_alert_marking(fixture_path, Some(sqlite_path), true)?;
```

### Milestone 4 — TUI (ratatui + Kanagawa Dark)

After deterministic core, storage, and runtime contracts are stable in Rust.

- Ratatui TUI per `plan.md` §9
- Kanagawa Dark hardcoded color palette
- Vim-style keybindings (full spec in `plan.md` §9)
- Dashboard, history, simulation, and profile views

### Milestone 5 — Daemon + Web UI

After TUI is stable.

- Axum HTTP API + UNIX socket
- Background job queue + auto recovery
- LLM provider configuration loading
- Static Web UI serve

### Milestone 6 — Cleanup

**Complete.** Python oracle retired, shell completions added, documentation updated.

- Delete all Python code per `plan.md` §13 ✅
- Delete `.venv/`, `.pytest_cache/`, `__pycache__/` ✅
- Update README ✅
- Shell completions (clap_complete) ✅

## Dependency Guidance

The dependency list in `plan.md` §11 is the target. Each milestone should add only
the dependencies it needs. In particular, do not add async runtimes, web frameworks,
TUI crates, graph engines, or LLM provider crates before the milestone that uses them.

## Documentation Rules

- `plan.md` is the authority for architecture, project structure, and build phases.
- Trellis specs should be updated before claiming a Rust layer is current.

## Guardrails

- Keep first-party Rust source under `src/` (per `plan.md` §10 project structure).
- Avoid framework-first expansion — add dependencies per milestone.
- Every new layer should preserve local-first, deterministic-first behavior.
