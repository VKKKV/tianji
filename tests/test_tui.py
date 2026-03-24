from support import *
from rich.layout import Layout
from rich.panel import Panel
from rich.text import Text
from tianji.tui import (
    KEY_ACTION_ALIASES,
    LENS_DOMINANT_FIELD_VALUES,
    LENS_LIMIT_VALUES,
    LENS_KEY_BINDINGS,
    build_detail_panel,
    build_compare_panel,
    build_layout,
    build_help_text,
    format_active_lens_summary,
    format_lens_change_message,
    format_top_group_evidence_diff_lines,
    get_compare_similarity_summary,
    handle_history_browser_key,
    resolve_history_browser_action,
    run_history_browser_session,
    run_history_list_browser,
)


class TuiTests(unittest.TestCase):
    def _run_browser_session(
        self,
        state: HistoryListState,
        keys: list[str],
        *,
        width: int = 100,
        height: int = 12,
    ) -> list[dict[str, str]]:
        class RecordingLive:
            def __init__(self) -> None:
                self.frames: list[dict[str, str]] = []

            def update(self, layout: object, refresh: bool = False) -> None:
                rendered_layout = cast(Layout, layout)
                frame = {
                    "header": cast(Text, rendered_layout["header"].renderable).plain,
                    "message": cast(Text, rendered_layout["message"].renderable).plain,
                    "footer": cast(Text, rendered_layout["footer"].renderable).plain,
                }
                if state.cached_detail_lines is not None:
                    frame["detail_cache"] = "\n".join(state.cached_detail_lines)
                if state.cached_compare_lines is not None:
                    frame["compare_cache"] = "\n".join(state.cached_compare_lines)
                body = rendered_layout["body"]
                if body.children:
                    left_panel = cast(Panel, body["left"].renderable)
                    right_panel = cast(Panel, body["right"].renderable)
                    frame["list"] = cast(Text, left_panel.renderable).plain
                    frame["right"] = cast(Text, right_panel.renderable).plain
                else:
                    body_panel = cast(Panel, body.renderable)
                    frame["body"] = cast(Text, body_panel.renderable).plain
                self.frames.append(frame)

        fake_console = mock.Mock()
        fake_console.size.height = height
        fake_console.size.width = width
        live = RecordingLive()
        key_sequence = iter(keys)

        run_history_browser_session(
            state,
            console=fake_console,
            live=live,
            read_key=lambda: next(key_sequence),
        )
        return live.frames

    def test_history_list_state_lens_defaults_share_all_lenses_key(self) -> None:
        state = HistoryListState(rows=[{"run_id": 10}], sqlite_path="dummy.sqlite3")

        self.assertIsNone(state.dominant_field)
        self.assertIsNone(state.limit_scored_events)
        self.assertIsNone(state.group_dominant_field)
        self.assertIsNone(state.limit_event_groups)
        self.assertFalse(state.only_matching_interventions)
        self.assertEqual(
            state.active_lens_key(),
            (None, None, None, None, False),
        )
        self.assertEqual(format_active_lens_summary(state), "lens:all-runs")
        self.assertEqual(
            LENS_KEY_BINDINGS,
            {
                "a": "dominant_field",
                "s": "limit_scored_events",
                "d": "group_dominant_field",
                "f": "limit_event_groups",
                "v": "only_matching_interventions",
            },
        )

    def test_history_list_state_cycles_and_toggles_all_lens_helpers(self) -> None:
        state = HistoryListState(rows=[{"run_id": 10}], sqlite_path="dummy.sqlite3")

        dominant_field_values = [state.dominant_field]
        for _ in LENS_DOMINANT_FIELD_VALUES:
            dominant_field_values.append(state.cycle_dominant_field_lens())
        self.assertEqual(dominant_field_values, [None, *LENS_DOMINANT_FIELD_VALUES])

        scored_limit_values = [state.limit_scored_events]
        for _ in LENS_LIMIT_VALUES:
            scored_limit_values.append(state.cycle_limit_scored_events_lens())
        self.assertEqual(scored_limit_values, [None, *LENS_LIMIT_VALUES])

        group_field_values = [state.group_dominant_field]
        for _ in LENS_DOMINANT_FIELD_VALUES:
            group_field_values.append(state.cycle_group_dominant_field_lens())
        self.assertEqual(group_field_values, [None, *LENS_DOMINANT_FIELD_VALUES])

        group_limit_values = [state.limit_event_groups]
        for _ in LENS_LIMIT_VALUES:
            group_limit_values.append(state.cycle_limit_event_groups_lens())
        self.assertEqual(group_limit_values, [None, *LENS_LIMIT_VALUES])

        self.assertTrue(state.toggle_only_matching_interventions())
        self.assertFalse(state.toggle_only_matching_interventions())

    def test_history_list_state_lens_helpers_invalidate_projected_panes_only(
        self,
    ) -> None:
        state = HistoryListState(
            rows=[{"run_id": 10}, {"run_id": 20}],
            sqlite_path="dummy.sqlite3",
            selected_index=1,
            scroll_offset=1,
            focused_pane="compare",
            zoomed=True,
            show_help=True,
            message="persist",
            staged_compare_left_run_id=10,
            active_view="compare",
            cached_detail_run_id=20,
            cached_detail_lens_key=("technology", 1, "diplomacy", 3, True),
            cached_detail_lines=["detail"],
            cached_compare_right_run_id=20,
            cached_compare_lens_key=("technology", 1, "diplomacy", 3, True),
            cached_compare_lines=["compare"],
            detail_scroll_offset=4,
        )

        state.cycle_dominant_field_lens()

        self.assertIsNone(state.cached_detail_run_id)
        self.assertIsNone(state.cached_detail_lines)
        self.assertIsNone(state.cached_compare_right_run_id)
        self.assertIsNone(state.cached_compare_lens_key)
        self.assertIsNone(state.cached_compare_lines)
        self.assertEqual(state.detail_scroll_offset, 0)
        self.assertEqual(state.selected_index, 1)
        self.assertEqual(state.scroll_offset, 1)
        self.assertEqual(state.focused_pane, "compare")
        self.assertEqual(state.active_view, "compare")
        self.assertEqual(state.staged_compare_left_run_id, 10)
        self.assertTrue(state.zoomed)
        self.assertTrue(state.show_help)
        self.assertEqual(state.message, "persist")

    def test_cli_tui_dispatches_to_history_browser(self) -> None:
        with mock.patch("tianji.cli.launch_history_tui", return_value=0) as launch_mock:
            exit_code = main(
                [
                    "tui",
                    "--sqlite-path",
                    "runs/tianji.sqlite3",
                    "--limit",
                    "12",
                ]
            )

        self.assertEqual(exit_code, 0)
        launch_mock.assert_called_once_with(
            sqlite_path="runs/tianji.sqlite3",
            limit=12,
        )

    def test_cli_tui_rejects_negative_limit(self) -> None:
        stderr = io.StringIO()
        with contextlib.redirect_stderr(stderr):
            with self.assertRaises(SystemExit) as error:
                main(
                    [
                        "tui",
                        "--sqlite-path",
                        "runs/tianji.sqlite3",
                        "--limit",
                        "-1",
                    ]
                )

        self.assertEqual(error.exception.code, 2)
        self.assertIn("--limit must be zero or greater.", stderr.getvalue())

    def test_history_list_state_keeps_selection_visible_when_scrolling(self) -> None:
        state = HistoryListState(
            rows=[{"run_id": index} for index in range(8)], sqlite_path="dummy.sqlite3"
        )

        state.move_selection(5, page_size=3)

        self.assertEqual(state.selected_index, 5)
        self.assertEqual(state.scroll_offset, 3)

        state.move_selection(-4, page_size=3)

        self.assertEqual(state.selected_index, 1)
        self.assertEqual(state.scroll_offset, 1)

    def test_history_list_state_toggles_focus_and_zoom(self) -> None:
        state = HistoryListState(
            rows=[{"run_id": index} for index in range(8)], sqlite_path="dummy.sqlite3"
        )
        self.assertEqual(state.focused_pane, "list")
        self.assertFalse(state.zoomed)
        self.assertFalse(state.show_help)

        state.focused_pane = "detail"
        self.assertEqual(state.focused_pane, "detail")

        state.zoomed = True
        self.assertTrue(state.zoomed)

        state.show_help = True
        self.assertTrue(state.show_help)

    def test_resolve_history_browser_action_maps_supported_keys(self) -> None:
        self.assertEqual(resolve_history_browser_action("q"), "quit")
        self.assertEqual(resolve_history_browser_action("Q"), "quit")
        self.assertEqual(resolve_history_browser_action("?"), "toggle_help")
        self.assertEqual(resolve_history_browser_action("a"), "cycle_event_field_lens")
        self.assertEqual(
            resolve_history_browser_action("s"), "cycle_scored_event_limit_lens"
        )
        self.assertEqual(resolve_history_browser_action("d"), "cycle_group_field_lens")
        self.assertEqual(resolve_history_browser_action("f"), "cycle_group_limit_lens")
        self.assertEqual(
            resolve_history_browser_action("v"),
            "toggle_matching_interventions_lens",
        )
        self.assertEqual(resolve_history_browser_action("["), "step_previous")
        self.assertEqual(resolve_history_browser_action("]"), "step_next")
        self.assertEqual(resolve_history_browser_action("j"), "move_down")
        self.assertEqual(resolve_history_browser_action("k"), "move_up")
        self.assertEqual(resolve_history_browser_action("g"), "jump_top")
        self.assertEqual(resolve_history_browser_action("G"), "jump_bottom")
        self.assertEqual(resolve_history_browser_action("c"), "stage_compare")
        self.assertEqual(resolve_history_browser_action("C"), "clear_compare")
        self.assertEqual(resolve_history_browser_action("\t"), "toggle_focus")
        self.assertEqual(resolve_history_browser_action("z"), "toggle_zoom")
        self.assertEqual(resolve_history_browser_action("\r"), "toggle_zoom")
        self.assertEqual(resolve_history_browser_action("\n"), "toggle_zoom")
        self.assertEqual(resolve_history_browser_action("\x1b[A"), "move_up")
        self.assertEqual(resolve_history_browser_action("\x1b[B"), "move_down")
        self.assertEqual(resolve_history_browser_action("\x1b[C"), "focus_active_view")
        self.assertEqual(resolve_history_browser_action("\x1b[D"), "focus_list")
        self.assertEqual(resolve_history_browser_action("\x1b[5~"), "page_up")
        self.assertEqual(resolve_history_browser_action("\x1b[6~"), "page_down")
        self.assertIsNone(resolve_history_browser_action("x"))
        self.assertEqual(KEY_ACTION_ALIASES["a"], "cycle_event_field_lens")

    def test_handle_history_browser_key_supports_help_quit_focus_zoom_and_noop(
        self,
    ) -> None:
        state = HistoryListState(
            rows=[{"run_id": 10}, {"run_id": 20}], sqlite_path="dummy.sqlite3"
        )

        decision = handle_history_browser_key(state, key="?", page_size=3)
        self.assertEqual(decision.action, "toggle_help")
        self.assertFalse(decision.should_exit)
        self.assertTrue(state.show_help)
        self.assertEqual(state.message, "help opened")

        decision = handle_history_browser_key(state, key="q", page_size=3)
        self.assertEqual(decision.action, "close_help")
        self.assertFalse(decision.should_exit)
        self.assertFalse(state.show_help)
        self.assertEqual(state.message, "help closed")

        decision = handle_history_browser_key(state, key="l", page_size=3)
        self.assertEqual(decision.action, "focus_active_view")
        self.assertEqual(state.focused_pane, "detail")
        self.assertEqual(state.message, "focus=detail")

        decision = handle_history_browser_key(state, key="\t", page_size=3)
        self.assertEqual(decision.action, "toggle_focus")
        self.assertEqual(state.focused_pane, "list")
        self.assertEqual(state.message, "focus=list")

        decision = handle_history_browser_key(state, key="z", page_size=3)
        self.assertEqual(decision.action, "toggle_zoom")
        self.assertTrue(state.zoomed)
        self.assertEqual(state.message, "zoom on")

        decision = handle_history_browser_key(state, key="q", page_size=3)
        self.assertEqual(decision.action, "disable_zoom")
        self.assertFalse(decision.should_exit)
        self.assertFalse(state.zoomed)
        self.assertEqual(state.message, "zoom off")

        decision = handle_history_browser_key(state, key="x", page_size=3)
        self.assertEqual(decision.action, "noop")
        self.assertFalse(decision.should_exit)
        self.assertIsNone(state.message)

        decision = handle_history_browser_key(state, key="q", page_size=3)
        self.assertEqual(decision.action, "quit")
        self.assertTrue(decision.should_exit)

    def test_handle_history_browser_key_cycles_lenses_and_compare_controls(
        self,
    ) -> None:
        state = HistoryListState(
            rows=[{"run_id": 10}, {"run_id": 20}, {"run_id": 30}],
            sqlite_path="dummy.sqlite3",
        )

        decision = handle_history_browser_key(state, key="a", page_size=2)
        self.assertEqual(decision.action, "cycle_event_field_lens")
        self.assertEqual(state.dominant_field, "conflict")
        self.assertEqual(state.message, "lens event field lens: conflict")

        decision = handle_history_browser_key(state, key="s", page_size=2)
        self.assertEqual(decision.action, "cycle_scored_event_limit_lens")
        self.assertEqual(state.limit_scored_events, 1)
        self.assertEqual(state.message, "lens scored-event limit: 1")

        decision = handle_history_browser_key(state, key="d", page_size=2)
        self.assertEqual(decision.action, "cycle_group_field_lens")
        self.assertEqual(state.group_dominant_field, "conflict")
        self.assertEqual(state.message, "lens group field lens: conflict")

        decision = handle_history_browser_key(state, key="f", page_size=2)
        self.assertEqual(decision.action, "cycle_group_limit_lens")
        self.assertEqual(state.limit_event_groups, 1)
        self.assertEqual(state.message, "lens group limit: 1")

        decision = handle_history_browser_key(state, key="v", page_size=2)
        self.assertEqual(decision.action, "toggle_matching_interventions_lens")
        self.assertTrue(state.only_matching_interventions)
        self.assertEqual(state.message, "lens matching interventions: on")

        decision = handle_history_browser_key(state, key="c", page_size=2)
        self.assertEqual(decision.action, "stage_compare")
        self.assertEqual(state.staged_compare_left_run_id, 10)
        self.assertEqual(state.message, "left run staged: 10")

        decision = handle_history_browser_key(state, key="c", page_size=2)
        self.assertEqual(decision.action, "stage_compare")
        self.assertEqual(state.active_view, "compare")
        self.assertEqual(state.selected_index, 1)
        self.assertEqual(state.message, "compare view active")

        state.focused_pane = "compare"
        decision = handle_history_browser_key(state, key="C", page_size=2)
        self.assertEqual(decision.action, "clear_compare")
        self.assertIsNone(state.staged_compare_left_run_id)
        self.assertEqual(state.active_view, "detail")
        self.assertEqual(state.focused_pane, "detail")
        self.assertEqual(state.message, "compare cleared")

    def test_handle_history_browser_key_routes_navigation_by_focus_and_view(
        self,
    ) -> None:
        state = HistoryListState(
            rows=[{"run_id": 10}, {"run_id": 20}, {"run_id": 30}],
            sqlite_path="dummy.sqlite3",
        )

        decision = handle_history_browser_key(state, key="j", page_size=2)
        self.assertEqual(decision.action, "move_down")
        self.assertEqual(state.selected_index, 1)

        decision = handle_history_browser_key(state, key="k", page_size=2)
        self.assertEqual(decision.action, "move_up")
        self.assertEqual(state.selected_index, 0)

        decision = handle_history_browser_key(state, key="]", page_size=2)
        self.assertEqual(decision.action, "step_next")
        self.assertEqual(state.selected_index, 1)

        decision = handle_history_browser_key(state, key="[", page_size=2)
        self.assertEqual(decision.action, "step_previous")
        self.assertEqual(state.selected_index, 0)

        decision = handle_history_browser_key(state, key="G", page_size=2)
        self.assertEqual(decision.action, "jump_bottom")
        self.assertEqual(state.selected_index, 2)
        self.assertEqual(state.message, "last run")

        decision = handle_history_browser_key(state, key="g", page_size=2)
        self.assertEqual(decision.action, "jump_top")
        self.assertEqual(state.selected_index, 0)
        self.assertEqual(state.message, "first run")

        state.focused_pane = "detail"
        state.cached_detail_lines = ["l1", "l2", "l3", "l4"]
        decision = handle_history_browser_key(state, key="\x1b[6~", page_size=2)
        self.assertEqual(decision.action, "page_down")
        self.assertEqual(state.detail_scroll_offset, 2)

        decision = handle_history_browser_key(state, key="\x1b[5~", page_size=2)
        self.assertEqual(decision.action, "page_up")
        self.assertEqual(state.detail_scroll_offset, 0)

        decision = handle_history_browser_key(state, key="G", page_size=2)
        self.assertEqual(decision.action, "jump_bottom")
        self.assertEqual(state.detail_scroll_offset, 2)
        self.assertEqual(state.message, "bottom of detail")

        decision = handle_history_browser_key(state, key="g", page_size=2)
        self.assertEqual(decision.action, "jump_top")
        self.assertEqual(state.detail_scroll_offset, 0)
        self.assertEqual(state.message, "top of detail")

        state = HistoryListState(
            rows=[{"run_id": 10}, {"run_id": 20}, {"run_id": 30}],
            sqlite_path="dummy.sqlite3",
            selected_index=0,
            staged_compare_left_run_id=10,
            active_view="compare",
            focused_pane="compare",
        )
        decision = handle_history_browser_key(state, key="]", page_size=2)
        self.assertEqual(decision.action, "step_next")
        self.assertEqual(state.selected_index, 1)

        decision = handle_history_browser_key(state, key="[", page_size=2)
        self.assertEqual(decision.action, "step_previous")
        self.assertEqual(state.selected_index, 1)
        self.assertEqual(state.message, "first compare target")

    def test_history_list_state_scrolls_detail_pane(self) -> None:
        state = HistoryListState(
            rows=[{"run_id": index} for index in range(8)], sqlite_path="dummy.sqlite3"
        )
        state.focused_pane = "detail"
        state.cached_detail_lines = ["line 1", "line 2", "line 3", "line 4", "line 5"]

        state.move_selection(2, page_size=2)
        self.assertEqual(state.detail_scroll_offset, 2)

        state.move_selection(10, page_size=2)
        self.assertEqual(state.detail_scroll_offset, 3)

        state.move_selection(-10, page_size=2)
        self.assertEqual(state.detail_scroll_offset, 0)

    def test_history_list_state_steps_runs_and_resets_detail_scroll(self) -> None:
        state = HistoryListState(
            rows=[{"run_id": index} for index in range(8)], sqlite_path="dummy.sqlite3"
        )
        state.focused_pane = "detail"
        state.detail_scroll_offset = 4
        state.cached_detail_run_id = 2
        state.cached_detail_lines = ["line 1", "line 2"]
        state.cached_compare_right_run_id = 3
        state.cached_compare_lines = ["compare 1"]

        state.step_run(1, page_size=3)

        self.assertEqual(state.selected_index, 1)
        self.assertEqual(state.scroll_offset, 0)
        self.assertEqual(state.detail_scroll_offset, 0)
        self.assertIsNone(state.cached_detail_run_id)
        self.assertIsNone(state.cached_detail_lines)
        self.assertIsNone(state.cached_compare_right_run_id)
        self.assertIsNone(state.cached_compare_lines)
        self.assertEqual(state.focused_pane, "detail")

    def test_history_list_state_step_run_stays_in_bounds(self) -> None:
        state = HistoryListState(
            rows=[{"run_id": index} for index in range(3)], sqlite_path="dummy.sqlite3"
        )

        state.step_run(-10, page_size=2)
        self.assertEqual(state.selected_index, 0)

        state.selected_index = 2
        state.step_run(10, page_size=2)
        self.assertEqual(state.selected_index, 2)

    @mock.patch("tianji.tui.get_previous_run_id", return_value=3)
    @mock.patch("tianji.tui.get_run_summary")
    def test_history_list_state_step_run_uses_persisted_previous_beyond_loaded_window(
        self, mock_get_run_summary, mock_get_previous_run_id
    ) -> None:
        mock_get_run_summary.return_value = {
            "run_id": 3,
            "schema_version": "tianji.run.v1",
            "generated_at": "2026-03-22T10:00:00+00:00",
            "mode": "fixture",
            "input_summary": {"raw_item_count": 2, "normalized_event_count": 1},
            "scenario_summary": {
                "dominant_field": "technology",
                "risk_level": "high",
                "headline": "Persisted run outside the loaded limit.",
                "event_groups": [],
            },
            "scored_events": [
                {
                    "event_id": "evt-3",
                    "divergence_score": 19.5,
                }
            ],
            "intervention_candidates": [],
        }
        state = HistoryListState(
            rows=[{"run_id": 5}, {"run_id": 4}],
            sqlite_path="dummy.sqlite3",
            selected_index=1,
            focused_pane="detail",
            cached_detail_run_id=4,
            cached_detail_lines=["detail"],
        )

        state.step_run(-1, page_size=2)

        self.assertEqual([row["run_id"] for row in state.rows], [4, 3])
        self.assertEqual(state.selected_index, 1)
        self.assertIsNone(state.cached_detail_run_id)
        self.assertIsNone(state.cached_detail_lines)
        mock_get_previous_run_id.assert_called_once_with(
            sqlite_path="dummy.sqlite3", run_id=4
        )
        mock_get_run_summary.assert_called_once_with(
            sqlite_path="dummy.sqlite3", run_id=3
        )

    @mock.patch("tianji.tui.get_next_run_id", return_value=6)
    @mock.patch("tianji.tui.get_run_summary")
    def test_history_list_state_step_run_uses_persisted_next_beyond_loaded_window(
        self, mock_get_run_summary, mock_get_next_run_id
    ) -> None:
        mock_get_run_summary.return_value = {
            "run_id": 6,
            "schema_version": "tianji.run.v1",
            "generated_at": "2026-03-22T10:00:00+00:00",
            "mode": "fixture",
            "input_summary": {"raw_item_count": 2, "normalized_event_count": 1},
            "scenario_summary": {
                "dominant_field": "diplomacy",
                "risk_level": "medium",
                "headline": "Persisted newer run outside the loaded limit.",
                "event_groups": [],
            },
            "scored_events": [],
            "intervention_candidates": [],
        }
        state = HistoryListState(
            rows=[{"run_id": 5}, {"run_id": 4}],
            sqlite_path="dummy.sqlite3",
            selected_index=0,
        )

        state.step_run(1, page_size=2)

        self.assertEqual([row["run_id"] for row in state.rows], [6, 5])
        self.assertEqual(state.selected_index, 0)
        mock_get_next_run_id.assert_called_once_with(
            sqlite_path="dummy.sqlite3", run_id=5
        )
        mock_get_run_summary.assert_called_once_with(
            sqlite_path="dummy.sqlite3", run_id=6
        )

    def test_history_list_state_step_compare_target_skips_left_run(self) -> None:
        state = HistoryListState(
            rows=[{"run_id": 10}, {"run_id": 20}, {"run_id": 30}],
            sqlite_path="dummy.sqlite3",
            selected_index=0,
            staged_compare_left_run_id=20,
            active_view="compare",
            focused_pane="compare",
        )

        state.step_compare_target(1, page_size=2)
        self.assertEqual(state.selected_index, 2)
        self.assertIsNone(state.message)

        state.step_compare_target(-1, page_size=2)
        self.assertEqual(state.selected_index, 0)
        self.assertIsNone(state.message)

    def test_history_list_state_step_compare_target_stays_in_bounds(self) -> None:
        state = HistoryListState(
            rows=[{"run_id": 10}, {"run_id": 20}, {"run_id": 30}],
            sqlite_path="dummy.sqlite3",
            selected_index=0,
            staged_compare_left_run_id=20,
            active_view="compare",
            focused_pane="compare",
        )

        state.step_compare_target(-1, page_size=2)
        self.assertEqual(state.selected_index, 0)
        self.assertEqual(state.message, "first compare target")

        state.selected_index = 2
        state.step_compare_target(1, page_size=2)
        self.assertEqual(state.selected_index, 2)
        self.assertEqual(state.message, "last compare target")

    @mock.patch("tianji.tui.get_next_run_id", side_effect=[20, 30])
    @mock.patch("tianji.tui.get_run_summary")
    def test_history_list_state_step_compare_target_uses_persisted_semantics_beyond_loaded_window(
        self, mock_get_run_summary, mock_get_next_run_id
    ) -> None:
        mock_get_run_summary.return_value = {
            "run_id": 30,
            "schema_version": "tianji.run.v1",
            "generated_at": "2026-03-22T10:00:00+00:00",
            "mode": "fixture",
            "input_summary": {"raw_item_count": 2, "normalized_event_count": 1},
            "scenario_summary": {
                "dominant_field": "conflict",
                "risk_level": "high",
                "headline": "Persisted compare target outside the loaded limit.",
                "event_groups": [],
            },
            "scored_events": [],
            "intervention_candidates": [],
        }
        state = HistoryListState(
            rows=[{"run_id": 20}, {"run_id": 10}],
            sqlite_path="dummy.sqlite3",
            selected_index=0,
            staged_compare_left_run_id=20,
            active_view="compare",
            focused_pane="compare",
            cached_compare_right_run_id=10,
            cached_compare_lines=["compare"],
        )

        state.step_compare_target(1, page_size=2)

        self.assertEqual([row["run_id"] for row in state.rows], [30, 20])
        self.assertEqual(state.selected_index, 0)
        self.assertIsNone(state.cached_compare_right_run_id)
        self.assertIsNone(state.cached_compare_lines)
        self.assertEqual(
            mock_get_next_run_id.call_args_list[0].kwargs,
            {"sqlite_path": "dummy.sqlite3", "run_id": 20},
        )
        self.assertEqual(
            mock_get_next_run_id.call_args_list[1].kwargs,
            {"sqlite_path": "dummy.sqlite3", "run_id": 20},
        )
        mock_get_run_summary.assert_called_once_with(
            sqlite_path="dummy.sqlite3", run_id=30
        )

    @mock.patch("tianji.tui.get_previous_run_id", return_value=None)
    def test_history_list_state_step_run_reports_first_persisted_boundary(
        self, mock_get_previous_run_id
    ) -> None:
        state = HistoryListState(
            rows=[{"run_id": 5}, {"run_id": 4}],
            sqlite_path="dummy.sqlite3",
            selected_index=1,
        )

        state.step_run(-1, page_size=2)

        self.assertEqual(state.selected_index, 1)
        self.assertEqual(state.message, "first run")
        mock_get_previous_run_id.assert_called_once_with(
            sqlite_path="dummy.sqlite3", run_id=4
        )

    @mock.patch("tianji.tui.get_next_run_id", side_effect=[20, None])
    def test_history_list_state_step_compare_target_reports_last_persisted_boundary(
        self, mock_get_next_run_id
    ) -> None:
        state = HistoryListState(
            rows=[{"run_id": 20}, {"run_id": 10}],
            sqlite_path="dummy.sqlite3",
            selected_index=0,
            staged_compare_left_run_id=20,
            active_view="compare",
            focused_pane="compare",
        )

        state.step_compare_target(1, page_size=2)

        self.assertEqual(state.selected_index, 0)
        self.assertEqual(state.message, "last compare target")
        self.assertEqual(len(mock_get_next_run_id.call_args_list), 2)

    def test_history_list_state_sets_transient_messages_on_bounds(self) -> None:
        state = HistoryListState(
            rows=[{"run_id": index} for index in range(3)], sqlite_path="dummy.sqlite3"
        )

        state.move_selection(-1, page_size=2)
        self.assertEqual(state.message, "first run")

        state.message = None
        state.move_selection(10, page_size=2)
        self.assertIsNone(state.message)

        state.move_selection(1, page_size=2)
        self.assertEqual(state.message, "last run")

        state.message = None
        state.step_run(-10, page_size=2)
        self.assertIsNone(state.message)

        state.step_run(-1, page_size=2)
        self.assertEqual(state.message, "first run")

        state.message = None
        state.step_run(10, page_size=2)
        self.assertIsNone(state.message)

        state.step_run(1, page_size=2)
        self.assertEqual(state.message, "last run")

        state.focused_pane = "detail"
        state.cached_detail_lines = ["line 1", "line 2", "line 3"]
        state.detail_scroll_offset = 0

        state.message = None
        state.move_selection(-1, page_size=2)
        self.assertEqual(state.message, "top of detail")

        state.message = None
        state.move_selection(10, page_size=2)
        self.assertIsNone(state.message)

        state.move_selection(1, page_size=2)
        self.assertEqual(state.message, "bottom of detail")

    def test_format_history_row_includes_core_fields_and_truncates_headline(
        self,
    ) -> None:
        row = {
            "run_id": 7,
            "generated_at": "2026-03-22T10:00:00+00:00",
            "mode": "fixture",
            "dominant_field": "technology",
            "risk_level": "high",
            "event_group_count": 2,
            "top_divergence_score": 18.375,
            "headline": "The strongest current branch is technology, driven by a very long explanation that should truncate.",
        }

        formatted = format_history_row(row, width=72)

        self.assertIn("  7", formatted)
        self.assertIn("fixture", formatted)
        self.assertIn("technology", formatted)
        self.assertIn("high", formatted)
        self.assertIn("18.38", formatted)
        self.assertLessEqual(len(formatted), 72)
        self.assertTrue(formatted.endswith("…") or formatted.endswith(" "))

        formatted_staged = format_history_row(row, width=72, is_staged_left=True)
        self.assertTrue(formatted_staged.startswith("*  7"))

    def test_wrap_text_splits_long_lines(self) -> None:
        text = "This is a very long headline that should be wrapped to multiple lines."
        lines = wrap_text(text, width=20)
        self.assertEqual(lines[0], "This is a very long")
        self.assertEqual(lines[1], "headline that should")
        self.assertEqual(lines[2], "be wrapped to")
        self.assertEqual(lines[3], "multiple lines.")

    def test_format_run_detail_includes_core_fields(self) -> None:
        summary = {
            "run_id": 42,
            "generated_at": "2026-03-22T10:00:00+00:00",
            "mode": "fixture",
            "input_summary": {"raw_item_count": 10, "normalized_event_count": 8},
            "scenario_summary": {
                "dominant_field": "technology",
                "risk_level": "high",
                "headline": "A major technology event occurred.",
                "event_groups": [{"dominant_field": "technology", "member_count": 3}],
            },
            "scored_events": [
                {
                    "title": "Tech Event 1",
                    "dominant_field": "technology",
                    "impact_score": 14.03,
                    "field_attraction": 7.75,
                    "divergence_score": 19.58,
                },
                {
                    "title": "Diplomatic follow-on event with a long title that should truncate",
                    "dominant_field": "diplomacy",
                    "impact_score": 12.09,
                    "field_attraction": 6.17,
                    "divergence_score": 16.19,
                },
            ],
            "intervention_candidates": [
                {
                    "target": "Tech Sector",
                    "intervention_type": "investigate",
                    "expected_effect": "Clarify the situation.",
                },
                {
                    "target": "Diplomatic Channel",
                    "intervention_type": "monitor",
                    "reason": "High tension.",
                },
            ],
        }
        lines = format_run_detail(summary, width=40)
        text = "\n".join(lines)
        self.assertIn("Run #42", text)
        self.assertIn("2026-03-22 10:00:00", text)
        self.assertIn("fixture", text)
        self.assertIn("Items: 10 raw -> 8 normalized", text)
        self.assertIn("Scenario: technology • Risk: high", text)
        self.assertIn("A major technology event occurred.", text)
        self.assertIn("Event Groups: 1", text)
        self.assertIn("1. technology (3 members)", text)
        self.assertIn("No summary available.", text)
        self.assertIn("Scored Events: 2", text)
        self.assertIn("1. technology Dv 19.58 Im 14.03", text)
        self.assertIn("Tech Event 1", text)
        self.assertIn("2. diplomacy Dv 16.19 Im 12.09", text)
        self.assertIn("Interventions: 2", text)
        self.assertIn("1. [investigate] Tech Sector", text)
        self.assertIn("Clarify the situation.", text)
        self.assertIn("2. [monitor] Diplomatic Channel", text)
        self.assertIn("High tension.", text)

    def test_format_run_detail_limits_event_group_preview_and_truncates_summaries(
        self,
    ) -> None:
        summary = {
            "run_id": 1,
            "generated_at": "2026-03-22T10:00:00+00:00",
            "mode": "fixture",
            "scenario_summary": {
                "headline": "Short headline.",
                "event_groups": [
                    {
                        "dominant_field": "technology",
                        "member_count": 3,
                        "causal_summary": "First causal summary that is intentionally very long and should truncate",
                    },
                    {
                        "dominant_field": "diplomacy",
                        "member_count": 2,
                        "headline_title": "Second headline title that is also very long and should truncate",
                    },
                    {
                        "dominant_field": "conflict",
                        "member_count": 4,
                        "causal_summary": "Third causal summary that is also very long and should truncate",
                    },
                    {
                        "dominant_field": "economy",
                        "member_count": 1,
                        "causal_summary": "Fourth summary should not appear in preview",
                    },
                ],
            },
            "scored_events": [],
            "intervention_candidates": [],
        }

        lines = format_run_detail(summary, width=34)
        text = "\n".join(lines)

        self.assertIn("1. technology (3 members)", text)
        self.assertIn("2. diplomacy (2 members)", text)
        self.assertIn("3. conflict (4 members)", text)
        self.assertNotIn("Fourth summary", text)
        for line in lines:
            self.assertLessEqual(len(line), 34)

    def test_format_run_detail_limits_scored_event_preview_and_truncates_titles(
        self,
    ) -> None:
        summary = {
            "run_id": 1,
            "generated_at": "2026-03-22T10:00:00+00:00",
            "mode": "fixture",
            "scenario_summary": {"headline": "Short headline."},
            "scored_events": [
                {
                    "title": "First event title that is intentionally very long",
                    "dominant_field": "technology",
                    "impact_score": 14.75,
                    "field_attraction": 7.75,
                    "divergence_score": 20.05,
                },
                {
                    "title": "Second event title that is also very long",
                    "dominant_field": "diplomacy",
                    "impact_score": 12.09,
                    "field_attraction": 6.17,
                    "divergence_score": 16.19,
                },
                {
                    "title": "Third event title that is also very long",
                    "dominant_field": "conflict",
                    "impact_score": 16.07,
                    "field_attraction": 3.60,
                    "divergence_score": 15.31,
                },
                {
                    "title": "Fourth event should not appear in preview",
                    "dominant_field": "economy",
                    "impact_score": 9.11,
                    "field_attraction": 4.22,
                    "divergence_score": 11.84,
                },
            ],
            "intervention_candidates": [],
        }

        lines = format_run_detail(summary, width=34)
        text = "\n".join(lines)

        self.assertIn("1. technology Dv 20.05", text)
        self.assertIn("2. diplomacy Dv 16.19", text)
        self.assertIn("3. conflict Dv 15.31", text)
        self.assertNotIn("Fourth event", text)
        for line in lines:
            self.assertLessEqual(len(line), 34)

    def test_format_run_detail_limits_intervention_preview_and_truncates_effects(
        self,
    ) -> None:
        summary = {
            "run_id": 1,
            "generated_at": "2026-03-22T10:00:00+00:00",
            "mode": "fixture",
            "scenario_summary": {"headline": "Short headline."},
            "scored_events": [],
            "intervention_candidates": [
                {
                    "target": "Target 1",
                    "intervention_type": "type1",
                    "expected_effect": "First effect that is intentionally very long and should truncate",
                },
                {
                    "target": "Target 2",
                    "intervention_type": "type2",
                    "expected_effect": "Second effect that is also very long and should truncate",
                },
                {
                    "target": "Target 3",
                    "intervention_type": "type3",
                    "expected_effect": "Third effect that is also very long and should truncate",
                },
                {
                    "target": "Target 4",
                    "intervention_type": "type4",
                    "expected_effect": "Fourth effect should not appear in preview",
                },
            ],
        }

        lines = format_run_detail(summary, width=34)
        text = "\n".join(lines)

        self.assertIn("1. [type1] Target 1", text)
        self.assertIn("2. [type2] Target 2", text)
        self.assertIn("3. [type3] Target 3", text)
        self.assertNotIn("Fourth effect", text)
        for line in lines:
            self.assertLessEqual(len(line), 34)

    def test_format_status_footer_shows_bounds_and_state(self) -> None:
        state = HistoryListState(
            rows=[{"run_id": 10}, {"run_id": 20}, {"run_id": 30}],
            sqlite_path="dummy.sqlite3",
            selected_index=0,
            focused_pane="list",
            zoomed=False,
        )

        footer = format_status_footer(state, width=50)
        self.assertIn("run 1/3", footer)
        self.assertIn("id:10", footer)
        self.assertIn("[first]", footer)
        self.assertIn("LIST", footer)
        self.assertNotIn("VIEW LENS", footer)
        self.assertNotIn("ZOOM", footer)
        self.assertNotIn("COMPARE", footer)

        state.selected_index = 1
        state.focused_pane = "detail"
        state.zoomed = True
        footer = format_status_footer(state, width=50)
        self.assertIn("run 2/3", footer)
        self.assertIn("id:20", footer)
        self.assertIn("[-]", footer)
        self.assertIn("DETAIL", footer)
        self.assertIn("ZOOM", footer)

        state.staged_compare_left_run_id = 10
        footer = format_status_footer(state, width=50)
        self.assertIn("COMPARE L:10", footer)
        self.assertNotIn("R:", footer)

        state.active_view = "compare"
        state.cached_compare_right_run_id = 20
        footer = format_status_footer(state, width=50)
        self.assertIn("COMPARE L:10 R:20", footer)

        state.staged_compare_left_run_id = None
        state.active_view = "detail"
        state.zoomed = False
        state.selected_index = 2
        footer = format_status_footer(state, width=50)
        self.assertIn("run 3/3", footer)
        self.assertIn("id:30", footer)
        self.assertIn("[last]", footer)

        state.rows = [{"run_id": 40}]
        state.selected_index = 0
        footer = format_status_footer(state, width=50)
        self.assertIn("run 1/1", footer)
        self.assertIn("id:40", footer)
        self.assertIn("[only]", footer)

        state.rows = []
        footer = format_status_footer(state, width=50)
        self.assertIn("0/0", footer)

    @mock.patch("tianji.tui.compare_runs", return_value=None)
    def test_status_footer_and_header_expose_active_lens_state(
        self, mock_compare_runs
    ) -> None:
        state = HistoryListState(
            rows=[{"run_id": 10}, {"run_id": 20}],
            sqlite_path="dummy.sqlite3",
            selected_index=1,
            focused_pane="compare",
            active_view="compare",
            staged_compare_left_run_id=10,
            dominant_field="technology",
            limit_scored_events=1,
            group_dominant_field="diplomacy",
            limit_event_groups=3,
            only_matching_interventions=True,
        )

        footer = format_status_footer(state, width=120)
        self.assertIn("COMPARE L:10 R:20", footer)
        self.assertIn(
            "VIEW LENS:EV=TECHNOLOGY,TOP=1,GRP=DIPLOMACY,GROUPS=3,MATCH-INT", footer
        )

        layout = build_layout(state, height=20, width=120, page_size=10)
        header_text = cast(Text, layout["header"].renderable).plain
        self.assertIn("a/s/d/f/v lens view", header_text)
        self.assertIn(
            "lens:ev=technology,top=1,grp=diplomacy,groups=3,match-int",
            header_text,
        )
        mock_compare_runs.assert_called_once_with(
            sqlite_path="dummy.sqlite3",
            left_run_id=10,
            right_run_id=20,
            dominant_field="technology",
            limit_scored_events=1,
            group_dominant_field="diplomacy",
            limit_event_groups=3,
            only_matching_interventions=True,
        )

    def test_help_text_lists_all_five_lens_controls(self) -> None:
        help_text = build_help_text().plain

        self.assertIn("a           : Cycle scored-event field lens", help_text)
        self.assertIn("s           : Cycle scored-event limit lens", help_text)
        self.assertIn("d           : Cycle event-group field lens", help_text)
        self.assertIn("f           : Cycle event-group limit lens", help_text)
        self.assertIn("v           : Toggle intervention-match lens", help_text)
        self.assertIn(
            "Active lens  : Projects detail/compare, list stays persisted",
            help_text,
        )

    def test_format_lens_summary_and_change_message_use_projection_vocabulary(
        self,
    ) -> None:
        state = HistoryListState(rows=[{"run_id": 10}], sqlite_path="dummy.sqlite3")

        self.assertEqual(format_active_lens_summary(state), "lens:all-runs")
        self.assertEqual(
            format_lens_change_message("event field lens", "technology"),
            "lens event field lens: technology",
        )

        state.dominant_field = "technology"
        state.limit_scored_events = 1
        state.group_dominant_field = "diplomacy"
        state.limit_event_groups = 3
        state.only_matching_interventions = True
        self.assertEqual(
            format_active_lens_summary(state),
            "lens:ev=technology,top=1,grp=diplomacy,groups=3,match-int",
        )

    def test_format_compare_detail_includes_core_fields(self) -> None:
        compare_result: dict[str, object] = {
            "left": {
                "run_id": 1,
                "mode": "fixture",
                "dominant_field": "technology",
                "risk_level": "high",
                "headline": "Left headline",
                "top_event_group": {
                    "dominant_field": "technology",
                    "member_count": 3,
                },
                "top_scored_event": {
                    "dominant_field": "technology",
                    "divergence_score": 19.58,
                    "impact_score": 14.03,
                },
                "top_intervention": {
                    "target": "Tech Sector",
                    "intervention_type": "investigate",
                },
            },
            "right": {
                "run_id": 2,
                "mode": "fetch",
                "dominant_field": "diplomacy",
                "risk_level": "medium",
                "headline": "Right headline",
                "top_event_group": {
                    "dominant_field": "diplomacy",
                    "member_count": 2,
                },
                "top_scored_event": {
                    "dominant_field": "diplomacy",
                    "divergence_score": 16.19,
                    "impact_score": 12.09,
                },
                "top_intervention": {
                    "target": "Diplomatic Channel",
                    "intervention_type": "monitor",
                },
            },
            "diff": {
                "dominant_field_changed": True,
                "risk_level_changed": True,
                "raw_item_count_delta": 5,
                "normalized_event_count_delta": 3,
                "event_group_count_delta": 1,
                "top_event_group_changed": True,
                "top_event_group_evidence_diff": {
                    "comparable": True,
                    "member_count_delta": 1,
                    "evidence_chain_link_count_delta": 0,
                    "right_only_member_event_ids": ["evt-2"],
                    "shared_keywords_added": ["new-kw"],
                    "chain_summary_changed": True,
                },
                "top_scored_event_changed": False,
                "top_scored_event_comparable": True,
                "top_divergence_score_delta": 2.5,
                "top_impact_score_delta": -1.2,
                "top_field_attraction_delta": 0.0,
                "top_intervention_changed": True,
            },
        }
        lines = format_compare_detail(compare_result, width=60)
        text = "\n".join(lines)
        self.assertIn("Compare: Run #1 (Left) vs Run #2 (Right)", text)
        self.assertIn("[Left] fixture • technology • Risk: high", text)
        self.assertIn("Left headline", text)
        self.assertIn("Top Group: technology (3 members)", text)
        self.assertIn("Top Event: technology Dv 19.58 Im 14.03", text)
        self.assertIn("Top Action: [investigate] Tech Sector", text)
        self.assertIn("[Right] fetch • diplomacy • Risk: medium", text)
        self.assertIn("Right headline", text)
        self.assertIn("Top Group: diplomacy (2 members)", text)
        self.assertIn("Top Event: diplomacy Dv 16.19 Im 12.09", text)
        self.assertIn("Top Action: [monitor] Diplomatic Channel", text)
        self.assertIn("Diff Highlights:", text)
        self.assertIn("Field changed: technology -> diplomacy", text)
        self.assertIn("Risk changed: high -> medium", text)
        self.assertIn("Items: +5 raw, +3 normalized", text)
        self.assertIn("Event Groups: +1", text)
        self.assertIn("Top event group changed", text)
        self.assertIn("Top group evidence (Comparable):", text)
        self.assertIn("Members: +1, Links: +0", text)
        self.assertIn("Added IDs: evt-2", text)
        self.assertIn("Added keywords: new-kw", text)
        self.assertIn("Chain summary changed", text)
        self.assertIn("Top event score deltas: Dv +2.50 Im -1.20 Fa +0.00", text)
        self.assertIn("Top intervention changed", text)

    def test_format_top_group_evidence_diff_lines_comparable(self) -> None:
        evidence_diff: dict[str, object] = {
            "comparable": True,
            "member_count_delta": 2,
            "evidence_chain_link_count_delta": 1,
            "right_only_member_event_ids": ["evt-3", "evt-4"],
            "left_only_member_event_ids": [],
            "shared_keywords_added": ["new-kw"],
            "shared_keywords_removed": ["old-kw"],
            "chain_summary_changed": True,
        }
        lines = format_top_group_evidence_diff_lines(evidence_diff, width=60)
        text = "\n".join(lines)
        self.assertIn("Top group evidence (Comparable):", text)
        self.assertIn("Members: +2, Links: +1", text)
        self.assertIn("Added IDs: evt-3, evt-4", text)
        self.assertNotIn("Removed IDs", text)
        self.assertIn("Added keywords: new-kw", text)
        self.assertIn("Removed keywords: old-kw", text)
        self.assertIn("Chain summary changed", text)

    def test_format_top_group_evidence_diff_lines_contrast(self) -> None:
        evidence_diff: dict[str, object] = {
            "comparable": False,
            "member_count_delta": -1,
            "evidence_chain_link_count_delta": 0,
            "right_only_member_event_ids": ["evt-2"],
            "left_only_member_event_ids": ["evt-1"],
            "shared_keywords_added": [],
            "shared_keywords_removed": [],
            "chain_summary_changed": False,
        }
        lines = format_top_group_evidence_diff_lines(evidence_diff, width=60)
        text = "\n".join(lines)
        self.assertIn("Top group evidence (Contrast):", text)
        self.assertIn("Members: -1, Links: +0", text)
        self.assertIn("Added IDs: evt-2", text)
        self.assertIn("Removed IDs: evt-1", text)
        self.assertNotIn("Added keywords", text)
        self.assertNotIn("Removed keywords", text)
        self.assertNotIn("Chain summary changed", text)

    def test_format_delta_handles_none(self) -> None:
        self.assertEqual(format_delta(None), "N/A")
        self.assertEqual(format_delta(2.5), "+2.50")
        self.assertEqual(format_delta(-1.2), "-1.20")
        self.assertEqual(format_delta(0), "+0.00")

    def test_get_compare_similarity_summary_identical(self) -> None:
        diff: dict[str, object] = {
            "dominant_field_changed": False,
            "risk_level_changed": False,
            "raw_item_count_delta": 0,
            "normalized_event_count_delta": 0,
            "event_group_count_delta": 0,
            "top_event_group_changed": False,
            "top_scored_event_changed": False,
            "top_divergence_score_delta": 0.0,
            "top_impact_score_delta": 0.0,
            "top_field_attraction_delta": 0.0,
            "top_intervention_changed": False,
        }
        summary = get_compare_similarity_summary(diff)
        self.assertEqual(
            summary, "Effectively identical: no meaningful differences found."
        )

    def test_get_compare_similarity_summary_high_similarity(self) -> None:
        diff: dict[str, object] = {
            "dominant_field_changed": False,
            "risk_level_changed": False,
            "raw_item_count_delta": 1,
            "normalized_event_count_delta": 0,
            "event_group_count_delta": 0,
            "top_event_group_changed": False,
            "top_scored_event_changed": False,
            "top_divergence_score_delta": 0.0,
            "top_impact_score_delta": 0.0,
            "top_field_attraction_delta": 0.0,
            "top_intervention_changed": False,
        }
        summary = get_compare_similarity_summary(diff)
        self.assertEqual(
            summary, "No major differences: top signals and fields remain stable."
        )

    def test_get_compare_similarity_summary_major_changes(self) -> None:
        diff: dict[str, object] = {
            "dominant_field_changed": True,
            "risk_level_changed": False,
        }
        summary = get_compare_similarity_summary(diff)
        self.assertIsNone(summary)

    def test_history_list_state_compare_transitions(self) -> None:
        state = HistoryListState(
            rows=[{"run_id": 1}, {"run_id": 2}], sqlite_path="dummy.sqlite3"
        )
        self.assertIsNone(state.staged_compare_left_run_id)
        self.assertEqual(state.active_view, "detail")

        state.stage_compare(1)
        self.assertEqual(state.staged_compare_left_run_id, 1)
        self.assertEqual(state.message, "left run staged: 1")
        self.assertEqual(state.active_view, "detail")

        state.stage_compare(1)
        self.assertEqual(state.staged_compare_left_run_id, 1)
        self.assertEqual(state.selected_index, 1)
        self.assertEqual(state.message, "compare view active")
        self.assertEqual(state.active_view, "compare")

        state.clear_compare()
        self.assertIsNone(state.staged_compare_left_run_id)
        self.assertEqual(state.message, "compare cleared")
        self.assertEqual(state.active_view, "detail")

    def test_history_list_state_compare_transitions_single_run(self) -> None:
        state = HistoryListState(rows=[{"run_id": 1}], sqlite_path="dummy.sqlite3")
        self.assertIsNone(state.staged_compare_left_run_id)
        self.assertEqual(state.active_view, "detail")

        state.stage_compare(1)
        self.assertEqual(state.staged_compare_left_run_id, 1)
        self.assertEqual(state.message, "left run staged: 1")
        self.assertEqual(state.active_view, "detail")

        state.stage_compare(1)
        self.assertEqual(state.staged_compare_left_run_id, 1)
        self.assertEqual(state.message, "cannot compare a run with itself")
        self.assertEqual(state.active_view, "detail")

    @mock.patch("tianji.tui.compare_runs", return_value=None)
    def test_build_compare_panel_title_shows_pairing(self, mock_compare) -> None:
        state = HistoryListState(
            rows=[{"run_id": 10}, {"run_id": 20}],
            sqlite_path="dummy.sqlite3",
            selected_index=1,
        )

        panel = build_compare_panel(state, width=50, page_size=10)
        self.assertEqual(panel.title, Text(" Compare ", style="bold"))

        state.staged_compare_left_run_id = 10
        panel = build_compare_panel(state, width=50, page_size=10)
        self.assertEqual(panel.title, Text(" Compare L:10 R:20 ", style="bold"))

        state.focused_pane = "compare"
        panel = build_compare_panel(state, width=50, page_size=10)
        self.assertEqual(
            panel.title, Text(" [Compare L:10 R:20] ", style="reverse bold")
        )

    @mock.patch("tianji.tui.compare_runs", return_value=None)
    def test_build_compare_panel_title_shows_scroll_indicator(
        self, mock_compare
    ) -> None:
        state = HistoryListState(
            rows=[{"run_id": 10}, {"run_id": 20}],
            sqlite_path="dummy.sqlite3",
            selected_index=1,
            staged_compare_left_run_id=10,
        )
        state.cached_compare_right_run_id = 20
        state.cached_compare_lens_key = (None, None, None, None, False)
        state.cached_compare_lines = ["line 1", "line 2", "line 3", "line 4", "line 5"]
        state.detail_scroll_offset = 1

        panel = build_compare_panel(state, width=50, page_size=2)
        self.assertEqual(panel.title, Text(" Compare L:10 R:20 2-3/5 ", style="bold"))

        state.focused_pane = "compare"
        panel = build_compare_panel(state, width=50, page_size=2)
        self.assertEqual(
            panel.title, Text(" [Compare L:10 R:20 2-3/5] ", style="reverse bold")
        )

    @mock.patch("tianji.tui.get_run_summary")
    def test_build_detail_panel_passes_active_lens_kwargs(
        self, mock_get_run_summary
    ) -> None:
        mock_get_run_summary.return_value = {
            "run_id": 10,
            "generated_at": "2026-03-22T10:00:00+00:00",
            "mode": "fixture",
            "input_summary": {"raw_item_count": 2, "normalized_event_count": 1},
            "scenario_summary": {
                "dominant_field": "technology",
                "risk_level": "high",
                "headline": "Projected detail.",
                "event_groups": [{"dominant_field": "diplomacy", "member_count": 2}],
            },
            "scored_events": [
                {
                    "title": "Projected event",
                    "dominant_field": "technology",
                    "impact_score": 14.03,
                    "field_attraction": 7.75,
                    "divergence_score": 19.58,
                }
            ],
            "intervention_candidates": [
                {
                    "target": "Projected target",
                    "intervention_type": "monitor",
                    "expected_effect": "Projected effect.",
                }
            ],
        }
        state = HistoryListState(
            rows=[{"run_id": 10}],
            sqlite_path="dummy.sqlite3",
            dominant_field="technology",
            limit_scored_events=1,
            group_dominant_field="diplomacy",
            limit_event_groups=3,
            only_matching_interventions=True,
        )

        panel = build_detail_panel(state, width=60, page_size=20)

        mock_get_run_summary.assert_called_once_with(
            sqlite_path="dummy.sqlite3",
            run_id=10,
            dominant_field="technology",
            limit_scored_events=1,
            group_dominant_field="diplomacy",
            limit_event_groups=3,
            only_matching_interventions=True,
        )
        self.assertEqual(state.cached_detail_run_id, 10)
        self.assertEqual(
            state.cached_detail_lens_key,
            ("technology", 1, "diplomacy", 3, True),
        )
        detail_text = cast(Text, panel.renderable).plain
        self.assertIn("Projected detail.", detail_text)

    @mock.patch("tianji.tui.get_run_summary")
    def test_build_detail_panel_reuses_cached_projection_until_lens_changes(
        self, mock_get_run_summary
    ) -> None:
        mock_get_run_summary.return_value = {
            "run_id": 10,
            "generated_at": "2026-03-22T10:00:00+00:00",
            "mode": "fixture",
            "input_summary": {"raw_item_count": 2, "normalized_event_count": 1},
            "scenario_summary": {
                "dominant_field": "technology",
                "risk_level": "high",
                "headline": "Cached detail.",
                "event_groups": [],
            },
            "scored_events": [],
            "intervention_candidates": [],
        }
        state = HistoryListState(rows=[{"run_id": 10}], sqlite_path="dummy.sqlite3")

        first_panel = build_detail_panel(state, width=60, page_size=20)
        second_panel = build_detail_panel(state, width=60, page_size=20)

        self.assertEqual(mock_get_run_summary.call_count, 1)
        self.assertIn("Cached detail.", cast(Text, first_panel.renderable).plain)
        self.assertIn("Cached detail.", cast(Text, second_panel.renderable).plain)

        state.cycle_limit_scored_events_lens()
        build_detail_panel(state, width=60, page_size=20)

        self.assertEqual(mock_get_run_summary.call_count, 2)
        self.assertEqual(
            mock_get_run_summary.call_args_list[1].kwargs,
            {
                "sqlite_path": "dummy.sqlite3",
                "run_id": 10,
                "dominant_field": None,
                "limit_scored_events": 1,
                "group_dominant_field": None,
                "limit_event_groups": None,
                "only_matching_interventions": False,
            },
        )
        self.assertEqual(state.cached_detail_lens_key, (None, 1, None, None, False))

    @mock.patch("tianji.tui.compare_runs")
    def test_build_compare_panel_passes_active_lens_kwargs(
        self, mock_compare_runs
    ) -> None:
        mock_compare_runs.return_value = {
            "left": {
                "run_id": 10,
                "mode": "fixture",
                "dominant_field": "technology",
                "risk_level": "high",
                "headline": "Left headline",
                "top_event_group": None,
                "top_scored_event": None,
                "top_intervention": None,
            },
            "right": {
                "run_id": 20,
                "mode": "fixture",
                "dominant_field": "technology",
                "risk_level": "high",
                "headline": "Right headline",
                "top_event_group": None,
                "top_scored_event": None,
                "top_intervention": None,
            },
            "diff": {
                "dominant_field_changed": False,
                "risk_level_changed": False,
                "raw_item_count_delta": 0,
                "normalized_event_count_delta": 0,
                "event_group_count_delta": 0,
                "top_event_group_changed": False,
                "top_scored_event_changed": False,
                "top_intervention_changed": False,
            },
        }
        state = HistoryListState(
            rows=[{"run_id": 10}, {"run_id": 20}],
            sqlite_path="dummy.sqlite3",
            selected_index=1,
            staged_compare_left_run_id=10,
            dominant_field="technology",
            limit_scored_events=1,
            group_dominant_field="diplomacy",
            limit_event_groups=3,
            only_matching_interventions=True,
        )

        panel = build_compare_panel(state, width=60, page_size=20)

        mock_compare_runs.assert_called_once_with(
            sqlite_path="dummy.sqlite3",
            left_run_id=10,
            right_run_id=20,
            dominant_field="technology",
            limit_scored_events=1,
            group_dominant_field="diplomacy",
            limit_event_groups=3,
            only_matching_interventions=True,
        )
        self.assertEqual(state.cached_compare_right_run_id, 20)
        self.assertEqual(
            state.cached_compare_lens_key,
            ("technology", 1, "diplomacy", 3, True),
        )
        compare_text = cast(Text, panel.renderable).plain
        self.assertIn("Compare: Run #10 (Left) vs Run #20 (Right)", compare_text)

    @mock.patch("tianji.tui.compare_runs")
    def test_build_compare_panel_reuses_cached_projection_until_lens_changes(
        self, mock_compare_runs
    ) -> None:
        mock_compare_runs.return_value = {
            "left": {
                "run_id": 10,
                "mode": "fixture",
                "dominant_field": "technology",
                "risk_level": "high",
                "headline": "Left headline",
                "top_event_group": None,
                "top_scored_event": None,
                "top_intervention": None,
            },
            "right": {
                "run_id": 20,
                "mode": "fixture",
                "dominant_field": "technology",
                "risk_level": "high",
                "headline": "Right headline",
                "top_event_group": None,
                "top_scored_event": None,
                "top_intervention": None,
            },
            "diff": {
                "dominant_field_changed": False,
                "risk_level_changed": False,
                "raw_item_count_delta": 0,
                "normalized_event_count_delta": 0,
                "event_group_count_delta": 0,
                "top_event_group_changed": False,
                "top_scored_event_changed": False,
                "top_intervention_changed": False,
            },
        }
        state = HistoryListState(
            rows=[{"run_id": 10}, {"run_id": 20}],
            sqlite_path="dummy.sqlite3",
            selected_index=1,
            staged_compare_left_run_id=10,
        )

        first_panel = build_compare_panel(state, width=60, page_size=20)
        second_panel = build_compare_panel(state, width=60, page_size=20)

        self.assertEqual(mock_compare_runs.call_count, 1)
        self.assertIn(
            "Compare: Run #10 (Left) vs Run #20 (Right)",
            cast(Text, first_panel.renderable).plain,
        )
        self.assertIn(
            "Compare: Run #10 (Left) vs Run #20 (Right)",
            cast(Text, second_panel.renderable).plain,
        )

        state.toggle_only_matching_interventions()
        build_compare_panel(state, width=60, page_size=20)

        self.assertEqual(mock_compare_runs.call_count, 2)
        self.assertEqual(
            mock_compare_runs.call_args_list[1].kwargs,
            {
                "sqlite_path": "dummy.sqlite3",
                "left_run_id": 10,
                "right_run_id": 20,
                "dominant_field": None,
                "limit_scored_events": None,
                "group_dominant_field": None,
                "limit_event_groups": None,
                "only_matching_interventions": True,
            },
        )
        self.assertEqual(state.cached_compare_lens_key, (None, None, None, None, True))

    @mock.patch("tianji.tui.compare_runs")
    def test_compare_panel_lens_changes_invalidate_cache_without_breaking_compare_flow(
        self, mock_compare_runs
    ) -> None:
        mock_compare_runs.return_value = {
            "left": {
                "run_id": 10,
                "mode": "fixture",
                "dominant_field": "technology",
                "risk_level": "high",
                "headline": "Left headline",
                "top_event_group": None,
                "top_scored_event": None,
                "top_intervention": None,
            },
            "right": {
                "run_id": 30,
                "mode": "fixture",
                "dominant_field": "diplomacy",
                "risk_level": "medium",
                "headline": "Right headline",
                "top_event_group": None,
                "top_scored_event": None,
                "top_intervention": None,
            },
            "diff": {
                "dominant_field_changed": True,
                "risk_level_changed": True,
                "raw_item_count_delta": 1,
                "normalized_event_count_delta": 1,
                "event_group_count_delta": 0,
                "top_event_group_changed": False,
                "top_scored_event_changed": False,
                "top_intervention_changed": False,
            },
        }
        state = HistoryListState(
            rows=[{"run_id": 10}, {"run_id": 20}, {"run_id": 30}],
            sqlite_path="dummy.sqlite3",
            selected_index=0,
            focused_pane="compare",
        )

        state.stage_compare(10, page_size=2)
        self.assertEqual(state.staged_compare_left_run_id, 10)
        self.assertEqual(state.active_view, "detail")

        state.stage_compare(10, page_size=2)
        self.assertEqual(state.active_view, "compare")
        self.assertEqual(state.focused_pane, "compare")
        self.assertEqual(state.selected_index, 1)

        build_compare_panel(state, width=60, page_size=20)
        first_call = mock_compare_runs.call_args_list[0]
        self.assertEqual(
            first_call.kwargs,
            {
                "sqlite_path": "dummy.sqlite3",
                "left_run_id": 10,
                "right_run_id": 20,
                "dominant_field": None,
                "limit_scored_events": None,
                "group_dominant_field": None,
                "limit_event_groups": None,
                "only_matching_interventions": False,
            },
        )

        state.step_compare_target(1, page_size=2)
        self.assertEqual(state.selected_index, 2)
        self.assertEqual(state.staged_compare_left_run_id, 10)
        self.assertIsNone(state.cached_compare_right_run_id)
        self.assertIsNone(state.cached_compare_lens_key)

        state.cycle_dominant_field_lens()
        self.assertEqual(state.active_view, "compare")
        self.assertEqual(state.focused_pane, "compare")
        self.assertEqual(state.staged_compare_left_run_id, 10)
        self.assertEqual(state.selected_index, 2)
        self.assertIsNone(state.cached_compare_right_run_id)
        self.assertIsNone(state.cached_compare_lens_key)

        build_compare_panel(state, width=60, page_size=20)
        second_call = mock_compare_runs.call_args_list[1]
        self.assertEqual(
            second_call.kwargs,
            {
                "sqlite_path": "dummy.sqlite3",
                "left_run_id": 10,
                "right_run_id": 30,
                "dominant_field": "conflict",
                "limit_scored_events": None,
                "group_dominant_field": None,
                "limit_event_groups": None,
                "only_matching_interventions": False,
            },
        )
        self.assertEqual(state.cached_compare_right_run_id, 30)
        self.assertEqual(
            state.cached_compare_lens_key,
            ("conflict", None, None, None, False),
        )
        self.assertEqual(mock_compare_runs.call_count, 2)

    @mock.patch("tianji.tui.get_run_summary")
    def test_build_detail_panel_treats_filtered_empty_projection_as_valid_detail(
        self, mock_get_run_summary
    ) -> None:
        mock_get_run_summary.return_value = {
            "run_id": 10,
            "generated_at": "2026-03-22T10:00:00+00:00",
            "mode": "fixture",
            "input_summary": {"raw_item_count": 3, "normalized_event_count": 3},
            "scenario_summary": {
                "dominant_field": "technology",
                "risk_level": "high",
                "headline": "Persisted truth remains visible.",
                "event_groups": [],
            },
            "scored_events": [],
            "intervention_candidates": [],
        }
        state = HistoryListState(
            rows=[{"run_id": 10}],
            sqlite_path="dummy.sqlite3",
            dominant_field="uncategorized",
            limit_scored_events=1,
            group_dominant_field="diplomacy",
            limit_event_groups=1,
            only_matching_interventions=True,
        )

        panel = build_detail_panel(state, width=60, page_size=20)

        detail_text = cast(Text, panel.renderable).plain
        self.assertIn("Run #10", detail_text)
        self.assertIn("Items: 3 raw -> 3 normalized", detail_text)
        self.assertIn("Scenario: technology • Risk: high", detail_text)
        self.assertIn("Persisted truth remains visible.", detail_text)
        self.assertIn("Event Groups: 0", detail_text)
        self.assertIn("Scored Events: 0", detail_text)
        self.assertIn("Interventions: 0", detail_text)
        self.assertNotIn("Run details not found.", detail_text)
        mock_get_run_summary.assert_called_once_with(
            sqlite_path="dummy.sqlite3",
            run_id=10,
            dominant_field="uncategorized",
            limit_scored_events=1,
            group_dominant_field="diplomacy",
            limit_event_groups=1,
            only_matching_interventions=True,
        )

    @mock.patch("tianji.tui.get_run_summary", return_value=None)
    def test_build_detail_panel_uses_missing_data_copy_for_missing_summary(
        self, mock_get_run_summary
    ) -> None:
        state = HistoryListState(
            rows=[{"run_id": 10}],
            sqlite_path="dummy.sqlite3",
            dominant_field="technology",
        )

        panel = build_detail_panel(state, width=70, page_size=20)

        detail_text = cast(Text, panel.renderable).plain
        self.assertIn("No persisted detail view is available.", detail_text)
        self.assertNotIn("No detail rows match the active lens.", detail_text)
        mock_get_run_summary.assert_called_once_with(
            sqlite_path="dummy.sqlite3",
            run_id=10,
            dominant_field="technology",
            limit_scored_events=None,
            group_dominant_field=None,
            limit_event_groups=None,
            only_matching_interventions=False,
        )

    @mock.patch(
        "tianji.tui.get_run_summary",
        return_value={
            "run_id": 10,
            "generated_at": "2026-03-22T10:00:00+00:00",
            "mode": "fixture",
            "input_summary": {"raw_item_count": 3, "normalized_event_count": 3},
            "scenario_summary": {
                "dominant_field": "technology",
                "risk_level": "high",
                "headline": "Persisted truth remains visible.",
                "event_groups": [],
            },
            "scored_events": [],
            "intervention_candidates": [],
        },
    )
    def test_build_detail_panel_shows_lens_empty_copy_for_filtered_empty_summary(
        self, mock_get_run_summary
    ) -> None:
        state = HistoryListState(
            rows=[{"run_id": 10}],
            sqlite_path="dummy.sqlite3",
            dominant_field="economy",
            group_dominant_field="conflict",
            only_matching_interventions=True,
        )

        panel = build_detail_panel(state, width=70, page_size=20)

        detail_text = cast(Text, panel.renderable).plain
        self.assertIn("Run #10", detail_text)
        self.assertIn("Persisted truth remains visible.", detail_text)
        self.assertIn("No event-group rows match the active lens.", detail_text)
        self.assertIn("No scored-event rows match the active lens.", detail_text)
        self.assertIn("No intervention rows match the active lens.", detail_text)
        self.assertIn("Persisted run data is", detail_text)
        self.assertIn("unchanged.", detail_text)
        self.assertIn("Event Groups: 0", detail_text)
        self.assertIn("Scored Events: 0", detail_text)
        self.assertIn("Interventions: 0", detail_text)
        self.assertNotIn("No persisted detail view is available.", detail_text)
        mock_get_run_summary.assert_called_once_with(
            sqlite_path="dummy.sqlite3",
            run_id=10,
            dominant_field="economy",
            limit_scored_events=None,
            group_dominant_field="conflict",
            limit_event_groups=None,
            only_matching_interventions=True,
        )

    @mock.patch(
        "tianji.tui.get_run_summary",
        return_value={
            "run_id": 10,
            "generated_at": "2026-03-22T10:00:00+00:00",
            "mode": "fixture",
            "input_summary": {"raw_item_count": 3, "normalized_event_count": 3},
            "scenario_summary": {
                "dominant_field": "technology",
                "risk_level": "high",
                "headline": "Persisted truth remains visible.",
                "event_groups": [
                    {
                        "dominant_field": "technology",
                        "member_count": 2,
                        "headline_title": "Grouped signal stays visible.",
                    }
                ],
            },
            "scored_events": [],
            "intervention_candidates": [
                {
                    "target": "usa",
                    "intervention_type": "monitor",
                    "expected_effect": "Remain visible.",
                }
            ],
        },
    )
    def test_build_detail_panel_explains_empty_scored_events_slice(
        self, mock_get_run_summary
    ) -> None:
        state = HistoryListState(
            rows=[{"run_id": 10}],
            sqlite_path="dummy.sqlite3",
            dominant_field="economy",
        )

        panel = build_detail_panel(state, width=72, page_size=20)

        detail_text = cast(Text, panel.renderable).plain
        self.assertIn("Scored Events: 0", detail_text)
        self.assertIn("No scored-event rows match the active lens.", detail_text)
        self.assertIn("Event Groups: 1", detail_text)
        self.assertIn("Grouped signal stays visible.", detail_text)
        self.assertIn("Interventions: 1", detail_text)
        self.assertNotIn("No event-group rows match the active lens.", detail_text)
        self.assertNotIn("No intervention rows match the active lens.", detail_text)
        mock_get_run_summary.assert_called_once_with(
            sqlite_path="dummy.sqlite3",
            run_id=10,
            dominant_field="economy",
            limit_scored_events=None,
            group_dominant_field=None,
            limit_event_groups=None,
            only_matching_interventions=False,
        )

    @mock.patch(
        "tianji.tui.get_run_summary",
        return_value={
            "run_id": 10,
            "generated_at": "2026-03-22T10:00:00+00:00",
            "mode": "fixture",
            "input_summary": {"raw_item_count": 3, "normalized_event_count": 3},
            "scenario_summary": {
                "dominant_field": "technology",
                "risk_level": "high",
                "headline": "Persisted truth remains visible.",
                "event_groups": [],
            },
            "scored_events": [
                {
                    "title": "Scored event stays visible.",
                    "dominant_field": "technology",
                    "impact_score": 14.0,
                    "field_attraction": 7.0,
                    "divergence_score": 19.0,
                }
            ],
            "intervention_candidates": [
                {
                    "target": "usa",
                    "intervention_type": "monitor",
                    "expected_effect": "Remain visible.",
                }
            ],
        },
    )
    def test_build_detail_panel_explains_empty_event_groups_slice(
        self, mock_get_run_summary
    ) -> None:
        state = HistoryListState(
            rows=[{"run_id": 10}],
            sqlite_path="dummy.sqlite3",
            group_dominant_field="economy",
        )

        panel = build_detail_panel(state, width=72, page_size=20)

        detail_text = cast(Text, panel.renderable).plain
        self.assertIn("Event Groups: 0", detail_text)
        self.assertIn("No event-group rows match the active lens.", detail_text)
        self.assertIn("Scored Events: 1", detail_text)
        self.assertIn("Scored event stays visible.", detail_text)
        self.assertIn("Interventions: 1", detail_text)
        self.assertNotIn("No scored-event rows match the active lens.", detail_text)
        self.assertNotIn("No intervention rows match the active lens.", detail_text)
        mock_get_run_summary.assert_called_once_with(
            sqlite_path="dummy.sqlite3",
            run_id=10,
            dominant_field=None,
            limit_scored_events=None,
            group_dominant_field="economy",
            limit_event_groups=None,
            only_matching_interventions=False,
        )

    @mock.patch(
        "tianji.tui.get_run_summary",
        return_value={
            "run_id": 10,
            "generated_at": "2026-03-22T10:00:00+00:00",
            "mode": "fixture",
            "input_summary": {"raw_item_count": 3, "normalized_event_count": 3},
            "scenario_summary": {
                "dominant_field": "technology",
                "risk_level": "high",
                "headline": "Persisted truth remains visible.",
                "event_groups": [
                    {
                        "dominant_field": "technology",
                        "member_count": 2,
                        "headline_title": "Grouped signal stays visible.",
                    }
                ],
            },
            "scored_events": [
                {
                    "title": "Scored event stays visible.",
                    "dominant_field": "technology",
                    "impact_score": 14.0,
                    "field_attraction": 7.0,
                    "divergence_score": 19.0,
                }
            ],
            "intervention_candidates": [],
        },
    )
    def test_build_detail_panel_explains_empty_interventions_slice(
        self, mock_get_run_summary
    ) -> None:
        state = HistoryListState(
            rows=[{"run_id": 10}],
            sqlite_path="dummy.sqlite3",
            only_matching_interventions=True,
        )

        panel = build_detail_panel(state, width=72, page_size=20)

        detail_text = cast(Text, panel.renderable).plain
        self.assertIn("Interventions: 0", detail_text)
        self.assertIn("No intervention rows match the active lens.", detail_text)
        self.assertIn("Event Groups: 1", detail_text)
        self.assertIn("Scored Events: 1", detail_text)
        self.assertIn("Scored event stays visible.", detail_text)
        self.assertNotIn("No scored-event rows match the active lens.", detail_text)
        self.assertNotIn("No event-group rows match the active lens.", detail_text)
        mock_get_run_summary.assert_called_once_with(
            sqlite_path="dummy.sqlite3",
            run_id=10,
            dominant_field=None,
            limit_scored_events=None,
            group_dominant_field=None,
            limit_event_groups=None,
            only_matching_interventions=True,
        )

    @mock.patch("tianji.tui.compare_runs")
    def test_build_compare_panel_treats_filtered_empty_projection_as_valid_compare(
        self, mock_compare_runs
    ) -> None:
        mock_compare_runs.return_value = {
            "left": {
                "run_id": 10,
                "mode": "fixture",
                "dominant_field": "technology",
                "risk_level": "high",
                "headline": "Left persisted truth.",
                "top_event_group": None,
                "top_scored_event": None,
                "top_intervention": None,
                "event_group_count": 0,
                "intervention_event_ids": [],
            },
            "right": {
                "run_id": 20,
                "mode": "fixture",
                "dominant_field": "technology",
                "risk_level": "high",
                "headline": "Right persisted truth.",
                "top_event_group": None,
                "top_scored_event": None,
                "top_intervention": None,
                "event_group_count": 0,
                "intervention_event_ids": [],
            },
            "diff": {
                "dominant_field_changed": False,
                "risk_level_changed": False,
                "raw_item_count_delta": 0,
                "normalized_event_count_delta": 0,
                "event_group_count_delta": 0,
                "top_event_group_changed": False,
                "top_event_group_evidence_diff": {
                    "comparable": False,
                    "member_count_delta": 0,
                    "evidence_chain_link_count_delta": 0,
                    "right_only_member_event_ids": [],
                    "left_only_member_event_ids": [],
                    "shared_keywords_added": [],
                    "shared_keywords_removed": [],
                    "chain_summary_changed": False,
                },
                "top_scored_event_changed": False,
                "top_scored_event_comparable": False,
                "top_impact_score_delta": None,
                "top_field_attraction_delta": None,
                "top_divergence_score_delta": None,
                "top_intervention_changed": False,
                "left_top_scored_event_id": None,
                "right_top_scored_event_id": None,
            },
        }
        state = HistoryListState(
            rows=[{"run_id": 10}, {"run_id": 20}],
            sqlite_path="dummy.sqlite3",
            selected_index=1,
            staged_compare_left_run_id=10,
            dominant_field="uncategorized",
            group_dominant_field="uncategorized",
            only_matching_interventions=True,
        )

        panel = build_compare_panel(state, width=70, page_size=40)

        compare_text = cast(Text, panel.renderable).plain
        self.assertIn("Compare: Run #10 (Left) vs Run #20 (Right)", compare_text)
        self.assertIn(
            "Summary: Effectively identical: no meaningful differences found.",
            compare_text,
        )
        self.assertIn("[Left] fixture • technology • Risk: high", compare_text)
        self.assertIn("Left persisted truth.", compare_text)
        self.assertIn("[Right] fixture • technology • Risk: high", compare_text)
        self.assertIn("Right persisted truth.", compare_text)
        self.assertIn("Diff Highlights:", compare_text)
        self.assertNotIn("Compare details not found.", compare_text)
        mock_compare_runs.assert_called_once_with(
            sqlite_path="dummy.sqlite3",
            left_run_id=10,
            right_run_id=20,
            dominant_field="uncategorized",
            limit_scored_events=None,
            group_dominant_field="uncategorized",
            limit_event_groups=None,
            only_matching_interventions=True,
        )

    @mock.patch("tianji.tui.compare_runs", return_value=None)
    def test_build_compare_panel_uses_missing_data_copy_for_missing_compare(
        self, mock_compare_runs
    ) -> None:
        state = HistoryListState(
            rows=[{"run_id": 10}, {"run_id": 20}],
            sqlite_path="dummy.sqlite3",
            selected_index=1,
            staged_compare_left_run_id=10,
            dominant_field="technology",
        )

        panel = build_compare_panel(state, width=70, page_size=40)

        compare_text = cast(Text, panel.renderable).plain
        self.assertIn("No persisted compare view is available.", compare_text)
        self.assertNotIn("No compare rows match the active lens.", compare_text)
        mock_compare_runs.assert_called_once_with(
            sqlite_path="dummy.sqlite3",
            left_run_id=10,
            right_run_id=20,
            dominant_field="technology",
            limit_scored_events=None,
            group_dominant_field=None,
            limit_event_groups=None,
            only_matching_interventions=False,
        )

    @mock.patch(
        "tianji.tui.compare_runs",
        return_value={
            "left": {
                "run_id": 10,
                "mode": "fixture",
                "dominant_field": "technology",
                "risk_level": "high",
                "headline": "Left persisted truth.",
                "top_event_group": None,
                "top_scored_event": None,
                "top_intervention": None,
                "event_group_count": 0,
                "intervention_event_ids": [],
            },
            "right": {
                "run_id": 20,
                "mode": "fixture",
                "dominant_field": "technology",
                "risk_level": "high",
                "headline": "Right persisted truth.",
                "top_event_group": None,
                "top_scored_event": None,
                "top_intervention": None,
                "event_group_count": 0,
                "intervention_event_ids": [],
            },
            "diff": {
                "dominant_field_changed": False,
                "risk_level_changed": False,
                "raw_item_count_delta": 0,
                "normalized_event_count_delta": 0,
                "event_group_count_delta": 0,
                "top_event_group_changed": False,
                "top_event_group_evidence_diff": {
                    "comparable": False,
                    "member_count_delta": 0,
                    "evidence_chain_link_count_delta": 0,
                    "right_only_member_event_ids": [],
                    "left_only_member_event_ids": [],
                    "shared_keywords_added": [],
                    "shared_keywords_removed": [],
                    "chain_summary_changed": False,
                },
                "top_scored_event_changed": False,
                "top_scored_event_comparable": False,
                "top_impact_score_delta": None,
                "top_field_attraction_delta": None,
                "top_divergence_score_delta": None,
                "top_intervention_changed": False,
                "left_top_scored_event_id": None,
                "right_top_scored_event_id": None,
            },
        },
    )
    def test_build_compare_panel_shows_lens_empty_copy_for_filtered_empty_compare(
        self, mock_compare_runs
    ) -> None:
        state = HistoryListState(
            rows=[{"run_id": 10}, {"run_id": 20}],
            sqlite_path="dummy.sqlite3",
            selected_index=1,
            staged_compare_left_run_id=10,
            dominant_field="economy",
            group_dominant_field="conflict",
            only_matching_interventions=True,
        )

        panel = build_compare_panel(state, width=70, page_size=40)

        compare_text = cast(Text, panel.renderable).plain
        self.assertIn("Compare: Run #10 (Left) vs Run #20 (Right)", compare_text)
        self.assertIn("Left persisted truth.", compare_text)
        self.assertIn("Right persisted truth.", compare_text)
        self.assertIn("No event-group rows match the active lens.", compare_text)
        self.assertIn("No scored-event rows match the active lens.", compare_text)
        self.assertIn("No intervention rows match the active lens.", compare_text)
        self.assertIn("Persisted run data is", compare_text)
        self.assertIn("unchanged.", compare_text)
        self.assertIn("Diff Highlights:", compare_text)
        self.assertNotIn("No persisted compare view is available.", compare_text)
        mock_compare_runs.assert_called_once_with(
            sqlite_path="dummy.sqlite3",
            left_run_id=10,
            right_run_id=20,
            dominant_field="economy",
            limit_scored_events=None,
            group_dominant_field="conflict",
            limit_event_groups=None,
            only_matching_interventions=True,
        )

    @mock.patch(
        "tianji.tui.get_run_summary",
        return_value={
            "run_id": 10,
            "generated_at": "2026-03-22T10:00:00+00:00",
            "mode": "fixture",
            "input_summary": {"raw_item_count": 1, "normalized_event_count": 1},
            "scenario_summary": {
                "dominant_field": "technology",
                "risk_level": "high",
                "headline": "Detail headline.",
                "event_groups": [],
            },
            "scored_events": [],
            "intervention_candidates": [],
        },
    )
    @mock.patch(
        "tianji.tui.compare_runs",
        return_value={
            "left": {
                "run_id": 10,
                "mode": "fixture",
                "dominant_field": "technology",
                "risk_level": "high",
                "headline": "Left headline",
                "top_event_group": None,
                "top_scored_event": None,
                "top_intervention": None,
            },
            "right": {
                "run_id": 20,
                "mode": "fixture",
                "dominant_field": "technology",
                "risk_level": "high",
                "headline": "Right headline",
                "top_event_group": None,
                "top_scored_event": None,
                "top_intervention": None,
            },
            "diff": {
                "dominant_field_changed": False,
                "risk_level_changed": False,
                "raw_item_count_delta": 0,
                "normalized_event_count_delta": 0,
                "event_group_count_delta": 0,
                "top_event_group_changed": False,
                "top_scored_event_changed": False,
                "top_intervention_changed": False,
            },
        },
    )
    def test_run_history_list_browser_preserves_non_lens_navigation_behavior(
        self, mock_compare_runs, mock_get_run_summary
    ) -> None:
        state = HistoryListState(
            rows=[{"run_id": 10}, {"run_id": 20}, {"run_id": 30}],
            sqlite_path="dummy.sqlite3",
        )
        key_sequence = iter(
            [
                "?",
                "?",
                "l",
                "z",
                "z",
                "c",
                "c",
                "l",
                "]",
                "[",
                "h",
                "j",
                "g",
                "G",
                "q",
            ]
        )

        class FakeLive:
            def __init__(self, *args: object, **kwargs: object) -> None:
                self.updates: list[object] = []

            def __enter__(self) -> "FakeLive":
                return self

            def __exit__(self, exc_type: object, exc: object, tb: object) -> bool:
                return False

            def update(self, layout: object, refresh: bool = False) -> None:
                self.updates.append((layout, refresh))

        fake_console = mock.Mock()
        fake_console.size.height = 12
        fake_console.size.width = 100

        with mock.patch("tianji.tui.Console", return_value=fake_console):
            with mock.patch("tianji.tui.Live", FakeLive):
                with mock.patch(
                    "tianji.tui.getch", side_effect=lambda: next(key_sequence)
                ):
                    run_history_list_browser(state)

        self.assertEqual(state.selected_index, 2)
        self.assertEqual(state.scroll_offset, 0)
        self.assertEqual(state.focused_pane, "list")
        self.assertEqual(state.active_view, "compare")
        self.assertFalse(state.show_help)
        self.assertFalse(state.zoomed)
        self.assertEqual(state.staged_compare_left_run_id, 10)
        self.assertIsNone(state.message)
        self.assertGreaterEqual(mock_get_run_summary.call_count, 1)
        self.assertGreaterEqual(mock_compare_runs.call_count, 1)

    @mock.patch(
        "tianji.tui.get_run_summary",
        side_effect=[
            {
                "run_id": 10,
                "generated_at": "2026-03-22T10:00:00+00:00",
                "mode": "fixture",
                "input_summary": {"raw_item_count": 3, "normalized_event_count": 3},
                "scenario_summary": {
                    "dominant_field": "technology",
                    "risk_level": "high",
                    "headline": "Run ten detail headline.",
                    "event_groups": [
                        {
                            "dominant_field": "technology",
                            "member_count": 2,
                            "headline_title": "Technology group remains visible.",
                        }
                    ],
                },
                "scored_events": [
                    {
                        "title": "Technology event remains visible.",
                        "dominant_field": "technology",
                        "impact_score": 14.0,
                        "field_attraction": 7.0,
                        "divergence_score": 19.0,
                    }
                ],
                "intervention_candidates": [
                    {
                        "target": "tech-sector",
                        "intervention_type": "monitor",
                        "expected_effect": "Remain visible.",
                    }
                ],
            },
            {
                "run_id": 20,
                "generated_at": "2026-03-22T11:00:00+00:00",
                "mode": "fixture",
                "input_summary": {"raw_item_count": 4, "normalized_event_count": 4},
                "scenario_summary": {
                    "dominant_field": "diplomacy",
                    "risk_level": "medium",
                    "headline": "Projected empty detail still shows persisted truth.",
                    "event_groups": [],
                },
                "scored_events": [],
                "intervention_candidates": [],
            },
        ],
    )
    @mock.patch("tianji.tui.compare_runs")
    @mock.patch("tianji.tui.get_next_run_id", side_effect=[20, 30])
    @mock.patch(
        "tianji.tui.get_previous_run_id",
        side_effect=sqlite3.OperationalError("fallback to loaded rows"),
    )
    def test_run_history_browser_session_covers_detail_compare_and_lens_empty_flows(
        self,
        mock_get_previous_run_id,
        mock_get_next_run_id,
        mock_compare_runs,
        mock_get_run_summary,
    ) -> None:
        compare_run_20 = {
            "left": {
                "run_id": 10,
                "mode": "fixture",
                "dominant_field": "technology",
                "risk_level": "high",
                "headline": "Left persisted truth.",
                "top_event_group": {"dominant_field": "technology", "member_count": 2},
                "top_scored_event": {
                    "dominant_field": "technology",
                    "divergence_score": 19.0,
                    "impact_score": 14.0,
                },
                "top_intervention": {
                    "target": "tech-sector",
                    "intervention_type": "monitor",
                },
                "event_group_count": 1,
                "intervention_event_ids": ["evt-10"],
            },
            "right": {
                "run_id": 20,
                "mode": "fixture",
                "dominant_field": "diplomacy",
                "risk_level": "medium",
                "headline": "Right projected-empty compare target.",
                "top_event_group": None,
                "top_scored_event": None,
                "top_intervention": None,
                "event_group_count": 0,
                "intervention_event_ids": [],
            },
            "diff": {
                "dominant_field_changed": True,
                "risk_level_changed": True,
                "raw_item_count_delta": 1,
                "normalized_event_count_delta": 1,
                "event_group_count_delta": -1,
                "top_event_group_changed": True,
                "top_event_group_evidence_diff": {
                    "comparable": False,
                    "member_count_delta": -2,
                    "evidence_chain_link_count_delta": 0,
                    "right_only_member_event_ids": [],
                    "left_only_member_event_ids": ["evt-10"],
                    "shared_keywords_added": [],
                    "shared_keywords_removed": [],
                    "chain_summary_changed": False,
                },
                "top_scored_event_changed": True,
                "top_scored_event_comparable": False,
                "top_divergence_score_delta": None,
                "top_impact_score_delta": None,
                "top_field_attraction_delta": None,
                "top_intervention_changed": True,
            },
        }
        compare_run_30 = {
            "left": compare_run_20["left"],
            "right": {
                "run_id": 30,
                "mode": "fixture",
                "dominant_field": "economy",
                "risk_level": "low",
                "headline": "Right compare target after persisted skip.",
                "top_event_group": {"dominant_field": "economy", "member_count": 1},
                "top_scored_event": {
                    "dominant_field": "economy",
                    "divergence_score": 11.0,
                    "impact_score": 9.0,
                },
                "top_intervention": {
                    "target": "market",
                    "intervention_type": "observe",
                },
                "event_group_count": 1,
                "intervention_event_ids": ["evt-30"],
            },
            "diff": {
                "dominant_field_changed": True,
                "risk_level_changed": True,
                "raw_item_count_delta": -1,
                "normalized_event_count_delta": -1,
                "event_group_count_delta": 0,
                "top_event_group_changed": True,
                "top_event_group_evidence_diff": {
                    "comparable": False,
                    "member_count_delta": -1,
                    "evidence_chain_link_count_delta": 0,
                    "right_only_member_event_ids": ["evt-30"],
                    "left_only_member_event_ids": ["evt-10"],
                    "shared_keywords_added": [],
                    "shared_keywords_removed": [],
                    "chain_summary_changed": False,
                },
                "top_scored_event_changed": True,
                "top_scored_event_comparable": False,
                "top_divergence_score_delta": None,
                "top_impact_score_delta": None,
                "top_field_attraction_delta": None,
                "top_intervention_changed": True,
            },
        }

        def compare_side_effect(*args: object, **kwargs: object) -> dict[str, object]:
            right_run_id = kwargs.get("right_run_id")
            if right_run_id == 20:
                return cast(dict[str, object], compare_run_20)
            if right_run_id == 30:
                return cast(dict[str, object], compare_run_30)
            raise AssertionError(f"unexpected compare target: {right_run_id}")

        mock_compare_runs.side_effect = compare_side_effect

        state = HistoryListState(
            rows=[
                {
                    "run_id": 10,
                    "generated_at": "2026-03-22T10:00",
                    "mode": "fixture",
                    "dominant_field": "technology",
                    "risk_level": "high",
                    "top_divergence_score": 19.0,
                    "headline": "Run ten headline.",
                },
                {
                    "run_id": 20,
                    "generated_at": "2026-03-22T11:00",
                    "mode": "fixture",
                    "dominant_field": "diplomacy",
                    "risk_level": "medium",
                    "top_divergence_score": 13.0,
                    "headline": "Run twenty headline.",
                },
                {
                    "run_id": 30,
                    "generated_at": "2026-03-22T12:00",
                    "mode": "fixture",
                    "dominant_field": "economy",
                    "risk_level": "low",
                    "top_divergence_score": 11.0,
                    "headline": "Run thirty headline.",
                },
            ],
            sqlite_path="dummy.sqlite3",
        )

        frames = self._run_browser_session(
            state,
            ["l", "c", "j", "c", "l", "a", "]", "[", "h", "j", "l", "q"],
            height=40,
        )

        self.assertEqual(state.selected_index, 2)
        self.assertEqual(state.focused_pane, "compare")
        self.assertEqual(state.active_view, "compare")
        self.assertEqual(state.staged_compare_left_run_id, 10)
        self.assertEqual(state.dominant_field, "conflict")
        self.assertIsNone(state.message)
        self.assertEqual(mock_get_run_summary.call_count, 1)
        self.assertEqual(
            mock_get_run_summary.call_args.kwargs,
            {
                "sqlite_path": "dummy.sqlite3",
                "run_id": 10,
                "dominant_field": None,
                "limit_scored_events": None,
                "group_dominant_field": None,
                "limit_event_groups": None,
                "only_matching_interventions": False,
            },
        )
        self.assertGreaterEqual(mock_compare_runs.call_count, 2)
        self.assertEqual(len(mock_get_next_run_id.call_args_list), 1)
        self.assertEqual(mock_get_previous_run_id.call_count, 1)

        detail_frame = next(
            frame
            for frame in frames
            if "right" in frame and "Run ten detail headline." in frame["right"]
        )
        self.assertIn("Technology event remains visible.", detail_frame["right"])
        self.assertIn("lens:all-runs", detail_frame["header"])

        compare_messages = [frame["message"] for frame in frames if "message" in frame]
        self.assertIn(" compare view active ", compare_messages)

        compare_cache_text = "\n".join(state.cached_compare_lines or [])
        self.assertIn("Compare: Run #10 (Left) vs Run #30", compare_cache_text)
        self.assertIn("Right compare target after persisted", compare_cache_text)

        compare_skip_frame = next(
            frame
            for frame in reversed(frames)
            if frame.get("footer") and "COMPARE L:10 R:30" in frame["footer"]
        )
        self.assertIn("lens:ev=conflict", compare_skip_frame["header"])
        self.assertIn("* 10", compare_skip_frame["list"])

        self.assertEqual(
            mock_get_next_run_id.call_args_list[0].kwargs,
            {"sqlite_path": "dummy.sqlite3", "run_id": 20},
        )

    @mock.patch(
        "tianji.tui.get_run_summary",
        return_value={
            "run_id": 20,
            "generated_at": "2026-03-22T11:00:00+00:00",
            "mode": "fixture",
            "input_summary": {"raw_item_count": 4, "normalized_event_count": 4},
            "scenario_summary": {
                "dominant_field": "diplomacy",
                "risk_level": "medium",
                "headline": "Projected empty detail still shows persisted truth.",
                "event_groups": [],
            },
            "scored_events": [],
            "intervention_candidates": [],
        },
    )
    def test_run_history_browser_session_shows_projected_empty_detail_copy(
        self, mock_get_run_summary
    ) -> None:
        state = HistoryListState(
            rows=[
                {
                    "run_id": 10,
                    "generated_at": "2026-03-22T10:00",
                    "mode": "fixture",
                    "dominant_field": "technology",
                    "risk_level": "high",
                    "top_divergence_score": 19.0,
                    "headline": "Run ten headline.",
                },
                {
                    "run_id": 20,
                    "generated_at": "2026-03-22T11:00",
                    "mode": "fixture",
                    "dominant_field": "diplomacy",
                    "risk_level": "medium",
                    "top_divergence_score": 13.0,
                    "headline": "Run twenty headline.",
                },
            ],
            sqlite_path="dummy.sqlite3",
        )

        frames = self._run_browser_session(
            state,
            ["j", "l", "a", "q"],
            height=60,
        )

        self.assertEqual(state.selected_index, 1)
        self.assertEqual(state.focused_pane, "detail")
        self.assertEqual(state.active_view, "detail")
        self.assertEqual(state.dominant_field, "conflict")
        self.assertEqual(mock_get_run_summary.call_count, 3)
        self.assertEqual(
            mock_get_run_summary.call_args_list[-1].kwargs,
            {
                "sqlite_path": "dummy.sqlite3",
                "run_id": 20,
                "dominant_field": "conflict",
                "limit_scored_events": None,
                "group_dominant_field": None,
                "limit_event_groups": None,
                "only_matching_interventions": False,
            },
        )

        detail_cache_text = "\n".join(state.cached_detail_lines or [])
        self.assertIn(
            "No scored-event rows match the active",
            detail_cache_text,
        )
        self.assertIn("Interventions: 0", detail_cache_text)
        self.assertIn(
            "Projected empty detail still shows",
            detail_cache_text,
        )
        self.assertIn("lens:ev=conflict", frames[-1]["header"])

    @mock.patch(
        "tianji.tui.get_run_summary",
        return_value={
            "run_id": 10,
            "generated_at": "2026-03-22T10:00:00+00:00",
            "mode": "fixture",
            "input_summary": {"raw_item_count": 3, "normalized_event_count": 3},
            "scenario_summary": {
                "dominant_field": "technology",
                "risk_level": "high",
                "headline": "Left detail before compare.",
                "event_groups": [],
            },
            "scored_events": [],
            "intervention_candidates": [],
        },
    )
    @mock.patch("tianji.tui.compare_runs")
    def test_run_history_browser_session_shows_projected_empty_compare_copy(
        self, mock_compare_runs, mock_get_run_summary
    ) -> None:
        mock_compare_runs.return_value = {
            "left": {
                "run_id": 10,
                "mode": "fixture",
                "dominant_field": "technology",
                "risk_level": "high",
                "headline": "Left persisted truth.",
                "top_event_group": {"dominant_field": "technology", "member_count": 2},
                "top_scored_event": {
                    "dominant_field": "technology",
                    "divergence_score": 19.0,
                    "impact_score": 14.0,
                },
                "top_intervention": {
                    "target": "tech-sector",
                    "intervention_type": "monitor",
                },
                "event_group_count": 1,
                "intervention_event_ids": ["evt-10"],
            },
            "right": {
                "run_id": 20,
                "mode": "fixture",
                "dominant_field": "diplomacy",
                "risk_level": "medium",
                "headline": "Right projected-empty compare target.",
                "top_event_group": None,
                "top_scored_event": None,
                "top_intervention": None,
                "event_group_count": 0,
                "intervention_event_ids": [],
            },
            "diff": {
                "dominant_field_changed": True,
                "risk_level_changed": True,
                "raw_item_count_delta": 1,
                "normalized_event_count_delta": 1,
                "event_group_count_delta": -1,
                "top_event_group_changed": True,
                "top_event_group_evidence_diff": {
                    "comparable": False,
                    "member_count_delta": -2,
                    "evidence_chain_link_count_delta": 0,
                    "right_only_member_event_ids": [],
                    "left_only_member_event_ids": ["evt-10"],
                    "shared_keywords_added": [],
                    "shared_keywords_removed": [],
                    "chain_summary_changed": False,
                },
                "top_scored_event_changed": True,
                "top_scored_event_comparable": False,
                "top_divergence_score_delta": None,
                "top_impact_score_delta": None,
                "top_field_attraction_delta": None,
                "top_intervention_changed": True,
            },
        }
        state = HistoryListState(
            rows=[
                {
                    "run_id": 10,
                    "generated_at": "2026-03-22T10:00",
                    "mode": "fixture",
                    "dominant_field": "technology",
                    "risk_level": "high",
                    "top_divergence_score": 19.0,
                    "headline": "Run ten headline.",
                },
                {
                    "run_id": 20,
                    "generated_at": "2026-03-22T11:00",
                    "mode": "fixture",
                    "dominant_field": "diplomacy",
                    "risk_level": "medium",
                    "top_divergence_score": 13.0,
                    "headline": "Run twenty headline.",
                },
            ],
            sqlite_path="dummy.sqlite3",
        )

        frames = self._run_browser_session(
            state,
            ["c", "j", "c", "l", "a", "q"],
            height=60,
        )

        compare_cache_text = "\n".join(state.cached_compare_lines or [])
        self.assertIn(
            "Compare: Run #10 (Left) vs Run #20",
            compare_cache_text,
        )
        self.assertIn("No scored-event rows match the active", compare_cache_text)
        self.assertIn("Persisted run data is unchanged.", compare_cache_text)
        self.assertIn("lens:ev=conflict", frames[-1]["header"])
        self.assertGreaterEqual(mock_get_run_summary.call_count, 1)

    def test_launch_history_tui_prints_empty_state_without_browser(self) -> None:
        stdout = io.StringIO()
        with contextlib.redirect_stdout(stdout):
            with mock.patch("tianji.tui.list_runs", return_value=[]):
                with mock.patch("tianji.tui.run_history_list_browser") as browser_mock:
                    exit_code = launch_history_tui(
                        sqlite_path="runs/tianji.sqlite3",
                        limit=20,
                    )

        self.assertEqual(exit_code, 0)
        self.assertIn(
            "No persisted runs are available for the TUI browser.", stdout.getvalue()
        )
        browser_mock.assert_not_called()

    def test_launch_history_tui_uses_browser_for_available_runs(self) -> None:
        rows = [
            {
                "run_id": 1,
                "generated_at": "2026-03-22T10:00:00+00:00",
                "mode": "fixture",
                "dominant_field": "technology",
                "risk_level": "high",
                "event_group_count": 1,
                "top_divergence_score": 18.2,
                "headline": "The strongest current branch is technology.",
            }
        ]

        with mock.patch("tianji.tui.list_runs", return_value=rows):
            with mock.patch("tianji.tui.run_history_list_browser") as browser_mock:
                exit_code = launch_history_tui(
                    sqlite_path="runs/tianji.sqlite3",
                    limit=20,
                )

        self.assertEqual(exit_code, 0)
        browser_mock.assert_called_once()
        browser_args = browser_mock.call_args.args
        self.assertEqual(browser_args[0].rows, rows)
