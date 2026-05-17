# Phase 5.3: Interactive Pruning

> Spec: `.trellis/spec/backend/phase-5.3-interactive-pruning.md`
> Last: plan.md §5 Phase 5.3

## Summary

Bridge async simulation with sync TUI event loop via mpsc channels.
Enable user to inspect running simulation state and prune branches from TUI.

## Requirements

1. Add `BranchSummary` struct to `src/nuwa/outcome.rs` (index, probability, divergence, event_count)
2. Add to `SimulationState`: `branches: Vec<BranchSummary>`
3. Add to `TuiState`: `sim_rx` (mpsc receiver), `pending_prune_tx` (oneshot sender), `prune_mode: bool`, `prune_selected: Vec<usize>`
4. Add `--interactive` flag to CLI `tui --simulate` subcommand
5. In `run_history_browser` (tui/mod.rs): spawn simulation via tokio::spawn when --interactive, store channel receiver in TuiState
6. In TUI event loop: call `sim_rx.try_recv()` each iteration, update simulation state
7. When `pending_prune_tx` is Some, enter prune mode — show branching list with checkboxes
8. Prune mode keys: Space=toggle, Enter=confirm prune, c=continue, Esc=cancel
9. In `nuwa/forward.rs`: accept optional `tx: UnboundedSender<SimulationState>` + `pruning_interval`
10. At checkpoint ticks, send SimulationState snapshot; if pruning_interval hit, create oneshot and await decision

## Minimal Approach

Use `try_recv()` on UnboundedReceiver in the synchronous TUI loop — non-blocking.
For prune decisions: simulation sends (SimulationState, oneshot::Sender) through the same channel,
TUI detects it as a pruning request and responds.

## Files Changed

- `src/nuwa/outcome.rs` — BranchSummary
- `src/tui/state.rs` — TuiState pruning fields
- `src/tui/mod.rs` — spawn simulation, channel bridge, event loop polling
- `src/tui/simulation.rs` — render prune mode UI
- `src/main.rs` — --interactive flag
- `src/nuwa/forward.rs` — checkpoint yielding

## Verification

- `cargo build` zero error
- `cargo test` all pass
- `cargo clippy -- -D warnings` clean
