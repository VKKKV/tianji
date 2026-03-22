from __future__ import annotations

import io
import contextlib
import json
from pathlib import Path
import sqlite3
import threading
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from tempfile import TemporaryDirectory
from typing import cast
import unittest

from tianji.cli import main
from tianji.backtrack import EventGroupSummary, backtrack_candidates
from tianji.fetch import TianJiInputError
from tianji.models import NormalizedEvent, ScoredEvent
from tianji import pipeline as pipeline_module
from tianji import storage
from tianji.pipeline import run_pipeline
from tianji.scoring import score_event


FIXTURE_PATH = Path(__file__).parent / "fixtures" / "sample_feed.xml"


class PipelineTests(unittest.TestCase):
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

    def test_score_event_exposes_explicit_im_fa_semantics(self) -> None:
        event = NormalizedEvent(
            event_id="evt-1",
            source="fixture:test",
            title="Coordinated chip sanctions and cyber controls expand",
            summary="Officials expand coordinated chip sanctions after cyber escalation.",
            link="https://example.com/evt-1",
            published_at="2026-03-22T12:00:00Z",
            keywords=[
                "coordinated",
                "chip",
                "sanctions",
                "cyber",
                "controls",
                "escalation",
            ],
            actors=["usa", "china"],
            regions=["east-asia", "united-states"],
            field_scores={
                "technology": 6.5,
                "diplomacy": 2.0,
                "economy": 1.5,
                "conflict": 0.0,
            },
        )

        scored = score_event(event)

        self.assertEqual(scored.dominant_field, "technology")
        self.assertEqual(scored.impact_score, 13.56)
        self.assertEqual(scored.field_attraction, 7.66)
        self.assertEqual(scored.divergence_score, 19.16)
        self.assertIn("Im=13.56", scored.rationale)
        self.assertIn("Fa=7.66", scored.rationale)
        self.assertIn("im_text_signal_intensity=0.84", scored.rationale)
        self.assertIn("dominant_field=technology:7.66", scored.rationale)

    def test_score_event_rewards_clearer_field_alignment_in_fa(self) -> None:
        clearer_event = NormalizedEvent(
            event_id="evt-clear",
            source="fixture:test",
            title="Clear technology signal",
            summary="A strong single-field technology event.",
            link="https://example.com/clear",
            published_at="2026-03-22T12:00:00Z",
            keywords=["chip", "cyber", "controls", "sanctions"],
            actors=["usa", "china"],
            regions=["east-asia", "united-states"],
            field_scores={
                "technology": 6.5,
                "diplomacy": 2.0,
                "economy": 1.5,
                "conflict": 0.0,
            },
        )
        ambiguous_event = NormalizedEvent(
            event_id="evt-ambiguous",
            source="fixture:test",
            title="Ambiguous technology signal",
            summary="An event split across multiple attractor fields.",
            link="https://example.com/ambiguous",
            published_at="2026-03-22T12:05:00Z",
            keywords=["chip", "cyber", "talks", "trade"],
            actors=["usa", "china"],
            regions=["east-asia", "united-states"],
            field_scores={
                "technology": 6.5,
                "diplomacy": 5.8,
                "economy": 5.2,
                "conflict": 0.0,
            },
        )

        clearer_scored = score_event(clearer_event)
        ambiguous_scored = score_event(ambiguous_event)

        self.assertGreater(
            clearer_scored.field_attraction, ambiguous_scored.field_attraction
        )
        self.assertGreater(
            clearer_scored.divergence_score, ambiguous_scored.divergence_score
        )

    def test_score_event_rewards_stronger_weighted_field_intensity_in_im(self) -> None:
        lower_intensity_event = NormalizedEvent(
            event_id="evt-low-im",
            source="fixture:test",
            title="Moderate technology escalation",
            summary="Moderate event with limited weighted field intensity.",
            link="https://example.com/low-im",
            published_at="2026-03-22T12:10:00Z",
            keywords=["chip", "cyber", "talks", "tariff"],
            actors=["usa", "china"],
            regions=["east-asia", "united-states"],
            field_scores={
                "technology": 4.0,
                "diplomacy": 0.0,
                "economy": 0.0,
                "conflict": 0.0,
            },
        )
        higher_intensity_event = NormalizedEvent(
            event_id="evt-high-im",
            source="fixture:test",
            title="Severe technology escalation",
            summary="Severe event with stronger weighted field intensity.",
            link="https://example.com/high-im",
            published_at="2026-03-22T12:15:00Z",
            keywords=["chip", "cyber", "talks", "tariff"],
            actors=["usa", "china"],
            regions=["east-asia", "united-states"],
            field_scores={
                "technology": 4.0,
                "diplomacy": 3.5,
                "economy": 3.0,
                "conflict": 0.0,
            },
        )

        lower_scored = score_event(lower_intensity_event)
        higher_scored = score_event(higher_intensity_event)

        self.assertGreater(higher_scored.impact_score, lower_scored.impact_score)

    def test_score_event_rewards_stronger_text_signal_intensity_in_im(self) -> None:
        weaker_text_event = NormalizedEvent(
            event_id="evt-weak-text",
            source="fixture:test",
            title="Technology policy update",
            summary="Officials discuss export policy changes and regional planning.",
            link="https://example.com/weak-text",
            published_at="2026-03-22T12:20:00Z",
            keywords=[
                "technology",
                "policy",
                "export",
                "changes",
                "regional",
                "planning",
            ],
            actors=["usa", "china"],
            regions=["east-asia", "united-states"],
            field_scores={
                "technology": 6.5,
                "diplomacy": 2.0,
                "economy": 1.5,
                "conflict": 0.0,
            },
        )
        stronger_text_event = NormalizedEvent(
            event_id="evt-strong-text",
            source="fixture:test",
            title="Chip and cyber controls tighten in technology dispute",
            summary="Officials expand chip controls and cyber restrictions after satellite concerns.",
            link="https://example.com/strong-text",
            published_at="2026-03-22T12:25:00Z",
            keywords=[
                "chip",
                "cyber",
                "controls",
                "satellite",
                "restrictions",
                "dispute",
            ],
            actors=["usa", "china"],
            regions=["east-asia", "united-states"],
            field_scores={
                "technology": 6.5,
                "diplomacy": 2.0,
                "economy": 1.5,
                "conflict": 0.0,
            },
        )

        weaker_scored = score_event(weaker_text_event)
        stronger_scored = score_event(stronger_text_event)

        self.assertGreater(stronger_scored.impact_score, weaker_scored.impact_score)
        self.assertEqual(
            stronger_scored.field_attraction, weaker_scored.field_attraction
        )

    def test_score_event_text_signal_intensity_does_not_reward_generic_keyword_mass(
        self,
    ) -> None:
        generic_token_event = NormalizedEvent(
            event_id="evt-generic-text",
            source="fixture:test",
            title="International policy developments remain under discussion",
            summary="Officials review committee process updates, planning notes, and general strategy language.",
            link="https://example.com/generic-text",
            published_at="2026-03-22T12:30:00Z",
            keywords=[
                "international",
                "policy",
                "developments",
                "discussion",
                "committee",
                "strategy",
            ],
            actors=["usa", "china"],
            regions=["east-asia", "united-states"],
            field_scores={
                "technology": 6.5,
                "diplomacy": 2.0,
                "economy": 1.5,
                "conflict": 0.0,
            },
        )
        branch_relevant_event = NormalizedEvent(
            event_id="evt-branch-text",
            source="fixture:test",
            title="AI chip and cyber dispute intensifies",
            summary="Officials review chip controls, cyber restrictions, and satellite exposure.",
            link="https://example.com/branch-text",
            published_at="2026-03-22T12:35:00Z",
            keywords=["ai", "chip", "cyber", "satellite", "controls", "restrictions"],
            actors=["usa", "china"],
            regions=["east-asia", "united-states"],
            field_scores={
                "technology": 6.5,
                "diplomacy": 2.0,
                "economy": 1.5,
                "conflict": 0.0,
            },
        )

        generic_scored = score_event(generic_token_event)
        branch_scored = score_event(branch_relevant_event)

        self.assertGreater(branch_scored.impact_score, generic_scored.impact_score)
        self.assertEqual(
            branch_scored.field_attraction, generic_scored.field_attraction
        )

    def test_score_event_text_signal_intensity_respects_cap(self) -> None:
        strong_text_event = NormalizedEvent(
            event_id="evt-strong-cap",
            source="fixture:test",
            title="AI chip cyber satellite controls escalate",
            summary="Officials review ai chip cyber satellite controls after new alerts.",
            link="https://example.com/strong-cap",
            published_at="2026-03-22T12:40:00Z",
            keywords=["ai", "chip", "cyber", "satellite", "controls", "alerts"],
            actors=["usa", "china"],
            regions=["east-asia", "united-states"],
            field_scores={
                "technology": 6.5,
                "diplomacy": 2.0,
                "economy": 1.5,
                "conflict": 0.0,
            },
        )
        exaggerated_text_event = NormalizedEvent(
            event_id="evt-exaggerated-cap",
            source="fixture:test",
            title="AI chip cyber satellite controls escalate with ai chip cyber satellite focus",
            summary="Officials review ai chip cyber satellite controls after ai chip cyber satellite alerts and ai chip cyber satellite exposure.",
            link="https://example.com/exaggerated-cap",
            published_at="2026-03-22T12:45:00Z",
            keywords=["ai", "chip", "cyber", "satellite", "controls", "exposure"],
            actors=["usa", "china"],
            regions=["east-asia", "united-states"],
            field_scores={
                "technology": 6.5,
                "diplomacy": 2.0,
                "economy": 1.5,
                "conflict": 0.0,
            },
        )

        strong_scored = score_event(strong_text_event)
        exaggerated_scored = score_event(exaggerated_text_event)

        self.assertEqual(strong_scored.impact_score, exaggerated_scored.impact_score)
        self.assertEqual(
            strong_scored.field_attraction, exaggerated_scored.field_attraction
        )

    def test_score_event_text_signal_intensity_ignores_incidental_substrings(
        self,
    ) -> None:
        neutral_text_event = NormalizedEvent(
            event_id="evt-neutral-text",
            source="fixture:test",
            title="Regional relief planning continues",
            summary="Officials discuss corridor planning and funding updates.",
            link="https://example.com/neutral-text",
            published_at="2026-03-22T12:50:00Z",
            keywords=[
                "regional",
                "relief",
                "planning",
                "corridor",
                "funding",
                "updates",
            ],
            actors=["usa", "china"],
            regions=["east-asia", "united-states"],
            field_scores={
                "technology": 6.5,
                "diplomacy": 2.0,
                "economy": 1.5,
                "conflict": 0.0,
            },
        )
        incidental_substring_event = NormalizedEvent(
            event_id="evt-incidental-text",
            source="fixture:test",
            title="Air aid planning continues",
            summary="Officials discuss air aid corridors and fair funding updates.",
            link="https://example.com/incidental-text",
            published_at="2026-03-22T12:55:00Z",
            keywords=[
                "regional",
                "relief",
                "planning",
                "corridor",
                "funding",
                "updates",
            ],
            actors=["usa", "china"],
            regions=["east-asia", "united-states"],
            field_scores={
                "technology": 6.5,
                "diplomacy": 2.0,
                "economy": 1.5,
                "conflict": 0.0,
            },
        )

        neutral_scored = score_event(neutral_text_event)
        incidental_scored = score_event(incidental_substring_event)

        self.assertEqual(incidental_scored.impact_score, neutral_scored.impact_score)
        self.assertEqual(
            incidental_scored.field_attraction, neutral_scored.field_attraction
        )

    def test_score_event_text_signal_intensity_matches_punctuation_adjacent_cues(
        self,
    ) -> None:
        plain_text_event = NormalizedEvent(
            event_id="evt-plain-text",
            source="fixture:test",
            title="Policy update remains under review",
            summary="Officials discuss restrictions and oversight planning.",
            link="https://example.com/plain-text",
            published_at="2026-03-22T13:00:00Z",
            keywords=[
                "policy",
                "update",
                "review",
                "restrictions",
                "oversight",
                "planning",
            ],
            actors=["usa", "china"],
            regions=["east-asia", "united-states"],
            field_scores={
                "technology": 6.5,
                "diplomacy": 2.0,
                "economy": 1.5,
                "conflict": 0.0,
            },
        )
        punctuated_cue_event = NormalizedEvent(
            event_id="evt-punctuated-text",
            source="fixture:test",
            title="AI-driven chip, cyber, and satellite controls tighten",
            summary="Officials review chip, cyber, and satellite restrictions after new alerts.",
            link="https://example.com/punctuated-text",
            published_at="2026-03-22T13:05:00Z",
            keywords=[
                "policy",
                "update",
                "review",
                "restrictions",
                "oversight",
                "planning",
            ],
            actors=["usa", "china"],
            regions=["east-asia", "united-states"],
            field_scores={
                "technology": 6.5,
                "diplomacy": 2.0,
                "economy": 1.5,
                "conflict": 0.0,
            },
        )

        plain_scored = score_event(plain_text_event)
        punctuated_scored = score_event(punctuated_cue_event)

        self.assertGreater(punctuated_scored.impact_score, plain_scored.impact_score)
        self.assertEqual(
            punctuated_scored.field_attraction, plain_scored.field_attraction
        )

    def test_group_events_clusters_obviously_related_events(self) -> None:
        related_a = ScoredEvent(
            event_id="evt-a",
            title="China and USA expand chip controls",
            source="fixture:test",
            link="https://example.com/a",
            published_at="2026-03-22T08:00:00Z",
            actors=["china", "usa"],
            regions=["east-asia", "united-states"],
            keywords=["chip", "controls", "export", "dispute"],
            dominant_field="technology",
            impact_score=14.03,
            field_attraction=7.75,
            divergence_score=19.58,
            rationale=["Im=14.03", "Fa=7.75"],
        )
        related_b = ScoredEvent(
            event_id="evt-b",
            title="USA and China deepen export chip restrictions",
            source="fixture:test",
            link="https://example.com/b",
            published_at="2026-03-22T09:00:00Z",
            actors=["usa", "china"],
            regions=["east-asia", "united-states"],
            keywords=["chip", "restrictions", "export", "controls"],
            dominant_field="technology",
            impact_score=13.5,
            field_attraction=7.1,
            divergence_score=18.31,
            rationale=["Im=13.5", "Fa=7.1"],
        )
        unrelated = ScoredEvent(
            event_id="evt-c",
            title="Iran diplomacy channel reopens",
            source="fixture:test",
            link="https://example.com/c",
            published_at="2026-03-22T10:00:00Z",
            actors=["iran"],
            regions=["middle-east"],
            keywords=["talks", "diplomacy", "channel", "iran"],
            dominant_field="diplomacy",
            impact_score=11.67,
            field_attraction=6.17,
            divergence_score=15.92,
            rationale=["Im=11.67", "Fa=6.17"],
        )

        groups: list[EventGroupSummary] = pipeline_module.group_events(
            [related_a, related_b, unrelated]
        )

        self.assertEqual(len(groups), 1)
        self.assertEqual(groups[0]["headline_event_id"], "evt-a")
        self.assertEqual(
            groups[0]["headline_title"], "China and USA expand chip controls"
        )
        self.assertEqual(groups[0]["member_event_ids"], ["evt-a", "evt-b"])
        self.assertEqual(groups[0]["shared_keywords"], ["chip", "controls", "export"])
        self.assertEqual(groups[0]["dominant_field"], "technology")
        self.assertEqual(groups[0]["shared_actors"], ["china", "usa"])
        self.assertEqual(groups[0]["shared_regions"], ["east-asia", "united-states"])
        self.assertEqual(len(groups[0]["evidence_chain"]), 1)
        self.assertEqual(groups[0]["evidence_chain"][0]["from_event_id"], "evt-a")
        self.assertEqual(groups[0]["evidence_chain"][0]["to_event_id"], "evt-b")
        self.assertEqual(
            groups[0]["evidence_chain"][0]["relationship"], "capability-race"
        )
        self.assertEqual(groups[0]["evidence_chain"][0]["shared_signal_count"], 7)
        self.assertEqual(groups[0]["evidence_chain"][0]["time_delta_hours"], 1.0)
        self.assertEqual(groups[0]["causal_ordered_event_ids"], ["evt-a", "evt-b"])
        self.assertEqual(groups[0]["causal_span_hours"], 1.0)
        self.assertIn(
            "2 related technology events reinforce", groups[0]["chain_summary"]
        )
        self.assertIn("chip, controls, export", groups[0]["chain_summary"])
        self.assertIn("capability-race cluster", groups[0]["causal_summary"])

    def test_group_events_do_not_cluster_distant_related_events(self) -> None:
        early_event = ScoredEvent(
            event_id="evt-early",
            title="China and USA expand chip controls",
            source="fixture:test",
            link="https://example.com/early",
            published_at="2026-03-22T08:00:00Z",
            actors=["china", "usa"],
            regions=["east-asia", "united-states"],
            keywords=["chip", "controls", "export", "dispute"],
            dominant_field="technology",
            impact_score=14.03,
            field_attraction=7.75,
            divergence_score=19.58,
            rationale=["Im=14.03", "Fa=7.75"],
        )
        late_event = ScoredEvent(
            event_id="evt-late",
            title="USA and China deepen export chip restrictions",
            source="fixture:test",
            link="https://example.com/late",
            published_at="2026-03-25T08:00:00Z",
            actors=["usa", "china"],
            regions=["east-asia", "united-states"],
            keywords=["chip", "restrictions", "export", "controls"],
            dominant_field="technology",
            impact_score=13.5,
            field_attraction=7.1,
            divergence_score=18.31,
            rationale=["Im=13.5", "Fa=7.1"],
        )

        groups: list[EventGroupSummary] = pipeline_module.group_events(
            [early_event, late_event]
        )

        self.assertEqual(groups, [])

    def test_group_events_allow_missing_timestamp_fallback(self) -> None:
        unknown_time_a = ScoredEvent(
            event_id="evt-a",
            title="China and USA expand chip controls",
            source="fixture:test",
            link="https://example.com/a",
            published_at=None,
            actors=["china", "usa"],
            regions=["east-asia", "united-states"],
            keywords=["chip", "controls", "export", "dispute"],
            dominant_field="technology",
            impact_score=14.03,
            field_attraction=7.75,
            divergence_score=19.58,
            rationale=["Im=14.03", "Fa=7.75"],
        )
        unknown_time_b = ScoredEvent(
            event_id="evt-b",
            title="USA and China deepen export chip restrictions",
            source="fixture:test",
            link="https://example.com/b",
            published_at=None,
            actors=["usa", "china"],
            regions=["east-asia", "united-states"],
            keywords=["chip", "restrictions", "export", "controls"],
            dominant_field="technology",
            impact_score=13.5,
            field_attraction=7.1,
            divergence_score=18.31,
            rationale=["Im=13.5", "Fa=7.1"],
        )

        groups: list[EventGroupSummary] = pipeline_module.group_events(
            [unknown_time_a, unknown_time_b]
        )

        self.assertEqual(len(groups), 1)
        self.assertIsNone(groups[0]["causal_span_hours"])
        self.assertIn("across 2 events.", groups[0]["causal_summary"])
        self.assertNotIn(" over ", groups[0]["causal_summary"])

    def test_group_events_compute_causal_span_from_known_timestamps(self) -> None:
        known_early = ScoredEvent(
            event_id="evt-a",
            title="China and USA expand chip controls",
            source="fixture:test",
            link="https://example.com/a",
            published_at="2026-03-22T08:00:00Z",
            actors=["china", "usa"],
            regions=["east-asia", "united-states"],
            keywords=["chip", "controls", "export", "dispute"],
            dominant_field="technology",
            impact_score=14.03,
            field_attraction=7.75,
            divergence_score=19.58,
            rationale=["Im=14.03", "Fa=7.75"],
        )
        unknown_time = ScoredEvent(
            event_id="evt-b",
            title="USA broadens export controls after chip dispute",
            source="fixture:test",
            link="https://example.com/b",
            published_at=None,
            actors=["usa", "china"],
            regions=["east-asia", "united-states"],
            keywords=["chip", "controls", "tariff", "sanctions"],
            dominant_field="technology",
            impact_score=13.5,
            field_attraction=7.1,
            divergence_score=18.31,
            rationale=["Im=13.5", "Fa=7.1"],
        )
        known_late = ScoredEvent(
            event_id="evt-c",
            title="USA widens chip tariff controls after export review",
            source="fixture:test",
            link="https://example.com/c",
            published_at="2026-03-22T10:00:00Z",
            actors=["usa", "china"],
            regions=["east-asia", "united-states"],
            keywords=["tariff", "sanctions", "controls", "review"],
            dominant_field="technology",
            impact_score=12.9,
            field_attraction=6.95,
            divergence_score=17.82,
            rationale=["Im=12.9", "Fa=6.95"],
        )

        groups: list[EventGroupSummary] = pipeline_module.group_events(
            [known_early, unknown_time, known_late]
        )

        self.assertEqual(len(groups), 1)
        self.assertEqual(groups[0]["causal_span_hours"], 2.0)
        self.assertIn(" over 2.0h", groups[0]["causal_summary"])

    def test_group_events_support_transitive_causal_clustering(self) -> None:
        anchor = ScoredEvent(
            event_id="evt-a",
            title="China expands chip controls",
            source="fixture:test",
            link="https://example.com/a",
            published_at="2026-03-22T08:00:00Z",
            actors=["china", "usa"],
            regions=["east-asia", "united-states"],
            keywords=["chip", "controls", "export", "dispute"],
            dominant_field="technology",
            impact_score=14.03,
            field_attraction=7.75,
            divergence_score=19.58,
            rationale=["Im=14.03", "Fa=7.75"],
        )
        bridge = ScoredEvent(
            event_id="evt-b",
            title="USA broadens export controls after chip dispute",
            source="fixture:test",
            link="https://example.com/b",
            published_at="2026-03-22T10:00:00Z",
            actors=["usa", "china"],
            regions=["east-asia", "united-states"],
            keywords=["chip", "controls", "tariff", "sanctions"],
            dominant_field="technology",
            impact_score=13.5,
            field_attraction=7.1,
            divergence_score=18.31,
            rationale=["Im=13.5", "Fa=7.1"],
        )
        transitive = ScoredEvent(
            event_id="evt-c",
            title="USA widens chip tariff controls after export review",
            source="fixture:test",
            link="https://example.com/c",
            published_at="2026-03-22T09:00:00Z",
            actors=["usa", "china"],
            regions=["east-asia", "united-states"],
            keywords=["tariff", "sanctions", "controls", "review"],
            dominant_field="technology",
            impact_score=12.9,
            field_attraction=6.95,
            divergence_score=17.82,
            rationale=["Im=12.9", "Fa=6.95"],
        )

        groups: list[EventGroupSummary] = pipeline_module.group_events(
            [anchor, bridge, transitive]
        )

        self.assertEqual(len(groups), 1)
        self.assertEqual(groups[0]["member_event_ids"], ["evt-a", "evt-b", "evt-c"])
        self.assertEqual(
            groups[0]["causal_ordered_event_ids"], ["evt-a", "evt-b", "evt-c"]
        )
        self.assertEqual(len(groups[0]["evidence_chain"]), 2)
        self.assertEqual(groups[0]["evidence_chain"][0]["from_event_id"], "evt-a")
        self.assertEqual(groups[0]["evidence_chain"][0]["to_event_id"], "evt-b")
        self.assertEqual(groups[0]["evidence_chain"][1]["from_event_id"], "evt-b")
        self.assertEqual(groups[0]["evidence_chain"][1]["to_event_id"], "evt-c")
        self.assertEqual(groups[0]["causal_span_hours"], 2.0)
        self.assertEqual(groups[0]["evidence_chain"][1]["time_delta_hours"], 1.0)
        self.assertIn("across 3 events", groups[0]["causal_summary"])

    def test_pipeline_surfaces_event_groups_in_scenario_summary(self) -> None:
        artifact = run_pipeline(
            fixture_paths=[str(FIXTURE_PATH)],
            fetch=False,
            source_urls=[],
            output_path=None,
        )

        self.assertIn("event_groups", artifact.scenario_summary)
        self.assertIsInstance(artifact.scenario_summary["event_groups"], list)
        for group in artifact.scenario_summary["event_groups"]:
            self.assertIn("headline_title", group)
            self.assertIn("shared_keywords", group)

    def test_backtrack_candidates_collapse_grouped_duplicate_events(self) -> None:
        grouped_a = ScoredEvent(
            event_id="evt-a",
            title="China and USA expand chip controls",
            source="fixture:test",
            link="https://example.com/a",
            published_at="2026-03-22T08:00:00Z",
            actors=["china", "usa"],
            regions=["east-asia", "united-states"],
            keywords=["chip", "controls", "export", "dispute"],
            dominant_field="technology",
            impact_score=14.03,
            field_attraction=7.75,
            divergence_score=19.58,
            rationale=["Im=14.03", "Fa=7.75"],
        )
        grouped_b = ScoredEvent(
            event_id="evt-b",
            title="USA and China deepen export chip restrictions",
            source="fixture:test",
            link="https://example.com/b",
            published_at="2026-03-22T09:00:00Z",
            actors=["usa", "china"],
            regions=["east-asia", "united-states"],
            keywords=["chip", "restrictions", "export", "controls"],
            dominant_field="technology",
            impact_score=13.5,
            field_attraction=7.1,
            divergence_score=18.31,
            rationale=["Im=13.5", "Fa=7.1"],
        )
        unrelated = ScoredEvent(
            event_id="evt-c",
            title="Iran diplomacy channel reopens",
            source="fixture:test",
            link="https://example.com/c",
            published_at="2026-03-22T10:00:00Z",
            actors=["iran"],
            regions=["middle-east"],
            keywords=["talks", "diplomacy", "channel", "iran"],
            dominant_field="diplomacy",
            impact_score=11.67,
            field_attraction=6.17,
            divergence_score=15.92,
            rationale=["Im=11.67", "Fa=6.17"],
        )
        groups: list[EventGroupSummary] = pipeline_module.group_events(
            [grouped_a, grouped_b, unrelated]
        )

        candidates = backtrack_candidates(
            [grouped_a, grouped_b, unrelated],
            event_groups=groups,
        )

        self.assertEqual(len(candidates), 2)
        self.assertEqual(candidates[0].event_id, "evt-a")
        self.assertEqual(candidates[1].event_id, "evt-c")
        self.assertIn("Evidence chain:", candidates[0].reason)
        self.assertIn("2 related technology events reinforce", candidates[0].reason)
        self.assertIn("Causal cluster:", candidates[0].reason)
        self.assertNotIn("Evidence chain:", candidates[1].reason)

    def test_pipeline_reduces_duplicate_interventions_for_grouped_events(self) -> None:
        fixture_a = ScoredEvent(
            event_id="evt-a",
            title="China and USA expand chip controls",
            source="fixture:test",
            link="https://example.com/a",
            published_at="2026-03-22T08:00:00Z",
            actors=["china", "usa"],
            regions=["east-asia", "united-states"],
            keywords=["chip", "controls", "export", "dispute"],
            dominant_field="technology",
            impact_score=14.03,
            field_attraction=7.75,
            divergence_score=19.58,
            rationale=["Im=14.03", "Fa=7.75"],
        )
        fixture_b = ScoredEvent(
            event_id="evt-b",
            title="USA and China deepen export chip restrictions",
            source="fixture:test",
            link="https://example.com/b",
            published_at="2026-03-22T09:00:00Z",
            actors=["usa", "china"],
            regions=["east-asia", "united-states"],
            keywords=["chip", "restrictions", "export", "controls"],
            dominant_field="technology",
            impact_score=13.5,
            field_attraction=7.1,
            divergence_score=18.31,
            rationale=["Im=13.5", "Fa=7.1"],
        )
        unrelated = ScoredEvent(
            event_id="evt-c",
            title="Iran diplomacy channel reopens",
            source="fixture:test",
            link="https://example.com/c",
            published_at="2026-03-22T10:00:00Z",
            actors=["iran"],
            regions=["middle-east"],
            keywords=["talks", "diplomacy", "channel", "iran"],
            dominant_field="diplomacy",
            impact_score=11.67,
            field_attraction=6.17,
            divergence_score=15.92,
            rationale=["Im=11.67", "Fa=6.17"],
        )
        groups: list[EventGroupSummary] = pipeline_module.group_events(
            [fixture_a, fixture_b, unrelated]
        )

        candidates = backtrack_candidates(
            [fixture_a, fixture_b, unrelated],
            event_groups=groups,
        )

        self.assertEqual(
            [candidate.event_id for candidate in candidates], ["evt-a", "evt-c"]
        )

    def test_cli_can_fetch_using_source_config(self) -> None:
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

        with TemporaryDirectory() as tmpdir:
            config_path = Path(tmpdir) / "sources.json"
            output_path = Path(tmpdir) / "config-report.json"
            config_path.write_text(
                json.dumps(
                    {
                        "sources": [
                            {
                                "name": "local-feed",
                                "url": f"http://127.0.0.1:{server.server_port}/feed.xml",
                            }
                        ]
                    },
                    ensure_ascii=False,
                    indent=2,
                ),
                encoding="utf-8",
            )

            exit_code = main(
                [
                    "run",
                    "--fetch",
                    "--source-config",
                    str(config_path),
                    "--source-name",
                    "local-feed",
                    "--output",
                    str(output_path),
                ]
            )

            self.assertEqual(exit_code, 0)
            self.assertTrue(output_path.exists())

            payload = json.loads(output_path.read_text(encoding="utf-8"))
            self.assertEqual(payload["mode"], "fetch")
            self.assertEqual(payload["input_summary"]["raw_item_count"], 3)
            self.assertEqual(payload["schema_version"], "tianji.run-artifact.v1")

    def test_cli_dedupes_config_and_explicit_source_urls(self) -> None:
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
            config_path = Path(tmpdir) / "sources.json"
            output_path = Path(tmpdir) / "dedupe-report.json"
            config_path.write_text(
                json.dumps(
                    {"sources": [{"name": "local-feed", "url": url}]},
                    ensure_ascii=False,
                    indent=2,
                ),
                encoding="utf-8",
            )

            exit_code = main(
                [
                    "run",
                    "--fetch",
                    "--source-config",
                    str(config_path),
                    "--source-name",
                    "local-feed",
                    "--source-url",
                    url,
                    "--output",
                    str(output_path),
                ]
            )

            self.assertEqual(exit_code, 0)
            payload = json.loads(output_path.read_text(encoding="utf-8"))
            self.assertEqual(payload["input_summary"]["raw_item_count"], 3)

    def test_run_pipeline_reports_malformed_fixture_cleanly(self) -> None:
        with TemporaryDirectory() as tmpdir:
            bad_fixture = Path(tmpdir) / "bad.xml"
            bad_fixture.write_text("<rss><channel><item>", encoding="utf-8")

            with self.assertRaises(TianJiInputError) as context:
                run_pipeline(
                    fixture_paths=[str(bad_fixture)],
                    fetch=False,
                    source_urls=[],
                    output_path=None,
                )

            self.assertIn("Failed to parse feed", str(context.exception))
            self.assertIn("bad.xml", str(context.exception))

    def test_run_pipeline_rejects_unsupported_feed_format_cleanly(self) -> None:
        with TemporaryDirectory() as tmpdir:
            bad_fixture = Path(tmpdir) / "unsupported.xml"
            bad_fixture.write_text(
                "<root><message>not a feed</message></root>",
                encoding="utf-8",
            )

            with self.assertRaises(TianJiInputError) as context:
                run_pipeline(
                    fixture_paths=[str(bad_fixture)],
                    fetch=False,
                    source_urls=[],
                    output_path=None,
                )

            self.assertIn("Unsupported feed format", str(context.exception))
            self.assertIn("unsupported.xml", str(context.exception))

    def test_cli_reports_fetch_failure_cleanly(self) -> None:
        stderr = io.StringIO()
        with contextlib.redirect_stderr(stderr):
            with self.assertRaises(SystemExit) as context:
                main(
                    [
                        "run",
                        "--fetch",
                        "--source-url",
                        "http://127.0.0.1:9/feed.xml",
                    ]
                )

        self.assertNotEqual(context.exception.code, 0)
        self.assertIn("Failed to fetch source URL", stderr.getvalue())

    def test_cli_reports_unknown_source_name_cleanly(self) -> None:
        stderr = io.StringIO()
        with TemporaryDirectory() as tmpdir:
            config_path = Path(tmpdir) / "sources.json"
            config_path.write_text(
                json.dumps(
                    {
                        "sources": [
                            {"name": "known", "url": "https://example.com/feed.xml"}
                        ]
                    },
                    ensure_ascii=False,
                    indent=2,
                ),
                encoding="utf-8",
            )

            with contextlib.redirect_stderr(stderr):
                with self.assertRaises(SystemExit) as context:
                    main(
                        [
                            "run",
                            "--fetch",
                            "--source-config",
                            str(config_path),
                            "--source-name",
                            "missing",
                        ]
                    )

        self.assertNotEqual(context.exception.code, 0)
        self.assertIn("Unknown source name(s)", stderr.getvalue())

    def test_cli_reports_duplicate_source_names_cleanly(self) -> None:
        stderr = io.StringIO()
        with TemporaryDirectory() as tmpdir:
            config_path = Path(tmpdir) / "sources.json"
            config_path.write_text(
                json.dumps(
                    {
                        "sources": [
                            {"name": "dup", "url": "https://example.com/one.xml"},
                            {"name": "dup", "url": "https://example.com/two.xml"},
                        ]
                    },
                    ensure_ascii=False,
                    indent=2,
                ),
                encoding="utf-8",
            )

            with contextlib.redirect_stderr(stderr):
                with self.assertRaises(SystemExit) as context:
                    main(
                        [
                            "run",
                            "--fetch",
                            "--source-config",
                            str(config_path),
                        ]
                    )

        self.assertNotEqual(context.exception.code, 0)
        self.assertIn("Duplicate source name", stderr.getvalue())

    def test_cli_requires_input_source(self) -> None:
        with self.assertRaises(SystemExit) as context:
            main(["run"])
        self.assertNotEqual(context.exception.code, 0)


if __name__ == "__main__":
    unittest.main()
