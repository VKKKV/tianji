from support import *


class HistoryCompareTests(unittest.TestCase):
        def test_cli_history_compare_reads_two_persisted_runs(self) -> None:
            with TemporaryDirectory() as tmpdir:
                sqlite_path = Path(tmpdir) / "tianji.sqlite3"
                run_pipeline(
                    fixture_paths=[str(FIXTURE_PATH)],
                    fetch=False,
                    source_urls=[],
                    output_path=None,
                    sqlite_path=str(sqlite_path),
                )

                empty_feed = """<?xml version="1.0" encoding="UTF-8"?>
    <rss version="2.0"><channel><title>Empty TianJi Feed</title></channel></rss>
    """
                empty_fixture = Path(tmpdir) / "empty.xml"
                empty_fixture.write_text(empty_feed, encoding="utf-8")
                run_pipeline(
                    fixture_paths=[str(empty_fixture)],
                    fetch=False,
                    source_urls=[],
                    output_path=None,
                    sqlite_path=str(sqlite_path),
                )

                stdout = io.StringIO()
                with contextlib.redirect_stdout(stdout):
                    exit_code = main(
                        [
                            "history-compare",
                            "--sqlite-path",
                            str(sqlite_path),
                            "--left-run-id",
                            "1",
                            "--right-run-id",
                            "2",
                        ]
                    )

                self.assertEqual(exit_code, 0)
                payload = json.loads(stdout.getvalue())
                self.assertEqual(payload["left_run_id"], 1)
                self.assertEqual(payload["right_run_id"], 2)
                self.assertEqual(payload["left"]["dominant_field"], "technology")
                self.assertEqual(payload["right"]["dominant_field"], "uncategorized")
                self.assertEqual(payload["diff"]["raw_item_count_delta"], -3)
                self.assertEqual(payload["left"]["event_group_count"], 0)
                self.assertEqual(payload["right"]["event_group_count"], 0)
                self.assertEqual(payload["diff"]["event_group_count_delta"], 0)
                self.assertFalse(payload["diff"]["top_event_group_changed"])
                self.assertIsNone(payload["diff"]["left_top_event_group_headline_event_id"])
                self.assertIsNone(
                    payload["diff"]["right_top_event_group_headline_event_id"]
                )
                self.assertTrue(payload["diff"]["top_scored_event_changed"])
                self.assertFalse(payload["diff"]["top_scored_event_comparable"])
                self.assertTrue(payload["diff"]["top_intervention_changed"])
                self.assertEqual(
                    payload["diff"]["left_top_scored_event_id"],
                    payload["left"]["top_scored_event"]["event_id"],
                )
                self.assertIsNone(payload["diff"]["right_top_scored_event_id"])
                self.assertEqual(
                    payload["diff"]["left_top_impact_score"],
                    payload["left"]["top_scored_event"]["impact_score"],
                )
                self.assertIsNone(payload["diff"]["right_top_impact_score"])
                self.assertIsNone(payload["diff"]["top_impact_score_delta"])
                self.assertEqual(
                    payload["diff"]["left_top_field_attraction"],
                    payload["left"]["top_scored_event"]["field_attraction"],
                )
                self.assertIsNone(payload["diff"]["right_top_field_attraction"])
                self.assertIsNone(payload["diff"]["top_field_attraction_delta"])
                self.assertEqual(
                    payload["diff"]["left_top_divergence_score"],
                    payload["left"]["top_scored_event"]["divergence_score"],
                )
                self.assertIsNone(payload["diff"]["right_top_divergence_score"])
                self.assertIsNone(payload["diff"]["top_divergence_score_delta"])
                self.assertEqual(
                    payload["diff"]["left_top_intervention_event_id"],
                    payload["left"]["top_intervention"]["event_id"],
                )
                self.assertIsNone(payload["diff"]["right_top_intervention_event_id"])
                self.assertEqual(
                    payload["diff"]["left_only_intervention_event_ids"],
                    [
                        payload["left"]["intervention_event_ids"][0],
                        payload["left"]["intervention_event_ids"][1],
                        payload["left"]["intervention_event_ids"][2],
                    ],
                )
                self.assertEqual(payload["diff"]["right_only_intervention_event_ids"], [])

        def test_cli_history_compare_rejects_non_positive_run_id(self) -> None:
            stderr = io.StringIO()
            with contextlib.redirect_stderr(stderr):
                with self.assertRaises(SystemExit) as error:
                    main(
                        [
                            "history-compare",
                            "--sqlite-path",
                            "runs/tianji.sqlite3",
                            "--run-id",
                            "0",
                            "--against-latest",
                        ]
                    )

            self.assertEqual(error.exception.code, 2)
            self.assertIn("--run-id must be greater than zero.", stderr.getvalue())

        def test_cli_history_compare_rejects_non_positive_explicit_pair_ids(self) -> None:
            stderr = io.StringIO()
            with contextlib.redirect_stderr(stderr):
                with self.assertRaises(SystemExit) as error:
                    main(
                        [
                            "history-compare",
                            "--sqlite-path",
                            "runs/tianji.sqlite3",
                            "--left-run-id",
                            "0",
                            "--right-run-id",
                            "2",
                        ]
                    )

            self.assertEqual(error.exception.code, 2)
            self.assertIn("--left-run-id must be greater than zero.", stderr.getvalue())

        def test_cli_history_compare_can_use_latest_pair(self) -> None:
            with TemporaryDirectory() as tmpdir:
                sqlite_path = Path(tmpdir) / "tianji.sqlite3"
                run_pipeline(
                    fixture_paths=[str(FIXTURE_PATH)],
                    fetch=False,
                    source_urls=[],
                    output_path=None,
                    sqlite_path=str(sqlite_path),
                )

                empty_feed = """<?xml version="1.0" encoding="UTF-8"?>
    <rss version="2.0"><channel><title>Empty TianJi Feed</title></channel></rss>
    """
                empty_fixture = Path(tmpdir) / "empty.xml"
                empty_fixture.write_text(empty_feed, encoding="utf-8")
                run_pipeline(
                    fixture_paths=[str(empty_fixture)],
                    fetch=False,
                    source_urls=[],
                    output_path=None,
                    sqlite_path=str(sqlite_path),
                )

                stdout = io.StringIO()
                with contextlib.redirect_stdout(stdout):
                    exit_code = main(
                        [
                            "history-compare",
                            "--sqlite-path",
                            str(sqlite_path),
                            "--latest-pair",
                        ]
                    )

                self.assertEqual(exit_code, 0)
                payload = json.loads(stdout.getvalue())
                self.assertEqual(payload["left_run_id"], 1)
                self.assertEqual(payload["right_run_id"], 2)

        def test_cli_history_compare_can_compare_run_against_latest(self) -> None:
            with TemporaryDirectory() as tmpdir:
                sqlite_path = Path(tmpdir) / "tianji.sqlite3"
                run_pipeline(
                    fixture_paths=[str(FIXTURE_PATH)],
                    fetch=False,
                    source_urls=[],
                    output_path=None,
                    sqlite_path=str(sqlite_path),
                )

                empty_feed = """<?xml version="1.0" encoding="UTF-8"?>
    <rss version="2.0"><channel><title>Empty TianJi Feed</title></channel></rss>
    """
                empty_fixture = Path(tmpdir) / "empty.xml"
                empty_fixture.write_text(empty_feed, encoding="utf-8")
                run_pipeline(
                    fixture_paths=[str(empty_fixture)],
                    fetch=False,
                    source_urls=[],
                    output_path=None,
                    sqlite_path=str(sqlite_path),
                )

                stdout = io.StringIO()
                with contextlib.redirect_stdout(stdout):
                    exit_code = main(
                        [
                            "history-compare",
                            "--sqlite-path",
                            str(sqlite_path),
                            "--run-id",
                            "1",
                            "--against-latest",
                        ]
                    )

                self.assertEqual(exit_code, 0)
                payload = json.loads(stdout.getvalue())
                self.assertEqual(payload["left_run_id"], 1)
                self.assertEqual(payload["right_run_id"], 2)

        def test_cli_history_compare_can_compare_run_against_previous(self) -> None:
            with TemporaryDirectory() as tmpdir:
                sqlite_path = Path(tmpdir) / "tianji.sqlite3"
                first_empty = """<?xml version="1.0" encoding="UTF-8"?>
    <rss version="2.0"><channel><title>First Empty Feed</title></channel></rss>
    """
                second_empty = """<?xml version="1.0" encoding="UTF-8"?>
    <rss version="2.0"><channel><title>Second Empty Feed</title></channel></rss>
    """
                first_fixture = Path(tmpdir) / "first-empty.xml"
                second_fixture = Path(tmpdir) / "second-empty.xml"
                first_fixture.write_text(first_empty, encoding="utf-8")
                second_fixture.write_text(second_empty, encoding="utf-8")

                run_pipeline(
                    fixture_paths=[str(FIXTURE_PATH)],
                    fetch=False,
                    source_urls=[],
                    output_path=None,
                    sqlite_path=str(sqlite_path),
                )
                run_pipeline(
                    fixture_paths=[str(first_fixture)],
                    fetch=False,
                    source_urls=[],
                    output_path=None,
                    sqlite_path=str(sqlite_path),
                )
                run_pipeline(
                    fixture_paths=[str(second_fixture)],
                    fetch=False,
                    source_urls=[],
                    output_path=None,
                    sqlite_path=str(sqlite_path),
                )

                stdout = io.StringIO()
                with contextlib.redirect_stdout(stdout):
                    exit_code = main(
                        [
                            "history-compare",
                            "--sqlite-path",
                            str(sqlite_path),
                            "--run-id",
                            "3",
                            "--against-previous",
                        ]
                    )

                self.assertEqual(exit_code, 0)
                payload = json.loads(stdout.getvalue())
                self.assertEqual(payload["left_run_id"], 2)
                self.assertEqual(payload["right_run_id"], 3)

        def test_cli_history_compare_rejects_negative_scored_event_limit(self) -> None:
            stderr = io.StringIO()
            with contextlib.redirect_stderr(stderr):
                with self.assertRaises(SystemExit) as error:
                    main(
                        [
                            "history-compare",
                            "--sqlite-path",
                            "runs/tianji.sqlite3",
                            "--left-run-id",
                            "1",
                            "--right-run-id",
                            "2",
                            "--limit-scored-events",
                            "-1",
                        ]
                    )

            self.assertEqual(error.exception.code, 2)
            self.assertIn(
                "--limit-scored-events must be zero or greater.", stderr.getvalue()
            )

        def test_cli_history_compare_rejects_negative_event_group_limit(self) -> None:
            stderr = io.StringIO()
            with contextlib.redirect_stderr(stderr):
                with self.assertRaises(SystemExit) as error:
                    main(
                        [
                            "history-compare",
                            "--sqlite-path",
                            "runs/tianji.sqlite3",
                            "--left-run-id",
                            "1",
                            "--right-run-id",
                            "2",
                            "--limit-event-groups",
                            "-1",
                        ]
                    )

            self.assertEqual(error.exception.code, 2)
            self.assertIn(
                "--limit-event-groups must be zero or greater.", stderr.getvalue()
            )

        def test_cli_history_compare_rejects_inverted_impact_score_range(self) -> None:
            stderr = io.StringIO()
            with contextlib.redirect_stderr(stderr):
                with self.assertRaises(SystemExit) as error:
                    main(
                        [
                            "history-compare",
                            "--sqlite-path",
                            "runs/tianji.sqlite3",
                            "--left-run-id",
                            "1",
                            "--right-run-id",
                            "2",
                            "--min-impact-score",
                            "5",
                            "--max-impact-score",
                            "4",
                        ]
                    )

            self.assertEqual(error.exception.code, 2)
            self.assertIn(
                "--min-impact-score cannot be greater than --max-impact-score.",
                stderr.getvalue(),
            )

        def test_cli_history_compare_rejects_inverted_field_attraction_range(
            self,
        ) -> None:
            stderr = io.StringIO()
            with contextlib.redirect_stderr(stderr):
                with self.assertRaises(SystemExit) as error:
                    main(
                        [
                            "history-compare",
                            "--sqlite-path",
                            "runs/tianji.sqlite3",
                            "--left-run-id",
                            "1",
                            "--right-run-id",
                            "2",
                            "--min-field-attraction",
                            "5",
                            "--max-field-attraction",
                            "4",
                        ]
                    )

            self.assertEqual(error.exception.code, 2)
            self.assertIn(
                "--min-field-attraction cannot be greater than --max-field-attraction.",
                stderr.getvalue(),
            )

        def test_cli_history_compare_rejects_inverted_divergence_score_range(
            self,
        ) -> None:
            stderr = io.StringIO()
            with contextlib.redirect_stderr(stderr):
                with self.assertRaises(SystemExit) as error:
                    main(
                        [
                            "history-compare",
                            "--sqlite-path",
                            "runs/tianji.sqlite3",
                            "--left-run-id",
                            "1",
                            "--right-run-id",
                            "2",
                            "--min-divergence-score",
                            "5",
                            "--max-divergence-score",
                            "4",
                        ]
                    )

            self.assertEqual(error.exception.code, 2)
            self.assertIn(
                "--min-divergence-score cannot be greater than --max-divergence-score.",
                stderr.getvalue(),
            )

        def test_cli_history_compare_rejects_mixed_explicit_pair_and_against_latest(
            self,
        ) -> None:
            stderr = io.StringIO()
            with contextlib.redirect_stderr(stderr):
                with self.assertRaises(SystemExit) as error:
                    main(
                        [
                            "history-compare",
                            "--sqlite-path",
                            "runs/tianji.sqlite3",
                            "--left-run-id",
                            "1",
                            "--right-run-id",
                            "2",
                            "--run-id",
                            "3",
                            "--against-latest",
                        ]
                    )

            self.assertEqual(error.exception.code, 2)
            self.assertIn(
                "Use either --latest-pair, --run-id with --against-latest, --run-id with --against-previous, or explicit --left-run-id/--right-run-id, not a mix.",
                stderr.getvalue(),
            )

        def test_cli_history_compare_rejects_mixed_explicit_pair_and_against_previous(
            self,
        ) -> None:
            stderr = io.StringIO()
            with contextlib.redirect_stderr(stderr):
                with self.assertRaises(SystemExit) as error:
                    main(
                        [
                            "history-compare",
                            "--sqlite-path",
                            "runs/tianji.sqlite3",
                            "--left-run-id",
                            "1",
                            "--right-run-id",
                            "2",
                            "--run-id",
                            "3",
                            "--against-previous",
                        ]
                    )

            self.assertEqual(error.exception.code, 2)
            self.assertIn(
                "Use either --latest-pair, --run-id with --against-latest, --run-id with --against-previous, or explicit --left-run-id/--right-run-id, not a mix.",
                stderr.getvalue(),
            )

        def test_cli_history_compare_surfaces_top_group_evidence_diff(self) -> None:
            grouped_feed_left = """<?xml version="1.0" encoding="UTF-8"?>
    <rss version="2.0">
      <channel>
        <title>Grouped TianJi Feed Left</title>
        <item>
          <title>China and USA expand chip controls across East Asia export lanes</title>
          <link>https://example.com/group-a</link>
          <pubDate>Sun, 22 Mar 2026 08:00:00 GMT</pubDate>
          <description>Officials in China and the USA expand chip export controls across East Asia supply lanes.</description>
        </item>
        <item>
          <title>USA and China widen chip export controls after East Asia dispute</title>
          <link>https://example.com/group-b</link>
          <pubDate>Sun, 22 Mar 2026 09:00:00 GMT</pubDate>
          <description>USA and China widen chip export controls after an East Asia technology dispute.</description>
        </item>
      </channel>
    </rss>
    """
            grouped_feed_right = """<?xml version="1.0" encoding="UTF-8"?>
    <rss version="2.0">
      <channel>
        <title>Grouped TianJi Feed Right</title>
        <item>
          <title>China and USA expand chip controls across East Asia export lanes</title>
          <link>https://example.com/group-a</link>
          <pubDate>Sun, 22 Mar 2026 08:00:00 GMT</pubDate>
          <description>Officials in China and the USA expand chip export controls across East Asia supply lanes.</description>
        </item>
        <item>
          <title>USA and China widen chip export controls after East Asia dispute</title>
          <link>https://example.com/group-b</link>
          <pubDate>Sun, 22 Mar 2026 09:00:00 GMT</pubDate>
          <description>USA and China widen chip export controls after an East Asia technology dispute.</description>
        </item>
        <item>
          <title>China and USA add East Asia chip controls after export review</title>
          <link>https://example.com/group-c</link>
          <pubDate>Sun, 22 Mar 2026 11:00:00 GMT</pubDate>
          <description>China and the USA add East Asia chip export controls after a new review.</description>
        </item>
      </channel>
    </rss>
    """

            with TemporaryDirectory() as tmpdir:
                sqlite_path = Path(tmpdir) / "tianji.sqlite3"
                fixture_path = Path(tmpdir) / "grouped.xml"
                fixture_path.write_text(grouped_feed_left, encoding="utf-8")

                run_pipeline(
                    fixture_paths=[str(fixture_path)],
                    fetch=False,
                    source_urls=[],
                    output_path=None,
                    sqlite_path=str(sqlite_path),
                )
                fixture_path.write_text(grouped_feed_right, encoding="utf-8")
                run_pipeline(
                    fixture_paths=[str(fixture_path)],
                    fetch=False,
                    source_urls=[],
                    output_path=None,
                    sqlite_path=str(sqlite_path),
                )

                stdout = io.StringIO()
                with contextlib.redirect_stdout(stdout):
                    exit_code = main(
                        [
                            "history-compare",
                            "--sqlite-path",
                            str(sqlite_path),
                            "--left-run-id",
                            "1",
                            "--right-run-id",
                            "2",
                        ]
                    )

                self.assertEqual(exit_code, 0)
                payload = json.loads(stdout.getvalue())
                evidence_diff = cast(
                    dict[str, object], payload["diff"]["top_event_group_evidence_diff"]
                )
                self.assertEqual(payload["left_run_id"], 1)
                self.assertEqual(payload["right_run_id"], 2)
                self.assertEqual(payload["left"]["event_group_count"], 1)
                self.assertEqual(payload["right"]["event_group_count"], 1)
                self.assertIn(
                    "causal_ordered_event_ids", payload["left"]["top_event_group"]
                )
                self.assertIn("evidence_chain", payload["left"]["top_event_group"])
                self.assertIn("causal_span_hours", payload["left"]["top_event_group"])
                self.assertIn("causal_summary", payload["left"]["top_event_group"])
                self.assertIn(
                    "causal_ordered_event_ids", payload["right"]["top_event_group"]
                )
                self.assertIn("evidence_chain", payload["right"]["top_event_group"])
                self.assertIn("causal_span_hours", payload["right"]["top_event_group"])
                self.assertIn("causal_summary", payload["right"]["top_event_group"])
                self.assertEqual(
                    payload["left"]["top_event_group"]["causal_ordered_event_ids"],
                    [
                        payload["left"]["top_event_group"]["evidence_chain"][0][
                            "from_event_id"
                        ],
                        payload["left"]["top_event_group"]["evidence_chain"][0][
                            "to_event_id"
                        ],
                    ],
                )
                self.assertEqual(
                    payload["left"]["top_event_group"]["causal_span_hours"], 1.0
                )
                self.assertEqual(
                    payload["left"]["top_event_group"]["evidence_chain"][0][
                        "from_event_id"
                    ],
                    payload["left"]["top_event_group"]["headline_event_id"],
                )
                self.assertEqual(
                    payload["right"]["top_event_group"]["causal_span_hours"], 3.0
                )
                self.assertEqual(
                    len(payload["right"]["top_event_group"]["causal_ordered_event_ids"]), 3
                )
                self.assertIn(
                    "capability-race cluster",
                    payload["left"]["top_event_group"]["causal_summary"],
                )
                self.assertNotIn("top_event_group_chain_summary", payload["left"])
                self.assertNotIn("top_event_group_member_event_ids", payload["left"])
                self.assertNotIn("top_event_group_shared_keywords", payload["left"])
                self.assertNotIn("top_event_group_shared_actors", payload["left"])
                self.assertNotIn("top_event_group_shared_regions", payload["left"])
                self.assertNotIn("top_event_group_evidence_chain", payload["left"])
                self.assertEqual(
                    payload["diff"]["left_only_event_group_headline_event_ids"], []
                )
                self.assertEqual(
                    payload["diff"]["right_only_event_group_headline_event_ids"], []
                )
                self.assertTrue(evidence_diff["comparable"])
                self.assertTrue(evidence_diff["same_headline_event_id"])
                self.assertEqual(evidence_diff["member_count_delta"], 1)
                self.assertEqual(evidence_diff["left_only_member_event_ids"], [])
                self.assertEqual(
                    len(cast(list[str], evidence_diff["right_only_member_event_ids"])), 1
                )
                self.assertEqual(evidence_diff["evidence_chain_link_count_delta"], 1)
                self.assertTrue(evidence_diff["chain_summary_changed"])
                self.assertGreaterEqual(
                    len(cast(list[str], evidence_diff["evidence_chain_links_added"])), 1
                )

        def test_cli_history_compare_can_filter_projected_views(self) -> None:
            grouped_feed = """<?xml version="1.0" encoding="UTF-8"?>
    <rss version="2.0">
      <channel>
        <title>Multi Grouped TianJi Feed</title>
        <item>
          <title>China and USA expand chip controls across East Asia export lanes</title>
          <link>https://example.com/group-a</link>
          <pubDate>Sun, 22 Mar 2026 08:00:00 GMT</pubDate>
          <description>Officials in China and the USA expand chip export controls across East Asia supply lanes.</description>
        </item>
        <item>
          <title>USA and China widen chip export controls after East Asia dispute</title>
          <link>https://example.com/group-b</link>
          <pubDate>Sun, 22 Mar 2026 09:00:00 GMT</pubDate>
          <description>USA and China widen chip export controls after an East Asia technology dispute.</description>
        </item>
        <item>
          <title>Iran diplomacy channel reopens for regional talks</title>
          <link>https://example.com/group-c</link>
          <pubDate>Sun, 22 Mar 2026 10:00:00 GMT</pubDate>
          <description>Iran reopens a diplomacy channel for new regional talks.</description>
        </item>
        <item>
          <title>Iran diplomats reopen regional talks through Oman channel</title>
          <link>https://example.com/group-d</link>
          <pubDate>Sun, 22 Mar 2026 11:00:00 GMT</pubDate>
          <description>Iran diplomats reopen regional talks through an Oman mediation channel.</description>
        </item>
      </channel>
    </rss>
    """

            with TemporaryDirectory() as tmpdir:
                sqlite_path = Path(tmpdir) / "tianji.sqlite3"
                grouped_fixture = Path(tmpdir) / "grouped.xml"
                grouped_fixture.write_text(grouped_feed, encoding="utf-8")

                run_pipeline(
                    fixture_paths=[str(grouped_fixture)],
                    fetch=False,
                    source_urls=[],
                    output_path=None,
                    sqlite_path=str(sqlite_path),
                )
                run_pipeline(
                    fixture_paths=[str(grouped_fixture)],
                    fetch=False,
                    source_urls=[],
                    output_path=None,
                    sqlite_path=str(sqlite_path),
                )

                stdout = io.StringIO()
                with contextlib.redirect_stdout(stdout):
                    exit_code = main(
                        [
                            "history-compare",
                            "--sqlite-path",
                            str(sqlite_path),
                            "--left-run-id",
                            "1",
                            "--right-run-id",
                            "2",
                            "--dominant-field",
                            "diplomacy",
                            "--limit-scored-events",
                            "1",
                            "--only-matching-interventions",
                            "--group-dominant-field",
                            "diplomacy",
                            "--limit-event-groups",
                            "1",
                        ]
                    )

                self.assertEqual(exit_code, 0)
                payload = json.loads(stdout.getvalue())
                self.assertEqual(
                    payload["left"]["top_scored_event"]["dominant_field"], "diplomacy"
                )
                self.assertEqual(
                    payload["right"]["top_scored_event"]["dominant_field"], "diplomacy"
                )
                self.assertEqual(len(payload["left"]["intervention_event_ids"]), 1)
                self.assertEqual(
                    payload["left"]["intervention_event_ids"][0],
                    payload["left"]["top_scored_event"]["event_id"],
                )
                self.assertEqual(
                    payload["left"]["event_group_count"],
                    1,
                )
                self.assertEqual(
                    payload["left"]["top_event_group"]["dominant_field"],
                    "diplomacy",
                )

        def test_cli_history_compare_can_return_empty_filtered_views(self) -> None:
            with TemporaryDirectory() as tmpdir:
                sqlite_path = Path(tmpdir) / "tianji.sqlite3"
                run_pipeline(
                    fixture_paths=[str(FIXTURE_PATH)],
                    fetch=False,
                    source_urls=[],
                    output_path=None,
                    sqlite_path=str(sqlite_path),
                )
                run_pipeline(
                    fixture_paths=[str(FIXTURE_PATH)],
                    fetch=False,
                    source_urls=[],
                    output_path=None,
                    sqlite_path=str(sqlite_path),
                )

                stdout = io.StringIO()
                with contextlib.redirect_stdout(stdout):
                    exit_code = main(
                        [
                            "history-compare",
                            "--sqlite-path",
                            str(sqlite_path),
                            "--left-run-id",
                            "1",
                            "--right-run-id",
                            "2",
                            "--dominant-field",
                            "uncategorized",
                            "--group-dominant-field",
                            "uncategorized",
                            "--only-matching-interventions",
                        ]
                    )

                self.assertEqual(exit_code, 0)
                payload = json.loads(stdout.getvalue())
                self.assertIsNone(payload["left"]["top_scored_event"])
                self.assertIsNone(payload["right"]["top_scored_event"])
                self.assertEqual(payload["left"]["intervention_event_ids"], [])
                self.assertEqual(payload["right"]["intervention_event_ids"], [])
                self.assertEqual(payload["left"]["event_group_count"], 0)
                self.assertEqual(payload["right"]["event_group_count"], 0)
                self.assertIsNone(payload["diff"]["left_top_scored_event_id"])
                self.assertIsNone(payload["diff"]["right_top_scored_event_id"])

        def test_history_compare_reports_group_diff_when_grouping_changes(self) -> None:
            left = {
                "run_id": 1,
                "schema_version": "tianji.run-artifact.v1",
                "mode": "fixture",
                "generated_at": "2026-03-22T10:00:00+00:00",
                "input_summary": {"raw_item_count": 3, "normalized_event_count": 3},
                "scenario_summary": {
                    "dominant_field": "technology",
                    "risk_level": "high",
                    "headline": "left",
                    "event_groups": [
                        {
                            "group_id": "group:evt-a",
                            "headline_event_id": "evt-a",
                            "headline_title": "China and USA expand chip controls",
                            "member_event_ids": ["evt-a", "evt-b"],
                            "member_count": 2,
                            "dominant_field": "technology",
                            "shared_keywords": ["chip", "controls", "export"],
                            "shared_actors": ["china", "usa"],
                            "shared_regions": ["east-asia", "united-states"],
                            "group_score": 37.89,
                        }
                    ],
                },
                "scored_events": [],
                "intervention_candidates": [],
            }
            right = {
                "run_id": 2,
                "schema_version": "tianji.run-artifact.v1",
                "mode": "fixture",
                "generated_at": "2026-03-22T11:00:00+00:00",
                "input_summary": {"raw_item_count": 2, "normalized_event_count": 2},
                "scenario_summary": {
                    "dominant_field": "diplomacy",
                    "risk_level": "medium",
                    "headline": "right",
                    "event_groups": [
                        {
                            "group_id": "group:evt-c",
                            "headline_event_id": "evt-c",
                            "headline_title": "Iran diplomacy channel reopens",
                            "member_event_ids": ["evt-c", "evt-d"],
                            "member_count": 2,
                            "dominant_field": "diplomacy",
                            "shared_keywords": ["channel", "diplomacy"],
                            "shared_actors": ["iran"],
                            "shared_regions": ["middle-east"],
                            "group_score": 20.5,
                        }
                    ],
                },
                "scored_events": [],
                "intervention_candidates": [],
            }

            left_summary = storage.build_compare_side(left)
            right_summary = storage.build_compare_side(right)
            diff = storage.build_compare_diff(left_summary, right_summary)
            evidence_diff = cast(dict[str, object], diff["top_event_group_evidence_diff"])

            self.assertEqual(left_summary["event_group_count"], 1)
            self.assertEqual(right_summary["event_group_count"], 1)
            self.assertEqual(diff["event_group_count_delta"], 0)
            self.assertTrue(diff["top_event_group_changed"])
            self.assertEqual(diff["left_top_event_group_headline_event_id"], "evt-a")
            self.assertEqual(diff["right_top_event_group_headline_event_id"], "evt-c")
            self.assertEqual(diff["left_only_event_group_headline_event_ids"], ["evt-a"])
            self.assertEqual(diff["right_only_event_group_headline_event_ids"], ["evt-c"])
            self.assertFalse(evidence_diff["comparable"])
            self.assertFalse(evidence_diff["same_headline_event_id"])

        def test_history_compare_reports_top_group_evidence_chain_deltas(self) -> None:
            left = {
                "run_id": 1,
                "schema_version": "tianji.run-artifact.v1",
                "mode": "fixture",
                "generated_at": "2026-03-22T10:00:00+00:00",
                "input_summary": {"raw_item_count": 3, "normalized_event_count": 3},
                "scenario_summary": {
                    "dominant_field": "technology",
                    "risk_level": "high",
                    "headline": "left",
                    "event_groups": [
                        {
                            "group_id": "group:evt-a",
                            "headline_event_id": "evt-a",
                            "headline_title": "China and USA expand chip controls",
                            "member_event_ids": ["evt-a", "evt-b"],
                            "member_count": 2,
                            "dominant_field": "technology",
                            "shared_keywords": ["chip", "controls", "export"],
                            "shared_actors": ["china", "usa"],
                            "shared_regions": ["east-asia", "united-states"],
                            "group_score": 37.89,
                            "evidence_chain": [
                                {
                                    "from_event_id": "evt-a",
                                    "to_event_id": "evt-b",
                                    "shared_keywords": ["chip", "controls", "export"],
                                    "shared_actors": ["china", "usa"],
                                    "shared_regions": ["east-asia", "united-states"],
                                    "time_delta_hours": 1.0,
                                }
                            ],
                            "chain_summary": "2 related technology events reinforce 'China and USA expand chip controls' via actors china, usa, regions east-asia, united-states, keywords chip, controls, export through 1 corroborating link.",
                        }
                    ],
                },
                "scored_events": [],
                "intervention_candidates": [],
            }
            right = {
                "run_id": 2,
                "schema_version": "tianji.run-artifact.v1",
                "mode": "fixture",
                "generated_at": "2026-03-22T11:00:00+00:00",
                "input_summary": {"raw_item_count": 4, "normalized_event_count": 4},
                "scenario_summary": {
                    "dominant_field": "technology",
                    "risk_level": "high",
                    "headline": "right",
                    "event_groups": [
                        {
                            "group_id": "group:evt-a",
                            "headline_event_id": "evt-a",
                            "headline_title": "China and USA expand chip controls",
                            "member_event_ids": ["evt-a", "evt-b", "evt-d"],
                            "member_count": 3,
                            "dominant_field": "technology",
                            "shared_keywords": ["chip", "controls", "east-asia", "export"],
                            "shared_actors": ["china", "usa"],
                            "shared_regions": ["east-asia", "united-states"],
                            "group_score": 52.11,
                            "evidence_chain": [
                                {
                                    "from_event_id": "evt-a",
                                    "to_event_id": "evt-b",
                                    "shared_keywords": ["chip", "controls", "export"],
                                    "shared_actors": ["china", "usa"],
                                    "shared_regions": ["east-asia", "united-states"],
                                    "time_delta_hours": 1.0,
                                },
                                {
                                    "from_event_id": "evt-b",
                                    "to_event_id": "evt-d",
                                    "shared_keywords": [
                                        "chip",
                                        "controls",
                                        "east-asia",
                                        "export",
                                    ],
                                    "shared_actors": ["china", "usa"],
                                    "shared_regions": ["east-asia", "united-states"],
                                    "time_delta_hours": 2.0,
                                },
                            ],
                            "chain_summary": "3 related technology events reinforce 'China and USA expand chip controls' via actors china, usa, regions east-asia, united-states, keywords chip, controls, east-asia, export through 2 corroborating links.",
                        }
                    ],
                },
                "scored_events": [],
                "intervention_candidates": [],
            }

            left_summary = storage.build_compare_side(left)
            right_summary = storage.build_compare_side(right)
            diff = storage.build_compare_diff(left_summary, right_summary)

            evidence_diff = cast(dict[str, object], diff["top_event_group_evidence_diff"])
            self.assertTrue(evidence_diff["comparable"])
            self.assertTrue(evidence_diff["same_headline_event_id"])
            self.assertEqual(evidence_diff["member_count_delta"], 1)
            self.assertEqual(evidence_diff["left_only_member_event_ids"], [])
            self.assertEqual(evidence_diff["right_only_member_event_ids"], ["evt-d"])
            self.assertEqual(evidence_diff["shared_keywords_added"], ["east-asia"])
            self.assertEqual(evidence_diff["shared_keywords_removed"], [])
            self.assertEqual(evidence_diff["shared_actors_added"], [])
            self.assertEqual(evidence_diff["shared_regions_added"], [])
            self.assertEqual(evidence_diff["evidence_chain_link_count_delta"], 1)
            self.assertEqual(evidence_diff["evidence_chain_links_removed"], [])
            self.assertEqual(
                evidence_diff["evidence_chain_links_added"],
                [
                    "evt-b->evt-d|keywords=chip,controls,east-asia,export|actors=china,usa|regions=east-asia,united-states|delta_h=2"
                ],
            )
            self.assertTrue(evidence_diff["chain_summary_changed"])
            self.assertIn(
                "2 related technology events reinforce",
                cast(str, evidence_diff["left_chain_summary"]),
            )
            self.assertIn(
                "3 related technology events reinforce",
                cast(str, evidence_diff["right_chain_summary"]),
            )

        def test_build_compare_side_avoids_duplicate_flattened_top_group_fields(
            self,
        ) -> None:
            run_payload = {
                "run_id": 1,
                "schema_version": "tianji.run-artifact.v1",
                "mode": "fixture",
                "generated_at": "2026-03-22T10:00:00+00:00",
                "input_summary": {"raw_item_count": 2, "normalized_event_count": 2},
                "scenario_summary": {
                    "dominant_field": "technology",
                    "risk_level": "high",
                    "headline": "left",
                    "event_groups": [
                        {
                            "group_id": "group:evt-a",
                            "headline_event_id": "evt-a",
                            "headline_title": "China and USA expand chip controls",
                            "member_event_ids": ["evt-a", "evt-b"],
                            "member_count": 2,
                            "dominant_field": "technology",
                            "shared_keywords": ["chip", "controls", "export"],
                            "shared_actors": ["china", "usa"],
                            "shared_regions": ["east-asia", "united-states"],
                            "group_score": 37.89,
                            "evidence_chain": [
                                {
                                    "from_event_id": "evt-a",
                                    "to_event_id": "evt-b",
                                    "shared_keywords": ["chip", "controls", "export"],
                                    "shared_actors": ["china", "usa"],
                                    "shared_regions": ["east-asia", "united-states"],
                                    "time_delta_hours": 1.0,
                                }
                            ],
                            "chain_summary": "2 related technology events reinforce 'China and USA expand chip controls' via actors china, usa, regions east-asia, united-states, keywords chip, controls, export through 1 corroborating link.",
                        }
                    ],
                },
                "scored_events": [],
                "intervention_candidates": [],
            }

            compare_side = storage.build_compare_side(run_payload)

            self.assertIn("top_event_group", compare_side)
            self.assertNotIn("top_event_group_chain_summary", compare_side)
            self.assertNotIn("top_event_group_member_event_ids", compare_side)
            self.assertNotIn("top_event_group_shared_keywords", compare_side)
            self.assertNotIn("top_event_group_shared_actors", compare_side)
            self.assertNotIn("top_event_group_shared_regions", compare_side)
            self.assertNotIn("top_event_group_evidence_chain", compare_side)

        def test_history_compare_reports_top_score_deltas(self) -> None:
            left = {
                "run_id": 1,
                "schema_version": "tianji.run-artifact.v1",
                "mode": "fixture",
                "generated_at": "2026-03-22T10:00:00+00:00",
                "input_summary": {"raw_item_count": 3, "normalized_event_count": 3},
                "scenario_summary": {
                    "dominant_field": "technology",
                    "risk_level": "high",
                    "headline": "left",
                    "event_groups": [],
                },
                "scored_events": [
                    {
                        "event_id": "evt-a",
                        "title": "China expands chip controls",
                        "source": "fixture",
                        "link": "https://example.com/a",
                        "published_at": "Sun, 22 Mar 2026 08:00:00 GMT",
                        "dominant_field": "technology",
                        "impact_score": 14.03,
                        "field_attraction": 7.75,
                        "divergence_score": 19.58,
                        "rationale": ["Im=14.03", "Fa=7.75"],
                    }
                ],
                "intervention_candidates": [],
            }
            right = {
                "run_id": 2,
                "schema_version": "tianji.run-artifact.v1",
                "mode": "fixture",
                "generated_at": "2026-03-22T11:00:00+00:00",
                "input_summary": {"raw_item_count": 4, "normalized_event_count": 4},
                "scenario_summary": {
                    "dominant_field": "technology",
                    "risk_level": "high",
                    "headline": "right",
                    "event_groups": [],
                },
                "scored_events": [
                    {
                        "event_id": "evt-a",
                        "title": "China expands chip controls",
                        "source": "fixture",
                        "link": "https://example.com/a",
                        "published_at": "Sun, 22 Mar 2026 08:00:00 GMT",
                        "dominant_field": "technology",
                        "impact_score": 15.0,
                        "field_attraction": 8.0,
                        "divergence_score": 20.55,
                        "rationale": ["Im=15.0", "Fa=8.0"],
                    }
                ],
                "intervention_candidates": [],
            }

            left_summary = storage.build_compare_side(left)
            right_summary = storage.build_compare_side(right)
            diff = storage.build_compare_diff(left_summary, right_summary)

            self.assertFalse(diff["top_scored_event_changed"])
            self.assertTrue(diff["top_scored_event_comparable"])
            self.assertEqual(diff["left_top_scored_event_id"], "evt-a")
            self.assertEqual(diff["right_top_scored_event_id"], "evt-a")
            self.assertEqual(diff["left_top_impact_score"], 14.03)
            self.assertEqual(diff["right_top_impact_score"], 15.0)
            self.assertEqual(diff["top_impact_score_delta"], 0.97)
            self.assertEqual(diff["left_top_field_attraction"], 7.75)
            self.assertEqual(diff["right_top_field_attraction"], 8.0)
            self.assertEqual(diff["top_field_attraction_delta"], 0.25)
            self.assertEqual(diff["left_top_divergence_score"], 19.58)
            self.assertEqual(diff["right_top_divergence_score"], 20.55)
            self.assertEqual(diff["top_divergence_score_delta"], 0.97)

        def test_history_compare_keeps_top_score_deltas_for_different_top_events(
            self,
        ) -> None:
            left = {
                "run_id": 1,
                "schema_version": "tianji.run-artifact.v1",
                "mode": "fixture",
                "generated_at": "2026-03-22T10:00:00+00:00",
                "input_summary": {"raw_item_count": 3, "normalized_event_count": 3},
                "scenario_summary": {
                    "dominant_field": "technology",
                    "risk_level": "high",
                    "headline": "left",
                    "event_groups": [],
                },
                "scored_events": [
                    {
                        "event_id": "evt-a",
                        "title": "China expands chip controls",
                        "source": "fixture",
                        "link": "https://example.com/a",
                        "published_at": "Sun, 22 Mar 2026 08:00:00 GMT",
                        "dominant_field": "technology",
                        "impact_score": 14.03,
                        "field_attraction": 7.75,
                        "divergence_score": 19.58,
                        "rationale": ["Im=14.03", "Fa=7.75"],
                    }
                ],
                "intervention_candidates": [],
            }
            right = {
                "run_id": 2,
                "schema_version": "tianji.run-artifact.v1",
                "mode": "fixture",
                "generated_at": "2026-03-22T11:00:00+00:00",
                "input_summary": {"raw_item_count": 4, "normalized_event_count": 4},
                "scenario_summary": {
                    "dominant_field": "diplomacy",
                    "risk_level": "high",
                    "headline": "right",
                    "event_groups": [],
                },
                "scored_events": [
                    {
                        "event_id": "evt-b",
                        "title": "Iran diplomacy channel reopens",
                        "source": "fixture",
                        "link": "https://example.com/b",
                        "published_at": "Sun, 22 Mar 2026 09:00:00 GMT",
                        "dominant_field": "diplomacy",
                        "impact_score": 11.67,
                        "field_attraction": 6.17,
                        "divergence_score": 15.92,
                        "rationale": ["Im=11.67", "Fa=6.17"],
                    }
                ],
                "intervention_candidates": [],
            }

            left_summary = storage.build_compare_side(left)
            right_summary = storage.build_compare_side(right)
            diff = storage.build_compare_diff(left_summary, right_summary)

            self.assertTrue(diff["top_scored_event_changed"])
            self.assertFalse(diff["top_scored_event_comparable"])
            self.assertEqual(diff["left_top_scored_event_id"], "evt-a")
            self.assertEqual(diff["right_top_scored_event_id"], "evt-b")
            self.assertEqual(diff["top_impact_score_delta"], -2.36)
            self.assertEqual(diff["top_field_attraction_delta"], -1.58)
            self.assertEqual(diff["top_divergence_score_delta"], -3.66)
