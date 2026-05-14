# Address Crucix Phase 2 Review Followups

## Goal

Resolve minor followups from the Crucix Phase 2 code review without breaking existing CLI compatibility.

## Context

- Crucix Phase 2 is complete and committed.
- Review found three minor followups:
  - NC1: `tianji run` currently discards `RunResult.delta` / `alert_tier` in CLI output.
  - NC2: `src/tianji.egg-info/` is stale Python packaging metadata in the Rust source tree.
  - NC3: daemon alert signal marking currently performs extra hot-memory I/O; this is not a correctness bug and should be handled in M3C schedule/housekeeping.
- `development-plan.md` explicitly requires CLI artifact output to preserve the shipped `RunArtifact` JSON. Therefore NC1 must be opt-in, not a default output-shape change.

## Requirements

- Add an opt-in CLI flag to `tianji run` for delta context, recommended name: `--show-delta`.
- Default `tianji run` output remains exactly `RunArtifact` JSON for compatibility.
- With `--show-delta`, output a wrapper JSON that includes:
  - `artifact`
  - `delta`
  - `alert_tier`
- Remove stale `src/tianji.egg-info/` if present.
- Update `plan.md` / `plan-crucix.md` to record NC3 as M3C schedule/housekeeping optimization, not a current bug.
- Add or update tests for default output compatibility and `--show-delta` wrapper output.

## Acceptance Criteria

- `tianji run --fixture ...` still emits the original top-level `RunArtifact` JSON.
- `tianji run --fixture ... --sqlite-path ... --show-delta` emits wrapper JSON with `artifact`, `delta`, and `alert_tier`.
- No `src/tianji.egg-info/` remains.
- M3C / housekeeping docs mention consolidating hot-memory update + mark-alerted I/O.
- `cargo test`, `cargo fmt --check`, and `cargo clippy -- -D warnings` pass.

## Out of Scope

- Default `tianji run` output shape change.
- Implementing M3C schedule.
- Optimizing daemon hot-memory I/O now.
- External webhook notifications.
