# Continue Development From Plan

## Goal

Continue TianJi development from the current post-M3/M3.5 state by selecting and implementing the next small, plan-aligned Rust slice without jumping ahead of available data contracts.

## What I Already Know

* `plan.md` is the authoritative architecture document.
* Current plan status says M1A+M1B+M2+M3A+M3B are complete, M4 TUI MVP is complete, and Crucix daemon auto-delta / alert-tier surfacing is complete.
* `.trellis/spec/backend/development-plan.md` marks Milestone 4 TUI as the next deferred area, with dashboard, history, simulation, and profile views.
* `src/tui.rs` currently implements a read-only persisted-run history browser with Kanagawa Dark styling and Vim-style movement.
* `plan.md` §9 says the current TUI MVP is `src/tui.rs` and explicitly defers Dashboard / simulation / profiles to Phase 4 full implementation.
* Simulation and profile views depend on Hongmeng/Nuwa/profile data that is still deferred or only planned.
* The safest next slice should reuse existing persisted run/delta data instead of inventing new simulation/profile storage contracts.

## Candidate Slices

### Option A — TUI Dashboard From Existing Run/Delta Data (Recommended)

Add a dashboard/home view to the Rust TUI using data already available from persisted run history and hot-memory delta state.

* Pros: directly matches plan §9 Dashboard, builds on existing storage/delta contracts, avoids Hongmeng/Nuwa dependency.
* Cons: dashboard fields must be scoped to available persisted data, not full future worldline baseline data.

### Option B — TUI Detail/Compare Parity

Extend the history browser with detail and compare panels backed by existing `history-show` / `history-compare` storage functions.

* Pros: aligns strongly with `tui-contract.md` persisted-run browser semantics.
* Cons: less aligned with the newly deferred Phase 4 full-view list in `plan.md`; more UI state complexity.

### Option C — TUI Placeholder View Shells

Add dashboard/simulation/profiles tabs as non-interactive placeholders.

* Pros: creates navigation skeleton quickly.
* Cons: low product value, risks UI scaffolding ahead of real data contracts.

## Recommended MVP

Implement Option A: a read-only TUI dashboard view using existing persisted run history and delta/hot-memory summaries.

## Decision (ADR-lite)

**Context**: The plan lists dashboard, history, simulation, and profile views as the full Phase 4 TUI direction. Only the history browser exists today, while Hongmeng/Nuwa/profile data contracts are still deferred.

**Decision**: Implement the dashboard view first, using only existing persisted run and delta/hot-memory data.

**Consequences**: The TUI gains a useful home/overview surface without adding write behavior or inventing future simulation/profile contracts. Some full-plan dashboard fields, such as baseline/worldline state, remain placeholders or are derived conservatively from existing read models until the proper backend data exists.

## Requirements (Evolving)

* Keep TUI read-only.
* Preserve existing history browser behavior and keybindings.
* Add a dashboard view that can be rendered/tested without requiring an interactive terminal.
* Source dashboard data only from existing storage/delta contracts.
* Provide a keyboard path between dashboard and history views using existing Vim-style conventions where practical.
* Use stable placeholder text for unavailable future fields such as baseline/worldline when they are not represented in current persisted data.
* Do not add Hongmeng, Nuwa, profile, or simulation runtime dependencies in this task.

## Acceptance Criteria (Evolving)

* [ ] Existing `tianji tui --sqlite-path ...` still works for history browsing.
* [ ] Dashboard rendering surfaces latest run identity/time, dominant field/risk, top divergence, and recent delta/alert information when available.
* [ ] Operators can switch between dashboard and history without leaving the TUI.
* [ ] Empty or missing data degrades to stable placeholder text instead of panicking.
* [ ] Unit tests cover dashboard state mapping and formatting without launching a terminal.
* [ ] `cargo test`, `cargo fmt --check`, and `cargo clippy -- -D warnings` pass.

## Definition of Done

* Tests added/updated.
* Lint, format, and tests pass.
* Specs updated if a new TUI contract is established.
* Task committed before finish-work.

## Out of Scope

* Live simulation monitoring.
* Profile YAML loading or browsing.
* Hongmeng actor orchestration.
* Nuwa simulation sandbox.
* TUI write actions, daemon control, run queueing, or feed fetching.
* Full future worldline/baseline model if not already represented in current persisted data.

## Technical Notes

* Inspected `plan.md` §9: Dashboard / History / Simulation view sketches and current MVP status.
* Inspected `.trellis/spec/backend/development-plan.md`: Milestone 4 deferred TUI expansion list.
* Inspected `src/tui.rs`: current read-only history browser state, rendering, and tests.
* Inspected `.trellis/spec/backend/contracts/tui-contract.md`: read-only persisted-run semantics and Rust TUI MVP contract.
