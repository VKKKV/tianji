from support import *

from tianji.tui_state import (
    KEY_ACTION_ALIASES,
    LENS_DOMINANT_FIELD_VALUES,
    LENS_KEY_BINDINGS,
    LENS_LIMIT_VALUES,
    HistoryListState,
    format_lens_change_message,
    handle_history_browser_key,
    resolve_history_browser_action,
)


class TuiStateTests(unittest.TestCase):
    def test_lens_defaults_and_bindings(self) -> None:
        state = HistoryListState(rows=[{"run_id": 10}], sqlite_path="dummy.sqlite3")

        self.assertEqual(state.active_lens_key(), (None, None, None, None, False))
        self.assertEqual(LENS_DOMINANT_FIELD_VALUES[0], "conflict")
        self.assertEqual(LENS_LIMIT_VALUES, (1, 3, 5))
        self.assertEqual(KEY_ACTION_ALIASES["a"], "cycle_event_field_lens")
        self.assertEqual(LENS_KEY_BINDINGS["v"], "only_matching_interventions")

    def test_lens_helpers_cycle_and_invalidate_projected_panes(self) -> None:
        state = HistoryListState(
            rows=[{"run_id": 10}, {"run_id": 20}],
            sqlite_path="dummy.sqlite3",
            cached_detail_run_id=20,
            cached_detail_lines=["detail"],
            cached_compare_right_run_id=20,
            cached_compare_lens_key=("technology", 1, "diplomacy", 3, True),
            cached_compare_lines=["compare"],
            detail_scroll_offset=3,
        )

        self.assertEqual(state.cycle_dominant_field_lens(), "conflict")
        self.assertEqual(state.cycle_limit_scored_events_lens(), 1)
        self.assertEqual(state.cycle_group_dominant_field_lens(), "conflict")
        self.assertEqual(state.cycle_limit_event_groups_lens(), 1)
        self.assertTrue(state.toggle_only_matching_interventions())
        self.assertIsNone(state.cached_detail_run_id)
        self.assertIsNone(state.cached_compare_right_run_id)
        self.assertEqual(state.detail_scroll_offset, 0)

    def test_resolve_history_browser_action_maps_supported_keys(self) -> None:
        self.assertEqual(resolve_history_browser_action("q"), "quit")
        self.assertEqual(resolve_history_browser_action("?"), "toggle_help")
        self.assertEqual(resolve_history_browser_action("]"), "step_next")
        self.assertEqual(resolve_history_browser_action("\x1b[5~"), "page_up")
        self.assertIsNone(resolve_history_browser_action("x"))

    def test_handle_history_browser_key_updates_focus_help_and_zoom(self) -> None:
        state = HistoryListState(
            rows=[{"run_id": 10}, {"run_id": 20}],
            sqlite_path="dummy.sqlite3",
        )

        decision = handle_history_browser_key(state, key="?", page_size=3)
        self.assertEqual(decision.action, "toggle_help")
        self.assertTrue(state.show_help)

        decision = handle_history_browser_key(state, key="q", page_size=3)
        self.assertEqual(decision.action, "close_help")
        self.assertFalse(state.show_help)

        decision = handle_history_browser_key(state, key="l", page_size=3)
        self.assertEqual(decision.action, "focus_active_view")
        self.assertEqual(state.focused_pane, "detail")

        decision = handle_history_browser_key(state, key="z", page_size=3)
        self.assertEqual(decision.action, "toggle_zoom")
        self.assertTrue(state.zoomed)

    def test_state_lens_message_format_matches_existing_contract(self) -> None:
        self.assertEqual(
            format_lens_change_message("event field lens", "technology"),
            "lens event field lens: technology",
        )

    def test_handle_history_browser_key_sets_lens_messages_without_render_dependency(
        self,
    ) -> None:
        state = HistoryListState(rows=[{"run_id": 10}], sqlite_path="dummy.sqlite3")

        decision = handle_history_browser_key(state, key="a", page_size=3)
        self.assertEqual(decision.action, "cycle_event_field_lens")
        self.assertEqual(state.message, "lens event field lens: conflict")

        decision = handle_history_browser_key(state, key="s", page_size=3)
        self.assertEqual(decision.action, "cycle_scored_event_limit_lens")
        self.assertEqual(state.message, "lens scored-event limit: 1")

        decision = handle_history_browser_key(state, key="v", page_size=3)
        self.assertEqual(decision.action, "toggle_matching_interventions_lens")
        self.assertEqual(state.message, "lens matching interventions: on")

    @mock.patch("tianji.tui_state.get_previous_run_id", return_value=3)
    @mock.patch("tianji.tui_state.get_run_summary")
    def test_step_run_uses_persisted_previous_beyond_loaded_window(
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
            "scored_events": [{"event_id": "evt-3", "divergence_score": 19.5}],
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
        mock_get_previous_run_id.assert_called_once_with(
            sqlite_path="dummy.sqlite3", run_id=4
        )
        mock_get_run_summary.assert_called_once_with(
            sqlite_path="dummy.sqlite3", run_id=3
        )

    @mock.patch("tianji.tui_state.get_next_run_id", return_value=100)
    @mock.patch("tianji.tui_state.get_run_summary")
    def test_step_run_uses_persisted_next_beyond_loaded_window(
        self, mock_get_run_summary, mock_get_next_run_id
    ) -> None:
        mock_get_run_summary.return_value = {
            "run_id": 100,
            "schema_version": "tianji.run.v1",
            "generated_at": "2026-03-22T10:00:00+00:00",
            "mode": "fixture",
            "input_summary": {"raw_item_count": 2, "normalized_event_count": 1},
            "scenario_summary": {
                "dominant_field": "technology",
                "risk_level": "high",
                "headline": "Persisted next run outside the loaded limit.",
                "event_groups": [],
            },
            "scored_events": [{"event_id": "evt-100", "divergence_score": 19.5}],
            "intervention_candidates": [],
        }
        state = HistoryListState(
            rows=[{"run_id": 11}, {"run_id": 10}],
            sqlite_path="dummy.sqlite3",
            selected_index=0,
            focused_pane="detail",
            cached_detail_run_id=11,
            cached_detail_lines=["detail"],
        )

        state.step_run(1, page_size=2)

        self.assertEqual([row["run_id"] for row in state.rows], [100, 11])
        self.assertEqual(state.selected_index, 0)
        self.assertIsNone(state.cached_detail_run_id)
        mock_get_next_run_id.assert_called_once_with(
            sqlite_path="dummy.sqlite3", run_id=11
        )
        mock_get_run_summary.assert_called_once_with(
            sqlite_path="dummy.sqlite3", run_id=100
        )

    @mock.patch("tianji.tui_state.get_next_run_id", side_effect=[20, None])
    def test_step_compare_target_reports_last_persisted_boundary(
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

    @mock.patch(
        "tianji.tui_state.get_previous_run_id",
        side_effect=sqlite3.OperationalError("db unavailable"),
    )
    def test_step_run_uses_loaded_row_fallback_only_on_operational_error(
        self, mock_get_previous_run_id
    ) -> None:
        state = HistoryListState(
            rows=[{"run_id": 30}, {"run_id": 20}, {"run_id": 10}],
            sqlite_path="dummy.sqlite3",
            selected_index=1,
            focused_pane="detail",
            cached_detail_run_id=20,
            cached_detail_lines=["detail"],
        )

        state.step_run(-1, page_size=3)

        self.assertEqual(state.selected_index, 2)
        self.assertEqual(state.current_run_id(), 10)
        self.assertIsNone(state.cached_detail_run_id)
        self.assertIsNone(state.cached_detail_lines)
        self.assertIsNone(state.message)
        mock_get_previous_run_id.assert_called_once_with(
            sqlite_path="dummy.sqlite3", run_id=20
        )

    @mock.patch("tianji.tui_state.get_next_run_id", return_value=None)
    def test_step_run_does_not_fallback_to_loaded_rows_on_true_persisted_boundary(
        self, mock_get_next_run_id
    ) -> None:
        state = HistoryListState(
            rows=[{"run_id": 20}, {"run_id": 10}],
            sqlite_path="dummy.sqlite3",
            selected_index=0,
            focused_pane="detail",
            cached_detail_run_id=20,
            cached_detail_lines=["detail"],
        )

        state.step_run(1, page_size=2)

        self.assertEqual(state.selected_index, 0)
        self.assertEqual(state.current_run_id(), 20)
        self.assertEqual(state.message, "last run")
        self.assertEqual(state.cached_detail_run_id, 20)
        self.assertEqual(state.cached_detail_lines, ["detail"])
        mock_get_next_run_id.assert_called_once_with(
            sqlite_path="dummy.sqlite3", run_id=20
        )

    @mock.patch("tianji.tui_state.get_next_run_id", side_effect=[20, 30])
    def test_step_compare_target_skips_staged_left_when_stepping_next(
        self, mock_get_next_run_id
    ) -> None:
        state = HistoryListState(
            rows=[{"run_id": 10}, {"run_id": 20}, {"run_id": 30}],
            sqlite_path="dummy.sqlite3",
            selected_index=0,
            staged_compare_left_run_id=20,
            active_view="compare",
            focused_pane="compare",
            cached_compare_right_run_id=10,
            cached_compare_lines=["compare"],
        )

        state.step_compare_target(1, page_size=3)

        self.assertEqual(state.selected_index, 2)
        self.assertEqual(state.current_run_id(), 30)
        self.assertIsNone(state.cached_compare_right_run_id)
        self.assertIsNone(state.cached_compare_lines)
        self.assertIsNone(state.message)
        self.assertEqual(
            mock_get_next_run_id.call_args_list,
            [
                mock.call(sqlite_path="dummy.sqlite3", run_id=10),
                mock.call(sqlite_path="dummy.sqlite3", run_id=20),
            ],
        )

    @mock.patch("tianji.tui_state.get_previous_run_id", side_effect=[20, None])
    def test_step_compare_target_reports_first_boundary_after_skipping_staged_left(
        self, mock_get_previous_run_id
    ) -> None:
        state = HistoryListState(
            rows=[{"run_id": 30}, {"run_id": 20}],
            sqlite_path="dummy.sqlite3",
            selected_index=0,
            staged_compare_left_run_id=20,
            active_view="compare",
            focused_pane="compare",
            cached_compare_right_run_id=30,
            cached_compare_lines=["compare"],
        )

        state.step_compare_target(-1, page_size=2)

        self.assertEqual(state.selected_index, 0)
        self.assertEqual(state.current_run_id(), 30)
        self.assertEqual(state.message, "first compare target")
        self.assertEqual(state.cached_compare_right_run_id, 30)
        self.assertEqual(state.cached_compare_lines, ["compare"])
        self.assertEqual(
            mock_get_previous_run_id.call_args_list,
            [
                mock.call(sqlite_path="dummy.sqlite3", run_id=30),
                mock.call(sqlite_path="dummy.sqlite3", run_id=20),
            ],
        )

    @mock.patch("tianji.tui_state.get_next_run_id", side_effect=[20, None])
    def test_step_compare_target_reports_last_boundary_after_skipping_staged_left(
        self, mock_get_next_run_id
    ) -> None:
        state = HistoryListState(
            rows=[{"run_id": 10}, {"run_id": 20}],
            sqlite_path="dummy.sqlite3",
            selected_index=0,
            staged_compare_left_run_id=20,
            active_view="compare",
            focused_pane="compare",
            cached_compare_right_run_id=10,
            cached_compare_lines=["compare"],
        )

        state.step_compare_target(1, page_size=2)

        self.assertEqual(state.selected_index, 0)
        self.assertEqual(state.current_run_id(), 10)
        self.assertEqual(state.message, "last compare target")
        self.assertEqual(state.cached_compare_right_run_id, 10)
        self.assertEqual(state.cached_compare_lines, ["compare"])
        self.assertEqual(
            mock_get_next_run_id.call_args_list,
            [
                mock.call(sqlite_path="dummy.sqlite3", run_id=10),
                mock.call(sqlite_path="dummy.sqlite3", run_id=20),
            ],
        )

    @mock.patch("tianji.tui_state.get_previous_run_id", side_effect=[20, 10])
    def test_step_compare_target_skips_staged_left_when_stepping_previous(
        self, mock_get_previous_run_id
    ) -> None:
        state = HistoryListState(
            rows=[{"run_id": 30}, {"run_id": 20}, {"run_id": 10}],
            sqlite_path="dummy.sqlite3",
            selected_index=0,
            staged_compare_left_run_id=20,
            active_view="compare",
            focused_pane="compare",
            cached_compare_right_run_id=30,
            cached_compare_lines=["compare"],
        )

        state.step_compare_target(-1, page_size=3)

        self.assertEqual(state.selected_index, 2)
        self.assertIsNone(state.cached_compare_right_run_id)
        self.assertIsNone(state.cached_compare_lines)
        self.assertIsNone(state.message)
        self.assertEqual(len(mock_get_previous_run_id.call_args_list), 2)

    @mock.patch("tianji.tui_state.get_previous_run_id", side_effect=[20, None])
    def test_step_compare_target_reports_first_persisted_boundary(
        self, mock_get_previous_run_id
    ) -> None:
        state = HistoryListState(
            rows=[{"run_id": 30}, {"run_id": 20}],
            sqlite_path="dummy.sqlite3",
            selected_index=0,
            staged_compare_left_run_id=20,
            active_view="compare",
            focused_pane="compare",
        )

        state.step_compare_target(-1, page_size=2)

        self.assertEqual(state.selected_index, 0)
        self.assertEqual(state.message, "first compare target")
        self.assertEqual(len(mock_get_previous_run_id.call_args_list), 2)

    def test_stage_compare_and_clear_compare_transitions(self) -> None:
        state = HistoryListState(
            rows=[{"run_id": 1}, {"run_id": 2}], sqlite_path="dummy.sqlite3"
        )

        state.stage_compare(1, page_size=1)
        self.assertEqual(state.staged_compare_left_run_id, 1)
        self.assertEqual(state.message, "left run staged: 1")

        state.stage_compare(1, page_size=1)
        self.assertEqual(state.active_view, "compare")
        self.assertEqual(state.selected_index, 1)

        state.clear_compare()
        self.assertIsNone(state.staged_compare_left_run_id)
        self.assertEqual(state.active_view, "detail")

    def test_stage_compare_and_clear_compare_invalidate_stale_compare_cache(
        self,
    ) -> None:
        state = HistoryListState(
            rows=[{"run_id": 10}, {"run_id": 20}, {"run_id": 30}],
            sqlite_path="dummy.sqlite3",
            staged_compare_left_run_id=10,
            active_view="compare",
            focused_pane="compare",
            cached_compare_right_run_id=20,
            cached_compare_lens_key=(None, None, None, None, False),
            cached_compare_lines=["Compare: Run #10 (Left) vs Run #20 (Right)"],
            detail_scroll_offset=4,
        )

        state.clear_compare()

        self.assertIsNone(state.staged_compare_left_run_id)
        self.assertIsNone(state.cached_compare_right_run_id)
        self.assertIsNone(state.cached_compare_lens_key)
        self.assertIsNone(state.cached_compare_lines)
        self.assertEqual(state.detail_scroll_offset, 0)

        state.stage_compare(30, page_size=3)

        self.assertEqual(state.staged_compare_left_run_id, 30)
        self.assertIsNone(state.cached_compare_right_run_id)
        self.assertIsNone(state.cached_compare_lens_key)
        self.assertIsNone(state.cached_compare_lines)

    def test_transient_messages_cover_list_and_detail_bounds(self) -> None:
        state = HistoryListState(
            rows=[{"run_id": idx} for idx in range(3)], sqlite_path="dummy.sqlite3"
        )

        state.move_selection(-1, page_size=2)
        self.assertEqual(state.message, "first run")

        state.focused_pane = "detail"
        state.cached_detail_lines = ["line 1", "line 2", "line 3"]
        state.detail_scroll_offset = 0
        state.message = None
        state.move_selection(-1, page_size=2)
        self.assertEqual(state.message, "top of detail")
