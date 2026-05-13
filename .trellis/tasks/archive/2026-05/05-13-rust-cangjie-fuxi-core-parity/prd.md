# Rust Cangjie/Fuxi Core Parity

## Goal

Advance the Rust migration through full Cangjie/Fuxi parity: scoring, grouping,
and backtracking, so that `cargo run -- run --fixture <path>` produces a
`RunArtifact` that is field-for-field compatible with the current Python fixture
pipeline output.

## Current Status

**Milestone 1A — Feed + Normalization Parity: DONE.**

- RSS 2.0 and Atom 1.0 local fixture parsing ✅
- SHA-256 entry identity and content hashes ✅
- Deterministic normalization (keywords, actors, regions, field scores, event IDs) ✅
- All 6 Rust tests pass ✅
- `cargo fmt --check` clean ✅
- Python code and tests intact ✅

**Next: Milestone 1B — Scoring + Grouping + Backtracking Parity.**

## Requirements (Milestone 1B)

Port the remaining Python pipeline stages to Rust so that one-shot fixture
output reaches full `RunArtifact` parity:

1. **Scoring** — port `tianji/scoring.py` to `src/scoring.rs`:
   - `compute_im`: actor weights, region weights, keyword density cap,
     dominant-field bonus, field-diversity bonus (thresholded at >= 1.0),
     text-signal intensity (boundary-aware cue matching)
   - `compute_fa`: dominant-field strength, dominance margin, coherence share,
     near-tie penalty, diffuse-third-field penalty
   - `divergence_score = f(Im, Fa)`
   - Rationale vocabulary matching Python additive rationale terms
   - Exact-value test matching Python oracle output for sample fixture

2. **Grouping** — port grouping logic to `src/grouping.rs`:
   - Shared keyword/actor/region + time window (24h) for event grouping
   - Causal ordering + evidence chain
   - `EventGroupSummary` with headline, member events, evidence, causal cluster
   - Transitive causal clustering and admission-path causal ordering
   - `causal_span_hours` when at least two timestamps are known

3. **Backtracking** — port `tianji/backtrack.py` to `src/backtrack.rs`:
   - Intervention candidate generation from grouped events
   - Dominant-field → intervention_type mapping
   - Field-aware intervention suggestions

4. **Artifact Assembly** — update `src/lib.rs` pipeline:
   - Replace placeholder scenario summary with real scoring/grouping results
   - Replace normalized-event-shaped payloads with proper `ScoredEvent` structs
   - Populate `intervention_candidates` with real backtrack output
   - Remove the "not implemented yet" headline

5. **Model Updates** — extend `src/models.rs`:
   - `ScoredEvent` struct with `impact_score`, `field_attraction`,
     `divergence_score`, `dominant_field`, `rationale`
   - `EventGroupSummary` struct matching Python vocabulary
   - `InterventionCandidate` struct matching Python vocabulary
   - Update `ScenarioSummary` with real derived fields

## Acceptance Criteria

- [ ] `cargo test` passes (existing + new tests).
- [ ] `cargo fmt --check` passes.
- [ ] `cargo run -- run --fixture tests/fixtures/sample_feed.xml` emits valid JSON.
- [ ] Rust scored events have correct `impact_score`, `field_attraction`,
      `divergence_score`, `dominant_field`, and `rationale` for the sample fixture.
- [ ] Rust event groups match Python grouping output for the sample fixture.
- [ ] Rust intervention candidates match Python backtrack output.
- [ ] Rust `RunArtifact` top-level and nested keys match the Python contract fixture.
- [ ] The "not implemented yet" headline is replaced with real scenario summary.
- [ ] Python tests pass via the repo-local `uv` environment.
- [ ] No new dependencies beyond what Milestone 1B requires (no async, web, TUI, LLM crates).

## Definition of Done

- Rust tests cover scoring exact-value parity, grouping, and backtracking.
- `cargo fmt --check` passes.
- `cargo test` passes.
- Python unittest suite passes via the repo-local `uv` environment.
- Trellis specs are updated if this task establishes new Rust migration contracts.
- Changes are committed before finish-work.

## Technical Approach

Add Rust modules `src/scoring.rs`, `src/grouping.rs`, `src/backtrack.rs` mirroring
the current Python stage boundaries. Extend `src/models.rs` with `ScoredEvent`,
`EventGroupSummary`, and `InterventionCandidate` structs. Update `src/lib.rs`
pipeline to chain all stages. Use the Python implementation and fixture tests as
the oracle. Do not introduce Hongmeng, Nuwa, storage, daemon, API, TUI, web UI,
or LLM concerns.

## Out of Scope

- SQLite persistence and history commands (Milestone 2).
- Fetching live URLs.
- Daemon/API/TUI/web UI replacement.
- Hongmeng or Nuwa architecture.
- Deleting or replacing Python code.
- `reqwest`, `tokio`, `axum`, `ratatui`, `rusqlite`, `async-openai`,
  `ollama-rs`, `petgraph` — none of these are needed for Milestone 1B.

## Technical Notes

- Relevant migration spec: `.trellis/spec/backend/development-plan.md`.
- Scoring spec: `.trellis/spec/backend/scoring-spec.md`.
- Current Rust implementation: `Cargo.toml`, `src/{main,lib,models,fetch,normalize}.rs`.
- Python oracle modules for 1B: `tianji/scoring.py`, `tianji/pipeline.py`,
  `tianji/backtrack.py`, `tianji/models.py`.
- Existing canonical fixture: `tests/fixtures/sample_feed.xml`.
- Existing contract fixture: `tests/fixtures/contracts/run_artifact_v1.json`.
- Existing Python tests include scoring, grouping, and backtrack behavior in
  `tests/test_scoring.py`, `tests/test_grouping.py`, `tests/test_pipeline.py`.
