use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Paragraph};
use ratatui::Frame;

use super::state::SimulationState;
use super::theme::KANAGAWA;

pub fn format_simulation(sim: &SimulationState) -> String {
    let mut output = String::new();

    // Header
    output.push_str(&format!(
        "mode: {}  field: {}  tick {}/{}\n",
        sim.mode, sim.target, sim.tick, sim.total_ticks
    ));
    output.push_str(&format!("status: {}\n", sim.status));

    // Worldline
    output.push_str("\nWorldline\n");
    if sim.field_values.is_empty() {
        output.push_str("  No field data available.\n");
    } else {
        for field in &sim.field_values {
            let delta_arrow = if field.delta > 0.01 {
                format!("+{:.2}", field.delta)
            } else if field.delta < -0.01 {
                format!("{:.2}", field.delta)
            } else {
                "—".to_string()
            };
            output.push_str(&format!(
                "  {}.{}   {:.2}  {}\n",
                field.region, field.domain, field.value, delta_arrow
            ));
        }
    }

    // Agents
    output.push_str("\nAgents\n");
    if sim.agent_statuses.is_empty() {
        output.push_str("  No agents.\n");
    } else {
        for agent in &sim.agent_statuses {
            let action_part = if agent.last_action == "none" {
                String::new()
            } else {
                format!("   ({})", agent.last_action)
            };
            output.push_str(&format!(
                "  {:<12} {:<10}{}\n",
                agent.actor_id, agent.status, action_part
            ));
        }
    }

    // Events
    output.push_str("\nEvents\n");
    if sim.event_log.is_empty() {
        output.push_str("  No events.\n");
    } else {
        for event in &sim.event_log {
            output.push_str(&format!("  {event}\n"));
        }
    }

    output
}

pub fn render_simulation(frame: &mut Frame<'_>, area: Rect, simulation: Option<&SimulationState>) {
    let sim = match simulation {
        Some(s) => s,
        None => {
            let paragraph = Paragraph::new("No simulation loaded.")
                .block(
                    Block::bordered()
                        .title(" Simulation ")
                        .border_style(Style::default().fg(KANAGAWA.border))
                        .style(super::render::base_style().bg(KANAGAWA.panel_bg)),
                )
                .style(super::render::base_style());
            frame.render_widget(paragraph, area);
            return;
        }
    };

    let mut lines: Vec<Line<'_>> = Vec::new();

    // Header section
    lines.push(Line::from(vec![
        Span::styled("  mode: ", Style::default().fg(KANAGAWA.label)),
        Span::styled(
            format!("{} ", sim.mode),
            Style::default().fg(KANAGAWA.value),
        ),
        Span::styled("field: ", Style::default().fg(KANAGAWA.label)),
        Span::styled(
            format!("{} ", sim.target),
            Style::default().fg(KANAGAWA.value),
        ),
        Span::styled("tick ", Style::default().fg(KANAGAWA.label)),
        Span::styled(
            format!("{}/{}", sim.tick, sim.total_ticks),
            Style::default().fg(KANAGAWA.value),
        ),
    ]));
    lines.push(Line::from(vec![
        Span::styled("  status: ", Style::default().fg(KANAGAWA.label)),
        Span::styled(sim.status.clone(), Style::default().fg(KANAGAWA.fg)),
    ]));

    // Blank line
    lines.push(Line::from(""));

    // Worldline section
    lines.push(Line::from(vec![Span::styled(
        "  Worldline",
        Style::default()
            .fg(KANAGAWA.title)
            .add_modifier(Modifier::BOLD),
    )]));

    for field in &sim.field_values {
        let (delta_str, delta_color) = if field.delta > 0.01 {
            (format!("+{:.2}", field.delta), KANAGAWA.up)
        } else if field.delta < -0.01 {
            (format!("{:.2}", field.delta), KANAGAWA.down)
        } else {
            ("—".to_string(), KANAGAWA.fg)
        };
        lines.push(Line::from(vec![
            Span::styled("    ", Style::default().fg(KANAGAWA.fg)),
            Span::styled(
                format!("{}.{}", field.region, field.domain),
                Style::default().fg(KANAGAWA.label),
            ),
            Span::styled(
                format!("   {:.2}  ", field.value),
                Style::default().fg(KANAGAWA.fg),
            ),
            Span::styled(delta_str, Style::default().fg(delta_color)),
        ]));
    }

    // Blank line
    lines.push(Line::from(""));

    // Agents section
    lines.push(Line::from(vec![Span::styled(
        "  Agents",
        Style::default()
            .fg(KANAGAWA.title)
            .add_modifier(Modifier::BOLD),
    )]));

    for agent in &sim.agent_statuses {
        let action_part = if agent.last_action == "none" {
            String::new()
        } else {
            format!("   ({})", agent.last_action)
        };
        lines.push(Line::from(vec![
            Span::styled("    ", Style::default().fg(KANAGAWA.fg)),
            Span::styled(
                format!("{:<12}", agent.actor_id),
                Style::default().fg(KANAGAWA.label),
            ),
            Span::styled(
                format!("{:<10}", agent.status),
                Style::default().fg(KANAGAWA.fg),
            ),
            Span::styled(action_part, Style::default().fg(KANAGAWA.fg)),
        ]));
    }

    // Blank line
    lines.push(Line::from(""));

    // Events section
    lines.push(Line::from(vec![Span::styled(
        "  Events",
        Style::default()
            .fg(KANAGAWA.title)
            .add_modifier(Modifier::BOLD),
    )]));

    for event in &sim.event_log {
        lines.push(Line::from(vec![
            Span::styled("    ", Style::default().fg(KANAGAWA.fg)),
            Span::styled(event.clone(), Style::default().fg(KANAGAWA.fg)),
        ]));
    }

    let paragraph = Paragraph::new(lines)
        .block(
            Block::bordered()
                .title(" Simulation ")
                .border_style(Style::default().fg(KANAGAWA.border))
                .style(super::render::base_style().bg(KANAGAWA.panel_bg)),
        )
        .style(super::render::base_style());
    frame.render_widget(paragraph, area);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::state::{SimAgent, SimField, SimulationState};

    fn sample_simulation() -> SimulationState {
        SimulationState {
            mode: "forward".to_string(),
            target: "east-asia.conflict".to_string(),
            horizon: 30,
            tick: 3,
            total_ticks: 30,
            status: "running".to_string(),
            field_values: vec![
                SimField {
                    region: "east-asia".to_string(),
                    domain: "conflict".to_string(),
                    value: 0.84,
                    delta: 0.12,
                },
                SimField {
                    region: "global".to_string(),
                    domain: "trade_volume".to_string(),
                    value: 0.55,
                    delta: -0.08,
                },
                SimField {
                    region: "europe".to_string(),
                    domain: "stability".to_string(),
                    value: 0.58,
                    delta: 0.0,
                },
            ],
            agent_statuses: vec![
                SimAgent {
                    actor_id: "china".to_string(),
                    status: "thinking".to_string(),
                    last_action: "naval exercise".to_string(),
                },
                SimAgent {
                    actor_id: "usa".to_string(),
                    status: "done".to_string(),
                    last_action: "diplomatic protest".to_string(),
                },
                SimAgent {
                    actor_id: "russia".to_string(),
                    status: "idle".to_string(),
                    last_action: "none".to_string(),
                },
            ],
            event_log: vec![
                "tick 3: conflict increased by 0.15".to_string(),
                "tick 2: diplomacy decreased by 0.05".to_string(),
                "tick 1: conflict increased by 0.12".to_string(),
            ],
        }
    }

    #[test]
    fn simulation_format_includes_header_fields_agents_events() {
        let sim = sample_simulation();
        let formatted = format_simulation(&sim);

        assert!(formatted.contains("mode: forward"));
        assert!(formatted.contains("field: east-asia.conflict"));
        assert!(formatted.contains("tick 3/30"));
        assert!(formatted.contains("status: running"));
        assert!(formatted.contains("Worldline"));
        assert!(formatted.contains("east-asia.conflict"));
        assert!(formatted.contains("0.84"));
        assert!(formatted.contains("Agents"));
        assert!(formatted.contains("china"));
        assert!(formatted.contains("naval exercise"));
        assert!(formatted.contains("Events"));
        assert!(formatted.contains("conflict increased by 0.15"));
    }

    #[test]
    fn simulation_format_shows_delta_arrows() {
        let sim = sample_simulation();
        let formatted = format_simulation(&sim);

        // Positive delta
        assert!(formatted.contains("+0.12"));
        // Negative delta
        assert!(formatted.contains("-0.08"));
        // Zero delta
        assert!(formatted.contains("—"));
    }

    #[test]
    fn simulation_state_construction() {
        let sim = sample_simulation();

        assert_eq!(sim.mode, "forward");
        assert_eq!(sim.target, "east-asia.conflict");
        assert_eq!(sim.horizon, 30);
        assert_eq!(sim.tick, 3);
        assert_eq!(sim.total_ticks, 30);
        assert_eq!(sim.status, "running");
        assert_eq!(sim.field_values.len(), 3);
        assert_eq!(sim.agent_statuses.len(), 3);
        assert_eq!(sim.event_log.len(), 3);
    }

    #[test]
    fn simulation_format_empty_fields() {
        let sim = SimulationState {
            mode: "forward".to_string(),
            target: "global.conflict".to_string(),
            horizon: 10,
            tick: 0,
            total_ticks: 10,
            status: "completed".to_string(),
            field_values: vec![],
            agent_statuses: vec![],
            event_log: vec![],
        };
        let formatted = format_simulation(&sim);

        assert!(formatted.contains("No field data available."));
        assert!(formatted.contains("No agents."));
        assert!(formatted.contains("No events."));
    }
}
