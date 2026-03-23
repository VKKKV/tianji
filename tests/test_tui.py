from support import *
from rich.text import Text
from tianji.tui import (
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
    run_history_list_browser,
)


class TuiTests(unittest.TestCase):
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
