from support import *

from http.server import ThreadingHTTPServer
import time
from urllib.error import HTTPError
from urllib.request import urlopen

from tianji.daemon import (
    ALLOWED_JOB_STATES,
    DEFAULT_HTTP_API_PORT,
    RunJobRequest,
    create_api_server,
    create_server,
    serve,
    send_daemon_request,
)


class DaemonTests(unittest.TestCase):
    def test_local_api_meta_returns_frozen_envelope_and_metadata_keys(self) -> None:
        api_fixture = cast(
            dict[str, object], load_contract_fixture("local_api_meta_v1.json")
        )

        with TemporaryDirectory() as tmpdir:
            sqlite_path = Path(tmpdir) / "tianji.sqlite3"
            self._persist_sample_run(sqlite_path=sqlite_path)

            server, base_url = self._start_api_server(sqlite_path=sqlite_path)
            self.addCleanup(server.shutdown)
            self.addCleanup(server.server_close)

            response = self._read_json(f"{base_url}/api/v1/meta")

            self.assertEqual(response["api_version"], api_fixture["api_version"])
            self.assertIsNone(response["error"])
            self.assertEqual(response["data"], api_fixture["data"])

    def test_local_api_runs_limit_mirrors_history_list_payload_vocabulary(self) -> None:
        api_fixture = cast(
            dict[str, object], load_contract_fixture("local_api_runs_v1.json")
        )
        list_item_fixture = cast(
            dict[str, object], load_contract_fixture("history_list_item_v1.json")
        )

        with TemporaryDirectory() as tmpdir:
            sqlite_path = Path(tmpdir) / "tianji.sqlite3"
            self._persist_sample_run(sqlite_path=sqlite_path)
            self._persist_sample_run(sqlite_path=sqlite_path)
            self._persist_grouped_run(sqlite_path=sqlite_path)

            server, base_url = self._start_api_server(sqlite_path=sqlite_path)
            self.addCleanup(server.shutdown)
            self.addCleanup(server.server_close)

            response = self._read_json(f"{base_url}/api/v1/runs?limit=2")

            self.assertEqual(response["api_version"], api_fixture["api_version"])
            self.assertIsNone(response["error"])
            data = cast(dict[str, object], response["data"])
            self.assertEqual(
                data["resource"],
                cast(dict[str, object], api_fixture["data"])["resource"],
            )
            self.assertEqual(
                data["item_contract_fixture"],
                cast(dict[str, object], api_fixture["data"])["item_contract_fixture"],
            )
            items = cast(list[dict[str, object]], data["items"])
            self.assertEqual(len(items), 2)
            for item in items:
                self.assertEqual(set(item), set(list_item_fixture))

    def test_local_api_run_detail_mirrors_history_detail_payload_vocabulary(
        self,
    ) -> None:
        detail_fixture = cast(
            dict[str, object], load_contract_fixture("history_detail_v1.json")
        )

        with TemporaryDirectory() as tmpdir:
            sqlite_path = Path(tmpdir) / "tianji.sqlite3"
            self._persist_grouped_run(sqlite_path=sqlite_path)

            server, base_url = self._start_api_server(sqlite_path=sqlite_path)
            self.addCleanup(server.shutdown)
            self.addCleanup(server.server_close)

            response = self._read_json(f"{base_url}/api/v1/runs/1")

            self.assertEqual(response["api_version"], "v1")
            self.assertIsNone(response["error"])
            data = cast(dict[str, object], response["data"])
            self.assertEqual(set(data), set(detail_fixture))
            self.assertEqual(data["run_id"], 1)

    def test_local_api_latest_returns_same_detail_shape_for_newest_run(self) -> None:
        with TemporaryDirectory() as tmpdir:
            sqlite_path = Path(tmpdir) / "tianji.sqlite3"
            self._persist_grouped_run(sqlite_path=sqlite_path)
            self._persist_sample_run(sqlite_path=sqlite_path)

            server, base_url = self._start_api_server(sqlite_path=sqlite_path)
            self.addCleanup(server.shutdown)
            self.addCleanup(server.server_close)

            latest_response = self._read_json(f"{base_url}/api/v1/runs/latest")
            direct_response = self._read_json(f"{base_url}/api/v1/runs/2")

            self.assertEqual(latest_response, direct_response)

    def test_local_api_compare_mirrors_history_compare_payload_vocabulary(self) -> None:
        compare_fixture = cast(
            dict[str, object], load_contract_fixture("history_compare_v1.json")
        )

        with TemporaryDirectory() as tmpdir:
            sqlite_path = Path(tmpdir) / "tianji.sqlite3"
            self._persist_grouped_run(sqlite_path=sqlite_path)
            self._persist_sample_run(sqlite_path=sqlite_path)

            server, base_url = self._start_api_server(sqlite_path=sqlite_path)
            self.addCleanup(server.shutdown)
            self.addCleanup(server.server_close)

            response = self._read_json(
                f"{base_url}/api/v1/compare?left_run_id=1&right_run_id=2"
            )

            self.assertEqual(response["api_version"], "v1")
            self.assertIsNone(response["error"])
            data = cast(dict[str, object], response["data"])
            self.assertEqual(set(data), set(compare_fixture))
            self.assertEqual(data["left_run_id"], 1)
            self.assertEqual(data["right_run_id"], 2)

    def test_local_api_returns_json_error_envelope_for_missing_run(self) -> None:
        with TemporaryDirectory() as tmpdir:
            sqlite_path = Path(tmpdir) / "tianji.sqlite3"
            self._persist_sample_run(sqlite_path=sqlite_path)

            server, base_url = self._start_api_server(sqlite_path=sqlite_path)
            self.addCleanup(server.shutdown)
            self.addCleanup(server.server_close)

            status, payload = self._read_json_error(f"{base_url}/api/v1/runs/99")

            self.assertEqual(status, 404)
            self.assertEqual(payload["api_version"], "v1")
            self.assertIsNone(payload["data"])
            error = cast(dict[str, object], payload["error"])
            self.assertEqual(error["code"], "run_not_found")
            self.assertEqual(error["message"], "Run not found: 99")

    def test_local_api_returns_json_error_envelope_for_malformed_compare_query(
        self,
    ) -> None:
        with TemporaryDirectory() as tmpdir:
            sqlite_path = Path(tmpdir) / "tianji.sqlite3"
            self._persist_grouped_run(sqlite_path=sqlite_path)
            self._persist_sample_run(sqlite_path=sqlite_path)

            server, base_url = self._start_api_server(sqlite_path=sqlite_path)
            self.addCleanup(server.shutdown)
            self.addCleanup(server.server_close)

            status, payload = self._read_json_error(
                f"{base_url}/api/v1/compare?left_run_id=1"
            )

            self.assertEqual(status, 400)
            self.assertEqual(payload["api_version"], "v1")
            self.assertIsNone(payload["data"])
            error = cast(dict[str, object], payload["error"])
            self.assertEqual(error["code"], "invalid_query")
            self.assertIn("Malformed compare query", cast(str, error["message"]))

    def test_queue_run_ack_stays_queued_when_job_record_mutates_before_response(
        self,
    ) -> None:
        queue_contract = cast(
            dict[str, object], load_contract_fixture("daemon_queue_request_v1.json")
        )

        with TemporaryDirectory() as tmpdir:
            socket_path = Path(tmpdir) / "tianji.sock"
            request_payload = dict(queue_contract)
            request_payload["fixture_paths"] = [str(FIXTURE_PATH)]

            server = create_server(socket_path=str(socket_path), host="127.0.0.1")
            original_enqueue = server.state.enqueue_job

            def enqueue_and_mutate(request: RunJobRequest) -> object:
                record = original_enqueue(request)
                record.state = "running"
                return record

            server.state.enqueue_job = enqueue_and_mutate  # type: ignore[method-assign]
            thread = threading.Thread(target=server.serve_forever, daemon=True)
            thread.start()
            self.addCleanup(server.shutdown)
            self.addCleanup(server.server_close)
            self.addCleanup(lambda: socket_path.unlink(missing_ok=True))

            queue_response = send_daemon_request(
                socket_path=str(socket_path),
                payload={"action": "queue_run", "payload": request_payload},
            )

            self.assertTrue(queue_response["ok"])
            queue_data = cast(dict[str, object], queue_response["data"])
            self.assertEqual(queue_data["state"], "queued")

    def test_daemon_can_queue_one_fixture_run_and_report_lifecycle(self) -> None:
        queue_contract = cast(
            dict[str, object], load_contract_fixture("daemon_queue_request_v1.json")
        )
        status_contract = cast(
            dict[str, object], load_contract_fixture("daemon_job_status_v1.json")
        )

        with TemporaryDirectory() as tmpdir:
            socket_path = Path(tmpdir) / "tianji.sock"
            sqlite_path = Path(tmpdir) / "tianji.sqlite3"
            request_payload = dict(queue_contract)
            request_payload["fixture_paths"] = [str(FIXTURE_PATH)]
            request_payload["sqlite_path"] = str(sqlite_path)

            server = create_server(socket_path=str(socket_path), host="127.0.0.1")
            thread = threading.Thread(target=server.serve_forever, daemon=True)
            thread.start()
            self.addCleanup(server.shutdown)
            self.addCleanup(server.server_close)
            self.addCleanup(lambda: socket_path.unlink(missing_ok=True))

            queue_response = send_daemon_request(
                socket_path=str(socket_path),
                payload={"action": "queue_run", "payload": request_payload},
            )
            self.assertTrue(queue_response["ok"])
            queue_data = cast(dict[str, object], queue_response["data"])
            self.assertEqual(queue_data["state"], "queued")
            job_id = queue_data["job_id"]
            self.assertIsInstance(job_id, str)

            seen_states: list[str] = []
            final_status: dict[str, object] | None = None
            for _ in range(100):
                status_response = send_daemon_request(
                    socket_path=str(socket_path),
                    payload={"action": "job_status", "job_id": job_id},
                )
                self.assertTrue(status_response["ok"])
                final_status = cast(dict[str, object], status_response["data"])
                state = cast(str, final_status["state"])
                seen_states.append(state)
                if state in {"succeeded", "failed"}:
                    break
                threading.Event().wait(0.02)

            self.assertIsNotNone(final_status)
            assert final_status is not None
            self.assertTrue(
                "running" in seen_states or final_status["state"] == "succeeded"
            )
            self.assertEqual(final_status["state"], "succeeded")
            self.assertEqual(set(final_status), set(status_contract))
            self.assertIn(cast(str, final_status["state"]), ALLOWED_JOB_STATES)
            self.assertIsNone(final_status["error"])
            self.assertIsInstance(final_status["run_id"], int)

            persisted_run_id = cast(int, final_status["run_id"])
            run_summary = storage.get_run_summary(
                sqlite_path=str(sqlite_path),
                run_id=persisted_run_id,
            )
            self.assertIsNotNone(run_summary)
            assert run_summary is not None
            self.assertEqual(run_summary["run_id"], persisted_run_id)
            self.assertEqual(run_summary["mode"], "fixture")
            self.assertEqual(run_summary["schema_version"], "tianji.run-artifact.v1")

    def test_daemon_rejects_non_loopback_host_cleanly(self) -> None:
        with TemporaryDirectory() as tmpdir:
            socket_path = Path(tmpdir) / "tianji.sock"
            with self.assertRaisesRegex(
                ValueError, "local-only and requires a loopback host"
            ):
                create_server(socket_path=str(socket_path), host="0.0.0.0")

    def test_serve_hosts_socket_and_http_api_together_on_plan_default_port(
        self,
    ) -> None:
        with TemporaryDirectory() as tmpdir:
            socket_path = Path(tmpdir) / "tianji.sock"
            sqlite_path = Path(tmpdir) / "tianji.sqlite3"
            port = 8875
            self._persist_sample_run(sqlite_path=sqlite_path)

            thread = threading.Thread(
                target=serve,
                kwargs={
                    "socket_path": str(socket_path),
                    "sqlite_path": str(sqlite_path),
                    "host": "127.0.0.1",
                    "port": port,
                },
                daemon=True,
            )
            thread.start()
            self._wait_for_path(socket_path)
            self.addCleanup(lambda: socket_path.unlink(missing_ok=True))

            meta_response = self._read_json(f"http://127.0.0.1:{port}/api/v1/meta")
            self.assertEqual(meta_response["api_version"], "v1")
            self.assertIsNone(meta_response["error"])

            status_response = send_daemon_request(
                socket_path=str(socket_path),
                payload={"action": "job_status", "job_id": "missing-job"},
            )
            self.assertFalse(status_response["ok"])
            error_payload = cast(dict[str, object], status_response["error"])
            self.assertIn("unknown job_id", cast(str, error_payload["message"]))

    def _persist_sample_run(self, *, sqlite_path: Path) -> None:
        run_pipeline(
            fixture_paths=[str(FIXTURE_PATH)],
            fetch=False,
            source_urls=[],
            output_path=str(sqlite_path.parent / f"sample-{sqlite_path.stem}.json"),
            sqlite_path=str(sqlite_path),
        )

    def _persist_grouped_run(self, *, sqlite_path: Path) -> None:
        grouped_fixture = sqlite_path.parent / "grouped.xml"
        grouped_fixture.write_text(_grouped_feed_xml(), encoding="utf-8")
        run_pipeline(
            fixture_paths=[str(grouped_fixture)],
            fetch=False,
            source_urls=[],
            output_path=str(sqlite_path.parent / f"grouped-{sqlite_path.stem}.json"),
            sqlite_path=str(sqlite_path),
        )

    def _start_api_server(
        self, *, sqlite_path: Path
    ) -> tuple[ThreadingHTTPServer, str]:
        server = create_api_server(
            sqlite_path=str(sqlite_path),
            host="127.0.0.1",
            port=0,
        )
        thread = threading.Thread(target=server.serve_forever, daemon=True)
        thread.start()
        host = cast(str, server.server_address[0])
        port = cast(int, server.server_address[1])
        return server, f"http://{host}:{port}"

    def _read_json(self, url: str) -> dict[str, object]:
        with urlopen(url) as response:
            return cast(dict[str, object], json.loads(response.read().decode("utf-8")))

    def _read_json_error(self, url: str) -> tuple[int, dict[str, object]]:
        with self.assertRaises(HTTPError) as error_context:
            urlopen(url)
        response = error_context.exception
        payload = cast(dict[str, object], json.loads(response.read().decode("utf-8")))
        return response.code, payload

    def _wait_for_path(self, path: Path) -> None:
        deadline = time.monotonic() + 2.0
        while time.monotonic() < deadline:
            if path.exists():
                return
            time.sleep(0.02)
        self.fail(f"Timed out waiting for path: {path}")


def _grouped_feed_xml() -> str:
    return """<?xml version="1.0" encoding="UTF-8"?>
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
