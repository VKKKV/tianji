from support import *
from rich.text import Text
from tianji.tui import (
    build_compare_panel,
    format_top_group_evidence_diff_lines,
    get_compare_similarity_summary,
)


class TuiTests(unittest.TestCase):
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
        state.cached_compare_lines = ["line 1", "line 2", "line 3", "line 4", "line 5"]
        state.detail_scroll_offset = 1

        panel = build_compare_panel(state, width=50, page_size=2)
        self.assertEqual(panel.title, Text(" Compare L:10 R:20 2-3/5 ", style="bold"))

        state.focused_pane = "compare"
        panel = build_compare_panel(state, width=50, page_size=2)
        self.assertEqual(
            panel.title, Text(" [Compare L:10 R:20 2-3/5] ", style="reverse bold")
        )

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
