from __future__ import annotations

import sqlite3
import sys
import termios
import tty
from collections.abc import Callable
from dataclasses import dataclass
from typing import Any

from rich.console import Console
from rich.layout import Layout
from rich.live import Live
from rich.panel import Panel
from rich.text import Text


from .storage import (
    compare_runs,
    get_next_run_id,
    get_previous_run_id,
    get_run_summary,
    list_runs,
)


LENS_DOMINANT_FIELD_VALUES = (
    "conflict",
    "diplomacy",
    "economy",
    "technology",
)
LENS_LIMIT_VALUES = (1, 3, 5)
LENS_KEY_BINDINGS = {
    "a": "dominant_field",
    "s": "limit_scored_events",
    "d": "group_dominant_field",
    "f": "limit_event_groups",
    "v": "only_matching_interventions",
}
KEY_ACTION_ALIASES = {
    "q": "quit",
    "Q": "quit",
    "?": "toggle_help",
    "\t": "toggle_focus",
    "h": "focus_list",
    "\x1b[D": "focus_list",
    "l": "focus_active_view",
    "\x1b[C": "focus_active_view",
    "c": "stage_compare",
    "C": "clear_compare",
    "a": "cycle_event_field_lens",
    "s": "cycle_scored_event_limit_lens",
    "d": "cycle_group_field_lens",
    "f": "cycle_group_limit_lens",
    "v": "toggle_matching_interventions_lens",
    "z": "toggle_zoom",
    "\r": "toggle_zoom",
    "\n": "toggle_zoom",
    "[": "step_previous",
    "]": "step_next",
    "j": "move_down",
    "\x1b[B": "move_down",
    "k": "move_up",
    "\x1b[A": "move_up",
    "\x1b[6~": "page_down",
    "\x1b[5~": "page_up",
    "g": "jump_top",
    "G": "jump_bottom",
}


@dataclass(frozen=True, slots=True)
class HistoryInputDecision:
    action: str
    should_exit: bool = False


@dataclass(slots=True)
class HistoryListState:
    rows: list[dict[str, object]]
    sqlite_path: str
    selected_index: int = 0
    scroll_offset: int = 0
    cached_detail_run_id: int | None = None
    cached_detail_lens_key: tuple[object, ...] | None = None
    cached_detail_lines: list[str] | None = None
    show_help: bool = False
    focused_pane: str = "list"
    zoomed: bool = False
    detail_scroll_offset: int = 0
    message: str | None = None
    staged_compare_left_run_id: int | None = None
    active_view: str = "detail"
    cached_compare_right_run_id: int | None = None
    cached_compare_lens_key: tuple[object, ...] | None = None
    cached_compare_lines: list[str] | None = None
    dominant_field: str | None = None
    limit_scored_events: int | None = None
    group_dominant_field: str | None = None
    limit_event_groups: int | None = None
    only_matching_interventions: bool = False

    def invalidate_projected_panes(self) -> None:
        self.cached_detail_run_id = None
        self.cached_detail_lines = None
        self.cached_compare_right_run_id = None
        self.cached_compare_lens_key = None
        self.cached_compare_lines = None
        self.detail_scroll_offset = 0

    def active_lens_key(self) -> tuple[object, ...]:
        return (
            self.dominant_field,
            self.limit_scored_events,
            self.group_dominant_field,
            self.limit_event_groups,
            self.only_matching_interventions,
        )

    def _cycle_nullable_str(
        self, current: str | None, values: tuple[str, ...]
    ) -> str | None:
        sequence: tuple[str | None, ...] = (None, *values)
        try:
            index = sequence.index(current)
        except ValueError:
            index = 0
        return sequence[(index + 1) % len(sequence)]

    def _cycle_nullable_int(
        self, current: int | None, values: tuple[int, ...]
    ) -> int | None:
        sequence: tuple[int | None, ...] = (None, *values)
        try:
            index = sequence.index(current)
        except ValueError:
            index = 0
        return sequence[(index + 1) % len(sequence)]

    def cycle_dominant_field_lens(self) -> str | None:
        self.dominant_field = self._cycle_nullable_str(
            self.dominant_field, LENS_DOMINANT_FIELD_VALUES
        )
        self.invalidate_projected_panes()
        return self.dominant_field

    def cycle_group_dominant_field_lens(self) -> str | None:
        self.group_dominant_field = self._cycle_nullable_str(
            self.group_dominant_field, LENS_DOMINANT_FIELD_VALUES
        )
        self.invalidate_projected_panes()
        return self.group_dominant_field

    def cycle_limit_scored_events_lens(self) -> int | None:
        self.limit_scored_events = self._cycle_nullable_int(
            self.limit_scored_events, LENS_LIMIT_VALUES
        )
        self.invalidate_projected_panes()
        return self.limit_scored_events

    def cycle_limit_event_groups_lens(self) -> int | None:
        self.limit_event_groups = self._cycle_nullable_int(
            self.limit_event_groups, LENS_LIMIT_VALUES
        )
        self.invalidate_projected_panes()
        return self.limit_event_groups

    def toggle_only_matching_interventions(self) -> bool:
        self.only_matching_interventions = not self.only_matching_interventions
        self.invalidate_projected_panes()
        return self.only_matching_interventions

    def _find_nearest_valid_compare_target_index(self) -> int | None:
        if not self.rows or len(self.rows) < 2:
            return None

        for delta in (1, -1):
            idx = self.selected_index + delta
            if 0 <= idx < len(self.rows):
                if (
                    coerce_int(self.rows[idx].get("run_id"))
                    != self.staged_compare_left_run_id
                ):
                    return idx

        for i in range(self.selected_index + 1, len(self.rows)):
            if (
                coerce_int(self.rows[i].get("run_id"))
                != self.staged_compare_left_run_id
            ):
                return i
        for i in range(self.selected_index - 1, -1, -1):
            if (
                coerce_int(self.rows[i].get("run_id"))
                != self.staged_compare_left_run_id
            ):
                return i

        return None

    def stage_compare(self, run_id: int, *, page_size: int = 1) -> None:
        if self.staged_compare_left_run_id is None:
            self.staged_compare_left_run_id = run_id
            self.message = f"left run staged: {run_id}"
        elif self.staged_compare_left_run_id == run_id:
            target_index = self._find_nearest_valid_compare_target_index()
            if target_index is not None:
                self.selected_index = target_index
                self.ensure_selection_visible(page_size=page_size)
                self.active_view = "compare"
                if self.focused_pane != "list":
                    self.focused_pane = "compare"
                self.message = "compare view active"
            else:
                self.message = "cannot compare a run with itself"
        else:
            self.active_view = "compare"
            if self.focused_pane != "list":
                self.focused_pane = "compare"
            self.message = "compare view active"

    def clear_compare(self) -> None:
        self.staged_compare_left_run_id = None
        self.active_view = "detail"
        if self.focused_pane == "compare":
            self.focused_pane = "detail"
        self.message = "compare cleared"

    def move_selection(self, delta: int, *, page_size: int) -> None:
        if self.focused_pane != "list":
            lines = (
                self.cached_detail_lines
                if self.active_view == "detail"
                else self.cached_compare_lines
            )
            if not lines:
                self.detail_scroll_offset = 0
                return
            max_offset = max(0, len(lines) - page_size)
            new_offset = min(max(self.detail_scroll_offset + delta, 0), max_offset)
            if new_offset == self.detail_scroll_offset and delta != 0:
                self.message = (
                    f"top of {self.active_view}"
                    if delta < 0
                    else f"bottom of {self.active_view}"
                )
            self.detail_scroll_offset = new_offset
            return

        if not self.rows:
            self.selected_index = 0
            self.scroll_offset = 0
            return
        next_index = min(max(self.selected_index + delta, 0), len(self.rows) - 1)
        if next_index == self.selected_index and delta != 0:
            self.message = "first run" if delta < 0 else "last run"
        elif next_index != self.selected_index:
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
        if not self._step_to_persisted_run(delta, page_size=page_size):
            self._step_to_loaded_run(delta, page_size=page_size)

    def step_compare_target(self, delta: int, *, page_size: int) -> None:
        if not self.rows or self.staged_compare_left_run_id is None:
            return

        starting_run_id = self.current_run_id()
        if starting_run_id is None:
            return

        candidate_run_id = starting_run_id
        while True:
            candidate_run_id = self._resolve_adjacent_run_id(candidate_run_id, delta)
            if candidate_run_id is False:
                self._step_compare_target_in_loaded_rows(delta, page_size=page_size)
                return
            if candidate_run_id is None:
                self.message = (
                    "first compare target" if delta < 0 else "last compare target"
                )
                return
            if candidate_run_id == self.staged_compare_left_run_id:
                continue
            self.select_run_id(candidate_run_id, page_size=page_size)
            self.invalidate_projected_panes()
            return

    def current_run_id(self) -> int | None:
        if not self.rows:
            return None
        return coerce_int(self.rows[self.selected_index].get("run_id"))

    def select_run_id(self, run_id: int, *, page_size: int) -> None:
        window_size = len(self.rows) or 20
        row_index = self._find_loaded_run_index(run_id)
        if row_index is None:
            try:
                self.rows = list_runs(sqlite_path=self.sqlite_path, limit=window_size)
                row_index = self._find_loaded_run_index(run_id)
            except sqlite3.OperationalError:
                row_index = None
            if row_index is None:
                fallback_row = get_run_summary(
                    sqlite_path=self.sqlite_path, run_id=run_id
                )
                if fallback_row is not None:
                    self.rows = merge_run_row_into_window(
                        self.rows,
                        build_history_row_from_summary(fallback_row),
                        window_size=window_size,
                    )
                    row_index = self._find_loaded_run_index(run_id)
                else:
                    return
        if row_index is None:
            return
        self.selected_index = row_index
        self.ensure_selection_visible(page_size=page_size)

    def _find_loaded_run_index(self, run_id: int) -> int | None:
        for index, row in enumerate(self.rows):
            if coerce_int(row.get("run_id")) == run_id:
                return index
        return None

    def _resolve_adjacent_run_id(self, run_id: int, delta: int) -> int | None | bool:
        try:
            if delta < 0:
                return get_previous_run_id(sqlite_path=self.sqlite_path, run_id=run_id)
            if delta > 0:
                return get_next_run_id(sqlite_path=self.sqlite_path, run_id=run_id)
        except sqlite3.OperationalError:
            return False
        return run_id

    def _step_to_persisted_run(self, delta: int, *, page_size: int) -> bool:
        if not self.rows:
            self.selected_index = 0
            self.scroll_offset = 0
            self.invalidate_projected_panes()
            return True
        current_run_id = self.current_run_id()
        if current_run_id is None:
            return True
        next_run_id = self._resolve_adjacent_run_id(current_run_id, delta)
        if next_run_id is False:
            return False
        if next_run_id is None and delta != 0:
            self.message = "first run" if delta < 0 else "last run"
            return True
        if next_run_id is not None and next_run_id != current_run_id:
            self.select_run_id(next_run_id, page_size=page_size)
            self.invalidate_projected_panes()
        self.ensure_selection_visible(page_size=page_size)
        return True

    def _step_to_loaded_run(self, delta: int, *, page_size: int) -> None:
        if not self.rows:
            self.selected_index = 0
            self.scroll_offset = 0
            self.invalidate_projected_panes()
            return
        next_index = min(max(self.selected_index + delta, 0), len(self.rows) - 1)
        if next_index == self.selected_index and delta != 0:
            self.message = "first run" if delta < 0 else "last run"
        elif next_index != self.selected_index:
            self.selected_index = next_index
            self.invalidate_projected_panes()
        self.ensure_selection_visible(page_size=page_size)

    def _step_compare_target_in_loaded_rows(
        self, delta: int, *, page_size: int
    ) -> None:
        next_index = self.selected_index
        while True:
            next_index += delta
            if next_index < 0 or next_index >= len(self.rows):
                self.message = (
                    "first compare target" if delta < 0 else "last compare target"
                )
                return
            run_id = coerce_int(self.rows[next_index].get("run_id"))
            if run_id == self.staged_compare_left_run_id:
                continue
            self.selected_index = next_index
            self.invalidate_projected_panes()
            self.ensure_selection_visible(page_size=page_size)
            return


def getch() -> str:
    fd = sys.stdin.fileno()
    old_settings = termios.tcgetattr(fd)
    try:
        tty.setraw(fd)
        ch = sys.stdin.read(1)
        if ch == "\x1b":
            ch += sys.stdin.read(2)
            if ch.endswith("5") or ch.endswith("6"):
                ch += sys.stdin.read(1)
    finally:
        termios.tcsetattr(fd, termios.TCSADRAIN, old_settings)
    return ch


def launch_history_tui(*, sqlite_path: str, limit: int) -> int:
    rows = list_runs(sqlite_path=sqlite_path, limit=limit)
    if not rows:
        print("No persisted runs are available for the TUI browser.")
        return 0
    state = HistoryListState(rows=rows, sqlite_path=sqlite_path)
    run_history_list_browser(state)
    return 0


def run_history_list_browser(state: HistoryListState) -> None:
    console = Console()
    with Live(screen=True, auto_refresh=False, console=console) as live:
        run_history_browser_session(state, console=console, live=live)


def run_history_browser_session(
    state: HistoryListState,
    *,
    console: Console,
    live: Any,
    read_key: Callable[[], str] | None = None,
) -> None:
    key_reader = getch if read_key is None else read_key
    while True:
        height = console.size.height
        width = console.size.width
        page_size = max(height - 5, 1)

        layout = build_layout(state, height, width, page_size)
        live.update(layout, refresh=True)

        key = key_reader()
        decision = handle_history_browser_key(state, key=key, page_size=page_size)
        if decision.should_exit:
            return


def resolve_history_browser_action(key: str) -> str | None:
    return KEY_ACTION_ALIASES.get(key)


def handle_history_browser_key(
    state: HistoryListState, *, key: str, page_size: int
) -> HistoryInputDecision:
    state.message = None
    action = resolve_history_browser_action(key)

    if action == "quit":
        if state.show_help:
            state.show_help = False
            state.message = "help closed"
            return HistoryInputDecision(action="close_help")
        if state.zoomed:
            state.zoomed = False
            state.message = "zoom off"
            return HistoryInputDecision(action="disable_zoom")
        return HistoryInputDecision(action="quit", should_exit=True)

    if action == "toggle_help":
        state.show_help = not state.show_help
        state.message = "help opened" if state.show_help else "help closed"
        return HistoryInputDecision(action=action)

    if state.show_help:
        state.show_help = False
        state.message = "help closed"
        return HistoryInputDecision(action="close_help")

    if action == "toggle_focus":
        state.focused_pane = (
            state.active_view if state.focused_pane == "list" else "list"
        )
        state.message = f"focus={state.focused_pane}"
        return HistoryInputDecision(action=action)

    if action == "focus_list":
        if state.focused_pane != "list":
            state.focused_pane = "list"
            state.message = "focus=list"
        return HistoryInputDecision(action=action)

    if action == "focus_active_view":
        if state.focused_pane == "list":
            state.focused_pane = state.active_view
            state.message = f"focus={state.active_view}"
        return HistoryInputDecision(action=action)

    if action == "stage_compare":
        if state.rows:
            current_run_id = coerce_int(state.rows[state.selected_index].get("run_id"))
            state.stage_compare(current_run_id, page_size=page_size)
        return HistoryInputDecision(action=action)

    if action == "clear_compare":
        state.clear_compare()
        return HistoryInputDecision(action=action)

    if action == "cycle_event_field_lens":
        dominant_field = state.cycle_dominant_field_lens()
        state.message = format_lens_change_message(
            "event field lens", dominant_field or "all"
        )
        return HistoryInputDecision(action=action)

    if action == "cycle_scored_event_limit_lens":
        limit_scored_events = state.cycle_limit_scored_events_lens()
        state.message = format_lens_change_message(
            "scored-event limit", str(limit_scored_events or "all")
        )
        return HistoryInputDecision(action=action)

    if action == "cycle_group_field_lens":
        group_dominant_field = state.cycle_group_dominant_field_lens()
        state.message = format_lens_change_message(
            "group field lens", group_dominant_field or "all"
        )
        return HistoryInputDecision(action=action)

    if action == "cycle_group_limit_lens":
        limit_event_groups = state.cycle_limit_event_groups_lens()
        state.message = format_lens_change_message(
            "group limit", str(limit_event_groups or "all")
        )
        return HistoryInputDecision(action=action)

    if action == "toggle_matching_interventions_lens":
        only_matching = state.toggle_only_matching_interventions()
        state.message = format_lens_change_message(
            "matching interventions", "on" if only_matching else "off"
        )
        return HistoryInputDecision(action=action)

    if action == "toggle_zoom":
        state.zoomed = not state.zoomed
        state.message = "zoom on" if state.zoomed else "zoom off"
        return HistoryInputDecision(action=action)

    if action == "step_previous":
        if state.active_view == "compare" and state.focused_pane == "compare":
            state.step_compare_target(-1, page_size=page_size)
        else:
            state.step_run(-1, page_size=page_size)
        return HistoryInputDecision(action=action)

    if action == "step_next":
        if state.active_view == "compare" and state.focused_pane == "compare":
            state.step_compare_target(1, page_size=page_size)
        else:
            state.step_run(1, page_size=page_size)
        return HistoryInputDecision(action=action)

    if action == "move_down":
        state.move_selection(1, page_size=page_size)
        return HistoryInputDecision(action=action)

    if action == "move_up":
        state.move_selection(-1, page_size=page_size)
        return HistoryInputDecision(action=action)

    if action == "page_down":
        state.move_selection(page_size, page_size=page_size)
        return HistoryInputDecision(action=action)

    if action == "page_up":
        state.move_selection(-page_size, page_size=page_size)
        return HistoryInputDecision(action=action)

    if action == "jump_top":
        if state.focused_pane != "list":
            state.detail_scroll_offset = 0
            state.message = f"top of {state.active_view}"
        else:
            state.selected_index = 0
            state.ensure_selection_visible(page_size=page_size)
            state.message = "first run"
        return HistoryInputDecision(action=action)

    if action == "jump_bottom":
        if state.focused_pane != "list":
            lines = (
                state.cached_detail_lines
                if state.active_view == "detail"
                else state.cached_compare_lines
            )
            if lines:
                state.detail_scroll_offset = max(0, len(lines) - page_size)
            state.message = f"bottom of {state.active_view}"
        elif state.rows:
            state.selected_index = len(state.rows) - 1
            state.ensure_selection_visible(page_size=page_size)
            state.message = "last run"
        return HistoryInputDecision(action=action)

    return HistoryInputDecision(action="noop")


def build_right_panel(state: HistoryListState, width: int, page_size: int) -> Panel:
    if state.active_view == "compare":
        return build_compare_panel(state, width, page_size)
    return build_detail_panel(state, width, page_size)


def build_compare_panel(state: HistoryListState, width: int, page_size: int) -> Panel:
    inner_width = max(width - 2, 1)
    title_text = " Compare "
    if not state.rows or state.staged_compare_left_run_id is None:
        content = Text("Select a second run to compare.")
    else:
        selected_row = state.rows[state.selected_index]
        right_run_id = coerce_int(selected_row.get("run_id"))
        title_text = f" Compare L:{state.staged_compare_left_run_id} R:{right_run_id} "

        if right_run_id == state.staged_compare_left_run_id:
            content = Text("Cannot compare a run with itself.\nSelect a different run.")
        else:
            compare_lens_key = state.active_lens_key()
            if (
                state.cached_compare_right_run_id != right_run_id
                or state.cached_compare_lens_key != compare_lens_key
            ):
                compare_result = compare_runs(
                    sqlite_path=state.sqlite_path,
                    left_run_id=state.staged_compare_left_run_id,
                    right_run_id=right_run_id,
                    dominant_field=state.dominant_field,
                    limit_scored_events=state.limit_scored_events,
                    group_dominant_field=state.group_dominant_field,
                    limit_event_groups=state.limit_event_groups,
                    only_matching_interventions=state.only_matching_interventions,
                )
                if compare_result:
                    state.cached_compare_lines = format_compare_detail(
                        compare_result,
                        width=inner_width,
                        projected_empty_messages=build_compare_projected_empty_messages(
                            compare_result,
                            state=state,
                        ),
                    )
                else:
                    state.cached_compare_lines = [
                        "No persisted compare view is available."
                    ]
                state.cached_compare_right_run_id = right_run_id
                state.cached_compare_lens_key = compare_lens_key
                state.detail_scroll_offset = 0

            if state.cached_compare_lines:
                total_lines = len(state.cached_compare_lines)
                if total_lines > page_size:
                    end_line = min(state.detail_scroll_offset + page_size, total_lines)
                    title_text = f"{title_text.rstrip()} {state.detail_scroll_offset + 1}-{end_line}/{total_lines} "

                visible_lines = state.cached_compare_lines[
                    state.detail_scroll_offset : state.detail_scroll_offset + page_size
                ]
                lines = [Text(line) for line in visible_lines]
                content = Text("\n").join(lines)
            else:
                content = Text("")

    title = (
        Text(f" [{title_text.strip()}] ", style="reverse bold")
        if state.focused_pane != "list"
        else Text(title_text, style="bold")
    )
    return Panel(content, title=title, title_align="left")


def build_layout(
    state: HistoryListState, height: int, width: int, page_size: int
) -> Layout:
    state.ensure_selection_visible(page_size=page_size)

    layout = Layout()
    layout.split_column(
        Layout(name="header", size=1),
        Layout(name="body"),
        Layout(name="message", size=1),
        Layout(name="footer", size=1),
    )

    status_line = (
        " TianJi | j/k move | h/l focus | [/] step | a/s/d/f/v lens view"
        f" | {format_active_lens_summary(state)} | Tab/Enter zoom | ? help | q quit "
    )
    layout["header"].update(Text(status_line.ljust(width), style="reverse"))

    if state.show_help:
        help_text = build_help_text()
        layout["body"].update(Panel(help_text, title="TianJi TUI Help", expand=False))
    else:
        if width < 60 or (state.zoomed and state.focused_pane == "list"):
            layout["body"].update(build_list_panel(state, width, page_size))
        elif state.zoomed and state.focused_pane != "list":
            layout["body"].update(build_right_panel(state, width, page_size))
        else:
            left_width = min(width // 2 + 10, 80)
            layout["body"].split_row(
                Layout(name="left", size=left_width), Layout(name="right")
            )
            layout["left"].update(build_list_panel(state, left_width, page_size))
            layout["right"].update(
                build_right_panel(state, width - left_width, page_size)
            )

    if state.message and height > 3:
        msg = f" {state.message} "
        layout["message"].update(Text(msg.ljust(len(msg)), style="reverse"))
    else:
        layout["message"].update(Text(""))

    footer_text = format_status_footer(state, width)
    layout["footer"].update(Text(footer_text, style="reverse"))

    return layout


def build_list_panel(state: HistoryListState, width: int, page_size: int) -> Panel:
    inner_width = max(width - 2, 1)
    visible_rows = state.rows[state.scroll_offset : state.scroll_offset + page_size]
    lines = []
    for index, row in enumerate(visible_rows):
        absolute_index = state.scroll_offset + index
        run_id = coerce_int(row.get("run_id"))
        is_staged_left = run_id == state.staged_compare_left_run_id
        row_text = format_history_row(
            row, width=inner_width, is_staged_left=is_staged_left
        )
        style = ""
        if absolute_index == state.selected_index:
            if state.focused_pane == "list":
                style = "reverse"
            else:
                style = "underline"
        lines.append(Text(row_text, style=style))

    content = Text("\n").join(lines) if lines else Text("")
    title = (
        Text(" [ Runs ] ", style="reverse bold")
        if state.focused_pane == "list"
        else Text(" Runs ", style="bold")
    )
    return Panel(content, title=title, title_align="left")


def build_detail_panel(state: HistoryListState, width: int, page_size: int) -> Panel:
    inner_width = max(width - 2, 1)
    if not state.rows:
        content = Text("")
    else:
        selected_row = state.rows[state.selected_index]
        run_id = coerce_int(selected_row.get("run_id"))
        detail_lens_key = state.active_lens_key()
        if (
            state.cached_detail_run_id != run_id
            or state.cached_detail_lens_key != detail_lens_key
        ):
            summary = get_run_summary(
                sqlite_path=state.sqlite_path,
                run_id=run_id,
                dominant_field=state.dominant_field,
                limit_scored_events=state.limit_scored_events,
                group_dominant_field=state.group_dominant_field,
                limit_event_groups=state.limit_event_groups,
                only_matching_interventions=state.only_matching_interventions,
            )
            if summary:
                state.cached_detail_lines = format_run_detail(
                    summary,
                    width=inner_width,
                    projected_empty_messages=build_detail_projected_empty_messages(
                        summary,
                        state=state,
                    ),
                )
            else:
                state.cached_detail_lines = ["No persisted detail view is available."]
            state.cached_detail_run_id = run_id
            state.cached_detail_lens_key = detail_lens_key
            state.detail_scroll_offset = 0

        if state.cached_detail_lines:
            visible_lines = state.cached_detail_lines[
                state.detail_scroll_offset : state.detail_scroll_offset + page_size
            ]
            lines = [Text(line) for line in visible_lines]
            content = Text("\n").join(lines)
        else:
            content = Text("")

    title = (
        Text(" [ Details ] ", style="reverse bold")
        if state.focused_pane == "detail"
        else Text(" Details ", style="bold")
    )
    return Panel(content, title=title, title_align="left")


def build_help_text() -> Text:
    help_lines = [
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
        " Compare:",
        "   c           : Stage/activate compare",
        "   C           : Clear compare",
        "",
        " Lenses:",
        "   a           : Cycle scored-event field lens",
        "   s           : Cycle scored-event limit lens",
        "   d           : Cycle event-group field lens",
        "   f           : Cycle event-group limit lens",
        "   v           : Toggle intervention-match lens",
        "   Active lens  : Projects detail/compare, list stays persisted",
        "",
        " General:",
        "   ?           : Toggle this help",
        "   q           : Quit / Close help / Unzoom",
    ]
    return Text("\n".join(help_lines))


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

    compare_state = ""
    if state.staged_compare_left_run_id is not None:
        if state.active_view == "compare":
            compare_state = f"COMPARE L:{state.staged_compare_left_run_id} R:{run_id}"
        else:
            compare_state = f"COMPARE L:{state.staged_compare_left_run_id}"

    lens_state = format_active_lens_summary(state)

    left = f" run {current}/{total} | id:{run_id} | {bounds} "

    right_parts = []
    if compare_state:
        right_parts.append(compare_state)
    if lens_state != "lens:all-runs":
        right_parts.append(f"VIEW {lens_state.upper()}")
    if zoom:
        right_parts.append(zoom)
    right_parts.append(focus)

    right = " | ".join(right_parts)
    right = f" {right} "

    if len(left) + len(right) <= width:
        return left + " " * (width - len(left) - len(right)) + right

    compact_lens = "" if lens_state == "lens:all-runs" else lens_state
    compact = (
        f" run {current}/{total} id:{run_id} {bounds}"
        f" {compare_state} {compact_lens} {zoom} {focus} "
    )
    compact = " ".join(compact.split())
    return shorten_text(compact.strip(), width).ljust(width)


def format_history_row(
    row: dict[str, object], *, width: int, is_staged_left: bool = False
) -> str:
    run_id = coerce_int(row.get("run_id"))
    generated_at = str(row.get("generated_at", ""))[:16].replace("T", " ")
    mode = str(row.get("mode", ""))
    dominant_field = str(row.get("dominant_field", "uncategorized"))
    risk_level = str(row.get("risk_level", "low"))
    top_divergence_score = format_optional_score(row.get("top_divergence_score"))
    headline = str(row.get("headline", ""))

    marker = "*" if is_staged_left else " "
    prefix = (
        f"{marker}{run_id:>3} {generated_at:<16} {mode:<8.8} {dominant_field:<10.10} "
        f"{risk_level:<4.4} {top_divergence_score:>5} "
    )
    if len(prefix) >= width:
        return shorten_text(prefix.rstrip(), width).ljust(width)
    available_headline_width = max(width - len(prefix), 0)
    text = prefix + shorten_text(headline, available_headline_width)
    return text.ljust(width)


def format_lens_change_message(label: str, value: str) -> str:
    return f"lens {label}: {value}"


def format_active_lens_summary(state: HistoryListState) -> str:
    parts = []
    if state.dominant_field is not None:
        parts.append(f"ev={state.dominant_field}")
    if state.limit_scored_events is not None:
        parts.append(f"top={state.limit_scored_events}")
    if state.group_dominant_field is not None:
        parts.append(f"grp={state.group_dominant_field}")
    if state.limit_event_groups is not None:
        parts.append(f"groups={state.limit_event_groups}")
    if state.only_matching_interventions:
        parts.append("match-int")
    if not parts:
        return "lens:all-runs"
    return "lens:" + ",".join(parts)


def format_projected_empty_message(state: HistoryListState, *, subject: str) -> str:
    lens_summary = format_active_lens_summary(state)
    if lens_summary == "lens:all-runs":
        return f"No persisted {subject} view is available."
    return f"No {subject} rows match the active lens. Persisted run data is unchanged."


def has_active_projection_lens(state: HistoryListState) -> bool:
    return format_active_lens_summary(state) != "lens:all-runs"


def format_projected_empty_slice_message(slice_name: str) -> str:
    return (
        f"No {slice_name} rows match the active lens. Persisted run data is unchanged."
    )


def build_detail_projected_empty_messages(
    summary: dict[str, object], *, state: HistoryListState
) -> dict[str, str]:
    messages: dict[str, str] = {}
    if not has_active_projection_lens(state):
        return messages

    scenario = summary.get("scenario_summary")
    event_groups = scenario.get("event_groups") if isinstance(scenario, dict) else None
    scored_events = summary.get("scored_events")
    interventions = summary.get("intervention_candidates")

    if (
        (state.group_dominant_field is not None or state.limit_event_groups is not None)
        and isinstance(event_groups, list)
        and not event_groups
    ):
        messages["event_groups"] = format_projected_empty_slice_message("event-group")

    if (
        (state.dominant_field is not None or state.limit_scored_events is not None)
        and isinstance(scored_events, list)
        and not scored_events
    ):
        messages["scored_events"] = format_projected_empty_slice_message("scored-event")

    if (
        state.only_matching_interventions
        and isinstance(interventions, list)
        and not interventions
    ):
        messages["interventions"] = format_projected_empty_slice_message("intervention")

    return messages


def build_compare_projected_empty_messages(
    compare_result: dict[str, object], *, state: HistoryListState
) -> dict[str, list[str]]:
    messages = {"left": [], "right": []}
    if not has_active_projection_lens(state):
        return messages

    left = compare_result.get("left")
    right = compare_result.get("right")
    if isinstance(left, dict):
        messages["left"] = build_compare_side_projected_empty_messages(
            left,
            state=state,
        )
    if isinstance(right, dict):
        messages["right"] = build_compare_side_projected_empty_messages(
            right,
            state=state,
        )
    return messages


def build_compare_side_projected_empty_messages(
    side: dict[str, object], *, state: HistoryListState
) -> list[str]:
    messages: list[str] = []

    if (
        (state.group_dominant_field is not None or state.limit_event_groups is not None)
        and side.get("top_event_group") is None
        and side.get("event_group_count") == 0
    ):
        messages.append(format_projected_empty_slice_message("event-group"))

    if (
        state.dominant_field is not None or state.limit_scored_events is not None
    ) and side.get("top_scored_event") is None:
        messages.append(format_projected_empty_slice_message("scored-event"))

    intervention_event_ids = side.get("intervention_event_ids")
    if (
        state.only_matching_interventions
        and side.get("top_intervention") is None
        and isinstance(intervention_event_ids, list)
        and not intervention_event_ids
    ):
        messages.append(format_projected_empty_slice_message("intervention"))

    return messages


def format_event_group_preview_lines(
    group: dict[str, object],
    *,
    rank: int,
    width: int,
) -> list[str]:
    dominant_field = str(group.get("dominant_field", "uncategorized"))
    member_count = group.get("member_count", 0)
    headline_title = str(group.get("headline_title", ""))
    causal_summary = str(group.get("causal_summary", ""))

    summary_line = shorten_text(
        f" {rank}. {dominant_field} ({member_count} members)",
        width,
    )
    snippet = causal_summary if causal_summary else headline_title
    if not snippet:
        snippet = "No summary available."
    snippet_line = shorten_text(f"    {snippet}", width)
    return [summary_line, snippet_line]


def format_compare_side_summaries(
    side: dict[str, object],
    *,
    width: int,
    projected_empty_messages: list[str] | None = None,
) -> list[str]:
    lines = []
    top_group = side.get("top_event_group")
    if isinstance(top_group, dict):
        dominant_field = str(top_group.get("dominant_field", "uncategorized"))
        member_count = top_group.get("member_count", 0)
        lines.append(
            shorten_text(
                f"  Top Group: {dominant_field} ({member_count} members)", width
            )
        )

    top_event = side.get("top_scored_event")
    if isinstance(top_event, dict):
        dominant_field = str(top_event.get("dominant_field", "uncategorized"))
        dv = format_optional_score(top_event.get("divergence_score"))
        im = format_optional_score(top_event.get("impact_score"))
        lines.append(
            shorten_text(f"  Top Event: {dominant_field} Dv {dv} Im {im}", width)
        )

    top_intervention = side.get("top_intervention")
    if isinstance(top_intervention, dict):
        target = str(top_intervention.get("target", "unknown"))
        itype = str(top_intervention.get("intervention_type", "unknown"))
        lines.append(shorten_text(f"  Top Action: [{itype}] {target}", width))

    if projected_empty_messages:
        for message in projected_empty_messages:
            lines.extend(wrap_text(f"  {message}", width))

    return lines


def format_top_group_evidence_diff_lines(
    evidence_diff: dict[str, object], *, width: int
) -> list[str]:
    lines = []
    comparable = evidence_diff.get("comparable", False)
    mode_text = "Comparable" if comparable else "Contrast"
    lines.append(shorten_text(f"  • Top group evidence ({mode_text}):", width))

    member_delta = evidence_diff.get("member_count_delta", 0)
    link_delta = evidence_diff.get("evidence_chain_link_count_delta", 0)
    if isinstance(member_delta, int) and isinstance(link_delta, int):
        lines.append(
            shorten_text(
                f"      Members: {member_delta:+d}, Links: {link_delta:+d}", width
            )
        )

    added_ids = evidence_diff.get("right_only_member_event_ids", [])
    if isinstance(added_ids, list) and added_ids:
        lines.append(
            shorten_text(f"      Added IDs: {', '.join(map(str, added_ids))}", width)
        )

    removed_ids = evidence_diff.get("left_only_member_event_ids", [])
    if isinstance(removed_ids, list) and removed_ids:
        lines.append(
            shorten_text(
                f"      Removed IDs: {', '.join(map(str, removed_ids))}", width
            )
        )

    added_kw = evidence_diff.get("shared_keywords_added", [])
    if isinstance(added_kw, list) and added_kw:
        lines.append(
            shorten_text(
                f"      Added keywords: {', '.join(map(str, added_kw))}", width
            )
        )

    removed_kw = evidence_diff.get("shared_keywords_removed", [])
    if isinstance(removed_kw, list) and removed_kw:
        lines.append(
            shorten_text(
                f"      Removed keywords: {', '.join(map(str, removed_kw))}", width
            )
        )

    if evidence_diff.get("chain_summary_changed"):
        lines.append(shorten_text("      Chain summary changed", width))

    return lines


def get_compare_similarity_summary(diff: dict[str, object]) -> str | None:
    if not diff:
        return None

    major_flags = [
        diff.get("dominant_field_changed"),
        diff.get("risk_level_changed"),
        diff.get("top_event_group_changed"),
        diff.get("top_scored_event_changed"),
        diff.get("top_intervention_changed"),
    ]
    if any(major_flags):
        return None

    item_delta = diff.get("raw_item_count_delta", 0)
    norm_delta = diff.get("normalized_event_count_delta", 0)
    group_delta = diff.get("event_group_count_delta", 0)
    has_count_changes = bool(item_delta or norm_delta or group_delta)

    dv_delta = diff.get("top_divergence_score_delta")
    im_delta = diff.get("top_impact_score_delta")
    fa_delta = diff.get("top_field_attraction_delta")

    def is_nonzero(val: object) -> bool:
        if not isinstance(val, int | float):
            return False
        return abs(float(val)) > 0.001

    has_score_changes = (
        is_nonzero(dv_delta) or is_nonzero(im_delta) or is_nonzero(fa_delta)
    )

    evidence_diff = diff.get("top_event_group_evidence_diff")
    has_evidence_changes = False
    if isinstance(evidence_diff, dict):
        has_evidence_changes = (
            bool(evidence_diff.get("member_count_delta"))
            or bool(evidence_diff.get("evidence_chain_link_count_delta"))
            or bool(evidence_diff.get("right_only_member_event_ids"))
            or bool(evidence_diff.get("left_only_member_event_ids"))
            or bool(evidence_diff.get("shared_keywords_added"))
            or bool(evidence_diff.get("shared_keywords_removed"))
            or bool(evidence_diff.get("chain_summary_changed"))
        )

    if not has_count_changes and not has_score_changes and not has_evidence_changes:
        return "Effectively identical: no meaningful differences found."

    return "No major differences: top signals and fields remain stable."


def format_compare_detail(
    compare_result: dict[str, object],
    *,
    width: int,
    projected_empty_messages: dict[str, list[str]] | None = None,
) -> list[str]:
    lines = []
    left = compare_result.get("left", {})
    right = compare_result.get("right", {})
    diff = compare_result.get("diff", {})

    if (
        not isinstance(left, dict)
        or not isinstance(right, dict)
        or not isinstance(diff, dict)
    ):
        return ["Invalid compare result."]

    left_id = left.get("run_id")
    right_id = right.get("run_id")
    lines.append(
        shorten_text(
            f"Compare: Run #{left_id} (Left) vs Run #{right_id} (Right)", width
        )
    )

    similarity_summary = get_compare_similarity_summary(diff)
    if similarity_summary:
        lines.append("")
        lines.append(shorten_text(f"Summary: {similarity_summary}", width))

    lines.append("")

    left_field = left.get("dominant_field", "uncategorized")
    left_risk = left.get("risk_level", "low")
    lines.append(
        shorten_text(
            f"[Left] {left.get('mode')} • {left_field} • Risk: {left_risk}", width
        )
    )
    left_headline = str(left.get("headline", ""))
    if left_headline:
        lines.extend(wrap_text(f"       {left_headline}", width))
    left_empty_messages = (
        projected_empty_messages.get("left", [])
        if isinstance(projected_empty_messages, dict)
        else []
    )
    lines.extend(
        format_compare_side_summaries(
            left,
            width=width,
            projected_empty_messages=left_empty_messages,
        )
    )
    lines.append("")

    right_field = right.get("dominant_field", "uncategorized")
    right_risk = right.get("risk_level", "low")
    lines.append(
        shorten_text(
            f"[Right] {right.get('mode')} • {right_field} • Risk: {right_risk}", width
        )
    )
    right_headline = str(right.get("headline", ""))
    if right_headline:
        lines.extend(wrap_text(f"        {right_headline}", width))
    right_empty_messages = (
        projected_empty_messages.get("right", [])
        if isinstance(projected_empty_messages, dict)
        else []
    )
    lines.extend(
        format_compare_side_summaries(
            right,
            width=width,
            projected_empty_messages=right_empty_messages,
        )
    )
    lines.append("")

    lines.append(shorten_text("Diff Highlights:", width))

    if diff.get("dominant_field_changed"):
        lines.append(
            shorten_text(f"  • Field changed: {left_field} -> {right_field}", width)
        )
    if diff.get("risk_level_changed"):
        lines.append(
            shorten_text(f"  • Risk changed: {left_risk} -> {right_risk}", width)
        )

    item_delta = diff.get("raw_item_count_delta", 0)
    norm_delta = diff.get("normalized_event_count_delta", 0)
    if isinstance(item_delta, int) and isinstance(norm_delta, int):
        lines.append(
            shorten_text(
                f"  • Items: {item_delta:+d} raw, {norm_delta:+d} normalized", width
            )
        )

    group_delta = diff.get("event_group_count_delta", 0)
    if isinstance(group_delta, int):
        lines.append(shorten_text(f"  • Event Groups: {group_delta:+d}", width))

    if diff.get("top_event_group_changed"):
        lines.append(shorten_text("  • Top event group changed", width))

    evidence_diff = diff.get("top_event_group_evidence_diff")
    if isinstance(evidence_diff, dict):
        lines.extend(format_top_group_evidence_diff_lines(evidence_diff, width=width))

    if diff.get("top_scored_event_changed"):
        lines.append(shorten_text("  • Top scored event changed", width))
    elif diff.get("top_scored_event_comparable"):
        dv_delta = format_delta(diff.get("top_divergence_score_delta"))
        im_delta = format_delta(diff.get("top_impact_score_delta"))
        fa_delta = format_delta(diff.get("top_field_attraction_delta"))
        lines.append(
            shorten_text(
                f"  • Top event score deltas: Dv {dv_delta} Im {im_delta} Fa {fa_delta}",
                width,
            )
        )

    if diff.get("top_intervention_changed"):
        lines.append(shorten_text("  • Top intervention changed", width))

    return lines


def format_delta(value: object) -> str:
    if not isinstance(value, int | float):
        return "N/A"
    return f"{float(value):+.2f}"


def format_run_detail(
    summary: dict[str, object],
    *,
    width: int,
    projected_empty_messages: dict[str, str] | None = None,
) -> list[str]:
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
            event_groups_empty_message = (
                projected_empty_messages.get("event_groups")
                if isinstance(projected_empty_messages, dict)
                else None
            )
            if event_groups_empty_message:
                lines.extend(wrap_text(event_groups_empty_message, width))
            for group_index, group in enumerate(event_groups[:3], start=1):
                if not isinstance(group, dict):
                    continue
                lines.extend(
                    format_event_group_preview_lines(
                        group,
                        rank=group_index,
                        width=width,
                    )
                )

    scored_events = summary.get("scored_events")
    if isinstance(scored_events, list):
        lines.append("")
        lines.append(shorten_text(f"Scored Events: {len(scored_events)}", width))
        scored_events_empty_message = (
            projected_empty_messages.get("scored_events")
            if isinstance(projected_empty_messages, dict)
            else None
        )
        if scored_events_empty_message:
            lines.extend(wrap_text(scored_events_empty_message, width))
        for event_index, event in enumerate(scored_events[:3], start=1):
            if not isinstance(event, dict):
                continue
            lines.extend(
                format_scored_event_preview_lines(
                    event,
                    rank=event_index,
                    width=width,
                )
            )

    interventions = summary.get("intervention_candidates")
    if isinstance(interventions, list):
        lines.append("")
        lines.append(shorten_text(f"Interventions: {len(interventions)}", width))
        interventions_empty_message = (
            projected_empty_messages.get("interventions")
            if isinstance(projected_empty_messages, dict)
            else None
        )
        if interventions_empty_message:
            lines.extend(wrap_text(interventions_empty_message, width))
        for intervention_index, intervention in enumerate(interventions[:3], start=1):
            if not isinstance(intervention, dict):
                continue
            lines.extend(
                format_intervention_preview_lines(
                    intervention,
                    rank=intervention_index,
                    width=width,
                )
            )

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


def format_scored_event_preview_lines(
    event: dict[str, object],
    *,
    rank: int,
    width: int,
) -> list[str]:
    dominant_field = str(event.get("dominant_field", "uncategorized"))
    divergence_score = format_optional_score(event.get("divergence_score"))
    impact_score = format_optional_score(event.get("impact_score"))
    field_attraction = format_optional_score(event.get("field_attraction"))
    title = str(event.get("title", ""))

    summary_line = shorten_text(
        f" {rank}. {dominant_field} Dv {divergence_score} Im {impact_score} Fa {field_attraction}",
        width,
    )
    title_line = shorten_text(f"    {title}", width)
    return [summary_line, title_line]


def format_intervention_preview_lines(
    intervention: dict[str, object],
    *,
    rank: int,
    width: int,
) -> list[str]:
    target = str(intervention.get("target", "unknown"))
    intervention_type = str(intervention.get("intervention_type", "unknown"))
    snippet = str(intervention.get("expected_effect", ""))
    if not snippet:
        snippet = str(intervention.get("reason", ""))

    summary_line = shorten_text(
        f" {rank}. [{intervention_type}] {target}",
        width,
    )
    snippet_line = shorten_text(f"    {snippet}", width)
    return [summary_line, snippet_line]


def build_history_row_from_summary(summary: dict[str, object]) -> dict[str, object]:
    scenario_summary = summary.get("scenario_summary")
    scenario = scenario_summary if isinstance(scenario_summary, dict) else {}
    event_groups = scenario.get("event_groups")
    event_group_count = len(event_groups) if isinstance(event_groups, list) else 0
    scored_events = summary.get("scored_events")
    top_scored_event = (
        scored_events[0] if isinstance(scored_events, list) and scored_events else None
    )
    return {
        "run_id": summary.get("run_id", 0),
        "schema_version": summary.get("schema_version", ""),
        "mode": summary.get("mode", ""),
        "generated_at": summary.get("generated_at", ""),
        "raw_item_count": nested_summary_value(
            summary, "input_summary", "raw_item_count", 0
        ),
        "normalized_event_count": nested_summary_value(
            summary, "input_summary", "normalized_event_count", 0
        ),
        "dominant_field": scenario.get("dominant_field", "uncategorized"),
        "risk_level": scenario.get("risk_level", "low"),
        "headline": scenario.get("headline", ""),
        "event_group_count": event_group_count,
        "top_divergence_score": (
            top_scored_event.get("divergence_score")
            if isinstance(top_scored_event, dict)
            else None
        ),
    }


def nested_summary_value(
    summary: dict[str, object],
    parent_key: str,
    child_key: str,
    default: object,
) -> object:
    parent = summary.get(parent_key)
    if not isinstance(parent, dict):
        return default
    return parent.get(child_key, default)


def merge_run_row_into_window(
    rows: list[dict[str, object]],
    new_row: dict[str, object],
    *,
    window_size: int,
) -> list[dict[str, object]]:
    run_id = coerce_int(new_row.get("run_id"))
    merged = [row for row in rows if coerce_int(row.get("run_id")) != run_id]
    insert_index = 0
    while insert_index < len(merged):
        existing_run_id = coerce_int(merged[insert_index].get("run_id"))
        if run_id > existing_run_id:
            break
        insert_index += 1
    merged.insert(insert_index, new_row)
    if len(merged) <= window_size:
        return merged
    start_index = min(insert_index, len(merged) - window_size)
    end_index = start_index + window_size
    return merged[start_index:end_index]


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
