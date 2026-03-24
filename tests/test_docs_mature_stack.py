from pathlib import Path
import unittest


class MatureStackDocsTests(unittest.TestCase):
    def read_text(self, relative_path: str) -> str:
        return Path(relative_path).read_text(encoding="utf-8")

    def assertContainsAll(self, text: str, substrings: list[str]) -> None:
        for substring in substrings:
            self.assertIn(substring, text)

    def test_required_docs_exist(self) -> None:
        for relative_path in [
            "README.md",
            "DEV_PLAN.md",
            "LOCAL_API_CONTRACT.md",
            "TUI_CONTRACT.md",
            "DAEMON_CONTRACT.md",
            "WEB_UI_CONTRACT.md",
        ]:
            self.assertTrue(Path(relative_path).exists(), relative_path)

    def test_readme_includes_operator_commands(self) -> None:
        text = self.read_text("README.md")
        self.assertContainsAll(
            text,
            [
                ".venv/bin/python -m tianji run --fixture tests/fixtures/sample_feed.xml",
                ".venv/bin/python -m tianji daemon status --socket-path runs/tianji.sock",
                ".venv/bin/python -m tianji daemon run --socket-path runs/tianji.sock --fixture tests/fixtures/sample_feed.xml",
                ".venv/bin/python -m tianji daemon schedule --socket-path runs/tianji.sock --every-seconds 300 --count 3 --fixture tests/fixtures/sample_feed.xml",
                "curl http://127.0.0.1:8765/api/v1/meta",
                "curl http://127.0.0.1:8765/api/v1/runs",
                'curl "http://127.0.0.1:8765/api/v1/compare?left_run_id=1&right_run_id=2"',
                ".venv/bin/python -m tianji.webui_server --api-base-url http://127.0.0.1:8765 --host 127.0.0.1 --port 8766",
            ],
        )

    def test_boundary_statements_remain_explicit(self) -> None:
        readme = self.read_text("README.md")
        local_api = self.read_text("LOCAL_API_CONTRACT.md")
        tui = self.read_text("TUI_CONTRACT.md")
        daemon = self.read_text("DAEMON_CONTRACT.md")
        web_ui = self.read_text("WEB_UI_CONTRACT.md")

        self.assertIn("source-of-truth write path", readme)
        self.assertIn("read-first and loopback-only", readme)
        self.assertIn("read-only terminal browser", tui)
        self.assertIn("CLI remains the write authority", daemon)
        self.assertIn("Optional and off by default", web_ui)
        self.assertIn("read-first", local_api)

    def test_stale_contradictions_are_absent(self) -> None:
        dev_plan = self.read_text("DEV_PLAN.md")
        agents = self.read_text("AGENTS.md")
        readme = self.read_text("README.md")

        self.assertNotIn("the local API remains future work", dev_plan)
        self.assertNotIn("no HTTP server ships in this repo yet", agents)
        self.assertNotIn("ships a **one-shot CLI MVP**", readme)


if __name__ == "__main__":
    unittest.main()
