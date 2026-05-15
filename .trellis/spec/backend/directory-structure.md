# Directory Structure

> How backend code is organized in this project.

---

## Overview

TianJi is a **pure Rust project**. The authoritative project structure is defined
in `plan.md` §10. Python oracle code was retired in Phase 6 (v0.2.0).

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
│   │   ├── feed.rs             # RSS/Atom (roxmltree)
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

### Current State (All Milestones Complete)

The Rust crate implements all shipped milestones:

```
src/
├── main.rs          # CLI entry (9 subcommands: run, history, history-show, history-compare, delta, daemon, webui, tui, completions)
├── lib.rs           # Pipeline orchestration + integration tests
├── models.rs        # RawItem, NormalizedEvent, ScoredEvent, RunArtifact, etc.
├── fetch.rs         # RSS/Atom parsing + canonical hashing (Cangjie)
├── normalize.rs     # Keyword/actor/region extraction + field scores (Cangjie)
├── scoring.rs       # Im/Fa scoring + rationale (Fuxi)
├── grouping.rs      # Event grouping + causal ordering (Fuxi)
├── backtrack.rs     # Intervention candidate generation (Fuxi)
├── storage.rs       # SQLite 6 tables + history CRUD
├── daemon.rs        # UNIX socket + job queue + serve
├── api.rs           # axum 6-route HTTP API
├── webui.rs         # Embedded static files + API proxy + /queue-run
├── tui.rs           # ratatui history browser (Kanagawa Dark)
├── delta.rs         # Crucix Delta Engine: compute_delta, severity
├── delta_memory.rs  # HotMemory, AlertDecayModel, AlertTier
└── utils.rs         # round2, days_since_epoch, collect_string_array
```

This will expand to the target structure as future phases are implemented.

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

## Examples of Well-Organized Rust Modules

- **Models**: `src/models.rs` — flat struct definitions for all pipeline data types
- **Scoring**: `src/scoring.rs` — `compute_im`, `compute_fa`, `compute_divergence_score`, `build_rationale`, `score_events`
- **Grouping**: `src/grouping.rs` — `group_events`, `summarize_group`, `build_evidence_chain`
- **Backtracking**: `src/backtrack.rs` — `backtrack_candidates`, `infer_intervention_type`, `build_reason`

---

**Language**: English
