from support import *


class HistoryShowTests(unittest.TestCase):
        def test_filter_scored_event_details_applies_thresholds_and_limit(self) -> None:
            scored_events = [
                {
                    "event_id": "evt-a",
                    "dominant_field": "technology",
                    "impact_score": 14.03,
                    "field_attraction": 7.75,
                    "divergence_score": 19.58,
                },
                {
                    "event_id": "evt-b",
                    "dominant_field": "diplomacy",
                    "impact_score": 11.67,
                    "field_attraction": 6.17,
                    "divergence_score": 15.92,
                },
                {
                    "event_id": "evt-c",
                    "dominant_field": "conflict",
                    "impact_score": 15.65,
                    "field_attraction": 3.6,
                    "divergence_score": 15.03,
                },
            ]

            filtered = storage.filter_scored_event_details(
                scored_events,
                dominant_field=None,
                min_impact_score=11.0,
                max_impact_score=15.0,
                min_field_attraction=6.0,
                max_field_attraction=None,
                min_divergence_score=15.0,
                max_divergence_score=16.0,
                limit_scored_events=1,
            )

            self.assertEqual([event["event_id"] for event in filtered], ["evt-b"])

        def test_filter_intervention_candidate_details_can_align_with_visible_events(
            self,
        ) -> None:
            intervention_candidates = [
                {"event_id": "evt-a", "priority": 1},
                {"event_id": "evt-b", "priority": 2},
                {"event_id": "evt-c", "priority": 3},
            ]

            filtered = storage.filter_intervention_candidate_details(
                intervention_candidates,
                visible_scored_event_ids={"evt-b"},
                only_matching_interventions=True,
            )

            self.assertEqual(filtered, [{"event_id": "evt-b", "priority": 2}])

        def test_filter_intervention_candidate_details_is_noop_without_alignment(
            self,
        ) -> None:
            intervention_candidates = [
                {"event_id": "evt-a", "priority": 1},
                {"event_id": "evt-b", "priority": 2},
            ]

            filtered = storage.filter_intervention_candidate_details(
                intervention_candidates,
                visible_scored_event_ids={"evt-b"},
                only_matching_interventions=False,
            )

            self.assertEqual(filtered, intervention_candidates)

        def test_filter_event_group_details_applies_field_filter_and_limit(self) -> None:
            event_groups: list[dict[str, object]] = [
                {"group_id": "group:evt-a", "dominant_field": "technology"},
                {"group_id": "group:evt-b", "dominant_field": "diplomacy"},
                {"group_id": "group:evt-c", "dominant_field": "technology"},
            ]

            filtered = storage.filter_event_group_details(
                event_groups,
                dominant_field="technology",
                limit_event_groups=1,
            )

            self.assertEqual(
                filtered,
                [{"group_id": "group:evt-a", "dominant_field": "technology"}],
            )

        def test_cli_history_show_reads_single_persisted_run(self) -> None:
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
                            "history-show",
                            "--sqlite-path",
                            str(sqlite_path),
                            "--run-id",
                            "1",
                        ]
                    )

                self.assertEqual(exit_code, 0)
                payload = json.loads(stdout.getvalue())
                self.assertEqual(payload["run_id"], 1)
                self.assertEqual(payload["schema_version"], "tianji.run-artifact.v1")
                self.assertEqual(payload["mode"], "fixture")
                self.assertEqual(payload["input_summary"]["raw_item_count"], 3)
                self.assertEqual(
                    payload["scenario_summary"]["dominant_field"], "technology"
                )
                self.assertEqual(len(payload["scored_events"]), 3)
                self.assertEqual(len(payload["intervention_candidates"]), 3)
                self.assertEqual(
                    payload["scored_events"][0]["dominant_field"],
                    "technology",
                )
                self.assertEqual(
                    payload["intervention_candidates"][0]["intervention_type"],
                    "capability-control",
                )

        def test_cli_history_show_filters_scored_events_by_scores_and_field(self) -> None:
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
                            "history-show",
                            "--sqlite-path",
                            str(sqlite_path),
                            "--run-id",
                            "1",
                            "--dominant-field",
                            "diplomacy",
                            "--min-impact-score",
                            "11",
                            "--min-field-attraction",
                            "6",
                            "--min-divergence-score",
                            "15",
                        ]
                    )

                self.assertEqual(exit_code, 0)
                payload = json.loads(stdout.getvalue())
                self.assertEqual(len(payload["scored_events"]), 1)
                self.assertEqual(payload["scored_events"][0]["dominant_field"], "diplomacy")
                self.assertGreaterEqual(
                    cast(float, payload["scored_events"][0]["impact_score"]), 11.0
                )
                self.assertGreaterEqual(
                    cast(float, payload["scored_events"][0]["field_attraction"]), 6.0
                )
                self.assertGreaterEqual(
                    cast(float, payload["scored_events"][0]["divergence_score"]), 15.0
                )
                self.assertEqual(len(payload["intervention_candidates"]), 3)

        def test_cli_history_show_rejects_non_positive_run_id(self) -> None:
            stderr = io.StringIO()
            with contextlib.redirect_stderr(stderr):
                with self.assertRaises(SystemExit) as error:
                    main(
                        [
                            "history-show",
                            "--sqlite-path",
                            "runs/tianji.sqlite3",
                            "--run-id",
                            "0",
                        ]
                    )

            self.assertEqual(error.exception.code, 2)
            self.assertIn("--run-id must be greater than zero.", stderr.getvalue())

        def test_cli_history_show_limits_scored_events(self) -> None:
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
                            "history-show",
                            "--sqlite-path",
                            str(sqlite_path),
                            "--run-id",
                            "1",
                            "--limit-scored-events",
                            "1",
                        ]
                    )

                self.assertEqual(exit_code, 0)
                payload = json.loads(stdout.getvalue())
                self.assertEqual(len(payload["scored_events"]), 1)
                self.assertEqual(
                    payload["scored_events"][0]["dominant_field"], "technology"
                )

        def test_cli_history_show_can_return_empty_scored_event_selection(self) -> None:
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
                            "history-show",
                            "--sqlite-path",
                            str(sqlite_path),
                            "--run-id",
                            "1",
                            "--dominant-field",
                            "uncategorized",
                        ]
                    )

                self.assertEqual(exit_code, 0)
                payload = json.loads(stdout.getvalue())
                self.assertEqual(payload["scored_events"], [])
                self.assertEqual(len(payload["intervention_candidates"]), 3)

        def test_cli_history_show_can_limit_interventions_to_visible_scored_events(
            self,
        ) -> None:
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
                            "history-show",
                            "--sqlite-path",
                            str(sqlite_path),
                            "--run-id",
                            "1",
                            "--dominant-field",
                            "diplomacy",
                            "--only-matching-interventions",
                        ]
                    )

                self.assertEqual(exit_code, 0)
                payload = json.loads(stdout.getvalue())
                self.assertEqual(len(payload["scored_events"]), 1)
                self.assertEqual(len(payload["intervention_candidates"]), 1)
                self.assertEqual(
                    payload["intervention_candidates"][0]["event_id"],
                    payload["scored_events"][0]["event_id"],
                )

        def test_cli_history_show_can_return_empty_matching_interventions(self) -> None:
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
                            "history-show",
                            "--sqlite-path",
                            str(sqlite_path),
                            "--run-id",
                            "1",
                            "--dominant-field",
                            "uncategorized",
                            "--only-matching-interventions",
                        ]
                    )

                self.assertEqual(exit_code, 0)
                payload = json.loads(stdout.getvalue())
                self.assertEqual(payload["scored_events"], [])
                self.assertEqual(payload["intervention_candidates"], [])

        def test_cli_history_show_aligns_interventions_after_limit(self) -> None:
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
                            "history-show",
                            "--sqlite-path",
                            str(sqlite_path),
                            "--run-id",
                            "1",
                            "--limit-scored-events",
                            "1",
                            "--only-matching-interventions",
                        ]
                    )

                self.assertEqual(exit_code, 0)
                payload = json.loads(stdout.getvalue())
                self.assertEqual(len(payload["scored_events"]), 1)
                self.assertEqual(len(payload["intervention_candidates"]), 1)
                self.assertEqual(
                    payload["intervention_candidates"][0]["event_id"],
                    payload["scored_events"][0]["event_id"],
                )

        def test_cli_history_show_filters_scored_events_by_max_thresholds(self) -> None:
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
                            "history-show",
                            "--sqlite-path",
                            str(sqlite_path),
                            "--run-id",
                            "1",
                            "--max-impact-score",
                            "12.5",
                            "--max-field-attraction",
                            "6.5",
                            "--max-divergence-score",
                            "16.5",
                        ]
                    )

                self.assertEqual(exit_code, 0)
                payload = json.loads(stdout.getvalue())
                self.assertEqual(len(payload["scored_events"]), 1)
                self.assertEqual(payload["scored_events"][0]["dominant_field"], "diplomacy")

        def test_cli_history_show_applies_filters_before_limit(self) -> None:
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
                            "history-show",
                            "--sqlite-path",
                            str(sqlite_path),
                            "--run-id",
                            "1",
                            "--dominant-field",
                            "diplomacy",
                            "--limit-scored-events",
                            "1",
                        ]
                    )

                self.assertEqual(exit_code, 0)
                payload = json.loads(stdout.getvalue())
                self.assertEqual(len(payload["scored_events"]), 1)
                self.assertEqual(payload["scored_events"][0]["dominant_field"], "diplomacy")

        def test_cli_history_show_rejects_negative_scored_event_limit(self) -> None:
            stderr = io.StringIO()
            with contextlib.redirect_stderr(stderr):
                with self.assertRaises(SystemExit) as error:
                    main(
                        [
                            "history-show",
                            "--sqlite-path",
                            "runs/tianji.sqlite3",
                            "--run-id",
                            "1",
                            "--limit-scored-events",
                            "-1",
                        ]
                    )

            self.assertEqual(error.exception.code, 2)
            self.assertIn(
                "--limit-scored-events must be zero or greater.", stderr.getvalue()
            )

        def test_cli_history_show_rejects_inverted_impact_score_range(self) -> None:
            stderr = io.StringIO()
            with contextlib.redirect_stderr(stderr):
                with self.assertRaises(SystemExit) as error:
                    main(
                        [
                            "history-show",
                            "--sqlite-path",
                            "runs/tianji.sqlite3",
                            "--run-id",
                            "1",
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

        def test_cli_history_show_rejects_inverted_field_attraction_range(self) -> None:
            stderr = io.StringIO()
            with contextlib.redirect_stderr(stderr):
                with self.assertRaises(SystemExit) as error:
                    main(
                        [
                            "history-show",
                            "--sqlite-path",
                            "runs/tianji.sqlite3",
                            "--run-id",
                            "1",
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

        def test_cli_history_show_rejects_inverted_divergence_score_range(self) -> None:
            stderr = io.StringIO()
            with contextlib.redirect_stderr(stderr):
                with self.assertRaises(SystemExit) as error:
                    main(
                        [
                            "history-show",
                            "--sqlite-path",
                            "runs/tianji.sqlite3",
                            "--run-id",
                            "1",
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

        def test_cli_history_show_preserves_group_evidence_chain_metadata(self) -> None:
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
                fixture_path = Path(tmpdir) / "grouped.xml"
                fixture_path.write_text(grouped_feed, encoding="utf-8")

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
                            "history-show",
                            "--sqlite-path",
                            str(sqlite_path),
                            "--run-id",
                            "1",
                        ]
                    )

                self.assertEqual(exit_code, 0)
                payload = json.loads(stdout.getvalue())
                event_groups = payload["scenario_summary"]["event_groups"]
                self.assertEqual(len(event_groups), 1)
                self.assertEqual(len(event_groups[0]["evidence_chain"]), 1)
                self.assertEqual(
                    event_groups[0]["evidence_chain"][0]["time_delta_hours"], 1.0
                )
                self.assertEqual(
                    event_groups[0]["causal_ordered_event_ids"],
                    [
                        event_groups[0]["evidence_chain"][0]["from_event_id"],
                        event_groups[0]["evidence_chain"][0]["to_event_id"],
                    ],
                )
                self.assertEqual(event_groups[0]["causal_span_hours"], 1.0)
                self.assertIn("capability-race cluster", event_groups[0]["causal_summary"])
                self.assertIn(
                    "2 related technology events reinforce",
                    event_groups[0]["chain_summary"],
                )
                self.assertIn(
                    "Evidence chain:", payload["intervention_candidates"][0]["reason"]
                )

        def test_cli_history_show_can_filter_and_limit_event_groups(self) -> None:
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
                fixture_path = Path(tmpdir) / "grouped.xml"
                fixture_path.write_text(grouped_feed, encoding="utf-8")

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
                            "history-show",
                            "--sqlite-path",
                            str(sqlite_path),
                            "--run-id",
                            "1",
                            "--group-dominant-field",
                            "diplomacy",
                            "--limit-event-groups",
                            "1",
                        ]
                    )

                self.assertEqual(exit_code, 0)
                payload = json.loads(stdout.getvalue())
                event_groups = payload["scenario_summary"]["event_groups"]
                self.assertEqual(len(event_groups), 1)
                self.assertEqual(event_groups[0]["dominant_field"], "diplomacy")
                self.assertEqual(event_groups[0]["member_count"], 2)

        def test_cli_history_show_projects_scored_events_and_event_groups_independently(
            self,
        ) -> None:
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
                fixture_path = Path(tmpdir) / "grouped.xml"
                fixture_path.write_text(grouped_feed, encoding="utf-8")

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
                            "history-show",
                            "--sqlite-path",
                            str(sqlite_path),
                            "--run-id",
                            "1",
                            "--dominant-field",
                            "technology",
                            "--limit-scored-events",
                            "1",
                            "--group-dominant-field",
                            "diplomacy",
                            "--limit-event-groups",
                            "1",
                        ]
                    )

                self.assertEqual(exit_code, 0)
                payload = json.loads(stdout.getvalue())
                self.assertEqual(len(payload["scored_events"]), 1)
                self.assertEqual(
                    payload["scored_events"][0]["dominant_field"], "technology"
                )
                self.assertEqual(len(payload["scenario_summary"]["event_groups"]), 1)
                self.assertEqual(
                    payload["scenario_summary"]["event_groups"][0]["dominant_field"],
                    "diplomacy",
                )

        def test_cli_history_show_rejects_negative_event_group_limit(self) -> None:
            stderr = io.StringIO()
            with contextlib.redirect_stderr(stderr):
                with self.assertRaises(SystemExit) as error:
                    main(
                        [
                            "history-show",
                            "--sqlite-path",
                            "runs/tianji.sqlite3",
                            "--run-id",
                            "1",
                            "--limit-event-groups",
                            "-1",
                        ]
                    )

            self.assertEqual(error.exception.code, 2)
            self.assertIn(
                "--limit-event-groups must be zero or greater.", stderr.getvalue()
            )

        def test_cli_history_show_can_read_latest_run(self) -> None:
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
                            "history-show",
                            "--sqlite-path",
                            str(sqlite_path),
                            "--latest",
                        ]
                    )

                self.assertEqual(exit_code, 0)
                payload = json.loads(stdout.getvalue())
                self.assertEqual(payload["run_id"], 2)
                self.assertEqual(
                    payload["scenario_summary"]["dominant_field"], "uncategorized"
                )

        def test_cli_history_show_can_read_previous_run(self) -> None:
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
                            "history-show",
                            "--sqlite-path",
                            str(sqlite_path),
                            "--run-id",
                            "3",
                            "--previous",
                        ]
                    )

                self.assertEqual(exit_code, 0)
                payload = json.loads(stdout.getvalue())
                self.assertEqual(payload["run_id"], 2)

        def test_cli_history_show_can_read_next_run(self) -> None:
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
                            "history-show",
                            "--sqlite-path",
                            str(sqlite_path),
                            "--run-id",
                            "1",
                            "--next",
                        ]
                    )

                self.assertEqual(exit_code, 0)
                payload = json.loads(stdout.getvalue())
                self.assertEqual(payload["run_id"], 2)
