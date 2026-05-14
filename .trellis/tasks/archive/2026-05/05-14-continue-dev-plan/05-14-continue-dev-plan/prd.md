# Continue TUI Development From Plan

## Goal

Continue TianJi development from the current post-dashboard TUI state by adding the next small, plan-aligned Rust TUI slice: a read-only single-run detail view backed by existing storage read models.

## What I Already Know

* `plan.md` is the authoritative architecture document for the Rust rewrite and TUI design.
* `.trellis/spec/backend/development-plan.md` marks Milestone 4 TUI as the active deferred area after M1-M3/M3.5 completion.
* The prior `05-14-continue-dev-plan` task selected and implemented the TUI dashboard slice.
* `src/tui.rs` now has dashboard and history views, Kanagawa Dark styling, read-only behavior, and view switching via `d`/`h`/`1`/`2`.
* `.trellis/spec/backend/contracts/tui-contract.md` recommends extending read-only persisted-run browsing in this order: history list, single-run detail, run compare, then live/runtime concerns.
* Existing storage already exposes `get_run_summary(sqlite_path, run_id, scored_filters, only_matching_interventions, group_filters)` for the `history-show` read surface.

## Requirements

* Keep the TUI read-only.
* Preserve existing dashboard and history behavior and keybindings.
* Add a single-run detail view reachable from the history view for the currently selected persisted run.
* Use the minimal detail-only MVP: Enter opens selected run detail, and Esc or `h` returns to history.
* Source detail payloads from existing storage `get_run_summary` semantics rather than duplicating scoring/grouping/projection logic in the TUI.
* Render detail output without requiring an interactive terminal in tests.
* Degrade stable empty/error states without panics when a selected run cannot be loaded.
* Do not add compare, filters/search, simulation, profile, daemon-control, queueing, feed fetching, or storage mutation in this task.

## Acceptance Criteria

* [ ] Existing `tianji tui --sqlite-path ...` still opens with the dashboard when persisted runs exist.
* [ ] Existing history browsing and dashboard/history switching still work.
* [ ] From history, operators can open a selected run detail view and return to history without leaving the TUI.
* [ ] Detail MVP does not add previous/next stepping, compare staging, search, filters, or projection lens controls.
* [ ] The detail view surfaces stable `history-show` fields: run id, schema version, mode, generated time, input/scenario summary, scored events, event groups, and intervention candidates where available.
* [ ] Missing detail data renders stable placeholder text instead of panicking.
* [ ] Unit tests cover detail state mapping, formatting, view switching, and preserved history/dashboard navigation behavior without launching a terminal.
* [ ] `cargo test`, `cargo fmt --check`, and `cargo clippy -- -D warnings` pass.

## Definition of Done

* Tests added or updated for the new detail view behavior.
* Rust formatting, tests, and clippy pass.
* Specs updated if this task establishes a new TUI detail contract.
* Task changes are committed before finish-work.

## Out of Scope

* Run compare view.
* Search and filter entry.
* Detail previous/next persisted-run stepping.
* Projection lens controls in the TUI.
* Live simulation monitoring.
* Profile browsing or profile YAML loading.
* Hongmeng actor orchestration or Nuwa simulation sandbox.
* TUI write actions, daemon control, run queueing, or feed fetching.

## Technical Approach

* Extend `TuiView` with a detail view.
* Keep `run_history_browser` as the public TUI entrypoint, but carry enough storage context in `TuiState` to load selected run detail on demand.
* Use `get_run_summary` with default `ScoredEventFilters`, default `EventGroupFilters`, and `only_matching_interventions=false` for the first detail slice.
* Add small formatter/state-mapping helpers in `src/tui.rs` so detail rendering can be unit-tested without terminal setup.
* Keep detail rendering conservative and textual, matching the existing dashboard/history implementation style.

## Decision (ADR-lite)

**Context**: Dashboard and history now exist in the Rust TUI, while simulation/profile data contracts remain deferred. Existing storage already provides the `history-show` detail read model.

**Decision**: Implement single-run detail before compare or live runtime controls.

**Consequences**: The TUI moves toward the documented read-only persisted-run browser contract without inventing new storage semantics. Projection lenses, compare, and previous/next persisted navigation remain explicit later slices.

## Confirmed Scope

* User selected Option 1: detail-only MVP.
* Include: Enter from history opens the selected run detail; Esc or `h` returns to history.
* Exclude: previous/next persisted-run stepping, compare view, filters/search, and projection lens controls.

## Technical Notes

* Inspected `.trellis/spec/backend/development-plan.md`: current migration state and Milestone 4 TUI direction.
* Inspected archived `05-14-continue-dev-plan/prd.md`: dashboard was the previous selected continuation slice.
* Inspected `src/tui.rs`: current dashboard/history implementation and tests.
* Inspected `.trellis/spec/backend/contracts/tui-contract.md`: read-only TUI contract and recommended later implementation order.
* Inspected `src/storage.rs`: `get_run_summary` and persisted-run navigation helpers already exist.
