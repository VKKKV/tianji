# Continue Rust CLI Development

## Goal

Create the first reviewable Rust migration slice for TianJi without changing the shipped Python product surface. This task starts the Rust scaffold and contract harness described by `plan.md` and `.trellis/spec/backend/rust-migration-plan.md`, using the existing Python pipeline and fixtures as the compatibility oracle.

## What I Already Know

- `plan.md` is a long-range Rust architecture vision, not shipped behavior.
- The current shipped implementation is Python under `tianji/` with `unittest` coverage under `tests/`.
- There is currently no `Cargo.toml` and no `src/**/*.rs` in the repository.
- The Rust migration spec says the first Rust work should port the deterministic CLI/core path before Hongmeng, Nuwa, daemon, TUI, API, or LLM work.
- Milestone 0 allows adding `Cargo.toml`, `src/main.rs`, `src/lib.rs`, initial model modules, and fixture-driven Rust tests or golden-contract checks.
- Existing contract fixtures include `tests/fixtures/sample_feed.xml` and `tests/fixtures/contracts/run_artifact_v1.json`.

## Requirements

- Limit this task to Milestone 0 only.
- Add a minimal Rust crate at the repository root.
- Provide a Rust CLI entrypoint for fixture-based one-shot artifact emission.
- Keep Python code, Python tests, and shipped operator contracts intact.
- Add a fixture-driven Rust contract harness that compares implemented output shape against the existing `RunArtifact` vocabulary where available.
- Make missing parity explicit rather than claiming the full `plan.md` architecture is shipped.
- Add only dependencies needed for this milestone.

## Acceptance Criteria

- [ ] `cargo test` passes locally.
- [ ] `cargo run -- run --fixture tests/fixtures/sample_feed.xml` emits valid JSON.
- [ ] The emitted JSON includes the current top-level `RunArtifact` contract keys.
- [ ] Rust tests pin the scaffolded artifact contract shape against `tests/fixtures/contracts/run_artifact_v1.json`.
- [ ] Python `unittest` suite still passes.
- [ ] No Python code is deleted or replaced.

## Definition of Done

- Tests added or updated for the Rust scaffold.
- `cargo test` passes.
- `.venv/bin/python -m unittest discover -s tests -v` or a documented equivalent passes.
- Docs or Trellis specs are updated only if this task discovers new migration guidance.
- The task is committed before finish-work.

## Technical Approach

Implement Milestone 0 as a small Rust scaffold: root `Cargo.toml`, `src/main.rs`, `src/lib.rs`, and model/output code sufficient to emit a fixture-mode JSON artifact shell. The first implementation should prefer deterministic local fixture input and contract-shape verification over porting full scoring/grouping/storage logic.

## Decision (ADR-lite)

**Context**: The root plan describes a full Rust rewrite with worldline, actor, simulation, daemon, TUI, and LLM layers, but the migration spec requires parity gates before replacing Python behavior.

**Decision**: Start with Rust Milestone 0 only: scaffold plus fixture contract harness.

**Consequences**: This produces an incremental Rust entrypoint that can be reviewed and tested now, while leaving Cangjie/Fuxi semantic parity, SQLite, daemon/API/TUI, Hongmeng, and Nuwa work for later tasks.

## Out of Scope

- Deleting or replacing Python code.
- SQLite history parity.
- Daemon, local API, TUI, or web UI replacement.
- Hongmeng actor runtime.
- Nuwa simulation, profiles, LLM providers, or checkpointing.
- Full field-for-field scoring/backtracking parity unless it naturally fits the scaffold without expanding scope.

## Technical Notes

- Relevant migration spec: `.trellis/spec/backend/rust-migration-plan.md`.
- Relevant shipped contract fixture: `tests/fixtures/contracts/run_artifact_v1.json`.
- Relevant current Python oracle modules: `tianji/pipeline.py`, `tianji/models.py`, `tianji/fetch.py`, `tianji/normalize.py`, `tianji/scoring.py`, `tianji/backtrack.py`.
- Relevant tests: `tests/test_pipeline.py`, `tests/test_scoring.py`, `tests/test_grouping.py`.
- Root docs must keep current Python reality distinct from future Rust target.

## Open Questions

- None.
