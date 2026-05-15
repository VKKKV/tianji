# Continue From Plan

## Goal

Implement the first ordered follow-up from `plan.md` after M3C completion: M3.5 hot-memory housekeeping that reduces duplicate daemon auto-delta / alert-marker I/O without changing externally visible daemon behavior.

## What I Already Know

* Root `plan.md` is the authoritative architecture document for the TianJi Rust rewrite.
* Current shipped/complete Rust work includes M1A, M1B, M2, M3A, M3B, M3C, M4 TUI MVP, and Crucix Delta Engine daemon auto-delta / AlertTier surfacing.
* `plan.md` lists M3.5 follow-ups as pending: external alert delivery, cold archive rotation, hot-memory pruning automation, and housekeeping to reduce hot-memory I/O.
* `plan.md` marks Hongmeng, Nuwa, full TUI, and cleanup as deferred.
* `src/delta_memory.rs` already has `AlertTier`, `AlertDecayModel`, `HotMemory`, alerted-signal suppression, pruning, and atomic save/load behavior.
* `src/daemon.rs` already computes delta/alert tier after daemon jobs and calls `mark_delta_signals_alerted`.

## Requirements

* Continue from `plan.md` using the first bounded, testable M3.5 follow-up slice.
* Reduce duplicated hot-memory load/save work in the daemon success path where a persisted run already updated hot memory and daemon alert marking currently reloads/saves it again.
* Preserve existing daemon job status output: `delta_tier`, `delta_summary`, `run_id`, and state behavior remain unchanged.
* Preserve existing hot-memory semantics: successful persisted runs still push compact run data, retain bounded hot runs, classify alert tier, prune stale signals, and mark delta signals alerted.
* Keep the implementation local-first and deterministic where fixture run timestamps are involved.
* Update `plan.md` if this housekeeping item is completed.
* Preserve Python oracle code until M6 retirement.
* Do not introduce LLM/provider dependencies unless the chosen scope explicitly starts Hongmeng/Nuwa.
* Do not implement external notifications, cold archive rotation, or new persisted schedule state in this task.

## Acceptance Criteria

* [ ] Daemon successful persisted jobs update hot memory and mark delta signals alerted with one consolidated memory write path, or an equivalent reduction that avoids the current duplicate load/save cycle.
* [ ] Existing `RunResult` and daemon job-status behavior remains compatible with current tests.
* [ ] Tests cover the consolidated daemon hot-memory update / mark-alerted behavior or protect against regression in the relevant helper.
* [ ] `plan.md` no longer lists this housekeeping optimization as pending once implemented.
* [ ] `cargo fmt --check`, `cargo test`, and `cargo clippy -- -D warnings` pass.
* [ ] `plan.md` or specs are updated if behavior/status changes.

## Definition of Done

* Tests added/updated where behavior changes.
* Lint, format, and tests pass.
* Spec update considered after implementation.
* Work committed before finish-work.

## Out of Scope

* Python retirement.
* Remote/multi-tenant daemon access.
* Claiming Hongmeng/Nuwa complete before parity gates pass.
* External alert delivery: Telegram, Discord, webhook, retries, secrets, and config.
* Cold archive rotation.
* Cron/calendar scheduling.

## Technical Approach

Refactor the persisted-run/hot-memory path so daemon success processing does not reload and resave the same hot memory solely to mark alert signals. Prefer a small helper that updates hot memory and alerted-signal state together after persistence, while preserving the existing `RunResult` fields used by CLI and daemon status.

## Decision (ADR-lite)

**Context**: `plan.md` says to continue in order, and the first remaining M3.5 follow-up is housekeeping to merge hot-memory update and mark-alerted write paths.

**Decision**: Implement M3.5 housekeeping first, before cold archive rotation, external notifications, or Hongmeng/Nuwa work.

**Consequences**: This keeps the next step small, deterministic, and within the already-shipped daemon/delta layer. More visible features remain separate future tasks.

## Technical Notes

* Inspected `plan.md` sections 1, 3, and 12.
* Inspected `.trellis/spec/backend/development-plan.md` migration guardrails.
* Inspected `src/delta_memory.rs` for current alert memory primitives.
* Inspected `src/daemon.rs` for job completion and alert marking flow.
