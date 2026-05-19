use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use super::state::{TuiState, ViewState};
use super::theme::KANAGAWA;

pub fn base_style() -> Style {
    Style::default().fg(KANAGAWA.fg).bg(KANAGAWA.bg)
}

pub fn render(frame: &mut Frame<'_>, state: &TuiState) {
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

    match &state.view {
        ViewState::Dashboard(dashboard) => {
            super::dashboard::render_dashboard(frame, root[1], dashboard)
        }
        ViewState::History(history) => {
            super::history::render_history(frame, root[1], state, history)
        }
        ViewState::Detail(detail) => super::detail::render_detail(frame, root[1], Some(detail)),
        ViewState::Compare(compare) => {
            super::compare::render_compare(frame, root[1], Some(compare))
        }
        ViewState::Simulation(simulation) => super::simulation::render_simulation(
            frame,
            root[1],
            simulation.sim_state.as_ref(),
            simulation.replay_cursor,
            simulation.replay_frame_count,
            simulation.prune_mode,
            &simulation.prune_selected,
        ),
    }

    let mut spans = vec![
        Span::styled("read-only ", Style::default().fg(KANAGAWA.label)),
        Span::styled("[d]", Style::default().fg(KANAGAWA.key_hint)),
        Span::raw(" dashboard  "),
        Span::styled("[h]", Style::default().fg(KANAGAWA.key_hint)),
        Span::raw(" history  "),
    ];
    if state.has_simulation() {
        spans.extend(vec![
            Span::styled("[3]", Style::default().fg(KANAGAWA.key_hint)),
            Span::raw(" simulation  "),
        ]);
    }
    if let ViewState::History(history) = &state.view {
        spans.extend(vec![
            Span::styled("[Enter]", Style::default().fg(KANAGAWA.key_hint)),
            Span::raw(if history.staged_left_run_id.is_some() {
                " compare  "
            } else {
                " detail  "
            }),
            Span::styled("[c]", Style::default().fg(KANAGAWA.key_hint)),
            Span::raw(" stage compare  "),
            Span::styled("[j/k]", Style::default().fg(KANAGAWA.key_hint)),
            Span::raw(" move  "),
            Span::styled(
                state.glyphs.nav_hint,
                Style::default().fg(KANAGAWA.key_hint),
            ),
            Span::raw(" move  "),
            Span::styled("[gg/G]", Style::default().fg(KANAGAWA.key_hint)),
            Span::raw(" first/last  "),
            Span::styled("[Ctrl+d/u]", Style::default().fg(KANAGAWA.key_hint)),
            Span::raw(" half-page  "),
            Span::styled("[/]", Style::default().fg(KANAGAWA.key_hint)),
            Span::raw(" search  "),
        ]);
    } else if matches!(state.view, ViewState::Detail(_) | ViewState::Compare(_)) {
        spans.extend(vec![
            Span::styled("[Esc]", Style::default().fg(KANAGAWA.key_hint)),
            Span::raw("/"),
            Span::styled("[h]", Style::default().fg(KANAGAWA.key_hint)),
            Span::raw(" back  "),
        ]);
    } else if matches!(state.view, ViewState::Simulation(_)) {
        spans.extend(vec![
            Span::styled("[Left/h]", Style::default().fg(KANAGAWA.key_hint)),
            Span::raw(" prev frame  "),
            Span::styled("[Right/l]", Style::default().fg(KANAGAWA.key_hint)),
            Span::raw(" next frame  "),
            Span::styled("[Esc]", Style::default().fg(KANAGAWA.key_hint)),
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

fn title_line(state: &TuiState) -> Line<'static> {
    let view = if state.pending_loading.is_some() {
        "loading..."
    } else {
        match state.view.kind() {
            super::state::TuiView::Dashboard => "dashboard",
            super::state::TuiView::History => "history",
            super::state::TuiView::Detail => "detail",
            super::state::TuiView::Compare => "compare",
            super::state::TuiView::Simulation => "simulation",
        }
    };
    let all_count = match &state.view {
        ViewState::History(history) => history.all_rows.len(),
        _ => state.rows.len(),
    };
    let count_text = if state.rows.len() < all_count {
        format!("· {}/{} persisted runs ", state.rows.len(), all_count)
    } else {
        format!("· {} persisted runs ", all_count)
    };
    Line::from(vec![
        Span::styled(
            " tianji ",
            Style::default()
                .fg(KANAGAWA.title)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(format!("· {view} "), Style::default().fg(KANAGAWA.label)),
        Span::styled(count_text, Style::default().fg(KANAGAWA.value)),
    ])
}
