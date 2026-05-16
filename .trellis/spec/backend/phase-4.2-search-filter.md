# Phase 4.2: TUI Search/Filter

> Part of plan.md §5.4 Phase 4 TUI Completion
> Target: add `/` search to filter history list by text match
> Status: implemented (Phase 4.2)

## Goal

Add Vim-style `/` search in the History view. Press `/` to enter search mode,
type a query, press Enter to filter the history list by matching against
run metadata (dominant_field, headline, risk_level). Press Esc to clear.

## Behavior

1. Press `/` in History view → enter search mode
2. Type query string (shown in a small input bar at bottom)
3. Press Enter → filter `rows` to only those matching query
4. Empty query → restore full list
5. Press Esc → clear search, restore full list
6. Navigation (j/k) works on filtered subset
7. Detail/Compare open from filtered subset — run_id still correct

## Implementation

### TuiState changes

```rust
pub struct TuiState {
    // ... existing fields ...
    pub search_query: String,
    pub search_active: bool,
    all_rows: Vec<HistoryRow>,      // unfiltered master list
    // rows becomes the filtered view
}
```

On search submit:
```rust
fn apply_search(&mut self) {
    if self.search_query.is_empty() {
        self.rows = self.all_rows.clone();
    } else {
        let q = self.search_query.to_lowercase();
        self.rows = self.all_rows.iter()
            .filter(|row| {
                row.dominant_field.to_lowercase().contains(&q)
                || row.headline.to_lowercase().contains(&q)
                || row.risk_level.to_lowercase().contains(&q)
            })
            .cloned()
            .collect();
    }
    self.selected = 0;
    self.search_active = false;
}
```

### Key handling

Add to `handle_key_code`:
```rust
// When search is active
if state.search_active {
    match code {
        KeyCode::Esc => {
            state.search_active = false;
            state.search_query.clear();
        }
        KeyCode::Enter => {
            state.apply_search();
        }
        KeyCode::Backspace => {
            state.search_query.pop();
        }
        KeyCode::Char(c) => {
            state.search_query.push(c);
        }
        _ => {}
    }
    return true;
}

// In History view, `/` activates search
KeyCode::Char('/') if state.view == TuiView::History => {
    state.search_active = true;
    state.search_query.clear();
    true
}
```

### Rendering

When `search_active`, show a small bar at bottom of history panel:
```
/ <query>▊
```
Use Kanagawa colors: input text in fg, cursor indicator in label blue.

### Search result indicator

After filtering, show count in history panel title:
```
History [5/42]        ← 5 filtered of 42 total
```

## Files Changed

- `src/tui.rs` — TuiState, handle_key_code, render_history, constructors

## Tests

- Unit: search filter matches dominant_field (case insensitive)
- Unit: search filter matches headline (case insensitive)
- Unit: empty query restores full list
- Unit: no matches shows empty list with count [0/N]
- Unit: search only filters, doesn't mutate all_rows
- Existing history navigation tests pass on filtered subset

## Verification

- `cargo build` zero error
- `cargo test` all pass
- `cargo clippy -- -D warnings` clean
