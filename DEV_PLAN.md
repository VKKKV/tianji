# TianJi Development Plan

## Current State

Owned TianJi source is intentionally narrow:

- `tianji/` — Python one-shot CLI pipeline
- `tests/` — fixture-first verification

Everything else at the workspace root is reference material, not the long-term product codebase.

## Product Direction

TianJi should grow in this order:

1. strengthen the owned Python core
2. add persistence and repeatable local workflows
3. formalize divergence and backtracking logic
4. complete the CLI-first operator workflow
5. add a terminal UI with Vim-style navigation on top of stable local data/contracts
6. introduce a daemon/orchestrator only after the one-shot path is solid
7. add an optional web UI only after CLI and TUI workflows are stable
8. retire embedded reference repos once their useful ideas are reimplemented in first-party code

## Phase 1 — Harden the Owned Core

Goal: turn the current MVP into a dependable local tool.

Shipped in the current branch:

- configurable source list instead of only ad hoc CLI URLs
- SQLite persistence for raw items, normalized events, and run artifacts
- stable artifact schema versioning
- more explicit error handling for malformed feeds and fetch failures
- broader deterministic tests for RSS, Atom, mixed-source runs, and core config/error branches

Still open before Phase 1 is fully closed:

- final doc/examples pass whenever the CLI surface changes again

Exit criteria:

- repeatable runs with local persistence
- no dependence on embedded reference repos at runtime
- test suite covers the core stage transitions

## Phase 2 — Formalize Divergence Logic

Goal: replace rough heuristics with a first-party TianJi scoring model.

Deliverables:

- dedicated scoring model spec inside first-party docs/code
- explicit definitions for TianJi versions of `Im` and `Fa`
- richer event grouping and causal clustering
- backtracking that references evidence chains, not only top-ranked events

Use references from:

- `DivergenceMeter/README.md` for vocabulary and conceptual framing
- `worldmonitor/` for signal extraction and ranking patterns

Do not do:

- direct runtime dependency on DivergenceMeter code
- opaque LLM scoring as the default path

## Phase 3 — Persistence to Local Operating System

Goal: move from one-shot report generation to a durable local system.

Deliverables:

- storage module in first-party TianJi code
- run history and replayable artifacts
- source configuration file and fetch policies
- idempotent dedupe and content-hash storage

This is the point where TianJi starts owning “source code” for ingestion and state instead of leaning on nearby references for design inspiration.

## Phase 4 — CLI Completion

Goal: finish the operator workflow in the terminal before adding any richer interface layer.

Deliverables:

- complete persisted history/query ergonomics
- strong compare/navigation shortcuts for stored runs
- stable local docs and commands for day-to-day operator usage
- no hidden dependency on any future daemon or UI layer

This phase ends when TianJi feels coherent as a terminal-first tool even without any additional interface.

## Phase 5 — Terminal UI (Vim-Motion TUI)

Goal: add a keyboard-first terminal interface after the CLI surface is complete.

Principles:

- TUI comes **after** CLI maturity, not before
- TUI comes **before** any web GUI
- navigation should be Vim-oriented by default
- TUI should reuse the same local run/history/artifact concepts the CLI already exposes
- no duplication of business logic inside the TUI layer

Planned shape:

- browse run history
- inspect one run and its scored events/interventions
- compare runs
- navigate with Vim-style motions and shortcuts

Do not do:

- do not turn the TUI into a second orchestration runtime
- do not bypass CLI/storage contracts with ad hoc state
- do not let TUI-specific UX force premature web/API design

## Phase 6 — Hongmeng Lite

Goal: introduce a small local orchestrator only when the data path is stable.

Deliverables:

- background process or daemon entrypoint
- local command API over UNIX domain sockets or an equivalent local transport
- job execution for on-demand and scheduled runs
- status inspection from CLI

Keep it narrow:

- no distributed system
- no cloud dependency
- no mandatory web stack

## Phase 7 — Optional Web UI

Goal: add a future web UI without coupling it to the core engine.

Principles:

- UI remains optional and off by default
- UI is a separate service or process boundary
- backend contract should already exist before UI work starts
- CLI and TUI should already be mature before UI work starts
- CLI remains the source-of-truth operator surface

Planned shape:

- lightweight API layer exposing run history, current status, artifacts, and intervention candidates
- WebSocket or polling for live run progress later, not in the first UI slice
- initial UI scope limited to:
  - run a pipeline
  - inspect artifacts
  - compare historical runs
  - browse intervention candidates

TUI remains the preferred rich local interface before this phase exists.

Reference use:

- borrow workflow presentation ideas from `MiroFish/frontend/`
- borrow decoupled service thinking from `worldmonitor/` and `oh-my-openagent/`
- do not adopt any reference frontend wholesale

## Phase 8 — Reference Repo Retirement

Goal: remove the embedded local reference repositories from the long-term TianJi workspace.

Strategy:

1. classify what each reference repo contributes
2. reimplement only the useful pieces inside first-party TianJi modules
3. keep external links or notes to upstream repos for historical context
4. remove the embedded copies once TianJi no longer needs local side-by-side study

Recommended contribution map:

- `worldmonitor/`
  - keep as inspiration for ingestion, signal extraction, caching, and service boundaries
  - reimplement only the narrow server/data ideas TianJi actually needs

- `DivergenceMeter/`
  - keep as conceptual input for divergence terminology
  - reimplement the formulas and tests in owned TianJi code

- `MiroFish/`
  - keep as inspiration for simulation-stage decomposition and future web workflow ideas
  - reimplement a much smaller simulation/report boundary later

- `oh-my-openagent/`
  - keep as inspiration for orchestration, terminal integration, and modular tool boundaries
  - reimplement only if TianJi truly needs those operating patterns

Retirement trigger:

- TianJi has first-party modules for ingestion, scoring, persistence, orchestration, and optional UI planning
- architecture docs cite upstream inspiration without requiring local vendored copies

## Immediate Backlog

1. define first-party TianJi `Im` / `Fa` spec
2. formalize richer event grouping and causal clustering
3. finish the remaining CLI ergonomics for persisted analysis workflows
4. define the Vim-motion TUI contract and navigation model
5. draft the future local API contract that a web UI would consume
6. begin actual local API implementation only when a real process boundary is chosen

Draft contract note now lives in `LOCAL_API_CONTRACT.md`; implementation remains future work.

## Guardrails

- keep first-party source under `tianji/` and `tests/`
- prefer reimplementation over cross-importing from references
- avoid framework-first expansion
- keep web UI future-compatible but not current-scope
- every new layer should preserve local-first, deterministic-first behavior
