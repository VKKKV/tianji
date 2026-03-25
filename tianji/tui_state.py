from __future__ import annotations

import sqlite3
from dataclasses import dataclass

from .storage import (
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

    def invalidate_compare_cache(self) -> None:
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
            self.invalidate_compare_cache()
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
        self.invalidate_compare_cache()
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


def resolve_history_browser_action(key: str) -> str | None:
    return KEY_ACTION_ALIASES.get(key)


def format_lens_change_message(label: str, value: str) -> str:
    return f"lens {label}: {value}"


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
