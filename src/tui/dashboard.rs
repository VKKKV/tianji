use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Paragraph};
use ratatui::Frame;

use super::state::{
    capitalize_first, compact_timestamp, format_alert_tier, placeholder_or_value, DashboardState,
    FieldStat,
};
use super::theme::KANAGAWA;
use crate::{classify_delta_tier, HotMemory};

use super::state::HistoryRow;

impl DashboardState {
    pub fn from_run_summary(
        rows: &[HistoryRow],
        memory: &HotMemory,
        run_summary: Option<serde_json::Value>,
    ) -> Self {
        let latest = rows.first();
        let latest_run_id = latest
            .map(|row| format!("#{}", row.run_id))
            .unwrap_or_else(|| "unavailable".to_string());
        let latest_generated_at = latest
            .map(|row| compact_timestamp(&row.generated_at))
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| "unavailable".to_string());
        let latest_mode = latest
            .map(|row| placeholder_or_value(&row.mode, "unavailable"))
            .unwrap_or_else(|| "unavailable".to_string());

        let (headline, field_summary, total_scored_events, top_events) = if let Some(ref summary) =
            run_summary
        {
            let hl = summary
                .get("scenario_summary")
                .and_then(|v| v.get("headline"))
                .and_then(|v| v.as_str())
                .filter(|s| !s.trim().is_empty())
                .map(|s| s.to_string())
                .or_else(|| {
                    latest.map(|row| placeholder_or_value(&row.headline, "No headline available."))
                })
                .unwrap_or_else(|| "No headline available.".to_string());

            let scored_events = summary
                .get("scored_events")
                .and_then(|v| v.as_array())
                .cloned()
                .unwrap_or_default();

            let total = scored_events.len();

            // Group by dominant_field → count + avg impact_score
            let mut field_map: std::collections::HashMap<String, Vec<f64>> =
                std::collections::HashMap::new();
            for event in &scored_events {
                let field = event
                    .get("dominant_field")
                    .and_then(|v| v.as_str())
                    .unwrap_or("uncategorized")
                    .to_string();
                let impact = event
                    .get("impact_score")
                    .and_then(|v| v.as_f64())
                    .unwrap_or(0.0);
                field_map.entry(field).or_default().push(impact);
            }
            let mut fs: Vec<FieldStat> = field_map
                .into_iter()
                .map(|(field, impacts)| {
                    let count = impacts.len();
                    let avg_impact = impacts.iter().sum::<f64>() / count as f64;
                    FieldStat {
                        field,
                        count,
                        avg_impact,
                    }
                })
                .collect();
            fs.sort_by(|a, b| b.count.cmp(&a.count).then_with(|| a.field.cmp(&b.field)));

            // Top 5 by impact_score desc
            let mut events_for_top: Vec<super::state::TopEvent> = scored_events
                .iter()
                .filter_map(|event| {
                    let title = event
                        .get("title")
                        .and_then(|v| v.as_str())
                        .unwrap_or("Untitled event")
                        .to_string();
                    let impact_score = event.get("impact_score")?.as_f64()?;
                    let dominant_field = event
                        .get("dominant_field")
                        .and_then(|v| v.as_str())
                        .unwrap_or("uncategorized")
                        .to_string();
                    Some(super::state::TopEvent {
                        title,
                        impact_score,
                        dominant_field,
                    })
                })
                .collect();
            events_for_top.sort_by(|a, b| {
                b.impact_score
                    .partial_cmp(&a.impact_score)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
            events_for_top.truncate(5);

            (hl, fs, total, events_for_top)
        } else {
            let hl = latest
                .map(|row| placeholder_or_value(&row.headline, "No headline available."))
                .unwrap_or_else(|| "No headline available.".to_string());
            (hl, Vec::new(), 0, Vec::new())
        };

        let newest_delta = memory.runs.front().and_then(|entry| entry.delta.as_ref());
        let alert_tier = newest_delta
            .and_then(classify_delta_tier)
            .map(format_alert_tier)
            .unwrap_or_else(|| "none".to_string());
        let delta_summary = newest_delta
            .map(|delta| {
                format!(
                    "{} total / {} critical / {} new signals",
                    delta.summary.total_changes,
                    delta.summary.critical_changes,
                    delta.summary.signal_breakdown.new_count
                )
            })
            .unwrap_or_else(|| "No recent delta available.".to_string());
        let delta_direction = newest_delta
            .map(|delta| format!("{:?}", delta.summary.direction))
            .unwrap_or_else(|| "unavailable".to_string());

        Self {
            latest_run_id,
            latest_generated_at,
            latest_mode,
            headline,
            field_summary,
            total_scored_events,
            top_events,
            alert_tier,
            delta_summary,
            delta_direction,
        }
    }
}

pub fn format_dashboard(dashboard: &DashboardState) -> String {
    let mut output = String::new();

    // Run metadata
    output.push_str(&format!(
        "Latest run\n  run: {}\n  generated: {}\n  mode: {}\n  headline: {}\n",
        dashboard.latest_run_id,
        dashboard.latest_generated_at,
        dashboard.latest_mode,
        dashboard.headline,
    ));

    // Field Summary
    output.push_str("\nField Summary\n");
    if dashboard.field_summary.is_empty() {
        output.push_str("  No field data available.\n");
    } else {
        for stat in &dashboard.field_summary {
            let event_word = if stat.count == 1 { "event " } else { "events" };
            output.push_str(&format!(
                "  {:<12} {:>2} {} avg impact {:.1}\n",
                stat.field, stat.count, event_word, stat.avg_impact,
            ));
        }
    }

    // Top Events
    output.push_str("\nTop Events\n");
    if dashboard.top_events.is_empty() {
        output.push_str("  No top events available.\n");
    } else {
        for (i, event) in dashboard.top_events.iter().enumerate() {
            output.push_str(&format!(
                "  #{:<2} {:<36} Im:{:.1}  {}\n",
                i + 1,
                event.title,
                event.impact_score,
                event.dominant_field,
            ));
        }
    }

    // Delta
    output.push_str(&format!(
        "\nRecent delta\n  alert tier: {}\n  summary: {}\n  direction: {}\n",
        dashboard.alert_tier, dashboard.delta_summary, dashboard.delta_direction,
    ));

    output
}

pub fn render_dashboard(frame: &mut Frame<'_>, area: Rect, dashboard: &DashboardState) {
    let mut lines: Vec<Line<'_>> = Vec::new();

    // Run metadata
    lines.push(Line::from(vec![
        Span::styled("  Run ", Style::default().fg(KANAGAWA.fg)),
        Span::styled(
            format!("{} ", dashboard.latest_run_id),
            Style::default().fg(KANAGAWA.value),
        ),
        Span::styled("· ", Style::default().fg(KANAGAWA.fg)),
        Span::styled(
            format!("{} ", dashboard.latest_mode),
            Style::default().fg(KANAGAWA.value),
        ),
        Span::styled("· ", Style::default().fg(KANAGAWA.fg)),
        Span::styled(
            dashboard.latest_generated_at.clone(),
            Style::default().fg(KANAGAWA.value),
        ),
    ]));
    lines.push(Line::from(vec![
        Span::styled("  Headline: ", Style::default().fg(KANAGAWA.label)),
        Span::styled(dashboard.headline.clone(), Style::default().fg(KANAGAWA.fg)),
    ]));

    // Blank line
    lines.push(Line::from(""));

    // Field Summary section
    lines.push(Line::from(vec![Span::styled(
        "  Field Summary",
        Style::default()
            .fg(KANAGAWA.title)
            .add_modifier(Modifier::BOLD),
    )]));

    for stat in &dashboard.field_summary {
        let event_word = if stat.count == 1 { "event " } else { "events" };
        let impact_color = if stat.avg_impact > 10.0 {
            KANAGAWA.up
        } else if stat.avg_impact > 5.0 {
            KANAGAWA.warn
        } else {
            KANAGAWA.fg
        };
        lines.push(Line::from(vec![
            Span::styled("    ", Style::default().fg(KANAGAWA.fg)),
            Span::styled(
                format!("{:<12}", stat.field),
                Style::default().fg(KANAGAWA.label),
            ),
            Span::styled(
                format!("{:>2} {} ", stat.count, event_word),
                Style::default().fg(KANAGAWA.fg),
            ),
            Span::styled("avg impact ", Style::default().fg(KANAGAWA.fg)),
            Span::styled(
                format!("{:.1}", stat.avg_impact),
                Style::default().fg(impact_color),
            ),
        ]));
    }

    // Blank line
    lines.push(Line::from(""));

    // Top Events section
    lines.push(Line::from(vec![Span::styled(
        "  Top Events",
        Style::default()
            .fg(KANAGAWA.title)
            .add_modifier(Modifier::BOLD),
    )]));

    for (i, event) in dashboard.top_events.iter().enumerate() {
        let impact_color = if event.impact_score > 10.0 {
            KANAGAWA.up
        } else if event.impact_score > 5.0 {
            KANAGAWA.warn
        } else {
            KANAGAWA.fg
        };
        lines.push(Line::from(vec![
            Span::styled(format!("    #{} ", i + 1), Style::default().fg(KANAGAWA.fg)),
            Span::styled(
                format!("{:<36}", event.title),
                Style::default().fg(KANAGAWA.fg),
            ),
            Span::styled("Im:", Style::default().fg(KANAGAWA.label)),
            Span::styled(
                format!("{:.1}", event.impact_score),
                Style::default().fg(impact_color),
            ),
            Span::styled("  ", Style::default().fg(KANAGAWA.fg)),
            Span::styled(
                event.dominant_field.clone(),
                Style::default().fg(KANAGAWA.label),
            ),
        ]));
    }

    // Blank line
    lines.push(Line::from(""));

    // Delta section
    let tier_color = match dashboard.alert_tier.as_str() {
        "flash" => KANAGAWA.down,
        "priority" => KANAGAWA.warn,
        _ => KANAGAWA.fg,
    };
    lines.push(Line::from(vec![
        Span::styled("  Delta · ", Style::default().fg(KANAGAWA.fg)),
        Span::styled(
            format!("{} ", capitalize_first(&dashboard.alert_tier)),
            Style::default().fg(tier_color),
        ),
        Span::styled(
            format!("· {}", dashboard.delta_summary),
            Style::default().fg(KANAGAWA.fg),
        ),
    ]));
    lines.push(Line::from(vec![
        Span::styled("  Direction: ", Style::default().fg(KANAGAWA.label)),
        Span::styled(
            dashboard.delta_direction.clone(),
            Style::default().fg(KANAGAWA.fg),
        ),
    ]));

    // Blank line
    lines.push(Line::from(""));

    let paragraph = Paragraph::new(lines)
        .block(
            Block::bordered()
                .title(" TianJi Dashboard ")
                .border_style(Style::default().fg(KANAGAWA.border))
                .style(super::render::base_style().bg(KANAGAWA.panel_bg)),
        )
        .style(super::render::base_style());
    frame.render_widget(paragraph, area);
}

#[cfg(test)]
mod tests {
    use super::*;
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

    fn dashboard_with_summary(summary: serde_json::Value) -> DashboardState {
        DashboardState::from_run_summary(&[row(1)], &HotMemory::default(), Some(summary))
    }

    #[test]
    fn dashboard_format_includes_run_delta_and_placeholders() {
        let formatted = format_dashboard(&dashboard());

        assert!(formatted.contains("Latest run"));
        assert!(formatted.contains("headline: headline"));
        assert!(formatted.contains("Recent delta"));
        assert!(formatted.contains("No recent delta available."));
        assert!(formatted.contains("Field Summary"));
        assert!(formatted.contains("No field data available."));
        assert!(formatted.contains("Top Events"));
        assert!(formatted.contains("No top events available."));
    }

    #[test]
    fn dashboard_format_includes_field_summary_and_top_events() {
        let summary = serde_json::json!({
            "scenario_summary": { "headline": "SCS escalation" },
            "scored_events": [
                { "title": "Carrier group enters SCS", "dominant_field": "conflict", "impact_score": 18.2 },
                { "title": "Iran nuclear talks", "dominant_field": "diplomacy", "impact_score": 12.1 },
                { "title": "EU chip framework", "dominant_field": "technology", "impact_score": 10.7 },
            ]
        });

        let dash = dashboard_with_summary(summary);
        let formatted = format_dashboard(&dash);

        assert!(formatted.contains("Field Summary"));
        assert!(formatted.contains("conflict"));
        assert!(formatted.contains("Top Events"));
        assert!(formatted.contains("Carrier group enters SCS"));
        assert!(formatted.contains("Im:18.2"));
        assert!(formatted.contains("SCS escalation"));
    }
}
