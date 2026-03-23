from __future__ import annotations

import curses
from dataclasses import dataclass

from .storage import list_runs, get_run_summary


@dataclass(slots=True)
class HistoryListState:
    rows: list[dict[str, object]]
    sqlite_path: str
    selected_index: int = 0
    scroll_offset: int = 0
    cached_detail_run_id: int | None = None
    cached_detail_lines: list[str] | None = None
    show_help: bool = False
    focused_pane: str = "list"
    zoomed: bool = False
    detail_scroll_offset: int = 0

    def move_selection(self, delta: int, *, page_size: int) -> None:
        if self.focused_pane == "detail":
            if not self.cached_detail_lines:
                self.detail_scroll_offset = 0
                return
            max_offset = max(0, len(self.cached_detail_lines) - page_size)
            self.detail_scroll_offset = min(
                max(self.detail_scroll_offset + delta, 0), max_offset
            )
            return

        if not self.rows:
            self.selected_index = 0
            self.scroll_offset = 0
            return
        next_index = min(max(self.selected_index + delta, 0), len(self.rows) - 1)
        if next_index != self.selected_index:
            self.selected_index = next_index
            self.detail_scroll_offset = 0
        self.ensure_selection_visible(page_size=page_size)

    def ensure_selection_visible(self, *, page_size: int) -> None:
        normalized_page_size = max(page_size, 1)
        if self.selected_index < self.scroll_offset:
            self.scroll_offset = self.selected_index
        elif self.selected_index >= self.scroll_offset + normalized_page_size:
            self.scroll_offset = self.selected_index - normalized_page_size + 1

    def step_run(self, delta: int, *, page_size: int) -> None:
        if not self.rows:
            self.selected_index = 0
            self.scroll_offset = 0
            self.detail_scroll_offset = 0
            self.cached_detail_run_id = None
            self.cached_detail_lines = None
            return
        next_index = min(max(self.selected_index + delta, 0), len(self.rows) - 1)
        if next_index != self.selected_index:
            self.selected_index = next_index
            self.detail_scroll_offset = 0
            self.cached_detail_run_id = None
            self.cached_detail_lines = None
        self.ensure_selection_visible(page_size=page_size)


def launch_history_tui(*, sqlite_path: str, limit: int) -> int:
    rows = list_runs(sqlite_path=sqlite_path, limit=limit)
    if not rows:
        print("No persisted runs are available for the TUI browser.")
        return 0
    state = HistoryListState(rows=rows, sqlite_path=sqlite_path)
    curses.wrapper(run_history_list_browser, state)
    return 0


def run_history_list_browser(
    stdscr: curses.window,
    state: HistoryListState,
) -> None:
    curses.curs_set(0)
    stdscr.keypad(True)
    while True:
        draw_history_list(stdscr, state)
        key = stdscr.getch()
        page_size = max(stdscr.getmaxyx()[0] - 3, 1)
        if key in (ord("q"), ord("Q")):
            if state.show_help:
                state.show_help = False
                continue
            if state.zoomed:
                state.zoomed = False
                continue
            return
        if key == ord("?"):
            state.show_help = not state.show_help
            continue
        if state.show_help:
            state.show_help = False
            continue
        if key in (9, ord("\t")):
            state.focused_pane = "detail" if state.focused_pane == "list" else "list"
            continue
        if key in (ord("h"), curses.KEY_LEFT):
            state.focused_pane = "list"
            continue
        if key in (ord("l"), curses.KEY_RIGHT):
            state.focused_pane = "detail"
            continue
        if key in (ord("z"), ord("\n"), curses.KEY_ENTER, 10, 13):
            state.zoomed = not state.zoomed
            continue
        if key == ord("["):
            state.step_run(-1, page_size=page_size)
            continue
        if key == ord("]"):
            state.step_run(1, page_size=page_size)
            continue
        if key in (ord("j"), curses.KEY_DOWN):
            state.move_selection(1, page_size=page_size)
            continue
        if key in (ord("k"), curses.KEY_UP):
            state.move_selection(-1, page_size=page_size)
            continue
        if key in (curses.KEY_NPAGE,):
            state.move_selection(page_size, page_size=page_size)
            continue
        if key in (curses.KEY_PPAGE,):
            state.move_selection(-page_size, page_size=page_size)
            continue
        if key in (ord("g"),):
            if state.focused_pane == "detail":
                state.detail_scroll_offset = 0
            else:
                state.selected_index = 0
                state.ensure_selection_visible(page_size=page_size)
            continue
        if key in (ord("G"),):
            if state.focused_pane == "detail":
                if state.cached_detail_lines:
                    state.detail_scroll_offset = max(
                        0, len(state.cached_detail_lines) - page_size
                    )
            else:
                state.selected_index = len(state.rows) - 1
                state.ensure_selection_visible(page_size=page_size)
            continue


def format_status_footer(state: HistoryListState, width: int) -> str:
    total = len(state.rows)
    if total == 0:
        return " 0/0 ".ljust(width)

    current = state.selected_index + 1
    run_id = state.rows[state.selected_index].get("run_id", "?")

    if total == 1:
        bounds = "[only]"
    elif current == 1:
        bounds = "[first]"
    elif current == total:
        bounds = "[last]"
    else:
        bounds = "[-]"

    focus = state.focused_pane.upper()
    zoom = "ZOOM" if state.zoomed else ""

    left = f" run {current}/{total} | id:{run_id} | {bounds} "
    right = f" {zoom} | {focus} " if zoom else f" {focus} "

    if len(left) + len(right) <= width:
        return left + " " * (width - len(left) - len(right)) + right

    compact = f" {current}/{total} id:{run_id} {bounds} {zoom} {focus} "
    return shorten_text(compact.strip(), width).ljust(width)


def draw_history_list(stdscr: curses.window, state: HistoryListState) -> None:
    height, width = stdscr.getmaxyx()
    page_size = max(height - 3, 1)
    state.ensure_selection_visible(page_size=page_size)
    stdscr.erase()

    status_line = (
        " TianJi | j/k move | h/l focus | [/] step | Tab/Enter zoom | ? help | q quit "
    )
    stdscr.addnstr(0, 0, status_line.ljust(width), width, curses.A_REVERSE)

    if state.show_help:
        draw_help_overlay(stdscr, height, width)
        stdscr.refresh()
        return

    if width < 60 or (state.zoomed and state.focused_pane == "list"):
        left_width = width
        right_width = 0
    elif state.zoomed and state.focused_pane == "detail":
        left_width = 0
        right_width = width
    else:
        left_width = min(width // 2 + 10, 80)
        right_width = width - left_width - 1

    if left_width > 0:
        list_header = " [ Runs ] " if state.focused_pane == "list" else " Runs "
        header_attr = curses.A_BOLD | (
            curses.A_REVERSE if state.focused_pane == "list" else 0
        )
        stdscr.addnstr(1, 0, list_header.ljust(left_width), left_width, header_attr)

        visible_rows = state.rows[state.scroll_offset : state.scroll_offset + page_size]
        for index, row in enumerate(visible_rows, start=0):
            absolute_index = state.scroll_offset + index
            row_text = format_history_row(row, width=left_width)
            attributes = (
                curses.A_REVERSE
                if absolute_index == state.selected_index
                and state.focused_pane == "list"
                else 0
            )
            if absolute_index == state.selected_index and state.focused_pane != "list":
                attributes = curses.A_UNDERLINE
            stdscr.addnstr(index + 2, 0, row_text, left_width, attributes)

    if left_width > 0 and right_width > 0:
        for i in range(1, height - 1):
            try:
                stdscr.addch(i, left_width, curses.ACS_VLINE)
            except curses.error:
                pass

    if right_width > 0 and state.rows:
        detail_header = (
            " [ Details ] " if state.focused_pane == "detail" else " Details "
        )
        header_attr = curses.A_BOLD | (
            curses.A_REVERSE if state.focused_pane == "detail" else 0
        )
        start_x = left_width + 1 if left_width > 0 else 0
        stdscr.addnstr(
            1, start_x, detail_header.ljust(right_width), right_width, header_attr
        )

        selected_row = state.rows[state.selected_index]
        run_id = coerce_int(selected_row.get("run_id"))
        if state.cached_detail_run_id != run_id:
            summary = get_run_summary(sqlite_path=state.sqlite_path, run_id=run_id)
            if summary:
                state.cached_detail_lines = format_run_detail(
                    summary, width=right_width
                )
            else:
                state.cached_detail_lines = ["Run details not found."]
            state.cached_detail_run_id = run_id
            state.detail_scroll_offset = 0

        if state.cached_detail_lines:
            visible_lines = state.cached_detail_lines[
                state.detail_scroll_offset : state.detail_scroll_offset + page_size
            ]
            for i, line in enumerate(visible_lines):
                if i + 2 >= height - 1:
                    break
                stdscr.addnstr(i + 2, start_x, line, right_width)

    footer_text = format_status_footer(state, width)
    stdscr.addnstr(height - 1, 0, footer_text, width, curses.A_REVERSE)

    stdscr.refresh()


def draw_help_overlay(stdscr: curses.window, height: int, width: int) -> None:
    help_lines = [
        " TianJi TUI Help ",
        "",
        " Navigation:",
        "   j / Down    : Move down",
        "   k / Up      : Move up",
        "   PgDn / PgUp : Page down / up",
        "   g / G       : Jump to top / bottom",
        "",
        " Panes & Zoom:",
        "   h / l       : Focus List / Detail pane",
        "   Tab         : Toggle pane focus",
        "   [ / ]       : Previous / Next run",
        "   Enter / z   : Toggle zoom on focused pane",
        "",
        " General:",
        "   ?           : Toggle this help",
        "   q           : Quit / Close help / Unzoom",
    ]

    box_width = max(len(line) for line in help_lines) + 4
    box_height = len(help_lines) + 2

    start_y = max((height - box_height) // 2, 1)
    start_x = max((width - box_width) // 2, 0)

    if start_y + box_height > height or start_x + box_width > width:
        start_y, start_x = 1, 0

    for i in range(box_height):
        if start_y + i >= height:
            break
        if i == 0 or i == box_height - 1:
            line = "+" + "-" * (box_width - 2) + "+"
        else:
            text = help_lines[i - 1] if i - 1 < len(help_lines) else ""
            line = "| " + text.ljust(box_width - 4) + " |"
        stdscr.addnstr(start_y + i, start_x, line[: width - start_x], width - start_x)


def format_history_row(row: dict[str, object], *, width: int) -> str:
    run_id = coerce_int(row.get("run_id"))
    generated_at = str(row.get("generated_at", ""))[:16].replace("T", " ")
    mode = str(row.get("mode", ""))
    dominant_field = str(row.get("dominant_field", "uncategorized"))
    risk_level = str(row.get("risk_level", "low"))
    top_divergence_score = format_optional_score(row.get("top_divergence_score"))
    headline = str(row.get("headline", ""))

    prefix = (
        f" {run_id:>3} {generated_at:<16} {mode:<8.8} {dominant_field:<10.10} "
        f"{risk_level:<4.4} {top_divergence_score:>5} "
    )
    if len(prefix) >= width:
        return shorten_text(prefix.rstrip(), width).ljust(width)
    available_headline_width = max(width - len(prefix), 0)
    text = prefix + shorten_text(headline, available_headline_width)
    return text.ljust(width)


def format_run_detail(summary: dict[str, object], *, width: int) -> list[str]:
    lines = []
    run_id = summary.get("run_id")
    generated_at = str(summary.get("generated_at", ""))[:19].replace("T", " ")
    mode = summary.get("mode")
    lines.append(shorten_text(f"Run #{run_id} • {generated_at} • {mode}", width))
    lines.append("")

    input_summary = summary.get("input_summary")
    if isinstance(input_summary, dict):
        raw_count = input_summary.get("raw_item_count", 0)
        norm_count = input_summary.get("normalized_event_count", 0)
        lines.append(
            shorten_text(f"Items: {raw_count} raw -> {norm_count} normalized", width)
        )

    scenario = summary.get("scenario_summary")
    if isinstance(scenario, dict):
        dominant_field = scenario.get("dominant_field", "uncategorized")
        risk_level = scenario.get("risk_level", "low")
        lines.append(
            shorten_text(f"Scenario: {dominant_field} • Risk: {risk_level}", width)
        )

        headline = str(scenario.get("headline", ""))
        if headline:
            lines.append("")
            lines.extend(wrap_text(headline, width))

        event_groups = scenario.get("event_groups")
        if isinstance(event_groups, list):
            lines.append("")
            lines.append(shorten_text(f"Event Groups: {len(event_groups)}", width))
            if event_groups and isinstance(event_groups[0], dict):
                top_group = event_groups[0]
                lines.append(
                    shorten_text(
                        f"  Top: {top_group.get('dominant_field')} ({top_group.get('member_count')} members)",
                        width,
                    )
                )

    scored_events = summary.get("scored_events")
    if isinstance(scored_events, list):
        lines.append("")
        lines.append(shorten_text(f"Scored Events: {len(scored_events)}", width))
        if scored_events and isinstance(scored_events[0], dict):
            top_event = scored_events[0]
            lines.append(shorten_text(f"  Top: {top_event.get('title', '')}", width))

    interventions = summary.get("intervention_candidates")
    if isinstance(interventions, list):
        lines.append("")
        lines.append(shorten_text(f"Interventions: {len(interventions)}", width))

    return lines


def wrap_text(text: str, width: int) -> list[str]:
    if width <= 0:
        return []
    words = text.split()
    lines = []
    current_line: list[str] = []
    current_length = 0
    for word in words:
        if current_length + len(word) + len(current_line) > width:
            if current_line:
                lines.append(" ".join(current_line))
                current_line = [word]
                current_length = len(word)
            else:
                lines.append(word[:width])
                current_line = []
                current_length = 0
        else:
            current_line.append(word)
            current_length += len(word)
    if current_line:
        lines.append(" ".join(current_line))
    return lines


def coerce_int(value: object) -> int:
    if isinstance(value, int):
        return value
    if isinstance(value, str):
        return int(value)
    return 0


def format_optional_score(value: object) -> str:
    if not isinstance(value, int | float):
        return "-"
    return f"{float(value):.2f}"


def shorten_text(value: str, max_width: int) -> str:
    if max_width <= 0:
        return ""
    if len(value) <= max_width:
        return value
    if max_width == 1:
        return "…"
    return value[: max_width - 1] + "…"
