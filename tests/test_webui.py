from support import *

import time
from urllib.request import urlopen
from urllib.request import Request

from tianji.daemon import create_api_server
from tianji.daemon import TianJiHttpApiServer
from tianji.daemon import create_server
from tianji.daemon import TianJiUnixDaemonServer
from tianji.webui_server import create_webui_server
from tianji.webui_server import TianJiWebUiServer


class WebUiTests(unittest.TestCase):
    def test_optional_webui_serves_static_shell_and_proxies_loopback_api(self) -> None:
        with TemporaryDirectory() as tmpdir:
            sqlite_path = Path(tmpdir) / "tianji.sqlite3"
            socket_path = Path(tmpdir) / "tianji.sock"
            self._persist_sample_run(sqlite_path=sqlite_path)
            self._persist_grouped_run(sqlite_path=sqlite_path)

            api_server, api_base_url = self._start_api_server(sqlite_path=sqlite_path)
            self.addCleanup(api_server.shutdown)
            self.addCleanup(api_server.server_close)

            daemon_server = create_server(
                socket_path=str(socket_path), host="127.0.0.1"
            )
            daemon_thread = threading.Thread(
                target=daemon_server.serve_forever, daemon=True
            )
            daemon_thread.start()
            self.addCleanup(daemon_server.shutdown)
            self.addCleanup(daemon_server.server_close)
            self.addCleanup(lambda: socket_path.unlink(missing_ok=True))
            self.addCleanup(daemon_thread.join, 1)

            webui_server, webui_base_url = self._start_webui_server(
                api_base_url=api_base_url,
                socket_path=str(socket_path),
                sqlite_path=str(sqlite_path),
            )
            self.addCleanup(webui_server.shutdown)
            self.addCleanup(webui_server.server_close)

            html = (
                urlopen(f"{webui_base_url}/index.html", timeout=5)
                .read()
                .decode("utf-8")
            )
            self.assertIn('data-testid="run-list"', html)
            self.assertIn('data-testid="run-detail-panel"', html)
            self.assertIn('data-testid="compare-left-run-id"', html)
            self.assertIn('data-testid="compare-right-run-id"', html)
            self.assertIn('data-testid="compare-load-button"', html)
            self.assertIn('data-testid="compare-panel"', html)
            self.assertIn('data-testid="queue-run-form"', html)
            self.assertIn('data-testid="queue-fixture-path"', html)
            self.assertIn('data-testid="queue-run-submit"', html)
            self.assertIn('data-testid="queue-run-status"', html)
            self.assertIn("optional and off by default", html)

            proxied_runs_payload = self._read_json(
                f"{webui_base_url}/api/v1/runs?limit=2"
            )
            self.assertEqual(proxied_runs_payload["api_version"], "v1")
            proxied_items = cast(dict[str, object], proxied_runs_payload["data"])[
                "items"
            ]
            self.assertEqual(len(cast(list[object], proxied_items)), 2)

            proxied_compare_payload = self._read_json(
                f"{webui_base_url}/api/v1/compare?left_run_id=1&right_run_id=2"
            )
            self.assertEqual(proxied_compare_payload["api_version"], "v1")
            compare_data = cast(dict[str, object], proxied_compare_payload["data"])
            self.assertEqual(compare_data["left_run_id"], 1)
            self.assertEqual(compare_data["right_run_id"], 2)

            queue_payload = self._post_json(
                f"{webui_base_url}/queue-run",
                {"fixture_path": str(FIXTURE_PATH)},
            )
            self.assertTrue(cast(bool, queue_payload["ok"]))
            queue_data = cast(dict[str, object], queue_payload["data"])
            self.assertEqual(queue_data["state"], "queued")
            self.assertIn("job_id", queue_data)
            self._wait_for_run_count(
                url=f"{webui_base_url}/api/v1/runs?limit=3", count=3
            )

    def test_queue_run_waits_briefly_for_clean_start_daemon_socket(self) -> None:
        with TemporaryDirectory() as tmpdir:
            sqlite_path = Path(tmpdir) / "tianji.sqlite3"
            socket_path = Path(tmpdir) / "tianji.sock"
            self._persist_sample_run(sqlite_path=sqlite_path)

            api_server, api_base_url = self._start_api_server(sqlite_path=sqlite_path)
            self.addCleanup(api_server.shutdown)
            self.addCleanup(api_server.server_close)

            webui_server, webui_base_url = self._start_webui_server(
                api_base_url=api_base_url,
                socket_path=str(socket_path),
                sqlite_path=str(sqlite_path),
            )
            self.addCleanup(webui_server.shutdown)
            self.addCleanup(webui_server.server_close)

            daemon_holder: dict[str, object] = {}

            def _delayed_start_daemon() -> None:
                time.sleep(0.2)
                daemon_server = create_server(
                    socket_path=str(socket_path), host="127.0.0.1"
                )
                daemon_thread = threading.Thread(
                    target=daemon_server.serve_forever, daemon=True
                )
                daemon_thread.start()
                daemon_holder["server"] = daemon_server
                daemon_holder["thread"] = daemon_thread

            starter_thread = threading.Thread(target=_delayed_start_daemon, daemon=True)
            starter_thread.start()
            self.addCleanup(starter_thread.join, 1)

            queue_payload = self._post_json(
                f"{webui_base_url}/queue-run",
                {"fixture_path": str(FIXTURE_PATH)},
            )
            starter_thread.join(timeout=1)
            daemon_server = daemon_holder.get("server")
            daemon_thread = daemon_holder.get("thread")
            self.assertIsInstance(daemon_server, TianJiUnixDaemonServer)
            self.assertIsInstance(daemon_thread, threading.Thread)
            self.addCleanup(cast(TianJiUnixDaemonServer, daemon_server).shutdown)
            self.addCleanup(cast(TianJiUnixDaemonServer, daemon_server).server_close)
            self.addCleanup(lambda: socket_path.unlink(missing_ok=True))
            self.addCleanup(cast(threading.Thread, daemon_thread).join, 1)

            self.assertTrue(cast(bool, queue_payload["ok"]))
            queue_data = cast(dict[str, object], queue_payload["data"])
            self.assertEqual(queue_data["state"], "queued")
            self.assertIn("job_id", queue_data)
            self._wait_for_run_count(
                url=f"{webui_base_url}/api/v1/runs?limit=2", count=2
            )

    def _persist_sample_run(self, *, sqlite_path: Path) -> None:
        artifact = run_pipeline(
            fixture_paths=[str(FIXTURE_PATH)],
            fetch=False,
            source_urls=[],
            output_path=None,
            sqlite_path=str(sqlite_path),
        )
        self.assertEqual(artifact.mode, "fixture")

    def _persist_grouped_run(self, *, sqlite_path: Path) -> None:
        grouped_feed = """<?xml version="1.0" encoding="UTF-8"?>
<rss version="2.0">
  <channel>
    <title>Grouped TianJi Feed</title>
    <item>
      <title>USA and China widen chip export controls after East Asia dispute</title>
      <link>https://example.com/group-a</link>
      <pubDate>Sun, 22 Mar 2026 09:00:00 GMT</pubDate>
      <description>China and USA extend technology controls across East Asia supply chains.</description>
    </item>
    <item>
      <title>China and USA expand chip controls across East Asia export lanes</title>
      <link>https://example.com/group-b</link>
      <pubDate>Sun, 22 Mar 2026 10:00:00 GMT</pubDate>
      <description>Technology export restrictions intensify after the East Asia dispute.</description>
    </item>
    <item>
      <title>Iran diplomacy channel reopens for regional talks</title>
      <link>https://example.com/group-c</link>
      <pubDate>Sun, 22 Mar 2026 10:00:00 GMT</pubDate>
      <description>Diplomacy talks resume in the Middle East after a regional dispute.</description>
    </item>
  </channel>
</rss>
"""
        with TemporaryDirectory() as tmpdir:
            fixture_path = Path(tmpdir) / "grouped.xml"
            fixture_path.write_text(grouped_feed, encoding="utf-8")
            artifact = run_pipeline(
                fixture_paths=[str(fixture_path)],
                fetch=False,
                source_urls=[],
                output_path=None,
                sqlite_path=str(sqlite_path),
            )
        self.assertEqual(artifact.mode, "fixture")

    def _start_api_server(
        self, *, sqlite_path: Path
    ) -> tuple[TianJiHttpApiServer, str]:
        server = create_api_server(
            sqlite_path=str(sqlite_path), host="127.0.0.1", port=0
        )
        thread = threading.Thread(target=server.serve_forever, daemon=True)
        thread.start()
        self.addCleanup(thread.join, 1)
        base_url = f"http://127.0.0.1:{server.server_port}"
        return server, base_url

    def _start_webui_server(
        self, *, api_base_url: str, socket_path: str, sqlite_path: str | None = None
    ) -> tuple[TianJiWebUiServer, str]:
        server = create_webui_server(
            host="127.0.0.1",
            port=0,
            api_base_url=api_base_url,
            socket_path=socket_path,
            sqlite_path=sqlite_path,
        )
        thread = threading.Thread(target=server.serve_forever, daemon=True)
        thread.start()
        self.addCleanup(thread.join, 1)
        base_url = f"http://127.0.0.1:{server.server_port}"
        return server, base_url

    def _read_json(self, url: str) -> dict[str, object]:
        with urlopen(url, timeout=5) as response:
            return cast(dict[str, object], json.loads(response.read().decode("utf-8")))

    def _post_json(self, url: str, payload: dict[str, object]) -> dict[str, object]:
        request = Request(
            url,
            data=json.dumps(payload).encode("utf-8"),
            headers={"Content-Type": "application/json"},
            method="POST",
        )
        with urlopen(request, timeout=5) as response:
            return cast(dict[str, object], json.loads(response.read().decode("utf-8")))

    def _wait_for_run_count(self, *, url: str, count: int) -> None:
        for _ in range(100):
            payload = self._read_json(url)
            items = cast(dict[str, object], payload["data"])["items"]
            if len(cast(list[object], items)) >= count:
                return
            time.sleep(0.05)
        self.fail(f"Timed out waiting for {count} persisted runs at {url}")
