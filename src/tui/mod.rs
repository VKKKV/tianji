mod compare;
mod dashboard;
mod detail;
mod history;
mod render;
mod state;
mod theme;

pub use compare::{format_compare, render_compare};
pub use dashboard::{format_dashboard, render_dashboard};
pub use detail::{format_detail, render_detail};
pub use history::{format_history_row, history_title, render_history};
pub use render::{base_style, render};
pub use state::{
    array_string_field, bool_field, capitalize_first, compact_json_field, compact_json_value,
    compact_timestamp, detect_glyph_mode, format_alert_tier, numeric_field, optional_f64_field,
    placeholder_or_value, signed_numeric_field, string_field, CompareState, DashboardState,
    DetailState, FieldStat, GlyphSet, HistoryRow, TopEvent, TuiState, TuiView, ASCII_GLYPHS,
    EMPTY_TUI_MESSAGE, NERD_GLYPHS,
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
}
