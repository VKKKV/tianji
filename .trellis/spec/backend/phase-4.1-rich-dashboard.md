# Phase 4.1: Rich Dashboard

> Part of plan.md §5 Phase 4 TUI Completion
> Target: enhance DashboardState to show per-field stats and top events
> Status: implemented (Phase 4.1)

## Goal

Replace the current text-only Dashboard with a structured view showing:
1. Run metadata (latest run #, mode, time)
2. Field breakdown — per dominant_field, count + avg impact
3. Top events — top 5 scored events by impact_score
4. Delta info — alert tier, summary, direction (keep existing)

## Data Source

The TUI already has `sqlite_path` in `TuiState`. Use it to call
`get_run_summary(sqlite_path, latest_run_id, &ScoredEventFilters, false, &EventGroupFilters)`
from the latest run. Extract `scored_events` array from the returned JSON.

## DashboardState Changes

```rust
pub struct DashboardState {
    // Run metadata (keep)
    pub latest_run_id: String,
    pub latest_generated_at: String,
    pub latest_mode: String,
    pub headline: String,

    // Field breakdown — NEW
    pub field_summary: Vec<FieldStat>,       // per dominant_field
    pub total_scored_events: usize,

    // Top events — NEW
    pub top_events: Vec<TopEvent>,

    // Delta (keep, slightly refactored)
    pub alert_tier: String,
    pub delta_summary: String,
    pub delta_direction: String,

    // Removed: dominant_field, risk_level, top_divergence_score, baseline_status, worldline_status
    // (dominant_field/risk_level shown in run metadata line; baseline/worldline deferred)
}

pub struct FieldStat {
    pub field: String,        // "conflict" / "diplomacy" / "economy" / "technology"
    pub count: usize,
    pub avg_impact: f64,
}

pub struct TopEvent {
    pub title: String,
    pub impact_score: f64,
    pub dominant_field: String,
}
```

## Data Extraction

From `get_run_summary` returned JSON:
- `scored_events[].dominant_field` → group by, count, avg `impact_score`
- `scored_events[]` sorted by `impact_score` desc → take top 5 for TopEvent
- Keep existing: `headline`, metadata from history rows

Construction: change `DashboardState::from_history_and_memory` to
`DashboardState::from_run_summary(rows, memory, run_summary_json)` where
`run_summary_json` is the JSON from `get_run_summary` for the latest run.

## Rendering Changes

Replace `format_dashboard` plain text with ratatui `Paragraph` + styled `Span`s:

```
┌─ TianJi Dashboard ───────────────────────────────────┐
│                                                        │
│  Run #42 · fixture · 2026-05-15 13:22                  │
│  Headline: US carrier group enters South China Sea     │
│                                                        │
│  Field Summary                                         │
│    conflict    12 events  avg impact 15.2              │
│    diplomacy    5 events  avg impact  9.1              │
│    technology   3 events  avg impact 18.7              │
│    economy      1 event   avg impact  4.3              │
│                                                        │
│  Top Events                                            │
│    #1  US carrier group enters SCS       Im:18.2  conflict │
│    #2  Iran nuclear talks resume         Im:12.1  diplomacy│
│    #3  EU chip export framework           Im:10.7  technology│
│                                                        │
│  Delta · Priority · 5 total / 2 critical / 1 new       │
│  Direction: RiskOn                                     │
│                                                        │
└────────────────────────────────────────────────────────┘
```

Use Kanagawa colors:
- Field labels: `KANAGAWA.label` (blue)
- Normal values: `KANAGAWA.fg`
- Impact scores: green if > 10, yellow if > 5
- Alert tier: peach for Flash, yellow for Priority, default for Routine

## Caller Change (TuiState::new / new_with_storage)

Now takes `Option<serde_json::Value>` for the latest run summary JSON.
If None (no runs, no sqlite), show empty dashboard.

In `run_tui`, after loading history rows, also load the latest run summary:
```rust
let latest_summary = if let Some(ref db) = sqlite_path {
    get_latest_run_id(db).ok().flatten()
        .and_then(|id| get_run_summary(db, id, &Default::default(), false, &Default::default()).ok())
        .flatten()
} else {
    None
};
```

## Tests

- Unit: `FieldStat` / `TopEvent` extraction from sample JSON
- Unit: DashboardState construction from sample run summary
- Integration: `format_dashboard` output sanity check

## Files Changed

- `src/tui.rs` — DashboardState, constructors, rendering, caller
- No new files (submodule split deferred to 4.5)

## Verification

- `cargo build` zero error
- `cargo test` all pass
- `cargo clippy -- -D warnings` clean
- Run: `cargo run -- tui --sqlite-path <path>` → dashboard shows fields + top events
