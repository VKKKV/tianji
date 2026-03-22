from __future__ import annotations

from datetime import UTC, datetime
import json
from pathlib import Path

from .backtrack import backtrack_candidates
from .fetch import fetch_url, parse_feed, read_fixture, source_name_from_url
from .models import RawItem, RunArtifact
from .normalize import normalize_items
from .scoring import score_events, summarize_scenario
from .storage import persist_run


def run_pipeline(
    *,
    fixture_paths: list[str],
    fetch: bool,
    source_urls: list[str],
    output_path: str | None,
    sqlite_path: str | None = None,
) -> RunArtifact:
    raw_items: list[RawItem] = []

    for fixture_path in fixture_paths:
        feed_text = read_fixture(fixture_path)
        raw_items.extend(
            parse_feed(feed_text, source=f"fixture:{Path(fixture_path).name}")
        )

    if fetch:
        for source_url in source_urls:
            feed_text = fetch_url(source_url)
            raw_items.extend(
                parse_feed(feed_text, source=source_name_from_url(source_url))
            )

    if not raw_items:
        raise ValueError(
            "No input items were loaded. Provide --fixture and/or --fetch --source-url."
        )

    normalized_events = normalize_items(raw_items)
    scored_events = score_events(normalized_events)
    scenario_summary = summarize_scenario(scored_events)
    interventions = backtrack_candidates(scored_events)
    artifact = RunArtifact(
        mode="fetch+fixture"
        if fetch and fixture_paths
        else "fetch"
        if fetch
        else "fixture",
        generated_at=datetime.now(UTC).isoformat(),
        input_summary={
            "raw_item_count": len(raw_items),
            "normalized_event_count": len(normalized_events),
            "sources": sorted({item.source for item in raw_items}),
        },
        scenario_summary=scenario_summary,
        scored_events=scored_events,
        intervention_candidates=interventions,
    )

    if sqlite_path:
        persist_run(
            sqlite_path=sqlite_path,
            artifact=artifact,
            raw_items=raw_items,
            normalized_events=normalized_events,
            scored_events=scored_events,
            intervention_candidates=interventions,
        )

    if output_path:
        output = Path(output_path)
        output.parent.mkdir(parents=True, exist_ok=True)
        output.write_text(
            json.dumps(artifact.to_dict(), ensure_ascii=False, indent=2),
            encoding="utf-8",
        )

    return artifact
