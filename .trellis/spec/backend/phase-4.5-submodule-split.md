# Phase 4.5: TUI Submodule Split

> Part of plan.md §5.5 Phase 4 TUI Completion
> Target: split 2676-line src/tui.rs into src/tui/{mod,dashboard,history,detail,compare,state,render}.rs
> Status: implemented (Phase 4.5)

## Goal

Split the monolithic `src/tui.rs` into submodules without changing any behavior.
All existing tests must pass. Pure code organization.

## Target Structure

```
src/tui/
├── mod.rs          # re-exports, run_tui(), handle_key()
├── state.rs        # TuiState, TuiView, DashboardState, DetailState, CompareState
│                   # HistoryRow, FieldStat, TopEvent, GlyphSet
│                   # all impl blocks for these types
├── dashboard.rs    # DashboardState constructors, format_dashboard, render_dashboard
├── history.rs      # HistoryRow, render_history, load_history_rows
├── detail.rs       # DetailState constructors, format_detail, render_detail, load_detail_state
├── compare.rs      # CompareState constructors, format_compare, render_compare, load_compare_state
├── render.rs       # render(), render_status_bar(), format_alert_tier()
└── theme.rs        # KANAGAWA const, Theme struct
```

## Rules

1. Zero behavior change — only move code between files
2. All `pub` items that were previously accessible from `tui.rs` must remain accessible
3. `mod.rs` re-exports the public API: `run_tui`, `TuiState`, `TuiView`, `DashboardState`, etc.
4. `handle_key` stays in `mod.rs` (it references all view types, cross-cutting)
5. `render()` and `render_status_bar()` go to `render.rs`
6. Helper functions used by only one module stay with that module
7. `use` statements: each submodule imports only what it needs
8. Tests stay in their respective module files (e.g., dashboard tests in dashboard.rs)
9. No new dependencies

## Verification

- `cargo build` zero error
- `cargo test` all 133+ pass
- `cargo clippy -- -D warnings` clean
- `cargo fmt --check` clean
- `src/tui.rs` deleted, `src/tui/mod.rs` created
- All tests still runnable via `cargo test`
