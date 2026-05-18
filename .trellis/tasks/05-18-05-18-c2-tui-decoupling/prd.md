# PRD — Phase C2: TUI View State Decoupling

> Priority: C (medium risk) | Spec: .trellis/spec/backend/phase-c2-tui-decoupling.md
> Pure refactor — zero behavioral change expected

## Goal

Replace monolithic `TuiState` (15+ fields) with `enum ViewState` dispatch.
Each view owns its state. Shared fields stay on TuiState.

## Steps

### Step 1 — Define ViewState enum + per-view structs

File: `src/tui/state.rs`

```rust
enum ViewState {
    Dashboard(DashboardState),
    History(HistoryViewState),
    Detail(DetailState),
    Compare(CompareState),
    Simulation(SimulationViewState),
}

struct HistoryViewState {
    all_rows: Vec<HistoryRow>,
    staged_left_run_id: Option<i64>,
    search_query: String,
    search_active: bool,
    pending_g: bool,
}

struct SimulationViewState {
    sim_state: Option<SimulationState>,
    prune_mode: bool,
    prune_selected: Vec<usize>,
    pending_sim_rx: Option<tokio::sync::mpsc::Receiver<SimUpdate>>,
    pending_prune_tx: Option<tokio::sync::oneshot::Sender<PruningDecision>>,
}
```

### Step 2 — Slim down TuiState

Remove from TuiState: `dashboard`, `detail`, `compare`, `simulation`, `staged_left_run_id`,
`pending_g`, `search_query`, `search_active`, `all_rows`, `prune_mode`, `prune_selected`,
`pending_sim_rx`, `pending_prune_tx`.

Move into `ViewState` variants. Keep `sqlite_path`, `rows`, `selected`, `glyphs`, `pending_loading`.

### Step 3 — Adapt TuiState constructor

`new()` creates `ViewState::Dashboard(dashboard)`. `new_with_storage()` same.
`show_dashboard/show_history/show_detail/show_compare/show_simulation` set `self.view` variant.

### Step 4 — Move view methods

`open_selected_detail`, `open_selected_compare`, `stage_selected_for_compare` →
move to `impl HistoryViewState`. Called from handle_key via destructuring.

### Step 5 — Per-view key handlers

File: `src/tui/mod.rs`

Extract from monolithic `handle_key`:
- `handle_dashboard_key(state: &mut TuiState, key: &KeyEvent) -> bool`
- `handle_history_key(state: &mut TuiState, hv: &mut HistoryViewState, key: &KeyEvent) -> bool`
- `handle_detail_compare_key(state: &mut TuiState, key: &KeyEvent) -> bool`
- `handle_simulation_key(state: &mut TuiState, sv: &mut SimulationViewState, key: &KeyEvent) -> bool`

### Step 6 — Update event loop

File: `src/tui/mod.rs`

Update simulation poll to destructure `ViewState::Simulation(sv)`.
Update loading poll — stays on TuiState, no change.
Update key dispatch to route to per-view handlers.

### Step 7 — Update render

File: `src/tui/render.rs`

Change match arms from `state.view` to destructure:
```rust
ViewState::Dashboard(d) => render_dashboard(...),
ViewState::History(h) => render_history(...),
ViewState::Detail(_) => render_detail(state.detail.as_ref()),
...
```

### Step 8 — Update history render

File: `src/tui/history.rs`

`render_history` now receives `&HistoryViewState` instead of `&TuiState`.

## Key Files

| Action | File | Lines changed |
|--------|------|---------------|
| Define ViewState + view structs | src/tui/state.rs | ~100 |
| Slim TuiState | src/tui/state.rs | ~80 |
| Per-view key handlers | src/tui/mod.rs | ~150 |
| Event loop update | src/tui/mod.rs | ~30 |
| Render dispatch | src/tui/render.rs | ~10 |
| History render adapt | src/tui/history.rs | ~10 |

## Commands

```bash
cargo build && cargo test && cargo clippy -- -D warnings && cargo fmt
# smoke: cargo run -- tui --db runs/tianji.sqlite3
# all 5 views navigable, identical behavior
```
