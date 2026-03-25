from support import *

import os
from urllib.request import urlopen

from tianji.cli import load_source_registry, resolve_sources
from tianji.daemon import create_server


class CliInputTests(unittest.TestCase):
    def test_support_imports_expose_cli_main_and_storage_module_facades(self) -> None:
        self.assertIs(main, __import__("tianji.cli", fromlist=["main"]).main)
        self.assertIs(storage, __import__("tianji", fromlist=["storage"]).storage)

    def test_top_level_help_separates_sync_run_and_daemon_controls(self) -> None:
        stdout = io.StringIO()
        with contextlib.redirect_stdout(stdout):
            exit_code = main(["--help"])

        self.assertEqual(exit_code, 0)
        help_text = stdout.getvalue()
        self.assertIn("run", help_text)
        self.assertIn("daemon", help_text)
        self.assertIn(
            "Synchronous one-shot runs plus thin local daemon controls.", help_text
        )
        self.assertIn("Usage:", help_text)

    def test_run_help_returns_zero_and_keeps_usage_surface(self) -> None:
        stdout = io.StringIO()
        with contextlib.redirect_stdout(stdout):
            exit_code = main(["run", "--help"])

        self.assertEqual(exit_code, 0)
        help_text = stdout.getvalue()
        self.assertIn("Usage:", help_text)
        self.assertIn("--fixture", help_text)
        self.assertIn("--fetch", help_text)
        self.assertIn("--source-config", help_text)

    def test_run_without_input_exits_with_usage_error_on_stderr(self) -> None:
        stderr = io.StringIO()
        with contextlib.redirect_stderr(stderr):
            with self.assertRaises(SystemExit) as context:
                main(["run"])

        self.assertEqual(context.exception.code, 2)
        error_text = stderr.getvalue()
        self.assertIn("Usage:", error_text)
        self.assertIn("Provide at least one --fixture", error_text)

    def test_daemon_help_separates_sync_run_from_daemon_controls(self) -> None:
        stdout = io.StringIO()
        with contextlib.redirect_stdout(stdout):
            exit_code = main(["daemon", "--help"])

        self.assertEqual(exit_code, 0)
        help_text = stdout.getvalue()
        self.assertIn("start", help_text)
        self.assertIn("status", help_text)
        self.assertIn("stop", help_text)
        self.assertIn("run", help_text)
        self.assertIn("schedule", help_text)
        self.assertIn("use `run` for synchronous", help_text)
        self.assertIn("writes", help_text)

    def test_daemon_schedule_rejects_non_positive_interval(self) -> None:
        stderr = io.StringIO()
        with contextlib.redirect_stderr(stderr):
            with self.assertRaises(SystemExit) as context:
                main(
                    [
                        "daemon",
                        "schedule",
                        "--every-seconds",
                        "0",
                        "--count",
                        "1",
                        "--fixture",
                        str(FIXTURE_PATH),
                    ]
                )

        self.assertEqual(context.exception.code, 2)
        self.assertIn("--every-seconds", stderr.getvalue())
        self.assertIn("greater than or equal to 60", stderr.getvalue())

    def test_daemon_schedule_rejects_interval_below_sixty_seconds(self) -> None:
        stderr = io.StringIO()
        with contextlib.redirect_stderr(stderr):
            with self.assertRaises(SystemExit) as context:
                main(
                    [
                        "daemon",
                        "schedule",
                        "--every-seconds",
                        "59",
                        "--count",
                        "1",
                        "--fixture",
                        str(FIXTURE_PATH),
                    ]
                )

        self.assertEqual(context.exception.code, 2)
        self.assertIn("--every-seconds", stderr.getvalue())
        self.assertIn("greater than or equal to 60", stderr.getvalue())

    def test_daemon_schedule_rejects_non_positive_count(self) -> None:
        stderr = io.StringIO()
        with contextlib.redirect_stderr(stderr):
            with self.assertRaises(SystemExit) as context:
                main(
                    [
                        "daemon",
                        "schedule",
                        "--every-seconds",
                        "60",
                        "--count",
                        "0",
                        "--fixture",
                        str(FIXTURE_PATH),
                    ]
                )

        self.assertEqual(context.exception.code, 2)
        self.assertIn("--count", stderr.getvalue())
        self.assertIn("greater than zero", stderr.getvalue())

    def test_daemon_run_reports_job_status_through_cli(self) -> None:
        with TemporaryDirectory() as tmpdir:
            socket_path = Path(tmpdir) / "tianji.sock"
            sqlite_path = Path(tmpdir) / "tianji.sqlite3"
            server = create_server(socket_path=str(socket_path), host="127.0.0.1")
            thread = threading.Thread(target=server.serve_forever, daemon=True)
            thread.start()
            self.addCleanup(server.shutdown)
            self.addCleanup(server.server_close)
            self.addCleanup(lambda: socket_path.unlink(missing_ok=True))

            queue_stdout = io.StringIO()
            with contextlib.redirect_stdout(queue_stdout):
                exit_code = main(
                    [
                        "daemon",
                        "run",
                        "--socket-path",
                        str(socket_path),
                        "--fixture",
                        str(FIXTURE_PATH),
                        "--sqlite-path",
                        str(sqlite_path),
                    ]
                )

            self.assertEqual(exit_code, 0)
            queue_payload = json.loads(queue_stdout.getvalue())
            self.assertEqual(queue_payload["state"], "queued")
            job_id = queue_payload["job_id"]

            final_payload = None
            for _ in range(100):
                status_stdout = io.StringIO()
                with contextlib.redirect_stdout(status_stdout):
                    exit_code = main(
                        [
                            "daemon",
                            "status",
                            "--socket-path",
                            str(socket_path),
                            "--job-id",
                            job_id,
                        ]
                    )
                self.assertEqual(exit_code, 0)
                final_payload = json.loads(status_stdout.getvalue())
                if final_payload["state"] in {"succeeded", "failed"}:
                    break
                threading.Event().wait(0.02)

            self.assertIsNotNone(final_payload)
            assert final_payload is not None
            self.assertEqual(final_payload["job_id"], job_id)
            self.assertIn(final_payload["state"], {"queued", "running", "succeeded"})
            self.assertEqual(final_payload["state"], "succeeded")
            self.assertIsInstance(final_payload["run_id"], int)

    def test_daemon_schedule_queues_bounded_repeated_runs(self) -> None:
        with TemporaryDirectory() as tmpdir:
            socket_path = Path(tmpdir) / "tianji.sock"
            server = create_server(socket_path=str(socket_path), host="127.0.0.1")
            thread = threading.Thread(target=server.serve_forever, daemon=True)
            thread.start()
            self.addCleanup(server.shutdown)
            self.addCleanup(server.server_close)
            self.addCleanup(lambda: socket_path.unlink(missing_ok=True))

            stdout = io.StringIO()
            with mock.patch("tianji.cli_daemon.time.sleep") as sleep_mock:
                with contextlib.redirect_stdout(stdout):
                    exit_code = main(
                        [
                            "daemon",
                            "schedule",
                            "--socket-path",
                            str(socket_path),
                            "--every-seconds",
                            "60",
                            "--count",
                            "2",
                            "--fixture",
                            str(FIXTURE_PATH),
                        ]
                    )

            self.assertEqual(exit_code, 0)
            payload = json.loads(stdout.getvalue())
            self.assertEqual(payload["schedule"], {"every_seconds": 60, "count": 2})
            self.assertEqual(len(payload["queued_runs"]), 2)
            self.assertEqual(
                sorted(payload["job_states"]),
                ["failed", "queued", "running", "succeeded"],
            )
            sleep_mock.assert_called_once_with(60)

    def test_daemon_help_shows_plan_default_socket_path(self) -> None:
        stdout = io.StringIO()
        with contextlib.redirect_stdout(stdout):
            exit_code = main(["daemon", "run", "--help"])

        self.assertEqual(exit_code, 0)
        self.assertIn("runs/tianji.sock", stdout.getvalue())

    def test_daemon_status_without_job_id_reports_process_surface(self) -> None:
        with TemporaryDirectory() as tmpdir:
            socket_path = Path(tmpdir) / "tianji.sock"
            pid_path = Path(f"{socket_path}.pid")
            pid_path.write_text(f"{os.getpid()}\n", encoding="utf-8")
            self.addCleanup(lambda: pid_path.unlink(missing_ok=True))

            stdout = io.StringIO()
            with contextlib.redirect_stdout(stdout):
                exit_code = main(
                    ["daemon", "status", "--socket-path", str(socket_path)]
                )

            self.assertEqual(exit_code, 0)
            payload = json.loads(stdout.getvalue())
            self.assertEqual(payload["socket_path"], str(socket_path))
            self.assertTrue(payload["pid"] >= 1)
            self.assertFalse(payload["running"])
            self.assertEqual(
                payload["job_states"], ["failed", "queued", "running", "succeeded"]
            )

    def test_daemon_start_and_stop_manage_local_process(self) -> None:
        with TemporaryDirectory() as tmpdir:
            socket_path = Path(tmpdir) / "tianji.sock"
            sqlite_path = Path(tmpdir) / "tianji.sqlite3"
            port = 8876

            start_stdout = io.StringIO()
            with contextlib.redirect_stdout(start_stdout):
                exit_code = main(
                    [
                        "daemon",
                        "start",
                        "--socket-path",
                        str(socket_path),
                        "--sqlite-path",
                        str(sqlite_path),
                        "--port",
                        str(port),
                    ]
                )

            self.assertEqual(exit_code, 0)
            start_payload = json.loads(start_stdout.getvalue())
            self.assertEqual(start_payload["socket_path"], str(socket_path))
            self.assertEqual(start_payload["sqlite_path"], str(sqlite_path))
            self.assertEqual(start_payload["host"], "127.0.0.1")
            self.assertEqual(start_payload["port"], port)
            self.assertEqual(start_payload["api_base_url"], f"http://127.0.0.1:{port}")
            self.assertTrue(start_payload["running"])
            self.assertTrue(socket_path.exists())

            with urlopen(f"http://127.0.0.1:{port}/api/v1/meta") as response:
                api_payload = json.loads(response.read().decode("utf-8"))
            self.assertEqual(api_payload["api_version"], "v1")
            self.assertIsNone(api_payload["error"])

            stop_stdout = io.StringIO()
            with contextlib.redirect_stdout(stop_stdout):
                exit_code = main(["daemon", "stop", "--socket-path", str(socket_path)])

            self.assertEqual(exit_code, 0)
            stop_payload = json.loads(stop_stdout.getvalue())
            self.assertEqual(stop_payload["socket_path"], str(socket_path))
            self.assertFalse(stop_payload["running"])

    def test_daemon_status_reports_unknown_job_cleanly(self) -> None:
        stderr = io.StringIO()
        with TemporaryDirectory() as tmpdir:
            socket_path = Path(tmpdir) / "tianji.sock"
            server = create_server(socket_path=str(socket_path), host="127.0.0.1")
            thread = threading.Thread(target=server.serve_forever, daemon=True)
            thread.start()
            self.addCleanup(server.shutdown)
            self.addCleanup(server.server_close)
            self.addCleanup(lambda: socket_path.unlink(missing_ok=True))

            with contextlib.redirect_stderr(stderr):
                with self.assertRaises(SystemExit) as context:
                    main(
                        [
                            "daemon",
                            "status",
                            "--socket-path",
                            str(socket_path),
                            "--job-id",
                            "missing-job",
                        ]
                    )

        self.assertEqual(context.exception.code, 2)
        self.assertIn("unknown job_id", stderr.getvalue())

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

    def test_cli_uses_config_default_and_per_source_fetch_policy(self) -> None:
        with TemporaryDirectory() as tmpdir:
            config_path = Path(tmpdir) / "sources.json"
            config_path.write_text(
                json.dumps(
                    {
                        "default_fetch_policy": "if-missing",
                        "sources": [
                            {
                                "name": "defaulted",
                                "url": "https://example.com/default.xml",
                            },
                            {
                                "name": "override",
                                "url": "https://example.com/override.xml",
                                "fetch_policy": "if-changed",
                            },
                        ],
                    },
                    ensure_ascii=False,
                    indent=2,
                ),
                encoding="utf-8",
            )

            registry, default_fetch_policy = load_source_registry(str(config_path))

            self.assertEqual(default_fetch_policy, "if-missing")
            self.assertEqual(
                resolve_sources(registry=registry, selected_names=[]),
                [
                    {
                        "name": "defaulted",
                        "url": "https://example.com/default.xml",
                        "fetch_policy": "if-missing",
                    },
                    {
                        "name": "override",
                        "url": "https://example.com/override.xml",
                        "fetch_policy": "if-changed",
                    },
                ],
            )

    def test_cli_fetch_policy_override_applies_to_all_selected_sources(self) -> None:
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

        explicit_url = f"http://127.0.0.1:{server.server_port}/explicit.xml"

        with TemporaryDirectory() as tmpdir:
            config_path = Path(tmpdir) / "sources.json"
            output_path = Path(tmpdir) / "policy-override-report.json"
            config_path.write_text(
                json.dumps(
                    {
                        "default_fetch_policy": "if-missing",
                        "sources": [
                            {
                                "name": "override",
                                "url": f"http://127.0.0.1:{server.server_port}/override.xml",
                                "fetch_policy": "if-changed",
                            }
                        ],
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
                    "override",
                    "--fetch-policy",
                    "always",
                    "--output",
                    str(output_path),
                ]
            )

            self.assertEqual(exit_code, 0)
            payload = json.loads(output_path.read_text(encoding="utf-8"))
            self.assertEqual(payload["input_summary"]["fetch_policy"], "always")
            self.assertEqual(
                payload["input_summary"]["source_fetch_details"],
                [
                    {
                        "name": "override",
                        "url": f"http://127.0.0.1:{server.server_port}/override.xml",
                        "fetch_policy": "always",
                    },
                ],
            )

    def test_cli_fetch_persistence_keeps_public_artifact_shape_unchanged(self) -> None:
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
            sqlite_path = Path(tmpdir) / "tianji.sqlite3"
            output_path = Path(tmpdir) / "fetch-persisted-report.json"

            exit_code = main(
                [
                    "run",
                    "--fetch",
                    "--source-url",
                    f"http://127.0.0.1:{server.server_port}/feed.xml",
                    "--sqlite-path",
                    str(sqlite_path),
                    "--output",
                    str(output_path),
                ]
            )

            self.assertEqual(exit_code, 0)
            payload = json.loads(output_path.read_text(encoding="utf-8"))
            self.assertEqual(
                set(payload),
                {
                    "schema_version",
                    "mode",
                    "generated_at",
                    "input_summary",
                    "scenario_summary",
                    "scored_events",
                    "intervention_candidates",
                },
            )

            history_show_stdout = io.StringIO()
            with contextlib.redirect_stdout(history_show_stdout):
                exit_code = main(
                    [
                        "history-show",
                        "--sqlite-path",
                        str(sqlite_path),
                        "--latest",
                    ]
                )

            self.assertEqual(exit_code, 0)
            history_payload = json.loads(history_show_stdout.getvalue())
            self.assertTrue(
                {
                    "input_summary",
                    "scenario_summary",
                    "scored_events",
                    "intervention_candidates",
                }.issubset(history_payload)
            )
            self.assertNotIn("content_hash", history_payload)

    def test_cli_reports_invalid_source_config_fetch_policy_cleanly(self) -> None:
        stderr = io.StringIO()
        with TemporaryDirectory() as tmpdir:
            config_path = Path(tmpdir) / "sources.json"
            config_path.write_text(
                json.dumps(
                    {
                        "default_fetch_policy": "stale-cache",
                        "sources": [
                            {"name": "known", "url": "https://example.com/feed.xml"}
                        ],
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
        self.assertIn("default_fetch_policy", stderr.getvalue())
        self.assertIn("always, if-missing, if-changed", stderr.getvalue())

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
