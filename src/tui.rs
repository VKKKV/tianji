use std::io::{self, Stdout};
use std::path::Path;
use std::time::Duration;

use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, List, ListItem, ListState, Paragraph};
use ratatui::{Frame, Terminal};

use crate::storage::{
    compare_runs, get_latest_run_id, get_run_summary, list_runs, CompareResult, EventGroupFilters,
    RunListFilters, ScoredEventFilters,
};
use crate::{classify_delta_tier, delta_memory_path, AlertTier, HotMemory, TianJiError};

pub const EMPTY_TUI_MESSAGE: &str = "No persisted runs are available for the TUI browser.";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Theme {
    pub bg: Color,
    pub panel_bg: Color,
    pub border: Color,
    pub fg: Color,
    pub label: Color,
    pub value: Color,
    pub up: Color,
    pub down: Color,
    pub warn: Color,
    pub status_bg: Color,
    pub key_hint: Color,
    pub title: Color,
}

pub const KANAGAWA: Theme = Theme {
    bg: Color::Rgb(0x1F, 0x1F, 0x28),
    panel_bg: Color::Rgb(0x27, 0x27, 0x27),
    border: Color::Rgb(0x36, 0x36, 0x46),
    fg: Color::Rgb(0xDC, 0xD7, 0xBA),
    label: Color::Rgb(0x7E, 0x9C, 0xD8),
    value: Color::Rgb(0xDC, 0xD7, 0xBA),
    up: Color::Rgb(0x98, 0xBB, 0x6C),
    down: Color::Rgb(0xE4, 0x68, 0x76),
    warn: Color::Rgb(0xFF, 0xA0, 0x66),
    status_bg: Color::Rgb(0x36, 0x36, 0x46),
    key_hint: Color::Rgb(0x93, 0x8A, 0xA9),
    title: Color::Rgb(0xE6, 0xC3, 0x84),
};

#[derive(Debug, Clone, PartialEq)]
pub struct HistoryRow {
    pub run_id: i64,
    pub generated_at: String,
    pub mode: String,
    pub dominant_field: String,
    pub risk_level: String,
    pub top_divergence_score: Option<f64>,
    pub headline: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FieldStat {
    pub field: String,
    pub count: usize,
    pub avg_impact: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TopEvent {
    pub title: String,
    pub impact_score: f64,
    pub dominant_field: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DashboardState {
    // Run metadata
    pub latest_run_id: String,
    pub latest_generated_at: String,
    pub latest_mode: String,
    pub headline: String,
    // Field breakdown
    pub field_summary: Vec<FieldStat>,
    pub total_scored_events: usize,
    // Top events
    pub top_events: Vec<TopEvent>,
    // Delta
    pub alert_tier: String,
    pub delta_summary: String,
    pub delta_direction: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DetailState {
    pub run_id: i64,
    pub status: String,
    pub schema_version: String,
    pub mode: String,
    pub generated_at: String,
    pub input_summary: String,
    pub scenario_summary: String,
    pub scored_events: Vec<String>,
    pub event_groups: Vec<String>,
    pub intervention_candidates: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CompareState {
    pub left_run_id: i64,
    pub right_run_id: i64,
    pub status: String,
    pub left_summary: Vec<String>,
    pub right_summary: Vec<String>,
    pub diff_lines: Vec<String>,
}

impl CompareState {
    pub fn missing(left_run_id: i64, right_run_id: i64) -> Self {
        Self {
            left_run_id,
            right_run_id,
            status: format!("Compare pair #{left_run_id} → #{right_run_id} could not be loaded."),
            left_summary: vec!["No left run summary available.".to_string()],
            right_summary: vec!["No right run summary available.".to_string()],
            diff_lines: vec!["No diff available.".to_string()],
        }
    }

    pub fn invalid(left_run_id: i64, right_run_id: i64, message: impl Into<String>) -> Self {
        let mut state = Self::missing(left_run_id, right_run_id);
        state.status = message.into();
        state
    }

    pub fn error(left_run_id: i64, right_run_id: i64, message: impl Into<String>) -> Self {
        let mut state = Self::missing(left_run_id, right_run_id);
        state.status = format!(
            "Compare pair #{left_run_id} → #{right_run_id} error: {}",
            message.into()
        );
        state
    }

    pub fn from_result(result: &CompareResult) -> Self {
        Self {
            left_run_id: result.left_run_id,
            right_run_id: result.right_run_id,
            status: "loaded".to_string(),
            left_summary: format_compare_side_lines(&result.left),
            right_summary: format_compare_side_lines(&result.right),
            diff_lines: format_compare_diff_lines(&result.diff),
        }
    }
}

impl DetailState {
    pub fn missing(run_id: i64) -> Self {
        Self {
            run_id,
            status: format!("Run #{run_id} could not be loaded."),
            schema_version: "unavailable".to_string(),
            mode: "unavailable".to_string(),
            generated_at: "unavailable".to_string(),
            input_summary: "No input summary available.".to_string(),
            scenario_summary: "No scenario summary available.".to_string(),
            scored_events: vec!["No scored events available.".to_string()],
            event_groups: vec!["No event groups available.".to_string()],
            intervention_candidates: vec!["No intervention candidates available.".to_string()],
        }
    }

    pub fn error(run_id: i64, message: impl Into<String>) -> Self {
        let mut state = Self::missing(run_id);
        state.status = format!("Run #{run_id} detail error: {}", message.into());
        state
    }

    pub fn from_json(value: &serde_json::Value) -> Self {
        let run_id = value.get("run_id").and_then(|v| v.as_i64()).unwrap_or(0);
        let schema_version = string_field(value, "schema_version", "unavailable");
        let mode = string_field(value, "mode", "unavailable");
        let generated_at = compact_timestamp(&string_field(value, "generated_at", "unavailable"));
        let input_summary_value = value
            .get("input_summary")
            .cloned()
            .unwrap_or(serde_json::Value::Null);
        let scenario_summary_value = value
            .get("scenario_summary")
            .cloned()
            .unwrap_or(serde_json::Value::Null);
        let scored_events = format_scored_event_lines(value.get("scored_events"));
        let event_groups = format_event_group_lines(scenario_summary_value.get("event_groups"));
        let intervention_candidates =
            format_intervention_lines(value.get("intervention_candidates"));

        Self {
            run_id,
            status: "loaded".to_string(),
            schema_version,
            mode,
            generated_at,
            input_summary: format_summary_value(&input_summary_value),
            scenario_summary: format_scenario_summary(&scenario_summary_value),
            scored_events,
            event_groups,
            intervention_candidates,
        }
    }
}

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

        let (headline, field_summary, total_scored_events, top_events) =
            if let Some(ref summary) = run_summary {
                let hl = summary
                    .get("scenario_summary")
                    .and_then(|v| v.get("headline"))
                    .and_then(|v| v.as_str())
                    .filter(|s| !s.trim().is_empty())
                    .map(|s| s.to_string())
                    .or_else(|| {
                        latest
                            .map(|row| placeholder_or_value(&row.headline, "No headline available."))
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
                let mut events_for_top: Vec<TopEvent> = scored_events
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
                        Some(TopEvent {
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TuiView {
    Dashboard,
    History,
    Detail,
    Compare,
}

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

#[derive(Debug, Clone)]
pub struct TuiState {
    pub rows: Vec<HistoryRow>,
    pub dashboard: DashboardState,
    pub view: TuiView,
    pub detail: Option<DetailState>,
    pub compare: Option<CompareState>,
    pub staged_left_run_id: Option<i64>,
    sqlite_path: Option<String>,
    selected: usize,
    pending_g: bool,
}

impl TuiState {
    pub fn new(rows: Vec<HistoryRow>, dashboard: DashboardState) -> Self {
        Self {
            rows,
            dashboard,
            view: TuiView::Dashboard,
            detail: None,
            compare: None,
            staged_left_run_id: None,
            sqlite_path: None,
            selected: 0,
            pending_g: false,
        }
    }

    pub fn new_with_storage(
        rows: Vec<HistoryRow>,
        dashboard: DashboardState,
        sqlite_path: impl Into<String>,
    ) -> Self {
        let mut state = Self::new(rows, dashboard);
        state.sqlite_path = Some(sqlite_path.into());
        state
    }

    pub fn selected(&self) -> usize {
        self.selected
    }

    pub fn select_next(&mut self) {
        if self.rows.is_empty() {
            self.selected = 0;
        } else {
            self.selected = (self.selected + 1).min(self.rows.len() - 1);
        }
    }

    pub fn select_previous(&mut self) {
        self.selected = self.selected.saturating_sub(1);
    }

    pub fn select_first(&mut self) {
        self.selected = 0;
    }

    pub fn select_last(&mut self) {
        self.selected = self.rows.len().saturating_sub(1);
    }

    pub fn show_dashboard(&mut self) {
        self.pending_g = false;
        self.view = TuiView::Dashboard;
    }

    pub fn show_history(&mut self) {
        self.pending_g = false;
        self.view = TuiView::History;
    }

    pub fn show_detail(&mut self, detail: DetailState) {
        self.pending_g = false;
        self.detail = Some(detail);
        self.view = TuiView::Detail;
    }

    pub fn show_compare(&mut self, compare: CompareState) {
        self.pending_g = false;
        self.compare = Some(compare);
        self.view = TuiView::Compare;
    }

    pub fn stage_selected_for_compare(&mut self) {
        self.pending_g = false;
        if self.view != TuiView::History {
            return;
        }
        self.staged_left_run_id = self.rows.get(self.selected).map(|row| row.run_id);
    }

    pub fn open_selected_detail(&mut self) {
        self.pending_g = false;
        if self.view != TuiView::History {
            return;
        }
        let Some(row) = self.rows.get(self.selected) else {
            return;
        };
        let detail = match self.sqlite_path.as_deref() {
            Some(sqlite_path) => load_detail_state(sqlite_path, row.run_id),
            None => DetailState::missing(row.run_id),
        };
        self.show_detail(detail);
    }

    pub fn open_selected_compare(&mut self) -> bool {
        self.pending_g = false;
        if self.view != TuiView::History {
            return false;
        }
        let Some(left_run_id) = self.staged_left_run_id else {
            return false;
        };
        let Some(right_row) = self.rows.get(self.selected) else {
            return false;
        };
        let right_run_id = right_row.run_id;
        let compare = match self.sqlite_path.as_deref() {
            Some(sqlite_path) => load_compare_state(sqlite_path, left_run_id, right_run_id),
            None => CompareState::missing(left_run_id, right_run_id),
        };
        self.show_compare(compare);
        true
    }

    fn list_state(&self) -> ListState {
        let mut state = ListState::default();
        if !self.rows.is_empty() {
            state.select(Some(self.selected));
        }
        state
    }
}

pub fn run_history_browser(sqlite_path: &str, limit: usize) -> Result<String, TianJiError> {
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
    run_terminal(TuiState::new_with_storage(rows, dashboard, sqlite_path))?;
    Ok(String::new())
}

fn is_missing_runs_table(error: &rusqlite::Error) -> bool {
    match error {
        rusqlite::Error::SqliteFailure(_, Some(message)) => message.contains("no such table: runs"),
        _ => false,
    }
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
            self.terminal.draw(|frame| render(frame, &state))?;
            if event::poll(Duration::from_millis(100))? {
                let Event::Key(key) = event::read()? else {
                    continue;
                };
                if key.kind != KeyEventKind::Press {
                    continue;
                }
                if !handle_key_code(&mut state, key.code) {
                    break;
                }
            }
        }
        Ok(())
    }
}

fn handle_key_code(state: &mut TuiState, code: KeyCode) -> bool {
    match code {
        KeyCode::Char('q') => false,
        KeyCode::Esc => {
            if matches!(state.view, TuiView::Detail | TuiView::Compare) {
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
        _ => {
            state.pending_g = false;
            true
        }
    }
}

impl Drop for TerminalSession {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let _ = execute!(self.terminal.backend_mut(), LeaveAlternateScreen);
        let _ = self.terminal.show_cursor();
    }
}

fn render(frame: &mut Frame<'_>, state: &TuiState) {
    let root = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(5),
            Constraint::Length(2),
        ])
        .split(frame.area());

    frame.render_widget(
        Paragraph::new(title_line(state)).style(base_style()),
        root[0],
    );

    match state.view {
        TuiView::Dashboard => render_dashboard(frame, root[1], &state.dashboard),
        TuiView::History => render_history(frame, root[1], state),
        TuiView::Detail => render_detail(frame, root[1], state.detail.as_ref()),
        TuiView::Compare => render_compare(frame, root[1], state.compare.as_ref()),
    }

    let mut spans = vec![
        Span::styled("read-only ", Style::default().fg(KANAGAWA.label)),
        Span::styled("[d]", Style::default().fg(KANAGAWA.key_hint)),
        Span::raw(" dashboard  "),
        Span::styled("[h]", Style::default().fg(KANAGAWA.key_hint)),
        Span::raw(" history  "),
    ];
    if state.view == TuiView::History {
        spans.extend(vec![
            Span::styled("[Enter]", Style::default().fg(KANAGAWA.key_hint)),
            Span::raw(if state.staged_left_run_id.is_some() {
                " compare  "
            } else {
                " detail  "
            }),
            Span::styled("[c]", Style::default().fg(KANAGAWA.key_hint)),
            Span::raw(" stage compare  "),
            Span::styled("[j/k]", Style::default().fg(KANAGAWA.key_hint)),
            Span::raw(" move  "),
            Span::styled("[↑/↓]", Style::default().fg(KANAGAWA.key_hint)),
            Span::raw(" move  "),
            Span::styled("[gg/G]", Style::default().fg(KANAGAWA.key_hint)),
            Span::raw(" first/last  "),
        ]);
    } else if matches!(state.view, TuiView::Detail | TuiView::Compare) {
        spans.extend(vec![
            Span::styled("[Esc]", Style::default().fg(KANAGAWA.key_hint)),
            Span::raw("/"),
            Span::styled("[h]", Style::default().fg(KANAGAWA.key_hint)),
            Span::raw(" back  "),
        ]);
    }
    spans.extend(vec![
        Span::styled("[q]", Style::default().fg(KANAGAWA.key_hint)),
        Span::raw(" quit"),
    ]);
    let status = Paragraph::new(Line::from(spans)).style(base_style().bg(KANAGAWA.status_bg));
    frame.render_widget(status, root[2]);
}

fn render_history(frame: &mut Frame<'_>, area: ratatui::layout::Rect, state: &TuiState) {
    let list_items: Vec<ListItem<'_>> = state
        .rows
        .iter()
        .map(|row| ListItem::new(format_history_row(row)).style(base_style()))
        .collect();
    let list = List::new(list_items)
        .block(
            Block::bordered()
                .title(history_title(state))
                .border_style(Style::default().fg(KANAGAWA.border))
                .style(base_style().bg(KANAGAWA.panel_bg)),
        )
        .highlight_style(
            Style::default()
                .fg(KANAGAWA.title)
                .bg(KANAGAWA.border)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("> ");
    let mut list_state = state.list_state();
    frame.render_stateful_widget(list, area, &mut list_state);
}

fn history_title(state: &TuiState) -> String {
    match state.staged_left_run_id {
        Some(run_id) => format!(" Run History · staged left #{run_id} "),
        None => " Run History ".to_string(),
    }
}

fn render_dashboard(
    frame: &mut Frame<'_>,
    area: ratatui::layout::Rect,
    dashboard: &DashboardState,
) {
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
        Span::styled(
            dashboard.headline.clone(),
            Style::default().fg(KANAGAWA.fg),
        ),
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
        let event_word = if stat.count == 1 {
            "event "
        } else {
            "events"
        };
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
                .style(base_style().bg(KANAGAWA.panel_bg)),
        )
        .style(base_style());
    frame.render_widget(paragraph, area);
}

fn render_detail(frame: &mut Frame<'_>, area: ratatui::layout::Rect, detail: Option<&DetailState>) {
    let text = detail
        .map(format_detail)
        .unwrap_or_else(|| "No detail loaded.".to_string());
    let paragraph = Paragraph::new(text)
        .block(
            Block::bordered()
                .title(" Run Detail ")
                .border_style(Style::default().fg(KANAGAWA.border))
                .style(base_style().bg(KANAGAWA.panel_bg)),
        )
        .style(base_style());
    frame.render_widget(paragraph, area);
}

fn render_compare(
    frame: &mut Frame<'_>,
    area: ratatui::layout::Rect,
    compare: Option<&CompareState>,
) {
    let text = compare
        .map(format_compare)
        .unwrap_or_else(|| "No compare loaded.".to_string());
    let paragraph = Paragraph::new(text)
        .block(
            Block::bordered()
                .title(" Run Compare ")
                .border_style(Style::default().fg(KANAGAWA.border))
                .style(base_style().bg(KANAGAWA.panel_bg)),
        )
        .style(base_style());
    frame.render_widget(paragraph, area);
}

fn base_style() -> Style {
    Style::default().fg(KANAGAWA.fg).bg(KANAGAWA.bg)
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
            let event_word = if stat.count == 1 {
                "event "
            } else {
                "events"
            };
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

fn load_detail_state(sqlite_path: &str, run_id: i64) -> DetailState {
    if !Path::new(sqlite_path).exists() {
        return DetailState::missing(run_id);
    }

    match get_run_summary(
        sqlite_path,
        run_id,
        &ScoredEventFilters::default(),
        false,
        &EventGroupFilters::default(),
    ) {
        Ok(Some(value)) => DetailState::from_json(&value),
        Ok(None) => DetailState::missing(run_id),
        Err(error) => DetailState::error(run_id, error.to_string()),
    }
}

fn load_compare_state(sqlite_path: &str, left_run_id: i64, right_run_id: i64) -> CompareState {
    if left_run_id == right_run_id {
        return CompareState::invalid(
            left_run_id,
            right_run_id,
            format!("Choose a different right run before comparing staged run #{left_run_id}."),
        );
    }
    if !Path::new(sqlite_path).exists() {
        return CompareState::missing(left_run_id, right_run_id);
    }

    match compare_runs(
        sqlite_path,
        left_run_id,
        right_run_id,
        &ScoredEventFilters::default(),
        false,
        &EventGroupFilters::default(),
    ) {
        Ok(Some(result)) => CompareState::from_result(&result),
        Ok(None) => CompareState::missing(left_run_id, right_run_id),
        Err(error) => CompareState::error(left_run_id, right_run_id, error.to_string()),
    }
}

fn format_compare_side_lines(value: &serde_json::Value) -> Vec<String> {
    let top_scored_event_id = value
        .get("top_scored_event")
        .and_then(|event| event.get("event_id"))
        .and_then(|v| v.as_str())
        .unwrap_or("none");
    let top_event_group_id = value
        .get("top_event_group")
        .and_then(|group| group.get("headline_event_id"))
        .and_then(|v| v.as_str())
        .unwrap_or("none");
    vec![
        format!("run: #{}", numeric_field(value, "run_id")),
        format!(
            "schema: {}",
            compact_json_field(value, "schema_version", "unavailable")
        ),
        format!("mode: {}", compact_json_field(value, "mode", "unavailable")),
        format!(
            "dominant field: {}",
            compact_json_field(value, "dominant_field", "uncategorized")
        ),
        format!(
            "risk: {}",
            compact_json_field(value, "risk_level", "unknown")
        ),
        format!(
            "headline: {}",
            compact_json_field(value, "headline", "No headline available.")
        ),
        format!(
            "raw/normalized: {}/{}",
            numeric_field(value, "raw_item_count"),
            numeric_field(value, "normalized_event_count")
        ),
        format!(
            "event groups: {} (top {top_event_group_id})",
            numeric_field(value, "event_group_count")
        ),
        format!("top scored event: {top_scored_event_id}"),
    ]
}

fn format_compare_diff_lines(value: &serde_json::Value) -> Vec<String> {
    if value
        .as_object()
        .map(|object| object.is_empty())
        .unwrap_or(true)
    {
        return vec!["No diff available.".to_string()];
    }
    vec![
        format!(
            "raw item delta: {}",
            signed_numeric_field(value, "raw_item_count_delta")
        ),
        format!(
            "normalized event delta: {}",
            signed_numeric_field(value, "normalized_event_count_delta")
        ),
        format!(
            "event group delta: {}",
            signed_numeric_field(value, "event_group_count_delta")
        ),
        format!(
            "dominant field changed: {}",
            bool_field(value, "dominant_field_changed")
        ),
        format!(
            "risk level changed: {}",
            bool_field(value, "risk_level_changed")
        ),
        format!(
            "top event group changed: {}",
            bool_field(value, "top_event_group_changed")
        ),
        format!(
            "top scored event changed: {}",
            bool_field(value, "top_scored_event_changed")
        ),
        format!(
            "top scored event comparable: {}",
            bool_field(value, "top_scored_event_comparable")
        ),
        format!(
            "top intervention changed: {}",
            bool_field(value, "top_intervention_changed")
        ),
        format!(
            "top divergence delta: {}",
            optional_f64_field(value, "top_divergence_score_delta")
        ),
        format!(
            "left-only event groups: {}",
            array_string_field(value, "left_only_event_group_headline_event_ids")
        ),
        format!(
            "right-only event groups: {}",
            array_string_field(value, "right_only_event_group_headline_event_ids")
        ),
        format!(
            "left-only interventions: {}",
            array_string_field(value, "left_only_intervention_event_ids")
        ),
        format!(
            "right-only interventions: {}",
            array_string_field(value, "right_only_intervention_event_ids")
        ),
    ]
}

fn compact_json_field(value: &serde_json::Value, key: &str, placeholder: &str) -> String {
    value
        .get(key)
        .map(compact_json_value)
        .filter(|text| !text.trim().is_empty() && text != "null")
        .unwrap_or_else(|| placeholder.to_string())
}

fn numeric_field(value: &serde_json::Value, key: &str) -> String {
    value
        .get(key)
        .and_then(|v| v.as_i64().or_else(|| v.as_u64().map(|n| n as i64)))
        .map(|number| number.to_string())
        .unwrap_or_else(|| "0".to_string())
}

fn signed_numeric_field(value: &serde_json::Value, key: &str) -> String {
    value
        .get(key)
        .and_then(|v| v.as_i64())
        .map(|number| format!("{number:+}"))
        .unwrap_or_else(|| "unavailable".to_string())
}

fn bool_field(value: &serde_json::Value, key: &str) -> String {
    value
        .get(key)
        .and_then(|v| v.as_bool())
        .map(|flag| flag.to_string())
        .unwrap_or_else(|| "unavailable".to_string())
}

fn optional_f64_field(value: &serde_json::Value, key: &str) -> String {
    value
        .get(key)
        .and_then(|v| v.as_f64())
        .map(|number| format!("{number:+.6}"))
        .unwrap_or_else(|| "unavailable".to_string())
}

fn array_string_field(value: &serde_json::Value, key: &str) -> String {
    let items: Vec<String> = value
        .get(key)
        .and_then(|v| v.as_array())
        .map(|array| {
            array
                .iter()
                .filter_map(|item| item.as_str().map(ToString::to_string))
                .collect()
        })
        .unwrap_or_default();
    if items.is_empty() {
        "none".to_string()
    } else {
        items.join(", ")
    }
}

fn string_field(value: &serde_json::Value, key: &str, placeholder: &str) -> String {
    value
        .get(key)
        .and_then(|v| v.as_str())
        .map(|s| placeholder_or_value(s, placeholder))
        .unwrap_or_else(|| placeholder.to_string())
}

fn format_summary_value(value: &serde_json::Value) -> String {
    if value.is_null() {
        return "No summary available.".to_string();
    }
    let Some(object) = value.as_object() else {
        return value.to_string();
    };
    if object.is_empty() {
        return "No summary available.".to_string();
    }
    object
        .iter()
        .map(|(key, value)| format!("{key}: {}", compact_json_value(value)))
        .collect::<Vec<_>>()
        .join("; ")
}

fn format_scenario_summary(value: &serde_json::Value) -> String {
    if value.is_null() {
        return "No scenario summary available.".to_string();
    }
    let Some(object) = value.as_object() else {
        return value.to_string();
    };
    let fields = ["headline", "dominant_field", "risk_level"];
    let parts: Vec<String> = fields
        .iter()
        .filter_map(|key| {
            object
                .get(*key)
                .map(|value| format!("{key}: {}", compact_json_value(value)))
        })
        .collect();
    if parts.is_empty() {
        format_summary_value(value)
    } else {
        parts.join("; ")
    }
}

fn format_scored_event_lines(value: Option<&serde_json::Value>) -> Vec<String> {
    let lines: Vec<String> = value
        .and_then(|v| v.as_array())
        .map(|events| {
            events
                .iter()
                .map(|event| {
                    let event_id = string_field(event, "event_id", "unknown");
                    let title = string_field(event, "title", "Untitled event");
                    let dominant_field = string_field(event, "dominant_field", "uncategorized");
                    let divergence = event
                        .get("divergence_score")
                        .and_then(|v| v.as_f64())
                        .map(|value| format!("{value:.6}"))
                        .unwrap_or_else(|| "-".to_string());
                    format!("{event_id} · {dominant_field} · div {divergence} · {title}")
                })
                .collect()
        })
        .unwrap_or_default();
    if lines.is_empty() {
        vec!["No scored events available.".to_string()]
    } else {
        lines
    }
}

fn format_event_group_lines(value: Option<&serde_json::Value>) -> Vec<String> {
    let lines: Vec<String> = value
        .and_then(|v| v.as_array())
        .map(|groups| {
            groups
                .iter()
                .map(|group| {
                    let headline_id = string_field(group, "headline_event_id", "unknown");
                    let dominant_field = string_field(group, "dominant_field", "uncategorized");
                    let member_count = group
                        .get("member_count")
                        .and_then(|v| v.as_u64())
                        .map(|value| value.to_string())
                        .unwrap_or_else(|| "0".to_string());
                    format!("{headline_id} · {dominant_field} · {member_count} members")
                })
                .collect()
        })
        .unwrap_or_default();
    if lines.is_empty() {
        vec!["No event groups available.".to_string()]
    } else {
        lines
    }
}

fn format_intervention_lines(value: Option<&serde_json::Value>) -> Vec<String> {
    let lines: Vec<String> = value
        .and_then(|v| v.as_array())
        .map(|items| {
            items
                .iter()
                .map(|item| {
                    let priority = item
                        .get("priority")
                        .and_then(|v| v.as_i64())
                        .map(|value| value.to_string())
                        .unwrap_or_else(|| "-".to_string());
                    let event_id = string_field(item, "event_id", "unknown");
                    let target = string_field(item, "target", "unknown target");
                    let intervention_type = string_field(item, "intervention_type", "unknown type");
                    format!("{priority}. {event_id} · {target} · {intervention_type}")
                })
                .collect()
        })
        .unwrap_or_default();
    if lines.is_empty() {
        vec!["No intervention candidates available.".to_string()]
    } else {
        lines
    }
}

fn compact_json_value(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Array(items) => format!("{} items", items.len()),
        serde_json::Value::Object(object) => format!("{} fields", object.len()),
        serde_json::Value::Null => "null".to_string(),
        _ => value.to_string(),
    }
}

fn compact_timestamp(value: &str) -> String {
    value
        .replace('T', " ")
        .trim_end_matches("+00:00")
        .trim_end_matches('Z')
        .to_string()
}

fn placeholder_or_value(value: &str, placeholder: &str) -> String {
    if value.trim().is_empty() {
        placeholder.to_string()
    } else {
        value.to_string()
    }
}

fn capitalize_first(value: &str) -> String {
    let mut chars = value.chars();
    match chars.next() {
        None => String::new(),
        Some(first) => {
            let upper: String = first.to_uppercase().collect();
            upper + chars.as_str()
        }
    }
}

fn format_alert_tier(tier: AlertTier) -> String {
    match tier {
        AlertTier::Flash => "flash",
        AlertTier::Priority => "priority",
        AlertTier::Routine => "routine",
    }
    .to_string()
}

fn title_line(state: &TuiState) -> Line<'static> {
    let view = match state.view {
        TuiView::Dashboard => "dashboard",
        TuiView::History => "history",
        TuiView::Detail => "detail",
        TuiView::Compare => "compare",
    };
    Line::from(vec![
        Span::styled(
            " tianji ",
            Style::default()
                .fg(KANAGAWA.title)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(format!("· {view} "), Style::default().fg(KANAGAWA.label)),
        Span::styled(
            format!("· {} persisted runs ", state.rows.len()),
            Style::default().fg(KANAGAWA.value),
        ),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::{BTreeMap, VecDeque};

    use crate::delta::{DeltaReport, DeltaSummary, RiskDirection, SignalBreakdown};
    use crate::delta_memory::HotRunEntry;

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

    fn history_state(rows: Vec<HistoryRow>) -> TuiState {
        let mut state = TuiState::new(rows, dashboard());
        state.show_history();
        state
    }

    fn temp_sqlite_path() -> String {
        use std::sync::atomic::{AtomicU64, Ordering};
        static COUNTER: AtomicU64 = AtomicU64::new(10_000);
        let id = COUNTER.fetch_add(1, Ordering::Relaxed);
        let path = format!("/tmp/tianji_tui_test_{id}.sqlite3");
        let _ = std::fs::remove_file(&path);
        path
    }

    fn delta_report(total_changes: usize, critical_changes: usize) -> DeltaReport {
        DeltaReport {
            timestamp: "1970-01-01T00:00:00+00:00".to_string(),
            previous_timestamp: Some("1969-12-31T00:00:00+00:00".to_string()),
            numeric_deltas: Vec::new(),
            count_deltas: Vec::new(),
            new_signals: Vec::new(),
            summary: DeltaSummary {
                total_changes,
                critical_changes,
                direction: RiskDirection::RiskOn,
                signal_breakdown: SignalBreakdown {
                    new_count: 2,
                    escalated_count: 1,
                    deescalated_count: 0,
                    unchanged_count: 3,
                },
            },
        }
    }

    #[test]
    fn state_navigation_clamps_to_available_rows() {
        let mut state = history_state(vec![row(1), row(2)]);

        assert_eq!(state.selected(), 0);
        state.select_previous();
        assert_eq!(state.selected(), 0);
        state.select_next();
        assert_eq!(state.selected(), 1);
        state.select_next();
        assert_eq!(state.selected(), 1);
    }

    #[test]
    fn state_selects_first_and_last_rows() {
        let mut state = history_state(vec![row(1), row(2), row(3)]);

        state.select_last();
        assert_eq!(state.selected(), 2);
        state.select_first();
        assert_eq!(state.selected(), 0);
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
    fn key_handler_maps_navigation_and_quit() {
        let mut state = history_state(vec![row(1), row(2)]);

        assert!(handle_key_code(&mut state, KeyCode::Char('j')));
        assert_eq!(state.selected(), 1);
        assert!(handle_key_code(&mut state, KeyCode::Up));
        assert_eq!(state.selected(), 0);
        assert!(handle_key_code(&mut state, KeyCode::Down));
        assert_eq!(state.selected(), 1);
        assert!(handle_key_code(&mut state, KeyCode::Char('k')));
        assert_eq!(state.selected(), 0);
        assert!(!handle_key_code(&mut state, KeyCode::Char('q')));
    }

    #[test]
    fn key_handler_maps_vim_first_and_last_navigation() {
        let mut state = history_state(vec![row(1), row(2), row(3)]);

        assert!(handle_key_code(&mut state, KeyCode::Char('G')));
        assert_eq!(state.selected(), 2);
        assert!(handle_key_code(&mut state, KeyCode::Char('g')));
        assert_eq!(state.selected(), 2);
        assert!(handle_key_code(&mut state, KeyCode::Char('g')));
        assert_eq!(state.selected(), 0);
    }

    #[test]
    fn key_handler_maps_home_and_end_aliases() {
        let mut state = history_state(vec![row(1), row(2), row(3)]);

        assert!(handle_key_code(&mut state, KeyCode::End));
        assert_eq!(state.selected(), 2);
        assert!(handle_key_code(&mut state, KeyCode::Home));
        assert_eq!(state.selected(), 0);
    }

    #[test]
    fn key_handler_clears_pending_g_after_unrelated_key() {
        let mut state = history_state(vec![row(1), row(2), row(3)]);
        state.select_last();

        assert!(handle_key_code(&mut state, KeyCode::Char('g')));
        assert_eq!(state.selected(), 2);
        assert!(handle_key_code(&mut state, KeyCode::Char('x')));
        assert_eq!(state.selected(), 2);
        assert!(handle_key_code(&mut state, KeyCode::Char('g')));
        assert_eq!(state.selected(), 2);
    }

    #[test]
    fn dashboard_maps_latest_run_and_missing_delta_placeholders() {
        let dashboard = DashboardState::from_run_summary(&[row(9)], &HotMemory::default(), None);

        assert_eq!(dashboard.latest_run_id, "#9");
        assert_eq!(dashboard.latest_mode, "fixture");
        assert_eq!(dashboard.headline, "headline");
        assert!(dashboard.field_summary.is_empty());
        assert_eq!(dashboard.total_scored_events, 0);
        assert!(dashboard.top_events.is_empty());
        assert_eq!(dashboard.alert_tier, "none");
        assert_eq!(dashboard.delta_summary, "No recent delta available.");
    }

    #[test]
    fn dashboard_maps_delta_summary_and_alert_tier() {
        let report = delta_report(4, 1);
        let mut memory = HotMemory {
            runs: VecDeque::new(),
            alerted_signals: BTreeMap::new(),
        };
        memory.runs.push_front(HotRunEntry {
            timestamp: "1970-01-01T00:00:00+00:00".to_string(),
            run_id: 1,
            compact: crate::delta_memory::compact_run_data(&serde_json::json!({})),
            delta: Some(report),
        });

        let dashboard = DashboardState::from_run_summary(&[row(1)], &memory, None);

        assert_eq!(dashboard.alert_tier, "priority");
        assert_eq!(
            dashboard.delta_summary,
            "4 total / 1 critical / 2 new signals"
        );
        assert_eq!(dashboard.delta_direction, "RiskOn");
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
    fn field_stat_extraction_from_run_summary_json() {
        let summary = serde_json::json!({
            "scenario_summary": { "headline": "test headline" },
            "scored_events": [
                { "title": "Event A", "dominant_field": "conflict", "impact_score": 15.0 },
                { "title": "Event B", "dominant_field": "conflict", "impact_score": 5.0 },
                { "title": "Event C", "dominant_field": "diplomacy", "impact_score": 12.0 },
                { "title": "Event D", "dominant_field": "technology", "impact_score": 18.0 },
                { "title": "Event E", "dominant_field": "technology", "impact_score": 10.0 },
                { "title": "Event F", "dominant_field": "economy", "impact_score": 4.0 },
            ]
        });

        let dash = dashboard_with_summary(summary);

        assert_eq!(dash.total_scored_events, 6);
        assert_eq!(dash.headline, "test headline");

        // Field summary sorted by count desc
        assert_eq!(dash.field_summary.len(), 4);
        assert_eq!(dash.field_summary[0].field, "conflict");
        assert_eq!(dash.field_summary[0].count, 2);
        assert!((dash.field_summary[0].avg_impact - 10.0).abs() < 0.01);

        assert_eq!(dash.field_summary[1].field, "technology");
        assert_eq!(dash.field_summary[1].count, 2);
        assert!((dash.field_summary[1].avg_impact - 14.0).abs() < 0.01);

        assert_eq!(dash.field_summary[2].field, "diplomacy");
        assert_eq!(dash.field_summary[2].count, 1);

        assert_eq!(dash.field_summary[3].field, "economy");
        assert_eq!(dash.field_summary[3].count, 1);
    }

    #[test]
    fn top_event_extraction_sorted_by_impact() {
        let summary = serde_json::json!({
            "scenario_summary": { "headline": "test" },
            "scored_events": [
                { "title": "Low event", "dominant_field": "economy", "impact_score": 2.0 },
                { "title": "Mid event", "dominant_field": "diplomacy", "impact_score": 8.0 },
                { "title": "High event", "dominant_field": "conflict", "impact_score": 20.0 },
                { "title": "Very high event", "dominant_field": "conflict", "impact_score": 25.0 },
                { "title": "Another mid", "dominant_field": "technology", "impact_score": 12.0 },
                { "title": "Third mid", "dominant_field": "diplomacy", "impact_score": 10.0 },
                { "title": "Extra event", "dominant_field": "economy", "impact_score": 6.0 },
            ]
        });

        let dash = dashboard_with_summary(summary);

        // Top 5 by impact desc
        assert_eq!(dash.top_events.len(), 5);
        assert_eq!(dash.top_events[0].title, "Very high event");
        assert!((dash.top_events[0].impact_score - 25.0).abs() < 0.01);
        assert_eq!(dash.top_events[0].dominant_field, "conflict");

        assert_eq!(dash.top_events[1].title, "High event");
        assert!((dash.top_events[1].impact_score - 20.0).abs() < 0.01);

        assert_eq!(dash.top_events[2].title, "Another mid");
        assert!((dash.top_events[2].impact_score - 12.0).abs() < 0.01);

        assert_eq!(dash.top_events[3].title, "Third mid");
        assert!((dash.top_events[3].impact_score - 10.0).abs() < 0.01);

        assert_eq!(dash.top_events[4].title, "Mid event");
        assert!((dash.top_events[4].impact_score - 8.0).abs() < 0.01);
    }

    #[test]
    fn dashboard_with_no_run_summary_shows_empty_fields() {
        let dash = DashboardState::from_run_summary(&[], &HotMemory::default(), None);

        assert_eq!(dash.latest_run_id, "unavailable");
        assert!(dash.field_summary.is_empty());
        assert_eq!(dash.total_scored_events, 0);
        assert!(dash.top_events.is_empty());
        assert_eq!(dash.headline, "No headline available.");
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

    #[test]
    fn key_handler_switches_between_dashboard_and_history() {
        let mut state = TuiState::new(vec![row(1), row(2)], dashboard());

        assert_eq!(state.view, TuiView::Dashboard);
        assert!(handle_key_code(&mut state, KeyCode::Char('j')));
        assert_eq!(state.selected(), 0);

        assert!(handle_key_code(&mut state, KeyCode::Char('h')));
        assert_eq!(state.view, TuiView::History);
        assert!(handle_key_code(&mut state, KeyCode::Char('j')));
        assert_eq!(state.selected(), 1);

        assert!(handle_key_code(&mut state, KeyCode::Char('d')));
        assert_eq!(state.view, TuiView::Dashboard);
    }

    #[test]
    fn detail_state_maps_from_history_show_payload() {
        let payload = serde_json::json!({
            "run_id": 42,
            "schema_version": "1.0",
            "mode": "fixture",
            "generated_at": "1970-01-01T00:00:00+00:00",
            "input_summary": {"raw_item_count": 3, "normalized_event_count": 2},
            "scenario_summary": {
                "headline": "technology pressure rises",
                "dominant_field": "technology",
                "risk_level": "high",
                "event_groups": [{
                    "headline_event_id": "evt-1",
                    "dominant_field": "technology",
                    "member_count": 2
                }]
            },
            "scored_events": [{
                "event_id": "evt-1",
                "title": "AI export controls expand",
                "dominant_field": "technology",
                "divergence_score": 8.5
            }],
            "intervention_candidates": [{
                "priority": 1,
                "event_id": "evt-1",
                "target": "technology",
                "intervention_type": "monitor"
            }]
        });

        let detail = DetailState::from_json(&payload);

        assert_eq!(detail.run_id, 42);
        assert_eq!(detail.schema_version, "1.0");
        assert_eq!(detail.mode, "fixture");
        assert!(detail.input_summary.contains("raw_item_count: 3"));
        assert!(detail
            .scenario_summary
            .contains("technology pressure rises"));
        assert!(detail.scored_events[0].contains("evt-1"));
        assert!(detail.event_groups[0].contains("2 members"));
        assert!(detail.intervention_candidates[0].contains("monitor"));
    }

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

    #[test]
    fn key_handler_opens_detail_from_history_and_returns_with_escape_or_h() {
        let mut state = history_state(vec![row(1), row(2)]);
        state.select_next();

        assert!(handle_key_code(&mut state, KeyCode::Enter));
        assert_eq!(state.view, TuiView::Detail);
        assert_eq!(state.detail.as_ref().map(|detail| detail.run_id), Some(2));
        assert_eq!(state.selected(), 1);

        assert!(handle_key_code(&mut state, KeyCode::Esc));
        assert_eq!(state.view, TuiView::History);
        assert_eq!(state.selected(), 1);

        assert!(handle_key_code(&mut state, KeyCode::Enter));
        assert_eq!(state.view, TuiView::Detail);
        assert!(handle_key_code(&mut state, KeyCode::Char('h')));
        assert_eq!(state.view, TuiView::History);
    }

    #[test]
    fn detail_load_uses_storage_summary_defaults() {
        let db_path = temp_sqlite_path();
        let _ = crate::run_fixture_path("tests/fixtures/sample_feed.xml", Some(&db_path))
            .expect("run + persist");

        let detail = load_detail_state(&db_path, 1);

        assert_eq!(detail.status, "loaded");
        assert_eq!(detail.run_id, 1);
        assert_eq!(detail.mode, "fixture");
        assert!(detail.input_summary.contains("raw_item_count: 3"));
        assert!(detail
            .scored_events
            .iter()
            .any(|line| line.contains("technology")));
        assert!(!detail.intervention_candidates.is_empty());

        let _ = std::fs::remove_file(&db_path);
    }

    #[test]
    fn detail_missing_state_is_stable_placeholder() {
        let db_path = temp_sqlite_path();
        let detail = load_detail_state(&db_path, 99);

        assert_eq!(detail.run_id, 99);
        assert!(detail.status.contains("could not"));
        assert!(format_detail(&detail).contains("No scored events available."));
        assert!(!Path::new(&db_path).exists());
    }

    #[test]
    fn compare_state_maps_from_storage_compare_result() {
        let result = CompareResult {
            left_run_id: 1,
            right_run_id: 2,
            left: serde_json::json!({
                "run_id": 1,
                "schema_version": "1.0",
                "mode": "fixture",
                "raw_item_count": 3,
                "normalized_event_count": 2,
                "dominant_field": "technology",
                "risk_level": "high",
                "headline": "left headline",
                "event_group_count": 1,
                "top_event_group": {"headline_event_id": "evt-left"},
                "top_scored_event": {"event_id": "scored-left"}
            }),
            right: serde_json::json!({
                "run_id": 2,
                "schema_version": "1.0",
                "mode": "fixture",
                "raw_item_count": 4,
                "normalized_event_count": 3,
                "dominant_field": "conflict",
                "risk_level": "critical",
                "headline": "right headline",
                "event_group_count": 2,
                "top_event_group": {"headline_event_id": "evt-right"},
                "top_scored_event": {"event_id": "scored-right"}
            }),
            diff: serde_json::json!({
                "raw_item_count_delta": 1,
                "normalized_event_count_delta": 1,
                "event_group_count_delta": 1,
                "dominant_field_changed": true,
                "risk_level_changed": true,
                "top_event_group_changed": true,
                "top_scored_event_changed": true,
                "top_scored_event_comparable": false,
                "top_intervention_changed": false,
                "top_divergence_score_delta": 0.75,
                "left_only_event_group_headline_event_ids": ["evt-left"],
                "right_only_event_group_headline_event_ids": ["evt-right"],
                "left_only_intervention_event_ids": [],
                "right_only_intervention_event_ids": ["scored-right"]
            }),
        };

        let compare = CompareState::from_result(&result);

        assert_eq!(compare.left_run_id, 1);
        assert_eq!(compare.right_run_id, 2);
        assert_eq!(compare.status, "loaded");
        assert!(compare
            .left_summary
            .iter()
            .any(|line| line.contains("technology")));
        assert!(compare
            .right_summary
            .iter()
            .any(|line| line.contains("right headline")));
        assert!(compare
            .diff_lines
            .iter()
            .any(|line| line.contains("dominant field changed: true")));
        assert!(compare
            .diff_lines
            .iter()
            .any(|line| line.contains("right-only interventions: scored-right")));
    }

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

    #[test]
    fn key_handler_stages_left_and_enter_opens_compare_from_history() {
        let mut state = history_state(vec![row(1), row(2)]);

        assert!(handle_key_code(&mut state, KeyCode::Char('c')));
        assert_eq!(state.staged_left_run_id, Some(1));
        assert!(history_title(&state).contains("staged left #1"));

        assert!(handle_key_code(&mut state, KeyCode::Char('j')));
        assert_eq!(state.selected(), 1);
        assert!(handle_key_code(&mut state, KeyCode::Enter));

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

        assert!(handle_key_code(&mut state, KeyCode::Enter));

        assert_eq!(state.view, TuiView::Detail);
        assert_eq!(state.detail.as_ref().map(|detail| detail.run_id), Some(2));
        assert!(state.compare.is_none());
    }

    #[test]
    fn compare_view_returns_to_history_and_ignores_history_navigation() {
        let mut state = history_state(vec![row(1), row(2)]);
        state.show_compare(CompareState::missing(1, 2));

        assert!(handle_key_code(&mut state, KeyCode::Char('j')));
        assert_eq!(state.selected(), 0);
        assert_eq!(state.view, TuiView::Compare);

        assert!(handle_key_code(&mut state, KeyCode::Esc));
        assert_eq!(state.view, TuiView::History);

        state.show_compare(CompareState::missing(1, 2));
        assert!(handle_key_code(&mut state, KeyCode::Char('h')));
        assert_eq!(state.view, TuiView::History);
    }

    #[test]
    fn compare_load_uses_storage_compare_defaults() {
        let db_path = temp_sqlite_path();
        let _ = crate::run_fixture_path("tests/fixtures/sample_feed.xml", Some(&db_path))
            .expect("first run + persist");
        let _ = crate::run_fixture_path("tests/fixtures/sample_feed.xml", Some(&db_path))
            .expect("second run + persist");

        let compare = load_compare_state(&db_path, 1, 2);

        assert_eq!(compare.status, "loaded");
        assert_eq!(compare.left_run_id, 1);
        assert_eq!(compare.right_run_id, 2);
        assert!(compare
            .left_summary
            .iter()
            .any(|line| line.contains("run: #1")));
        assert!(compare
            .right_summary
            .iter()
            .any(|line| line.contains("run: #2")));
        assert!(compare
            .diff_lines
            .iter()
            .any(|line| line.contains("dominant field changed")));

        let _ = std::fs::remove_file(&db_path);
    }

    #[test]
    fn compare_missing_state_is_stable_placeholder_without_creating_db() {
        let db_path = temp_sqlite_path();
        let compare = load_compare_state(&db_path, 1, 2);

        assert_eq!(compare.left_run_id, 1);
        assert_eq!(compare.right_run_id, 2);
        assert!(compare.status.contains("could not"));
        assert!(format_compare(&compare).contains("No diff available."));
        assert!(!Path::new(&db_path).exists());
    }
}
