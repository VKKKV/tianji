# Phase 4.4: Ctrl+d/u Page Scroll

> Part of plan.md §5.4 Phase 4 TUI Completion
> Target: distinguish Ctrl+d from 'd', add Ctrl+d/u half-page scroll
> Status: implemented (Phase 4.4)

## Goal

Fix the key conflict where 'd' is Dashboard shortcut but Ctrl+d should be
half-page scroll down (Vim convention). Add Ctrl+u for half-page scroll up.

## Current Problem

`handle_key_code(state, KeyCode)` only receives KeyCode, losing modifier info.
`KeyCode::Char('d')` with `KeyModifiers::CONTROL` (Ctrl+d) is indistinguishable
from plain `KeyCode::Char('d')`.

## Changes

### 1. Event loop (line ~710)

Change from:
```rust
if !handle_key_code(&mut state, key.code) {
```
to:
```rust
if !handle_key(&mut state, &key) {
```

### 2. Function signature

```rust
fn handle_key(state: &mut TuiState, key: &crossterm::event::KeyEvent) -> bool
```

### 3. Ctrl+d / Ctrl+u handling

Add at top of match (before existing Char/d/D/1 dashboard handler):
```rust
// Ctrl+d / Ctrl+u — half-page scroll in History view
if key.modifiers.contains(KeyModifiers::CONTROL) {
    match key.code {
        KeyCode::Char('d') => {
            if state.view == TuiView::History {
                let page = state.rows.len().max(1) / 2;
                state.selected = (state.selected + page).min(state.rows.len().saturating_sub(1));
            }
            return true;
        }
        KeyCode::Char('u') => {
            if state.view == TuiView::History {
                let page = state.rows.len().max(1) / 2;
                state.selected = state.selected.saturating_sub(page);
            }
            return true;
        }
        _ => {}
    }
}
```

Then the rest of the match stays the same but operates on `key.code`.

### 4. Import

Add `KeyModifiers` to crossterm imports.

### 5. Test updates

All existing tests that call `handle_key_code(state, KeyCode::Char(...))` need to
be updated to create `KeyEvent::new(KeyCode::Char(...), KeyModifiers::NONE)`.
Add new tests for Ctrl+d and Ctrl+u page scroll.

A helper for tests:
```rust
fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}
fn ctrl_key(c: char) -> KeyEvent {
    KeyEvent::new(KeyCode::Char(c), KeyModifiers::CONTROL)
}
```

## Files Changed

- `src/tui.rs` — function signature, event loop, key handling, tests

## Verification

- `cargo build` zero error
- `cargo test` all pass
- `cargo clippy -- -D warnings` clean
