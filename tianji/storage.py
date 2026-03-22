from __future__ import annotations

import json
from pathlib import Path
import sqlite3

from .models import (
    RUN_ARTIFACT_SCHEMA_VERSION,
    InterventionCandidate,
    NormalizedEvent,
    RawItem,
    RunArtifact,
    ScoredEvent,
)


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
