from __future__ import annotations

from contextlib import closing
import json
import sqlite3
from typing import TypeAlias, cast

from .storage_filters import (
    filter_event_group_details,
    filter_intervention_candidate_details,
    filter_run_list_items,
    filter_scored_event_details,
)


RunRow: TypeAlias = tuple[int, str, str, str, str, str]
ScoredEventRow: TypeAlias = tuple[
    str,
    str,
    str,
    str,
    str | None,
    str,
    float,
    float,
    float,
    str,
]
InterventionCandidateRow: TypeAlias = tuple[int, str, str, str, str, str]


def build_run_list_item(
    row: RunRow,
    *,
    top_scored_event: dict[str, object] | None = None,
) -> dict[str, object]:
    (
        run_id,
        schema_version,
        mode,
        generated_at,
        input_summary_json,
        scenario_summary_json,
    ) = row
    input_summary = json.loads(cast(str, input_summary_json))
    scenario_summary = json.loads(cast(str, scenario_summary_json))
    event_groups = cast(
        list[dict[str, object]], scenario_summary.get("event_groups", [])
    )
    top_event_group = event_groups[0] if event_groups else None
    return {
        "run_id": run_id,
        "schema_version": schema_version,
        "mode": mode,
        "generated_at": generated_at,
        "raw_item_count": input_summary.get("raw_item_count", 0),
        "normalized_event_count": input_summary.get("normalized_event_count", 0),
        "dominant_field": scenario_summary.get("dominant_field", "uncategorized"),
        "risk_level": scenario_summary.get("risk_level", "low"),
        "headline": scenario_summary.get("headline", ""),
        "event_group_count": len(event_groups),
        "top_event_group_headline_event_id": (
            top_event_group.get("headline_event_id")
            if top_event_group is not None
            else None
        ),
        "top_event_group_dominant_field": (
            top_event_group.get("dominant_field")
            if top_event_group is not None
            else None
        ),
        "top_event_group_member_count": (
            top_event_group.get("member_count") if top_event_group is not None else None
        ),
        "top_scored_event_id": (
            top_scored_event.get("event_id") if top_scored_event is not None else None
        ),
        "top_scored_event_dominant_field": (
            top_scored_event.get("dominant_field")
            if top_scored_event is not None
            else None
        ),
        "top_impact_score": (
            top_scored_event.get("impact_score")
            if top_scored_event is not None
            else None
        ),
        "top_field_attraction": (
            top_scored_event.get("field_attraction")
            if top_scored_event is not None
            else None
        ),
        "top_divergence_score": (
            top_scored_event.get("divergence_score")
            if top_scored_event is not None
            else None
        ),
    }


def get_top_scored_event_summaries(
    connection: sqlite3.Connection,
    run_ids: list[int],
) -> dict[int, dict[str, object]]:
    if not run_ids:
        return {}
    placeholders = ", ".join("?" for _ in run_ids)
    rows = connection.execute(
        f"""
        SELECT run_id, event_id, dominant_field, impact_score, field_attraction, divergence_score
        FROM scored_events
        WHERE run_id IN ({placeholders})
        ORDER BY run_id ASC, divergence_score DESC, id ASC
        """,
        tuple(run_ids),
    ).fetchall()

    summaries_by_run_id: dict[int, dict[str, object]] = {}
    for row in rows:
        run_id = row[0]
        if not isinstance(run_id, int | str):
            raise RuntimeError("Unexpected run id type in top scored event summary row")
        run_id_value = int(run_id)
        if run_id_value in summaries_by_run_id:
            continue
        summaries_by_run_id[run_id_value] = {
            "event_id": str(row[1]),
            "dominant_field": str(row[2]),
            "impact_score": float(row[3]),
            "field_attraction": float(row[4]),
            "divergence_score": float(row[5]),
        }
    return summaries_by_run_id


def list_runs(
    *,
    sqlite_path: str,
    limit: int = 20,
    mode: str | None = None,
    dominant_field: str | None = None,
    risk_level: str | None = None,
    since: str | None = None,
    until: str | None = None,
    min_top_impact_score: float | None = None,
    max_top_impact_score: float | None = None,
    min_top_field_attraction: float | None = None,
    max_top_field_attraction: float | None = None,
    min_top_divergence_score: float | None = None,
    max_top_divergence_score: float | None = None,
    top_group_dominant_field: str | None = None,
    min_event_group_count: int | None = None,
    max_event_group_count: int | None = None,
) -> list[dict[str, object]]:
    with closing(sqlite3.connect(sqlite_path)) as connection:
        rows = connection.execute(
            """
            SELECT id, schema_version, mode, generated_at, input_summary_json, scenario_summary_json
            FROM runs
            ORDER BY id DESC
            """,
        ).fetchall()
        typed_rows = [coerce_run_row(row) for row in rows]
        top_scored_events_by_run_id = get_top_scored_event_summaries(
            connection,
            [run_id for run_id, *_ in typed_rows],
        )

    items = [
        build_run_list_item(
            row,
            top_scored_event=top_scored_events_by_run_id.get(row[0]),
        )
        for row in typed_rows
    ]
    filtered = filter_run_list_items(
        items,
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
    return filtered[:limit]


def get_run_summary(
    *,
    sqlite_path: str,
    run_id: int,
    dominant_field: str | None = None,
    min_impact_score: float | None = None,
    max_impact_score: float | None = None,
    min_field_attraction: float | None = None,
    max_field_attraction: float | None = None,
    min_divergence_score: float | None = None,
    max_divergence_score: float | None = None,
    limit_scored_events: int | None = None,
    only_matching_interventions: bool = False,
    group_dominant_field: str | None = None,
    limit_event_groups: int | None = None,
) -> dict[str, object] | None:
    with closing(sqlite3.connect(sqlite_path)) as connection:
        row = connection.execute(
            """
            SELECT id, schema_version, mode, generated_at, input_summary_json, scenario_summary_json
            FROM runs
            WHERE id = ?
            """,
            (run_id,),
        ).fetchone()
        scored_event_rows = connection.execute(
            """
            SELECT event_id, title, source, link, published_at, dominant_field,
                   impact_score, field_attraction, divergence_score, rationale_json
            FROM scored_events
            WHERE run_id = ?
            ORDER BY divergence_score DESC, id ASC
            """,
            (run_id,),
        ).fetchall()
        intervention_rows = connection.execute(
            """
            SELECT priority, event_id, target, intervention_type, reason, expected_effect
            FROM intervention_candidates
            WHERE run_id = ?
            ORDER BY priority ASC, id ASC
            """,
            (run_id,),
        ).fetchall()

    if row is None:
        return None

    payload = build_run_detail(coerce_run_row(row))
    scenario_summary = cast(dict[str, object], payload["scenario_summary"])
    event_groups = cast(
        list[dict[str, object]], scenario_summary.get("event_groups", [])
    )
    scenario_summary["event_groups"] = filter_event_group_details(
        event_groups,
        dominant_field=group_dominant_field,
        limit_event_groups=limit_event_groups,
    )
    scored_events = [
        build_scored_event_detail(coerce_scored_event_row(event_row))
        for event_row in scored_event_rows
    ]
    filtered_scored_events = filter_scored_event_details(
        scored_events,
        dominant_field=dominant_field,
        min_impact_score=min_impact_score,
        max_impact_score=max_impact_score,
        min_field_attraction=min_field_attraction,
        max_field_attraction=max_field_attraction,
        min_divergence_score=min_divergence_score,
        max_divergence_score=max_divergence_score,
        limit_scored_events=limit_scored_events,
    )
    payload["scored_events"] = filtered_scored_events
    intervention_candidates = [
        build_intervention_candidate_detail(
            coerce_intervention_candidate_row(intervention_row)
        )
        for intervention_row in intervention_rows
    ]
    payload["intervention_candidates"] = filter_intervention_candidate_details(
        intervention_candidates,
        visible_scored_event_ids={
            cast(str, event["event_id"]) for event in filtered_scored_events
        },
        only_matching_interventions=only_matching_interventions,
    )
    return payload


def get_latest_run_id(*, sqlite_path: str) -> int | None:
    with closing(sqlite3.connect(sqlite_path)) as connection:
        row = connection.execute(
            "SELECT id FROM runs ORDER BY id DESC LIMIT 1"
        ).fetchone()
    if row is None:
        return None
    run_id = row[0]
    if not isinstance(run_id, int | str):
        raise RuntimeError("Unexpected run id type in latest-run query")
    return int(run_id)


def get_latest_run_pair(*, sqlite_path: str) -> tuple[int, int] | None:
    with closing(sqlite3.connect(sqlite_path)) as connection:
        rows = connection.execute(
            "SELECT id FROM runs ORDER BY id DESC LIMIT 2"
        ).fetchall()
    if len(rows) < 2:
        return None
    newer = rows[0][0]
    older = rows[1][0]
    if not isinstance(newer, int | str) or not isinstance(older, int | str):
        raise RuntimeError("Unexpected run id type in latest-run pair query")
    return int(older), int(newer)


def get_previous_run_id(*, sqlite_path: str, run_id: int) -> int | None:
    with closing(sqlite3.connect(sqlite_path)) as connection:
        row = connection.execute(
            "SELECT id FROM runs WHERE id < ? ORDER BY id DESC LIMIT 1",
            (run_id,),
        ).fetchone()
    if row is None:
        return None
    previous_run_id = row[0]
    if not isinstance(previous_run_id, int | str):
        raise RuntimeError("Unexpected run id type in previous-run query")
    return int(previous_run_id)


def get_next_run_id(*, sqlite_path: str, run_id: int) -> int | None:
    with closing(sqlite3.connect(sqlite_path)) as connection:
        row = connection.execute(
            "SELECT id FROM runs WHERE id > ? ORDER BY id ASC LIMIT 1",
            (run_id,),
        ).fetchone()
    if row is None:
        return None
    next_run_id = row[0]
    if not isinstance(next_run_id, int | str):
        raise RuntimeError("Unexpected run id type in next-run query")
    return int(next_run_id)


def build_run_detail(row: RunRow) -> dict[str, object]:
    (
        run_id,
        schema_version,
        mode,
        generated_at,
        input_summary_json,
        scenario_summary_json,
    ) = row
    return {
        "run_id": run_id,
        "schema_version": schema_version,
        "mode": mode,
        "generated_at": generated_at,
        "input_summary": json.loads(cast(str, input_summary_json)),
        "scenario_summary": json.loads(cast(str, scenario_summary_json)),
    }


def coerce_run_row(row: sqlite3.Row | tuple[object, ...]) -> RunRow:
    (
        run_id,
        schema_version,
        mode,
        generated_at,
        input_summary_json,
        scenario_summary_json,
    ) = row
    if not isinstance(run_id, int | str):
        raise RuntimeError("Unexpected run id type in SQLite row")
    return (
        int(run_id),
        str(schema_version),
        str(mode),
        str(generated_at),
        str(input_summary_json),
        str(scenario_summary_json),
    )


def build_scored_event_detail(row: ScoredEventRow) -> dict[str, object]:
    (
        event_id,
        title,
        source,
        link,
        published_at,
        dominant_field,
        impact_score,
        field_attraction,
        divergence_score,
        rationale_json,
    ) = row
    return {
        "event_id": event_id,
        "title": title,
        "source": source,
        "link": link,
        "published_at": published_at,
        "dominant_field": dominant_field,
        "impact_score": impact_score,
        "field_attraction": field_attraction,
        "divergence_score": divergence_score,
        "rationale": json.loads(rationale_json),
    }


def build_intervention_candidate_detail(
    row: InterventionCandidateRow,
) -> dict[str, object]:
    priority, event_id, target, intervention_type, reason, expected_effect = row
    return {
        "priority": priority,
        "event_id": event_id,
        "target": target,
        "intervention_type": intervention_type,
        "reason": reason,
        "expected_effect": expected_effect,
    }


def coerce_scored_event_row(
    row: sqlite3.Row | tuple[object, ...],
) -> ScoredEventRow:
    (
        event_id,
        title,
        source,
        link,
        published_at,
        dominant_field,
        impact_score,
        field_attraction,
        divergence_score,
        rationale_json,
    ) = row
    if published_at is not None and not isinstance(published_at, str):
        raise RuntimeError("Unexpected published_at type in scored event row")
    if not isinstance(impact_score, int | float | str):
        raise RuntimeError("Unexpected impact_score type in scored event row")
    if not isinstance(field_attraction, int | float | str):
        raise RuntimeError("Unexpected field_attraction type in scored event row")
    if not isinstance(divergence_score, int | float | str):
        raise RuntimeError("Unexpected divergence_score type in scored event row")
    impact_score_value = cast(int | float | str, impact_score)
    field_attraction_value = cast(int | float | str, field_attraction)
    divergence_score_value = cast(int | float | str, divergence_score)
    return (
        str(event_id),
        str(title),
        str(source),
        str(link),
        published_at,
        str(dominant_field),
        float(impact_score_value),
        float(field_attraction_value),
        float(divergence_score_value),
        str(rationale_json),
    )


def coerce_intervention_candidate_row(
    row: sqlite3.Row | tuple[object, ...],
) -> InterventionCandidateRow:
    priority, event_id, target, intervention_type, reason, expected_effect = row
    if not isinstance(priority, int | str):
        raise RuntimeError("Unexpected priority type in intervention candidate row")
    return (
        int(priority),
        str(event_id),
        str(target),
        str(intervention_type),
        str(reason),
        str(expected_effect),
    )
