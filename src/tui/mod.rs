mod compare;
mod dashboard;
mod detail;
mod history;
mod render;
mod simulation;
pub(crate) mod state;
mod theme;

pub use compare::{format_compare, render_compare};
pub use dashboard::{format_dashboard, render_dashboard};
pub use detail::{format_detail, render_detail};
pub use history::{format_history_row, history_title, render_history};
pub use render::{base_style, render};
pub use simulation::{format_simulation, format_simulation_view, render_simulation};
pub use state::{
    array_string_field, bool_field, capitalize_first, compact_json_field, compact_json_value,
    compact_stick_value, compact_timestamp, detect_glyph_mode, format_alert_tier, numeric_field,
    optional_f64_field, placeholder_or_value, signed_numeric_field, string_field, CompareState,
    DashboardState, DetailState, FieldStat, GlyphSet, HistoryRow, HistoryViewState, LoadingState,
    SimAgent, SimAuditAction, SimField, SimReplayFrame, SimulationState, SimulationViewState,
    TopEvent, TuiState, TuiView, ViewState, ASCII_GLYPHS, EMPTY_TUI_MESSAGE, NERD_GLYPHS,
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
use ratatui::layout::Rect;
use ratatui::Terminal;

use crate::storage::{
    get_latest_run_id, get_run_summary, list_runs, EventGroupFilters, RunListFilters,
    ScoredEventFilters,
};
use crate::{delta_memory_path, HotMemory, TianJiError};

pub async fn run_history_browser(
    sqlite_path: Option<&str>,
    limit: usize,
    simulate: Option<&str>,
    interactive: bool,
    trace_jsonl: Option<&str>,
    replay_bundle_dir: Option<&str>,
    render_once: bool,
) -> Result<String, TianJiError> {
    let replay_trace = load_replay_trace(trace_jsonl, replay_bundle_dir)?;

    let sqlite_path = match sqlite_path {
        Some(path) => path.to_string(),
        None => {
            if replay_trace.is_some() {
                "/tmp/tianji-tui-replay-placeholder.sqlite3".to_string()
            } else {
                return Ok(EMPTY_TUI_MESSAGE.to_string());
            }
        }
    };

    if replay_trace.is_none() && !Path::new(&sqlite_path).exists() {
        return Ok(EMPTY_TUI_MESSAGE.to_string());
    }

    let values = if Path::new(&sqlite_path).exists() {
        match list_runs(&sqlite_path, limit, &RunListFilters::default()) {
            Ok(rows) => rows,
            Err(TianJiError::Storage(error)) if is_missing_runs_table(&error) => Vec::new(),
            Err(error) => return Err(error),
        }
    } else {
        Vec::new()
    };
    let rows: Vec<HistoryRow> = values.iter().map(HistoryRow::from_json).collect();
    if rows.is_empty() && replay_trace.is_none() {
        return Ok(EMPTY_TUI_MESSAGE.to_string());
    }

    let memory = HotMemory::load(&delta_memory_path(&sqlite_path));

    let latest_summary = if !rows.is_empty() {
        get_latest_run_id(&sqlite_path)
            .ok()
            .flatten()
            .and_then(|id| {
                get_run_summary(
                    &sqlite_path,
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
    let mut tui_state = TuiState::new_with_storage(rows, dashboard, &sqlite_path);

    if let Some(trace) = replay_trace {
        tui_state.show_simulation(SimulationState::from_trace(&trace));
    }

    // Optionally run a simulation if --simulate was provided
    if let Some(sim_spec) = simulate {
        if interactive {
            let (tx, rx) = tokio::sync::mpsc::channel(64);
            let spec = sim_spec.to_string();
            let maybe = prepare_simulation_sandbox(&spec);
            if let Some((base_wl, agents, mode, config, provider)) = maybe {
                tokio::spawn(async move {
                    crate::nuwa::forward::run_interactive_forward(
                        &base_wl,
                        &agents,
                        &mode,
                        &config,
                        provider.as_ref(),
                        tx,
                        3,
                    )
                    .await;
                });
                tui_state.view = ViewState::Simulation({
                    let mut state = SimulationViewState::new(None);
                    state.pending_sim_rx = Some(rx);
                    state
                });
            }
        } else if let Some(sim_state) = run_demo_simulation(sim_spec).await {
            tui_state.show_simulation(sim_state);
        }
    }

    if render_once {
        return Ok(render_once_text(&tui_state));
    }

    run_terminal(tui_state)?;
    Ok(String::new())
}

fn load_replay_trace(
    trace_jsonl: Option<&str>,
    replay_bundle_dir: Option<&str>,
) -> Result<Option<crate::nuwa::SimulationTrace>, TianJiError> {
    match (trace_jsonl, replay_bundle_dir) {
        (Some(_), Some(_)) => Err(TianJiError::Usage(
            "Use either --trace-jsonl or --replay-bundle-dir for TUI replay loading, not both."
                .to_string(),
        )),
        (Some(path), None) => {
            let trace = crate::nuwa::read_trace_jsonl(path)?;
            validate_trace_integrity(&trace)?;
            Ok(Some(trace))
        }
        (None, Some(dir)) => {
            let dir = Path::new(dir);
            let manifest_path = dir.join(crate::nuwa::REPLAY_BUNDLE_MANIFEST_FILE);
            let manifest_file = std::fs::File::open(&manifest_path)?;
            let manifest: crate::nuwa::ReplayBundleManifest =
                serde_json::from_reader(manifest_file)?;
            if manifest.schema_version != crate::nuwa::REPLAY_BUNDLE_SCHEMA_VERSION {
                return Err(TianJiError::DataIntegrity(format!(
                    "replay bundle schema_version must be {}, got {}",
                    crate::nuwa::REPLAY_BUNDLE_SCHEMA_VERSION,
                    manifest.schema_version
                )));
            }
            if manifest.trace_file != crate::nuwa::REPLAY_BUNDLE_TRACE_FILE
                || manifest.outcome_file != crate::nuwa::REPLAY_BUNDLE_OUTCOME_FILE
            {
                return Err(TianJiError::DataIntegrity(
                    "replay bundle manifest must reference trace.jsonl and outcome.json"
                        .to_string(),
                ));
            }
            let trace_path = dir.join(crate::nuwa::REPLAY_BUNDLE_TRACE_FILE);
            let trace_bytes = std::fs::metadata(&trace_path)?.len();
            if manifest.trace_bytes != trace_bytes {
                return Err(TianJiError::DataIntegrity(format!(
                    "replay bundle trace_bytes mismatch: manifest {} actual {}",
                    manifest.trace_bytes, trace_bytes
                )));
            }
            let outcome_path = dir.join(crate::nuwa::REPLAY_BUNDLE_OUTCOME_FILE);
            let outcome_bytes = std::fs::metadata(&outcome_path)?.len();
            if manifest.outcome_bytes != outcome_bytes {
                return Err(TianJiError::DataIntegrity(format!(
                    "replay bundle outcome_bytes mismatch: manifest {} actual {}",
                    manifest.outcome_bytes, outcome_bytes
                )));
            }
            let outcome_file = std::fs::File::open(outcome_path)?;
            let outcome: crate::nuwa::SimulationOutcome = serde_json::from_reader(outcome_file)?;
            let trace = crate::nuwa::read_trace_jsonl(trace_path)?;
            validate_trace_integrity(&trace)?;
            if manifest.frame_count != trace.frames.len() {
                return Err(TianJiError::DataIntegrity(format!(
                    "replay bundle frame_count mismatch: manifest {} trace {}",
                    manifest.frame_count,
                    trace.frames.len()
                )));
            }
            let outcome_json = serde_json::to_value(&outcome)?;
            let trace_outcome_json = serde_json::to_value(&trace.completed.outcome)?;
            if outcome_json != trace_outcome_json {
                return Err(TianJiError::DataIntegrity(
                    "replay bundle outcome.json does not match trace completed outcome".to_string(),
                ));
            }
            if manifest.mode != trace.metadata.mode
                || manifest.target_field != trace.metadata.target_field
                || manifest.horizon_ticks != trace.metadata.horizon_ticks
            {
                return Err(TianJiError::DataIntegrity(
                    "replay bundle manifest metadata does not match trace metadata".to_string(),
                ));
            }
            Ok(Some(trace))
        }
        (None, None) => Ok(None),
    }
}

fn validate_trace_integrity(trace: &crate::nuwa::SimulationTrace) -> Result<(), TianJiError> {
    if trace.metadata.schema_version != crate::nuwa::SIM_TRACE_SCHEMA_VERSION {
        return Err(TianJiError::DataIntegrity(format!(
            "trace schema_version must be {}, got {}",
            crate::nuwa::SIM_TRACE_SCHEMA_VERSION,
            trace.metadata.schema_version
        )));
    }
    if trace.metadata.frame_count != trace.frames.len() {
        return Err(TianJiError::DataIntegrity(format!(
            "trace metadata frame_count mismatch: metadata {} trace {}",
            trace.metadata.frame_count,
            trace.frames.len()
        )));
    }
    Ok(())
}

fn render_once_text(state: &TuiState) -> String {
    match &state.view {
        ViewState::Dashboard(dashboard) => format_dashboard(dashboard),
        ViewState::History(_) => state
            .rows
            .iter()
            .map(format_history_row)
            .collect::<Vec<_>>()
            .join("\n"),
        ViewState::Detail(detail) => format_detail(detail),
        ViewState::Compare(compare) => format_compare(compare),
        ViewState::Simulation(simulation) => format_simulation_view(simulation),
    }
}

#[allow(clippy::type_complexity)]
fn prepare_simulation_sandbox(
    spec: &str,
) -> Option<(
    crate::worldline::types::Worldline,
    Vec<crate::hongmeng::agent::Agent>,
    crate::nuwa::SimulationMode,
    crate::hongmeng::HongmengConfig,
    Option<crate::llm::ProviderRegistry>,
)> {
    use crate::hongmeng::agent::Agent;
    use crate::hongmeng::HongmengConfig;
    use crate::nuwa::SimulationMode;
    use crate::profile::types::{ActorProfile, ActorTier, Capabilities};
    use crate::worldline::types::{FieldKey, Worldline};
    use std::collections::{BTreeMap, BTreeSet};

    let (field_str, horizon) = spec.rsplit_once(':')?;
    let horizon: u64 = horizon.parse().ok()?;
    let (region, domain) = field_str.rsplit_once('.')?;

    let mut fields = BTreeMap::new();
    fields.insert(
        FieldKey {
            region: region.to_string(),
            domain: domain.to_string(),
        },
        3.5,
    );
    let hash = Worldline::compute_snapshot_hash(&fields);
    let base_worldline = Worldline {
        id: 1,
        fields,
        events: vec![],
        causal_graph: petgraph::graph::DiGraph::new(),
        active_actors: BTreeSet::new(),
        divergence: 0.0,
        parent: None,
        diverge_tick: 0,
        snapshot_hash: hash,
        created_at: chrono::Utc::now(),
    };
    let mode = SimulationMode::Forward {
        target_field: FieldKey {
            region: region.to_string(),
            domain: domain.to_string(),
        },
        horizon_ticks: horizon,
    };
    let config = HongmengConfig::default();
    let profiles = [
        ("usa", vec!["observe", "diplomatic_protest"]),
        ("china", vec!["observe", "naval_exercise"]),
        ("russia", vec!["observe", "diplomatic_protest"]),
    ];
    let agents: Vec<Agent> = profiles
        .iter()
        .map(|(id, pats)| {
            Agent::from_profile(ActorProfile {
                id: id.to_string(),
                name: id.to_string(),
                tier: ActorTier::Nation,
                interests: vec![],
                red_lines: vec![],
                capabilities: Capabilities::default(),
                behavior_patterns: pats.iter().map(|s| s.to_string()).collect(),
                historical_analogues: vec![],
            })
        })
        .collect();
    Some((base_worldline, agents, mode, config, None))
}

fn is_missing_runs_table(error: &rusqlite::Error) -> bool {
    match error {
        rusqlite::Error::SqliteFailure(_, Some(message)) => message.contains("no such table: runs"),
        _ => false,
    }
}

/// Parse a `--simulate` spec like "east-asia.conflict:30" and run a forward simulation.
/// Returns `Some(SimulationState)` on success, `None` on parse or execution failure.
async fn run_demo_simulation(spec: &str) -> Option<SimulationState> {
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

    let outcome = run_forward(&worldline, &agents, &mode, &config, None).await;

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

    // Build agent_statuses from branch events because run_forward keeps action history internal.
    let last_event = primary_branch
        .and_then(|branch| branch.event_sequence.last())
        .cloned()
        .unwrap_or_else(|| "observe".to_string());
    let agent_statuses: Vec<SimAgent> = agents
        .iter()
        .map(|agent| SimAgent {
            actor_id: agent.actor_id.clone(),
            status: "done".to_string(),
            last_action: last_event.clone(),
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
        branches: vec![],
        replay_frames: vec![],
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
            // Poll simulation channel for live updates (non-blocking)
            if let ViewState::Simulation(sim_view) = &mut state.view {
                if let Some(mut rx) = sim_view.pending_sim_rx.take() {
                    while let Ok(update) = rx.try_recv() {
                        match update {
                            crate::nuwa::outcome::SimUpdate::Tick { state: sim_state } => {
                                sim_view.set_sim_state(sim_state);
                            }
                            crate::nuwa::outcome::SimUpdate::PruneRequest {
                                state: sim_state,
                                response,
                            } => {
                                sim_view.set_sim_state(sim_state);
                                sim_view.prune_mode = true;
                                sim_view.pending_prune_tx = Some(response);
                            }
                            crate::nuwa::outcome::SimUpdate::Completed => {}
                        }
                    }
                    sim_view.pending_sim_rx = Some(rx);
                }
            }

            // Poll background detail/compare loading (non-blocking)
            if let Some(ref loading) = state.pending_loading {
                let done = match loading {
                    LoadingState::Detail(rx) => {
                        if let Ok(detail) = rx.try_recv() {
                            state.show_detail(detail);
                            true
                        } else {
                            false
                        }
                    }
                    LoadingState::Compare(rx) => {
                        if let Ok(compare) = rx.try_recv() {
                            state.show_compare(compare);
                            true
                        } else {
                            false
                        }
                    }
                    LoadingState::ImmediateDetail(_) | LoadingState::ImmediateCompare(_) => true,
                };
                if done {
                    state.pending_loading = None;
                }
            }

            self.terminal
                .draw(|frame| self::render::render(frame, &state))?;
            if event::poll(Duration::from_millis(100))? {
                match event::read()? {
                    Event::Key(key) if key.kind == KeyEventKind::Press => {
                        if !handle_key(&mut state, &key) {
                            break;
                        }
                    }
                    Event::Resize(cols, rows) => {
                        self.terminal.resize(Rect::new(0, 0, cols, rows))?;
                    }
                    _ => {}
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
    let dashboard_snapshot = state.dashboard_snapshot();
    let view = std::mem::replace(&mut state.view, ViewState::Dashboard(dashboard_snapshot));
    match view {
        ViewState::Dashboard(dashboard) => {
            state.view = ViewState::Dashboard(dashboard.clone());
            handle_dashboard_key(state, dashboard, key)
        }
        ViewState::History(mut history) => {
            state.view = ViewState::History(history.clone());
            handle_history_key(state, &mut history, key)
        }
        ViewState::Detail(detail) => {
            state.view = ViewState::Detail(detail);
            handle_detail_compare_key(state, key)
        }
        ViewState::Compare(compare) => {
            state.view = ViewState::Compare(compare);
            handle_detail_compare_key(state, key)
        }
        ViewState::Simulation(mut simulation) => handle_simulation_key(state, &mut simulation, key),
    }
}

fn handle_dashboard_key(state: &mut TuiState, dashboard: DashboardState, key: &KeyEvent) -> bool {
    match key.code {
        KeyCode::Char('q') => false,
        KeyCode::Char('h') | KeyCode::Char('H') | KeyCode::Char('2') => {
            state.show_history();
            true
        }
        KeyCode::Char('3') if state.has_simulation() => {
            state.show_existing_simulation();
            true
        }
        _ => {
            state.view = ViewState::Dashboard(dashboard);
            true
        }
    }
}

fn handle_history_key(
    state: &mut TuiState,
    history: &mut HistoryViewState,
    key: &KeyEvent,
) -> bool {
    if history.search_active {
        match key.code {
            KeyCode::Esc => {
                history.search_active = false;
                history.search_query.clear();
                state.rows = history.all_rows.clone();
                state.selected = 0;
            }
            KeyCode::Enter => {
                history.apply_search(&mut state.rows, &mut state.selected);
            }
            KeyCode::Backspace => {
                history.search_query.pop();
            }
            KeyCode::Char(c) => {
                history.search_query.push(c);
            }
            _ => {}
        }
        state.view = ViewState::History(history.clone());
        return true;
    }

    if key.modifiers.contains(KeyModifiers::CONTROL) {
        match key.code {
            KeyCode::Char('d') => {
                let page = state.rows.len().max(1) / 2;
                state.selected = (state.selected + page).min(state.rows.len().saturating_sub(1));
                state.view = ViewState::History(history.clone());
                return true;
            }
            KeyCode::Char('u') => {
                let page = state.rows.len().max(1) / 2;
                state.selected = state.selected.saturating_sub(page);
                state.view = ViewState::History(history.clone());
                return true;
            }
            _ => {}
        }
    }

    match key.code {
        KeyCode::Char('q') => false,
        KeyCode::Esc => {
            history.pending_g = false;
            state.view = ViewState::History(history.clone());
            true
        }
        KeyCode::Enter => {
            state.view = ViewState::History(history.clone());
            if !state.open_selected_compare() {
                state.open_selected_detail();
            }
            true
        }
        KeyCode::Char('d') | KeyCode::Char('D') | KeyCode::Char('1') => {
            history.pending_g = false;
            state.view = ViewState::History(history.clone());
            state.show_dashboard();
            true
        }
        KeyCode::Char('h') | KeyCode::Char('H') | KeyCode::Char('2') => {
            history.pending_g = false;
            state.view = ViewState::History(history.clone());
            true
        }
        KeyCode::Char('3') => {
            if state.has_simulation() {
                history.pending_g = false;
                state.view = ViewState::History(history.clone());
                state.show_existing_simulation();
            } else {
                state.view = ViewState::History(history.clone());
            }
            true
        }
        KeyCode::Char('c') => {
            history.stage_selected_for_compare(&state.rows, state.selected);
            state.view = ViewState::History(history.clone());
            true
        }
        KeyCode::Char('j') | KeyCode::Down => {
            history.pending_g = false;
            state.select_next();
            state.view = ViewState::History(history.clone());
            true
        }
        KeyCode::Char('k') | KeyCode::Up => {
            history.pending_g = false;
            state.select_previous();
            state.view = ViewState::History(history.clone());
            true
        }
        KeyCode::Char('G') | KeyCode::End => {
            history.pending_g = false;
            state.select_last();
            state.view = ViewState::History(history.clone());
            true
        }
        KeyCode::Char('g') => {
            if history.pending_g {
                state.select_first();
                history.pending_g = false;
            } else {
                history.pending_g = true;
            }
            state.view = ViewState::History(history.clone());
            true
        }
        KeyCode::Home => {
            history.pending_g = false;
            state.select_first();
            state.view = ViewState::History(history.clone());
            true
        }
        KeyCode::Char('/') => {
            history.search_active = true;
            history.search_query.clear();
            state.view = ViewState::History(history.clone());
            true
        }
        _ => {
            history.pending_g = false;
            state.view = ViewState::History(history.clone());
            true
        }
    }
}

fn handle_detail_compare_key(state: &mut TuiState, key: &KeyEvent) -> bool {
    match key.code {
        KeyCode::Char('q') => false,
        KeyCode::Esc | KeyCode::Char('h') | KeyCode::Char('H') | KeyCode::Char('2') => {
            state.show_history();
            true
        }
        KeyCode::Char('d') | KeyCode::Char('D') | KeyCode::Char('1') => {
            state.show_dashboard();
            true
        }
        KeyCode::Char('3') if state.has_simulation() => {
            state.show_existing_simulation();
            true
        }
        _ => true,
    }
}

fn handle_simulation_key(
    state: &mut TuiState,
    simulation: &mut SimulationViewState,
    key: &KeyEvent,
) -> bool {
    if simulation.prune_mode {
        match key.code {
            KeyCode::Char(' ') => {
                if simulation.prune_selected.contains(&state.selected) {
                    simulation.prune_selected.retain(|i| *i != state.selected);
                } else {
                    simulation.prune_selected.push(state.selected);
                    simulation.prune_selected.sort_unstable();
                }
            }
            KeyCode::Enter => {
                let decision = if simulation.prune_selected.is_empty() {
                    crate::nuwa::PruningDecision::Continue
                } else {
                    crate::nuwa::PruningDecision::Prune(simulation.prune_selected.clone())
                };
                if let Some(tx) = simulation.pending_prune_tx.take() {
                    let _ = tx.send(decision);
                }
                simulation.prune_mode = false;
                simulation.prune_selected.clear();
            }
            KeyCode::Char('c') | KeyCode::Esc => {
                if let Some(tx) = simulation.pending_prune_tx.take() {
                    let _ = tx.send(crate::nuwa::PruningDecision::Continue);
                }
                simulation.prune_mode = false;
                simulation.prune_selected.clear();
            }
            KeyCode::Char('j') | KeyCode::Down => {
                if let Some(ref sim) = simulation.sim_state {
                    let max = sim.branches.len().saturating_sub(1);
                    state.selected = (state.selected + 1).min(max);
                }
            }
            KeyCode::Char('k') | KeyCode::Up => {
                state.selected = state.selected.saturating_sub(1);
            }
            _ => {}
        }
        state.view = ViewState::Simulation(std::mem::replace(
            simulation,
            SimulationViewState::new(None),
        ));
        return true;
    }

    match key.code {
        KeyCode::Char('q') => false,
        KeyCode::Left | KeyCode::Char('h') => {
            simulation.previous_replay_frame();
            state.view = ViewState::Simulation(std::mem::replace(
                simulation,
                SimulationViewState::new(None),
            ));
            true
        }
        KeyCode::Right | KeyCode::Char('l') => {
            simulation.next_replay_frame();
            state.view = ViewState::Simulation(std::mem::replace(
                simulation,
                SimulationViewState::new(None),
            ));
            true
        }
        KeyCode::Esc | KeyCode::Char('H') | KeyCode::Char('2') => {
            state.view = ViewState::Simulation(std::mem::replace(
                simulation,
                SimulationViewState::new(None),
            ));
            state.show_dashboard();
            true
        }
        _ => {
            state.view = ViewState::Simulation(std::mem::replace(
                simulation,
                SimulationViewState::new(None),
            ));
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

    fn simulation_state() -> SimulationState {
        SimulationState {
            mode: "forward".to_string(),
            target: "global.conflict".to_string(),
            horizon: 10,
            tick: 5,
            total_ticks: 10,
            status: "running".to_string(),
            field_values: vec![],
            agent_statuses: vec![],
            event_log: vec![],
            branches: vec![],
            replay_frames: vec![],
        }
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
    fn history_staging_survives_dashboard_roundtrip() {
        let mut state = history_state(vec![row(1), row(2)]);

        assert!(handle_key(&mut state, &key(KeyCode::Char('c'))));
        assert_eq!(state.history().unwrap().staged_left_run_id, Some(1));

        assert!(handle_key(&mut state, &key(KeyCode::Char('d'))));
        assert_eq!(state.view, TuiView::Dashboard);

        assert!(handle_key(&mut state, &key(KeyCode::Char('h'))));
        assert_eq!(state.view, TuiView::History);
        assert_eq!(state.history().unwrap().staged_left_run_id, Some(1));
        assert!(history_title(&state).contains("staged left #1"));
    }

    #[test]
    fn history_staging_survives_simulation_roundtrip() {
        let mut state = history_state(vec![row(1), row(2)]);
        state.show_simulation(simulation_state());
        assert!(handle_key(&mut state, &key(KeyCode::Esc)));
        assert!(handle_key(&mut state, &key(KeyCode::Char('h'))));

        assert!(handle_key(&mut state, &key(KeyCode::Char('c'))));
        assert_eq!(state.history().unwrap().staged_left_run_id, Some(1));

        assert!(handle_key(&mut state, &key(KeyCode::Char('3'))));
        assert_eq!(state.view, TuiView::Simulation);

        assert!(handle_key(&mut state, &key(KeyCode::Esc)));
        assert!(handle_key(&mut state, &key(KeyCode::Char('h'))));
        assert_eq!(state.view, TuiView::History);
        assert_eq!(state.history().unwrap().staged_left_run_id, Some(1));
    }

    #[test]
    fn key_handler_opens_detail_from_history_and_returns_with_escape_or_h() {
        let mut state = history_state(vec![row(1), row(2)]);
        state.select_next();

        assert!(handle_key(&mut state, &key(KeyCode::Enter)));
        assert_eq!(state.view, TuiView::Detail);
        assert_eq!(state.detail().map(|d| d.run_id), Some(2));
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
        assert_eq!(state.history().unwrap().staged_left_run_id, Some(1));
        assert!(history_title(&state).contains("staged left #1"));

        assert!(handle_key(&mut state, &key(KeyCode::Char('j'))));
        assert_eq!(state.selected(), 1);
        assert!(handle_key(&mut state, &key(KeyCode::Enter)));

        assert_eq!(state.view, TuiView::Compare);
        let compare = state.compare().expect("compare state");
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
        assert_eq!(state.detail().map(|d| d.run_id), Some(2));
        assert!(state.compare().is_none());
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
        assert!(!state.history().unwrap().search_active);
        assert!(handle_key(&mut state, &key(KeyCode::Char('/'))));
        assert!(state.history().unwrap().search_active);
        assert!(state.history_mut().search_query.is_empty());
    }

    #[test]
    fn key_handler_search_type_and_submit() {
        let mut state = history_state(vec![row(1), row(2)]);
        assert!(handle_key(&mut state, &key(KeyCode::Char('/'))));
        assert!(handle_key(&mut state, &key(KeyCode::Char('t'))));
        assert!(handle_key(&mut state, &key(KeyCode::Char('e'))));
        assert!(handle_key(&mut state, &key(KeyCode::Char('c'))));
        assert!(handle_key(&mut state, &key(KeyCode::Char('h'))));
        assert_eq!(state.history_mut().search_query, "tech");
        assert!(state.history().unwrap().search_active);
        assert!(handle_key(&mut state, &key(KeyCode::Enter)));
        assert!(!state.history().unwrap().search_active);
        assert!(!state.rows.is_empty());
    }

    #[test]
    fn key_handler_esc_clears_search_and_restores() {
        let mut state = history_state(vec![row(1), row(2)]);
        assert!(handle_key(&mut state, &key(KeyCode::Char('/'))));
        assert!(handle_key(&mut state, &key(KeyCode::Char('z'))));
        assert!(handle_key(&mut state, &key(KeyCode::Esc)));
        assert!(!state.history().unwrap().search_active);
        assert!(state.history_mut().search_query.is_empty());
        assert_eq!(state.rows.len(), state.history().unwrap().all_rows.len());
    }

    #[test]
    fn key_handler_backspace_in_search() {
        let mut state = history_state(vec![row(1), row(2)]);
        assert!(handle_key(&mut state, &key(KeyCode::Char('/'))));
        assert!(handle_key(&mut state, &key(KeyCode::Char('a'))));
        assert!(handle_key(&mut state, &key(KeyCode::Char('b'))));
        assert_eq!(state.history_mut().search_query, "ab");
        assert!(handle_key(&mut state, &key(KeyCode::Backspace)));
        assert_eq!(state.history_mut().search_query, "a");
    }

    #[test]
    fn key_handler_search_blocks_quit_when_active() {
        let mut state = history_state(vec![row(1)]);
        assert!(handle_key(&mut state, &key(KeyCode::Char('/'))));
        assert!(handle_key(&mut state, &key(KeyCode::Char('q'))));
        assert_eq!(state.history_mut().search_query, "q");
        assert!(state.history().unwrap().search_active);
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
        assert!(state.simulation().is_none());

        // Key 3 without simulation state should not switch view
        assert!(handle_key(&mut state, &key(KeyCode::Char('3'))));
        assert_eq!(state.view, TuiView::Dashboard);

        // Add simulation state
        state.show_simulation(simulation_state());

        // Key 3 should now switch to Simulation view
        assert!(handle_key(&mut state, &key(KeyCode::Char('3'))));
        assert_eq!(state.view, TuiView::Simulation);
    }

    #[test]
    fn simulation_view_esc_returns_to_dashboard() {
        let mut state = TuiState::new(vec![row(1)], dashboard());
        state.show_simulation(simulation_state());

        assert!(handle_key(&mut state, &key(KeyCode::Esc)));
        assert_eq!(state.view, TuiView::Dashboard);
    }

    #[test]
    fn simulation_key_handler_moves_replay_cursor() {
        let mut state = TuiState::new(vec![row(1)], dashboard());
        state.show_simulation(simulation_state());
        let ViewState::Simulation(simulation) = &state.view else {
            panic!("expected simulation view");
        };
        assert_eq!(simulation.replay_cursor, 4);

        assert!(handle_key(&mut state, &key(KeyCode::Left)));
        let ViewState::Simulation(simulation) = &state.view else {
            panic!("expected simulation view");
        };
        assert_eq!(simulation.replay_cursor, 3);

        assert!(handle_key(&mut state, &key(KeyCode::Char('l'))));
        let ViewState::Simulation(simulation) = &state.view else {
            panic!("expected simulation view");
        };
        assert_eq!(simulation.replay_cursor, 4);
    }

    #[tokio::test]
    async fn tui_render_once_loads_trace_jsonl() {
        use crate::nuwa::trace::{
            write_trace_jsonl, SimulationTrace, SimulationTraceCompleted, SimulationTraceFrame,
            SimulationTraceMetadata, TraceAgentAction, SIM_TRACE_SCHEMA_VERSION,
        };
        use crate::nuwa::{ConvergenceReason, SimulationMode, SimulationOutcome};
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
                field_values: BTreeMap::from([(field.clone(), 4.25)]),
                field_changes: vec![crate::hongmeng::referee::FieldChange {
                    region: "global".to_string(),
                    domain: "conflict".to_string(),
                    delta: 0.75,
                }],
                agent_actions: vec![TraceAgentAction {
                    actor_id: "actor-a".to_string(),
                    action_type: "observe".to_string(),
                    target: None,
                    confidence: 0.8,
                    rationale: "trace rationale".to_string(),
                    assessment: "trace assessment".to_string(),
                    category: "trace_category".to_string(),
                    drivers: vec!["trace_driver".to_string()],
                }],
                event_sequence_len: 7,
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
        let path = std::env::temp_dir().join(format!(
            "tianji_tui_trace_render_once_{}_{}.jsonl",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("time")
                .as_nanos()
        ));
        write_trace_jsonl(&path, &trace).expect("write trace");

        let rendered = run_history_browser(
            Some("/tmp/tianji-missing-for-trace-render.sqlite3"),
            20,
            None,
            false,
            path.to_str(),
            None,
            true,
        )
        .await
        .expect("render trace");
        let _ = std::fs::remove_file(&path);

        assert!(rendered.contains("frame 1/1"));
        assert!(rendered.contains("replay controls: Left/h previous frame"));
        assert!(rendered.contains("audit coverage: 1 action(s), 1 driver signal(s)"));
        assert!(rendered.contains("event sequence length 7"));
        assert!(rendered.contains("Agent audit"));
        assert!(rendered.contains("assessment: trace assessment"));
    }

    #[tokio::test]
    async fn tui_replay_bundle_rejects_manifest_mismatch() {
        use crate::nuwa::trace::{
            write_replay_bundle_dir, SimulationTrace, SimulationTraceCompleted,
            SimulationTraceFrame, SimulationTraceMetadata, SIM_TRACE_SCHEMA_VERSION,
        };
        use crate::nuwa::{ConvergenceReason, SimulationMode, SimulationOutcome};
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
                field_values: BTreeMap::from([(field.clone(), 4.25)]),
                field_changes: vec![],
                agent_actions: vec![],
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
        let dir = std::env::temp_dir().join(format!(
            "tianji_tui_bundle_bad_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("time")
                .as_nanos()
        ));
        let mut manifest = write_replay_bundle_dir(&dir, &trace).expect("write bundle");
        manifest.frame_count = 99;
        let manifest_path = dir.join(crate::nuwa::REPLAY_BUNDLE_MANIFEST_FILE);
        serde_json::to_writer_pretty(
            std::fs::File::create(&manifest_path).expect("manifest"),
            &manifest,
        )
        .expect("rewrite manifest");

        let err = run_history_browser(None, 20, None, false, None, dir.to_str(), true)
            .await
            .expect_err("bad manifest rejected");
        let _ = std::fs::remove_dir_all(&dir);

        assert!(
            matches!(err, TianJiError::DataIntegrity(message) if message.contains("frame_count"))
        );
    }

    #[tokio::test]
    async fn run_demo_simulation_parses_valid_spec() {
        let sim = run_demo_simulation("global.conflict:5").await;
        assert!(sim.is_some());
        let sim = sim.unwrap();
        assert_eq!(sim.mode, "forward");
        assert_eq!(sim.target, "global.conflict");
        assert_eq!(sim.horizon, 5);
        assert_eq!(sim.total_ticks, 5);
    }

    #[tokio::test]
    async fn run_demo_simulation_rejects_invalid_spec() {
        // Missing colon
        assert!(run_demo_simulation("global.conflict").await.is_none());
        // Missing dot in field
        assert!(run_demo_simulation("conflict:30").await.is_none());
        // Non-numeric horizon
        assert!(run_demo_simulation("global.conflict:abc").await.is_none());
    }
}
