# Continue TUI Development From Plan

## Goal

Continue TianJi development from the current post-dashboard/post-detail TUI state by adding the next plan-aligned Rust TUI slice: a read-only run compare view backed by existing storage read models.

## What I Already Know

* `plan.md` is the authoritative architecture document for the Rust rewrite and TUI design.
* `.trellis/spec/backend/development-plan.md` marks Milestone 4 TUI as the active deferred area after M1-M3/M3.5 completion.
* The Rust TUI now has dashboard, history, and single-run detail views.
* `.trellis/spec/backend/contracts/tui-contract.md` recommends extending read-only persisted-run browsing in this order: history list, single-run detail, run compare, then live/runtime concerns.
* `src/storage.rs` already exposes `compare_runs(sqlite_path, left_run_id, right_run_id, scored_filters, only_matching_interventions, group_filters)` for the `history-compare` read surface.
* `src/tui.rs` already carries selected history row state and storage path context for detail loading.

## Candidate Slices

### Option A — TUI Run Compare MVP (Recommended)

Add a read-only compare view using the selected history row plus the next selected row as a staged pair, backed by existing `compare_runs` storage semantics.

* Pros: follows the documented TUI implementation order, reuses existing storage/CLI compare semantics, and completes the persisted-run browser loop before live/runtime work.
* Cons: introduces pair-selection state and one more rendering path.

### Option B — Detail Previous/Next Navigation

Extend detail view with storage-backed previous/next run stepping.

* Pros: improves detail ergonomics and aligns with the broader contract.
* Cons: lower priority than compare in the documented later implementation order; still leaves compare absent.

### Option C — TUI Search/Filter Skeleton

Add slash-search/filter scaffolding for history/detail.

* Pros: visible UX improvement.
* Cons: risks inventing TUI-only filtering before compare and projection controls are settled.

## Recommended MVP

Implement Option A: a minimal read-only compare view.

## Requirements

* Keep the TUI read-only.
* Preserve existing dashboard, history, and detail behavior and keybindings.
* Add a compare view reachable from the history view.
* Use the minimal key path: `c` stages the selected history row as the left run, then `Enter` compares the staged left run against the currently selected right run.
* Source compare payloads from existing storage `compare_runs` semantics rather than duplicating diff logic in the TUI.
* Use default scored-event filters, default event-group filters, and `only_matching_interventions=false` for the MVP.
* Provide visible staged-pair feedback in history before compare activates.
* Render compare output without requiring an interactive terminal in tests.
* Degrade stable missing/error states without panics when a compare pair cannot be loaded.
* Do not add search/filter entry, projection lens controls, simulation, profile, daemon-control, queueing, feed fetching, or storage mutation in this task.

## Acceptance Criteria

* [ ] Existing `tianji tui --sqlite-path ...` still opens with the dashboard when persisted runs exist.
* [ ] Existing dashboard/history/detail behavior still works.
* [ ] From history, operators can stage one run and compare it against another selected run without leaving the TUI.
* [ ] The MVP key path is `c` to stage left and `Enter` to compare staged left vs selected right.
* [ ] Compare uses `compare_runs` with default filters and `only_matching_interventions=false`.
* [ ] Compare view surfaces stable `history-compare` fields: left/right run IDs, side summaries, and diff fields where available.
* [ ] Missing or invalid compare pairs render stable placeholder text instead of panicking.
* [ ] Unit tests cover compare state mapping, formatting, staged-pair key handling, and preserved dashboard/history/detail navigation without launching a terminal.
* [ ] `cargo test`, `cargo fmt --check`, and `cargo clippy -- -D warnings` pass.

## Definition of Done

* Tests added or updated for the new compare view behavior.
* Rust formatting, tests, and clippy pass.
* Specs updated if this task establishes a new TUI compare contract.
* Task changes are committed before finish-work.

## Out of Scope

* Search and filter entry.
* Projection lens controls in the TUI.
* Detail previous/next persisted-run stepping.
* Compare previous/next target stepping beyond the staged pair MVP.
* Live simulation monitoring.
* Profile browsing or profile YAML loading.
* Hongmeng actor orchestration or Nuwa simulation sandbox.
* TUI write actions, daemon control, run queueing, or feed fetching.

## Technical Approach

* Extend `TuiView` with a compare view if not already present.
* Add compare state to `TuiState`, including staged left run and loaded compare result/error placeholder.
* Add the minimal history key path: `c` stages the selected run as left, and `Enter` opens compare with the current selected run as right when a left run is staged.
* Use `compare_runs` with default filters and `only_matching_interventions=false`.
* Add formatter/state-mapping helpers in `src/tui.rs` so compare rendering can be unit-tested without terminal setup.
* Keep compare rendering conservative and textual, matching existing dashboard/detail style.

## Decision (ADR-lite)

**Context**: Dashboard and single-run detail now exist in the Rust TUI. Simulation/profile data contracts remain deferred, while existing storage already provides the `history-compare` read model.

**Decision**: Implement read-only staged-pair compare before live runtime controls or search/filter scaffolding.

**Consequences**: The TUI advances toward the documented persisted-run browser contract without inventing new storage semantics. Projection lenses and storage-backed compare-target stepping remain explicit later slices.

## Open Questions

* Resolved: user selected Option 1, `c` stages and `Enter` compares.

## Confirmed Scope

* User selected Option 1: `c` stages the selected left run, then `Enter` compares against the currently selected right run.
* Include visible staged-pair feedback in history and stable compare placeholders/errors.
* Exclude adjacent-run auto-compare, search/filter entry, projection lens controls, and previous/next compare-target stepping.

## Technical Notes

* Inspected `.trellis/spec/backend/development-plan.md`: current migration state and Milestone 4 TUI direction.
* Inspected `.trellis/spec/backend/contracts/tui-contract.md`: read-only TUI contract, detail contract, compare workflow, and recommended later implementation order.
* Inspected `plan.md` §9: TUI design, view list, and keybinding intent.
* Inspected `src/tui.rs`: current dashboard/history/detail implementation and tests.
* Inspected `src/storage.rs`: `compare_runs` and persisted compare result fields already exist.
