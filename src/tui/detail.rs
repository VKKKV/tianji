use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::widgets::{Block, Paragraph};
use ratatui::Frame;

use super::state::DetailState;
use super::theme::KANAGAWA;

pub fn format_detail(detail: &DetailState) -> String {
    format!(
        "Run #{}\n  status: {}\n  schema: {}\n  mode: {}\n  generated: {}\n\nInput summary\n  {}\n\nScenario summary\n  {}\n\nScored events\n  {}\n\nEvent groups\n  {}\n\nIntervention candidates\n  {}",
        detail.run_id,
        detail.status,
        detail.schema_version,
        detail.mode,
        detail.generated_at,
        detail.input_summary,
        detail.scenario_summary,
        detail.scored_events.join("\n  "),
        detail.event_groups.join("\n  "),
        detail.intervention_candidates.join("\n  ")
    )
}

pub fn render_detail(frame: &mut Frame<'_>, area: Rect, detail: Option<&DetailState>) {
    let text = detail
        .map(format_detail)
        .unwrap_or_else(|| "No detail loaded.".to_string());
    let paragraph = Paragraph::new(text)
        .block(
            Block::bordered()
                .title(" Run Detail ")
                .border_style(Style::default().fg(KANAGAWA.border))
                .style(super::render::base_style().bg(KANAGAWA.panel_bg)),
        )
        .style(super::render::base_style());
    frame.render_widget(paragraph, area);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::state::DetailState;

    #[test]
    fn detail_format_includes_history_show_sections() {
        let detail = DetailState::from_json(&serde_json::json!({
            "run_id": 7,
            "schema_version": "1.0",
            "mode": "fixture",
            "generated_at": "1970-01-01T00:00:00+00:00",
            "input_summary": {"raw_item_count": 1},
            "scenario_summary": {"headline": "talks resume", "event_groups": []},
            "scored_events": [],
            "intervention_candidates": []
        }));

        let formatted = format_detail(&detail);

        assert!(formatted.contains("Run #7"));
        assert!(formatted.contains("Input summary"));
        assert!(formatted.contains("Scenario summary"));
        assert!(formatted.contains("Scored events"));
        assert!(formatted.contains("No event groups available."));
        assert!(formatted.contains("No intervention candidates available."));
    }
}
