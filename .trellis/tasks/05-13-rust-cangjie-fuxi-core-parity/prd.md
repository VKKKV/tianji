# Rust Cangjie/Fuxi Core Parity

## Goal

Advance the Rust migration from Milestone 0 scaffold into the first deterministic Cangjie/Fuxi parity slice while keeping Python as the compatibility oracle. This task should replace the placeholder fixture counting with real fixture feed parsing, canonical source hashes, and normalized event emission that matches the current Python vocabulary where implemented.

## What I Already Know

- The previous task added a minimal Rust crate with `cargo run -- run --fixture <path>` and contract-shape tests.
- The Rust migration plan defines Milestone 1 as RSS/Atom parsing, canonical hashing, normalization, scoring, grouping, and backtrack parity.
- Python remains the oracle until parity is reviewed and accepted.
- Current Python feed parsing lives in `tianji/fetch.py` and supports RSS 2.0 plus Atom 1.0 from local fixtures.
- Current Python canonical hashes are SHA-256 over cleaned text:
  - `entry_identity_hash = sha256(clean(link) + "|" + clean(published_at or ""))`
  - `content_hash = sha256(clean(title) + "|" + clean(summary) + "|" + clean(published_at or ""))`
- Current Python normalization lives in `tianji/normalize.py` and extracts keywords, actors, regions, field scores, and event IDs deterministically.
- Current Python scoring/grouping/backtracking is larger and should remain the oracle until Rust parsing/normalization is stable.

## Recommended Scope

Implement **Milestone 1A: Feed + normalization parity**.

## Requirements

- Keep this task scoped to Milestone 1A only.
- Replace placeholder `<item>` counting with real local fixture parsing for RSS 2.0 and Atom 1.0.
- Add Rust model structs for raw items and normalized events using the current Python field vocabulary.
- Implement canonical entry identity and content hashes compatible with Python.
- Implement deterministic normalization compatible with Python for:
  - cleaned title and summary text
  - keyword extraction limit and token rules
  - actor and region matching
  - field score derivation
  - event ID derivation
- Emit normalized-event-shaped data through the existing Rust artifact until scoring parity exists.
- Keep scoring, grouping, and backtracking parity explicitly out of scope unless we choose a larger slice.
- Keep Python code and tests intact.
- Add only dependencies needed for local parsing/hashing/normalization.

## Acceptance Criteria

- [ ] `cargo test` passes.
- [ ] `cargo run -- run --fixture tests/fixtures/sample_feed.xml` emits valid JSON.
- [ ] Rust fixture parsing produces the same raw item count as Python for RSS and Atom fixtures.
- [ ] Rust canonical hashes match Python expectations for the sample fixture.
- [ ] Rust normalized event IDs, keywords, actors, regions, and field scores match Python for `tests/fixtures/sample_feed.xml`.
- [ ] Rust output still makes missing scoring/backtracking parity explicit.
- [ ] Python tests pass via `uv` environment: `uv pip install -e . && .venv/bin/python -m unittest discover -s tests -v`.

## Definition of Done

- Rust tests cover RSS, Atom, canonical hashes, and normalization parity.
- `cargo fmt --check` passes.
- `cargo test` passes.
- Python unittest suite passes via the repo-local `uv` environment.
- Trellis specs are updated if this task establishes new Rust migration contracts.
- Changes are committed before finish-work.

## Technical Approach

Add small Rust modules mirroring the current Python stage boundaries only where needed for Milestone 1A: feed parsing, normalization, and artifact assembly. Use the Python implementation and existing fixture tests as the oracle; do not introduce Hongmeng, Nuwa, storage, daemon, API, TUI, web UI, or LLM concerns.

## Decision (ADR-lite)

**Context**: Full Milestone 1 includes parsing, normalization, scoring, grouping, and backtracking. Implementing all of that in one task would mix parsing correctness with scoring semantics and make review harder.

**Decision**: Start with Milestone 1A: feed parsing, canonical hashes, and normalization parity.

**Consequences**: Rust moves beyond scaffold into real deterministic data extraction, while scoring/grouping/backtrack parity remains clearly deferred for a follow-up task.

## Out of Scope

- Full `Im` / `Fa` scoring parity.
- Scenario summary parity beyond explicit no-scoring placeholder text.
- Event grouping, causal clustering, and grouped summaries.
- Backtrack intervention candidate parity.
- SQLite persistence and history commands.
- Fetching live URLs.
- Daemon/API/TUI/web UI replacement.
- Hongmeng or Nuwa architecture.
- Deleting or replacing Python code.

## Technical Notes

- Relevant migration spec: `.trellis/spec/backend/rust-migration-plan.md`.
- Current Rust scaffold: `Cargo.toml`, `src/lib.rs`, `src/main.rs`, `src/models.rs`.
- Python oracle modules: `tianji/fetch.py`, `tianji/normalize.py`, `tianji/models.py`.
- Larger deferred Python oracle modules: `tianji/scoring.py`, `tianji/pipeline.py`, `tianji/backtrack.py`.
- Existing canonical fixture: `tests/fixtures/sample_feed.xml`.
- Existing Python tests include RSS/Atom parsing and duplicate taxonomy/hash behavior in `tests/test_pipeline.py`.

## Open Questions

- None.
