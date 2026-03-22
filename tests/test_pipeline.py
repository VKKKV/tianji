from support import *


class PipelineIntegrationTests(unittest.TestCase):
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
