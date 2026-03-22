from __future__ import annotations

from collections.abc import Sequence
from typing import TypedDict

from .models import InterventionCandidate, ScoredEvent


class EventChainLink(TypedDict):
    from_event_id: str
    to_event_id: str
    shared_keywords: list[str]
    shared_actors: list[str]
    shared_regions: list[str]
    relationship: str
    shared_signal_count: int
    time_delta_hours: float | None


class EventGroupSummary(TypedDict):
    group_id: str
    headline_event_id: str
    headline_title: str
    member_event_ids: list[str]
    member_count: int
    dominant_field: str
    shared_keywords: list[str]
    shared_actors: list[str]
    shared_regions: list[str]
    group_score: float
    causal_ordered_event_ids: list[str]
    causal_span_hours: float | None
    evidence_chain: list[EventChainLink]
    chain_summary: str
    causal_summary: str


def backtrack_candidates(
    scored_events: list[ScoredEvent],
    limit: int = 5,
    event_groups: Sequence[EventGroupSummary] | None = None,
) -> list[InterventionCandidate]:
    candidates: list[InterventionCandidate] = []
    event_group_by_headline_id = {
        group["headline_event_id"]: group for group in event_groups or []
    }
    selected_events = select_backtrack_events(scored_events, limit, event_groups)
    for index, event in enumerate(selected_events, start=1):
        target = (
            event.actors[0]
            if event.actors
            else event.regions[0]
            if event.regions
            else event.source
        )
        intervention_type = infer_intervention_type(event)
        expected_effect = infer_expected_effect(event)
        reason = build_reason(event, event_group_by_headline_id.get(event.event_id))
        candidates.append(
            InterventionCandidate(
                priority=index,
                event_id=event.event_id,
                target=target,
                intervention_type=intervention_type,
                reason=reason,
                expected_effect=expected_effect,
            )
        )
    return candidates


def select_backtrack_events(
    scored_events: list[ScoredEvent],
    limit: int,
    event_groups: Sequence[EventGroupSummary] | None,
) -> list[ScoredEvent]:
    if not event_groups:
        return scored_events[:limit]

    events_by_id = {event.event_id: event for event in scored_events}
    selected: list[ScoredEvent] = []
    seen_event_ids: set[str] = set()

    for group in event_groups:
        headline_event_id = group["headline_event_id"]
        member_event_ids = group["member_event_ids"]
        headline_event = events_by_id.get(headline_event_id)
        if headline_event is None:
            continue
        selected.append(headline_event)
        seen_event_ids.update(member_event_ids)
        if len(selected) >= limit:
            return selected[:limit]

    for event in scored_events:
        if event.event_id in seen_event_ids:
            continue
        selected.append(event)
        if len(selected) >= limit:
            break

    return selected[:limit]


def infer_intervention_type(event: ScoredEvent) -> str:
    if event.dominant_field == "conflict":
        return "de-escalation"
    if event.dominant_field == "diplomacy":
        return "negotiation"
    if event.dominant_field == "economy":
        return "economic-pressure"
    if event.dominant_field == "technology":
        return "capability-control"
    return "information-gathering"


def infer_expected_effect(event: ScoredEvent) -> str:
    if event.dominant_field == "conflict":
        return "Reduce near-term escalation incentives around the triggering event."
    if event.dominant_field == "diplomacy":
        return "Shift the branch toward a negotiated or paused outcome."
    if event.dominant_field == "economy":
        return "Change economic signaling before it compounds into a wider crisis."
    if event.dominant_field == "technology":
        return "Constrain a fast-moving capability race before spillover grows."
    return "Collect better evidence before attempting stronger intervention."


def build_reason(
    event: ScoredEvent,
    event_group: EventGroupSummary | None = None,
) -> str:
    actor_text = ", ".join(event.actors) if event.actors else event.source
    region_text = ", ".join(event.regions) if event.regions else "global"
    base_reason = (
        f"Backtracked from event '{event.title}' because its divergence score is {event.divergence_score} "
        f"with field={event.dominant_field}, actors={actor_text}, regions={region_text}."
    )
    if event_group is None:
        return base_reason
    return (
        f"{base_reason} Evidence chain: {event_group['chain_summary']} "
        f"Causal cluster: {event_group['causal_summary']}"
    )
