from __future__ import annotations

from contextlib import closing
from datetime import datetime
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
from .fetch import derive_canonical_content_hash, derive_canonical_entry_identity_hash


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

    with closing(sqlite3.connect(database_path)) as connection:
        connection.execute("PRAGMA foreign_keys = ON")
        initialize_schema(connection)
        run_id = insert_run(connection, artifact)
        canonical_source_item_ids = ensure_canonical_source_items(connection, raw_items)
        insert_raw_items(connection, run_id, raw_items, canonical_source_item_ids)
        insert_normalized_events(
            connection,
            run_id,
            normalized_events,
            canonical_source_item_ids,
        )
        insert_scored_events(connection, run_id, scored_events)
        insert_intervention_candidates(connection, run_id, intervention_candidates)
        connection.commit()


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


def compare_runs(
    *,
    sqlite_path: str,
    left_run_id: int,
    right_run_id: int,
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
    left = get_run_summary(
        sqlite_path=sqlite_path,
        run_id=left_run_id,
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
    right = get_run_summary(
        sqlite_path=sqlite_path,
        run_id=right_run_id,
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

        CREATE TABLE IF NOT EXISTS source_items (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            entry_identity_hash TEXT NOT NULL,
            content_hash TEXT NOT NULL,
            source TEXT NOT NULL,
            title TEXT NOT NULL,
            summary TEXT NOT NULL,
            link TEXT NOT NULL,
            published_at TEXT,
            UNIQUE(entry_identity_hash, content_hash)
        );

        CREATE TABLE IF NOT EXISTS raw_items (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            run_id INTEGER NOT NULL,
            canonical_source_item_id INTEGER,
            source TEXT NOT NULL,
            title TEXT NOT NULL,
            summary TEXT NOT NULL,
            link TEXT NOT NULL,
            published_at TEXT,
            FOREIGN KEY (run_id) REFERENCES runs(id) ON DELETE CASCADE,
            FOREIGN KEY (canonical_source_item_id) REFERENCES source_items(id)
        );

        CREATE TABLE IF NOT EXISTS normalized_events (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            run_id INTEGER NOT NULL,
            canonical_source_item_id INTEGER,
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
            FOREIGN KEY (run_id) REFERENCES runs(id) ON DELETE CASCADE,
            FOREIGN KEY (canonical_source_item_id) REFERENCES source_items(id)
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
    ensure_column(
        connection,
        table_name="raw_items",
        column_name="canonical_source_item_id",
        column_definition="INTEGER REFERENCES source_items(id)",
    )
    ensure_column(
        connection,
        table_name="normalized_events",
        column_name="canonical_source_item_id",
        column_definition="INTEGER REFERENCES source_items(id)",
    )


def ensure_column(
    connection: sqlite3.Connection,
    *,
    table_name: str,
    column_name: str,
    column_definition: str,
) -> None:
    rows = connection.execute(f"PRAGMA table_info({table_name})").fetchall()
    existing_column_names = {str(row[1]) for row in rows}
    if column_name in existing_column_names:
        return
    connection.execute(
        f"ALTER TABLE {table_name} ADD COLUMN {column_name} {column_definition}"
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


def ensure_canonical_source_items(
    connection: sqlite3.Connection,
    raw_items: list[RawItem],
) -> dict[tuple[str, str], int]:
    canonical_ids: dict[tuple[str, str], int] = {}
    for item in raw_items:
        identity_hash = (
            item.entry_identity_hash or derive_canonical_entry_identity_hash(item)
        )
        content_hash = item.content_hash or derive_canonical_content_hash(item)
        item.entry_identity_hash = identity_hash
        item.content_hash = content_hash
        key = (identity_hash, content_hash)
        if key in canonical_ids:
            continue
        row = connection.execute(
            """
            SELECT id
            FROM source_items
            WHERE entry_identity_hash = ? AND content_hash = ?
            """,
            key,
        ).fetchone()
        if row is None:
            cursor = connection.execute(
                """
                INSERT INTO source_items (
                    entry_identity_hash,
                    content_hash,
                    source,
                    title,
                    summary,
                    link,
                    published_at
                ) VALUES (?, ?, ?, ?, ?, ?, ?)
                """,
                (
                    identity_hash,
                    content_hash,
                    item.source,
                    item.title,
                    item.summary,
                    item.link,
                    item.published_at,
                ),
            )
            lastrowid = cursor.lastrowid
            if not isinstance(lastrowid, int):
                raise RuntimeError("Failed to persist canonical source item row")
            canonical_ids[key] = lastrowid
            continue
        canonical_id = row[0]
        if not isinstance(canonical_id, int | str):
            raise RuntimeError("Unexpected canonical source item id type")
        canonical_ids[key] = int(canonical_id)
    return canonical_ids


def insert_raw_items(
    connection: sqlite3.Connection,
    run_id: int,
    raw_items: list[RawItem],
    canonical_source_item_ids: dict[tuple[str, str], int],
) -> None:
    connection.executemany(
        """
        INSERT INTO raw_items (
            run_id,
            canonical_source_item_id,
            source,
            title,
            summary,
            link,
            published_at
        )
        VALUES (?, ?, ?, ?, ?, ?, ?)
        """,
        [
            (
                run_id,
                canonical_source_item_ids[
                    (item.entry_identity_hash, item.content_hash)
                ],
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
    canonical_source_item_ids: dict[tuple[str, str], int],
) -> None:
    connection.executemany(
        """
        INSERT INTO normalized_events (
            run_id,
            canonical_source_item_id,
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
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        """,
        [
            (
                run_id,
                canonical_source_item_ids[
                    (event.entry_identity_hash, event.content_hash)
                ],
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


def filter_scored_event_details(
    scored_events: list[dict[str, object]],
    *,
    dominant_field: str | None,
    min_impact_score: float | None,
    max_impact_score: float | None,
    min_field_attraction: float | None,
    max_field_attraction: float | None,
    min_divergence_score: float | None,
    max_divergence_score: float | None,
    limit_scored_events: int | None,
) -> list[dict[str, object]]:
    filtered = list(scored_events)
    if dominant_field is not None:
        filtered = [
            event for event in filtered if event.get("dominant_field") == dominant_field
        ]
    if min_impact_score is not None:
        filtered = [
            event
            for event in filtered
            if is_numeric_run_metric_at_or_above(
                event.get("impact_score"), min_impact_score
            )
        ]
    if max_impact_score is not None:
        filtered = [
            event
            for event in filtered
            if is_numeric_run_metric_at_or_below(
                event.get("impact_score"), max_impact_score
            )
        ]
    if min_field_attraction is not None:
        filtered = [
            event
            for event in filtered
            if is_numeric_run_metric_at_or_above(
                event.get("field_attraction"), min_field_attraction
            )
        ]
    if max_field_attraction is not None:
        filtered = [
            event
            for event in filtered
            if is_numeric_run_metric_at_or_below(
                event.get("field_attraction"), max_field_attraction
            )
        ]
    if min_divergence_score is not None:
        filtered = [
            event
            for event in filtered
            if is_numeric_run_metric_at_or_above(
                event.get("divergence_score"), min_divergence_score
            )
        ]
    if max_divergence_score is not None:
        filtered = [
            event
            for event in filtered
            if is_numeric_run_metric_at_or_below(
                event.get("divergence_score"), max_divergence_score
            )
        ]
    if limit_scored_events is not None:
        return filtered[:limit_scored_events]
    return filtered


def filter_intervention_candidate_details(
    intervention_candidates: list[dict[str, object]],
    *,
    visible_scored_event_ids: set[str],
    only_matching_interventions: bool,
) -> list[dict[str, object]]:
    if not only_matching_interventions:
        return intervention_candidates
    return [
        candidate
        for candidate in intervention_candidates
        if candidate.get("event_id") in visible_scored_event_ids
    ]


def filter_event_group_details(
    event_groups: list[dict[str, object]],
    *,
    dominant_field: str | None,
    limit_event_groups: int | None,
) -> list[dict[str, object]]:
    filtered = list(event_groups)
    if dominant_field is not None:
        filtered = [
            group for group in filtered if group.get("dominant_field") == dominant_field
        ]
    if limit_event_groups is not None:
        return filtered[:limit_event_groups]
    return filtered


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
    event_groups = cast(
        list[dict[str, object]], scenario_summary.get("event_groups", [])
    )
    scored_events = cast(list[dict[str, object]], run_payload["scored_events"])
    intervention_candidates = cast(
        list[dict[str, object]], run_payload["intervention_candidates"]
    )
    top_event_group = event_groups[0] if event_groups else None
    top_scored_event = scored_events[0] if scored_events else None
    top_intervention = intervention_candidates[0] if intervention_candidates else None
    event_group_headline_event_ids = [
        cast(str, group["headline_event_id"]) for group in event_groups
    ]
    return {
        "run_id": run_payload["run_id"],
        "schema_version": run_payload["schema_version"],
        "mode": run_payload["mode"],
        "raw_item_count": input_summary.get("raw_item_count", 0),
        "normalized_event_count": input_summary.get("normalized_event_count", 0),
        "dominant_field": scenario_summary.get("dominant_field", "uncategorized"),
        "risk_level": scenario_summary.get("risk_level", "low"),
        "headline": scenario_summary.get("headline", ""),
        "event_group_count": len(event_groups),
        "event_group_headline_event_ids": event_group_headline_event_ids,
        "top_event_group": top_event_group,
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
    left_top_event_group = cast(dict[str, object] | None, left["top_event_group"])
    right_top_event_group = cast(dict[str, object] | None, right["top_event_group"])
    left_top_intervention = cast(dict[str, object] | None, left["top_intervention"])
    right_top_intervention = cast(dict[str, object] | None, right["top_intervention"])
    left_top_event_group_headline_event_id = (
        cast(str, left_top_event_group["headline_event_id"])
        if left_top_event_group is not None
        else None
    )
    right_top_event_group_headline_event_id = (
        cast(str, right_top_event_group["headline_event_id"])
        if right_top_event_group is not None
        else None
    )
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
    left_top_impact_score = get_top_score_metric(left_top_scored_event, "impact_score")
    right_top_impact_score = get_top_score_metric(
        right_top_scored_event, "impact_score"
    )
    left_top_field_attraction = get_top_score_metric(
        left_top_scored_event, "field_attraction"
    )
    right_top_field_attraction = get_top_score_metric(
        right_top_scored_event, "field_attraction"
    )
    left_top_divergence_score = get_top_score_metric(
        left_top_scored_event, "divergence_score"
    )
    right_top_divergence_score = get_top_score_metric(
        right_top_scored_event, "divergence_score"
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
    left_event_group_headline_ids = cast(
        list[str], left["event_group_headline_event_ids"]
    )
    right_event_group_headline_ids = cast(
        list[str], right["event_group_headline_event_ids"]
    )
    return {
        "raw_item_count_delta": cast(int, right["raw_item_count"])
        - cast(int, left["raw_item_count"]),
        "normalized_event_count_delta": cast(int, right["normalized_event_count"])
        - cast(int, left["normalized_event_count"]),
        "event_group_count_delta": cast(int, right["event_group_count"])
        - cast(int, left["event_group_count"]),
        "dominant_field_changed": left["dominant_field"] != right["dominant_field"],
        "risk_level_changed": left["risk_level"] != right["risk_level"],
        "top_event_group_changed": left_top_event_group_headline_event_id
        != right_top_event_group_headline_event_id,
        "left_top_event_group_headline_event_id": left_top_event_group_headline_event_id,
        "right_top_event_group_headline_event_id": right_top_event_group_headline_event_id,
        "top_scored_event_changed": left_top_scored_event_id
        != right_top_scored_event_id,
        "top_scored_event_comparable": left_top_scored_event_id is not None
        and right_top_scored_event_id is not None
        and left_top_scored_event_id == right_top_scored_event_id,
        "top_intervention_changed": left_top_intervention_event_id
        != right_top_intervention_event_id,
        "left_top_scored_event_id": left_top_scored_event_id,
        "right_top_scored_event_id": right_top_scored_event_id,
        "left_top_impact_score": left_top_impact_score,
        "right_top_impact_score": right_top_impact_score,
        "top_impact_score_delta": build_score_delta(
            left_top_impact_score,
            right_top_impact_score,
        ),
        "left_top_field_attraction": left_top_field_attraction,
        "right_top_field_attraction": right_top_field_attraction,
        "top_field_attraction_delta": build_score_delta(
            left_top_field_attraction,
            right_top_field_attraction,
        ),
        "left_top_divergence_score": left_top_divergence_score,
        "right_top_divergence_score": right_top_divergence_score,
        "top_divergence_score_delta": build_score_delta(
            left_top_divergence_score,
            right_top_divergence_score,
        ),
        "left_top_intervention_event_id": left_top_intervention_event_id,
        "right_top_intervention_event_id": right_top_intervention_event_id,
        "left_only_event_group_headline_event_ids": [
            event_id
            for event_id in left_event_group_headline_ids
            if event_id not in right_event_group_headline_ids
        ],
        "right_only_event_group_headline_event_ids": [
            event_id
            for event_id in right_event_group_headline_ids
            if event_id not in left_event_group_headline_ids
        ],
        "top_event_group_evidence_diff": build_top_event_group_evidence_diff(
            left_top_event_group,
            right_top_event_group,
        ),
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


def get_top_score_metric(
    top_scored_event: dict[str, object] | None,
    metric_name: str,
) -> float | None:
    if top_scored_event is None:
        return None
    value = top_scored_event.get(metric_name)
    if not isinstance(value, int | float):
        return None
    return float(value)


def build_score_delta(
    left_value: float | None, right_value: float | None
) -> float | None:
    if left_value is None or right_value is None:
        return None
    return round(right_value - left_value, 2)


def build_top_event_group_evidence_diff(
    left_top_event_group: dict[str, object] | None,
    right_top_event_group: dict[str, object] | None,
) -> dict[str, object]:
    left_member_event_ids = sorted(
        cast(list[str], left_top_event_group.get("member_event_ids", []))
        if left_top_event_group is not None
        else []
    )
    right_member_event_ids = sorted(
        cast(list[str], right_top_event_group.get("member_event_ids", []))
        if right_top_event_group is not None
        else []
    )
    left_shared_keywords = sorted(
        cast(list[str], left_top_event_group.get("shared_keywords", []))
        if left_top_event_group is not None
        else []
    )
    right_shared_keywords = sorted(
        cast(list[str], right_top_event_group.get("shared_keywords", []))
        if right_top_event_group is not None
        else []
    )
    left_shared_actors = sorted(
        cast(list[str], left_top_event_group.get("shared_actors", []))
        if left_top_event_group is not None
        else []
    )
    right_shared_actors = sorted(
        cast(list[str], right_top_event_group.get("shared_actors", []))
        if right_top_event_group is not None
        else []
    )
    left_shared_regions = sorted(
        cast(list[str], left_top_event_group.get("shared_regions", []))
        if left_top_event_group is not None
        else []
    )
    right_shared_regions = sorted(
        cast(list[str], right_top_event_group.get("shared_regions", []))
        if right_top_event_group is not None
        else []
    )
    left_chain_summary = (
        cast(str, left_top_event_group.get("chain_summary"))
        if left_top_event_group is not None
        and left_top_event_group.get("chain_summary")
        else None
    )
    right_chain_summary = (
        cast(str, right_top_event_group.get("chain_summary"))
        if right_top_event_group is not None
        and right_top_event_group.get("chain_summary")
        else None
    )
    left_evidence_links = sorted(
        [
            format_evidence_chain_link(link)
            for link in cast(
                list[dict[str, object]],
                left_top_event_group.get("evidence_chain", []),
            )
        ]
        if left_top_event_group is not None
        else []
    )
    right_evidence_links = sorted(
        [
            format_evidence_chain_link(link)
            for link in cast(
                list[dict[str, object]],
                right_top_event_group.get("evidence_chain", []),
            )
        ]
        if right_top_event_group is not None
        else []
    )
    left_headline_event_id = (
        cast(str, left_top_event_group.get("headline_event_id"))
        if left_top_event_group is not None
        and left_top_event_group.get("headline_event_id")
        else None
    )
    right_headline_event_id = (
        cast(str, right_top_event_group.get("headline_event_id"))
        if right_top_event_group is not None
        and right_top_event_group.get("headline_event_id")
        else None
    )

    return {
        "comparable": left_headline_event_id is not None
        and right_headline_event_id is not None
        and left_headline_event_id == right_headline_event_id,
        "same_headline_event_id": left_headline_event_id == right_headline_event_id,
        "member_count_delta": len(right_member_event_ids) - len(left_member_event_ids),
        "left_member_event_ids": left_member_event_ids,
        "right_member_event_ids": right_member_event_ids,
        "left_only_member_event_ids": [
            event_id
            for event_id in left_member_event_ids
            if event_id not in right_member_event_ids
        ],
        "right_only_member_event_ids": [
            event_id
            for event_id in right_member_event_ids
            if event_id not in left_member_event_ids
        ],
        "shared_keywords_added": [
            keyword
            for keyword in right_shared_keywords
            if keyword not in left_shared_keywords
        ],
        "shared_keywords_removed": [
            keyword
            for keyword in left_shared_keywords
            if keyword not in right_shared_keywords
        ],
        "shared_actors_added": [
            actor for actor in right_shared_actors if actor not in left_shared_actors
        ],
        "shared_actors_removed": [
            actor for actor in left_shared_actors if actor not in right_shared_actors
        ],
        "shared_regions_added": [
            region
            for region in right_shared_regions
            if region not in left_shared_regions
        ],
        "shared_regions_removed": [
            region
            for region in left_shared_regions
            if region not in right_shared_regions
        ],
        "evidence_chain_link_count_delta": len(right_evidence_links)
        - len(left_evidence_links),
        "left_evidence_chain_links": left_evidence_links,
        "right_evidence_chain_links": right_evidence_links,
        "evidence_chain_links_added": [
            link for link in right_evidence_links if link not in left_evidence_links
        ],
        "evidence_chain_links_removed": [
            link for link in left_evidence_links if link not in right_evidence_links
        ],
        "chain_summary_changed": left_chain_summary != right_chain_summary,
        "left_chain_summary": left_chain_summary,
        "right_chain_summary": right_chain_summary,
    }


def format_evidence_chain_link(link: dict[str, object]) -> str:
    shared_keywords = cast(list[str], link.get("shared_keywords", []))
    shared_actors = cast(list[str], link.get("shared_actors", []))
    shared_regions = cast(list[str], link.get("shared_regions", []))
    time_delta_hours = link.get("time_delta_hours")
    time_delta_text = "unknown"
    if isinstance(time_delta_hours, int | float):
        time_delta_text = str(float(time_delta_hours)).rstrip("0").rstrip(".")
    return (
        f"{link.get('from_event_id', '?')}->{link.get('to_event_id', '?')}"
        f"|keywords={','.join(sorted(shared_keywords))}"
        f"|actors={','.join(sorted(shared_actors))}"
        f"|regions={','.join(sorted(shared_regions))}"
        f"|delta_h={time_delta_text}"
    )


def filter_run_list_items(
    items: list[dict[str, object]],
    *,
    mode: str | None,
    dominant_field: str | None,
    risk_level: str | None,
    since: str | None,
    until: str | None,
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
    filtered = items
    if mode is not None:
        filtered = [item for item in filtered if item.get("mode") == mode]
    if dominant_field is not None:
        filtered = [
            item for item in filtered if item.get("dominant_field") == dominant_field
        ]
    if risk_level is not None:
        filtered = [item for item in filtered if item.get("risk_level") == risk_level]
    since_value = parse_history_timestamp(since)
    if since_value is not None:
        filtered = [
            item
            for item in filtered
            if is_history_timestamp_on_or_after(item.get("generated_at"), since_value)
        ]
    until_value = parse_history_timestamp(until)
    if until_value is not None:
        filtered = [
            item
            for item in filtered
            if is_history_timestamp_on_or_before(item.get("generated_at"), until_value)
        ]
    if min_top_impact_score is not None:
        filtered = [
            item
            for item in filtered
            if is_numeric_run_metric_at_or_above(
                item.get("top_impact_score"), min_top_impact_score
            )
        ]
    if max_top_impact_score is not None:
        filtered = [
            item
            for item in filtered
            if is_numeric_run_metric_at_or_below(
                item.get("top_impact_score"), max_top_impact_score
            )
        ]
    if min_top_field_attraction is not None:
        filtered = [
            item
            for item in filtered
            if is_numeric_run_metric_at_or_above(
                item.get("top_field_attraction"), min_top_field_attraction
            )
        ]
    if max_top_field_attraction is not None:
        filtered = [
            item
            for item in filtered
            if is_numeric_run_metric_at_or_below(
                item.get("top_field_attraction"), max_top_field_attraction
            )
        ]
    if min_top_divergence_score is not None:
        filtered = [
            item
            for item in filtered
            if is_numeric_run_metric_at_or_above(
                item.get("top_divergence_score"), min_top_divergence_score
            )
        ]
    if max_top_divergence_score is not None:
        filtered = [
            item
            for item in filtered
            if is_numeric_run_metric_at_or_below(
                item.get("top_divergence_score"), max_top_divergence_score
            )
        ]
    if top_group_dominant_field is not None:
        filtered = [
            item
            for item in filtered
            if item.get("top_event_group_dominant_field") == top_group_dominant_field
        ]
    if min_event_group_count is not None:
        filtered = [
            item
            for item in filtered
            if is_numeric_run_metric_at_or_above(
                item.get("event_group_count"), float(min_event_group_count)
            )
        ]
    if max_event_group_count is not None:
        filtered = [
            item
            for item in filtered
            if is_numeric_run_metric_at_or_below(
                item.get("event_group_count"), float(max_event_group_count)
            )
        ]
    return filtered


def is_numeric_run_metric_at_or_above(value: object, threshold: float) -> bool:
    if not isinstance(value, int | float):
        return False
    return float(value) >= threshold


def is_numeric_run_metric_at_or_below(value: object, threshold: float) -> bool:
    if not isinstance(value, int | float):
        return False
    return float(value) <= threshold


def parse_history_timestamp(value: str | None) -> datetime | None:
    if value is None:
        return None
    return datetime.fromisoformat(value.replace("Z", "+00:00"))


def is_history_timestamp_on_or_after(value: object, threshold: datetime) -> bool:
    if not isinstance(value, str):
        return False
    parsed = parse_history_timestamp(value)
    return parsed is not None and parsed >= threshold


def is_history_timestamp_on_or_before(value: object, threshold: datetime) -> bool:
    if not isinstance(value, str):
        return False
    parsed = parse_history_timestamp(value)
    return parsed is not None and parsed <= threshold


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
