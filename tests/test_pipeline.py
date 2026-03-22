from __future__ import annotations

import io
import contextlib
import json
from pathlib import Path
import sqlite3
import threading
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from tempfile import TemporaryDirectory
import unittest

from tianji.cli import main
from tianji.fetch import TianJiInputError
from tianji.models import NormalizedEvent
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
        self.assertEqual(scored.impact_score, 12.72)
        self.assertEqual(scored.field_attraction, 7.66)
        self.assertEqual(scored.divergence_score, 18.61)
        self.assertIn("Im=12.72", scored.rationale)
        self.assertIn("Fa=7.66", scored.rationale)
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
