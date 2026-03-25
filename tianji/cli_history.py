from __future__ import annotations

import json
from typing import Callable, cast

import click

from .cli_validation import (
    _resolve_compare_run_ids,
    validate_positive_run_id,
    validate_score_range,
)
from .storage import (
    compare_runs,
    get_latest_run_id,
    get_latest_run_pair,
    get_next_run_id,
    get_previous_run_id,
    get_run_summary,
    list_runs,
)
from .tui import launch_history_tui as _launch_history_tui


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
        get_latest_run_pair=get_latest_run_pair,
        get_latest_run_id=get_latest_run_id,
        get_previous_run_id=get_previous_run_id,
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


def _handle_tui(
    *,
    sqlite_path: str,
    limit: int,
    launch_history_tui: Callable[..., int] = _launch_history_tui,
) -> int:
    if limit < 0:
        raise click.UsageError("--limit must be zero or greater.")
    return launch_history_tui(sqlite_path=sqlite_path, limit=limit)
