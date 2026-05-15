# Phase 4.1: Rich Dashboard

> Spec: `.trellis/spec/backend/phase-4.1-rich-dashboard.md`

## Summary

Replace the current text-only Dashboard with a structured view showing per-field stats and top events, sourced from `get_run_summary` JSON.

## Requirements

1. Add `FieldStat` and `TopEvent` structs to `src/tui.rs`
2. Replace `DashboardState` fields: remove `dominant_field`, `risk_level`, `top_divergence_score`, `baseline_status`, `worldline_status`; add `field_summary: Vec<FieldStat>`, `total_scored_events: usize`, `top_events: Vec<TopEvent>`
3. Change constructor to `DashboardState::from_run_summary(rows, memory, run_summary_json: Option<serde_json::Value>)`
4. Extract field breakdown: group `scored_events[].dominant_field` → count + avg `impact_score`
5. Extract top 5 events by `impact_score` desc
6. Rewrite `render_dashboard` to use styled `Span`s with Kanagawa colors (field labels blue, impact green>10/yellow>5, alert tier peach/yellow/default)
7. Update `run_history_browser` to load latest run summary via `get_latest_run_id` + `get_run_summary`
8. Update `TuiState::new` / `new_with_storage` if needed
9. Update `format_dashboard` (used by CLI `history dashboard`) to match new fields
10. Update all tests

## Files Changed

- `src/tui.rs` — all changes in one file

## Verification

- `cargo build` zero error
- `cargo test` all pass
- `cargo clippy -- -D warnings` clean
