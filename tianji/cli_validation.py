from __future__ import annotations

from typing import Callable

import click


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


def _validate_schedule_spec(*, every_seconds: int, count: int) -> None:
    if every_seconds < 60:
        raise click.BadParameter(
            "must be an integer greater than or equal to 60.",
            param_hint="--every-seconds",
        )
    if count < 1:
        raise click.BadParameter("must be greater than zero.", param_hint="--count")


def _resolve_compare_run_ids(
    *,
    sqlite_path: str,
    left_run_id: int | None,
    right_run_id: int | None,
    latest_pair: bool,
    run_id: int | None,
    against_latest: bool,
    against_previous: bool,
    get_latest_run_pair: Callable[..., tuple[int, int] | None],
    get_latest_run_id: Callable[..., int | None],
    get_previous_run_id: Callable[..., int | None],
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
        latest_resolved_run_id = get_latest_run_id(sqlite_path=sqlite_path)
        if latest_resolved_run_id is None:
            raise click.UsageError("No persisted runs are available.")
        return run_id, latest_resolved_run_id

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
