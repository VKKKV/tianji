# A2 Remove Deprecated Delta Memory Functions

## Goal

Remove deprecated wall-clock-based `HotMemory` alert helpers so delta memory
alert suppression remains deterministic and tied to persisted run timestamps.

## What I Already Know

- `plan.md` Phase A2 requires deleting `is_signal_suppressed`, `mark_alerted`, and `prune_stale_signals` from `src/delta_memory.rs`.
- Repository search shows these methods have no callers outside their own definitions.
- Current callers and tests already use timestamp-explicit variants: `is_signal_suppressed_at`, `is_signal_suppressed_at_timestamp`, `mark_alerted_at`, `prune_stale_signals_at`, and `prune_stale_signals_at_timestamp`.
- The deprecated helpers depend on `unix_now()`, which is intentionally avoided in alert behavior that must be deterministic across persisted runs.

## Requirements

- Delete `HotMemory::is_signal_suppressed`.
- Delete `HotMemory::mark_alerted`.
- Delete `HotMemory::prune_stale_signals`.
- Keep all timestamp-explicit replacement methods unchanged.
- Preserve current tests and behavior for timestamp-based alert suppression, marking, and pruning.
- Remove now-unused imports or helpers only if they become unused after deletion.

## Acceptance Criteria

- [ ] No non-test or test caller references the deleted deprecated methods.
- [ ] `cargo test` passes.
- [ ] `cargo clippy -- -D warnings` passes without deprecated-method warnings or unused-code warnings introduced by this task.
- [ ] `cargo fmt --check` status is recorded.

## Definition of Done

- Deprecated methods are removed from `src/delta_memory.rs`.
- No behavior change to timestamp-explicit alert APIs.
- No new dependencies.
- Spec update considered after implementation.

## Technical Approach

- Remove the three deprecated wrapper methods only.
- Leave `unix_now()` in place if still used by non-deprecated code; otherwise remove it with its associated imports.
- Run a repository search after deletion to ensure no stale references remain.

## Decision (ADR-lite)

**Context**: Alert suppression must use persisted run timestamps for deterministic behavior. Wall-clock wrapper methods were deprecated after timestamp-explicit APIs were introduced.

**Decision**: Remove the deprecated wrappers instead of keeping compatibility shims, because there are no current callers and `plan.md` explicitly calls for deletion.

**Consequences**: Internal API surface is smaller and future call sites cannot accidentally reintroduce wall-clock nondeterminism.

## Out of Scope

- Renaming or refactoring the timestamp-explicit methods.
- Changing alert decay thresholds or pruning semantics.
- Introducing a time utility module; that is tracked separately as Phase B1.

## Technical Notes

- Relevant file: `src/delta_memory.rs`.
- Relevant specs: `plan.md`, `.trellis/spec/backend/index.md`, `.trellis/spec/backend/quality-guidelines.md`.
