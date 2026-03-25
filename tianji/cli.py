from __future__ import annotations

import json
import importlib
from pathlib import Path
import sys
from typing import Any, cast

import click

from .cli_validation import (
    _validate_schedule_spec,
)
from .cli_sources import (
    FETCH_POLICY_CHOICES,
    _resolve_run_request,
    load_source_registry,
    resolve_sources,
)
from .fetch import TianJiInputError
from .pipeline import run_pipeline
from .tui import launch_history_tui


DEFAULT_DAEMON_SOCKET_PATH = "runs/tianji.sock"
DEFAULT_DAEMON_HOST = "127.0.0.1"
DEFAULT_DAEMON_SQLITE_PATH = "runs/tianji.sqlite3"
DEFAULT_DAEMON_PORT = 8765


def _cli_daemon_module() -> Any:
    return importlib.import_module("tianji.cli_daemon")


def _cli_history_module() -> Any:
    return importlib.import_module("tianji.cli_history")


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
    daemon_module = _cli_daemon_module()
    return daemon_module._handle_daemon_start(
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
    daemon_module = _cli_daemon_module()
    return daemon_module._handle_daemon_stop(socket_path=socket_path)


@daemon_group.command("status")
@click.option(
    "--socket-path",
    default=DEFAULT_DAEMON_SOCKET_PATH,
    show_default=True,
    help="UNIX socket path for daemon control",
)
@click.option("--job-id", default=None, help="Optional queued daemon job identifier")
def daemon_status_command(socket_path: str, job_id: str | None) -> int:
    daemon_module = _cli_daemon_module()
    return daemon_module._handle_daemon_status(socket_path=socket_path, job_id=job_id)


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
    daemon_module = _cli_daemon_module()
    return daemon_module._handle_daemon_run(
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
    daemon_module = _cli_daemon_module()
    return daemon_module._handle_daemon_schedule(
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
    history_module = _cli_history_module()
    return history_module._handle_history(
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
    history_module = _cli_history_module()
    return history_module._handle_history_show(
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
    history_module = _cli_history_module()
    return history_module._handle_history_compare(
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
    history_module = _cli_history_module()
    return history_module._handle_tui(
        sqlite_path=sqlite_path,
        limit=limit,
        launch_history_tui=launch_history_tui,
    )


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
