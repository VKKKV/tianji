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
use crate::TianJiError;

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
    selected: usize,
}

impl TuiState {
    pub fn new(rows: Vec<HistoryRow>) -> Self {
        Self { rows, selected: 0 }
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

    run_terminal(TuiState::new(rows))?;
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
        KeyCode::Char('j') | KeyCode::Down => {
            state.select_next();
            true
        }
        KeyCode::Char('k') | KeyCode::Up => {
            state.select_previous();
            true
        }
        _ => true,
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
    frame.render_stateful_widget(list, root[1], &mut list_state);

    let status = Paragraph::new(Line::from(vec![
        Span::styled("read-only ", Style::default().fg(KANAGAWA.label)),
        Span::styled("[j/k]", Style::default().fg(KANAGAWA.key_hint)),
        Span::raw(" move  "),
        Span::styled("[↑/↓]", Style::default().fg(KANAGAWA.key_hint)),
        Span::raw(" move  "),
        Span::styled("[q]", Style::default().fg(KANAGAWA.key_hint)),
        Span::raw(" quit"),
    ]))
    .style(base_style().bg(KANAGAWA.status_bg));
    frame.render_widget(status, root[2]);
}

fn base_style() -> Style {
    Style::default().fg(KANAGAWA.fg).bg(KANAGAWA.bg)
}

pub fn format_history_row(row: &HistoryRow) -> String {
    let generated_at = row.generated_at.replace('T', " ");
    let generated_at = generated_at.trim_end_matches("+00:00");
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

fn title_line(state: &TuiState) -> Line<'static> {
    Line::from(vec![
        Span::styled(
            " tianji ",
            Style::default()
                .fg(KANAGAWA.title)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled("· history ", Style::default().fg(KANAGAWA.label)),
        Span::styled(
            format!("· {} persisted runs ", state.rows.len()),
            Style::default().fg(KANAGAWA.value),
        ),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

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

    #[test]
    fn state_navigation_clamps_to_available_rows() {
        let mut state = TuiState::new(vec![row(1), row(2)]);

        assert_eq!(state.selected(), 0);
        state.select_previous();
        assert_eq!(state.selected(), 0);
        state.select_next();
        assert_eq!(state.selected(), 1);
        state.select_next();
        assert_eq!(state.selected(), 1);
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
        let mut state = TuiState::new(vec![row(1), row(2)]);

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
}
