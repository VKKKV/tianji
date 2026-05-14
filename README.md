# TianJi (天机)

TianJi is a geopolitical intelligence engine — fetch signals, infer the current branch, backtrack likely intervention points. The authoritative architecture document is `plan.md`.

## Current State

TianJi is actively migrating from Python to Rust on the `rust-cli` branch.

**Rust (product direction):**
- Milestone 1A+1B complete: full Cangjie/Fuxi pipeline parity with Python oracle
- `cargo run -- run --fixture <path>` produces field-for-field compatible `RunArtifact` JSON
- 18 tests passing, `cargo fmt` clean, `cargo clippy` clean

**Python (migration oracle):**
- Full pipeline under `tianji/`: fetch → normalize → score → backtrack → emit
- SQLite persistence, history, TUI, daemon, web UI
- Preserved as compatibility reference — not the product direction
- 233 tests passing

## Quick Start

### Rust (current)

```bash
cargo build
cargo test
cargo run -- run --fixture tests/fixtures/sample_feed.xml
```

### Python oracle (compatibility reference)

```bash
uv venv .venv && uv pip install -e .
.venv/bin/python -m tianji run --fixture tests/fixtures/sample_feed.xml
.venv/bin/python -m unittest discover -s tests -v
```

## Local Daemon and API

The daemon keeps the CLI as the source-of-truth write path. It is read-first and loopback-only: UNIX socket commands control local background runs, while the HTTP API exposes persisted run metadata on `127.0.0.1`.

```bash
.venv/bin/python -m tianji daemon status --socket-path runs/tianji.sock
.venv/bin/python -m tianji daemon run --socket-path runs/tianji.sock --fixture tests/fixtures/sample_feed.xml
.venv/bin/python -m tianji daemon schedule --socket-path runs/tianji.sock --every-seconds 300 --count 3 --fixture tests/fixtures/sample_feed.xml

curl http://127.0.0.1:8765/api/v1/meta
curl http://127.0.0.1:8765/api/v1/runs
curl "http://127.0.0.1:8765/api/v1/compare?left_run_id=1&right_run_id=2"

.venv/bin/python -m tianji.webui_server --api-base-url http://127.0.0.1:8765 --host 127.0.0.1 --port 8766
```

## What the Pipeline Does

1. **Fetch / Load** — Load RSS or Atom input from local fixture files
2. **Normalize** — Extract keywords, actors, regions, field scores from raw items
3. **Score** — Compute Im (impact) and Fa (field attraction), produce divergence score
4. **Group** — Cluster related events by shared signals and time window
5. **Backtrack** — Generate intervention candidates from top-scored events
6. **Emit** — Write schema-versioned JSON artifact

## Output Artifact

```bash
cargo run -- run --fixture tests/fixtures/sample_feed.xml
```

Emits JSON with:

- `schema_version`: stable artifact contract version
- `input_summary`: item counts and source list
- `scenario_summary`: dominant field, top actors, top regions, risk level, headline
- `scored_events`: events with `impact_score`, `field_attraction`, `divergence_score`, `dominant_field`, `rationale`
- `intervention_candidates`: ranked backtracked actions

## Migration Roadmap

Per `plan.md`:

| Phase | Scope | Status |
|-------|-------|--------|
| 1 | Worldline core + pipeline (Cangjie/Fuxi) | 1A+1B complete |
| 2 | Hongmeng orchestration layer | Deferred |
| 3 | Nuwa simulation sandbox | Deferred |
| 4 | TUI (ratatui + Kanagawa Dark) | Deferred |
| 5 | Daemon + Web UI | Deferred |
| 6 | Cleanup + Python retirement | Deferred |

## Repository Layout

```
tianji/
├── Cargo.toml              # Rust crate
├── src/
│   ├── main.rs             # CLI entry
│   ├── lib.rs              # Pipeline orchestration + tests
│   ├── models.rs           # All data structures
│   ├── fetch.rs            # RSS/Atom parsing + canonical hashes
│   ├── normalize.rs        # Keyword/actor/region extraction
│   ├── scoring.rs          # Im/Fa scoring + rationale
│   ├── grouping.rs         # Event grouping + causal ordering
│   └── backtrack.rs        # Intervention candidate generation
├── tianji/                 # Python oracle (migration reference)
├── tests/                  # Python + Rust tests + fixtures
├── plan.md                 # Authoritative architecture document
└── README.md
```

## Four Subsystems

Per `plan.md`:

| Subsystem | Purpose | Phase |
|-----------|---------|-------|
| Cangjie (仓颉) | Signal ingestion, normalization, content-hash dedup | 1 (current) |
| Fuxi (伏羲) | Divergence modeling, scoring, worldline state machine | 1 (current) |
| Hongmeng (鸿蒙) | Agent orchestration, Board/Stick, checkpoint | 2 |
| Nuwa (女娲) | Simulation sandbox, forward/backward reasoning | 3 |

## Principles

1. **Local First** — usable without external cloud dependencies
2. **Deterministic First** — inference inspectable before adding model-driven layers
3. **CLI First** — prove the operator workflow before introducing services or UI
4. **No Bloatware** — keep the core lean and understandable
