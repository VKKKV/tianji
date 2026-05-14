# Sync Plans After Crucix Phase 2

## Goal

Synchronize `plan.md` and `plan-crucix.md` with the completed Crucix Delta Engine Phase 2 implementation so the authoritative planning docs match the current Rust state.

## Context

- Crucix Phase 2 was implemented and committed in `fd33f55` and `b3883a4`.
- `.trellis/spec/backend/development-plan.md` already records daemon auto-delta contracts.
- `plan.md` still says Crucix daemon auto-delta is pending and reports outdated test counts/status.
- `plan-crucix.md` still lists Phase 2 items as pending.

## Requirements

- Update `plan.md` current-state summary to reflect:
  - Crucix Delta Engine daemon auto-delta / AlertTier surfacing is complete.
  - Rust test count is now 85.
  - M3C schedule remains deferred.
- Update `plan.md` Milestone 3.5 section:
  - Add completed Phase 2 items: `RunResult`, daemon job status delta fields, `/api/v1/delta/latest`, CLI `alert_tier`, f64 numeric thresholds, shared `collect_string_array` utility.
  - Remove or revise pending daemon auto-delta item.
  - Keep truly pending items limited to external notification delivery / cold archive / cron-like housekeeping if still relevant.
- Update `plan-crucix.md` migration path:
  - Mark Phase 1 complete.
  - Mark Phase 2 complete.
  - Keep Phase 3 external webhook notifications as pending / on demand.
- Preserve the distinction that Python under `tianji/` and `tests/` remains the migration oracle until M6.

## Acceptance Criteria

- `plan.md` no longer says daemon auto-delta is pending.
- `plan-crucix.md` no longer says Phase 2 is pending.
- No code changes are made.
- `git diff` shows only planning/doc files plus task metadata.

## Out of Scope

- New Rust code changes.
- Implementing external notifications.
- M3C schedule.
- Python retirement.
