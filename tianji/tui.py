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

    def move_selection(self, delta: int, *, page_size: int) -> None:
        if not self.rows:
            self.selected_index = 0
            self.scroll_offset = 0
            return
        next_index = min(max(self.selected_index + delta, 0), len(self.rows) - 1)
        self.selected_index = next_index
        self.ensure_selection_visible(page_size=page_size)

    def ensure_selection_visible(self, *, page_size: int) -> None:
        normalized_page_size = max(page_size, 1)
        if self.selected_index < self.scroll_offset:
            self.scroll_offset = self.selected_index
        elif self.selected_index >= self.scroll_offset + normalized_page_size:
            self.scroll_offset = self.selected_index - normalized_page_size + 1


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
        page_size = max(stdscr.getmaxyx()[0] - 1, 1)
        if key in (ord("q"), ord("Q")):
            return
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
            state.selected_index = 0
            state.ensure_selection_visible(page_size=page_size)
            continue
        if key in (ord("G"),):
            state.selected_index = len(state.rows) - 1
            state.ensure_selection_visible(page_size=page_size)


def draw_history_list(stdscr: curses.window, state: HistoryListState) -> None:
    height, width = stdscr.getmaxyx()
    page_size = max(height - 1, 1)
    state.ensure_selection_visible(page_size=page_size)
    stdscr.erase()
    
    status_line = " TianJi | j/k move | PgUp/PgDn scroll | g/G jump | q quit "
    stdscr.addnstr(0, 0, status_line.ljust(width), width, curses.A_REVERSE)
    
    if width < 60:
        left_width = width
        right_width = 0
    else:
        left_width = min(width // 2 + 10, 80)
        right_width = width - left_width - 1

    visible_rows = state.rows[state.scroll_offset : state.scroll_offset + page_size]
    for index, row in enumerate(visible_rows, start=0):
        absolute_index = state.scroll_offset + index
        row_text = format_history_row(row, width=left_width)
        attributes = curses.A_REVERSE if absolute_index == state.selected_index else 0
        stdscr.addnstr(index + 1, 0, row_text, left_width, attributes)
        
    if right_width > 10 and state.rows:
        selected_row = state.rows[state.selected_index]
        run_id = coerce_int(selected_row.get("run_id"))
        if state.cached_detail_run_id != run_id:
            summary = get_run_summary(sqlite_path=state.sqlite_path, run_id=run_id)
            if summary:
                state.cached_detail_lines = format_run_detail(summary, width=right_width)
            else:
                state.cached_detail_lines = ["Run details not found."]
            state.cached_detail_run_id = run_id
            
        if state.cached_detail_lines:
            for i, line in enumerate(state.cached_detail_lines):
                if i + 1 >= height:
                    break
                stdscr.addnstr(i + 1, left_width + 1, line, right_width)
                
    stdscr.refresh()


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
        lines.append(shorten_text(f"Items: {raw_count} raw -> {norm_count} normalized", width))
    
    scenario = summary.get("scenario_summary")
    if isinstance(scenario, dict):
        dominant_field = scenario.get("dominant_field", "uncategorized")
        risk_level = scenario.get("risk_level", "low")
        lines.append(shorten_text(f"Scenario: {dominant_field} • Risk: {risk_level}", width))
        
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
                lines.append(shorten_text(f"  Top: {top_group.get('dominant_field')} ({top_group.get('member_count')} members)", width))
        
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
