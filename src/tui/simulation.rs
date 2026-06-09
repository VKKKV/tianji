use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Paragraph};
use ratatui::Frame;

use super::state::{SimReplayFrame, SimulationState, SimulationViewState};
use super::theme::KANAGAWA;

pub fn format_simulation(sim: &SimulationState) -> String {
    format_simulation_with_replay(sim, 0, 0)
}

pub fn format_simulation_view(simulation: &SimulationViewState) -> String {
    match simulation.sim_state.as_ref() {
        Some(sim) => format_simulation_with_replay(
            sim,
            simulation.replay_cursor,
            simulation.replay_frame_count,
        ),
        None => "No simulation loaded.\n".to_string(),
    }
}

fn format_simulation_with_replay(
    sim: &SimulationState,
    replay_cursor: usize,
    replay_frame_count: usize,
) -> String {
    let mut output = String::new();
    let (frame_number, frame_count) = timeline_position(replay_cursor, replay_frame_count, sim);
    let selected_frame = selected_replay_frame(sim, replay_cursor);
    let display_tick = selected_frame.map(|frame| frame.tick).unwrap_or(sim.tick);

    // Header
    output.push_str(&format!(
        "mode: {}  field: {}  tick {}/{}  frame {}/{}\n",
        sim.mode, sim.target, display_tick, sim.total_ticks, frame_number, frame_count
    ));
    output.push_str(&format!("status: {}\n", sim.status));
    if let Some(frame) = selected_frame {
        output.push_str(&format!(
            "frame metadata: tick {} · event sequence length {}\n",
            frame.tick, frame.event_sequence_len
        ));
    }

    // Worldline
    output.push_str("\nWorldline\n");
    let field_values = selected_frame
        .map(|frame| frame.field_values.as_slice())
        .unwrap_or(sim.field_values.as_slice());
    if field_values.is_empty() {
        output.push_str("  No field data available.\n");
    } else {
        for field in field_values {
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

    if let Some(frame) = selected_frame {
        output.push_str("\nField changes\n");
        if frame.field_changes.is_empty() {
            output.push_str("  No field changes in selected frame.\n");
        } else {
            for field in &frame.field_changes {
                output.push_str(&format!(
                    "  {}.{}   value {:.2}  delta {:+.2}\n",
                    field.region, field.domain, field.value, field.delta
                ));
            }
        }
    }

    // Agents
    output.push_str("\nAgents\n");
    let frame_agents = selected_frame.map(frame_agents);
    let agents = frame_agents
        .as_deref()
        .unwrap_or(sim.agent_statuses.as_slice());
    if agents.is_empty() {
        output.push_str("  No agents.\n");
    } else {
        for agent in agents {
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
    let frame_events = selected_frame.map(frame_events);
    let events = frame_events.as_deref().unwrap_or(sim.event_log.as_slice());
    if events.is_empty() {
        output.push_str("  No events.\n");
    } else {
        for event in events {
            output.push_str(&format!("  {event}\n"));
        }
    }

    if let Some(frame) = selected_frame {
        output.push_str("\nAgent audit\n");
        if frame.audit_actions.is_empty() {
            output.push_str("  No agent audit actions in selected frame.\n");
        } else {
            for action in &frame.audit_actions {
                let target = action.target.as_deref().unwrap_or("none");
                let drivers = if action.drivers.is_empty() {
                    "none".to_string()
                } else {
                    action.drivers.join(", ")
                };
                output.push_str(&format!(
                    "  {} · action {} · target {} · confidence {:.2} · category {}\n",
                    action.actor_id, action.action_type, target, action.confidence, action.category
                ));
                output.push_str(&format!("    assessment: {}\n", action.assessment));
                output.push_str(&format!("    drivers: {}\n", drivers));
                output.push_str(&format!("    rationale: {}\n", action.rationale));
            }
        }
    }

    output
}

fn selected_replay_frame(sim: &SimulationState, replay_cursor: usize) -> Option<&SimReplayFrame> {
    sim.replay_frames
        .get(replay_cursor.min(sim.replay_frames.len().saturating_sub(1)))
}

fn timeline_position(
    replay_cursor: usize,
    replay_frame_count: usize,
    sim: &SimulationState,
) -> (usize, usize) {
    let frame_count = replay_frame_count.max(sim.total_ticks.max(sim.tick).max(1) as usize);
    let frame_number = replay_cursor.min(frame_count - 1) + 1;
    (frame_number, frame_count)
}

fn frame_agents(frame: &SimReplayFrame) -> Vec<super::state::SimAgent> {
    frame
        .audit_actions
        .iter()
        .map(|action| super::state::SimAgent {
            actor_id: action.actor_id.clone(),
            status: action.category.clone(),
            last_action: action.action_type.clone(),
        })
        .collect()
}

fn frame_events(frame: &SimReplayFrame) -> Vec<String> {
    vec![format!(
        "event sequence length: {}",
        frame.event_sequence_len
    )]
}

pub fn render_simulation(
    frame: &mut Frame<'_>,
    area: Rect,
    simulation: Option<&SimulationState>,
    replay_cursor: usize,
    replay_frame_count: usize,
    prune_mode: bool,
    prune_selected: &[usize],
) {
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
    let (frame_number, frame_count) = timeline_position(replay_cursor, replay_frame_count, sim);
    let selected_frame = selected_replay_frame(sim, replay_cursor);
    let display_tick = selected_frame.map(|frame| frame.tick).unwrap_or(sim.tick);

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
            format!("{}/{}", display_tick, sim.total_ticks),
            Style::default().fg(KANAGAWA.value),
        ),
        Span::styled("  frame ", Style::default().fg(KANAGAWA.label)),
        Span::styled(
            format!("{frame_number}/{frame_count}"),
            Style::default().fg(KANAGAWA.value),
        ),
    ]));
    lines.push(Line::from(vec![
        Span::styled("  status: ", Style::default().fg(KANAGAWA.label)),
        Span::styled(sim.status.clone(), Style::default().fg(KANAGAWA.fg)),
    ]));
    if let Some(frame) = selected_frame {
        lines.push(Line::from(vec![
            Span::styled("  frame metadata: ", Style::default().fg(KANAGAWA.label)),
            Span::styled(
                format!(
                    "tick {} · event sequence length {}",
                    frame.tick, frame.event_sequence_len
                ),
                Style::default().fg(KANAGAWA.fg),
            ),
        ]));
    }

    // Blank line
    lines.push(Line::from(""));

    // Worldline section
    lines.push(Line::from(vec![Span::styled(
        "  Worldline",
        Style::default()
            .fg(KANAGAWA.title)
            .add_modifier(Modifier::BOLD),
    )]));

    let field_values = selected_frame
        .map(|frame| frame.field_values.as_slice())
        .unwrap_or(sim.field_values.as_slice());
    for field in field_values {
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

    if let Some(frame) = selected_frame {
        lines.push(Line::from(""));
        lines.push(Line::from(vec![Span::styled(
            "  Field changes",
            Style::default()
                .fg(KANAGAWA.title)
                .add_modifier(Modifier::BOLD),
        )]));
        if frame.field_changes.is_empty() {
            lines.push(Line::from("    No field changes in selected frame."));
        } else {
            for field in &frame.field_changes {
                let color = if field.delta > 0.01 {
                    KANAGAWA.up
                } else if field.delta < -0.01 {
                    KANAGAWA.down
                } else {
                    KANAGAWA.fg
                };
                lines.push(Line::from(vec![
                    Span::styled("    ", Style::default().fg(KANAGAWA.fg)),
                    Span::styled(
                        format!("{}.{}", field.region, field.domain),
                        Style::default().fg(KANAGAWA.label),
                    ),
                    Span::styled(
                        format!("   value {:.2}  delta ", field.value),
                        Style::default().fg(KANAGAWA.fg),
                    ),
                    Span::styled(format!("{:+.2}", field.delta), Style::default().fg(color)),
                ]));
            }
        }
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

    let frame_agents = selected_frame.map(frame_agents);
    let agents = frame_agents
        .as_deref()
        .unwrap_or(sim.agent_statuses.as_slice());
    for agent in agents {
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

    let frame_events = selected_frame.map(frame_events);
    let events = frame_events.as_deref().unwrap_or(sim.event_log.as_slice());
    for event in events {
        lines.push(Line::from(vec![
            Span::styled("    ", Style::default().fg(KANAGAWA.fg)),
            Span::styled(event.clone(), Style::default().fg(KANAGAWA.fg)),
        ]));
    }

    if let Some(frame) = selected_frame {
        lines.push(Line::from(""));
        lines.push(Line::from(vec![Span::styled(
            "  Agent audit",
            Style::default()
                .fg(KANAGAWA.title)
                .add_modifier(Modifier::BOLD),
        )]));
        if frame.audit_actions.is_empty() {
            lines.push(Line::from("    No agent audit actions in selected frame."));
        } else {
            for action in &frame.audit_actions {
                let target = action.target.as_deref().unwrap_or("none");
                let drivers = if action.drivers.is_empty() {
                    "none".to_string()
                } else {
                    action.drivers.join(", ")
                };
                lines.push(Line::from(vec![
                    Span::styled("    ", Style::default().fg(KANAGAWA.fg)),
                    Span::styled(&action.actor_id, Style::default().fg(KANAGAWA.label)),
                    Span::raw(format!(
                        " · action {} · target {} · confidence {:.2} · category {}",
                        action.action_type, target, action.confidence, action.category
                    )),
                ]));
                lines.push(Line::from(format!(
                    "      assessment: {}",
                    action.assessment
                )));
                lines.push(Line::from(format!("      drivers: {drivers}")));
                lines.push(Line::from(format!("      rationale: {}", action.rationale)));
            }
        }
    }

    // Prune mode overlay
    if prune_mode && !sim.branches.is_empty() {
        lines.push(Line::from(""));
        lines.push(Line::from(vec![Span::styled(
            "  -- Prune Branches (Space=toggle, Enter=confirm, c/Esc=cancel) --",
            Style::default()
                .fg(KANAGAWA.title)
                .add_modifier(Modifier::BOLD),
        )]));
        for branch in &sim.branches {
            let selected = prune_selected.contains(&branch.index);
            let checkbox = if selected { "[x]" } else { "[ ]" };
            let checkbox_color = if selected { KANAGAWA.warn } else { KANAGAWA.fg };
            lines.push(Line::from(vec![
                Span::styled("    ", Style::default().fg(KANAGAWA.fg)),
                Span::styled(
                    format!("{} #{:<2}", checkbox, branch.index),
                    Style::default().fg(checkbox_color),
                ),
                Span::styled(
                    format!(
                        "  p={:.3}  div={:.2}  events={}",
                        branch.probability, branch.divergence, branch.event_count
                    ),
                    Style::default().fg(KANAGAWA.fg),
                ),
            ]));
        }
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
            branches: vec![],
            replay_frames: vec![],
        }
    }

    fn sample_trace_simulation() -> SimulationState {
        use crate::nuwa::outcome::{ConvergenceReason, SimulationOutcome};
        use crate::nuwa::sandbox::SimulationMode;
        use crate::nuwa::trace::{
            SimulationTrace, SimulationTraceCompleted, SimulationTraceFrame,
            SimulationTraceMetadata, TraceAgentAction, SIM_TRACE_SCHEMA_VERSION,
        };
        use crate::worldline::types::FieldKey;
        use std::collections::BTreeMap;

        let field = FieldKey {
            region: "global".to_string(),
            domain: "conflict".to_string(),
        };
        let empty_outcome = SimulationOutcome {
            mode: SimulationMode::Forward {
                target_field: field.clone(),
                horizon_ticks: 2,
            },
            branches: vec![],
            intervention_paths: vec![],
            tick_count: 2,
            convergence_reason: ConvergenceReason::MaxTicksReached(2),
        };
        let trace = SimulationTrace {
            metadata: SimulationTraceMetadata {
                schema_version: SIM_TRACE_SCHEMA_VERSION.to_string(),
                mode: "forward".to_string(),
                target_field: Some(field.clone()),
                horizon_ticks: 2,
                frame_count: 2,
            },
            frames: vec![
                SimulationTraceFrame {
                    tick: 1,
                    field_values: BTreeMap::from([(field.clone(), 3.75)]),
                    field_changes: vec![crate::hongmeng::referee::FieldChange {
                        region: "global".to_string(),
                        domain: "conflict".to_string(),
                        delta: 0.25,
                    }],
                    agent_actions: vec![TraceAgentAction {
                        actor_id: "actor-a".to_string(),
                        action_type: "observe".to_string(),
                        target: Some("actor-b".to_string()),
                        confidence: 0.71,
                        rationale: "watching escalation indicators".to_string(),
                        assessment: "pressure is rising".to_string(),
                        category: "monitoring".to_string(),
                        drivers: vec!["field_delta".to_string(), "recent_action".to_string()],
                    }],
                    event_sequence_len: 4,
                },
                SimulationTraceFrame {
                    tick: 2,
                    field_values: BTreeMap::from([(field.clone(), 4.5)]),
                    field_changes: vec![crate::hongmeng::referee::FieldChange {
                        region: "global".to_string(),
                        domain: "conflict".to_string(),
                        delta: 0.75,
                    }],
                    agent_actions: vec![TraceAgentAction {
                        actor_id: "actor-c".to_string(),
                        action_type: "signal".to_string(),
                        target: None,
                        confidence: 0.62,
                        rationale: "signaling restraint".to_string(),
                        assessment: "pressure is stabilizing".to_string(),
                        category: "signaling".to_string(),
                        drivers: vec!["latest_frame".to_string()],
                    }],
                    event_sequence_len: 5,
                },
            ],
            completed: SimulationTraceCompleted {
                outcome: empty_outcome,
            },
        };
        SimulationState::from_trace(&trace)
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
    fn simulation_format_includes_timeline_position() {
        let sim = sample_simulation();
        let view = SimulationViewState::new(Some(sim));
        let formatted = format_simulation_view(&view);

        assert!(formatted.contains("frame 3/30"));
    }

    #[test]
    fn tui_replay_contract_formats_cursor_and_metadata() {
        let mut view = SimulationViewState::new(Some(sample_simulation()));

        assert_eq!(view.replay_cursor, 2);
        assert_eq!(view.replay_frame_count, 30);
        assert_eq!(
            format_simulation_view(&view).lines().next(),
            Some("mode: forward  field: east-asia.conflict  tick 3/30  frame 3/30")
        );

        view.previous_replay_frame();
        assert_eq!(view.replay_cursor, 1);
        assert_eq!(
            format_simulation_view(&view).lines().next(),
            Some("mode: forward  field: east-asia.conflict  tick 3/30  frame 2/30")
        );
    }

    #[test]
    fn trace_replay_format_uses_selected_frame_data() {
        let sim = sample_trace_simulation();
        let mut view = SimulationViewState::new(Some(sim));

        assert_eq!(view.replay_cursor, 1);
        let latest = format_simulation_view(&view);
        assert!(latest.contains("tick 2/2"));
        assert!(latest.contains("frame 2/2"));
        assert!(latest.contains("value 4.50  delta +0.75"));
        assert!(latest.contains("event sequence length 5"));
        assert!(latest.contains("actor-c"));
        assert!(latest.contains("signal"));
        assert!(!latest.contains("actor-a · action observe"));

        view.previous_replay_frame();
        let previous = format_simulation_view(&view);
        assert!(previous.contains("tick 1/2"));
        assert!(previous.contains("frame 1/2"));
        assert!(previous.contains("value 3.75  delta +0.25"));
        assert!(previous.contains("event sequence length 4"));
        assert!(previous.contains("actor-a"));
        assert!(previous.contains("observe"));
        assert!(!previous.contains("actor-c · action signal"));
    }

    #[test]
    fn trace_replay_format_renders_agent_audit_fields() {
        let sim = sample_trace_simulation();
        let mut view = SimulationViewState::new(Some(sim));
        view.previous_replay_frame();
        let formatted = format_simulation_view(&view);

        assert!(formatted.contains("Agent audit"));
        assert!(formatted.contains("actor-a · action observe · target actor-b"));
        assert!(formatted.contains("confidence 0.71"));
        assert!(formatted.contains("category monitoring"));
        assert!(formatted.contains("assessment: pressure is rising"));
        assert!(formatted.contains("drivers: field_delta, recent_action"));
        assert!(formatted.contains("rationale: watching escalation indicators"));
    }

    #[test]
    fn trace_replay_sanitizes_control_characters() {
        use crate::nuwa::outcome::{ConvergenceReason, SimulationOutcome};
        use crate::nuwa::sandbox::SimulationMode;
        use crate::nuwa::trace::{
            SimulationTrace, SimulationTraceCompleted, SimulationTraceFrame,
            SimulationTraceMetadata, TraceAgentAction, SIM_TRACE_SCHEMA_VERSION,
        };
        use crate::worldline::types::FieldKey;
        use std::collections::BTreeMap;

        let field = FieldKey {
            region: "global".to_string(),
            domain: "conflict".to_string(),
        };
        let trace = SimulationTrace {
            metadata: SimulationTraceMetadata {
                schema_version: SIM_TRACE_SCHEMA_VERSION.to_string(),
                mode: "forward".to_string(),
                target_field: Some(field.clone()),
                horizon_ticks: 1,
                frame_count: 1,
            },
            frames: vec![SimulationTraceFrame {
                tick: 1,
                field_values: BTreeMap::from([(field.clone(), 1.0)]),
                field_changes: vec![],
                agent_actions: vec![TraceAgentAction {
                    actor_id: "actor\u{1b}[31m".to_string(),
                    action_type: "observe".to_string(),
                    target: None,
                    confidence: 0.5,
                    rationale: format!("line1\nline2\t{}", "x".repeat(300)),
                    assessment: "bad\u{1b}[0m\nassessment".to_string(),
                    category: "cat\rname".to_string(),
                    drivers: vec!["driver\nA".to_string()],
                }],
                event_sequence_len: 1,
            }],
            completed: SimulationTraceCompleted {
                outcome: SimulationOutcome {
                    mode: SimulationMode::Forward {
                        target_field: field,
                        horizon_ticks: 1,
                    },
                    branches: vec![],
                    intervention_paths: vec![],
                    tick_count: 1,
                    convergence_reason: ConvergenceReason::MaxTicksReached(1),
                },
            },
        };
        let view = SimulationViewState::new(Some(SimulationState::from_trace(&trace)));
        let formatted = format_simulation_view(&view);

        assert!(!formatted.contains('\u{1b}'));
        assert!(formatted.contains("assessment: bad assessment"));
        assert!(formatted.contains("drivers: driver A"));
        assert!(formatted.contains("rationale: line1 line2"));
        assert!(formatted.contains('…'));
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
            branches: vec![],
            replay_frames: vec![],
        };
        let formatted = format_simulation(&sim);

        assert!(formatted.contains("No field data available."));
        assert!(formatted.contains("No agents."));
        assert!(formatted.contains("No events."));
    }
}
