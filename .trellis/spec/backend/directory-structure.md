# Directory Structure

> How backend code is organized in this project.

---

## Overview

TianJi is a **Rust project** with a Python oracle codebase preserved for
compatibility verification. The authoritative project structure is defined
in `plan.md` §10.

---

## Rust Directory Layout (Target)

The target Rust project structure per `plan.md` §10:

```
tianji/
├── Cargo.toml
├── src/
│   ├── main.rs
│   ├── lib.rs
│   ├── models.rs               # Worldline, Event, Profile, ActionProposal...
│   ├── error.rs
│   │
│   ├── cangjie/
│   │   ├── mod.rs
│   │   ├── feed.rs             # RSS/Atom (quick-xml)
│   │   ├── fetch.rs            # HTTP (reqwest)
│   │   ├── normalize.rs        # regex keyword/actor/region extraction
│   │   └── sources.rs          # source registry + fetch policy
│   │
│   ├── fuxi/
│   │   ├── mod.rs
│   │   ├── worldline.rs        # Worldline state machine + Blake3 snapshot
│   │   ├── scoring.rs          # Im/Fa + divergence
│   │   ├── grouping.rs         # event grouping + causal ordering
│   │   ├── backtrack.rs        # intervention candidates
│   │   ├── triggers.rs         # threshold/pattern detection
│   │   └── dependency.rs       # petgraph field DAG
│   │
│   ├── hongmeng/               # Phase 2+
│   ├── nuwa/                   # Phase 3+
│   ├── storage.rs              # rusqlite: worldlines, runs, profiles, checkpoints
│   ├── llm.rs                  # LLM abstraction layer
│   │
│   ├── cli/                    # clap derive
│   ├── tui/                    # ratatui
│   ├── daemon/                 # axum + UNIX socket
│   ├── webui.rs                # axum serve static
│   └── output.rs               # terminal formatting (tabled + JSON)
│
├── profiles/                   # Actor profile YAML
├── rules/                      # Auto trigger rules
├── tianji/webui/               # Static Web UI (preserved)
├── tests/
│   ├── fixtures/sample_feed.xml
│   ├── test_pipeline.rs
│   ├── test_scoring.rs
│   └── ...
├── plan.md
└── README.md
```

### Current State (Milestone 1A+1B Complete)

The Rust crate currently implements Cangjie/Fuxi core parity:

```
src/
├── main.rs          # CLI entry: cargo run -- run --fixture <path>
├── lib.rs           # Pipeline orchestration + integration tests
├── models.rs        # RawItem, NormalizedEvent, ScoredEvent, RunArtifact, etc.
├── fetch.rs         # RSS/Atom parsing + canonical hashing (Cangjie)
├── normalize.rs     # Keyword/actor/region extraction + field scores (Cangjie)
├── scoring.rs       # Im/Fa scoring + rationale (Fuxi)
├── grouping.rs      # Event grouping + causal ordering (Fuxi)
└── backtrack.rs     # Intervention candidate generation (Fuxi)
```

This will expand to the target structure as milestones are implemented.

---

## Rust Module Organization

### Stage-Oriented Modules

Each pipeline stage gets its own module, grouped under subsystem namespaces:

- `cangjie::feed` → `cangjie::normalize` (Milestone 1A, currently flat in `src/`)
- `fuxi::scoring` → `fuxi::grouping` → `fuxi::backtrack` (Milestone 1B, currently flat in `src/`)

### Naming Conventions

| Convention | Pattern | When to Use |
|------------|---------|-------------|
| Stage modules | `{stage}.rs` | One file per pipeline stage (`scoring.rs`, `backtrack.rs`) |
| Subsystem dirs | `{subsystem}/mod.rs` + `*.rs` | When a subsystem has 3+ modules (`cangjie/`, `fuxi/`) |
| CLI commands | `cli/{command}.rs` | One file per CLI command (`cli/run.rs`, `cli/history.rs`) |
| Test modules | `#[cfg(test)] mod tests` inside each module | Unit tests co-located with code |
| Integration tests | Tests in `src/lib.rs` | End-to-end pipeline tests |

### Spec Document Naming

Specification documents under `.trellis/spec/` use **lowercase kebab-case** filenames.

```text
.trellis/spec/backend/scoring-spec.md
.trellis/spec/backend/contracts/local-api-contract.md
```

### Forbidden Patterns

- **No `utils.rs` catch-all** — every file has a specific purpose and name
- **No premature subsystem directories** — create `cangjie/` when it has 3+ files, not before
- **No root-doc uppercase names inside `.trellis/spec/`** — use lowercase kebab-case

---

## Python Oracle Directory Layout (Compatibility Reference)

The Python codebase is preserved as the migration oracle. It is NOT the product
direction — it is the compatibility contract that Rust must match.

```
tianji/
├── tianji/                  # Python oracle source
│   ├── __init__.py
│   ├── __main__.py          # Entry: python3 -m tianji
│   ├── cli.py               # Click CLI entry
│   ├── cli_*.py             # CLI subcommand handlers
│   ├── models.py            # Dataclasses: RawItem, NormalizedEvent, ScoredEvent...
│   ├── fetch.py             # Feed parsing + canonical hashing
│   ├── normalize.py         # Event extraction + field scoring
│   ├── scoring.py           # Im/Fa scoring + rationale
│   ├── backtrack.py         # Intervention candidates
│   ├── pipeline.py          # Orchestration + grouping
│   ├── storage*.py          # SQLite persistence hub + sub-modules
│   ├── daemon.py            # UNIX-socket daemon
│   ├── api.py               # Loopback HTTP API
│   ├── tui*.py              # Rich TUI (oracle — target is ratatui per plan.md §9)
│   └── webui*/              # Optional web UI
├── tests/                   # Python oracle tests
│   ├── support.py           # Shared imports hub
│   ├── test_*.py            # Feature tests
│   └── fixtures/            # Test data + contract fixtures
├── pyproject.toml
└── README.md
```

---

## Examples of Well-Organized Rust Modules

- **Models**: `src/models.rs` — flat struct definitions for all pipeline data types
- **Scoring**: `src/scoring.rs` — `compute_im`, `compute_fa`, `compute_divergence_score`, `build_rationale`, `score_events`
- **Grouping**: `src/grouping.rs` — `group_events`, `summarize_group`, `build_evidence_chain`
- **Backtracking**: `src/backtrack.rs` — `backtrack_candidates`, `infer_intervention_type`, `build_reason`

---

**Language**: English
