# Continue TianJi Rust Development

## Goal

Continue the Rust rewrite from the current completed Milestone 3 state by implementing the first Rust TUI slice, while preserving Python oracle parity and the Trellis migration guardrails.

## What I Already Know

* Root `plan.md` is the authoritative architecture document for the Rust rewrite.
* Rust Milestone 1A, 1B, 2, and 3 are marked complete: feed/normalization, scoring/grouping/backtracking, storage/history, daemon/API/web UI.
* `.trellis/spec/backend/development-plan.md` lists Milestone 4 TUI as the next deferred milestone.
* `plan.md` also notes Milestone 3C bounded schedule as postponed from the M3 runtime slice.
* `Cargo.toml` currently has no `ratatui` or `crossterm` dependency.
* `src/main.rs` currently has `run`, `history`, `history-show`, `history-compare`, `daemon`, and `webui` commands, with no Rust `tui` command yet.
* No Rust integration tests were found under `tests/**/*.rs`; current Rust tests appear to be unit tests in `src/`.
* User selected Candidate A: first Rust TUI slice.
* `.trellis/spec/backend/contracts/tui-contract.md` says the Rust target is ratatui + Kanagawa per `plan.md` §9, while preserving read-only persisted-run navigation semantics from the Python TUI.
* Python TUI entrypoint is `python3 -m tianji tui --sqlite-path ...`; it loads persisted run rows and exits cleanly with `No persisted runs are available for the TUI browser.` when empty.

## Assumptions (Temporary)

* The first Rust TUI slice should stay read-only and storage-backed.
* The first Rust TUI slice should not attempt the complete dashboard/history/simulation/profiles surface at once.

## Open Questions

* None.

## Requirements (Evolving)

* Preserve existing Rust CLI/runtime behavior.
* Keep Python oracle code intact.
* Add only dependencies needed by the selected milestone.
* Follow `plan.md` and backend spec guardrails.
* Add a Rust `tui` command backed by persisted SQLite run reads.
* Use ratatui + crossterm with the Kanagawa palette from `plan.md` §9.
* Keep the TUI read-only; running pipelines stays with CLI/daemon surfaces.
* MVP scope is history browser only.
* Show persisted run rows from SQLite using existing storage read semantics.
* Support empty-state handling when no persisted runs are available.
* Support keyboard navigation for the run list with `j`/`k` and arrow keys.
* Support `q` quit.

## Acceptance Criteria (Evolving)

* [ ] `tianji tui --sqlite-path <path>` exists and has clear read-only behavior.
* [ ] Empty SQLite/history state exits cleanly or renders a clear empty-state message.
* [ ] History list renders persisted run triage fields without changing storage semantics.
* [ ] `j`/`k` and arrow keys move selection within available rows.
* [ ] `q` quits without mutating stored data.
* [ ] Relevant Rust tests are added or updated.
* [ ] `cargo fmt --check` passes.
* [ ] `cargo test` passes.
* [ ] `cargo clippy -- -D warnings` passes.

## Definition of Done

* Tests added/updated where appropriate.
* Rust lint/type/test checks pass.
* Docs/spec notes updated if behavior or milestone status changes.
* Python oracle remains unchanged unless explicitly required.

## Out of Scope (Evolving)

* Full Hongmeng orchestration and Nuwa simulation.
* Python retirement/cleanup.
* Any claim that unimplemented architecture is shipped.
* Simulation monitoring, profile browser, live run execution, or daemon control from the TUI unless explicitly included later.
* Run detail view.
* Run compare staging/view.
* TUI filtering, text search, and lens controls.

## Technical Notes

* Inspected: `plan.md`, `.trellis/spec/backend/index.md`, `.trellis/spec/backend/development-plan.md`, `.trellis/spec/backend/contracts/tui-contract.md`, `Cargo.toml`, `src/main.rs`, `tianji/tui.py`.
* Candidate A: first Rust TUI slice per `plan.md` §9 and development-plan Milestone 4.
* Candidate B: bounded daemon schedule (`daemon schedule --every-seconds N --count M`) postponed from Milestone 3C.

## Decision (ADR-lite)

**Context**: After Milestone 3, the plan marks Rust TUI as the next deferred milestone, and the Rust CLI has no `tui` command yet.

**Decision**: Implement the first Rust TUI slice.

**Consequences**: This introduces terminal UI dependencies and a new read-only CLI surface. Scope must stay small to avoid pulling in full simulation/profile/operator workflow before the persisted-run browser is stable.

## MVP Scope Decision

**Context**: The Python TUI can browse list/detail/compare, but the Rust TUI has no CLI surface or dependencies yet.

**Decision**: Implement only the history browser for this task.

**Consequences**: The first slice validates dependency wiring, terminal lifecycle, Kanagawa styling, and persisted-run list rendering. Detail and compare remain explicit follow-up tasks.
