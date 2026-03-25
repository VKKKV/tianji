from __future__ import annotations

import json
import os
from pathlib import Path
import signal
import subprocess
import sys
import time
from contextlib import suppress
from urllib.error import HTTPError, URLError
from urllib.request import urlopen
from typing import cast

import click

from .cli_sources import _resolve_run_request
from .cli_validation import _validate_schedule_spec
from .daemon import (
    ALLOWED_JOB_STATES,
    DEFAULT_HTTP_API_PORT,
    DEFAULT_SQLITE_PATH,
    send_daemon_request,
)


DEFAULT_DAEMON_SOCKET_PATH = "runs/tianji.sock"
DEFAULT_DAEMON_HOST = "127.0.0.1"
DEFAULT_DAEMON_SQLITE_PATH = DEFAULT_SQLITE_PATH
DEFAULT_DAEMON_PORT = DEFAULT_HTTP_API_PORT
DEFAULT_DAEMON_POLL_INTERVAL_SECONDS = 0.1
DEFAULT_DAEMON_START_TIMEOUT_SECONDS = 2.0
DEFAULT_DAEMON_STOP_TIMEOUT_SECONDS = 2.0


def _pid_file_for_socket(socket_path: str) -> Path:
    socket_file = Path(socket_path)
    return socket_file.with_name(f"{socket_file.name}.pid")


def _read_pid_file(socket_path: str) -> int | None:
    pid_file = _pid_file_for_socket(socket_path)
    if not pid_file.exists():
        return None
    raw_value = pid_file.read_text(encoding="utf-8").strip()
    if not raw_value:
        return None
    try:
        return int(raw_value)
    except ValueError as error:
        raise click.UsageError(f"Daemon pid file is malformed: {pid_file}") from error


def _write_pid_file(socket_path: str, *, pid: int) -> None:
    pid_file = _pid_file_for_socket(socket_path)
    pid_file.parent.mkdir(parents=True, exist_ok=True)
    pid_file.write_text(f"{pid}\n", encoding="utf-8")


def _remove_pid_file(socket_path: str) -> None:
    pid_file = _pid_file_for_socket(socket_path)
    try:
        pid_file.unlink()
    except FileNotFoundError:
        return


def _is_pid_running(pid: int) -> bool:
    with suppress(ChildProcessError):
        waited_pid, _status = os.waitpid(pid, os.WNOHANG)
        if waited_pid == pid:
            return False
    try:
        os.kill(pid, 0)
    except ProcessLookupError:
        return False
    except PermissionError:
        return True
    return True


def _wait_for_socket(socket_path: str, *, timeout_seconds: float) -> bool:
    deadline = time.monotonic() + timeout_seconds
    socket_file = Path(socket_path)
    while time.monotonic() < deadline:
        if socket_file.exists():
            return True
        time.sleep(DEFAULT_DAEMON_POLL_INTERVAL_SECONDS)
    return False


def _wait_for_api(*, host: str, port: int, timeout_seconds: float) -> bool:
    deadline = time.monotonic() + timeout_seconds
    url = f"http://{host}:{port}/api/v1/meta"
    while time.monotonic() < deadline:
        try:
            with urlopen(url, timeout=0.5) as response:
                if response.status == 200:
                    return True
        except (HTTPError, URLError, ConnectionError, OSError):
            time.sleep(DEFAULT_DAEMON_POLL_INTERVAL_SECONDS)
    return False


def _send_daemon_payload(
    *, socket_path: str, payload: dict[str, object]
) -> dict[str, object]:
    try:
        response = send_daemon_request(socket_path=socket_path, payload=payload)
    except FileNotFoundError as error:
        raise click.UsageError(
            f"Daemon socket not found: {socket_path}. Start the daemon first."
        ) from error
    except ConnectionRefusedError as error:
        raise click.UsageError(
            f"Daemon socket refused connection: {socket_path}."
        ) from error
    except OSError as error:
        raise click.UsageError(f"Failed to contact daemon: {error}") from error

    ok = response.get("ok")
    if ok is not True:
        error_payload = response.get("error")
        if isinstance(error_payload, dict):
            message = error_payload.get("message")
            if isinstance(message, str) and message.strip():
                raise click.UsageError(message)
        raise click.UsageError("Daemon returned an invalid error response.")
    return response


def _handle_daemon_start(
    *, socket_path: str, sqlite_path: str, host: str, port: int
) -> int:
    existing_pid = _read_pid_file(socket_path)
    if existing_pid is not None and _is_pid_running(existing_pid):
        raise click.UsageError(
            f"Daemon already appears to be running for {socket_path} with pid {existing_pid}."
        )
    if existing_pid is not None and not _is_pid_running(existing_pid):
        _remove_pid_file(socket_path)

    socket_file = Path(socket_path)
    socket_file.parent.mkdir(parents=True, exist_ok=True)
    process = subprocess.Popen(
        [
            sys.executable,
            "-m",
            "tianji.daemon",
            "--socket-path",
            socket_path,
            "--sqlite-path",
            sqlite_path,
            "--host",
            host,
            "--port",
            str(port),
        ],
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
        start_new_session=True,
    )
    _write_pid_file(socket_path, pid=process.pid)
    if not _wait_for_socket(
        socket_path, timeout_seconds=DEFAULT_DAEMON_START_TIMEOUT_SECONDS
    ):
        _remove_pid_file(socket_path)
        process.terminate()
        raise click.UsageError(
            f"Daemon did not become ready within {DEFAULT_DAEMON_START_TIMEOUT_SECONDS:.1f}s for socket {socket_path}."
        )
    if not _wait_for_api(
        host=host,
        port=port,
        timeout_seconds=DEFAULT_DAEMON_START_TIMEOUT_SECONDS,
    ):
        _remove_pid_file(socket_path)
        process.terminate()
        raise click.UsageError(
            f"Daemon HTTP API did not become ready within {DEFAULT_DAEMON_START_TIMEOUT_SECONDS:.1f}s at http://{host}:{port}/api/v1/meta."
        )

    payload = {
        "socket_path": socket_path,
        "sqlite_path": sqlite_path,
        "pid": process.pid,
        "host": host,
        "port": port,
        "api_base_url": f"http://{host}:{port}",
        "running": True,
    }
    click.echo(json.dumps(payload, ensure_ascii=False, indent=2))
    return 0


def _handle_daemon_stop(*, socket_path: str) -> int:
    pid = _read_pid_file(socket_path)
    if pid is None:
        raise click.UsageError(
            f"No daemon pid file found for socket {socket_path}. Start the daemon first."
        )
    if not _is_pid_running(pid):
        _remove_pid_file(socket_path)
        raise click.UsageError(f"Daemon pid {pid} is not running.")

    os.kill(pid, signal.SIGTERM)
    deadline = time.monotonic() + DEFAULT_DAEMON_STOP_TIMEOUT_SECONDS
    while time.monotonic() < deadline:
        if not _is_pid_running(pid):
            _remove_pid_file(socket_path)
            with suppress(FileNotFoundError):
                Path(socket_path).unlink()
            click.echo(
                json.dumps(
                    {
                        "socket_path": socket_path,
                        "pid": pid,
                        "running": False,
                    },
                    ensure_ascii=False,
                    indent=2,
                )
            )
            return 0
        time.sleep(DEFAULT_DAEMON_POLL_INTERVAL_SECONDS)

    os.kill(pid, signal.SIGKILL)
    deadline = time.monotonic() + DEFAULT_DAEMON_STOP_TIMEOUT_SECONDS
    while time.monotonic() < deadline:
        if not _is_pid_running(pid):
            _remove_pid_file(socket_path)
            with suppress(FileNotFoundError):
                Path(socket_path).unlink()
            click.echo(
                json.dumps(
                    {
                        "socket_path": socket_path,
                        "pid": pid,
                        "running": False,
                    },
                    ensure_ascii=False,
                    indent=2,
                )
            )
            return 0
        time.sleep(DEFAULT_DAEMON_POLL_INTERVAL_SECONDS)

    raise click.UsageError(
        f"Daemon pid {pid} did not stop within {DEFAULT_DAEMON_STOP_TIMEOUT_SECONDS:.1f}s."
    )


def _handle_daemon_status(*, socket_path: str, job_id: str | None) -> int:
    pid = _read_pid_file(socket_path)
    if job_id is None:
        payload = {
            "socket_path": socket_path,
            "pid": pid,
            "running": bool(
                pid is not None and _is_pid_running(pid) and Path(socket_path).exists()
            ),
            "job_states": sorted(ALLOWED_JOB_STATES),
        }
        click.echo(json.dumps(payload, ensure_ascii=False, indent=2))
        return 0

    response = _send_daemon_payload(
        socket_path=socket_path,
        payload={"action": "job_status", "job_id": job_id},
    )
    click.echo(json.dumps(response["data"], ensure_ascii=False, indent=2))
    return 0


def _handle_daemon_run(
    *,
    socket_path: str,
    fixture: tuple[str, ...],
    fetch: bool,
    source_url: tuple[str, ...],
    source_config: str | None,
    source_name: tuple[str, ...],
    fetch_policy: str | None,
    output: str | None,
    sqlite_path: str | None,
) -> int:
    request_payload = _resolve_run_request(
        fixture=fixture,
        fetch=fetch,
        source_url=source_url,
        source_config=source_config,
        source_name=source_name,
        fetch_policy=fetch_policy,
        output=output,
        sqlite_path=sqlite_path,
    )
    response = _send_daemon_payload(
        socket_path=socket_path,
        payload={"action": "queue_run", "payload": request_payload},
    )
    click.echo(json.dumps(response["data"], ensure_ascii=False, indent=2))
    return 0


def _handle_daemon_schedule(
    *,
    socket_path: str,
    every_seconds: int,
    count: int,
    fixture: tuple[str, ...],
    fetch: bool,
    source_url: tuple[str, ...],
    source_config: str | None,
    source_name: tuple[str, ...],
    fetch_policy: str | None,
    output: str | None,
    sqlite_path: str | None,
) -> int:
    _validate_schedule_spec(every_seconds=every_seconds, count=count)
    request_payload = _resolve_run_request(
        fixture=fixture,
        fetch=fetch,
        source_url=source_url,
        source_config=source_config,
        source_name=source_name,
        fetch_policy=fetch_policy,
        output=output,
        sqlite_path=sqlite_path,
    )
    queued_runs: list[dict[str, object]] = []
    for index in range(count):
        response = _send_daemon_payload(
            socket_path=socket_path,
            payload={"action": "queue_run", "payload": request_payload},
        )
        data = cast(dict[str, object], response["data"])
        queued_runs.append(data)
        if index < count - 1:
            time.sleep(every_seconds)
    click.echo(
        json.dumps(
            {
                "schedule": {
                    "every_seconds": every_seconds,
                    "count": count,
                },
                "queued_runs": queued_runs,
                "job_states": sorted(ALLOWED_JOB_STATES),
            },
            ensure_ascii=False,
            indent=2,
        )
    )
    return 0
