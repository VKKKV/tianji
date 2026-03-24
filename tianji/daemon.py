from __future__ import annotations

from collections import deque
from collections.abc import Sequence
from dataclasses import dataclass, field
import argparse
from contextlib import suppress
from http.server import ThreadingHTTPServer
import json
from pathlib import Path
import socketserver
import threading
import uuid
from typing import cast

from .api import TianJiApiRequestHandler
from .pipeline import run_pipeline
from .storage import get_latest_run_id


ALLOWED_JOB_STATES = {"queued", "running", "succeeded", "failed"}
LOOPBACK_HOSTS = {"127.0.0.1", "localhost", "::1"}
DEFAULT_HTTP_API_PORT = 8765
DEFAULT_SQLITE_PATH = "runs/tianji.sqlite3"


def validate_loopback_host(host: str) -> str:
    normalized_host = host.strip()
    if normalized_host not in LOOPBACK_HOSTS:
        raise ValueError(
            f"TianJi daemon is local-only and requires a loopback host; got '{host}'."
        )
    return normalized_host


@dataclass(slots=True)
class RunJobRequest:
    fixture_paths: list[str] = field(default_factory=list)
    fetch: bool = False
    source_urls: list[str] = field(default_factory=list)
    fetch_policy: str = "always"
    source_fetch_details: list[dict[str, str]] = field(default_factory=list)
    output_path: str | None = None
    sqlite_path: str | None = None

    @classmethod
    def from_payload(cls, payload: dict[str, object]) -> RunJobRequest:
        fixture_paths = _coerce_string_list(
            payload.get("fixture_paths"), field_name="fixture_paths"
        )
        source_urls = _coerce_string_list(
            payload.get("source_urls"), field_name="source_urls"
        )
        fetch = payload.get("fetch", False)
        if not isinstance(fetch, bool):
            raise ValueError("queue request field 'fetch' must be a boolean")
        fetch_policy = payload.get("fetch_policy", "always")
        if not isinstance(fetch_policy, str):
            raise ValueError("queue request field 'fetch_policy' must be a string")
        output_path = _coerce_optional_string(
            payload.get("output_path"), field_name="output_path"
        )
        sqlite_path = _coerce_optional_string(
            payload.get("sqlite_path"), field_name="sqlite_path"
        )
        source_fetch_details = _coerce_source_fetch_details(
            payload.get("source_fetch_details", [])
        )
        return cls(
            fixture_paths=fixture_paths,
            fetch=fetch,
            source_urls=source_urls,
            fetch_policy=fetch_policy,
            source_fetch_details=source_fetch_details,
            output_path=output_path,
            sqlite_path=sqlite_path,
        )


@dataclass(slots=True)
class JobRecord:
    job_id: str
    state: str
    request: RunJobRequest
    run_id: int | None = None
    error: str | None = None

    def to_status_payload(self) -> dict[str, object]:
        payload: dict[str, object] = {
            "job_id": self.job_id,
            "state": self.state,
            "run_id": self.run_id,
            "error": self.error,
        }
        return payload


class DaemonState:
    def __init__(self) -> None:
        self._lock = threading.Lock()
        self._jobs: dict[str, JobRecord] = {}
        self._queue: deque[str] = deque()
        self._queue_event = threading.Event()
        self._stop_event = threading.Event()

    def enqueue_job(self, request: RunJobRequest) -> JobRecord:
        with self._lock:
            job_id = f"job-{uuid.uuid4().hex[:12]}"
            record = JobRecord(job_id=job_id, state="queued", request=request)
            self._jobs[job_id] = record
            self._queue.append(job_id)
            self._queue_event.set()
            return record

    def get_job(self, job_id: str) -> JobRecord | None:
        with self._lock:
            return self._jobs.get(job_id)

    def set_job_running(self, job_id: str) -> JobRecord:
        with self._lock:
            record = self._jobs[job_id]
            record.state = "running"
            return record

    def set_job_succeeded(self, job_id: str, *, run_id: int | None) -> None:
        with self._lock:
            record = self._jobs[job_id]
            record.state = "succeeded"
            record.run_id = run_id

    def set_job_failed(self, job_id: str, *, error: str) -> None:
        with self._lock:
            record = self._jobs[job_id]
            record.state = "failed"
            record.error = error

    def pop_next_job(self, *, timeout: float) -> JobRecord | None:
        while not self._stop_event.is_set():
            with self._lock:
                if self._queue:
                    job_id = self._queue.popleft()
                    if not self._queue:
                        self._queue_event.clear()
                    return self._jobs[job_id]
            self._queue_event.wait(timeout)
            self._queue_event.clear()
        return None

    def stop(self) -> None:
        self._stop_event.set()
        self._queue_event.set()


class TianJiUnixDaemonServer(
    socketserver.ThreadingMixIn, socketserver.UnixStreamServer
):
    daemon_threads = True
    block_on_close = False
    allow_reuse_address = True

    def __init__(
        self,
        server_address: str,
        handler_class: type[socketserver.StreamRequestHandler],
        *,
        host: str,
    ) -> None:
        self.host = validate_loopback_host(host)
        self.state = DaemonState()
        self._worker_thread = threading.Thread(
            target=self._worker_loop, name="tianji-daemon-worker"
        )
        super().__init__(server_address, handler_class)
        self._worker_thread.start()

    def server_close(self) -> None:
        self.state.stop()
        super().server_close()
        if self._worker_thread.is_alive():
            self._worker_thread.join(timeout=2)

    def _worker_loop(self) -> None:
        while not self.state._stop_event.is_set():
            record = self.state.pop_next_job(timeout=0.1)
            if record is None:
                continue
            self.state.set_job_running(record.job_id)
            try:
                run_pipeline(
                    fixture_paths=record.request.fixture_paths,
                    fetch=record.request.fetch,
                    source_urls=record.request.source_urls,
                    fetch_policy=record.request.fetch_policy,
                    source_fetch_details=record.request.source_fetch_details,
                    output_path=record.request.output_path,
                    sqlite_path=record.request.sqlite_path,
                )
                run_id = None
                if record.request.sqlite_path:
                    run_id = get_latest_run_id(sqlite_path=record.request.sqlite_path)
                self.state.set_job_succeeded(record.job_id, run_id=run_id)
            except Exception as exc:
                error_message = f"{exc.__class__.__name__}: {exc}"
                self.state.set_job_failed(record.job_id, error=error_message)


class TianJiHttpApiServer(ThreadingHTTPServer):
    daemon_threads = True

    def __init__(
        self, server_address: tuple[str, int], *, sqlite_path: str, host: str
    ) -> None:
        self.host = validate_loopback_host(host)
        self.sqlite_path = sqlite_path
        super().__init__(server_address, TianJiApiRequestHandler)


class TianJiDaemonRequestHandler(socketserver.StreamRequestHandler):
    def handle(self) -> None:
        raw_line = self.rfile.readline()
        if not raw_line:
            return
        try:
            request = json.loads(raw_line.decode("utf-8"))
            if not isinstance(request, dict):
                raise ValueError("request body must be a JSON object")
            response = self._dispatch(request)
        except Exception as exc:
            response = {
                "ok": False,
                "error": {
                    "message": f"{exc.__class__.__name__}: {exc}",
                },
            }
        self.wfile.write(json.dumps(response, ensure_ascii=False).encode("utf-8"))
        self.wfile.write(b"\n")

    def _dispatch(self, request: dict[str, object]) -> dict[str, object]:
        server = cast(TianJiUnixDaemonServer, self.server)
        action = request.get("action")
        if not isinstance(action, str):
            raise ValueError("request field 'action' must be a string")
        if action == "queue_run":
            payload = request.get("payload")
            if not isinstance(payload, dict):
                raise ValueError("queue_run requires an object 'payload'")
            run_request = RunJobRequest.from_payload(payload)
            record = server.state.enqueue_job(run_request)
            return {
                "ok": True,
                "data": {
                    "job_id": record.job_id,
                    "state": "queued",
                },
                "error": None,
            }
        if action == "job_status":
            job_id = request.get("job_id")
            if not isinstance(job_id, str):
                raise ValueError("job_status requires string field 'job_id'")
            record = server.state.get_job(job_id)
            if record is None:
                raise ValueError(f"unknown job_id '{job_id}'")
            return {"ok": True, "data": record.to_status_payload(), "error": None}
        raise ValueError(f"unsupported action '{action}'")


def send_daemon_request(
    *, socket_path: str, payload: dict[str, object]
) -> dict[str, object]:
    import socket

    with socket.socket(socket.AF_UNIX, socket.SOCK_STREAM) as client:
        client.connect(socket_path)
        client.sendall(json.dumps(payload, ensure_ascii=False).encode("utf-8") + b"\n")
        response = b""
        while not response.endswith(b"\n"):
            chunk = client.recv(4096)
            if not chunk:
                break
            response += chunk
    decoded = json.loads(response.decode("utf-8"))
    if not isinstance(decoded, dict):
        raise RuntimeError("daemon response was not a JSON object")
    return decoded


def create_server(
    *, socket_path: str, host: str = "127.0.0.1"
) -> TianJiUnixDaemonServer:
    validate_loopback_host(host)
    socket_file = Path(socket_path)
    socket_file.parent.mkdir(parents=True, exist_ok=True)
    with suppress(FileNotFoundError):
        socket_file.unlink()
    return TianJiUnixDaemonServer(
        str(socket_file),
        TianJiDaemonRequestHandler,
        host=host,
    )


def create_api_server(
    *, sqlite_path: str, host: str = "127.0.0.1", port: int = DEFAULT_HTTP_API_PORT
) -> TianJiHttpApiServer:
    validated_host = validate_loopback_host(host)
    return TianJiHttpApiServer(
        (validated_host, port),
        sqlite_path=sqlite_path,
        host=validated_host,
    )


def serve(
    *,
    socket_path: str,
    sqlite_path: str,
    host: str = "127.0.0.1",
    port: int = DEFAULT_HTTP_API_PORT,
) -> TianJiUnixDaemonServer:
    server = create_server(socket_path=socket_path, host=host)
    api_server = create_api_server(sqlite_path=sqlite_path, host=host, port=port)
    api_thread = threading.Thread(
        target=api_server.serve_forever,
        name="tianji-http-api-server",
    )
    api_thread.start()
    try:
        server.serve_forever()
    finally:
        api_server.shutdown()
        api_server.server_close()
        if api_thread.is_alive():
            api_thread.join(timeout=2)
        server.server_close()
        with suppress(FileNotFoundError):
            Path(socket_path).unlink()
    return server


def build_arg_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        description="Run the TianJi local UNIX-socket daemon."
    )
    parser.add_argument(
        "--socket-path", required=True, help="UNIX socket path for daemon control"
    )
    parser.add_argument(
        "--host",
        default="127.0.0.1",
        help="Loopback host marker used to enforce local-only startup",
    )
    parser.add_argument(
        "--port",
        type=int,
        default=DEFAULT_HTTP_API_PORT,
        help="Loopback HTTP API port",
    )
    parser.add_argument(
        "--sqlite-path",
        required=True,
        help="SQLite database path backing the loopback read API",
    )
    return parser


def main(argv: Sequence[str] | None = None) -> int:
    parser = build_arg_parser()
    args = parser.parse_args(argv)
    try:
        serve(
            socket_path=args.socket_path,
            sqlite_path=args.sqlite_path,
            host=args.host,
            port=args.port,
        )
    except ValueError as exc:
        parser.exit(status=2, message=f"{exc}\n")
    return 0


def _coerce_string_list(value: object, *, field_name: str) -> list[str]:
    if value is None:
        return []
    if not isinstance(value, list) or not all(isinstance(item, str) for item in value):
        raise ValueError(
            f"queue request field '{field_name}' must be a list of strings"
        )
    return list(value)


def _coerce_optional_string(value: object, *, field_name: str) -> str | None:
    if value is None:
        return None
    if not isinstance(value, str):
        raise ValueError(f"queue request field '{field_name}' must be a string or null")
    return value


def _coerce_source_fetch_details(value: object) -> list[dict[str, str]]:
    if not isinstance(value, list):
        raise ValueError("queue request field 'source_fetch_details' must be a list")
    details: list[dict[str, str]] = []
    for item in value:
        if not isinstance(item, dict):
            raise ValueError("each source_fetch_details item must be an object")
        detail: dict[str, str] = {}
        for key in ("name", "url", "fetch_policy"):
            field_value = item.get(key)
            if field_value is None:
                continue
            if not isinstance(field_value, str):
                raise ValueError(f"source_fetch_details field '{key}' must be a string")
            detail[key] = field_value
        details.append(detail)
    return details


if __name__ == "__main__":
    raise SystemExit(main())
