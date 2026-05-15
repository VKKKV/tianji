mod compare;
mod dashboard;
mod detail;
mod history;
mod render;
mod simulation;
mod state;
mod theme;

pub use compare::{format_compare, render_compare};
pub use dashboard::{format_dashboard, render_dashboard};
pub use detail::{format_detail, render_detail};
pub use history::{format_history_row, history_title, render_history};
pub use render::{base_style, render};
pub use simulation::{format_simulation, render_simulation};
pub use state::{
    array_string_field, bool_field, capitalize_first, compact_json_field, compact_json_value,
    compact_timestamp, detect_glyph_mode, format_alert_tier, numeric_field, optional_f64_field,
    placeholder_or_value, signed_numeric_field, string_field, CompareState, DashboardState,
    DetailState, FieldStat, GlyphSet, HistoryRow, SimAgent, SimField, SimulationState, TopEvent,
    TuiState, TuiView, ASCII_GLYPHS, EMPTY_TUI_MESSAGE, NERD_GLYPHS,
};
pub use theme::{Theme, KANAGAWA};

use std::io::{self, Stdout};
use std::path::Path;
use std::time::Duration;

use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;

use crate::storage::{
    get_latest_run_id, get_run_summary, list_runs, EventGroupFilters, RunListFilters,
    ScoredEventFilters,
};
use crate::{delta_memory_path, HotMemory, TianJiError};

pub fn run_history_browser(
    sqlite_path: &str,
    limit: usize,
    simulate: Option<&str>,
) -> Result<String, TianJiError> {
    if !Path::new(sqlite_path).exists() {
        return Ok(EMPTY_TUI_MESSAGE.to_string());
    }

    let values = match list_runs(sqlite_path, limit, &RunListFilters::default()) {
        Ok(rows) => rows,
        Err(TianJiError::Storage(error)) if is_missing_runs_table(&error) => Vec::new(),
        Err(error) => return Err(error),
    };
    let rows: Vec<HistoryRow> = values.iter().map(HistoryRow::from_json).collect();
    if rows.is_empty() {
        return Ok(EMPTY_TUI_MESSAGE.to_string());
    }

    let memory = HotMemory::load(&delta_memory_path(sqlite_path));

    let latest_summary = if !rows.is_empty() {
        get_latest_run_id(sqlite_path)
            .ok()
            .flatten()
            .and_then(|id| {
                get_run_summary(
                    sqlite_path,
                    id,
                    &ScoredEventFilters::default(),
                    false,
                    &EventGroupFilters::default(),
                )
                .ok()
                .flatten()
            })
    } else {
        None
    };

    let dashboard = DashboardState::from_run_summary(&rows, &memory, latest_summary);
    let mut tui_state = TuiState::new_with_storage(rows, dashboard, sqlite_path);

    // Optionally run a simulation if --simulate was provided
    if let Some(sim_spec) = simulate {
        if let Some(sim_state) = run_demo_simulation(sim_spec) {
            tui_state.simulation = Some(sim_state);
        }
    }

    run_terminal(tui_state)?;
    Ok(String::new())
}

fn is_missing_runs_table(error: &rusqlite::Error) -> bool {
    match error {
        rusqlite::Error::SqliteFailure(_, Some(message)) => message.contains("no such table: runs"),
        _ => false,
    }
}

/// Parse a `--simulate` spec like "east-asia.conflict:30" and run a forward simulation.
/// Returns `Some(SimulationState)` on success, `None` on parse or execution failure.
fn run_demo_simulation(spec: &str) -> Option<SimulationState> {
    use std::collections::BTreeMap;

    use crate::hongmeng::Agent;
    use crate::hongmeng::HongmengConfig;
    use crate::nuwa::forward::run_forward;
    use crate::nuwa::sandbox::SimulationMode;
    use crate::profile::types::{ActorProfile, ActorTier, Capabilities};
    use crate::profile::ProfileRegistry;
    use crate::worldline::types::{FieldKey, Worldline};

    let parts: Vec<&str> = spec.split(':').collect();
    if parts.len() != 2 {
        return None;
    }
    let field_str = parts[0];
    let horizon: u64 = parts[1].parse().ok()?;
    let field_parts: Vec<&str> = field_str.split('.').collect();
    if field_parts.len() != 2 {
        return None;
    }
    let target_field = FieldKey {
        region: field_parts[0].to_string(),
        domain: field_parts[1].to_string(),
    };

    // Load profiles or create stub agents
    let agents = match ProfileRegistry::load_from_dir(std::path::Path::new("profiles/")) {
        Ok(registry) if !registry.profiles.is_empty() => registry
            .profiles
            .values()
            .map(|p| Agent::from_profile(p.clone()))
            .collect::<Vec<_>>(),
        _ => {
            let stub_profile = ActorProfile {
                id: "stub".to_string(),
                name: "Stub Agent".to_string(),
                tier: ActorTier::Nation,
                interests: vec![],
                red_lines: vec![],
                capabilities: Capabilities::default(),
                behavior_patterns: vec!["observe".to_string(), "diplomatic_protest".to_string()],
                historical_analogues: vec![],
            };
            vec![Agent::from_profile(stub_profile)]
        }
    };

    // Create stub worldline with the target field
    let mut fields = BTreeMap::new();
    fields.insert(target_field.clone(), 3.5);
    let hash = Worldline::compute_snapshot_hash(&fields);
    let worldline = Worldline {
        id: 0,
        fields,
        events: vec![],
        causal_graph: petgraph::graph::DiGraph::new(),
        active_actors: std::collections::BTreeSet::new(),
        divergence: 0.0,
        parent: None,
        diverge_tick: 0,
        snapshot_hash: hash,
        created_at: chrono::Utc::now(),
    };

    let mode = SimulationMode::Forward {
        target_field: target_field.clone(),
        horizon_ticks: horizon,
    };
    let config = HongmengConfig::default();

    let outcome = run_forward(&worldline, &agents, &mode, &config);

    // Convert outcome → SimulationState
    let primary_branch = outcome.branches.first();
    let branch_worldline = primary_branch.map(|b| &b.worldline);

    let tick = outcome.tick_count;
    let total_ticks = horizon;
    let status = match &outcome.convergence_reason {
        crate::nuwa::outcome::ConvergenceReason::MaxTicksReached(_) => "completed",
        crate::nuwa::outcome::ConvergenceReason::FieldTargetReached => "converged",
        crate::nuwa::outcome::ConvergenceReason::FieldStabilized(_) => "stabilized",
        _ => "completed",
    };

    // Build field_values from the primary branch worldline
    let mut field_values: Vec<SimField> = Vec::new();
    if let Some(wl) = branch_worldline {
        for (key, value) in &wl.fields {
            let base_value = worldline.fields.get(key).copied().unwrap_or(0.0);
            field_values.push(SimField {
                region: key.region.clone(),
                domain: key.domain.clone(),
                value: *value,
                delta: value - base_value,
            });
        }
        field_values.sort_by(|a, b| {
            b.delta
                .abs()
                .partial_cmp(&a.delta.abs())
                .unwrap_or(std::cmp::Ordering::Equal)
        });
    }

    // Build agent_statuses from the agents used
    let agent_statuses: Vec<SimAgent> = agents
        .iter()
        .map(|agent| {
            let last_action = agent
                .action_history
                .last()
                .map(|a| a.action_type.clone())
                .unwrap_or_else(|| "none".to_string());
            SimAgent {
                actor_id: agent.actor_id.clone(),
                status: "done".to_string(),
                last_action,
            }
        })
        .collect();

    // Build event_log from the primary branch
    let event_log = primary_branch
        .map(|b| b.event_sequence.clone())
        .unwrap_or_default();

    Some(SimulationState {
        mode: "forward".to_string(),
        target: format!("{}.{}", target_field.region, target_field.domain),
        horizon,
        tick,
        total_ticks,
        status: status.to_string(),
        field_values,
        agent_statuses,
        event_log,
    })
}

fn run_terminal(state: TuiState) -> Result<(), TianJiError> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    let mut setup_guard = TerminalSetupGuard::active();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let terminal = Terminal::new(backend)?;
    setup_guard.disarm();
    let mut session = TerminalSession { terminal };
    session.run(state)
}

struct TerminalSetupGuard {
    active: bool,
}

impl TerminalSetupGuard {
    fn active() -> Self {
        Self { active: true }
    }

    fn disarm(&mut self) {
        self.active = false;
    }
}

impl Drop for TerminalSetupGuard {
    fn drop(&mut self) {
        if self.active {
            let _ = disable_raw_mode();
            let _ = execute!(io::stdout(), LeaveAlternateScreen);
        }
    }
}

struct TerminalSession {
    terminal: Terminal<CrosstermBackend<Stdout>>,
}

impl TerminalSession {
    fn run(&mut self, mut state: TuiState) -> Result<(), TianJiError> {
        loop {
            self.terminal
                .draw(|frame| self::render::render(frame, &state))?;
            if event::poll(Duration::from_millis(100))? {
                let Event::Key(key) = event::read()? else {
                    continue;
                };
                if key.kind != KeyEventKind::Press {
                    continue;
                }
                if !handle_key(&mut state, &key) {
                    break;
                }
            }
        }
        Ok(())
    }
}

impl Drop for TerminalSession {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let _ = execute!(self.terminal.backend_mut(), LeaveAlternateScreen);
        let _ = self.terminal.show_cursor();
    }
}

fn handle_key(state: &mut TuiState, key: &KeyEvent) -> bool {
    if state.search_active {
        match key.code {
            KeyCode::Esc => {
                state.search_active = false;
                state.search_query.clear();
                state.rows = state.all_rows.clone();
                state.selected = 0;
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

    if key.modifiers.contains(KeyModifiers::CONTROL) {
        match key.code {
            KeyCode::Char('d') => {
                if state.view == TuiView::History {
                    let page = state.rows.len().max(1) / 2;
                    state.selected =
                        (state.selected + page).min(state.rows.len().saturating_sub(1));
                }
                return true;
            }
            KeyCode::Char('u') => {
                if state.view == TuiView::History {
                    let page = state.rows.len().max(1) / 2;
                    state.selected = state.selected.saturating_sub(page);
                }
                return true;
            }
            _ => {}
        }
    }

    match key.code {
        KeyCode::Char('q') => false,
        KeyCode::Esc => {
            if state.view == TuiView::Simulation {
                state.show_dashboard();
            } else if matches!(state.view, TuiView::Detail | TuiView::Compare) {
                state.show_history();
            } else {
                state.pending_g = false;
            }
            true
        }
        KeyCode::Enter => {
            if state.view == TuiView::History {
                if !state.open_selected_compare() {
                    state.open_selected_detail();
                }
            } else {
                state.pending_g = false;
            }
            true
        }
        KeyCode::Char('d') | KeyCode::Char('D') | KeyCode::Char('1') => {
            state.show_dashboard();
            true
        }
        KeyCode::Char('h') | KeyCode::Char('H') | KeyCode::Char('2') => {
            state.show_history();
            true
        }
        KeyCode::Char('3') => {
            if state.simulation.is_some() {
                state.view = TuiView::Simulation;
            }
            true
        }
        KeyCode::Char('c') => {
            state.stage_selected_for_compare();
            true
        }
        KeyCode::Char('j') | KeyCode::Down => {
            state.pending_g = false;
            if state.view == TuiView::History {
                state.select_next();
            }
            true
        }
        KeyCode::Char('k') | KeyCode::Up => {
            state.pending_g = false;
            if state.view == TuiView::History {
                state.select_previous();
            }
            true
        }
        KeyCode::Char('G') | KeyCode::End => {
            state.pending_g = false;
            if state.view == TuiView::History {
                state.select_last();
            }
            true
        }
        KeyCode::Char('g') => {
            if state.pending_g {
                if state.view == TuiView::History {
                    state.select_first();
                }
                state.pending_g = false;
            } else {
                state.pending_g = true;
            }
            true
        }
        KeyCode::Home => {
            state.pending_g = false;
            if state.view == TuiView::History {
                state.select_first();
            }
            true
        }
        KeyCode::Char('/') if state.view == TuiView::History => {
            state.search_active = true;
            state.search_query.clear();
            true
        }
        _ => {
            state.pending_g = false;
            true
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::HotMemory;

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    fn ctrl_key(c: char) -> KeyEvent {
        KeyEvent::new(KeyCode::Char(c), KeyModifiers::CONTROL)
    }

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
    fn key_handler_maps_navigation_and_quit() {
        let mut state = history_state(vec![row(1), row(2)]);

        assert!(handle_key(&mut state, &key(KeyCode::Char('j'))));
        assert_eq!(state.selected(), 1);
        assert!(handle_key(&mut state, &key(KeyCode::Up)));
        assert_eq!(state.selected(), 0);
        assert!(handle_key(&mut state, &key(KeyCode::Down)));
        assert_eq!(state.selected(), 1);
        assert!(handle_key(&mut state, &key(KeyCode::Char('k'))));
        assert_eq!(state.selected(), 0);
        assert!(!handle_key(&mut state, &key(KeyCode::Char('q'))));
    }

    #[test]
    fn key_handler_maps_vim_first_and_last_navigation() {
        let mut state = history_state(vec![row(1), row(2), row(3)]);

        assert!(handle_key(&mut state, &key(KeyCode::Char('G'))));
        assert_eq!(state.selected(), 2);
        assert!(handle_key(&mut state, &key(KeyCode::Char('g'))));
        assert_eq!(state.selected(), 2);
        assert!(handle_key(&mut state, &key(KeyCode::Char('g'))));
        assert_eq!(state.selected(), 0);
    }

    #[test]
    fn key_handler_maps_home_and_end_aliases() {
        let mut state = history_state(vec![row(1), row(2), row(3)]);

        assert!(handle_key(&mut state, &key(KeyCode::End)));
        assert_eq!(state.selected(), 2);
        assert!(handle_key(&mut state, &key(KeyCode::Home)));
        assert_eq!(state.selected(), 0);
    }

    #[test]
    fn key_handler_clears_pending_g_after_unrelated_key() {
        let mut state = history_state(vec![row(1), row(2), row(3)]);
        state.select_last();

        assert!(handle_key(&mut state, &key(KeyCode::Char('g'))));
        assert_eq!(state.selected(), 2);
        assert!(handle_key(&mut state, &key(KeyCode::Char('x'))));
        assert_eq!(state.selected(), 2);
        assert!(handle_key(&mut state, &key(KeyCode::Char('g'))));
        assert_eq!(state.selected(), 2);
    }

    #[test]
    fn key_handler_switches_between_dashboard_and_history() {
        let mut state = TuiState::new(vec![row(1), row(2)], dashboard());

        assert_eq!(state.view, TuiView::Dashboard);
        assert!(handle_key(&mut state, &key(KeyCode::Char('j'))));
        assert_eq!(state.selected(), 0);

        assert!(handle_key(&mut state, &key(KeyCode::Char('h'))));
        assert_eq!(state.view, TuiView::History);
        assert!(handle_key(&mut state, &key(KeyCode::Char('j'))));
        assert_eq!(state.selected(), 1);

        assert!(handle_key(&mut state, &key(KeyCode::Char('d'))));
        assert_eq!(state.view, TuiView::Dashboard);
    }

    #[test]
    fn key_handler_opens_detail_from_history_and_returns_with_escape_or_h() {
        let mut state = history_state(vec![row(1), row(2)]);
        state.select_next();

        assert!(handle_key(&mut state, &key(KeyCode::Enter)));
        assert_eq!(state.view, TuiView::Detail);
        assert_eq!(state.detail.as_ref().map(|d| d.run_id), Some(2));
        assert_eq!(state.selected(), 1);

        assert!(handle_key(&mut state, &key(KeyCode::Esc)));
        assert_eq!(state.view, TuiView::History);
        assert_eq!(state.selected(), 1);

        assert!(handle_key(&mut state, &key(KeyCode::Enter)));
        assert_eq!(state.view, TuiView::Detail);
        assert!(handle_key(&mut state, &key(KeyCode::Char('h'))));
        assert_eq!(state.view, TuiView::History);
    }

    #[test]
    fn key_handler_stages_left_and_enter_opens_compare_from_history() {
        let mut state = history_state(vec![row(1), row(2)]);

        assert!(handle_key(&mut state, &key(KeyCode::Char('c'))));
        assert_eq!(state.staged_left_run_id, Some(1));
        assert!(history_title(&state).contains("staged left #1"));

        assert!(handle_key(&mut state, &key(KeyCode::Char('j'))));
        assert_eq!(state.selected(), 1);
        assert!(handle_key(&mut state, &key(KeyCode::Enter)));

        assert_eq!(state.view, TuiView::Compare);
        let compare = state.compare.as_ref().expect("compare state");
        assert_eq!(compare.left_run_id, 1);
        assert_eq!(compare.right_run_id, 2);
        assert!(compare.status.contains("could not"));
        assert_eq!(state.selected(), 1);
    }

    #[test]
    fn enter_without_staged_left_preserves_detail_behavior() {
        let mut state = history_state(vec![row(1), row(2)]);
        state.select_next();

        assert!(handle_key(&mut state, &key(KeyCode::Enter)));

        assert_eq!(state.view, TuiView::Detail);
        assert_eq!(state.detail.as_ref().map(|d| d.run_id), Some(2));
        assert!(state.compare.is_none());
    }

    #[test]
    fn compare_view_returns_to_history_and_ignores_history_navigation() {
        let mut state = history_state(vec![row(1), row(2)]);
        state.show_compare(CompareState::missing(1, 2));

        assert!(handle_key(&mut state, &key(KeyCode::Char('j'))));
        assert_eq!(state.selected(), 0);
        assert_eq!(state.view, TuiView::Compare);

        assert!(handle_key(&mut state, &key(KeyCode::Esc)));
        assert_eq!(state.view, TuiView::History);

        state.show_compare(CompareState::missing(1, 2));
        assert!(handle_key(&mut state, &key(KeyCode::Char('h'))));
        assert_eq!(state.view, TuiView::History);
    }

    #[test]
    fn key_handler_slash_activates_search_in_history() {
        let mut state = history_state(vec![row(1), row(2)]);
        assert!(!state.search_active);
        assert!(handle_key(&mut state, &key(KeyCode::Char('/'))));
        assert!(state.search_active);
        assert!(state.search_query.is_empty());
    }

    #[test]
    fn key_handler_search_type_and_submit() {
        let mut state = history_state(vec![row(1), row(2)]);
        assert!(handle_key(&mut state, &key(KeyCode::Char('/'))));
        assert!(handle_key(&mut state, &key(KeyCode::Char('t'))));
        assert!(handle_key(&mut state, &key(KeyCode::Char('e'))));
        assert!(handle_key(&mut state, &key(KeyCode::Char('c'))));
        assert!(handle_key(&mut state, &key(KeyCode::Char('h'))));
        assert_eq!(state.search_query, "tech");
        assert!(state.search_active);
        assert!(handle_key(&mut state, &key(KeyCode::Enter)));
        assert!(!state.search_active);
        assert!(!state.rows.is_empty());
    }

    #[test]
    fn key_handler_esc_clears_search_and_restores() {
        let mut state = history_state(vec![row(1), row(2)]);
        assert!(handle_key(&mut state, &key(KeyCode::Char('/'))));
        assert!(handle_key(&mut state, &key(KeyCode::Char('z'))));
        assert!(handle_key(&mut state, &key(KeyCode::Esc)));
        assert!(!state.search_active);
        assert!(state.search_query.is_empty());
        assert_eq!(state.rows.len(), state.all_rows.len());
    }

    #[test]
    fn key_handler_backspace_in_search() {
        let mut state = history_state(vec![row(1), row(2)]);
        assert!(handle_key(&mut state, &key(KeyCode::Char('/'))));
        assert!(handle_key(&mut state, &key(KeyCode::Char('a'))));
        assert!(handle_key(&mut state, &key(KeyCode::Char('b'))));
        assert_eq!(state.search_query, "ab");
        assert!(handle_key(&mut state, &key(KeyCode::Backspace)));
        assert_eq!(state.search_query, "a");
    }

    #[test]
    fn key_handler_search_blocks_quit_when_active() {
        let mut state = history_state(vec![row(1)]);
        assert!(handle_key(&mut state, &key(KeyCode::Char('/'))));
        assert!(handle_key(&mut state, &key(KeyCode::Char('q'))));
        assert_eq!(state.search_query, "q");
        assert!(state.search_active);
    }

    #[test]
    fn ctrl_d_half_page_scroll_down_in_history() {
        let rows: Vec<HistoryRow> = (1..=10).map(row).collect();
        let mut state = history_state(rows);
        assert_eq!(state.selected(), 0);

        assert!(handle_key(&mut state, &ctrl_key('d')));
        assert_eq!(state.selected(), 5);

        assert!(handle_key(&mut state, &ctrl_key('d')));
        assert_eq!(state.selected(), 9);

        assert!(handle_key(&mut state, &ctrl_key('d')));
        assert_eq!(state.selected(), 9);
    }

    #[test]
    fn ctrl_u_half_page_scroll_up_in_history() {
        let rows: Vec<HistoryRow> = (1..=10).map(row).collect();
        let mut state = history_state(rows);
        state.select_last();
        assert_eq!(state.selected(), 9);

        assert!(handle_key(&mut state, &ctrl_key('u')));
        assert_eq!(state.selected(), 4);

        assert!(handle_key(&mut state, &ctrl_key('u')));
        assert_eq!(state.selected(), 0);

        assert!(handle_key(&mut state, &ctrl_key('u')));
        assert_eq!(state.selected(), 0);
    }

    #[test]
    fn ctrl_d_does_not_affect_dashboard_view() {
        let mut state = TuiState::new(vec![row(1), row(2), row(3)], dashboard());
        assert_eq!(state.view, TuiView::Dashboard);
        assert!(handle_key(&mut state, &ctrl_key('d')));
        assert_eq!(state.view, TuiView::Dashboard);
    }

    #[test]
    fn plain_d_still_switches_to_dashboard() {
        let rows: Vec<HistoryRow> = (1..=5).map(row).collect();
        let mut state = history_state(rows);
        assert_eq!(state.view, TuiView::History);

        assert!(handle_key(&mut state, &key(KeyCode::Char('d'))));
        assert_eq!(state.view, TuiView::Dashboard);
    }

    #[test]
    fn ctrl_d_u_with_odd_row_count_uses_floor_division() {
        let rows: Vec<HistoryRow> = (1..=7).map(row).collect();
        let mut state = history_state(rows);
        assert_eq!(state.selected(), 0);

        assert!(handle_key(&mut state, &ctrl_key('d')));
        assert_eq!(state.selected(), 3);

        assert!(handle_key(&mut state, &ctrl_key('d')));
        assert_eq!(state.selected(), 6);
    }

    #[test]
    fn ctrl_d_u_with_single_row_does_not_panic() {
        let mut state = history_state(vec![row(1)]);
        assert_eq!(state.selected(), 0);

        assert!(handle_key(&mut state, &ctrl_key('d')));
        assert_eq!(state.selected(), 0);

        assert!(handle_key(&mut state, &ctrl_key('u')));
        assert_eq!(state.selected(), 0);
    }

    #[test]
    fn key_3_switches_to_simulation_when_available() {
        let mut state = TuiState::new(vec![row(1)], dashboard());
        assert_eq!(state.view, TuiView::Dashboard);
        assert!(state.simulation.is_none());

        // Key 3 without simulation state should not switch view
        assert!(handle_key(&mut state, &key(KeyCode::Char('3'))));
        assert_eq!(state.view, TuiView::Dashboard);

        // Add simulation state
        state.simulation = Some(SimulationState {
            mode: "forward".to_string(),
            target: "global.conflict".to_string(),
            horizon: 10,
            tick: 5,
            total_ticks: 10,
            status: "running".to_string(),
            field_values: vec![],
            agent_statuses: vec![],
            event_log: vec![],
        });

        // Key 3 should now switch to Simulation view
        assert!(handle_key(&mut state, &key(KeyCode::Char('3'))));
        assert_eq!(state.view, TuiView::Simulation);
    }

    #[test]
    fn simulation_view_esc_returns_to_dashboard() {
        let mut state = TuiState::new(vec![row(1)], dashboard());
        state.simulation = Some(SimulationState {
            mode: "forward".to_string(),
            target: "global.conflict".to_string(),
            horizon: 10,
            tick: 5,
            total_ticks: 10,
            status: "running".to_string(),
            field_values: vec![],
            agent_statuses: vec![],
            event_log: vec![],
        });
        state.view = TuiView::Simulation;

        assert!(handle_key(&mut state, &key(KeyCode::Esc)));
        assert_eq!(state.view, TuiView::Dashboard);
    }

    #[test]
    fn run_demo_simulation_parses_valid_spec() {
        let sim = run_demo_simulation("global.conflict:5");
        assert!(sim.is_some());
        let sim = sim.unwrap();
        assert_eq!(sim.mode, "forward");
        assert_eq!(sim.target, "global.conflict");
        assert_eq!(sim.horizon, 5);
        assert_eq!(sim.total_ticks, 5);
    }

    #[test]
    fn run_demo_simulation_rejects_invalid_spec() {
        // Missing colon
        assert!(run_demo_simulation("global.conflict").is_none());
        // Missing dot in field
        assert!(run_demo_simulation("conflict:30").is_none());
        // Non-numeric horizon
        assert!(run_demo_simulation("global.conflict:abc").is_none());
    }
}
