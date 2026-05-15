use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, List, ListItem, Paragraph};
use ratatui::Frame;

use super::state::{compact_timestamp, HistoryRow, TuiState};
use super::theme::KANAGAWA;

impl HistoryRow {
    pub fn from_json(value: &serde_json::Value) -> Self {
        Self {
            run_id: value.get("run_id").and_then(|v| v.as_i64()).unwrap_or(0),
            generated_at: value
                .get("generated_at")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            mode: value
                .get("mode")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            dominant_field: value
                .get("dominant_field")
                .and_then(|v| v.as_str())
                .unwrap_or("uncategorized")
                .to_string(),
            risk_level: value
                .get("risk_level")
                .and_then(|v| v.as_str())
                .unwrap_or("low")
                .to_string(),
            top_divergence_score: value.get("top_divergence_score").and_then(|v| v.as_f64()),
            headline: value
                .get("headline")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
        }
    }
}

pub fn format_history_row(row: &HistoryRow) -> String {
    let generated_at = compact_timestamp(&row.generated_at);
    let divergence = row
        .top_divergence_score
        .map(|value| format!("{value:.6}"))
        .unwrap_or_else(|| "-".to_string());
    format!(
        "#{:<4} {:<19} {:<8} {:<14} {:<6} {:>10}  {}",
        row.run_id,
        generated_at,
        row.mode,
        row.dominant_field,
        row.risk_level,
        divergence,
        row.headline
    )
}

pub fn render_history(frame: &mut Frame<'_>, area: Rect, state: &TuiState) {
    let (list_area, search_area) = if state.search_active {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(3), Constraint::Length(1)])
            .split(area);
        (chunks[0], Some(chunks[1]))
    } else {
        (area, None)
    };

    let list_items: Vec<ListItem<'_>> = state
        .rows
        .iter()
        .map(|row| ListItem::new(format_history_row(row)).style(super::render::base_style()))
        .collect();
    let list = List::new(list_items)
        .block(
            Block::bordered()
                .title(history_title(state))
                .border_style(Style::default().fg(KANAGAWA.border))
                .style(super::render::base_style().bg(KANAGAWA.panel_bg)),
        )
        .highlight_style(
            Style::default()
                .fg(KANAGAWA.title)
                .bg(KANAGAWA.border)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("> ");
    let mut list_state = state.list_state();
    frame.render_stateful_widget(list, list_area, &mut list_state);

    if let Some(search_area) = search_area {
        let search_bar = Paragraph::new(Line::from(vec![
            Span::styled("/ ", Style::default().fg(KANAGAWA.fg)),
            Span::styled(state.search_query.clone(), Style::default().fg(KANAGAWA.fg)),
            Span::styled("▊", Style::default().fg(KANAGAWA.label)),
        ]))
        .style(super::render::base_style().bg(KANAGAWA.panel_bg));
        frame.render_widget(search_bar, search_area);
    }
}

pub fn history_title(state: &TuiState) -> String {
    let filter_indicator = if state.rows.len() < state.all_rows.len() {
        format!(" [{}/{}]", state.rows.len(), state.all_rows.len())
    } else {
        String::new()
    };
    match state.staged_left_run_id {
        Some(run_id) => format!(" Run History · staged left #{run_id}{filter_indicator} "),
        None => format!(" Run History{filter_indicator} "),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::state::DashboardState;
    use crate::HotMemory;

    fn row(run_id: i64) -> HistoryRow {
        HistoryRow {
            run_id,
            generated_at: "1970-01-01T00:00:00+00:00".to_string(),
            mode: "fixture".to_string(),
            dominant_field: "technology".to_string(),
            risk_level: "high".to_string(),
            top_divergence_score: Some(20.73),
            headline: "headline".to_string(),
        }
    }

    fn dashboard() -> DashboardState {
        DashboardState::from_run_summary(&[row(1)], &HotMemory::default(), None)
    }

    fn history_state(rows: Vec<HistoryRow>) -> TuiState {
        let mut state = TuiState::new(rows, dashboard());
        state.show_history();
        state
    }

    #[test]
    fn history_row_maps_from_storage_payload() {
        let value = serde_json::json!({
            "run_id": 42,
            "generated_at": "1970-01-01T00:00:00+00:00",
            "mode": "fixture",
            "dominant_field": "technology",
            "risk_level": "high",
            "top_divergence_score": 20.73,
            "headline": "technology pressure rises"
        });

        let row = HistoryRow::from_json(&value);

        assert_eq!(row.run_id, 42);
        assert_eq!(row.mode, "fixture");
        assert_eq!(row.dominant_field, "technology");
        assert_eq!(row.top_divergence_score, Some(20.73));
    }

    #[test]
    fn history_row_format_includes_triage_fields() {
        let row = HistoryRow {
            run_id: 7,
            generated_at: "1970-01-01T00:00:00+00:00".to_string(),
            mode: "fixture".to_string(),
            dominant_field: "diplomacy".to_string(),
            risk_level: "low".to_string(),
            top_divergence_score: Some(1.25),
            headline: "talks resume".to_string(),
        };

        let formatted = format_history_row(&row);

        assert!(formatted.contains("#7"));
        assert!(formatted.contains("fixture"));
        assert!(formatted.contains("diplomacy"));
        assert!(formatted.contains("1.250000"));
        assert!(formatted.contains("talks resume"));
    }

    #[test]
    fn history_title_shows_filtered_count() {
        let rows = vec![
            HistoryRow {
                run_id: 1,
                generated_at: String::new(),
                mode: String::new(),
                dominant_field: "conflict".to_string(),
                risk_level: "high".to_string(),
                top_divergence_score: None,
                headline: "event A".to_string(),
            },
            HistoryRow {
                run_id: 2,
                generated_at: String::new(),
                mode: String::new(),
                dominant_field: "diplomacy".to_string(),
                risk_level: "low".to_string(),
                top_divergence_score: None,
                headline: "event B".to_string(),
            },
        ];
        let mut state = history_state(rows);
        assert_eq!(history_title(&state), " Run History ");

        state.search_query = "conflict".to_string();
        state.apply_search();
        assert_eq!(state.rows.len(), 1);
        assert_eq!(state.all_rows.len(), 2);
        let title = history_title(&state);
        assert!(title.contains("[1/2]"));
    }
}
