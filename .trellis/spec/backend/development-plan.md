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

### Milestone 3 — Local Runtime Parity

**Complete.** Port the thin local runtime with behavior parity.

- Daemon core: in-memory job queue (4 states: queued/running/succeeded/failed), worker loop, subprocess model ✅
- UNIX socket control plane: JSON-lines protocol, `queue_run` / `job_status` actions ✅
- HTTP read API: 5 axum routes (meta, runs, runs/:id, runs/latest, compare), envelope matching frozen fixtures ✅
- Web UI: compile-time embedded static files, reverse proxy, /queue-run with 2s retry ✅
- CLI: `daemon start/stop/status/run/serve`, `webui`, PID file management ✅
- Loopback enforcement, schedule deferred (D5) ✅
- 52 tests pass, `cargo fmt --check` clean, `cargo clippy -- -D warnings` clean ✅

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
