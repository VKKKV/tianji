# Design Rust Refactor And Development Plan

## Goal

Align the existing `plan.md` full Rust rewrite vision with Trellis so the next Rust work starts from a staged, verifiable migration plan instead of a single broad rewrite. The plan must preserve the currently shipped TianJi Python operator contract as the compatibility baseline while defining the first Rust slices that can be built, tested, and reviewed incrementally on the `rust-cli` branch.

## What I Already Know

* The current branch is `rust-cli`, but the repository still has no Rust source files or `Cargo.toml`.
* The active product surface is still the Python implementation under `tianji/` with `unittest` tests under `tests/`.
* Root `plan.md` describes a full Rust rewrite with four major subsystems: Cangjie, Fuxi, Hongmeng, and Nuwa.
* `plan.md` proposes Rust dependencies including `clap`, `serde`, `quick-xml`, `reqwest`, `tokio`, `rusqlite`, `ratatui`, `axum`, `petgraph`, `blake3`, and LLM-related crates.
* Current docs and Trellis specs emphasize that shipped reality must stay distinct from future architecture.
* Current shipped contracts include one-shot `run`, SQLite persistence, history/detail/compare reads, read-only Rich TUI, thin daemon controls, loopback read API, and optional web UI.
* Existing backend specs still describe a Python 3.12+ stdlib-first implementation, so a Rust migration plan must explicitly update specs before implementation claims Rust as current reality.

## Constraints

* Do not delete the Python implementation until Rust has parity for the agreed migration gate.
* Do not treat aspirational Hongmeng/Nuwa architecture as shipped behavior.
* Keep CLI writes as the source of truth until a Rust replacement explicitly matches or supersedes the current contract.
* Keep local-first and deterministic-first behavior as non-negotiable product principles.
* Avoid importing or vendoring reference repositories; use them only as citation-level inspiration.
* Separate planning/spec updates from large implementation commits.

## Requirements

* Create a Rust migration plan that maps `plan.md` into Trellis phases and implementation slices.
* Define a compatibility baseline for Rust Phase 0/1 using current Python commands, artifact vocabulary, and tests as reference behavior.
* Identify which Rust modules should be built first and which plan.md subsystems remain deferred.
* Define explicit acceptance criteria for the first Rust implementation slice.
* Update Trellis context so future implement/check sub-agents load the Rust migration plan and backend specs.
* Keep this task scoped to planning/spec alignment only; do not write Rust code in this task.

## Proposed Scope Boundary

### MVP For The Rust Refactor Plan

* Produce planning artifacts and spec alignment only.
* Treat the first implementation milestone as Rust CLI/core parity for `run` over fixture input, deterministic normalization/scoring/backtracking, JSON artifact output, and tests.
* Keep SQLite history, daemon, TUI, HTTP API, web UI, Hongmeng actor runtime, Nuwa simulation, LLM providers, and full checkpointing out of the first implementation milestone unless explicitly re-scoped.
* Do not add `Cargo.toml`, `src/**/*.rs`, or Rust dependencies in this task.

### Deferred From First Rust Slice

* Full Tokio actor runtime.
* Board/Stick multi-agent protocol.
* Forward and backward LLM simulation.
* Ratatui TUI replacement.
* Axum HTTP API replacement.
* SQLite schema migration beyond what is needed for artifact parity.
* Deleting Python code.

## Acceptance Criteria

* [ ] `plan.md` is reconciled with Trellis by writing an implementation-ready PRD for the Rust refactor path.
* [ ] The PRD names what is current shipped behavior versus future Rust architecture.
* [ ] The first Rust implementation slice has a narrow parity target and explicit out-of-scope list.
* [ ] `implement.jsonl` and `check.jsonl` contain real context entries for backend specs and this plan.
* [ ] User confirms the scope before this task is started for execution.
* [ ] No Rust source files are created by this planning task.

## Definition Of Done

* PRD is complete enough for a `trellis-implement` sub-agent to create the next concrete Rust scaffolding plan or code slice.
* Relevant Trellis backend spec files are registered in task context JSONL.
* No runtime behavior is changed by this planning task unless the user explicitly expands scope.

## Technical Approach

* Use the current Python implementation as the executable compatibility oracle.
* Convert `plan.md` into a staged Rust roadmap rather than implementing every subsystem at once.
* Treat Cangjie + Fuxi as the first Rust domain because they map directly onto current `fetch -> normalize -> score -> backtrack -> emit` behavior.
* Treat Hongmeng + Nuwa as later layers after Rust core parity exists.
* Keep root docs and Trellis specs honest by labeling Rust architecture as migration target until implemented.

## Candidate Implementation Milestones

### Milestone 0: Rust Scaffold And Contract Harness

* Add `Cargo.toml`, `src/main.rs`, `src/lib.rs`, core models, and fixture-driven tests.
* Implement `tianji run --fixture tests/fixtures/sample_feed.xml` or equivalent Rust binary command.
* Emit JSON shaped to match the current Python `RunArtifact` contract where feasible.
* Keep Python code intact.

### Milestone 1: Cangjie + Fuxi Core Parity

* Port RSS/Atom fixture parsing, normalization keywords/actors/regions, scoring `Im`/`Fa`, grouping, and backtrack candidates.
* Add exact or semantic parity tests against existing fixture expectations.
* Keep persistence and daemon out unless needed for parity validation.

### Milestone 2: Storage And History Parity

* Port SQLite persistence and history/detail/compare read surfaces after the one-shot artifact is stable.
* Preserve the current run-centric read model and projection vocabulary.

### Milestone 3: Local Runtime Parity

* Port daemon/control plane and read-first loopback API after storage parity.
* Keep API loopback-only and read-first unless a new contract is approved.

### Milestone 4: TUI/Web And Advanced Architecture

* Rebuild TUI or web slices only after CLI/storage/API contracts are stable in Rust.
* Start Hongmeng/Nuwa simulation work only after the deterministic core has migrated.

## Open Questions

* None. User selected planning/spec only for this task.

## Out Of Scope

* Immediate full Rust rewrite in one task.
* Creating Rust scaffold or implementation code in this task.
* Deleting Python code in the first Rust milestone.
* Replacing the shipped daemon, TUI, API, or web UI before one-shot Rust parity.
* Introducing mandatory cloud or LLM dependencies into the core path.

## Technical Notes

* Root plan reviewed: `plan.md`.
* Current roadmap spec reviewed: `.trellis/spec/backend/development-plan.md`.
* Current directory and quality guidelines reviewed: `.trellis/spec/backend/directory-structure.md`, `.trellis/spec/backend/quality-guidelines.md`.
* Current README confirms Python is shipped reality and Rust-style Cangjie/Hongmeng/Fuxi/Nuwa remains long-term direction.
* Scope decision: user chose planning/spec only as the first Trellis task boundary.
