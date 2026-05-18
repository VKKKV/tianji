# Phase C2 — TUI View State Decoupling

> Status: spec | Risk: medium | Files: ~4 | Pure refactor, no behavior change

## Current State

`TuiState` (state.rs:164-183) is a monolithic struct with 15+ fields serving 5 views.
All views share the same struct, leading to:

- `Option<DetailState>` and `Option<CompareState>` that are never simultaneously Some
- `staged_left_run_id` only relevant to History view
- `prune_mode`/`prune_selected` only relevant to Simulation view
- `pending_sim_rx`/`pending_prune_tx` only relevant to Simulation view with interactive flag
- `search_query`/`search_active`/`all_rows` only relevant to History view

The `handle_key` function (mod.rs:429) is a single large match dispatching on `state.view`.

## Design — `enum ViewState`

```rust
enum ViewState {
    Dashboard(DashboardState),
    History(HistoryViewState),
    Detail(DetailState),
    Compare(CompareState),
    Simulation(SimulationViewState),
}
```

### Shared state stays on TuiState
- `rows: Vec<HistoryRow>` — shared by Dashboard/History
- `glyphs: &'static GlyphSet` — UI config
- `sqlite_path: Option<String>` — data source
- `selected: usize` — list cursor (History only, but small)

### View-specific state moves to ViewState variants

| View | Dedicated struct | Fields |
|------|-----------------|--------|
| Dashboard | (none, uses DashboardState directly) | — |
| History | `HistoryViewState` | `all_rows`, `staged_left_run_id`, `search_query`, `search_active`, `pending_g` |
| Detail | `DetailState` (existing) | — |
| Compare | `CompareState` (existing) | — |
| Simulation | `SimulationViewState` | `sim_state: SimulationState`, `prune_mode`, `prune_selected`, `pending_sim_rx`, `pending_prune_tx` |

### Key handler dispatch

Replace monolithic `handle_key` with per-view handlers:

```rust
fn handle_key(state: &mut TuiState, key: &KeyEvent) -> bool {
    match &mut state.view {
        ViewState::Dashboard(_) => handle_dashboard_key(state, key),
        ViewState::History(h) => handle_history_key(state, h, key),
        ViewState::Detail(_) | ViewState::Compare(_) => handle_detail_compare_key(state, key),
        ViewState::Simulation(s) => handle_simulation_key(state, s, key),
    }
}
```

### Render dispatch

Already clean (render.rs:29-47) — just change the match arms to destructure ViewState.

### Loading state

`pending_loading: Option<LoadingState>` stays on TuiState — it's transient and not view-specific.

## Files Changed

- `src/tui/state.rs` — ViewState enum, HistoryViewState, SimulationViewState
- `src/tui/mod.rs` — dispatch handle_key, update event loop
- `src/tui/render.rs` — destructure ViewState in match arms
- `src/tui/history.rs` — accept HistoryViewState instead of &TuiState

## Verification

```bash
cargo build && cargo test && cargo clippy -- -D warnings
# All 310 tests pass — pure refactor
# cargo run -- tui --db runs/tianji.sqlite3 unchanged behavior
```
