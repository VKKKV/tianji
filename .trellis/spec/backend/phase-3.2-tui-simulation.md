# Phase 3.2: TUI Simulation View

> Part of plan.md §5.2 — connects Nuwa sandbox to TUI display
> Target: add Simulation view to TUI showing live sim state
> Status: spec

## Goal

Add a new TUI view that shows simulation progress when running `predict` or
`backtrack`. No real-time interaction yet — display-only for MVP. Manual
pruning interaction (pause → choose → continue) deferred.

## Changes

### 1. TuiView + SimulationState

```rust
// src/tui/state.rs — add to TuiView enum
pub enum TuiView {
    Dashboard,
    History,
    Detail,
    Compare,
    Simulation,    // NEW
}

pub struct SimulationState {
    pub mode: String,                  // "forward" | "backward"
    pub target: String,                // field or goal description
    pub horizon: u64,                  // 0 if backward
    pub tick: u64,
    pub total_ticks: u64,
    pub status: String,                // "running" | "converged" | "failed"
    pub field_values: Vec<SimField>,   // current field state
    pub agent_statuses: Vec<SimAgent>, // per-agent
    pub event_log: Vec<String>,        // last N events
}

pub struct SimField {
    pub region: String,
    pub domain: String,
    pub value: f64,
    pub delta: f64,         // change from start
}

pub struct SimAgent {
    pub actor_id: String,
    pub status: String,     // "idle" | "thinking" | "done"
    pub last_action: String,
}
```

### 2. TuiState changes

```rust
pub struct TuiState {
    // ... existing fields ...
    pub simulation: Option<SimulationState>,
}
```

### 3. Key handling

Add `'s'` or `'3'` key to switch to Simulation view (only if simulation is active).

### 4. Rendering — render_simulation()

```
┌─ Simulation ──────────────────────────────────────┐
│ mode: forward  field: east-asia.conflict  tick 3/30│
│ status: running                                     │
├─ Worldline ────────────────────────────────────────┤
│ east-asia.conflict   0.84  ↑0.12                    │
│ global.trade_volume  0.55  ↓0.08                    │
│ europe.stability     0.58  —                        │
├─ Agents ───────────────────────────────────────────┤
│ China      thinking   (naval exercise)              │
│ USA        done       (diplomatic protest)          │
│ Russia     idle                                     │
├─ Events ───────────────────────────────────────────┤
│ tick 3: conflict increased by 0.15                  │
│ tick 2: diplomacy decreased by 0.05                 │
│ tick 1: conflict increased by 0.12                  │
└─────────────────────────────────────────────────────┘
```

### 5. Connection point

In `run_history_browser`, after loading data, optionally run a simulation
and populate `SimulationState`. For MVP: allow `--simulate` CLI flag on
`tianji tui` that runs a forward sim with stub agents and shows results.

```
tianji tui --sqlite-path runs/tianji.sqlite3 --simulate east-asia.conflict:30
```

Add to main.rs Cli::Tui variant:
```rust
Cli::Tui {
    sqlite_path: String,
    limit: usize,
    simulate: Option<String>,  // "field:horizon" e.g. "east-asia.conflict:30"
}
```

### 6. Simulation runner (in tui/mod.rs)

```rust
fn run_demo_simulation(field: &str, horizon: u64) -> SimulationState {
    // 1. Create stub worldline
    // 2. Load profiles from profiles/
    // 3. Create agents
    // 4. Run NuwaSandbox::run_forward()
    // 5. Convert outcome → SimulationState
}
```

## Files Changed

- `src/tui/state.rs` — add SimulationState, SimField, SimAgent, TuiView::Simulation
- `src/tui/mod.rs` — key handling, render dispatch, run_demo_simulation
- `src/tui/render.rs` — add render_simulation()
- `src/main.rs` — add --simulate flag to Tui command, pass to run_history_browser

## Tests

- Unit: SimulationState construction
- Unit: TUI view switch to Simulation
- Unit: render_simulation output contains field values

## Verification

- `cargo build` zero error
- `cargo test` all pass
- `cargo run -- tui --sqlite-path runs/tianji.sqlite3 --simulate east-asia.conflict:5`
