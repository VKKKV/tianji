from support import *


class PipelineIntegrationTests(unittest.TestCase):
    def test_run_artifact_contract_fixture_freezes_v1_vocabulary(self) -> None:
        artifact_fixture = cast(
            dict[str, object], load_contract_fixture("run_artifact_v1.json")
        )

        artifact = run_pipeline(
            fixture_paths=[str(FIXTURE_PATH)],
            fetch=False,
            source_urls=[],
            output_path=None,
        )
        payload = artifact.to_dict()

        self.assertEqual(set(payload), set(artifact_fixture))
        self.assertEqual(
            set(cast(dict[str, object], payload["input_summary"])),
            set(cast(dict[str, object], artifact_fixture["input_summary"])),
        )
        self.assertEqual(
            set(cast(dict[str, object], payload["scenario_summary"])),
            set(cast(dict[str, object], artifact_fixture["scenario_summary"])),
        )
        scored_events = cast(list[dict[str, object]], payload["scored_events"])
        scored_event_fixture = cast(
            list[dict[str, object]], artifact_fixture["scored_events"]
        )
        self.assertEqual(set(scored_events[0]), set(scored_event_fixture[0]))
        intervention_candidates = cast(
            list[dict[str, object]], payload["intervention_candidates"]
        )
        intervention_fixture = cast(
            list[dict[str, object]], artifact_fixture["intervention_candidates"]
        )
        self.assertEqual(
            set(intervention_candidates[0]),
            set(intervention_fixture[0]),
        )
        self.assertEqual(payload["schema_version"], artifact_fixture["schema_version"])
        self.assertEqual(payload["mode"], artifact_fixture["mode"])
        self.assertEqual(
            cast(dict[str, object], payload["input_summary"])["fetch_policy"],
            cast(dict[str, object], artifact_fixture["input_summary"])["fetch_policy"],
        )

    def test_fixture_pipeline_writes_expected_artifact(self) -> None:
        with TemporaryDirectory() as tmpdir:
            output_path = Path(tmpdir) / "report.json"
            artifact = run_pipeline(
                fixture_paths=[str(FIXTURE_PATH)],
                fetch=False,
                source_urls=[],
                output_path=str(output_path),
            )

            self.assertTrue(output_path.exists())
            payload = json.loads(output_path.read_text(encoding="utf-8"))
            self.assertEqual(payload["schema_version"], "tianji.run-artifact.v1")
            self.assertEqual(payload["mode"], "fixture")
            self.assertEqual(payload["input_summary"]["raw_item_count"], 3)
            self.assertGreater(len(payload["scored_events"]), 0)
            self.assertGreater(len(payload["intervention_candidates"]), 0)
            self.assertIn("headline", payload["scenario_summary"])
            self.assertEqual(len(artifact.scored_events), 3)
            self.assertEqual(
                artifact.to_dict()["schema_version"], "tianji.run-artifact.v1"
            )

    def test_fetch_pipeline_can_read_from_local_http_server(self) -> None:
        fixture_bytes = FIXTURE_PATH.read_bytes()

        class Handler(BaseHTTPRequestHandler):
            def do_GET(self) -> None:  # noqa: N802
                self.send_response(200)
                self.send_header("Content-Type", "application/rss+xml")
                self.send_header("Content-Length", str(len(fixture_bytes)))
                self.end_headers()
                self.wfile.write(fixture_bytes)

            def log_message(self, format: str, *args: object) -> None:
                return

        server = ThreadingHTTPServer(("127.0.0.1", 0), Handler)
        thread = threading.Thread(target=server.serve_forever, daemon=True)
        thread.start()
        self.addCleanup(server.shutdown)
        self.addCleanup(server.server_close)
        url = f"http://127.0.0.1:{server.server_port}/feed.xml"

        with TemporaryDirectory() as tmpdir:
            output_path = Path(tmpdir) / "fetched-report.json"
            artifact = run_pipeline(
                fixture_paths=[],
                fetch=True,
                source_urls=[url],
                output_path=str(output_path),
            )

            self.assertEqual(artifact.mode, "fetch")
            self.assertEqual(artifact.input_summary["raw_item_count"], 3)
            self.assertTrue(output_path.exists())

    def test_pipeline_parses_atom_feed_deterministically(self) -> None:
        atom_feed = """<?xml version="1.0" encoding="utf-8"?>
    <feed xmlns="http://www.w3.org/2005/Atom">
      <title>TianJi Atom Feed</title>
      <entry>
        <title>EU opens new negotiation channel after cyber dispute</title>
        <link href="https://example.com/eu-negotiation" />
        <updated>2026-03-22T10:00:00Z</updated>
        <content>European Union officials opened a new negotiation channel after a cyber dispute with Beijing.</content>
      </entry>
      <entry>
        <title> </title>
        <link href="https://example.com/ignored" />
        <updated>2026-03-22T11:00:00Z</updated>
        <summary>This entry should be ignored because it has no usable title.</summary>
      </entry>
    </feed>
    """

        with TemporaryDirectory() as tmpdir:
            fixture_path = Path(tmpdir) / "sample_atom.xml"
            fixture_path.write_text(atom_feed, encoding="utf-8")

            artifact = run_pipeline(
                fixture_paths=[str(fixture_path)],
                fetch=False,
                source_urls=[],
                output_path=None,
            )

        self.assertEqual(artifact.mode, "fixture")
        self.assertEqual(artifact.input_summary["raw_item_count"], 1)
        self.assertEqual(len(artifact.scored_events), 1)
        self.assertEqual(
            artifact.scored_events[0].title,
            "EU opens new negotiation channel after cyber dispute",
        )

    def test_pipeline_emits_empty_artifact_for_empty_rss_feed(self) -> None:
        empty_feed = """<?xml version="1.0" encoding="UTF-8"?>
    <rss version="2.0">
      <channel>
        <title>Empty TianJi Feed</title>
      </channel>
    </rss>
    """

        with TemporaryDirectory() as tmpdir:
            fixture_path = Path(tmpdir) / "empty_feed.xml"
            fixture_path.write_text(empty_feed, encoding="utf-8")

            artifact = run_pipeline(
                fixture_paths=[str(fixture_path)],
                fetch=False,
                source_urls=[],
                output_path=None,
            )

        self.assertEqual(artifact.mode, "fixture")
        self.assertEqual(artifact.input_summary["raw_item_count"], 0)
        self.assertEqual(artifact.input_summary["normalized_event_count"], 0)
        self.assertEqual(artifact.input_summary["sources"], [])
        self.assertEqual(
            artifact.scenario_summary["headline"],
            "No high-signal events were available for inference.",
        )
        self.assertEqual(artifact.scenario_summary["dominant_field"], "uncategorized")
        self.assertEqual(artifact.scenario_summary["risk_level"], "low")
        self.assertEqual(artifact.scored_events, [])
        self.assertEqual(artifact.intervention_candidates, [])

    def test_pipeline_emits_empty_artifact_for_empty_atom_feed(self) -> None:
        empty_feed = """<?xml version="1.0" encoding="utf-8"?>
    <feed xmlns="http://www.w3.org/2005/Atom">
      <title>Empty Atom Feed</title>
    </feed>
    """

        with TemporaryDirectory() as tmpdir:
            fixture_path = Path(tmpdir) / "empty_atom.xml"
            fixture_path.write_text(empty_feed, encoding="utf-8")

            artifact = run_pipeline(
                fixture_paths=[str(fixture_path)],
                fetch=False,
                source_urls=[],
                output_path=None,
            )

        self.assertEqual(artifact.mode, "fixture")
        self.assertEqual(artifact.input_summary["raw_item_count"], 0)
        self.assertEqual(artifact.scored_events, [])
        self.assertEqual(artifact.intervention_candidates, [])
        self.assertEqual(artifact.scenario_summary["event_groups"], [])

    def test_pipeline_marks_mixed_fixture_and_fetch_mode(self) -> None:
        fixture_bytes = FIXTURE_PATH.read_bytes()

        class Handler(BaseHTTPRequestHandler):
            def do_GET(self) -> None:  # noqa: N802
                self.send_response(200)
                self.send_header("Content-Type", "application/rss+xml")
                self.send_header("Content-Length", str(len(fixture_bytes)))
                self.end_headers()
                self.wfile.write(fixture_bytes)

            def log_message(self, format: str, *args: object) -> None:
                return

        server = ThreadingHTTPServer(("127.0.0.1", 0), Handler)
        thread = threading.Thread(target=server.serve_forever, daemon=True)
        thread.start()
        self.addCleanup(server.shutdown)
        self.addCleanup(server.server_close)
        url = f"http://127.0.0.1:{server.server_port}/feed.xml"

        artifact = run_pipeline(
            fixture_paths=[str(FIXTURE_PATH)],
            fetch=True,
            source_urls=[url],
            output_path=None,
        )

        self.assertEqual(artifact.mode, "fetch+fixture")
        self.assertEqual(artifact.input_summary["raw_item_count"], 6)

    def test_fixture_pipeline_can_persist_run_to_sqlite(self) -> None:
        with TemporaryDirectory() as tmpdir:
            output_path = Path(tmpdir) / "report.json"
            sqlite_path = Path(tmpdir) / "tianji.sqlite3"

            artifact = run_pipeline(
                fixture_paths=[str(FIXTURE_PATH)],
                fetch=False,
                source_urls=[],
                output_path=str(output_path),
                sqlite_path=str(sqlite_path),
            )

            self.assertTrue(sqlite_path.exists())
            self.assertEqual(artifact.input_summary["raw_item_count"], 3)

            with sqlite3.connect(sqlite_path) as connection:
                run_count = connection.execute("SELECT COUNT(*) FROM runs").fetchone()[
                    0
                ]
                raw_item_count = connection.execute(
                    "SELECT COUNT(*) FROM raw_items"
                ).fetchone()[0]
                normalized_count = connection.execute(
                    "SELECT COUNT(*) FROM normalized_events"
                ).fetchone()[0]
                scored_count = connection.execute(
                    "SELECT COUNT(*) FROM scored_events"
                ).fetchone()[0]
                intervention_count = connection.execute(
                    "SELECT COUNT(*) FROM intervention_candidates"
                ).fetchone()[0]
                schema_version = connection.execute(
                    "SELECT schema_version FROM runs"
                ).fetchone()[0]

            self.assertEqual(run_count, 1)
            self.assertEqual(raw_item_count, 3)
            self.assertEqual(normalized_count, 3)
            self.assertEqual(scored_count, 3)
            self.assertEqual(intervention_count, 3)
            self.assertEqual(schema_version, "tianji.run-artifact.v1")

    def test_persistence_reuses_canonical_source_item_rows_for_identical_reruns(
        self,
    ) -> None:
        with TemporaryDirectory() as tmpdir:
            sqlite_path = Path(tmpdir) / "tianji.sqlite3"

            first_artifact = run_pipeline(
                fixture_paths=[str(FIXTURE_PATH)],
                fetch=False,
                source_urls=[],
                output_path=None,
                sqlite_path=str(sqlite_path),
            )
            second_artifact = run_pipeline(
                fixture_paths=[str(FIXTURE_PATH)],
                fetch=False,
                source_urls=[],
                output_path=None,
                sqlite_path=str(sqlite_path),
            )

            self.assertEqual(
                first_artifact.to_dict()["input_summary"],
                second_artifact.to_dict()["input_summary"],
            )

            with sqlite3.connect(sqlite_path) as connection:
                run_count = connection.execute("SELECT COUNT(*) FROM runs").fetchone()[
                    0
                ]
                raw_item_count = connection.execute(
                    "SELECT COUNT(*) FROM raw_items"
                ).fetchone()[0]
                normalized_count = connection.execute(
                    "SELECT COUNT(*) FROM normalized_events"
                ).fetchone()[0]
                canonical_count = connection.execute(
                    "SELECT COUNT(*) FROM source_items"
                ).fetchone()[0]
                identity_pairs = connection.execute(
                    """
                        SELECT entry_identity_hash, content_hash, COUNT(*)
                        FROM source_items
                        GROUP BY entry_identity_hash, content_hash
                        """
                ).fetchall()
                raw_link_count = connection.execute(
                    "SELECT COUNT(*) FROM raw_items WHERE canonical_source_item_id IS NOT NULL"
                ).fetchone()[0]
                normalized_link_count = connection.execute(
                    "SELECT COUNT(*) FROM normalized_events WHERE canonical_source_item_id IS NOT NULL"
                ).fetchone()[0]

            self.assertEqual(run_count, 2)
            self.assertEqual(raw_item_count, 6)
            self.assertEqual(normalized_count, 6)
            self.assertEqual(canonical_count, 3)
            self.assertTrue(all(row[2] == 1 for row in identity_pairs))
            self.assertEqual(raw_link_count, 6)
            self.assertEqual(normalized_link_count, 6)
            self.assertEqual(second_artifact.input_summary["raw_item_count"], 3)

            history_payload = storage.list_runs(sqlite_path=str(sqlite_path))
            self.assertEqual(len(history_payload), 2)
            self.assertEqual(history_payload[0]["raw_item_count"], 3)
            self.assertEqual(history_payload[1]["raw_item_count"], 3)

    def test_each_successful_invocation_persists_one_run_row_even_when_reusing_canonical_content(
        self,
    ) -> None:
        with TemporaryDirectory() as tmpdir:
            sqlite_path = Path(tmpdir) / "tianji.sqlite3"

            for expected_run_count in (1, 2, 3):
                artifact = run_pipeline(
                    fixture_paths=[str(FIXTURE_PATH)],
                    fetch=False,
                    source_urls=[],
                    output_path=None,
                    sqlite_path=str(sqlite_path),
                )

                with sqlite3.connect(sqlite_path) as connection:
                    run_count = connection.execute(
                        "SELECT COUNT(*) FROM runs"
                    ).fetchone()[0]
                    canonical_count = connection.execute(
                        "SELECT COUNT(*) FROM source_items"
                    ).fetchone()[0]
                    raw_item_count = connection.execute(
                        "SELECT COUNT(*) FROM raw_items"
                    ).fetchone()[0]

                self.assertEqual(run_count, expected_run_count)
                self.assertEqual(canonical_count, 3)
                self.assertEqual(raw_item_count, expected_run_count * 3)
                self.assertEqual(artifact.input_summary["raw_item_count"], 3)

            history_payload = storage.list_runs(sqlite_path=str(sqlite_path))
            self.assertEqual(len(history_payload), 3)
            self.assertEqual([row["run_id"] for row in history_payload], [3, 2, 1])

    def test_persistence_reuses_canonical_source_items_across_fixture_and_fetch(
        self,
    ) -> None:
        fixture_bytes = FIXTURE_PATH.read_bytes()

        class Handler(BaseHTTPRequestHandler):
            def do_GET(self) -> None:  # noqa: N802
                self.send_response(200)
                self.send_header("Content-Type", "application/rss+xml")
                self.send_header("Content-Length", str(len(fixture_bytes)))
                self.end_headers()
                self.wfile.write(fixture_bytes)

            def log_message(self, format: str, *args: object) -> None:
                return

        server = ThreadingHTTPServer(("127.0.0.1", 0), Handler)
        thread = threading.Thread(target=server.serve_forever, daemon=True)
        thread.start()
        self.addCleanup(server.shutdown)
        self.addCleanup(server.server_close)
        url = f"http://127.0.0.1:{server.server_port}/feed.xml"

        with TemporaryDirectory() as tmpdir:
            sqlite_path = Path(tmpdir) / "tianji.sqlite3"

            fixture_artifact = run_pipeline(
                fixture_paths=[str(FIXTURE_PATH)],
                fetch=False,
                source_urls=[],
                output_path=None,
                sqlite_path=str(sqlite_path),
            )
            fetch_artifact = run_pipeline(
                fixture_paths=[],
                fetch=True,
                source_urls=[url],
                output_path=None,
                sqlite_path=str(sqlite_path),
            )

            self.assertEqual(fixture_artifact.input_summary["raw_item_count"], 3)
            self.assertEqual(fetch_artifact.input_summary["raw_item_count"], 3)

            with sqlite3.connect(sqlite_path) as connection:
                run_count = connection.execute("SELECT COUNT(*) FROM runs").fetchone()[
                    0
                ]
                canonical_count = connection.execute(
                    "SELECT COUNT(*) FROM source_items"
                ).fetchone()[0]
                raw_link_count = connection.execute(
                    "SELECT COUNT(*) FROM raw_items WHERE canonical_source_item_id IS NOT NULL"
                ).fetchone()[0]
                normalized_link_count = connection.execute(
                    "SELECT COUNT(*) FROM normalized_events WHERE canonical_source_item_id IS NOT NULL"
                ).fetchone()[0]
                per_identity_versions = connection.execute(
                    """
                    SELECT entry_identity_hash, COUNT(DISTINCT content_hash)
                    FROM source_items
                    GROUP BY entry_identity_hash
                    ORDER BY entry_identity_hash ASC
                    """
                ).fetchall()

            self.assertEqual(run_count, 2)
            self.assertEqual(canonical_count, 3)
            self.assertEqual(raw_link_count, 6)
            self.assertEqual(normalized_link_count, 6)
            self.assertTrue(all(row[1] == 1 for row in per_identity_versions))

    def test_persistence_keeps_distinct_content_versions_for_one_identity_while_runs_stay_run_centric(
        self,
    ) -> None:
        with TemporaryDirectory() as tmpdir:
            sqlite_path = Path(tmpdir) / "tianji.sqlite3"
            base_feed = FIXTURE_PATH.read_text(encoding="utf-8")
            updated_feed = base_feed.replace(
                "China expands chip controls after new AI export dispute with the United States",
                "China expands chip controls after new AI export dispute with the United States and allied partners",
                1,
            ).replace(
                "Chinese officials announced expanded chip export controls after a new AI dispute with Washington.",
                "Chinese officials announced expanded chip export controls after a new AI dispute with Washington while allied partners joined the response.",
                1,
            )

            first_fixture_path = Path(tmpdir) / "first.xml"
            second_fixture_path = Path(tmpdir) / "second.xml"
            first_fixture_path.write_text(base_feed, encoding="utf-8")
            second_fixture_path.write_text(updated_feed, encoding="utf-8")

            run_pipeline(
                fixture_paths=[str(first_fixture_path)],
                fetch=False,
                source_urls=[],
                output_path=None,
                sqlite_path=str(sqlite_path),
            )
            run_pipeline(
                fixture_paths=[str(second_fixture_path)],
                fetch=False,
                source_urls=[],
                output_path=None,
                sqlite_path=str(sqlite_path),
            )

            with sqlite3.connect(sqlite_path) as connection:
                run_count = connection.execute("SELECT COUNT(*) FROM runs").fetchone()[
                    0
                ]
                version_rows = connection.execute(
                    """
                    SELECT entry_identity_hash, COUNT(DISTINCT content_hash)
                    FROM source_items
                    GROUP BY entry_identity_hash
                    ORDER BY entry_identity_hash ASC
                    """
                ).fetchall()

            self.assertEqual(run_count, 2)
            self.assertEqual(
                sorted(row[1] for row in version_rows),
                [1, 1, 2],
            )

            first_summary = storage.get_run_summary(
                sqlite_path=str(sqlite_path), run_id=1
            )
            second_summary = storage.get_run_summary(
                sqlite_path=str(sqlite_path), run_id=2
            )
            self.assertIsNotNone(first_summary)
            self.assertIsNotNone(second_summary)
            typed_first_summary = cast(dict[str, object], first_summary)
            typed_second_summary = cast(dict[str, object], second_summary)
            self.assertEqual(typed_first_summary["run_id"], 1)
            self.assertEqual(typed_second_summary["run_id"], 2)
            self.assertEqual(
                cast(dict[str, object], typed_first_summary["input_summary"])[
                    "raw_item_count"
                ],
                3,
            )
            self.assertEqual(
                cast(dict[str, object], typed_second_summary["input_summary"])[
                    "raw_item_count"
                ],
                3,
            )

    def test_direct_persist_run_accepts_normalized_events_from_normalize_items(
        self,
    ) -> None:
        with TemporaryDirectory() as tmpdir:
            sqlite_path = Path(tmpdir) / "tianji.sqlite3"
            raw_items = pipeline_module.parse_feed(
                FIXTURE_PATH.read_text(encoding="utf-8"),
                source="fixture:sample_feed.xml",
            )
            normalized_events = pipeline_module.normalize_items(raw_items)
            scored_events = pipeline_module.score_events(normalized_events)
            scenario_summary = pipeline_module.summarize_scenario(scored_events)
            scenario_summary["event_groups"] = pipeline_module.group_events(
                scored_events
            )
            interventions = pipeline_module.backtrack_candidates(
                scored_events,
                event_groups=scenario_summary["event_groups"],
            )
            artifact = pipeline_module.RunArtifact(
                mode="fixture",
                generated_at="2026-03-24T00:00:00+00:00",
                input_summary={
                    "raw_item_count": len(raw_items),
                    "normalized_event_count": len(normalized_events),
                    "sources": sorted({item.source for item in raw_items}),
                    "fetch_policy": "always",
                    "source_fetch_details": [],
                },
                scenario_summary=scenario_summary,
                scored_events=scored_events,
                intervention_candidates=interventions,
            )

            storage.persist_run(
                sqlite_path=str(sqlite_path),
                artifact=artifact,
                raw_items=raw_items,
                normalized_events=normalized_events,
                scored_events=scored_events,
                intervention_candidates=interventions,
            )

            with sqlite3.connect(sqlite_path) as connection:
                canonical_count = connection.execute(
                    "SELECT COUNT(*) FROM source_items"
                ).fetchone()[0]
                normalized_link_count = connection.execute(
                    "SELECT COUNT(*) FROM normalized_events WHERE canonical_source_item_id IS NOT NULL"
                ).fetchone()[0]

            self.assertEqual(canonical_count, 3)
            self.assertEqual(normalized_link_count, 3)

    def test_persistence_versions_changed_content_under_same_identity(self) -> None:
        with TemporaryDirectory() as tmpdir:
            sqlite_path = Path(tmpdir) / "tianji.sqlite3"
            served_feed = {"content": FIXTURE_PATH.read_bytes()}
            updated_feed = (
                FIXTURE_PATH.read_text(encoding="utf-8")
                .replace(
                    "China expands chip controls after new AI export dispute with the United States",
                    "China expands chip controls after new AI export dispute with the United States and allied partners",
                    1,
                )
                .replace(
                    "Chinese officials announced expanded chip export controls after a new AI dispute with Washington.",
                    "Chinese officials announced expanded chip export controls after a new AI dispute with Washington while allied partners joined the response.",
                    1,
                )
            )

            class Handler(BaseHTTPRequestHandler):
                def do_GET(self) -> None:  # noqa: N802
                    body = served_feed["content"]
                    self.send_response(200)
                    self.send_header("Content-Type", "application/rss+xml")
                    self.send_header("Content-Length", str(len(body)))
                    self.end_headers()
                    self.wfile.write(body)

                def log_message(self, format: str, *args: object) -> None:
                    return

            server = ThreadingHTTPServer(("127.0.0.1", 0), Handler)
            thread = threading.Thread(target=server.serve_forever, daemon=True)
            thread.start()
            self.addCleanup(server.shutdown)
            self.addCleanup(server.server_close)
            url = f"http://127.0.0.1:{server.server_port}/feed.xml"

            run_pipeline(
                fixture_paths=[],
                fetch=True,
                source_urls=[url],
                output_path=None,
                sqlite_path=str(sqlite_path),
            )
            served_feed["content"] = updated_feed.encode("utf-8")
            run_pipeline(
                fixture_paths=[],
                fetch=True,
                source_urls=[url],
                output_path=None,
                sqlite_path=str(sqlite_path),
            )

            with sqlite3.connect(sqlite_path) as connection:
                run_count = connection.execute("SELECT COUNT(*) FROM runs").fetchone()[
                    0
                ]
                canonical_count = connection.execute(
                    "SELECT COUNT(*) FROM source_items"
                ).fetchone()[0]
                versioned_identity_rows = connection.execute(
                    """
                        SELECT entry_identity_hash, COUNT(DISTINCT content_hash)
                        FROM source_items
                        GROUP BY entry_identity_hash
                        HAVING COUNT(DISTINCT content_hash) > 1
                        """
                ).fetchall()
                versioned_identity_hash = cast(str, versioned_identity_rows[0][0])
                first_run_title = connection.execute(
                    """
                    SELECT raw_items.title
                    FROM raw_items
                    JOIN source_items
                      ON source_items.id = raw_items.canonical_source_item_id
                    WHERE raw_items.run_id = ?
                      AND source_items.entry_identity_hash = ?
                    LIMIT 1
                    """,
                    (1, versioned_identity_hash),
                ).fetchone()[0]
                second_run_title = connection.execute(
                    """
                    SELECT raw_items.title
                    FROM raw_items
                    JOIN source_items
                      ON source_items.id = raw_items.canonical_source_item_id
                    WHERE raw_items.run_id = ?
                      AND source_items.entry_identity_hash = ?
                    LIMIT 1
                    """,
                    (2, versioned_identity_hash),
                ).fetchone()[0]

            self.assertEqual(run_count, 2)
            self.assertEqual(canonical_count, 4)
            self.assertEqual(len(versioned_identity_rows), 1)
            self.assertEqual(
                first_run_title,
                "China expands chip controls after new AI export dispute with the United States",
            )
            self.assertEqual(
                second_run_title,
                "China expands chip controls after new AI export dispute with the United States and allied partners",
            )

            first_summary = storage.get_run_summary(
                sqlite_path=str(sqlite_path), run_id=1
            )
            second_summary = storage.get_run_summary(
                sqlite_path=str(sqlite_path), run_id=2
            )
            compare_payload = storage.compare_runs(
                sqlite_path=str(sqlite_path),
                left_run_id=1,
                right_run_id=2,
            )
            self.assertIsNotNone(first_summary)
            self.assertIsNotNone(second_summary)
            self.assertIsNotNone(compare_payload)
            typed_first_summary = cast(dict[str, object], first_summary)
            typed_second_summary = cast(dict[str, object], second_summary)
            typed_compare_payload = cast(dict[str, object], compare_payload)
            self.assertEqual(
                cast(dict[str, object], typed_first_summary["input_summary"])[
                    "raw_item_count"
                ],
                3,
            )
            self.assertEqual(
                cast(dict[str, object], typed_second_summary["input_summary"])[
                    "raw_item_count"
                ],
                3,
            )
            self.assertEqual(
                cast(dict[str, object], typed_compare_payload["left"])[
                    "raw_item_count"
                ],
                3,
            )
            self.assertEqual(
                cast(dict[str, object], typed_compare_payload["right"])[
                    "raw_item_count"
                ],
                3,
            )
            self.assertTrue(
                cast(dict[str, object], typed_compare_payload["diff"])[
                    "top_scored_event_changed"
                ]
            )

    def test_duplicate_taxonomy_hashes_distinguish_identity_from_content(
        self,
    ) -> None:
        base_item = pipeline_module.parse_feed(
            FIXTURE_PATH.read_text(encoding="utf-8"),
            source="fixture:sample_feed.xml",
        )[0]
        replay_item = pipeline_module.parse_feed(
            FIXTURE_PATH.read_text(encoding="utf-8"),
            source="fixture:sample_feed.xml",
        )[0]
        changed_item = pipeline_module.parse_feed(
            FIXTURE_PATH.read_text(encoding="utf-8"),
            source="fixture:sample_feed.xml",
        )[0]
        changed_item.summary = f"{changed_item.summary} Updated briefing context."

        base_event = pipeline_module.normalize_items([base_item])[0]
        replay_event = pipeline_module.normalize_items([replay_item])[0]
        changed_event = pipeline_module.normalize_items([changed_item])[0]

        self.assertEqual(
            base_event.entry_identity_hash, replay_event.entry_identity_hash
        )
        self.assertEqual(base_event.content_hash, replay_event.content_hash)
        self.assertEqual(base_event.event_id, replay_event.event_id)
        self.assertEqual(
            base_event.entry_identity_hash, changed_event.entry_identity_hash
        )
        self.assertNotEqual(base_event.content_hash, changed_event.content_hash)
        self.assertEqual(base_event.event_id, changed_event.event_id)

    def test_fixture_and_fetch_paths_assign_same_canonical_hashes(self) -> None:
        fixture_item = pipeline_module.parse_feed(
            FIXTURE_PATH.read_text(encoding="utf-8"),
            source="fixture:sample_feed.xml",
        )[0]
        fetched_item = pipeline_module.parse_feed(
            FIXTURE_PATH.read_text(encoding="utf-8"),
            source="127.0.0.1",
        )[0]

        pipeline_module.assign_canonical_hashes([fixture_item])
        pipeline_module.assign_canonical_hashes([fetched_item])

        self.assertEqual(
            fixture_item.entry_identity_hash,
            fetched_item.entry_identity_hash,
        )
        self.assertEqual(fixture_item.content_hash, fetched_item.content_hash)

    def test_fixture_pipeline_has_stable_scoring_and_backtrack_order(self) -> None:
        artifact = run_pipeline(
            fixture_paths=[str(FIXTURE_PATH)],
            fetch=False,
            source_urls=[],
            output_path=None,
        )

        self.assertEqual(artifact.scenario_summary["dominant_field"], "technology")
        self.assertEqual(artifact.scenario_summary["risk_level"], "high")
        self.assertEqual(artifact.scored_events[0].dominant_field, "technology")
        self.assertEqual(
            artifact.scored_events[0].title,
            "China expands chip controls after new AI export dispute with the United States",
        )
        self.assertEqual(artifact.intervention_candidates[0].priority, 1)
        self.assertEqual(artifact.intervention_candidates[0].target, "usa")
        self.assertEqual(
            artifact.intervention_candidates[0].intervention_type,
            "capability-control",
        )
