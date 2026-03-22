from __future__ import annotations

import json
from pathlib import Path
import threading
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from tempfile import TemporaryDirectory
import unittest

from tianji.cli import main
from tianji.pipeline import run_pipeline


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

    def test_cli_requires_input_source(self) -> None:
        with self.assertRaises(SystemExit) as context:
            main(["run"])
        self.assertNotEqual(context.exception.code, 0)


if __name__ == "__main__":
    unittest.main()
