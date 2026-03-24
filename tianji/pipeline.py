from __future__ import annotations

from collections.abc import Iterable
from datetime import UTC, datetime, timedelta
from email.utils import parsedate_to_datetime
import json
from pathlib import Path

from .backtrack import EventChainLink, EventGroupSummary, backtrack_candidates
from .fetch import (
    assign_canonical_hashes,
    fetch_url,
    parse_feed,
    read_fixture,
    source_name_from_url,
)
from .models import RawItem, RunArtifact, ScoredEvent
from .normalize import normalize_items
from .scoring import score_events, summarize_scenario
from .storage import persist_run


MIN_SHARED_KEYWORDS = 2
MAX_GROUP_TIME_DELTA = timedelta(hours=24)


def run_pipeline(
    *,
    fixture_paths: list[str],
    fetch: bool,
    source_urls: list[str],
    fetch_policy: str = "always",
    source_fetch_details: list[dict[str, str]] | None = None,
    output_path: str | None,
    sqlite_path: str | None = None,
) -> RunArtifact:
    raw_items: list[RawItem] = []
    loaded_sources: list[str] = []
    resolved_source_fetch_details = source_fetch_details or [
        {
            "name": source_url,
            "url": source_url,
            "fetch_policy": fetch_policy,
        }
        for source_url in source_urls
    ]

    for fixture_path in fixture_paths:
        source = f"fixture:{Path(fixture_path).name}"
        feed_text = read_fixture(fixture_path)
        loaded_sources.append(source)
        raw_items.extend(assign_canonical_hashes(parse_feed(feed_text, source=source)))

    if fetch:
        for source_url in source_urls:
            source = source_name_from_url(source_url)
            feed_text = fetch_url(source_url)
            loaded_sources.append(source)
            raw_items.extend(
                assign_canonical_hashes(parse_feed(feed_text, source=source))
            )

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
            "fetch_policy": fetch_policy,
            "source_fetch_details": resolved_source_fetch_details,
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
    parent_event_ids_by_group: list[dict[str, str | None]] = []

    for event in ordered_events:
        best_group_match = select_best_group_match(event, groups)
        if best_group_match is None:
            groups.append([event])
            parent_event_ids_by_group.append({event.event_id: None})
        else:
            best_group_index, parent_event_id = best_group_match
            groups[best_group_index].append(event)
            parent_event_ids_by_group[best_group_index][event.event_id] = (
                parent_event_id
            )

    summaries = [
        summarize_group(group, parent_event_ids_by_group[index])
        for index, group in enumerate(groups)
        if len(group) > 1
    ]
    return sorted(summaries, key=event_group_sort_key)


def matches_group(event: ScoredEvent, group: list[ScoredEvent]) -> bool:
    return best_group_link(event, group) is not None


def select_best_group_match(
    event: ScoredEvent, groups: list[list[ScoredEvent]]
) -> tuple[int, str] | None:
    best_index: int | None = None
    best_parent_event_id: str | None = None
    best_score: tuple[int, float] | None = None
    for index, group in enumerate(groups):
        link = best_group_link(event, group)
        if link is None:
            continue
        score = (link[0], -link[1])
        if best_score is None or score > best_score:
            best_score = score
            best_index = index
            best_parent_event_id = link[2]
    if best_index is None or best_parent_event_id is None:
        return None
    return best_index, best_parent_event_id


def best_group_link(
    event: ScoredEvent,
    group: list[ScoredEvent],
) -> tuple[int, float, str] | None:
    best_score: tuple[int, float, str] | None = None
    for member in group:
        link_score = link_score_between_events(event, member)
        if link_score is None:
            continue
        score = (link_score[0], -link_score[1], member.event_id)
        if best_score is None or score > best_score:
            best_score = score
    return best_score


def link_score_between_events(
    left: ScoredEvent,
    right: ScoredEvent,
) -> tuple[int, float] | None:
    if left.dominant_field != right.dominant_field:
        return None
    if not is_within_group_time_window(left, right):
        return None

    shared_actors = intersection(left.actors, right.actors)
    shared_regions = intersection(left.regions, right.regions)
    if not shared_actors and not shared_regions:
        return None

    shared_keywords = intersection(left.keywords, right.keywords)
    if len(shared_keywords) < MIN_SHARED_KEYWORDS:
        return None

    time_delta_hours = compute_time_delta_hours(left.published_at, right.published_at)
    return (
        len(shared_keywords) + len(shared_actors) + len(shared_regions),
        time_delta_hours if time_delta_hours is not None else 10_000.0,
    )


def summarize_group(
    group: list[ScoredEvent],
    parent_event_ids: dict[str, str | None],
) -> EventGroupSummary:
    ordered_group = sorted(
        group, key=lambda event: (-event.divergence_score, event.event_id)
    )
    causal_ordered_group = sort_group_for_causal_chain(group, parent_event_ids)
    anchor = ordered_group[0]
    shared_keywords = shared_values(event.keywords for event in ordered_group)
    shared_actors = shared_values(event.actors for event in ordered_group)
    shared_regions = shared_values(event.regions for event in ordered_group)
    evidence_chain = build_evidence_chain(causal_ordered_group, parent_event_ids)
    summary: EventGroupSummary = {
        "group_id": f"group:{anchor.event_id}",
        "headline_event_id": anchor.event_id,
        "headline_title": anchor.title,
        "member_event_ids": [event.event_id for event in ordered_group],
        "member_count": len(ordered_group),
        "dominant_field": anchor.dominant_field,
        "shared_keywords": shared_keywords,
        "shared_actors": shared_actors,
        "shared_regions": shared_regions,
        "group_score": round(sum(event.divergence_score for event in ordered_group), 2),
        "causal_ordered_event_ids": [event.event_id for event in causal_ordered_group],
        "causal_span_hours": compute_group_causal_span_hours(causal_ordered_group),
        "evidence_chain": evidence_chain,
        "chain_summary": build_chain_summary(
            anchor=anchor,
            member_count=len(ordered_group),
            shared_keywords=shared_keywords,
            shared_actors=shared_actors,
            shared_regions=shared_regions,
            evidence_chain=evidence_chain,
        ),
        "causal_summary": build_causal_summary(causal_ordered_group, evidence_chain),
    }
    return summary


def event_group_sort_key(group: EventGroupSummary) -> tuple[float, str]:
    return (-group["group_score"], group["headline_event_id"])


def build_evidence_chain(
    ordered_group: list[ScoredEvent],
    parent_event_ids: dict[str, str | None],
) -> list[EventChainLink]:
    evidence_chain: list[EventChainLink] = []
    events_by_id = {event.event_id: event for event in ordered_group}
    for current_event in ordered_group:
        parent_event_id = parent_event_ids.get(current_event.event_id)
        if parent_event_id is None:
            continue
        previous_event = events_by_id[parent_event_id]
        evidence_chain.append(
            {
                "from_event_id": previous_event.event_id,
                "to_event_id": current_event.event_id,
                "shared_keywords": intersection(
                    previous_event.keywords, current_event.keywords
                ),
                "shared_actors": intersection(
                    previous_event.actors, current_event.actors
                ),
                "shared_regions": intersection(
                    previous_event.regions, current_event.regions
                ),
                "relationship": infer_group_relationship(previous_event),
                "shared_signal_count": compute_shared_signal_count(
                    previous_event,
                    current_event,
                ),
                "time_delta_hours": compute_time_delta_hours(
                    previous_event.published_at,
                    current_event.published_at,
                ),
            }
        )
    return evidence_chain


def build_chain_summary(
    *,
    anchor: ScoredEvent,
    member_count: int,
    shared_keywords: list[str],
    shared_actors: list[str],
    shared_regions: list[str],
    evidence_chain: list[EventChainLink],
) -> str:
    evidence_parts: list[str] = []
    if shared_actors:
        evidence_parts.append(f"actors {', '.join(shared_actors)}")
    if shared_regions:
        evidence_parts.append(f"regions {', '.join(shared_regions)}")
    if shared_keywords:
        evidence_parts.append(f"keywords {', '.join(shared_keywords)}")

    if evidence_parts:
        evidence_text = ", ".join(evidence_parts)
    else:
        evidence_text = "repeated field-aligned evidence"

    chain_link_count = len(evidence_chain)
    chain_text = (
        f" through {chain_link_count} corroborating link"
        f"{'s' if chain_link_count != 1 else ''}"
        if chain_link_count > 0
        else ""
    )
    return (
        f"{member_count} related {anchor.dominant_field} events reinforce '{anchor.title}'"
        f" via {evidence_text}{chain_text}."
    )


def build_causal_summary(
    causal_ordered_group: list[ScoredEvent],
    evidence_chain: list[EventChainLink],
) -> str:
    if not causal_ordered_group:
        return "No causal cluster available."
    first_event = causal_ordered_group[0]
    last_event = causal_ordered_group[-1]
    span_hours = compute_group_causal_span_hours(causal_ordered_group)
    relationship = (
        evidence_chain[0]["relationship"]
        if evidence_chain
        else infer_group_relationship(first_event)
    )
    span_text = f" over {span_hours}h" if span_hours is not None else ""
    if first_event.event_id == last_event.event_id:
        return f"Single-event {relationship} cluster anchored on '{first_event.title}'."
    if span_hours is None:
        return (
            f"{relationship} cluster linking '{first_event.title}' to '{last_event.title}'"
            f" across {len(causal_ordered_group)} events."
        )
    return (
        f"{relationship} cluster from '{first_event.title}' to '{last_event.title}'"
        f" across {len(causal_ordered_group)} events{span_text}."
    )


def sort_group_for_causal_chain(
    group: list[ScoredEvent],
    parent_event_ids: dict[str, str | None],
) -> list[ScoredEvent]:
    events_by_id = {event.event_id: event for event in group}
    ordered_events = sorted(
        group, key=lambda event: (-event.divergence_score, event.event_id)
    )
    root = next(
        (
            event
            for event in ordered_events
            if parent_event_ids.get(event.event_id) is None
        ),
        ordered_events[0],
    )

    children_by_parent_id: dict[str, list[ScoredEvent]] = {}
    for event in ordered_events:
        parent_event_id = parent_event_ids.get(event.event_id)
        if parent_event_id is None:
            continue
        children_by_parent_id.setdefault(parent_event_id, []).append(event)

    def child_sort_key(event: ScoredEvent) -> tuple[datetime, float, str]:
        parsed_time = parse_event_time(event.published_at)
        return (
            parsed_time or datetime.max.replace(tzinfo=UTC),
            -event.divergence_score,
            event.event_id,
        )

    ordered_chain: list[ScoredEvent] = []

    def visit(event: ScoredEvent) -> None:
        ordered_chain.append(event)
        for child in sorted(
            children_by_parent_id.get(event.event_id, []), key=child_sort_key
        ):
            visit(child)

    visit(root)
    return ordered_chain


def infer_group_relationship(event: ScoredEvent) -> str:
    if event.dominant_field == "conflict":
        return "escalation"
    if event.dominant_field == "diplomacy":
        return "negotiation"
    if event.dominant_field == "economy":
        return "pressure"
    if event.dominant_field == "technology":
        return "capability-race"
    return "reinforcing"


def compute_shared_signal_count(left: ScoredEvent, right: ScoredEvent) -> int:
    return (
        len(intersection(left.keywords, right.keywords))
        + len(intersection(left.actors, right.actors))
        + len(intersection(left.regions, right.regions))
    )


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


def compute_time_delta_hours(
    left_published_at: str | None,
    right_published_at: str | None,
) -> float | None:
    left_time = parse_event_time(left_published_at)
    right_time = parse_event_time(right_published_at)
    if left_time is None or right_time is None:
        return None
    return round(abs(right_time - left_time).total_seconds() / 3600, 2)


def compute_group_causal_span_hours(group: list[ScoredEvent]) -> float | None:
    if len(group) < 2:
        return 0.0
    parsed_times = [parse_event_time(event.published_at) for event in group]
    known_times = [
        parsed_time for parsed_time in parsed_times if parsed_time is not None
    ]
    if len(known_times) < 2:
        return None
    return round(abs(max(known_times) - min(known_times)).total_seconds() / 3600, 2)


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
