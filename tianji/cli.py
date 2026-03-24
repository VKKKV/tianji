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

from .daemon import (
    ALLOWED_JOB_STATES,
    DEFAULT_HTTP_API_PORT,
    DEFAULT_SQLITE_PATH,
    send_daemon_request,
)
from .fetch import TianJiInputError
from .pipeline import run_pipeline
from .storage import (
    compare_runs,
    get_latest_run_id,
    get_latest_run_pair,
    get_next_run_id,
    get_previous_run_id,
    get_run_summary,
    list_runs,
)
from .tui import launch_history_tui


FETCH_POLICY_CHOICES = ("always", "if-missing", "if-changed")
DEFAULT_DAEMON_SOCKET_PATH = "runs/tianji.sock"
DEFAULT_DAEMON_HOST = "127.0.0.1"
DEFAULT_DAEMON_SQLITE_PATH = DEFAULT_SQLITE_PATH
DEFAULT_DAEMON_PORT = DEFAULT_HTTP_API_PORT
DEFAULT_DAEMON_POLL_INTERVAL_SECONDS = 0.1
DEFAULT_DAEMON_START_TIMEOUT_SECONDS = 2.0
DEFAULT_DAEMON_STOP_TIMEOUT_SECONDS = 2.0


SourceConfigEntry = dict[str, str]
ResolvedSourceEntry = dict[str, str]


def validate_fetch_policy(*, value: object, context: str) -> str:
    if not isinstance(value, str) or not value.strip():
        raise ValueError(f"{context} must be a non-empty string.")
    normalized = value.strip().lower()
    if normalized not in FETCH_POLICY_CHOICES:
        allowed = ", ".join(FETCH_POLICY_CHOICES)
        raise ValueError(f"{context} must be one of: {allowed}.")
    return normalized


def load_source_registry(path: str) -> tuple[dict[str, SourceConfigEntry], str]:
    try:
        payload = json.loads(Path(path).read_text(encoding="utf-8"))
    except FileNotFoundError as error:
        raise ValueError(f"Source config file not found: {path}") from error
    except json.JSONDecodeError as error:
        raise ValueError(f"Source config is not valid JSON: {path}") from error

    sources = payload.get("sources")
    if not isinstance(sources, list) or not sources:
        raise ValueError("Source config must contain a non-empty 'sources' list.")

    default_fetch_policy = payload.get("default_fetch_policy", "always")
    validated_default_fetch_policy = validate_fetch_policy(
        value=default_fetch_policy,
        context="Source config 'default_fetch_policy'",
    )

    registry: dict[str, SourceConfigEntry] = {}
    for entry in sources:
        if not isinstance(entry, dict):
            raise ValueError("Each source entry must be an object.")
        name = entry.get("name")
        url = entry.get("url")
        if not isinstance(name, str) or not name.strip():
            raise ValueError(
                "Each source entry must include a non-empty string 'name'."
            )
        if not isinstance(url, str) or not url.strip():
            raise ValueError("Each source entry must include a non-empty string 'url'.")
        if name in registry:
            raise ValueError(f"Duplicate source name in config: {name}")
        source_fetch_policy = entry.get("fetch_policy", validated_default_fetch_policy)
        validated_source_fetch_policy = validate_fetch_policy(
            value=source_fetch_policy,
            context=f"Source '{name}' fetch_policy",
        )
        registry[name] = {
            "name": name,
            "url": url,
            "fetch_policy": validated_source_fetch_policy,
        }

    return registry, validated_default_fetch_policy


def resolve_sources(
    *,
    registry: dict[str, SourceConfigEntry],
    selected_names: list[str],
) -> list[ResolvedSourceEntry]:
    if not selected_names:
        return [dict(entry) for entry in registry.values()]

    missing = [name for name in selected_names if name not in registry]
    if missing:
        names = ", ".join(sorted(missing))
        raise ValueError(f"Unknown source name(s) in config selection: {names}")

    return [dict(registry[name]) for name in selected_names]


def dedupe_sources(sources: list[ResolvedSourceEntry]) -> list[ResolvedSourceEntry]:
    seen: set[str] = set()
    deduped: list[ResolvedSourceEntry] = []
    for source in sources:
        url = source["url"]
        if url in seen:
            continue
        seen.add(url)
        deduped.append(source)
    return deduped


def validate_score_range(
    *,
    min_value: float | None,
    max_value: float | None,
    min_flag: str,
    max_flag: str,
) -> None:
    if min_value is not None and max_value is not None and min_value > max_value:
        raise click.UsageError(f"{min_flag} cannot be greater than {max_flag}.")


def validate_positive_run_id(*, value: int | None, flag: str) -> None:
    if value is not None and value < 1:
        raise click.UsageError(f"{flag} must be greater than zero.")


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


def _resolve_run_request(
    *,
    fixture: tuple[str, ...],
    fetch: bool,
    source_url: tuple[str, ...],
    source_config: str | None,
    source_name: tuple[str, ...],
    fetch_policy: str | None,
    output: str | None,
    sqlite_path: str | None,
) -> dict[str, object]:
    fixture_paths = list(fixture)
    if not fixture_paths and not fetch:
        raise click.UsageError(
            "Provide at least one --fixture or enable --fetch with --source-url and/or --source-config."
        )

    resolved_sources: list[ResolvedSourceEntry] = [
        {
            "name": source_url_value,
            "url": source_url_value,
            "fetch_policy": "always",
        }
        for source_url_value in source_url
    ]
    if source_name and not source_config:
        raise click.UsageError("--source-name requires --source-config.")

    if source_config:
        try:
            registry, default_fetch_policy = load_source_registry(source_config)
            resolved_sources.extend(
                resolve_sources(
                    registry=registry,
                    selected_names=list(source_name),
                )
            )
        except ValueError as error:
            raise click.UsageError(str(error)) from error
    else:
        default_fetch_policy = "always"

    if fetch_policy is not None:
        resolved_fetch_policy = fetch_policy
        resolved_sources = [
            {
                **source,
                "fetch_policy": resolved_fetch_policy,
            }
            for source in resolved_sources
        ]
    else:
        resolved_fetch_policy = default_fetch_policy

    resolved_sources = dedupe_sources(resolved_sources)
    resolved_source_urls = [source["url"] for source in resolved_sources]

    if fetch and not resolved_source_urls:
        raise click.UsageError(
            "--fetch requires at least one source from --source-url or --source-config."
        )

    return {
        "fixture_paths": fixture_paths,
        "fetch": fetch,
        "source_urls": resolved_source_urls,
        "fetch_policy": resolved_fetch_policy,
        "source_fetch_details": resolved_sources,
        "output_path": output,
        "sqlite_path": sqlite_path,
    }


def _validate_schedule_spec(*, every_seconds: int, count: int) -> None:
    if every_seconds < 60:
        raise click.BadParameter(
            "must be an integer greater than or equal to 60.",
            param_hint="--every-seconds",
        )
    if count < 1:
        raise click.BadParameter("must be greater than zero.", param_hint="--count")


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


def _handle_run(
    *,
    fixture: tuple[str, ...],
    fetch: bool,
    source_url: tuple[str, ...],
    source_config: str | None,
    source_name: tuple[str, ...],
    fetch_policy: str | None,
    output: str,
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

    try:
        artifact = run_pipeline(
            fixture_paths=cast(list[str], request_payload["fixture_paths"]),
            fetch=cast(bool, request_payload["fetch"]),
            source_urls=cast(list[str], request_payload["source_urls"]),
            fetch_policy=cast(str, request_payload["fetch_policy"]),
            source_fetch_details=cast(
                list[dict[str, str]], request_payload["source_fetch_details"]
            ),
            output_path=cast(str | None, request_payload["output_path"]),
            sqlite_path=cast(str | None, request_payload["sqlite_path"]),
        )
    except TianJiInputError as error:
        raise click.UsageError(str(error)) from error

    click.echo(json.dumps(artifact.to_dict(), ensure_ascii=False, indent=2))
    click.echo(f"\nArtifact written to: {Path(output)}")
    return 0


def _handle_history(
    *,
    sqlite_path: str,
    limit: int,
    mode: str | None,
    dominant_field: str | None,
    risk_level: str | None,
    since: str | None,
    until: str | None,
    min_top_impact_score: float | None,
    max_top_impact_score: float | None,
    min_top_field_attraction: float | None,
    max_top_field_attraction: float | None,
    min_top_divergence_score: float | None,
    max_top_divergence_score: float | None,
    top_group_dominant_field: str | None,
    min_event_group_count: int | None,
    max_event_group_count: int | None,
) -> int:
    if limit < 0:
        raise click.UsageError("--limit must be zero or greater.")
    if min_event_group_count is not None and min_event_group_count < 0:
        raise click.UsageError("--min-event-group-count must be zero or greater.")
    if max_event_group_count is not None and max_event_group_count < 0:
        raise click.UsageError("--max-event-group-count must be zero or greater.")
    validate_score_range(
        min_value=min_top_impact_score,
        max_value=max_top_impact_score,
        min_flag="--min-top-impact-score",
        max_flag="--max-top-impact-score",
    )
    validate_score_range(
        min_value=min_top_field_attraction,
        max_value=max_top_field_attraction,
        min_flag="--min-top-field-attraction",
        max_flag="--max-top-field-attraction",
    )
    validate_score_range(
        min_value=min_top_divergence_score,
        max_value=max_top_divergence_score,
        min_flag="--min-top-divergence-score",
        max_flag="--max-top-divergence-score",
    )
    if (
        min_event_group_count is not None
        and max_event_group_count is not None
        and min_event_group_count > max_event_group_count
    ):
        raise click.UsageError(
            "--min-event-group-count cannot be greater than --max-event-group-count."
        )

    payload = list_runs(
        sqlite_path=sqlite_path,
        limit=limit,
        mode=mode,
        dominant_field=dominant_field,
        risk_level=risk_level,
        since=since,
        until=until,
        min_top_impact_score=min_top_impact_score,
        max_top_impact_score=max_top_impact_score,
        min_top_field_attraction=min_top_field_attraction,
        max_top_field_attraction=max_top_field_attraction,
        min_top_divergence_score=min_top_divergence_score,
        max_top_divergence_score=max_top_divergence_score,
        top_group_dominant_field=top_group_dominant_field,
        min_event_group_count=min_event_group_count,
        max_event_group_count=max_event_group_count,
    )
    click.echo(json.dumps(payload, ensure_ascii=False, indent=2))
    return 0


def _handle_history_show(
    *,
    sqlite_path: str,
    run_id: int | None,
    latest: bool,
    previous: bool,
    next_: bool,
    dominant_field: str | None,
    min_impact_score: float | None,
    max_impact_score: float | None,
    min_field_attraction: float | None,
    max_field_attraction: float | None,
    min_divergence_score: float | None,
    max_divergence_score: float | None,
    limit_scored_events: int | None,
    only_matching_interventions: bool,
    group_dominant_field: str | None,
    limit_event_groups: int | None,
) -> int:
    if sum([bool(latest), bool(previous), bool(next_)]) > 1:
        raise click.UsageError(
            "Use only one history-show navigation mode: --latest, --previous, or --next."
        )
    if limit_scored_events is not None and limit_scored_events < 0:
        raise click.UsageError("--limit-scored-events must be zero or greater.")
    if limit_event_groups is not None and limit_event_groups < 0:
        raise click.UsageError("--limit-event-groups must be zero or greater.")
    validate_score_range(
        min_value=min_impact_score,
        max_value=max_impact_score,
        min_flag="--min-impact-score",
        max_flag="--max-impact-score",
    )
    validate_score_range(
        min_value=min_field_attraction,
        max_value=max_field_attraction,
        min_flag="--min-field-attraction",
        max_flag="--max-field-attraction",
    )
    validate_score_range(
        min_value=min_divergence_score,
        max_value=max_divergence_score,
        min_flag="--min-divergence-score",
        max_flag="--max-divergence-score",
    )
    validate_positive_run_id(value=run_id, flag="--run-id")
    if latest and run_id is not None:
        raise click.UsageError(
            "Use either --run-id or --latest for history-show, not both."
        )
    if (previous or next_) and run_id is None:
        raise click.UsageError("history-show with --previous/--next requires --run-id.")
    if not latest and not previous and not next_ and run_id is None:
        raise click.UsageError(
            "history-show requires --run-id, --latest, --previous, or --next."
        )

    resolved_run_id = run_id
    if latest:
        resolved_run_id = get_latest_run_id(sqlite_path=sqlite_path)
        if resolved_run_id is None:
            raise click.UsageError("No persisted runs are available.")
    elif previous:
        asserted_run_id = cast(int, run_id)
        previous_run_id = get_previous_run_id(
            sqlite_path=sqlite_path, run_id=asserted_run_id
        )
        if previous_run_id is None:
            raise click.UsageError(
                f"No previous persisted run exists before run {asserted_run_id}."
            )
        resolved_run_id = previous_run_id
    elif next_:
        asserted_run_id = cast(int, run_id)
        next_run_id = get_next_run_id(sqlite_path=sqlite_path, run_id=asserted_run_id)
        if next_run_id is None:
            raise click.UsageError(
                f"No next persisted run exists after run {asserted_run_id}."
            )
        resolved_run_id = next_run_id

    final_run_id = cast(int, resolved_run_id)

    payload = get_run_summary(
        sqlite_path=sqlite_path,
        run_id=final_run_id,
        dominant_field=dominant_field,
        min_impact_score=min_impact_score,
        max_impact_score=max_impact_score,
        min_field_attraction=min_field_attraction,
        max_field_attraction=max_field_attraction,
        min_divergence_score=min_divergence_score,
        max_divergence_score=max_divergence_score,
        limit_scored_events=limit_scored_events,
        only_matching_interventions=only_matching_interventions,
        group_dominant_field=group_dominant_field,
        limit_event_groups=limit_event_groups,
    )
    if payload is None:
        raise click.UsageError(f"Run not found: {final_run_id}")

    click.echo(json.dumps(payload, ensure_ascii=False, indent=2))
    return 0


def _resolve_compare_run_ids(
    *,
    sqlite_path: str,
    left_run_id: int | None,
    right_run_id: int | None,
    latest_pair: bool,
    run_id: int | None,
    against_latest: bool,
    against_previous: bool,
) -> tuple[int, int]:
    validate_positive_run_id(value=run_id, flag="--run-id")
    validate_positive_run_id(value=left_run_id, flag="--left-run-id")
    validate_positive_run_id(value=right_run_id, flag="--right-run-id")

    mixed_pair_message = (
        "Use either --latest-pair, --run-id with --against-latest, --run-id with "
        "--against-previous, or explicit --left-run-id/--right-run-id, not a mix."
    )

    if latest_pair and (
        left_run_id is not None
        or right_run_id is not None
        or run_id is not None
        or against_latest
        or against_previous
    ):
        raise click.UsageError(mixed_pair_message)

    if latest_pair:
        pair = get_latest_run_pair(sqlite_path=sqlite_path)
        if pair is None:
            raise click.UsageError(
                "At least two persisted runs are required for --latest-pair."
            )
        return pair

    if against_latest:
        if against_previous:
            raise click.UsageError(
                "Use only one comparison preset: --against-latest or --against-previous."
            )
        if left_run_id is not None or right_run_id is not None:
            raise click.UsageError(mixed_pair_message)
        if run_id is None:
            raise click.UsageError(
                "history-compare with --against-latest requires --run-id."
            )
        latest_run_id = get_latest_run_id(sqlite_path=sqlite_path)
        if latest_run_id is None:
            raise click.UsageError("No persisted runs are available.")
        return run_id, latest_run_id

    if against_previous:
        if left_run_id is not None or right_run_id is not None:
            raise click.UsageError(mixed_pair_message)
        if run_id is None:
            raise click.UsageError(
                "history-compare with --against-previous requires --run-id."
            )
        previous_run_id = get_previous_run_id(sqlite_path=sqlite_path, run_id=run_id)
        if previous_run_id is None:
            raise click.UsageError(
                f"No previous persisted run exists before run {run_id}."
            )
        return previous_run_id, run_id

    if run_id is not None:
        raise click.UsageError(
            "Use --run-id only with --against-latest or --against-previous for history-compare."
        )
    if left_run_id is None or right_run_id is None:
        raise click.UsageError(
            "history-compare requires --latest-pair, --run-id with --against-latest, --run-id with --against-previous, or both --left-run-id and --right-run-id."
        )
    return left_run_id, right_run_id


def _handle_history_compare(
    *,
    sqlite_path: str,
    left_run_id: int | None,
    right_run_id: int | None,
    latest_pair: bool,
    run_id: int | None,
    against_latest: bool,
    against_previous: bool,
    dominant_field: str | None,
    min_impact_score: float | None,
    max_impact_score: float | None,
    min_field_attraction: float | None,
    max_field_attraction: float | None,
    min_divergence_score: float | None,
    max_divergence_score: float | None,
    limit_scored_events: int | None,
    only_matching_interventions: bool,
    group_dominant_field: str | None,
    limit_event_groups: int | None,
) -> int:
    if limit_scored_events is not None and limit_scored_events < 0:
        raise click.UsageError("--limit-scored-events must be zero or greater.")
    if limit_event_groups is not None and limit_event_groups < 0:
        raise click.UsageError("--limit-event-groups must be zero or greater.")
    validate_score_range(
        min_value=min_impact_score,
        max_value=max_impact_score,
        min_flag="--min-impact-score",
        max_flag="--max-impact-score",
    )
    validate_score_range(
        min_value=min_field_attraction,
        max_value=max_field_attraction,
        min_flag="--min-field-attraction",
        max_flag="--max-field-attraction",
    )
    validate_score_range(
        min_value=min_divergence_score,
        max_value=max_divergence_score,
        min_flag="--min-divergence-score",
        max_flag="--max-divergence-score",
    )

    resolved_left_run_id, resolved_right_run_id = _resolve_compare_run_ids(
        sqlite_path=sqlite_path,
        left_run_id=left_run_id,
        right_run_id=right_run_id,
        latest_pair=latest_pair,
        run_id=run_id,
        against_latest=against_latest,
        against_previous=against_previous,
    )

    payload = compare_runs(
        sqlite_path=sqlite_path,
        left_run_id=resolved_left_run_id,
        right_run_id=resolved_right_run_id,
        dominant_field=dominant_field,
        min_impact_score=min_impact_score,
        max_impact_score=max_impact_score,
        min_field_attraction=min_field_attraction,
        max_field_attraction=max_field_attraction,
        min_divergence_score=min_divergence_score,
        max_divergence_score=max_divergence_score,
        limit_scored_events=limit_scored_events,
        only_matching_interventions=only_matching_interventions,
        group_dominant_field=group_dominant_field,
        limit_event_groups=limit_event_groups,
    )
    if payload is None:
        raise click.UsageError(
            f"Run not found for comparison: {resolved_left_run_id} vs {resolved_right_run_id}"
        )

    click.echo(json.dumps(payload, ensure_ascii=False, indent=2))
    return 0


def _handle_tui(*, sqlite_path: str, limit: int) -> int:
    if limit < 0:
        raise click.UsageError("--limit must be zero or greater.")
    return launch_history_tui(sqlite_path=sqlite_path, limit=limit)


@click.group(
    context_settings={"help_option_names": ["-h", "--help"]},
    no_args_is_help=False,
    help="Synchronous one-shot runs plus thin local daemon controls.",
)
def cli() -> None:
    pass


@cli.command("run")
@click.option("--fixture", multiple=True, help="Path to a local RSS/Atom fixture file")
@click.option("--fetch", is_flag=True, help="Fetch one or more live feeds once")
@click.option("--source-url", multiple=True, help="Feed URL used with --fetch")
@click.option(
    "--source-config",
    default=None,
    help="Optional JSON file containing named source URLs",
)
@click.option(
    "--source-name",
    multiple=True,
    help="Optional source name from --source-config; repeat to select multiple",
)
@click.option(
    "--fetch-policy",
    type=click.Choice(FETCH_POLICY_CHOICES, case_sensitive=False),
    default=None,
    help=(
        "Optional one-run fetch policy override for all selected fetch sources; "
        "source-config defaults and per-source overrides use the same vocabulary"
    ),
)
@click.option(
    "--output",
    default="runs/latest-run.json",
    show_default=True,
    help="Path for the generated JSON artifact",
)
@click.option(
    "--sqlite-path",
    default=None,
    help="Optional SQLite database path for persisting run data",
)
def run_command(
    fixture: tuple[str, ...],
    fetch: bool,
    source_url: tuple[str, ...],
    source_config: str | None,
    source_name: tuple[str, ...],
    fetch_policy: str | None,
    output: str,
    sqlite_path: str | None,
) -> int:
    return _handle_run(
        fixture=fixture,
        fetch=fetch,
        source_url=source_url,
        source_config=source_config,
        source_name=source_name,
        fetch_policy=fetch_policy,
        output=output,
        sqlite_path=sqlite_path,
    )


@cli.group(
    "daemon",
    help="Daemon-backed start/status/stop/queue controls; use `run` for synchronous writes.",
)
def daemon_group() -> None:
    pass


@daemon_group.command("start")
@click.option(
    "--socket-path",
    default=DEFAULT_DAEMON_SOCKET_PATH,
    show_default=True,
    help="UNIX socket path for daemon control",
)
@click.option(
    "--host",
    default=DEFAULT_DAEMON_HOST,
    show_default=True,
    help="Loopback host marker passed to the daemon entrypoint",
)
@click.option(
    "--port",
    default=DEFAULT_DAEMON_PORT,
    show_default=True,
    help="Loopback HTTP API port",
)
@click.option(
    "--sqlite-path",
    default=DEFAULT_DAEMON_SQLITE_PATH,
    show_default=True,
    help="SQLite database path backing the loopback read API",
)
def daemon_start_command(
    socket_path: str, host: str, port: int, sqlite_path: str
) -> int:
    return _handle_daemon_start(
        socket_path=socket_path,
        sqlite_path=sqlite_path,
        host=host,
        port=port,
    )


@daemon_group.command("stop")
@click.option(
    "--socket-path",
    default=DEFAULT_DAEMON_SOCKET_PATH,
    show_default=True,
    help="UNIX socket path for daemon control",
)
def daemon_stop_command(socket_path: str) -> int:
    return _handle_daemon_stop(socket_path=socket_path)


@daemon_group.command("status")
@click.option(
    "--socket-path",
    default=DEFAULT_DAEMON_SOCKET_PATH,
    show_default=True,
    help="UNIX socket path for daemon control",
)
@click.option("--job-id", default=None, help="Optional queued daemon job identifier")
def daemon_status_command(socket_path: str, job_id: str | None) -> int:
    return _handle_daemon_status(socket_path=socket_path, job_id=job_id)


@daemon_group.command("run")
@click.option(
    "--socket-path",
    default=DEFAULT_DAEMON_SOCKET_PATH,
    show_default=True,
    help="UNIX socket path for daemon control",
)
@click.option("--fixture", multiple=True, help="Path to a local RSS/Atom fixture file")
@click.option("--fetch", is_flag=True, help="Fetch one or more live feeds once")
@click.option("--source-url", multiple=True, help="Feed URL used with --fetch")
@click.option(
    "--source-config",
    default=None,
    help="Optional JSON file containing named source URLs",
)
@click.option(
    "--source-name",
    multiple=True,
    help="Optional source name from --source-config; repeat to select multiple",
)
@click.option(
    "--fetch-policy",
    type=click.Choice(FETCH_POLICY_CHOICES, case_sensitive=False),
    default=None,
    help="Optional one-run fetch policy override for all selected fetch sources",
)
@click.option(
    "--output",
    default=None,
    help="Optional output path the daemon job should write when it runs",
)
@click.option(
    "--sqlite-path",
    default=None,
    help="Optional SQLite database path for persisting run data",
)
def daemon_run_command(
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
    return _handle_daemon_run(
        socket_path=socket_path,
        fixture=fixture,
        fetch=fetch,
        source_url=source_url,
        source_config=source_config,
        source_name=source_name,
        fetch_policy=fetch_policy,
        output=output,
        sqlite_path=sqlite_path,
    )


@daemon_group.command("schedule")
@click.option(
    "--socket-path",
    default=DEFAULT_DAEMON_SOCKET_PATH,
    show_default=True,
    help="UNIX socket path for daemon control",
)
@click.option(
    "--every-seconds",
    type=int,
    required=True,
    help="Bounded fixed interval between queue submissions",
)
@click.option(
    "--count",
    type=int,
    required=True,
    help="Total number of daemon queue submissions to make",
)
@click.option("--fixture", multiple=True, help="Path to a local RSS/Atom fixture file")
@click.option("--fetch", is_flag=True, help="Fetch one or more live feeds once")
@click.option("--source-url", multiple=True, help="Feed URL used with --fetch")
@click.option(
    "--source-config",
    default=None,
    help="Optional JSON file containing named source URLs",
)
@click.option(
    "--source-name",
    multiple=True,
    help="Optional source name from --source-config; repeat to select multiple",
)
@click.option(
    "--fetch-policy",
    type=click.Choice(FETCH_POLICY_CHOICES, case_sensitive=False),
    default=None,
    help="Optional one-run fetch policy override for all selected fetch sources",
)
@click.option(
    "--output",
    default=None,
    help="Optional output path each daemon job should write when it runs",
)
@click.option(
    "--sqlite-path",
    default=None,
    help="Optional SQLite database path for persisting run data",
)
def daemon_schedule_command(
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
    return _handle_daemon_schedule(
        socket_path=socket_path,
        every_seconds=every_seconds,
        count=count,
        fixture=fixture,
        fetch=fetch,
        source_url=source_url,
        source_config=source_config,
        source_name=source_name,
        fetch_policy=fetch_policy,
        output=output,
        sqlite_path=sqlite_path,
    )


@cli.command("history")
@click.option(
    "--sqlite-path",
    required=True,
    help="SQLite database path containing persisted TianJi runs",
)
@click.option(
    "--limit",
    type=int,
    default=20,
    show_default=True,
    help="Maximum number of runs to list",
)
@click.option(
    "--mode",
    default=None,
    help="Optional run mode filter (for example: fixture, fetch, fetch+fixture)",
)
@click.option(
    "--dominant-field",
    default=None,
    help="Optional dominant field filter for persisted runs",
)
@click.option(
    "--risk-level", default=None, help="Optional risk level filter for persisted runs"
)
@click.option(
    "--since",
    default=None,
    help="Optional inclusive lower bound for generated_at (ISO timestamp)",
)
@click.option(
    "--until",
    default=None,
    help="Optional inclusive upper bound for generated_at (ISO timestamp)",
)
@click.option(
    "--min-top-impact-score",
    type=float,
    default=None,
    help="Optional minimum impact_score for the persisted run's top scored event",
)
@click.option(
    "--max-top-impact-score",
    type=float,
    default=None,
    help="Optional maximum impact_score for the persisted run's top scored event",
)
@click.option(
    "--min-top-field-attraction",
    type=float,
    default=None,
    help="Optional minimum field_attraction for the persisted run's top scored event",
)
@click.option(
    "--max-top-field-attraction",
    type=float,
    default=None,
    help="Optional maximum field_attraction for the persisted run's top scored event",
)
@click.option(
    "--min-top-divergence-score",
    type=float,
    default=None,
    help="Optional minimum divergence_score for the persisted run's top scored event",
)
@click.option(
    "--max-top-divergence-score",
    type=float,
    default=None,
    help="Optional maximum divergence_score for the persisted run's top scored event",
)
@click.option(
    "--top-group-dominant-field",
    default=None,
    help="Optional dominant field filter for the persisted run's top event group",
)
@click.option(
    "--min-event-group-count",
    type=int,
    default=None,
    help="Optional minimum persisted event-group count for the run",
)
@click.option(
    "--max-event-group-count",
    type=int,
    default=None,
    help="Optional maximum persisted event-group count for the run",
)
def history_command(
    sqlite_path: str,
    limit: int,
    mode: str | None,
    dominant_field: str | None,
    risk_level: str | None,
    since: str | None,
    until: str | None,
    min_top_impact_score: float | None,
    max_top_impact_score: float | None,
    min_top_field_attraction: float | None,
    max_top_field_attraction: float | None,
    min_top_divergence_score: float | None,
    max_top_divergence_score: float | None,
    top_group_dominant_field: str | None,
    min_event_group_count: int | None,
    max_event_group_count: int | None,
) -> int:
    return _handle_history(
        sqlite_path=sqlite_path,
        limit=limit,
        mode=mode,
        dominant_field=dominant_field,
        risk_level=risk_level,
        since=since,
        until=until,
        min_top_impact_score=min_top_impact_score,
        max_top_impact_score=max_top_impact_score,
        min_top_field_attraction=min_top_field_attraction,
        max_top_field_attraction=max_top_field_attraction,
        min_top_divergence_score=min_top_divergence_score,
        max_top_divergence_score=max_top_divergence_score,
        top_group_dominant_field=top_group_dominant_field,
        min_event_group_count=min_event_group_count,
        max_event_group_count=max_event_group_count,
    )


@cli.command("history-show")
@click.option(
    "--sqlite-path",
    required=True,
    help="SQLite database path containing persisted TianJi runs",
)
@click.option("--run-id", type=int, default=None, help="Run identifier to inspect")
@click.option(
    "--latest",
    is_flag=True,
    help="Show the latest persisted run instead of specifying --run-id",
)
@click.option(
    "--previous",
    is_flag=True,
    help="Show the persisted run immediately before --run-id",
)
@click.option(
    "--next",
    "next_",
    is_flag=True,
    help="Show the persisted run immediately after --run-id",
)
@click.option(
    "--dominant-field",
    default=None,
    help="Optional dominant field filter for scored events inside the selected run",
)
@click.option(
    "--min-impact-score",
    type=float,
    default=None,
    help="Optional minimum impact_score for scored events inside the selected run",
)
@click.option(
    "--max-impact-score",
    type=float,
    default=None,
    help="Optional maximum impact_score for scored events inside the selected run",
)
@click.option(
    "--min-field-attraction",
    type=float,
    default=None,
    help="Optional minimum field_attraction for scored events inside the selected run",
)
@click.option(
    "--max-field-attraction",
    type=float,
    default=None,
    help="Optional maximum field_attraction for scored events inside the selected run",
)
@click.option(
    "--min-divergence-score",
    type=float,
    default=None,
    help="Optional minimum divergence_score for scored events inside the selected run",
)
@click.option(
    "--max-divergence-score",
    type=float,
    default=None,
    help="Optional maximum divergence_score for scored events inside the selected run",
)
@click.option(
    "--limit-scored-events",
    type=int,
    default=None,
    help="Optional maximum number of scored events to return for the selected run",
)
@click.option(
    "--only-matching-interventions",
    is_flag=True,
    help="Keep only intervention candidates whose event_id remains in the final visible scored-event set after filters and limits",
)
@click.option(
    "--group-dominant-field",
    default=None,
    help="Optional dominant field filter for persisted event groups inside the selected run",
)
@click.option(
    "--limit-event-groups",
    type=int,
    default=None,
    help="Optional maximum number of persisted event groups to return for the selected run",
)
def history_show_command(
    sqlite_path: str,
    run_id: int | None,
    latest: bool,
    previous: bool,
    next_: bool,
    dominant_field: str | None,
    min_impact_score: float | None,
    max_impact_score: float | None,
    min_field_attraction: float | None,
    max_field_attraction: float | None,
    min_divergence_score: float | None,
    max_divergence_score: float | None,
    limit_scored_events: int | None,
    only_matching_interventions: bool,
    group_dominant_field: str | None,
    limit_event_groups: int | None,
) -> int:
    return _handle_history_show(
        sqlite_path=sqlite_path,
        run_id=run_id,
        latest=latest,
        previous=previous,
        next_=next_,
        dominant_field=dominant_field,
        min_impact_score=min_impact_score,
        max_impact_score=max_impact_score,
        min_field_attraction=min_field_attraction,
        max_field_attraction=max_field_attraction,
        min_divergence_score=min_divergence_score,
        max_divergence_score=max_divergence_score,
        limit_scored_events=limit_scored_events,
        only_matching_interventions=only_matching_interventions,
        group_dominant_field=group_dominant_field,
        limit_event_groups=limit_event_groups,
    )


@cli.command("history-compare")
@click.option(
    "--sqlite-path",
    required=True,
    help="SQLite database path containing persisted TianJi runs",
)
@click.option(
    "--left-run-id",
    type=int,
    default=None,
    help="Left-side run identifier for comparison",
)
@click.option(
    "--right-run-id",
    type=int,
    default=None,
    help="Right-side run identifier for comparison",
)
@click.option(
    "--latest-pair",
    is_flag=True,
    help="Compare the two latest persisted runs instead of specifying run ids",
)
@click.option(
    "--run-id",
    type=int,
    default=None,
    help="Compare one explicit run against the latest persisted run",
)
@click.option(
    "--against-latest",
    is_flag=True,
    help="Use the latest persisted run as the right-hand side for comparison",
)
@click.option(
    "--against-previous",
    is_flag=True,
    help="Use the immediately previous persisted run as the left-hand side for comparison",
)
@click.option(
    "--dominant-field",
    default=None,
    help="Optional dominant field filter for scored events inside both compared runs",
)
@click.option(
    "--min-impact-score",
    type=float,
    default=None,
    help="Optional minimum impact_score for scored events inside both compared runs",
)
@click.option(
    "--max-impact-score",
    type=float,
    default=None,
    help="Optional maximum impact_score for scored events inside both compared runs",
)
@click.option(
    "--min-field-attraction",
    type=float,
    default=None,
    help="Optional minimum field_attraction for scored events inside both compared runs",
)
@click.option(
    "--max-field-attraction",
    type=float,
    default=None,
    help="Optional maximum field_attraction for scored events inside both compared runs",
)
@click.option(
    "--min-divergence-score",
    type=float,
    default=None,
    help="Optional minimum divergence_score for scored events inside both compared runs",
)
@click.option(
    "--max-divergence-score",
    type=float,
    default=None,
    help="Optional maximum divergence_score for scored events inside both compared runs",
)
@click.option(
    "--limit-scored-events",
    type=int,
    default=None,
    help="Optional maximum number of scored events to return for each compared run",
)
@click.option(
    "--only-matching-interventions",
    is_flag=True,
    help="Keep only intervention candidates whose event_id remains in the final visible scored-event set for each compared run",
)
@click.option(
    "--group-dominant-field",
    default=None,
    help="Optional dominant field filter for persisted event groups inside both compared runs",
)
@click.option(
    "--limit-event-groups",
    type=int,
    default=None,
    help="Optional maximum number of persisted event groups to return for each compared run",
)
def history_compare_command(
    sqlite_path: str,
    left_run_id: int | None,
    right_run_id: int | None,
    latest_pair: bool,
    run_id: int | None,
    against_latest: bool,
    against_previous: bool,
    dominant_field: str | None,
    min_impact_score: float | None,
    max_impact_score: float | None,
    min_field_attraction: float | None,
    max_field_attraction: float | None,
    min_divergence_score: float | None,
    max_divergence_score: float | None,
    limit_scored_events: int | None,
    only_matching_interventions: bool,
    group_dominant_field: str | None,
    limit_event_groups: int | None,
) -> int:
    return _handle_history_compare(
        sqlite_path=sqlite_path,
        left_run_id=left_run_id,
        right_run_id=right_run_id,
        latest_pair=latest_pair,
        run_id=run_id,
        against_latest=against_latest,
        against_previous=against_previous,
        dominant_field=dominant_field,
        min_impact_score=min_impact_score,
        max_impact_score=max_impact_score,
        min_field_attraction=min_field_attraction,
        max_field_attraction=max_field_attraction,
        min_divergence_score=min_divergence_score,
        max_divergence_score=max_divergence_score,
        limit_scored_events=limit_scored_events,
        only_matching_interventions=only_matching_interventions,
        group_dominant_field=group_dominant_field,
        limit_event_groups=limit_event_groups,
    )


@cli.command("tui")
@click.option(
    "--sqlite-path",
    required=True,
    help="SQLite database path containing persisted TianJi runs",
)
@click.option(
    "--limit",
    type=int,
    default=20,
    show_default=True,
    help="Maximum number of runs to load into the history browser",
)
def tui_command(sqlite_path: str, limit: int) -> int:
    return _handle_tui(sqlite_path=sqlite_path, limit=limit)


def main(argv: list[str] | None = None) -> int:
    try:
        result = cli.main(
            args=argv,
            prog_name="python -m tianji",
            standalone_mode=False,
        )
    except click.ClickException as error:
        error.show(file=sys.stderr)
        raise SystemExit(error.exit_code) from error
    except click.exceptions.Exit as error:
        raise SystemExit(error.exit_code) from error

    if isinstance(result, int):
        return result
    return 0
