from __future__ import annotations

from collections.abc import Iterable
from datetime import UTC, datetime, timedelta
from email.utils import parsedate_to_datetime
import json
from pathlib import Path
from typing import TypedDict

from .backtrack import backtrack_candidates
from .fetch import fetch_url, parse_feed, read_fixture, source_name_from_url
from .models import RawItem, RunArtifact, ScoredEvent
from .normalize import normalize_items
from .scoring import score_events, summarize_scenario
from .storage import persist_run


MIN_SHARED_KEYWORDS = 2
MAX_GROUP_TIME_DELTA = timedelta(hours=24)


class EventGroupSummary(TypedDict):
    group_id: str
    headline_event_id: str
    member_event_ids: list[str]
    dominant_field: str
    shared_actors: list[str]
    shared_regions: list[str]
    group_score: float


def run_pipeline(
    *,
    fixture_paths: list[str],
    fetch: bool,
    source_urls: list[str],
    output_path: str | None,
    sqlite_path: str | None = None,
) -> RunArtifact:
    raw_items: list[RawItem] = []
    loaded_sources: list[str] = []

    for fixture_path in fixture_paths:
        source = f"fixture:{Path(fixture_path).name}"
        feed_text = read_fixture(fixture_path)
        loaded_sources.append(source)
        raw_items.extend(parse_feed(feed_text, source=source))

    if fetch:
        for source_url in source_urls:
            source = source_name_from_url(source_url)
            feed_text = fetch_url(source_url)
            loaded_sources.append(source)
            raw_items.extend(parse_feed(feed_text, source=source))

    if not loaded_sources:
        raise ValueError(
            "No input items were loaded. Provide --fixture and/or --fetch --source-url."
        )

    normalized_events = normalize_items(raw_items)
    scored_events = score_events(normalized_events)
    scenario_summary = summarize_scenario(scored_events)
    scenario_summary["event_groups"] = group_events(scored_events)
    interventions = backtrack_candidates(
        scored_events,
        event_groups=scenario_summary["event_groups"],
    )
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
            "sources": sorted({item.source for item in raw_items}) if raw_items else [],
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


def group_events(scored_events: list[ScoredEvent]) -> list[EventGroupSummary]:
    ordered_events = sorted(
        scored_events,
        key=lambda event: (-event.divergence_score, event.event_id),
    )
    groups: list[list[ScoredEvent]] = []

    for event in ordered_events:
        placed = False
        for group in groups:
            if matches_group(event, group):
                group.append(event)
                placed = True
                break
        if not placed:
            groups.append([event])

    summaries = [summarize_group(group) for group in groups if len(group) > 1]
    return sorted(summaries, key=event_group_sort_key)


def matches_group(event: ScoredEvent, group: list[ScoredEvent]) -> bool:
    anchor = sorted(group, key=lambda item: (-item.divergence_score, item.event_id))[0]
    if event.dominant_field != anchor.dominant_field:
        return False

    if not is_within_group_time_window(event, anchor):
        return False

    shared_actors = intersection(event.actors, anchor.actors)
    shared_regions = intersection(event.regions, anchor.regions)
    if not shared_actors and not shared_regions:
        return False

    shared_keywords = intersection(event.keywords, anchor.keywords)
    return len(shared_keywords) >= MIN_SHARED_KEYWORDS


def summarize_group(group: list[ScoredEvent]) -> EventGroupSummary:
    ordered_group = sorted(
        group, key=lambda event: (-event.divergence_score, event.event_id)
    )
    anchor = ordered_group[0]
    shared_actors = shared_values(event.actors for event in ordered_group)
    shared_regions = shared_values(event.regions for event in ordered_group)
    return {
        "group_id": f"group:{anchor.event_id}",
        "headline_event_id": anchor.event_id,
        "member_event_ids": [event.event_id for event in ordered_group],
        "dominant_field": anchor.dominant_field,
        "shared_actors": shared_actors,
        "shared_regions": shared_regions,
        "group_score": round(sum(event.divergence_score for event in ordered_group), 2),
    }


def event_group_sort_key(group: EventGroupSummary) -> tuple[float, str]:
    return (-group["group_score"], group["headline_event_id"])


def shared_values(value_lists: Iterable[list[str]]) -> list[str]:
    iterator = iter(value_lists)
    try:
        shared = set(next(iterator))
    except StopIteration:
        return []

    for values in iterator:
        shared &= set(values)

    return sorted(shared)


def intersection(left: list[str], right: list[str]) -> list[str]:
    return sorted(set(left) & set(right))


def is_within_group_time_window(event: ScoredEvent, anchor: ScoredEvent) -> bool:
    event_time = parse_event_time(event.published_at)
    anchor_time = parse_event_time(anchor.published_at)
    if event_time is None or anchor_time is None:
        return True
    return abs(event_time - anchor_time) <= MAX_GROUP_TIME_DELTA


def parse_event_time(value: str | None) -> datetime | None:
    if not value:
        return None

    try:
        parsed = datetime.fromisoformat(value.replace("Z", "+00:00"))
    except ValueError:
        try:
            parsed = parsedate_to_datetime(value)
        except (TypeError, ValueError, IndexError, OverflowError):
            return None

    if parsed.tzinfo is None:
        return parsed.replace(tzinfo=UTC)
    return parsed.astimezone(UTC)
