from support import *

import shutil
import subprocess
from urllib.request import Request
from urllib.request import urlopen

from tianji.daemon import create_api_server
from tianji.daemon import create_server
from tianji.daemon import TianJiHttpApiServer
from tianji.webui_server import create_webui_server
from tianji.webui_server import TianJiWebUiServer


class WebUiBrowserTests(unittest.TestCase):
    def test_optional_webui_browser_flow_via_playwright(self) -> None:
        if shutil.which("npx") is None:
            self.skipTest("npx is required for local Playwright browser verification")

        repo_root = Path(__file__).resolve().parent.parent

        with TemporaryDirectory() as tmpdir:
            sqlite_path = Path(tmpdir) / "tianji.sqlite3"
            socket_path = Path(tmpdir) / "tianji.sock"
            script_path = Path(tmpdir) / "webui-browser-smoke.mjs"

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
            )
            self.addCleanup(webui_server.shutdown)
            self.addCleanup(webui_server.server_close)

            try:
                subprocess.run(
                    [
                        "npx",
                        "--yes",
                        "-p",
                        "playwright",
                        "node",
                        "--eval",
                        'import("playwright").then(() => process.exit(0))',
                    ],
                    check=True,
                    capture_output=True,
                    text=True,
                    cwd=repo_root,
                    timeout=60,
                )
            except (subprocess.CalledProcessError, subprocess.TimeoutExpired):
                self.skipTest(
                    "Playwright package is not available locally via npx for browser verification"
                )

            script_path.write_text(
                self._browser_script(base_url=webui_base_url),
                encoding="utf-8",
            )
            result = subprocess.run(
                [
                    "npx",
                    "--yes",
                    "-p",
                    "playwright",
                    "node",
                    str(script_path),
                ],
                check=False,
                capture_output=True,
                text=True,
                cwd=repo_root,
                timeout=120,
            )
            if result.returncode != 0:
                self.fail(
                    "Playwright browser verification failed:\n"
                    f"STDOUT:\n{result.stdout}\nSTDERR:\n{result.stderr}"
                )

    def _browser_script(self, *, base_url: str) -> str:
        return f"""
import {{ chromium }} from 'playwright';

const browser = await chromium.launch({{ headless: true }});
const page = await browser.newPage();
try {{
  await page.goto('{base_url}/index.html');

  await page.waitForSelector('[data-testid="run-list"]');
  await page.waitForSelector('[data-testid="run-row-1"]');
  await page.getByTestId('run-row-1').locator('button').click();
  await page.waitForFunction(() => document.querySelector('[data-testid="run-detail-panel"]')?.textContent?.includes('Run #1'));
  await page.waitForFunction(() => document.querySelector('[data-testid="intervention-list"]')?.textContent?.includes('capability-control'));

  await page.getByTestId('compare-left-run-id').fill('1');
  await page.getByTestId('compare-right-run-id').fill('2');
  await page.getByTestId('compare-load-button').click();
  await page.waitForFunction(() => document.querySelector('[data-testid="compare-panel"]')?.textContent?.includes('Diff summary'));
}} finally {{
  await browser.close();
}}
"""

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
        self, *, api_base_url: str, socket_path: str
    ) -> tuple[TianJiWebUiServer, str]:
        server = create_webui_server(
            host="127.0.0.1",
            port=0,
            api_base_url=api_base_url,
            socket_path=socket_path,
        )
        thread = threading.Thread(target=server.serve_forever, daemon=True)
        thread.start()
        self.addCleanup(thread.join, 1)
        base_url = f"http://127.0.0.1:{server.server_port}"
        return server, base_url
