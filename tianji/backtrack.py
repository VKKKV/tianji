from __future__ import annotations

from collections.abc import Sequence
from typing import TypedDict

from .models import InterventionCandidate, ScoredEvent


FIELD_INTERVENTION_TYPES = {
    "conflict": "de-escalation",
    "diplomacy": "negotiation",
    "economy": "economic-pressure",
    "technology": "capability-control",
}

STRONG_GROUP_INTERVENTION_TYPES = {
    "conflict": "escalation-override",
    "diplomacy": "treaty-invalidation",
    "economy": "market-freeze",
    "technology": "capability-freeze",
}


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
        event_group = event_group_by_headline_id.get(event.event_id)
        target = select_intervention_target(
            event,
            event_group=event_group,
        )
        intervention_type = infer_intervention_type(
            event,
            event_group=event_group,
        )
        expected_effect = infer_expected_effect(
            event,
            event_group=event_group,
        )
        reason = build_reason(event, event_group)
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


def select_intervention_target(
    event: ScoredEvent,
    *,
    event_group: EventGroupSummary | None = None,
) -> str:
    if event_group is not None:
        if event_group["shared_actors"]:
            return event_group["shared_actors"][0]
        if event_group["shared_regions"]:
            return event_group["shared_regions"][0]
    if event.actors:
        return event.actors[0]
    if event.regions:
        return event.regions[0]
    return event.source


def infer_intervention_type(
    event: ScoredEvent,
    *,
    event_group: EventGroupSummary | None = None,
) -> str:
    if event_group is not None:
        grouped_intervention_type = infer_group_intervention_type(event_group)
        if grouped_intervention_type is not None:
            return grouped_intervention_type
    return infer_field_intervention_type(event.dominant_field)


def infer_group_intervention_type(
    event_group: EventGroupSummary,
) -> str | None:
    if event_group["member_count"] < 3 or len(event_group["evidence_chain"]) < 2:
        return None
    return STRONG_GROUP_INTERVENTION_TYPES.get(
        event_group["dominant_field"],
        "pattern-disruption",
    )


def infer_field_intervention_type(dominant_field: str) -> str:
    return FIELD_INTERVENTION_TYPES.get(dominant_field, "information-gathering")


def infer_expected_effect(
    event: ScoredEvent,
    *,
    event_group: EventGroupSummary | None = None,
) -> str:
    if event_group is not None:
        return infer_group_expected_effect(event, event_group)
    if event.dominant_field == "conflict":
        return "Reduce near-term escalation incentives around the triggering event."
    if event.dominant_field == "diplomacy":
        return "Shift the branch toward a negotiated or paused outcome."
    if event.dominant_field == "economy":
        return "Change economic signaling before it compounds into a wider crisis."
    if event.dominant_field == "technology":
        return "Constrain a fast-moving capability race before spillover grows."
    return "Collect better evidence before attempting stronger intervention."


def infer_group_expected_effect(
    event: ScoredEvent,
    event_group: EventGroupSummary,
) -> str:
    member_count = event_group["member_count"]
    link_count = len(event_group["evidence_chain"])
    chain_type = "reinforcing chain" if link_count >= 2 else "linked cluster"
    if event.dominant_field == "conflict":
        return f"Disrupt the {chain_type} before escalation compounds across {member_count} related events."
    if event.dominant_field == "diplomacy":
        return f"Stabilize the {chain_type} so {member_count} related diplomatic moves do not harden into a wider standoff."
    if event.dominant_field == "economy":
        return f"Interrupt the {chain_type} before {member_count} linked economic signals compound into a broader shock."
    if event.dominant_field == "technology":
        return f"Disrupt the {chain_type} before {member_count} related capability moves harden into a broader race."
    return f"Break the {chain_type} and collect better evidence before {member_count} related events reinforce the branch further."


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
    shared_actor_text = (
        f" shared actors={', '.join(event_group['shared_actors'])};"
        if event_group["shared_actors"]
        else ""
    )
    shared_region_text = (
        f" shared regions={', '.join(event_group['shared_regions'])};"
        if event_group["shared_regions"]
        else ""
    )
    span_text = (
        f" over {event_group['causal_span_hours']}h"
        if event_group["causal_span_hours"] is not None
        else ""
    )
    return (
        f"{base_reason} Grouped context: {event_group['member_count']}-event {event_group['dominant_field']} cluster"
        f" with {len(event_group['evidence_chain'])} causal link(s){span_text};"
        f"{shared_actor_text}{shared_region_text}"
        f" Evidence chain: {event_group['chain_summary']} "
        f"Causal cluster: {event_group['causal_summary']}"
    )
