# PRD — Phase E3: TUI Snapshot Timeline Replay

> Priority: E3 | Spec: `.trellis/spec/backend/phase-e3-tui-snapshot-timeline-replay.md`

## Goal

Add keyboard-driven snapshot/timeline replay affordances to TianJi's simulation TUI.

## Requirements

1. State:
   - Add replay cursor/frame-count state to simulation TUI state.
   - Cursor starts on latest frame when showing simulation.
   - Cursor clamps at bounds.

2. Controls:
   - In simulation view, `Left` / `h` move to previous replay frame.
   - `Right` / `l` move to next replay frame.
   - Do not break `Esc`, `q`, history/dashboard navigation, or prune mode controls.

3. Rendering:
   - Simulation render path exposes selected frame/tick position in visible text.
   - Status/help hints mention timeline controls when simulation view is active.

4. Tests:
   - replay cursor init/bounds
   - key handler changes cursor
   - render output includes timeline hint

## Allowed Files

- `src/tui/state.rs`
- `src/tui/mod.rs`
- `src/tui/render.rs`
- `src/tui/simulation.rs`

## Verification

Run:

```bash
cargo fmt
cargo test --quiet tui
cargo test --quiet
cargo clippy -- -D warnings
git diff --check
```
