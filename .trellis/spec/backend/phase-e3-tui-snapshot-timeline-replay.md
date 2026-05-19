# Phase E3 — TUI Snapshot Timeline Replay

## Goal

Add deterministic timeline replay controls to the simulation TUI so users can scrub through simulation snapshots with arrow keys and inspect field changes per tick/frame.

## Scope

This phase should be small and testable:

- Extend `SimulationViewState` / `SimulationState` with replay cursor support.
- Derive replay frames from existing simulation state where possible.
- Allow Left/Right or h/l keys in simulation view to move the cursor backward/forward.
- Render the replay cursor and selected frame in simulation view.
- Preserve existing pruning controls and Dashboard/History behavior.

## Contract

Replay state:

- cursor starts at the latest frame/tick when a simulation is shown
- cursor is clamped to `[0, frame_count - 1]`
- empty simulation has cursor `0`
- changing simulation should reset cursor to latest

Keyboard:

- `Left` / `h`: previous frame in simulation view
- `Right` / `l`: next frame in simulation view
- existing `Esc`, `q`, pruning keys keep working

Rendering:

- simulation panel should show a timeline hint like `frame X/Y` or `tick A/B`
- event log/fields can remain current-state for this phase, but selected frame metadata must be visible and tested

## Tests

Add focused tests for:

- new simulation starts at latest replay frame
- previous/next replay cursor clamps at bounds
- simulation key handler changes replay cursor
- rendering text contains timeline position/hint

## Verification

Run:

```bash
cargo fmt
cargo test --quiet tui
cargo test --quiet
cargo clippy -- -D warnings
git diff --check
```
