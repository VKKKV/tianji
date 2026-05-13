# Rust Migration Plan

This document reconciles the root `plan.md` Rust rewrite vision with the shipped
TianJi product surface. It is a migration handoff, not a claim that Rust is the
current implementation.

## Current Compatibility Baseline

The executable compatibility oracle remains the Python implementation under
`tianji/` plus the `unittest` suite under `tests/`.

Current shipped behavior that Rust must preserve before replacing any Python
surface:

- synchronous `run` over fixture or fetched RSS/Atom input
- deterministic `fetch -> normalize -> score -> backtrack -> emit` behavior
- `RunArtifact` JSON vocabulary, including scored events, grouped summaries,
  and intervention candidates
- local-first and deterministic-first operation
- SQLite-backed history/detail/compare semantics after one-shot artifact parity
- CLI writes as the source of truth until a replacement contract is explicitly
  approved

Do not delete Python code, change operator contracts, or mark Rust as shipped
until the relevant parity gate has passed.

## Mapping `plan.md` To Trellis Phases

`plan.md` remains useful as long-range architecture. Its subsystem names map to
Trellis work in this order:

| Plan subsystem | Rust migration role | Trellis status |
|---|---|---|
| Cangjie | Feed parsing, source items, normalization inputs | First migration target |
| Fuxi | Scoring, grouping, backtrack candidates, artifact assembly | First migration target |
| Hongmeng | Actor/runtime orchestration, daemon expansion, checkpointing | Deferred until core/storage parity |
| Nuwa | Simulation sandbox, forward/backward reasoning, profile/LLM work | Deferred until runtime parity |

The first Rust work should port the current deterministic CLI/core path before
introducing the broader worldline, actor, simulation, or LLM architecture from
`plan.md`.

## Implementation Milestones

### Milestone 0 — Rust Scaffold And Contract Harness

Goal: create a reviewable Rust entrypoint without changing shipped behavior.

Allowed scope:

- add `Cargo.toml`, `src/main.rs`, `src/lib.rs`, and initial model modules
- add fixture-driven Rust tests or golden-contract checks
- expose a Rust command shape for fixture-based one-shot artifact emission
- keep Python code and Python tests intact

Acceptance criteria:

- Rust build/test commands run locally without public-network dependencies
- fixture input can produce JSON shaped against the current Python
  `RunArtifact` contract where implemented
- missing parity is explicit in tests or milestone notes, not hidden behind new
  architecture claims

Out of scope:

- SQLite history parity
- daemon, API, TUI, or web UI replacement
- Hongmeng actor runtime
- Nuwa simulation or LLM providers
- deleting Python code

#### Milestone 0 Code Contract

The provisional Rust CLI command is:

```bash
cargo run -- run --fixture <path>
```

Contract:

- `<path>` must point to a local fixture file readable by the process.
- The command writes a pretty JSON `RunArtifact` payload to stdout.
- The payload must include the current top-level `RunArtifact` keys:
  `schema_version`, `mode`, `generated_at`, `input_summary`,
  `scenario_summary`, `scored_events`, and `intervention_candidates`.
- `input_summary` and `scenario_summary` must preserve the current nested key
  vocabulary from `tests/fixtures/contracts/run_artifact_v1.json` where the
  Milestone 0 scaffold implements those sections.
- Missing Cangjie/Fuxi parity must stay explicit: scaffold output may leave
  scored events and intervention candidates empty, but must not imply scoring,
  grouping, or backtracking parity is complete.

Validation and errors:

| Condition | Required behavior |
|---|---|
| command is not `run --fixture <path>` | exit non-zero and print usage to stderr |
| fixture cannot be read as UTF-8 text | exit non-zero and print the read error to stderr |
| JSON serialization fails | exit non-zero and print the serialization error to stderr |

Good/base/bad cases:

- Good: `cargo run -- run --fixture tests/fixtures/sample_feed.xml` emits valid
  JSON with the current artifact key vocabulary.
- Base: the scaffold reports fixture mode and a deterministic explicit
  no-parity headline while semantic parity is incomplete.
- Bad: adding daemon/API/TUI/LLM dependencies or deleting Python code during
  Milestone 0.

Tests required:

- Rust tests compare emitted top-level and nested summary keys against
  `tests/fixtures/contracts/run_artifact_v1.json`.
- Rust tests assert incomplete scoring/backtracking parity is explicit.
- Existing Python unittest coverage must still pass using the repo-local `uv`
  environment when available; if `.venv` is absent, create it with `uv venv`
  before using `.venv/bin/python`.

Wrong vs correct:

```text
Wrong: cargo run -- predict --field east-asia.conflict
Correct: cargo run -- run --fixture tests/fixtures/sample_feed.xml
```

### Milestone 1 — Cangjie + Fuxi Core Parity

Goal: Rust one-shot output is semantically compatible with the current Python
fixture pipeline.

Scope:

- RSS/Atom fixture parsing
- canonical item identity/content hashing compatible with current artifacts
- deterministic keyword, actor, region, and field-score extraction
- current `Im` / `Fa` scoring semantics and rationale vocabulary
- event grouping, causal/evidence summaries, and backtrack candidates needed for
  artifact parity

Acceptance criteria:

- fixture-driven Rust output matches the current Python artifact vocabulary
  field-for-field where the Python contract is frozen
- score-specific tests pin current deterministic scoring semantics
- Python remains available as the oracle until parity is reviewed and accepted

### Milestone 2 — Storage And History Parity

Goal: port the durable local read model only after one-shot artifact parity.

Scope:

- SQLite persistence compatible with the current run-centric model
- history, history-show, and history-compare read semantics
- projection vocabulary including scored-event and event-group filters

Acceptance criteria:

- Rust history reads preserve the current operator vocabulary
- each successful invocation still creates a run row
- canonical source content reuse does not suppress run history

### Milestone 3 — Local Runtime Parity

Goal: port the thin local runtime only after storage parity.

Scope:

- bounded daemon controls equivalent to the shipped Python surface
- read-first loopback API over the same persisted vocabulary
- no write HTTP routes unless a new contract is approved

### Milestone 4 — TUI/Web And Advanced Architecture

Goal: migrate rich interfaces and future architecture after the deterministic
core, storage, and runtime contracts are stable in Rust.

Scope stays deferred for:

- Ratatui replacement for the Rich TUI
- Axum replacement for optional web/API serving
- Hongmeng Board/Stick actor orchestration
- Nuwa simulation, profile, LLM, and checkpoint-heavy workflows

## Dependency Guidance

The dependency list in `plan.md` is aspirational. The first Rust slices should
add only dependencies needed for the accepted milestone. In particular, do not
add async runtimes, web frameworks, TUI crates, graph engines, or LLM provider
crates before the milestone that uses them.

## Documentation Rules During Migration

- Root docs must distinguish shipped Python reality from Rust migration target.
- Trellis specs should be updated before claiming a Rust layer is current.
- Reference projects may be cited as inspiration only; do not vendor them.
- Compatibility changes should name the Python command, artifact field, or test
  behavior they preserve.
