use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::widgets::{Block, Paragraph};
use ratatui::Frame;

use super::state::CompareState;
use super::theme::KANAGAWA;

pub fn format_compare(compare: &CompareState) -> String {
    format!(
        "Compare #{} → #{}\n  status: {}\n\nLeft summary\n  {}\n\nRight summary\n  {}\n\nDiff\n  {}",
        compare.left_run_id,
        compare.right_run_id,
        compare.status,
        compare.left_summary.join("\n  "),
        compare.right_summary.join("\n  "),
        compare.diff_lines.join("\n  ")
    )
}

pub fn render_compare(frame: &mut Frame<'_>, area: Rect, compare: Option<&CompareState>) {
    let text = compare
        .map(format_compare)
        .unwrap_or_else(|| "No compare loaded.".to_string());
    let paragraph = Paragraph::new(text)
        .block(
            Block::bordered()
                .title(" Run Compare ")
                .border_style(Style::default().fg(KANAGAWA.border))
                .style(super::render::base_style().bg(KANAGAWA.panel_bg)),
        )
        .style(super::render::base_style());
    frame.render_widget(paragraph, area);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::state::CompareState;

    #[test]
    fn compare_format_includes_pair_summaries_and_diff() {
        let compare = CompareState {
            left_run_id: 4,
            right_run_id: 5,
            status: "loaded".to_string(),
            left_summary: vec![
                "run: #4".to_string(),
                "dominant field: technology".to_string(),
            ],
            right_summary: vec![
                "run: #5".to_string(),
                "dominant field: conflict".to_string(),
            ],
            diff_lines: vec!["dominant field changed: true".to_string()],
        };

        let formatted = format_compare(&compare);

        assert!(formatted.contains("Compare #4 → #5"));
        assert!(formatted.contains("Left summary"));
        assert!(formatted.contains("Right summary"));
        assert!(formatted.contains("Diff"));
        assert!(formatted.contains("dominant field changed: true"));
    }
}
