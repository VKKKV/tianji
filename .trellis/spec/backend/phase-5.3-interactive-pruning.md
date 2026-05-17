# Phase 5.3: Human-in-the-loop Pruning

> Part of plan.md §5 Phase 5 Completion
> Target: interactive simulation with TUI pruning controls
> Status: in_progress

## Goal

Enable the user to pause a running simulation, inspect branches, and prune
unwanted worldlines from the TUI before resuming.

Currently simulations run to completion before the TUI starts. This phase adds
a channel-based bridge so the simulation can send state snapshots and wait for
pruning decisions from the TUI event loop.

## Architecture

```
┌──────────────┐    mpsc::unbounded    ┌────────────────┐
│ tokio thread │ ──── SimulationState ──→ │ TUI event loop │
│ (simulation) │                        │ (ratatui)      │
│              │ ←── PruningDecision ── │                │
└──────────────┘    oneshot channel     └────────────────┘
```

The simulation runs in a `tokio::spawn` task. At each checkpoint:
1. Simulation sends a `SimulationState` snapshot through an unbounded mpsc channel
2. If pruning is needed (user configured --interactive or every N ticks),
   simulation creates a `oneshot::channel` and awaits the response
3. TUI receives the snapshot, updates its state, renders
4. User presses 'p' to enter prune mode, selects branches to prune
5. TUI sends `PruningDecision` through the oneshot, simulation resumes

## Requirements

1. **TuiState** — add field:
   - `pruning_tx: Option<tokio::sync::mpsc::UnboundedSender<SimulationState>>`
   - `pruning_rx: Option<tokio::sync::oneshot::Sender<PruningDecision>>`
   - `prune_mode: bool` — whether TUI is in branching pruning mode
   - `prune_selected: Vec<usize>` — indices user has selected to prune

2. **SimulationState** — add field:
   - `branches: Vec<BranchSummary>` — worldline branches available for pruning
     where BranchSummary = { index, probability, divergence, event_count }

3. **run_demo_simulation** — change to accept pruning config:
   - `pruning_interval: Option<u64>` — pause every N ticks
   - Return a tokio JoinHandle + the mpsc receiver
   - Or: spawn internally and return SimulationHandle

4. **SimulationHandle** — new struct:
   ```rust
   pub struct SimulationHandle {
       pub join_handle: tokio::task::JoinHandle<Result<(), TianJiError>>,
       pub state_rx: tokio::sync::mpsc::UnboundedReceiver<SimulationState>,
       pub decision_tx: Option<tokio::sync::oneshot::Sender<PruningDecision>>,
   }
   ```

5. **TUI mod.rs** — `run_history_browser` changes:
   - If `--simulate` with `--interactive`:
     a. Spawn simulation via tokio::spawn
     b. Store the mpsc receiver and oneshot sender in TuiState
     c. In the TUI event loop, poll the mpsc receiver before rendering
     d. When pruning point reached, enter prune mode
     e. On user decision, send through oneshot, resume

6. **TUI simulation view** — add pruning UI:
   - When `prune_mode == true`, show branch list with checkboxes
   - Space to toggle selection
   - Enter to confirm -> send PruningDecision::Prune(selected)
   - 'c' to continue -> send PruningDecision::Continue
   - Esc to cancel -> send PruningDecision::Continue

7. **CLI** — add `--interactive` flag to `tianji tui --simulate`:
   - `#[arg(long)] interactive: bool`

## Files Changed

- `src/tui/state.rs` — SimulationState + TuiState pruning fields
- `src/tui/mod.rs` — spawn simulation task, channel bridge, event loop polling
- `src/tui/simulation.rs` — rendering prune mode UI
- `src/main.rs` — add --interactive CLI flag
- `src/nuwa/outcome.rs` — add BranchSummary
- `src/nuwa/forward.rs` — support checkpoint yielding (if configured)

## Verification

- `cargo build` zero error
- `cargo test` all pass
- `cargo clippy -- -D warnings` clean
