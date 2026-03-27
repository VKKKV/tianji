from support import *
from rich.layout import Layout
from rich.panel import Panel
from rich.text import Text
from tianji.tui import (
    launch_history_tui,
    run_history_browser_session,
    run_history_list_browser,
)
from tianji.tui_state import HistoryListState


class TuiIntegrationTests(unittest.TestCase):
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

    @mock.patch(
        "tianji.tui_render.get_run_summary",
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
        "tianji.tui_render.compare_runs",
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
        self.assertEqual(state.focused_pane, "list")
        self.assertEqual(state.active_view, "compare")
        self.assertFalse(state.show_help)
        self.assertFalse(state.zoomed)
        self.assertEqual(state.staged_compare_left_run_id, 10)
        self.assertGreaterEqual(mock_get_run_summary.call_count, 1)
        self.assertGreaterEqual(mock_compare_runs.call_count, 1)

    @mock.patch(
        "tianji.tui_render.get_run_summary",
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
    @mock.patch("tianji.tui_render.compare_runs")
    @mock.patch("tianji.tui_state.get_next_run_id", side_effect=[20, 30])
    @mock.patch(
        "tianji.tui_state.get_previous_run_id",
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
        self.assertEqual(mock_get_run_summary.call_count, 1)
        self.assertGreaterEqual(mock_compare_runs.call_count, 2)
        self.assertEqual(len(mock_get_next_run_id.call_args_list), 1)
        self.assertEqual(mock_get_previous_run_id.call_count, 1)
        self.assertIn(
            "Compare: Run #10 (Left) vs Run #30",
            "\n".join(state.cached_compare_lines or []),
        )
        self.assertIn("lens:ev=conflict", frames[-1]["header"])

    @mock.patch(
        "tianji.tui_render.get_run_summary",
        side_effect=[
            {
                "run_id": 3,
                "generated_at": "2026-03-22T09:00:00+00:00",
                "mode": "fixture",
                "input_summary": {"raw_item_count": 2, "normalized_event_count": 1},
                "scenario_summary": {
                    "dominant_field": "technology",
                    "risk_level": "high",
                    "headline": "Persisted previous run outside the loaded limit.",
                    "event_groups": [],
                },
                "scored_events": [
                    {
                        "title": "Persisted previous event.",
                        "dominant_field": "technology",
                        "impact_score": 12.0,
                        "field_attraction": 6.0,
                        "divergence_score": 18.0,
                    }
                ],
                "intervention_candidates": [],
            },
            {
                "run_id": 3,
                "generated_at": "2026-03-22T09:00:00+00:00",
                "mode": "fixture",
                "input_summary": {"raw_item_count": 2, "normalized_event_count": 1},
                "scenario_summary": {
                    "dominant_field": "technology",
                    "risk_level": "high",
                    "headline": "Persisted previous run outside the loaded limit.",
                    "event_groups": [],
                },
                "scored_events": [
                    {
                        "title": "Persisted previous event.",
                        "dominant_field": "technology",
                        "impact_score": 12.0,
                        "field_attraction": 6.0,
                        "divergence_score": 18.0,
                    }
                ],
                "intervention_candidates": [],
            },
        ],
    )
    @mock.patch(
        "tianji.tui_state.get_run_summary",
        return_value={
            "run_id": 3,
            "generated_at": "2026-03-22T09:00:00+00:00",
            "mode": "fixture",
            "input_summary": {"raw_item_count": 2, "normalized_event_count": 1},
            "scenario_summary": {
                "dominant_field": "technology",
                "risk_level": "high",
                "headline": "Persisted previous run outside the loaded limit.",
                "event_groups": [],
            },
            "scored_events": [
                {
                    "title": "Persisted previous event.",
                    "dominant_field": "technology",
                    "impact_score": 12.0,
                    "field_attraction": 6.0,
                    "divergence_score": 18.0,
                }
            ],
            "intervention_candidates": [],
        },
    )
    @mock.patch("tianji.tui_state.get_previous_run_id", side_effect=[3, None])
    def test_run_history_browser_session_uses_persisted_previous_and_reports_true_first_boundary(
        self,
        mock_get_previous_run_id,
        mock_get_state_run_summary,
        mock_get_render_run_summary,
    ) -> None:
        state = HistoryListState(
            rows=[
                {
                    "run_id": 5,
                    "generated_at": "2026-03-22T11:00",
                    "mode": "fixture",
                    "dominant_field": "technology",
                    "risk_level": "high",
                    "top_divergence_score": 20.0,
                    "headline": "Run five headline.",
                },
                {
                    "run_id": 4,
                    "generated_at": "2026-03-22T10:00",
                    "mode": "fixture",
                    "dominant_field": "technology",
                    "risk_level": "medium",
                    "top_divergence_score": 17.0,
                    "headline": "Run four headline.",
                },
            ],
            sqlite_path="dummy.sqlite3",
            selected_index=1,
            focused_pane="detail",
        )

        frames = self._run_browser_session(state, ["[", "[", "q"], height=40)

        self.assertEqual([row["run_id"] for row in state.rows], [4, 3])
        self.assertEqual(state.selected_index, 1)
        self.assertIsNone(state.message)
        self.assertEqual(len(mock_get_previous_run_id.call_args_list), 2)
        self.assertEqual(mock_get_state_run_summary.call_count, 1)
        self.assertEqual(mock_get_render_run_summary.call_count, 2)
        detail_text = "\n".join(state.cached_detail_lines or [])
        self.assertIn("Run #3", detail_text)
        self.assertIn(
            "Persisted previous run outside the loaded limit.",
            " ".join(detail_text.split()),
        )
        self.assertIn("first run", {frame["message"].strip() for frame in frames})

    @mock.patch(
        "tianji.tui_render.get_run_summary",
        side_effect=[
            {
                "run_id": 100,
                "generated_at": "2026-03-22T12:00:00+00:00",
                "mode": "fixture",
                "input_summary": {"raw_item_count": 3, "normalized_event_count": 2},
                "scenario_summary": {
                    "dominant_field": "technology",
                    "risk_level": "high",
                    "headline": "Persisted next run outside the loaded limit.",
                    "event_groups": [],
                },
                "scored_events": [
                    {
                        "title": "Persisted next event.",
                        "dominant_field": "technology",
                        "impact_score": 13.0,
                        "field_attraction": 7.0,
                        "divergence_score": 19.0,
                    }
                ],
                "intervention_candidates": [],
            },
            {
                "run_id": 100,
                "generated_at": "2026-03-22T12:00:00+00:00",
                "mode": "fixture",
                "input_summary": {"raw_item_count": 3, "normalized_event_count": 2},
                "scenario_summary": {
                    "dominant_field": "technology",
                    "risk_level": "high",
                    "headline": "Persisted next run outside the loaded limit.",
                    "event_groups": [],
                },
                "scored_events": [
                    {
                        "title": "Persisted next event.",
                        "dominant_field": "technology",
                        "impact_score": 13.0,
                        "field_attraction": 7.0,
                        "divergence_score": 19.0,
                    }
                ],
                "intervention_candidates": [],
            },
        ],
    )
    @mock.patch(
        "tianji.tui_state.get_run_summary",
        return_value={
            "run_id": 100,
            "generated_at": "2026-03-22T12:00:00+00:00",
            "mode": "fixture",
            "input_summary": {"raw_item_count": 3, "normalized_event_count": 2},
            "scenario_summary": {
                "dominant_field": "technology",
                "risk_level": "high",
                "headline": "Persisted next run outside the loaded limit.",
                "event_groups": [],
            },
            "scored_events": [
                {
                    "title": "Persisted next event.",
                    "dominant_field": "technology",
                    "impact_score": 13.0,
                    "field_attraction": 7.0,
                    "divergence_score": 19.0,
                }
            ],
            "intervention_candidates": [],
        },
    )
    @mock.patch("tianji.tui_state.get_next_run_id", side_effect=[100, None])
    def test_run_history_browser_session_uses_persisted_next_and_reports_true_last_boundary(
        self,
        mock_get_next_run_id,
        mock_get_state_run_summary,
        mock_get_render_run_summary,
    ) -> None:
        state = HistoryListState(
            rows=[
                {
                    "run_id": 11,
                    "generated_at": "2026-03-22T11:00",
                    "mode": "fixture",
                    "dominant_field": "technology",
                    "risk_level": "high",
                    "top_divergence_score": 19.0,
                    "headline": "Run eleven headline.",
                },
                {
                    "run_id": 10,
                    "generated_at": "2026-03-22T10:00",
                    "mode": "fixture",
                    "dominant_field": "technology",
                    "risk_level": "medium",
                    "top_divergence_score": 16.0,
                    "headline": "Run ten headline.",
                },
            ],
            sqlite_path="dummy.sqlite3",
            selected_index=0,
            focused_pane="detail",
        )

        frames = self._run_browser_session(state, ["]", "]", "q"], height=40)

        self.assertEqual([row["run_id"] for row in state.rows], [100, 11])
        self.assertEqual(state.selected_index, 0)
        self.assertIsNone(state.message)
        self.assertEqual(len(mock_get_next_run_id.call_args_list), 2)
        self.assertEqual(mock_get_state_run_summary.call_count, 1)
        self.assertEqual(mock_get_render_run_summary.call_count, 2)
        detail_text = "\n".join(state.cached_detail_lines or [])
        self.assertIn("Run #100", detail_text)
        self.assertIn(
            "Persisted next run outside the loaded limit.",
            " ".join(detail_text.split()),
        )
        self.assertIn("last run", {frame["message"].strip() for frame in frames})

    @mock.patch(
        "tianji.tui_render.get_run_summary",
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

        frames = self._run_browser_session(state, ["j", "l", "a", "q"], height=60)

        self.assertEqual(state.dominant_field, "conflict")
        self.assertEqual(mock_get_run_summary.call_count, 3)
        self.assertIn(
            "No scored-event rows match the active",
            "\n".join(state.cached_detail_lines or []),
        )
        self.assertIn("lens:ev=conflict", frames[-1]["header"])

    @mock.patch(
        "tianji.tui_render.get_run_summary",
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
    @mock.patch("tianji.tui_render.compare_runs")
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
            state, ["c", "j", "c", "l", "a", "q"], height=60
        )

        compare_cache_text = "\n".join(state.cached_compare_lines or [])
        self.assertIn("Compare: Run #10 (Left) vs Run #20", compare_cache_text)
        self.assertIn("No scored-event rows match the active", compare_cache_text)
        self.assertIn("Persisted run data is unchanged.", compare_cache_text)
        self.assertIn("lens:ev=conflict", frames[-1]["header"])
        self.assertGreaterEqual(mock_get_run_summary.call_count, 1)

    @mock.patch("tianji.tui_render.compare_runs")
    @mock.patch("tianji.tui_state.get_next_run_id", side_effect=[20, None])
    def test_run_history_browser_session_reports_compare_boundary_without_selecting_staged_left(
        self, mock_get_next_run_id, mock_compare_runs
    ) -> None:
        mock_compare_runs.return_value = {
            "left": {
                "run_id": 20,
                "mode": "fixture",
                "dominant_field": "diplomacy",
                "risk_level": "medium",
                "headline": "Left staged compare truth.",
                "top_event_group": {"dominant_field": "diplomacy", "member_count": 2},
                "top_scored_event": {
                    "dominant_field": "diplomacy",
                    "divergence_score": 13.0,
                    "impact_score": 10.0,
                },
                "top_intervention": {
                    "target": "treaty-desk",
                    "intervention_type": "monitor",
                },
                "event_group_count": 1,
                "intervention_event_ids": ["evt-20"],
            },
            "right": {
                "run_id": 30,
                "mode": "fixture",
                "dominant_field": "economy",
                "risk_level": "low",
                "headline": "Right compare target remains selected.",
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
                    "left_only_member_event_ids": ["evt-20"],
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
                    "run_id": 30,
                    "generated_at": "2026-03-22T12:00",
                    "mode": "fixture",
                    "dominant_field": "economy",
                    "risk_level": "low",
                    "top_divergence_score": 11.0,
                    "headline": "Run thirty headline.",
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
                    "run_id": 10,
                    "generated_at": "2026-03-22T10:00",
                    "mode": "fixture",
                    "dominant_field": "technology",
                    "risk_level": "high",
                    "top_divergence_score": 19.0,
                    "headline": "Run ten headline.",
                },
            ],
            sqlite_path="dummy.sqlite3",
            selected_index=0,
            staged_compare_left_run_id=20,
            active_view="compare",
            focused_pane="compare",
        )

        frames = self._run_browser_session(state, ["]", "q"], height=40)

        self.assertEqual(state.selected_index, 0)
        self.assertEqual(state.staged_compare_left_run_id, 20)
        self.assertIn(
            "last compare target", {frame["message"].strip() for frame in frames}
        )
        self.assertIn(
            "Compare: Run #20 (Left) vs Run #30",
            "\n".join(state.cached_compare_lines or []),
        )
        self.assertEqual(len(mock_get_next_run_id.call_args_list), 2)

    @mock.patch("tianji.tui_render.compare_runs")
    @mock.patch(
        "tianji.tui_render.get_run_summary",
        side_effect=lambda *args, **kwargs: {
            10: {
                "run_id": 10,
                "generated_at": "2026-03-22T10:00:00+00:00",
                "mode": "fixture",
                "input_summary": {"raw_item_count": 3, "normalized_event_count": 3},
                "scenario_summary": {
                    "dominant_field": "technology",
                    "risk_level": "high",
                    "headline": "Loaded run before persisted navigation.",
                    "event_groups": [],
                },
                "scored_events": [
                    {
                        "title": "Loaded run event.",
                        "dominant_field": "technology",
                        "impact_score": 14.0,
                        "field_attraction": 7.0,
                        "divergence_score": 19.0,
                    }
                ],
                "intervention_candidates": [],
            },
            30: {
                "run_id": 30,
                "generated_at": "2026-03-22T12:00:00+00:00",
                "mode": "fixture",
                "input_summary": {"raw_item_count": 3, "normalized_event_count": 2},
                "scenario_summary": {
                    "dominant_field": "economy",
                    "risk_level": "low",
                    "headline": "Loaded compare target before compare focus.",
                    "event_groups": [],
                },
                "scored_events": [
                    {
                        "title": "Loaded compare target event.",
                        "dominant_field": "economy",
                        "impact_score": 9.0,
                        "field_attraction": 5.0,
                        "divergence_score": 11.0,
                    }
                ],
                "intervention_candidates": [],
            },
            3: {
                "run_id": 3,
                "generated_at": "2026-03-22T09:00:00+00:00",
                "mode": "fixture",
                "input_summary": {"raw_item_count": 2, "normalized_event_count": 1},
                "scenario_summary": {
                    "dominant_field": "technology",
                    "risk_level": "high",
                    "headline": "Persisted previous run outside the loaded limit.",
                    "event_groups": [],
                },
                "scored_events": [
                    {
                        "title": "Persisted previous event.",
                        "dominant_field": "technology",
                        "impact_score": 12.0,
                        "field_attraction": 6.0,
                        "divergence_score": 18.0,
                    }
                ],
                "intervention_candidates": [],
            },
            20: {
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
        }[kwargs["run_id"]],
    )
    @mock.patch(
        "tianji.tui_state.get_run_summary",
        side_effect=lambda *args, **kwargs: {
            10: {
                "run_id": 10,
                "generated_at": "2026-03-22T10:00:00+00:00",
                "mode": "fixture",
                "input_summary": {"raw_item_count": 3, "normalized_event_count": 3},
                "scenario_summary": {
                    "dominant_field": "technology",
                    "risk_level": "high",
                    "headline": "Loaded run before persisted navigation.",
                    "event_groups": [],
                },
                "scored_events": [
                    {
                        "title": "Loaded run event.",
                        "dominant_field": "technology",
                        "impact_score": 14.0,
                        "field_attraction": 7.0,
                        "divergence_score": 19.0,
                    }
                ],
                "intervention_candidates": [],
            },
            30: {
                "run_id": 30,
                "generated_at": "2026-03-22T12:00:00+00:00",
                "mode": "fixture",
                "input_summary": {"raw_item_count": 3, "normalized_event_count": 2},
                "scenario_summary": {
                    "dominant_field": "economy",
                    "risk_level": "low",
                    "headline": "Loaded compare target before compare focus.",
                    "event_groups": [],
                },
                "scored_events": [
                    {
                        "title": "Loaded compare target event.",
                        "dominant_field": "economy",
                        "impact_score": 9.0,
                        "field_attraction": 5.0,
                        "divergence_score": 11.0,
                    }
                ],
                "intervention_candidates": [],
            },
            3: {
                "run_id": 3,
                "generated_at": "2026-03-22T09:00:00+00:00",
                "mode": "fixture",
                "input_summary": {"raw_item_count": 2, "normalized_event_count": 1},
                "scenario_summary": {
                    "dominant_field": "technology",
                    "risk_level": "high",
                    "headline": "Persisted previous run outside the loaded limit.",
                    "event_groups": [],
                },
                "scored_events": [
                    {
                        "title": "Persisted previous event.",
                        "dominant_field": "technology",
                        "impact_score": 12.0,
                        "field_attraction": 6.0,
                        "divergence_score": 18.0,
                    }
                ],
                "intervention_candidates": [],
            },
            20: {
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
        }[kwargs["run_id"]],
    )
    @mock.patch("tianji.tui_state.get_next_run_id", side_effect=[20, 30, None])
    @mock.patch("tianji.tui_state.get_previous_run_id", side_effect=[3, None])
    def test_run_history_browser_session_preserves_persisted_navigation_parity_across_detail_and_compare_flows(
        self,
        mock_get_previous_run_id,
        mock_get_next_run_id,
        mock_get_state_run_summary,
        mock_get_render_run_summary,
        mock_compare_runs,
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
            ["l", "[", "[", "j", "l", "a", "h", "j", "c", "]", "]", "q"],
            height=50,
        )

        self.assertEqual(
            sorted([cast(int, row["run_id"]) for row in state.rows]), [3, 20, 30]
        )
        self.assertEqual(cast(int, state.rows[state.selected_index]["run_id"]), 30)
        self.assertEqual(state.dominant_field, "conflict")
        self.assertEqual(mock_get_previous_run_id.call_count, 2)
        self.assertGreaterEqual(mock_get_next_run_id.call_count, 2)
        self.assertGreaterEqual(mock_get_render_run_summary.call_count, 2)

        messages = {frame["message"].strip() for frame in frames}
        self.assertIn("first run", messages)
        self.assertNotIn("first compare target", messages)
        self.assertIn("lens:ev=conflict", frames[-1]["header"])

    @mock.patch(
        "tianji.tui_render.get_run_summary",
        return_value={
            "run_id": 10,
            "generated_at": "2026-03-22T10:00:00+00:00",
            "mode": "fixture",
            "input_summary": {"raw_item_count": 3, "normalized_event_count": 3},
            "scenario_summary": {
                "dominant_field": "technology",
                "risk_level": "high",
                "headline": "Reusable detail headline.",
                "event_groups": [],
            },
            "scored_events": [],
            "intervention_candidates": [],
        },
    )
    @mock.patch("tianji.tui_render.compare_runs")
    def test_run_history_browser_session_recomputes_compare_after_clear_and_restage(
        self, mock_compare_runs, _mock_get_run_summary
    ) -> None:
        def compare_side_effect(*args: object, **kwargs: object) -> dict[str, object]:
            left_run_id = kwargs.get("left_run_id")
            right_run_id = kwargs.get("right_run_id")
            if left_run_id == 10 and right_run_id == 20:
                return {
                    "left": {
                        "run_id": 10,
                        "mode": "fixture",
                        "dominant_field": "technology",
                        "risk_level": "high",
                        "headline": "Original left compare truth.",
                        "top_event_group": None,
                        "top_scored_event": None,
                        "top_intervention": None,
                        "event_group_count": 0,
                        "intervention_event_ids": [],
                    },
                    "right": {
                        "run_id": 20,
                        "mode": "fixture",
                        "dominant_field": "diplomacy",
                        "risk_level": "medium",
                        "headline": "Shared right compare target.",
                        "top_event_group": None,
                        "top_scored_event": None,
                        "top_intervention": None,
                        "event_group_count": 0,
                        "intervention_event_ids": [],
                    },
                    "diff": {
                        "dominant_field_changed": True,
                        "risk_level_changed": True,
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
                        "top_divergence_score_delta": None,
                        "top_impact_score_delta": None,
                        "top_field_attraction_delta": None,
                        "top_intervention_changed": False,
                    },
                }
            if left_run_id == 30 and right_run_id == 20:
                return {
                    "left": {
                        "run_id": 30,
                        "mode": "fixture",
                        "dominant_field": "economy",
                        "risk_level": "low",
                        "headline": "Restaged left compare truth.",
                        "top_event_group": None,
                        "top_scored_event": None,
                        "top_intervention": None,
                        "event_group_count": 0,
                        "intervention_event_ids": [],
                    },
                    "right": {
                        "run_id": 20,
                        "mode": "fixture",
                        "dominant_field": "diplomacy",
                        "risk_level": "medium",
                        "headline": "Shared right compare target.",
                        "top_event_group": None,
                        "top_scored_event": None,
                        "top_intervention": None,
                        "event_group_count": 0,
                        "intervention_event_ids": [],
                    },
                    "diff": {
                        "dominant_field_changed": True,
                        "risk_level_changed": True,
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
                        "top_divergence_score_delta": None,
                        "top_impact_score_delta": None,
                        "top_field_attraction_delta": None,
                        "top_intervention_changed": False,
                    },
                }
            raise AssertionError(
                f"unexpected compare pair: L={left_run_id} R={right_run_id}"
            )

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

        self._run_browser_session(
            state, ["c", "j", "c", "C", "j", "c", "k", "c", "l", "q"], height=60
        )

        compare_cache_text = "\n".join(state.cached_compare_lines or [])
        self.assertIn("Compare: Run #30 (Left) vs Run #20", compare_cache_text)
        self.assertIn("Restaged left compare truth.", compare_cache_text)
        self.assertNotIn("Original left compare truth.", compare_cache_text)
        self.assertEqual(mock_compare_runs.call_count, 2)

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
        self.assertEqual(browser_mock.call_args.args[0].rows, rows)
