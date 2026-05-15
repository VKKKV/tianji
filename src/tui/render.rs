use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use super::state::TuiState;
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

    match state.view {
        super::state::TuiView::Dashboard => {
            super::dashboard::render_dashboard(frame, root[1], &state.dashboard)
        }
        super::state::TuiView::History => super::history::render_history(frame, root[1], state),
        super::state::TuiView::Detail => {
            super::detail::render_detail(frame, root[1], state.detail.as_ref())
        }
        super::state::TuiView::Compare => {
            super::compare::render_compare(frame, root[1], state.compare.as_ref())
        }
        super::state::TuiView::Simulation => {
            super::simulation::render_simulation(frame, root[1], state.simulation.as_ref())
        }
    }

    let mut spans = vec![
        Span::styled("read-only ", Style::default().fg(KANAGAWA.label)),
        Span::styled("[d]", Style::default().fg(KANAGAWA.key_hint)),
        Span::raw(" dashboard  "),
        Span::styled("[h]", Style::default().fg(KANAGAWA.key_hint)),
        Span::raw(" history  "),
    ];
    if state.simulation.is_some() {
        spans.extend(vec![
            Span::styled("[3]", Style::default().fg(KANAGAWA.key_hint)),
            Span::raw(" simulation  "),
        ]);
    }
    if state.view == super::state::TuiView::History {
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
    } else if matches!(
        state.view,
        super::state::TuiView::Detail
            | super::state::TuiView::Compare
            | super::state::TuiView::Simulation
    ) {
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

fn title_line(state: &TuiState) -> Line<'static> {
    let view = match state.view {
        super::state::TuiView::Dashboard => "dashboard",
        super::state::TuiView::History => "history",
        super::state::TuiView::Detail => "detail",
        super::state::TuiView::Compare => "compare",
        super::state::TuiView::Simulation => "simulation",
    };
    let count_text = if state.rows.len() < state.all_rows.len() {
        format!(
            "· {}/{} persisted runs ",
            state.rows.len(),
            state.all_rows.len()
        )
    } else {
        format!("· {} persisted runs ", state.all_rows.len())
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
