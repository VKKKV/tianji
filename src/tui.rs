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

use crate::storage::{list_runs, RunListFilters};
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
pub struct DashboardState {
    pub latest_run_id: String,
    pub latest_generated_at: String,
    pub latest_mode: String,
    pub dominant_field: String,
    pub risk_level: String,
    pub top_divergence_score: String,
    pub headline: String,
    pub alert_tier: String,
    pub delta_summary: String,
    pub delta_direction: String,
    pub baseline_status: String,
    pub worldline_status: String,
}

impl DashboardState {
    pub fn from_history_and_memory(rows: &[HistoryRow], memory: &HotMemory) -> Self {
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
        let dominant_field = latest
            .map(|row| placeholder_or_value(&row.dominant_field, "uncategorized"))
            .unwrap_or_else(|| "uncategorized".to_string());
        let risk_level = latest
            .map(|row| placeholder_or_value(&row.risk_level, "unknown"))
            .unwrap_or_else(|| "unknown".to_string());
        let top_divergence_score = latest
            .and_then(|row| row.top_divergence_score)
            .map(|value| format!("{value:.6}"))
            .unwrap_or_else(|| "unavailable".to_string());
        let headline = latest
            .map(|row| placeholder_or_value(&row.headline, "No headline available."))
            .unwrap_or_else(|| "No headline available.".to_string());

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
            dominant_field,
            risk_level,
            top_divergence_score,
            headline,
            alert_tier,
            delta_summary,
            delta_direction,
            baseline_status: "Baseline/worldline model unavailable in current persisted data."
                .to_string(),
            worldline_status: "Simulation/profile state deferred; dashboard is read-only."
                .to_string(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TuiView {
    Dashboard,
    History,
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
    selected: usize,
    pending_g: bool,
}

impl TuiState {
    pub fn new(rows: Vec<HistoryRow>, dashboard: DashboardState) -> Self {
        Self {
            rows,
            dashboard,
            view: TuiView::Dashboard,
            selected: 0,
            pending_g: false,
        }
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
    let dashboard = DashboardState::from_history_and_memory(&rows, &memory);
    run_terminal(TuiState::new(rows, dashboard))?;
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
        KeyCode::Char('d') | KeyCode::Char('D') | KeyCode::Char('1') => {
            state.show_dashboard();
            true
        }
        KeyCode::Char('h') | KeyCode::Char('H') | KeyCode::Char('2') => {
            state.show_history();
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
            Span::styled("[j/k]", Style::default().fg(KANAGAWA.key_hint)),
            Span::raw(" move  "),
            Span::styled("[↑/↓]", Style::default().fg(KANAGAWA.key_hint)),
            Span::raw(" move  "),
            Span::styled("[gg/G]", Style::default().fg(KANAGAWA.key_hint)),
            Span::raw(" first/last  "),
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
                .title(" Run History ")
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

fn render_dashboard(
    frame: &mut Frame<'_>,
    area: ratatui::layout::Rect,
    dashboard: &DashboardState,
) {
    let paragraph = Paragraph::new(format_dashboard(dashboard))
        .block(
            Block::bordered()
                .title(" Dashboard ")
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
    format!(
        "Latest run\n  run: {}\n  generated: {}\n  mode: {}\n  dominant field: {}\n  risk: {}\n  top divergence: {}\n  headline: {}\n\nRecent delta\n  alert tier: {}\n  summary: {}\n  direction: {}\n\nDeferred worldline\n  baseline: {}\n  worldline: {}",
        dashboard.latest_run_id,
        dashboard.latest_generated_at,
        dashboard.latest_mode,
        dashboard.dominant_field,
        dashboard.risk_level,
        dashboard.top_divergence_score,
        dashboard.headline,
        dashboard.alert_tier,
        dashboard.delta_summary,
        dashboard.delta_direction,
        dashboard.baseline_status,
        dashboard.worldline_status
    )
}

fn compact_timestamp(value: &str) -> String {
    value
        .replace('T', " ")
        .trim_end_matches("+00:00")
        .to_string()
}

fn placeholder_or_value(value: &str, placeholder: &str) -> String {
    if value.trim().is_empty() {
        placeholder.to_string()
    } else {
        value.to_string()
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
        DashboardState::from_history_and_memory(&[row(1)], &HotMemory::default())
    }

    fn history_state(rows: Vec<HistoryRow>) -> TuiState {
        let mut state = TuiState::new(rows, dashboard());
        state.show_history();
        state
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
        let dashboard = DashboardState::from_history_and_memory(&[row(9)], &HotMemory::default());

        assert_eq!(dashboard.latest_run_id, "#9");
        assert_eq!(dashboard.latest_mode, "fixture");
        assert_eq!(dashboard.dominant_field, "technology");
        assert_eq!(dashboard.risk_level, "high");
        assert_eq!(dashboard.top_divergence_score, "20.730000");
        assert_eq!(dashboard.alert_tier, "none");
        assert_eq!(dashboard.delta_summary, "No recent delta available.");
        assert!(dashboard.baseline_status.contains("unavailable"));
        assert!(dashboard.worldline_status.contains("read-only"));
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

        let dashboard = DashboardState::from_history_and_memory(&[row(1)], &memory);

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
        assert!(formatted.contains("dominant field: technology"));
        assert!(formatted.contains("Recent delta"));
        assert!(formatted.contains("No recent delta available."));
        assert!(formatted.contains("Deferred worldline"));
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
}
