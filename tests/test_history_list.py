from support import *


class HistoryListTests(unittest.TestCase):
        def test_cli_history_lists_persisted_runs(self) -> None:
            with TemporaryDirectory() as tmpdir:
                sqlite_path = Path(tmpdir) / "tianji.sqlite3"
                grouped_feed = """<?xml version="1.0" encoding="UTF-8"?>
    <rss version="2.0">
      <channel>
        <title>Grouped TianJi Feed</title>
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
      </channel>
    </rss>
    """
                grouped_fixture = Path(tmpdir) / "grouped.xml"
                grouped_fixture.write_text(grouped_feed, encoding="utf-8")
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
                            "history",
                            "--sqlite-path",
                            str(sqlite_path),
                        ]
                    )

                self.assertEqual(exit_code, 0)
                payload = json.loads(stdout.getvalue())
                detail_stdout = io.StringIO()
                with contextlib.redirect_stdout(detail_stdout):
                    exit_code = main(
                        [
                            "history-show",
                            "--sqlite-path",
                            str(sqlite_path),
                            "--run-id",
                            str(payload[0]["run_id"]),
                        ]
                    )
                self.assertEqual(exit_code, 0)
                detail_payload = json.loads(detail_stdout.getvalue())
                self.assertEqual(len(payload), 1)
                self.assertEqual(payload[0]["schema_version"], "tianji.run-artifact.v1")
                self.assertEqual(payload[0]["mode"], "fixture")
                self.assertEqual(payload[0]["raw_item_count"], 3)
                self.assertEqual(payload[0]["dominant_field"], "technology")
                self.assertEqual(payload[0]["event_group_count"], 1)
                self.assertEqual(
                    payload[0]["top_event_group_headline_event_id"],
                    detail_payload["scenario_summary"]["event_groups"][0][
                        "headline_event_id"
                    ],
                )
                self.assertEqual(payload[0]["top_event_group_dominant_field"], "technology")
                self.assertEqual(payload[0]["top_event_group_member_count"], 2)
                self.assertIsNotNone(payload[0]["top_scored_event_dominant_field"])
                self.assertGreater(cast(float, payload[0]["top_impact_score"]), 0.0)
                self.assertGreater(cast(float, payload[0]["top_field_attraction"]), 0.0)
                self.assertGreater(cast(float, payload[0]["top_divergence_score"]), 0.0)

        def test_cli_history_lists_no_group_fields_for_runs_without_groups(self) -> None:
            with TemporaryDirectory() as tmpdir:
                sqlite_path = Path(tmpdir) / "tianji.sqlite3"
                run_pipeline(
                    fixture_paths=[str(FIXTURE_PATH)],
                    fetch=False,
                    source_urls=[],
                    output_path=None,
                    sqlite_path=str(sqlite_path),
                )

                stdout = io.StringIO()
                with contextlib.redirect_stdout(stdout):
                    exit_code = main(["history", "--sqlite-path", str(sqlite_path)])

                self.assertEqual(exit_code, 0)
                payload = json.loads(stdout.getvalue())
                self.assertEqual(len(payload), 1)
                self.assertEqual(payload[0]["event_group_count"], 0)
                self.assertIsNone(payload[0]["top_event_group_headline_event_id"])
                self.assertIsNone(payload[0]["top_event_group_dominant_field"])
                self.assertIsNone(payload[0]["top_event_group_member_count"])

        def test_cli_history_filters_runs_by_min_top_divergence_score(self) -> None:
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
                            "history",
                            "--sqlite-path",
                            str(sqlite_path),
                            "--min-top-divergence-score",
                            "18",
                        ]
                    )

                self.assertEqual(exit_code, 0)
                payload = json.loads(stdout.getvalue())
                self.assertEqual(len(payload), 1)
                self.assertEqual(payload[0]["dominant_field"], "technology")
                self.assertGreaterEqual(
                    cast(float, payload[0]["top_divergence_score"]), 18.0
                )

        def test_cli_history_filters_runs_by_top_impact_and_field_attraction(self) -> None:
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
                            "history",
                            "--sqlite-path",
                            str(sqlite_path),
                            "--min-top-impact-score",
                            "12",
                            "--min-top-field-attraction",
                            "7",
                        ]
                    )

                self.assertEqual(exit_code, 0)
                payload = json.loads(stdout.getvalue())
                self.assertEqual(len(payload), 1)
                self.assertGreaterEqual(cast(float, payload[0]["top_impact_score"]), 12.0)
                self.assertGreaterEqual(
                    cast(float, payload[0]["top_field_attraction"]), 7.0
                )

        def test_cli_history_filters_runs_by_max_top_scores(self) -> None:
            with TemporaryDirectory() as tmpdir:
                sqlite_path = Path(tmpdir) / "tianji.sqlite3"
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
                            "history",
                            "--sqlite-path",
                            str(sqlite_path),
                            "--max-top-impact-score",
                            "12",
                            "--max-top-field-attraction",
                            "7",
                            "--max-top-divergence-score",
                            "16",
                        ]
                    )

                self.assertEqual(exit_code, 0)
                payload = json.loads(stdout.getvalue())
                self.assertEqual(payload, [])

        def test_cli_history_rejects_inverted_top_impact_score_range(self) -> None:
            stderr = io.StringIO()
            with contextlib.redirect_stderr(stderr):
                with self.assertRaises(SystemExit) as error:
                    main(
                        [
                            "history",
                            "--sqlite-path",
                            "runs/tianji.sqlite3",
                            "--min-top-impact-score",
                            "5",
                            "--max-top-impact-score",
                            "4",
                        ]
                    )

            self.assertEqual(error.exception.code, 2)
            self.assertIn(
                "--min-top-impact-score cannot be greater than --max-top-impact-score.",
                stderr.getvalue(),
            )

        def test_cli_history_rejects_inverted_top_field_attraction_range(self) -> None:
            stderr = io.StringIO()
            with contextlib.redirect_stderr(stderr):
                with self.assertRaises(SystemExit) as error:
                    main(
                        [
                            "history",
                            "--sqlite-path",
                            "runs/tianji.sqlite3",
                            "--min-top-field-attraction",
                            "5",
                            "--max-top-field-attraction",
                            "4",
                        ]
                    )

            self.assertEqual(error.exception.code, 2)
            self.assertIn(
                "--min-top-field-attraction cannot be greater than --max-top-field-attraction.",
                stderr.getvalue(),
            )

        def test_cli_history_rejects_inverted_top_divergence_score_range(self) -> None:
            stderr = io.StringIO()
            with contextlib.redirect_stderr(stderr):
                with self.assertRaises(SystemExit) as error:
                    main(
                        [
                            "history",
                            "--sqlite-path",
                            "runs/tianji.sqlite3",
                            "--min-top-divergence-score",
                            "5",
                            "--max-top-divergence-score",
                            "4",
                        ]
                    )

            self.assertEqual(error.exception.code, 2)
            self.assertIn(
                "--min-top-divergence-score cannot be greater than --max-top-divergence-score.",
                stderr.getvalue(),
            )

        def test_cli_history_rejects_negative_limit(self) -> None:
            stderr = io.StringIO()
            with contextlib.redirect_stderr(stderr):
                with self.assertRaises(SystemExit) as error:
                    main(
                        [
                            "history",
                            "--sqlite-path",
                            "runs/tianji.sqlite3",
                            "--limit",
                            "-1",
                        ]
                    )

            self.assertEqual(error.exception.code, 2)
            self.assertIn("--limit must be zero or greater.", stderr.getvalue())

        def test_cli_history_filters_runs_by_mode_and_dominant_field(self) -> None:
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
                            "history",
                            "--sqlite-path",
                            str(sqlite_path),
                            "--mode",
                            "fixture",
                            "--dominant-field",
                            "technology",
                        ]
                    )

                self.assertEqual(exit_code, 0)
                payload = json.loads(stdout.getvalue())
                self.assertEqual(len(payload), 1)
                self.assertEqual(payload[0]["mode"], "fixture")
                self.assertEqual(payload[0]["dominant_field"], "technology")

        def test_cli_history_filters_runs_by_top_group_field_and_group_count(self) -> None:
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
                    fixture_paths=[str(FIXTURE_PATH)],
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

                baseline_stdout = io.StringIO()
                with contextlib.redirect_stdout(baseline_stdout):
                    exit_code = main(["history", "--sqlite-path", str(sqlite_path)])
                self.assertEqual(exit_code, 0)
                baseline_payload = json.loads(baseline_stdout.getvalue())
                grouped_run = baseline_payload[0]

                stdout = io.StringIO()
                with contextlib.redirect_stdout(stdout):
                    exit_code = main(
                        [
                            "history",
                            "--sqlite-path",
                            str(sqlite_path),
                            "--top-group-dominant-field",
                            cast(str, grouped_run["top_event_group_dominant_field"]),
                            "--min-event-group-count",
                            str(grouped_run["event_group_count"]),
                        ]
                    )

                self.assertEqual(exit_code, 0)
                payload = json.loads(stdout.getvalue())
                self.assertEqual(len(payload), 1)
                self.assertEqual(payload[0]["run_id"], grouped_run["run_id"])
                self.assertEqual(
                    payload[0]["event_group_count"], grouped_run["event_group_count"]
                )
                self.assertEqual(
                    payload[0]["top_event_group_dominant_field"],
                    grouped_run["top_event_group_dominant_field"],
                )

        def test_cli_history_filters_runs_by_max_event_group_count(self) -> None:
            grouped_feed = """<?xml version="1.0" encoding="UTF-8"?>
    <rss version="2.0">
      <channel>
        <title>Grouped TianJi Feed</title>
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
                            "history",
                            "--sqlite-path",
                            str(sqlite_path),
                            "--max-event-group-count",
                            "0",
                        ]
                    )

                self.assertEqual(exit_code, 0)
                payload = json.loads(stdout.getvalue())
                self.assertEqual(len(payload), 1)
                self.assertEqual(payload[0]["event_group_count"], 0)

        def test_cli_history_rejects_invalid_event_group_count_ranges(self) -> None:
            stderr = io.StringIO()
            with contextlib.redirect_stderr(stderr):
                with self.assertRaises(SystemExit) as error:
                    main(
                        [
                            "history",
                            "--sqlite-path",
                            "runs/tianji.sqlite3",
                            "--min-event-group-count",
                            "2",
                            "--max-event-group-count",
                            "1",
                        ]
                    )

            self.assertEqual(error.exception.code, 2)
            self.assertIn(
                "--min-event-group-count cannot be greater than --max-event-group-count.",
                stderr.getvalue(),
            )

            stderr = io.StringIO()
            with contextlib.redirect_stderr(stderr):
                with self.assertRaises(SystemExit) as error:
                    main(
                        [
                            "history",
                            "--sqlite-path",
                            "runs/tianji.sqlite3",
                            "--min-event-group-count",
                            "-1",
                        ]
                    )

            self.assertEqual(error.exception.code, 2)
            self.assertIn(
                "--min-event-group-count must be zero or greater.",
                stderr.getvalue(),
            )

            stderr = io.StringIO()
            with contextlib.redirect_stderr(stderr):
                with self.assertRaises(SystemExit) as error:
                    main(
                        [
                            "history",
                            "--sqlite-path",
                            "runs/tianji.sqlite3",
                            "--max-event-group-count",
                            "-1",
                        ]
                    )

            self.assertEqual(error.exception.code, 2)
            self.assertIn(
                "--max-event-group-count must be zero or greater.",
                stderr.getvalue(),
            )

        def test_filter_run_list_items_applies_event_group_filters(self) -> None:
            items = [
                {
                    "run_id": 1,
                    "mode": "fixture",
                    "dominant_field": "technology",
                    "risk_level": "high",
                    "generated_at": "2026-03-22T10:00:00+00:00",
                    "event_group_count": 1,
                    "top_event_group_dominant_field": "technology",
                },
                {
                    "run_id": 2,
                    "mode": "fixture",
                    "dominant_field": "diplomacy",
                    "risk_level": "medium",
                    "generated_at": "2026-03-22T11:00:00+00:00",
                    "event_group_count": 2,
                    "top_event_group_dominant_field": "technology",
                },
                {
                    "run_id": 3,
                    "mode": "fixture",
                    "dominant_field": "uncategorized",
                    "risk_level": "low",
                    "generated_at": "2026-03-22T12:00:00+00:00",
                    "event_group_count": 0,
                    "top_event_group_dominant_field": None,
                },
            ]

            filtered = storage.filter_run_list_items(
                items,
                mode=None,
                dominant_field=None,
                risk_level=None,
                since=None,
                until=None,
                top_group_dominant_field="technology",
                min_event_group_count=2,
                max_event_group_count=2,
            )

            self.assertEqual([item["run_id"] for item in filtered], [2])

        def test_cli_history_filters_runs_by_risk_level(self) -> None:
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
                            "history",
                            "--sqlite-path",
                            str(sqlite_path),
                            "--risk-level",
                            "low",
                        ]
                    )

                self.assertEqual(exit_code, 0)
                payload = json.loads(stdout.getvalue())
                self.assertEqual(len(payload), 1)
                self.assertEqual(payload[0]["risk_level"], "low")

        def test_cli_history_filters_runs_by_since_timestamp(self) -> None:
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

                baseline_stdout = io.StringIO()
                with contextlib.redirect_stdout(baseline_stdout):
                    exit_code = main(["history", "--sqlite-path", str(sqlite_path)])
                self.assertEqual(exit_code, 0)
                baseline_payload = json.loads(baseline_stdout.getvalue())

                stdout = io.StringIO()
                with contextlib.redirect_stdout(stdout):
                    exit_code = main(
                        [
                            "history",
                            "--sqlite-path",
                            str(sqlite_path),
                            "--since",
                            baseline_payload[0]["generated_at"],
                        ]
                    )

                self.assertEqual(exit_code, 0)
                payload = json.loads(stdout.getvalue())
                self.assertEqual(len(payload), 1)
                self.assertEqual(payload[0]["run_id"], baseline_payload[0]["run_id"])

        def test_cli_history_filters_runs_by_until_timestamp(self) -> None:
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

                baseline_stdout = io.StringIO()
                with contextlib.redirect_stdout(baseline_stdout):
                    exit_code = main(["history", "--sqlite-path", str(sqlite_path)])
                self.assertEqual(exit_code, 0)
                baseline_payload = json.loads(baseline_stdout.getvalue())

                stdout = io.StringIO()
                with contextlib.redirect_stdout(stdout):
                    exit_code = main(
                        [
                            "history",
                            "--sqlite-path",
                            str(sqlite_path),
                            "--until",
                            baseline_payload[1]["generated_at"],
                        ]
                    )

                self.assertEqual(exit_code, 0)
                payload = json.loads(stdout.getvalue())
                self.assertEqual(len(payload), 1)
                self.assertEqual(payload[0]["run_id"], baseline_payload[1]["run_id"])

        def test_cli_history_score_filters_exclude_runs_without_scored_events(self) -> None:
            with TemporaryDirectory() as tmpdir:
                sqlite_path = Path(tmpdir) / "tianji.sqlite3"
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
                            "history",
                            "--sqlite-path",
                            str(sqlite_path),
                            "--min-top-divergence-score",
                            "1",
                        ]
                    )

                self.assertEqual(exit_code, 0)
                payload = json.loads(stdout.getvalue())
                self.assertEqual(payload, [])

                baseline_stdout = io.StringIO()
                with contextlib.redirect_stdout(baseline_stdout):
                    exit_code = main(["history", "--sqlite-path", str(sqlite_path)])
                self.assertEqual(exit_code, 0)
                baseline_payload = json.loads(baseline_stdout.getvalue())
                self.assertEqual(len(baseline_payload), 1)
                self.assertIsNone(baseline_payload[0]["top_scored_event_id"])
                self.assertIsNone(baseline_payload[0]["top_impact_score"])
                self.assertIsNone(baseline_payload[0]["top_field_attraction"])
                self.assertIsNone(baseline_payload[0]["top_divergence_score"])

        def test_cli_history_applies_filters_before_limit(self) -> None:
            with TemporaryDirectory() as tmpdir:
                sqlite_path = Path(tmpdir) / "tianji.sqlite3"

                empty_feed = """<?xml version="1.0" encoding="UTF-8"?>
    <rss version="2.0"><channel><title>Empty TianJi Feed</title></channel></rss>
    """
                empty_fixture = Path(tmpdir) / "empty.xml"
                empty_fixture.write_text(empty_feed, encoding="utf-8")

                run_pipeline(
                    fixture_paths=[str(FIXTURE_PATH)],
                    fetch=False,
                    source_urls=[],
                    output_path=None,
                    sqlite_path=str(sqlite_path),
                )
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
                            "history",
                            "--sqlite-path",
                            str(sqlite_path),
                            "--limit",
                            "1",
                            "--min-top-divergence-score",
                            "18",
                        ]
                    )

                self.assertEqual(exit_code, 0)
                payload = json.loads(stdout.getvalue())
                self.assertEqual(len(payload), 1)
                self.assertEqual(payload[0]["dominant_field"], "technology")
                self.assertEqual(payload[0]["run_id"], 1)

        def test_filter_run_list_items_applies_top_score_thresholds(self) -> None:
            items = [
                {
                    "run_id": 1,
                    "mode": "fixture",
                    "dominant_field": "technology",
                    "risk_level": "high",
                    "generated_at": "2026-03-22T10:00:00+00:00",
                    "top_impact_score": 14.03,
                    "top_field_attraction": 7.75,
                    "top_divergence_score": 19.58,
                },
                {
                    "run_id": 2,
                    "mode": "fixture",
                    "dominant_field": "uncategorized",
                    "risk_level": "low",
                    "generated_at": "2026-03-22T11:00:00+00:00",
                    "top_impact_score": None,
                    "top_field_attraction": None,
                    "top_divergence_score": None,
                },
                {
                    "run_id": 3,
                    "mode": "fixture",
                    "dominant_field": "diplomacy",
                    "risk_level": "medium",
                    "generated_at": "2026-03-22T12:00:00+00:00",
                    "top_impact_score": 6.5,
                    "top_field_attraction": 3.1,
                    "top_divergence_score": 8.45,
                },
            ]

            filtered = storage.filter_run_list_items(
                items,
                mode=None,
                dominant_field=None,
                risk_level=None,
                since=None,
                until=None,
                min_top_impact_score=6.0,
                max_top_divergence_score=10.0,
                min_top_field_attraction=3.0,
            )

            self.assertEqual([item["run_id"] for item in filtered], [3])
