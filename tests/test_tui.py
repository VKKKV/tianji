from support import *


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
                "scored_events": [{"title": "Tech Event 1"}],
                "intervention_candidates": [{}, {}],
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
            self.assertIn("Top: technology (3 members)", text)
            self.assertIn("Scored Events: 1", text)
            self.assertIn("Top: Tech Event 1", text)
            self.assertIn("Interventions: 2", text)

        def test_launch_history_tui_prints_empty_state_without_curses(self) -> None:
            stdout = io.StringIO()
            with contextlib.redirect_stdout(stdout):
                with mock.patch("tianji.tui.list_runs", return_value=[]):
                    with mock.patch("tianji.tui.curses.wrapper") as wrapper_mock:
                        exit_code = launch_history_tui(
                            sqlite_path="runs/tianji.sqlite3",
                            limit=20,
                        )

            self.assertEqual(exit_code, 0)
            self.assertIn(
                "No persisted runs are available for the TUI browser.", stdout.getvalue()
            )
            wrapper_mock.assert_not_called()

        def test_launch_history_tui_uses_curses_wrapper_for_available_runs(self) -> None:
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
                with mock.patch("tianji.tui.curses.wrapper") as wrapper_mock:
                    exit_code = launch_history_tui(
                        sqlite_path="runs/tianji.sqlite3",
                        limit=20,
                    )

            self.assertEqual(exit_code, 0)
            wrapper_mock.assert_called_once()
            wrapper_args = wrapper_mock.call_args.args
            self.assertEqual(wrapper_args[0].__name__, "run_history_list_browser")
            self.assertEqual(wrapper_args[1].rows, rows)
