from support import *
from rich.panel import Panel
from rich.text import Text

from tianji.tui_render import (
    build_compare_projected_empty_messages,
    build_compare_panel,
    build_detail_panel,
    build_layout,
    build_list_panel,
    build_help_text,
    format_active_lens_summary,
    format_compare_detail,
    format_lens_change_message,
    format_status_footer,
    format_top_group_evidence_diff_lines,
)
from tianji.tui_state import HistoryListState


class TuiRenderTests(unittest.TestCase):
    def test_footer_and_lens_summary_show_state(self) -> None:
        state = HistoryListState(
            rows=[{"run_id": 10}, {"run_id": 20}],
            sqlite_path="dummy.sqlite3",
            dominant_field="technology",
            staged_compare_left_run_id=10,
            active_view="compare",
            selected_index=1,
            only_matching_interventions=True,
        )

        self.assertEqual(
            format_active_lens_summary(state),
            "lens:ev=technology,matching-interventions",
        )
        footer = format_status_footer(state, width=140)
        self.assertIn("COMPARE L:10 R:20", footer)
        self.assertIn("VIEW LENS:EV=TECHNOLOGY,MATCHING-INTERVENTIONS", footer)
        self.assertEqual(
            format_lens_change_message("event field lens", "technology"),
            "lens event field lens: technology",
        )

    def test_help_text_lists_all_five_lens_controls(self) -> None:
        help_text = build_help_text().plain
        self.assertIn("a           : Cycle scored-event field lens", help_text)
        self.assertIn("s           : Cycle scored-event limit lens", help_text)
        self.assertIn("d           : Cycle event-group field lens", help_text)
        self.assertIn("f           : Cycle event-group limit lens", help_text)
        self.assertIn("v           : Toggle intervention-match lens", help_text)

    def test_format_compare_detail_includes_core_fields(self) -> None:
        compare_result: dict[str, object] = {
            "left": {
                "run_id": 10,
                "mode": "fixture",
                "dominant_field": "technology",
                "risk_level": "high",
                "headline": "Left headline.",
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
            },
            "right": {
                "run_id": 20,
                "mode": "fixture",
                "dominant_field": "diplomacy",
                "risk_level": "medium",
                "headline": "Right headline.",
                "top_event_group": None,
                "top_scored_event": None,
                "top_intervention": None,
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
                    "left_only_member_event_ids": [],
                    "shared_keywords_added": [],
                    "shared_keywords_removed": [],
                    "chain_summary_changed": False,
                },
                "top_scored_event_changed": True,
                "top_scored_event_comparable": False,
                "top_intervention_changed": True,
            },
        }

        lines = format_compare_detail(compare_result, width=60)
        self.assertIn("Compare: Run #10 (Left) vs Run #20 (Right)", lines[0])
        self.assertTrue(any("Diff Highlights:" in line for line in lines))
        self.assertTrue(
            any("Field changed: technology -> diplomacy" in line for line in lines)
        )

    def test_build_compare_projected_empty_messages_handles_asymmetric_sides(
        self,
    ) -> None:
        compare_result: dict[str, object] = {
            "left": {
                "run_id": 10,
                "mode": "fixture",
                "dominant_field": "technology",
                "risk_level": "high",
                "headline": "Left projected-empty side.",
                "top_event_group": None,
                "top_scored_event": None,
                "top_intervention": None,
                "event_group_count": 0,
                "intervention_event_ids": [],
            },
            "right": {
                "run_id": 20,
                "mode": "fixture",
                "dominant_field": "economy",
                "risk_level": "low",
                "headline": "Right side still has projected data.",
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
                "intervention_event_ids": ["evt-20"],
            },
            "diff": {},
        }
        state = HistoryListState(
            rows=[{"run_id": 10}, {"run_id": 20}],
            sqlite_path="dummy.sqlite3",
            dominant_field="conflict",
            group_dominant_field="conflict",
            only_matching_interventions=True,
        )

        messages = build_compare_projected_empty_messages(compare_result, state=state)
        lines = format_compare_detail(
            compare_result,
            width=80,
            projected_empty_messages=messages,
        )
        rendered = "\n".join(lines)

        self.assertIn("No event-group rows match the active lens.", rendered)
        self.assertIn("No scored-event rows match the active lens.", rendered)
        self.assertIn("No intervention rows match the active lens.", rendered)
        self.assertIn("[Right] fixture • economy • Risk: low", rendered)
        self.assertEqual(messages["right"], [])

    def test_format_top_group_evidence_diff_lines_contrast(self) -> None:
        lines = format_top_group_evidence_diff_lines(
            {
                "comparable": False,
                "member_count_delta": -2,
                "evidence_chain_link_count_delta": 1,
                "right_only_member_event_ids": ["evt-20"],
                "left_only_member_event_ids": ["evt-10"],
                "shared_keywords_added": ["ceasefire"],
                "shared_keywords_removed": ["sanctions"],
                "chain_summary_changed": True,
            },
            width=80,
        )
        self.assertTrue(any("Top group evidence (Contrast)" in line for line in lines))
        self.assertTrue(any("Chain summary changed" in line for line in lines))

    @mock.patch("tianji.tui_state.get_run_summary")
    def test_prepare_detail_cache_passes_active_lens_kwargs(
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
            dominant_field="economy",
            only_matching_interventions=True,
        )

        state.prepare_detail_cache(width=70)
        panel = build_detail_panel(state, width=70, page_size=20)

        self.assertIsInstance(panel.renderable, Text)
        mock_get_run_summary.assert_called_once_with(
            sqlite_path="dummy.sqlite3",
            run_id=10,
            dominant_field="economy",
            limit_scored_events=None,
            group_dominant_field=None,
            limit_event_groups=None,
            only_matching_interventions=True,
        )
        self.assertIn(
            "Persisted truth remains visible.",
            cast(Text, panel.renderable).plain,
        )

    @mock.patch("tianji.tui_state.compare_runs")
    def test_prepare_compare_cache_passes_active_lens_kwargs(
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
            dominant_field="economy",
            group_dominant_field="conflict",
            only_matching_interventions=True,
        )

        state.prepare_compare_cache(width=70)
        panel = build_compare_panel(state, width=70, page_size=40)

        self.assertIsInstance(panel.renderable, Text)
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
        self.assertIn("Left persisted truth.", cast(Text, panel.renderable).plain)

    @mock.patch("tianji.tui_state.compare_runs", return_value=None)
    def test_build_layout_renders_header_and_uses_prepared_compare_panel(
        self, mock_compare_runs
    ) -> None:
        state = HistoryListState(
            rows=[{"run_id": 10}, {"run_id": 20}],
            sqlite_path="dummy.sqlite3",
            selected_index=1,
            staged_compare_left_run_id=10,
            active_view="compare",
            focused_pane="compare",
            dominant_field="technology",
        )

        state.prepare_active_view_cache(width=100)
        layout = build_layout(state, height=20, width=100, page_size=10)
        header = cast(Text, layout["header"].renderable).plain
        right_panel = cast(Panel, layout["right"].renderable)
        self.assertIn("lens:ev=technology", header)
        self.assertEqual(mock_compare_runs.call_count, 1)
        self.assertIn(
            "No persisted compare view is available.",
            cast(Text, right_panel.renderable).plain,
        )

        with mock.patch(
            "tianji.tui_state.compare_runs",
            side_effect=AssertionError(
                "build_layout should consume prepared compare cache without storage reads"
            ),
        ):
            second_layout = build_layout(state, height=20, width=100, page_size=10)

        second_right_panel = cast(Panel, second_layout["right"].renderable)
        self.assertIn(
            "No persisted compare view is available.",
            cast(Text, second_right_panel.renderable).plain,
        )

    def test_build_layout_uses_list_only_body_when_narrow(self) -> None:
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
            staged_compare_left_run_id=10,
            active_view="compare",
            focused_pane="compare",
        )

        layout = build_layout(state, height=20, width=50, page_size=10)

        self.assertFalse(layout["body"].children)
        body_panel = cast(Panel, layout["body"].renderable)
        body_text = cast(Text, body_panel.renderable).plain
        self.assertIn("10", body_text)
        self.assertIn("20", body_text)

    def test_lens_changes_do_not_mutate_list_panel_rows(self) -> None:
        rows = [
            {
                "run_id": 10,
                "generated_at": "2026-03-22T10:00",
                "mode": "fixture",
                "dominant_field": "technology",
                "risk_level": "high",
                "top_divergence_score": 19.0,
                "headline": "Persisted row remains visible.",
            },
            {
                "run_id": 20,
                "generated_at": "2026-03-22T11:00",
                "mode": "fixture",
                "dominant_field": "diplomacy",
                "risk_level": "medium",
                "top_divergence_score": 13.0,
                "headline": "Second persisted row remains visible.",
            },
        ]
        state = HistoryListState(rows=rows, sqlite_path="dummy.sqlite3")

        before = cast(
            Text, build_list_panel(state, width=80, page_size=10).renderable
        ).plain
        state.cycle_dominant_field_lens()
        state.cycle_limit_scored_events_lens()
        state.toggle_only_matching_interventions()
        after = cast(
            Text, build_list_panel(state, width=80, page_size=10).renderable
        ).plain

        self.assertEqual(before, after)
        self.assertEqual(state.rows, rows)
