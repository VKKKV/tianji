# TianJi Development Plan

## Authority

Root `plan.md` is the authoritative architecture document for the TianJi Rust rewrite.
It defines the four subsystems (Cangjie, Fuxi, Hongmeng, Nuwa), the project structure,
the dependency list, the TUI design spec (§9), and the phased build order.

**Python code under `tianji/` and `tests/` is preserved as the migration oracle.**
It is not the product direction — it is the compatibility reference that Rust
implementations must match gate-by-gate before replacing any Python surface.

Do not delete Python code or mark any Rust layer as shipped until the relevant
parity gate has passed. After parity is verified, Python code is retired per
`plan.md` §13 (Deletion List).

## Migration Alignment

`plan.md` defines this build order:

| Phase | Scope | Status |
|-------|-------|--------|
| 1 | Worldline core + pipeline (Cangjie/Fuxi) | Milestone 1A+1B complete |
| 2 | Storage + History | Milestone 2 complete |
| 3 | Local Runtime (daemon + API + webui) | Milestone 3 complete |
| - | Hongmeng orchestration layer | Deferred |
| 3 | Nuwa simulation sandbox | Deferred |
| 4 | TUI (ratatui + Kanagawa Dark) | Deferred |
| 5 | Daemon + Web UI | Absorbed by Milestone 3 |
| 6 | Cleanup + docs (Python retirement) | Deferred |

Each phase must reach parity with the current Python behavior before moving to the next.
Python remains the executable oracle until the relevant Rust gate is reviewed and accepted.

### Milestone 1A — Feed + Normalization Parity

**Complete.** Replaces the Milestone 0 scaffold with real deterministic
feed parsing, canonical hashing, and normalized event emission.

- RSS 2.0 and Atom 1.0 local fixture parsing ✅
- SHA-256 entry identity and content hashes compatible with Python ✅
- Deterministic normalization: keywords, actors, regions, field scores, event IDs ✅
- Normalized-event-shaped payloads emitted through the Rust artifact ✅
- Python code and tests intact ✅

### Milestone 1B — Scoring + Grouping + Backtracking Parity

**Complete.** Rust one-shot output is semantically compatible with the Python
fixture pipeline.

- `Im` / `Fa` scoring semantics and rationale vocabulary ✅
- Event grouping, causal/evidence summaries ✅
- Backtrack intervention candidates ✅
- Full `RunArtifact` field-for-field parity with Python oracle ✅
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

**Core complete.** Adds a Crucix-inspired cross-run analysis layer on top of the
persisted SQLite read model without changing the six-table schema.

#### 1. Scope / Trigger

- Trigger: `tianji run --sqlite-path <path>` now updates a hot-memory JSON file
  after successful persistence, and `tianji delta` exposes manual run-pair diffing.
- Scope: compute structured deltas between two persisted runs, keep recent compact
  run snapshots, and classify alert tiers.
- Out of scope: daemon push notifications, cold archive rotation, and schema-backed
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

impl HotMemory {
    pub fn load(path: &Path) -> Self;
    pub fn save_atomic(&self, path: &Path) -> Result<(), TianJiError>;
    pub fn push_run(&mut self, compact: CompactRunData, delta: Option<DeltaReport>, max_runs: usize);
    pub fn prune_stale_signals_at_timestamp(&mut self, decay: &AlertDecayModel, now_rfc3339: &str);
}
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
- `save_atomic` writes a temporary file, preserves the previous hot file as `.bak`, then renames
  the temporary file into place.

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

#### 5. Good/Base/Bad Cases

- Good: two persisted fixture runs produce a hot memory file with two entries, newest first,
  and the newest entry has `delta: Some(...)`.
- Base: one persisted fixture run produces a hot memory file with one entry and `delta: None`.
- Bad: using `Utc::now()` or `SystemTime` in the run-persistence pruning path makes fixture
  tests and replayed runs nondeterministic.

#### 6. Tests Required

- Unit-test numeric, count, new-signal, and risk-direction delta behavior.
- Unit-test alert suppression and stale-signal pruning with injected timestamps.
- Unit-test hot-memory primary/backup load fallback and atomic save behavior.
- Integration-test two persisted runs update hot memory in newest-first order.
- Regression-test reused temp DB names reset stale hot memory on first run.
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

- Delete all Python code per `plan.md` §13
- Delete `.venv/`, `.pytest_cache/`, `__pycache__/`
- Update README
- Shell completions (clap generate)

## Dependency Guidance

The dependency list in `plan.md` §11 is the target. Each milestone should add only
the dependencies it needs. In particular, do not add async runtimes, web frameworks,
TUI crates, graph engines, or LLM provider crates before the milestone that uses them.

## Documentation Rules During Migration

- `plan.md` is the authority for architecture, project structure, and build phases.
- Root docs must distinguish shipped Python reality from Rust migration target.
- Trellis specs should be updated before claiming a Rust layer is current.
- Compatibility changes should name the Python command, artifact field, or test
  behavior they preserve.
- Python code under `tianji/` and `tests/` is the oracle, not the direction.

## Shipped Python Surface (Migration Oracle Reference)

This section records the current Python product surface for parity verification.
It is not the development direction — it is the compatibility contract Rust must match.

### One-Shot Pipeline

- `python3 -m tianji run --fixture <path>` or `--fetch --source-url <url>`
- Stages: fetch → normalize → score → backtrack → emit
- Output: `RunArtifact` JSON with `schema_version`, `mode`, `generated_at`,
  `input_summary`, `scenario_summary`, `scored_events`, `intervention_candidates`

### Scoring Model (Im / Fa)

- `Im` inputs: actor weights, region weights, keyword density, dominant-field bonus,
  field-diversity bonus, text-signal intensity
- `Fa` inputs: dominant-field strength, dominance margin, coherence share,
  near-tie penalty, diffuse-third-field penalty
- `divergence_score = f(Im, Fa)`
- Spec: `.trellis/spec/backend/scoring-spec.md`

### Persistence + History

- SQLite-backed run persistence
- `history`: list/filter runs by mode, field, risk, score, grouped-analysis signals
- `history-show`: single-run detail with scored-event and event-group projection lenses
- `history-compare`: pair compare with same projection lenses, presets (latest, previous)

### TUI (Rich, Read-Only)

- `python3 -m tianji tui --sqlite-path <path>`
- Read-only Rich-based browser over persisted runs
- Split-pane list/detail layout, compare staging, Vim-style movement
- Contract: `.trellis/spec/backend/contracts/tui-contract.md`

### Daemon + Local API + Web UI

- `tianji daemon start/stop/status/run/schedule`
- Loopback HTTP API at `127.0.0.1:8765`, read-first
- Optional web UI at `127.0.0.1:8766`
- Contracts: `contracts/daemon-contract.md`, `contracts/local-api-contract.md`,
  `contracts/web-ui-contract.md`

## Guardrails

- Keep first-party Rust source under `src/` (per `plan.md` §10 project structure).
- Keep Python source under `tianji/` and `tests/` until retirement milestone.
- Prefer reimplementation over cross-importing from Python.
- Avoid framework-first expansion — add dependencies per milestone.
- Every new layer should preserve local-first, deterministic-first behavior.
- Do not claim a Rust layer is shipped until parity with the Python oracle is verified.
