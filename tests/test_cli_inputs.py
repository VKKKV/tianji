from support import *


class CliInputTests(unittest.TestCase):
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
