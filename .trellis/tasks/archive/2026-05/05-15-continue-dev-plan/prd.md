# Continue Dev Plan

## Goal

Continue from the authoritative Rust rewrite plan by closing the remaining M3C daemon schedule bookkeeping gap before moving to later Hongmeng/Nuwa work.

## What I Already Know

* Root `plan.md` is the architecture authority for the TianJi Rust rewrite.
* `plan.md` marks M3C daemon schedule as the next immediate item: `tianji daemon schedule --every-seconds N --count M`.
* Archived task `.trellis/tasks/archive/2026-05/05-14-m3c-schedule/prd.md` defines the M3C schedule scope and acceptance criteria.
* Current Rust code already contains `DaemonCommands::Schedule`, validation for `--every-seconds >= 60` and `--count >= 1`, repeated `queue_run` submission, JSON schedule output, and unit tests.
* `git status` showed `plan.md` as an existing uncommitted change before this task started; treat existing plan edits as user work and only make minimal targeted edits if needed.

## Requirements

* Verify the existing M3C daemon schedule implementation against the archived M3C PRD acceptance criteria.
* If verification passes, update `plan.md` minimally so M3C is no longer described as in development.
* Preserve the current rule that Python code remains the migration oracle until M6 retirement.
* Do not begin Hongmeng, Nuwa, or new LLM work in this task.
* Do not modify unrelated user changes in `plan.md`.

## Acceptance Criteria

* [ ] `plan.md` no longer lists M3C schedule as pending/in development if the Rust implementation verifies.
* [ ] `cargo test` passes.
* [ ] `cargo fmt --check` passes.
* [ ] `cargo clippy -- -D warnings` passes or any failure is reported with root cause.
* [ ] No unrelated plan rewrites are introduced.

## Definition of Done

* Requirements verified against code and plan state.
* Minimal plan/status updates applied if needed.
* Quality checks run.
* Spec update considered.

## Out of Scope

* Starting Hongmeng orchestration.
* Starting Nuwa simulation sandbox.
* Adding external notifications or cold archive rotation.
* Adding persisted schedules, cron/calendar scheduling, or schedule cancellation APIs.
* Retiring Python code.

## Technical Approach

Use the existing schedule implementation in `src/main.rs` as the source for verification. If it satisfies the archived M3C PRD, update only the stale M3C status references in `plan.md` and run the standard Rust quality checks.

## Decision (ADR-lite)

**Context**: `plan.md` still presents M3C schedule as the next development item, but the codebase already contains the schedule implementation and tests.

**Decision**: Treat this task as an M3C closure/synchronization task instead of starting a new future-phase feature.

**Consequences**: The plan becomes aligned with the codebase, and future development can choose the next milestone from a clean state.

## Technical Notes

* Inspected `plan.md` sections 1, 4, 8, and 9.
* Inspected archived M3C PRD at `.trellis/tasks/archive/2026-05/05-14-m3c-schedule/prd.md`.
* Inspected `src/main.rs` schedule enum, handler, dispatch, and tests.
* Inspected `.trellis/spec/backend/development-plan.md` for migration guardrails.
