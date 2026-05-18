# A1 Input Size Limits

## Goal

Add deterministic input-size caps to prevent oversized feeds and scored-event
payloads from overwhelming the pipeline, storage, API, or TUI while preserving
current behavior for normal feeds.

## What I Already Know

- `plan.md` Phase A1 requires `MAX_RAW_ITEMS: usize = 500` in `src/fetch.rs` and `MAX_SCORED_EVENTS: usize = 500` in `src/lib.rs`.
- `parse_feed` currently returns all valid RSS `<item>` and Atom `<entry>` records.
- `run_feed_text_with_alert_marking` currently scores all normalized events and serializes all scored events into the run artifact and persistence layer.
- The project uses in-module Rust unit/integration tests in `src/lib.rs`, with `cargo test`, `cargo fmt --check`, and `cargo clippy -- -D warnings` as the quality bar.

## Requirements

- Define `MAX_RAW_ITEMS: usize = 500` in `src/fetch.rs` and cap parsed feed items to the first 500 valid titled RSS/Atom entries.
- Define `MAX_SCORED_EVENTS: usize = 500` in `src/lib.rs` and cap the pipeline's scored events to the first 500 scored events before summarization, grouping, backtracking, serialization, and persistence.
- Keep `input_summary.raw_item_count` and `input_summary.normalized_event_count` consistent with the capped pipeline data.
- Preserve existing behavior for feeds at or below the limits.
- Add tests proving RSS/Atom raw parsing truncates at 500 and full pipeline artifacts/persistence do not exceed 500 scored events.

## Acceptance Criteria

- [ ] `parse_feed` returns at most `MAX_RAW_ITEMS` valid items for RSS and Atom feeds.
- [ ] `run_feed_text` artifacts expose at most `MAX_SCORED_EVENTS` scored events.
- [ ] Pipeline summary counts match capped raw/normalized data.
- [ ] Existing sample fixture assertions continue to pass unchanged.
- [ ] `cargo test`, `cargo fmt --check`, and `cargo clippy -- -D warnings` pass.

## Definition of Done

- Tests added or updated for limit behavior.
- Rust formatting and clippy warnings are clean.
- No new dependencies.
- Spec update considered after implementation.

## Technical Approach

- Apply the raw limit by taking only the first `MAX_RAW_ITEMS` parsed valid titled entries in both RSS and Atom parser paths.
- Apply the scored limit immediately after `score_events`, before downstream consumers derive summaries, groups, intervention candidates, JSON output, and persistence inputs.
- Keep constants public only where tests or downstream modules need direct access; otherwise keep them module-scoped.

## Decision (ADR-lite)

**Context**: The current pipeline processes every feed item and scored event, which can create unbounded memory/CPU and large persistence/API payloads.

**Decision**: Enforce fixed deterministic first-N caps at the parser boundary and scored-event boundary, both set to 500 per `plan.md`.

**Consequences**: Oversized feeds are safely bounded with stable ordering. Events beyond the caps are silently omitted for now; observability/warnings are out of scope for this cleanup task.

## Out of Scope

- Making limits configurable.
- Adding warning/logging metrics for truncation.
- Reordering events before applying limits beyond existing pipeline order.
- Changing scoring formulas or feed parsing semantics unrelated to caps.

## Technical Notes

- Relevant files: `src/fetch.rs`, `src/lib.rs`.
- Relevant specs: `plan.md`, `.trellis/spec/backend/index.md`, `.trellis/spec/backend/quality-guidelines.md`.
