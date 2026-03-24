from __future__ import annotations

from argparse import ArgumentParser
from http.server import SimpleHTTPRequestHandler, ThreadingHTTPServer
from http import HTTPStatus
import json
from pathlib import Path
from socketserver import BaseServer
import time
from urllib.error import HTTPError, URLError
from urllib.request import Request, urlopen
from typing import Any, Sequence, cast
from urllib.parse import urlencode, urlsplit

from .daemon import validate_loopback_host
from .daemon import send_daemon_request


WEBUI_DIR = Path(__file__).with_name("webui")
QUEUE_RUN_SOCKET_READY_TIMEOUT_SECONDS = 2.0
QUEUE_RUN_SOCKET_RETRY_INTERVAL_SECONDS = 0.05


class TianJiWebUiServer(ThreadingHTTPServer):
    api_base_url: str
    socket_path: str
    sqlite_path: str | None

    def __init__(
        self,
        server_address: tuple[str, int],
        handler: type[SimpleHTTPRequestHandler],
        *,
        api_base_url: str,
        socket_path: str,
        sqlite_path: str | None,
    ) -> None:
        self.api_base_url = api_base_url.rstrip("/")
        self.socket_path = socket_path
        self.sqlite_path = sqlite_path
        super().__init__(server_address, handler)


class TianJiWebUiRequestHandler(SimpleHTTPRequestHandler):
    def end_headers(self) -> None:
        self.send_header("Cache-Control", "no-store")
        super().end_headers()

    def do_GET(self) -> None:  # noqa: N802
        split = urlsplit(self.path)
        if split.path.startswith("/api/v1/") or split.path == "/api/v1":
            self._proxy_api_request(split_path=split.path, query=split.query)
            return
        if split.path in {"", "/"}:
            query = split.query
            location = "/index.html"
            if query:
                location = f"{location}?{query}"
            self.send_response(302)
            self.send_header("Location", location)
            self.end_headers()
            return
        return super().do_GET()

    def do_POST(self) -> None:  # noqa: N802
        split = urlsplit(self.path)
        if split.path == "/queue-run":
            self._handle_queue_run()
            return
        self.send_error(HTTPStatus.NOT_FOUND)

    def log_message(self, format: str, *args: object) -> None:
        return

    def _proxy_api_request(self, *, split_path: str, query: str) -> None:
        server = cast(TianJiWebUiServer, self.server)
        upstream_url = f"{server.api_base_url}{split_path}"
        if query:
            upstream_url = f"{upstream_url}?{query}"
        request = Request(
            upstream_url, method="GET", headers={"Accept": "application/json"}
        )
        try:
            with urlopen(request, timeout=5) as response:
                body = response.read()
                content_type = response.headers.get_content_type()
                charset = response.headers.get_content_charset() or "utf-8"
                self.send_response(response.status)
                self.send_header("Content-Type", f"{content_type}; charset={charset}")
                self.send_header("Content-Length", str(len(body)))
                self.end_headers()
                self.wfile.write(body)
        except HTTPError as exc:
            body = exc.read()
            charset = exc.headers.get_content_charset() or "utf-8"
            self.send_response(exc.code)
            self.send_header("Content-Type", f"application/json; charset={charset}")
            self.send_header("Content-Length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)
        except URLError:
            body = b'{"api_version":"v1","data":null,"error":{"code":"upstream_unavailable","message":"Optional web UI could not reach the local API."}}'
            self.send_response(HTTPStatus.BAD_GATEWAY)
            self.send_header("Content-Type", "application/json; charset=utf-8")
            self.send_header("Content-Length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)

    def _handle_queue_run(self) -> None:
        content_length = int(self.headers.get("Content-Length", "0"))
        body = self.rfile.read(content_length)
        try:
            decoded = json.loads(body.decode("utf-8")) if body else {}
            if not isinstance(decoded, dict):
                raise ValueError("request body must be an object")
            fixture_path = decoded.get("fixture_path")
            if not isinstance(fixture_path, str) or not fixture_path.strip():
                raise ValueError("fixture_path must be a non-empty string")
            server = cast(TianJiWebUiServer, self.server)
            response = _send_queue_run_request_with_retry(
                socket_path=server.socket_path,
                fixture_path=fixture_path.strip(),
                sqlite_path=server.sqlite_path,
            )
            response_body = json.dumps(response, ensure_ascii=False).encode("utf-8")
            status = HTTPStatus.OK if response.get("ok") else HTTPStatus.BAD_REQUEST
        except (ValueError, RuntimeError, OSError, json.JSONDecodeError) as exc:
            response_body = json.dumps(
                {
                    "ok": False,
                    "data": None,
                    "error": {
                        "message": f"{exc.__class__.__name__}: {exc}",
                    },
                },
                ensure_ascii=False,
            ).encode("utf-8")
            status = HTTPStatus.BAD_REQUEST
        self.send_response(status)
        self.send_header("Content-Type", "application/json; charset=utf-8")
        self.send_header("Content-Length", str(len(response_body)))
        self.end_headers()
        self.wfile.write(response_body)


def _send_queue_run_request_with_retry(
    *, socket_path: str, fixture_path: str, sqlite_path: str | None
) -> dict[str, object]:
    deadline = time.monotonic() + QUEUE_RUN_SOCKET_READY_TIMEOUT_SECONDS
    last_error: OSError | None = None
    run_payload: dict[str, object] = {
        "fixture_paths": [fixture_path],
    }
    if sqlite_path:
        run_payload["sqlite_path"] = sqlite_path
    payload = {
        "action": "queue_run",
        "payload": run_payload,
    }
    while True:
        try:
            return send_daemon_request(socket_path=socket_path, payload=payload)
        except FileNotFoundError as exc:
            last_error = exc
        except ConnectionRefusedError as exc:
            last_error = exc
        if time.monotonic() >= deadline:
            if last_error is not None:
                raise last_error
            raise RuntimeError("queue-run proxy could not reach daemon socket")
        time.sleep(QUEUE_RUN_SOCKET_RETRY_INTERVAL_SECONDS)


def create_webui_server(
    *,
    host: str,
    port: int,
    api_base_url: str,
    socket_path: str = "runs/tianji.sock",
    sqlite_path: str | None = None,
) -> TianJiWebUiServer:
    validated_host = validate_loopback_host(host)

    class _ConfiguredWebUiHandler(TianJiWebUiRequestHandler):
        def __init__(
            self, request: Any, client_address: Any, server: BaseServer
        ) -> None:
            super().__init__(
                request,
                client_address,
                server,
                directory=str(WEBUI_DIR),
            )

    return TianJiWebUiServer(
        (validated_host, port),
        _ConfiguredWebUiHandler,
        api_base_url=api_base_url,
        socket_path=socket_path,
        sqlite_path=sqlite_path,
    )


def build_arg_parser() -> ArgumentParser:
    parser = ArgumentParser(description="Serve the optional TianJi local web UI.")
    parser.add_argument(
        "--host",
        default="127.0.0.1",
        help="Loopback host for the optional local-only web UI",
    )
    parser.add_argument(
        "--port",
        type=int,
        default=8766,
        help="Loopback port for the optional local-only web UI",
    )
    parser.add_argument(
        "--api-base-url",
        default="http://127.0.0.1:8765",
        help="Loopback API base URL consumed by the thin browser client",
    )
    parser.add_argument(
        "--socket-path",
        default="runs/tianji.sock",
        help="UNIX socket path used by the optional queue-run proxy",
    )
    parser.add_argument(
        "--sqlite-path",
        default=None,
        help="Optional SQLite path forwarded to queued runs so web UI queueing persists into local history",
    )
    return parser


def main(argv: Sequence[str] | None = None) -> int:
    parser = build_arg_parser()
    args = parser.parse_args(argv)
    try:
        validate_loopback_host(args.host)
        server = create_webui_server(
            host=args.host,
            port=args.port,
            api_base_url=args.api_base_url,
            socket_path=args.socket_path,
            sqlite_path=args.sqlite_path,
        )
    except ValueError as exc:
        parser.exit(status=2, message=f"{exc}\n")

    base_query = urlencode({"api_base_url": args.api_base_url})
    print(
        f"TianJi optional web UI serving at http://{args.host}:{args.port}/index.html?{base_query}"
    )
    try:
        server.serve_forever()
    except KeyboardInterrupt:
        pass
    finally:
        server.server_close()
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
