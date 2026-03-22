from __future__ import annotations

import json
from pathlib import Path
import sqlite3
from typing import TypeAlias, cast

from .models import (
    RUN_ARTIFACT_SCHEMA_VERSION,
    InterventionCandidate,
    NormalizedEvent,
    RawItem,
    RunArtifact,
    ScoredEvent,
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


def persist_run(
    *,
    sqlite_path: str,
    artifact: RunArtifact,
    raw_items: list[RawItem],
    normalized_events: list[NormalizedEvent],
    scored_events: list[ScoredEvent],
    intervention_candidates: list[InterventionCandidate],
) -> None:
    database_path = Path(sqlite_path)
    database_path.parent.mkdir(parents=True, exist_ok=True)

    with sqlite3.connect(database_path) as connection:
        connection.execute("PRAGMA foreign_keys = ON")
        initialize_schema(connection)
        run_id = insert_run(connection, artifact)
        insert_raw_items(connection, run_id, raw_items)
        insert_normalized_events(connection, run_id, normalized_events)
        insert_scored_events(connection, run_id, scored_events)
        insert_intervention_candidates(connection, run_id, intervention_candidates)
        connection.commit()


def list_runs(*, sqlite_path: str, limit: int = 20) -> list[dict[str, object]]:
    with sqlite3.connect(sqlite_path) as connection:
        rows = connection.execute(
            """
            SELECT id, schema_version, mode, generated_at, input_summary_json, scenario_summary_json
            FROM runs
            ORDER BY id DESC
            LIMIT ?
            """,
            (limit,),
        ).fetchall()

    typed_rows = [coerce_run_row(row) for row in rows]
    return [build_run_list_item(row) for row in typed_rows]


def get_run_summary(*, sqlite_path: str, run_id: int) -> dict[str, object] | None:
    with sqlite3.connect(sqlite_path) as connection:
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
    payload["scored_events"] = [
        build_scored_event_detail(coerce_scored_event_row(event_row))
        for event_row in scored_event_rows
    ]
    payload["intervention_candidates"] = [
        build_intervention_candidate_detail(
            coerce_intervention_candidate_row(intervention_row)
        )
        for intervention_row in intervention_rows
    ]
    return payload


def compare_runs(
    *,
    sqlite_path: str,
    left_run_id: int,
    right_run_id: int,
) -> dict[str, object] | None:
    left = get_run_summary(sqlite_path=sqlite_path, run_id=left_run_id)
    right = get_run_summary(sqlite_path=sqlite_path, run_id=right_run_id)
    if left is None or right is None:
        return None

    left_summary = build_compare_side(left)
    right_summary = build_compare_side(right)
    return {
        "left_run_id": left_run_id,
        "right_run_id": right_run_id,
        "left": left_summary,
        "right": right_summary,
        "diff": build_compare_diff(left_summary, right_summary),
    }


def initialize_schema(connection: sqlite3.Connection) -> None:
    connection.executescript(
        """
        CREATE TABLE IF NOT EXISTS runs (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            schema_version TEXT NOT NULL,
            mode TEXT NOT NULL,
            generated_at TEXT NOT NULL,
            input_summary_json TEXT NOT NULL,
            scenario_summary_json TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS raw_items (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            run_id INTEGER NOT NULL,
            source TEXT NOT NULL,
            title TEXT NOT NULL,
            summary TEXT NOT NULL,
            link TEXT NOT NULL,
            published_at TEXT,
            FOREIGN KEY (run_id) REFERENCES runs(id) ON DELETE CASCADE
        );

        CREATE TABLE IF NOT EXISTS normalized_events (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            run_id INTEGER NOT NULL,
            event_id TEXT NOT NULL,
            source TEXT NOT NULL,
            title TEXT NOT NULL,
            summary TEXT NOT NULL,
            link TEXT NOT NULL,
            published_at TEXT,
            keywords_json TEXT NOT NULL,
            actors_json TEXT NOT NULL,
            regions_json TEXT NOT NULL,
            field_scores_json TEXT NOT NULL,
            FOREIGN KEY (run_id) REFERENCES runs(id) ON DELETE CASCADE
        );

        CREATE TABLE IF NOT EXISTS scored_events (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            run_id INTEGER NOT NULL,
            event_id TEXT NOT NULL,
            title TEXT NOT NULL,
            source TEXT NOT NULL,
            link TEXT NOT NULL,
            published_at TEXT,
            actors_json TEXT NOT NULL,
            regions_json TEXT NOT NULL,
            keywords_json TEXT NOT NULL,
            dominant_field TEXT NOT NULL,
            impact_score REAL NOT NULL,
            field_attraction REAL NOT NULL,
            divergence_score REAL NOT NULL,
            rationale_json TEXT NOT NULL,
            FOREIGN KEY (run_id) REFERENCES runs(id) ON DELETE CASCADE
        );

        CREATE TABLE IF NOT EXISTS intervention_candidates (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            run_id INTEGER NOT NULL,
            priority INTEGER NOT NULL,
            event_id TEXT NOT NULL,
            target TEXT NOT NULL,
            intervention_type TEXT NOT NULL,
            reason TEXT NOT NULL,
            expected_effect TEXT NOT NULL,
            FOREIGN KEY (run_id) REFERENCES runs(id) ON DELETE CASCADE
        );
        """
    )


def insert_run(connection: sqlite3.Connection, artifact: RunArtifact) -> int:
    cursor = connection.execute(
        """
        INSERT INTO runs (
            schema_version,
            mode,
            generated_at,
            input_summary_json,
            scenario_summary_json
        ) VALUES (?, ?, ?, ?, ?)
        """,
        (
            RUN_ARTIFACT_SCHEMA_VERSION,
            artifact.mode,
            artifact.generated_at,
            json.dumps(artifact.input_summary, ensure_ascii=False),
            json.dumps(artifact.scenario_summary, ensure_ascii=False),
        ),
    )
    lastrowid = cursor.lastrowid
    if not isinstance(lastrowid, int):
        raise RuntimeError("Failed to persist run row")
    return lastrowid


def insert_raw_items(
    connection: sqlite3.Connection, run_id: int, raw_items: list[RawItem]
) -> None:
    connection.executemany(
        """
        INSERT INTO raw_items (run_id, source, title, summary, link, published_at)
        VALUES (?, ?, ?, ?, ?, ?)
        """,
        [
            (
                run_id,
                item.source,
                item.title,
                item.summary,
                item.link,
                item.published_at,
            )
            for item in raw_items
        ],
    )


def insert_normalized_events(
    connection: sqlite3.Connection,
    run_id: int,
    normalized_events: list[NormalizedEvent],
) -> None:
    connection.executemany(
        """
        INSERT INTO normalized_events (
            run_id,
            event_id,
            source,
            title,
            summary,
            link,
            published_at,
            keywords_json,
            actors_json,
            regions_json,
            field_scores_json
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        """,
        [
            (
                run_id,
                event.event_id,
                event.source,
                event.title,
                event.summary,
                event.link,
                event.published_at,
                json.dumps(event.keywords, ensure_ascii=False),
                json.dumps(event.actors, ensure_ascii=False),
                json.dumps(event.regions, ensure_ascii=False),
                json.dumps(event.field_scores, ensure_ascii=False),
            )
            for event in normalized_events
        ],
    )


def insert_scored_events(
    connection: sqlite3.Connection, run_id: int, scored_events: list[ScoredEvent]
) -> None:
    connection.executemany(
        """
        INSERT INTO scored_events (
            run_id,
            event_id,
            title,
            source,
            link,
            published_at,
            actors_json,
            regions_json,
            keywords_json,
            dominant_field,
            impact_score,
            field_attraction,
            divergence_score,
            rationale_json
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        """,
        [
            (
                run_id,
                event.event_id,
                event.title,
                event.source,
                event.link,
                event.published_at,
                json.dumps(event.actors, ensure_ascii=False),
                json.dumps(event.regions, ensure_ascii=False),
                json.dumps(event.keywords, ensure_ascii=False),
                event.dominant_field,
                event.impact_score,
                event.field_attraction,
                event.divergence_score,
                json.dumps(event.rationale, ensure_ascii=False),
            )
            for event in scored_events
        ],
    )


def insert_intervention_candidates(
    connection: sqlite3.Connection,
    run_id: int,
    intervention_candidates: list[InterventionCandidate],
) -> None:
    connection.executemany(
        """
        INSERT INTO intervention_candidates (
            run_id,
            priority,
            event_id,
            target,
            intervention_type,
            reason,
            expected_effect
        ) VALUES (?, ?, ?, ?, ?, ?, ?)
        """,
        [
            (
                run_id,
                candidate.priority,
                candidate.event_id,
                candidate.target,
                candidate.intervention_type,
                candidate.reason,
                candidate.expected_effect,
            )
            for candidate in intervention_candidates
        ],
    )


def build_run_list_item(row: RunRow) -> dict[str, object]:
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
    }


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


def build_compare_side(run_payload: dict[str, object]) -> dict[str, object]:
    input_summary = cast(dict[str, object], run_payload["input_summary"])
    scenario_summary = cast(dict[str, object], run_payload["scenario_summary"])
    scored_events = cast(list[dict[str, object]], run_payload["scored_events"])
    intervention_candidates = cast(
        list[dict[str, object]], run_payload["intervention_candidates"]
    )
    top_scored_event = scored_events[0] if scored_events else None
    top_intervention = intervention_candidates[0] if intervention_candidates else None
    return {
        "run_id": run_payload["run_id"],
        "schema_version": run_payload["schema_version"],
        "mode": run_payload["mode"],
        "raw_item_count": input_summary.get("raw_item_count", 0),
        "normalized_event_count": input_summary.get("normalized_event_count", 0),
        "dominant_field": scenario_summary.get("dominant_field", "uncategorized"),
        "risk_level": scenario_summary.get("risk_level", "low"),
        "headline": scenario_summary.get("headline", ""),
        "top_scored_event": top_scored_event,
        "top_intervention": top_intervention,
        "intervention_event_ids": [
            cast(str, candidate["event_id"]) for candidate in intervention_candidates
        ],
    }


def build_compare_diff(
    left: dict[str, object],
    right: dict[str, object],
) -> dict[str, object]:
    left_intervention_ids = cast(list[str], left["intervention_event_ids"])
    right_intervention_ids = cast(list[str], right["intervention_event_ids"])
    left_top_scored_event = cast(dict[str, object] | None, left["top_scored_event"])
    right_top_scored_event = cast(dict[str, object] | None, right["top_scored_event"])
    left_top_intervention = cast(dict[str, object] | None, left["top_intervention"])
    right_top_intervention = cast(dict[str, object] | None, right["top_intervention"])
    left_top_scored_event_id = (
        cast(str, left_top_scored_event["event_id"])
        if left_top_scored_event is not None
        else None
    )
    right_top_scored_event_id = (
        cast(str, right_top_scored_event["event_id"])
        if right_top_scored_event is not None
        else None
    )
    left_top_intervention_event_id = (
        cast(str, left_top_intervention["event_id"])
        if left_top_intervention is not None
        else None
    )
    right_top_intervention_event_id = (
        cast(str, right_top_intervention["event_id"])
        if right_top_intervention is not None
        else None
    )
    return {
        "raw_item_count_delta": cast(int, right["raw_item_count"])
        - cast(int, left["raw_item_count"]),
        "normalized_event_count_delta": cast(int, right["normalized_event_count"])
        - cast(int, left["normalized_event_count"]),
        "dominant_field_changed": left["dominant_field"] != right["dominant_field"],
        "risk_level_changed": left["risk_level"] != right["risk_level"],
        "top_scored_event_changed": left_top_scored_event_id
        != right_top_scored_event_id,
        "top_intervention_changed": left_top_intervention_event_id
        != right_top_intervention_event_id,
        "left_top_scored_event_id": left_top_scored_event_id,
        "right_top_scored_event_id": right_top_scored_event_id,
        "left_top_intervention_event_id": left_top_intervention_event_id,
        "right_top_intervention_event_id": right_top_intervention_event_id,
        "left_only_intervention_event_ids": [
            event_id
            for event_id in left_intervention_ids
            if event_id not in right_intervention_ids
        ],
        "right_only_intervention_event_ids": [
            event_id
            for event_id in right_intervention_ids
            if event_id not in left_intervention_ids
        ],
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
