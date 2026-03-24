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

WEAK_GROUP_INTERVENTION_TYPES = {
    "conflict": "escalation-containment",
    "diplomacy": "channel-stabilization",
    "economy": "market-stabilization",
    "technology": "capability-containment",
}

MIN_STRONG_GROUP_SHARED_SIGNAL_COUNT = 5
FAST_GROUP_CAUSAL_SPAN_HOURS = 2.0


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
        headline_role_text = infer_group_headline_role_text(event_group)
        if headline_role_text in {
            " headline role=chain endpoint;",
            " headline role=chain pivot;",
        }:
            if event.actors:
                return event.actors[0]
            if event.regions:
                return event.regions[0]
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
    member_count = event_group["member_count"]
    evidence_chain = event_group["evidence_chain"]
    link_count = len(evidence_chain)
    if (
        member_count >= 3
        and link_count >= 2
        and all(
            link["shared_signal_count"] >= MIN_STRONG_GROUP_SHARED_SIGNAL_COUNT
            for link in evidence_chain
        )
    ):
        return STRONG_GROUP_INTERVENTION_TYPES.get(
            event_group["dominant_field"],
            "pattern-disruption",
        )
    if member_count >= 2 and link_count >= 1:
        return WEAK_GROUP_INTERVENTION_TYPES.get(
            event_group["dominant_field"],
            "pattern-monitoring",
        )
    return None


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
    relationship_phrase = infer_group_effect_relationship_phrase(event_group)
    role_phrase = infer_group_effect_role_phrase(event_group)
    urgency_prefix = infer_group_effect_urgency_prefix(event_group, link_count)
    conflict_action = "disrupt" if urgency_prefix else "Disrupt"
    diplomacy_action = "stabilize" if urgency_prefix else "Stabilize"
    economy_action = "interrupt" if urgency_prefix else "Interrupt"
    generic_action = "break" if urgency_prefix else "Break"
    if event.dominant_field == "conflict":
        return f"{urgency_prefix}{conflict_action} the {chain_type}{relationship_phrase}{role_phrase} before escalation compounds across {member_count} related events."
    if event.dominant_field == "diplomacy":
        return f"{urgency_prefix}{diplomacy_action} the {chain_type}{relationship_phrase}{role_phrase} so {member_count} related diplomatic moves do not harden into a wider standoff."
    if event.dominant_field == "economy":
        return f"{urgency_prefix}{economy_action} the {chain_type}{relationship_phrase}{role_phrase} before {member_count} linked economic signals compound into a broader shock."
    if event.dominant_field == "technology":
        return f"{urgency_prefix}{conflict_action} the {chain_type}{relationship_phrase}{role_phrase} before {member_count} related capability moves harden into a broader race."
    return f"{urgency_prefix}{generic_action} the {chain_type}{relationship_phrase}{role_phrase} and collect better evidence before {member_count} related events reinforce the branch further."


def infer_group_effect_relationship_phrase(event_group: EventGroupSummary) -> str:
    dominant_relationship = infer_group_dominant_relationship(event_group)
    return (
        ""
        if dominant_relationship == "reinforcing"
        else f" in the {dominant_relationship} pattern"
    )


def infer_group_effect_role_phrase(event_group: EventGroupSummary) -> str:
    headline_role_text = infer_group_headline_role_text(event_group)
    if headline_role_text == " headline role=chain origin;":
        return " at the chain origin"
    if headline_role_text == " headline role=chain endpoint;":
        return " at the chain endpoint"
    if headline_role_text == " headline role=chain pivot;":
        return " at the chain pivot"
    return ""


def infer_group_effect_urgency_prefix(
    event_group: EventGroupSummary,
    link_count: int,
) -> str:
    causal_span_hours = event_group["causal_span_hours"]
    if causal_span_hours is None:
        return ""
    if causal_span_hours <= FAST_GROUP_CAUSAL_SPAN_HOURS:
        return "Urgently " if link_count >= 2 else "Quickly "
    return ""


def infer_group_corroboration_text(event_group: EventGroupSummary) -> str:
    if all(
        link["shared_signal_count"] >= MIN_STRONG_GROUP_SHARED_SIGNAL_COUNT
        for link in event_group["evidence_chain"]
    ):
        return " high corroboration across causal links;"
    return " moderate corroboration across causal links;"


def infer_group_dominant_relationship(event_group: EventGroupSummary) -> str:
    relationship_counts: dict[str, int] = {}
    for link in event_group["evidence_chain"]:
        relationship = link["relationship"]
        relationship_counts[relationship] = relationship_counts.get(relationship, 0) + 1
    return min(
        relationship_counts.items(),
        key=lambda item: (-item[1], item[0]),
    )[0]


def infer_group_relationship_text(event_group: EventGroupSummary) -> str:
    dominant_relationship = infer_group_dominant_relationship(event_group)
    return f" dominant relationship={dominant_relationship};"


def infer_group_signal_support_text(event_group: EventGroupSummary) -> str:
    signal_counts = [
        link["shared_signal_count"] for link in event_group["evidence_chain"]
    ]
    min_signal_count = min(signal_counts)
    max_signal_count = max(signal_counts)
    if min_signal_count == max_signal_count:
        return f" signal support={min_signal_count};"
    return f" signal support range={min_signal_count}-{max_signal_count};"


def infer_group_link_tempo_text(event_group: EventGroupSummary) -> str:
    link_deltas = [
        link["time_delta_hours"]
        for link in event_group["evidence_chain"]
        if link["time_delta_hours"] is not None
    ]
    if not link_deltas:
        return ""
    min_link_delta = min(link_deltas)
    max_link_delta = max(link_deltas)
    if min_link_delta == max_link_delta:
        return f" link tempo={min_link_delta}h;"
    return f" link tempo range={min_link_delta}-{max_link_delta}h;"


def infer_group_headline_role_text(event_group: EventGroupSummary) -> str:
    causal_ordered_event_ids = event_group["causal_ordered_event_ids"]
    headline_event_id = event_group["headline_event_id"]
    if not causal_ordered_event_ids or len(causal_ordered_event_ids) == 1:
        return " headline role=standalone;"
    if headline_event_id == causal_ordered_event_ids[0]:
        return " headline role=chain origin;"
    if headline_event_id == causal_ordered_event_ids[-1]:
        return " headline role=chain endpoint;"
    return " headline role=chain pivot;"


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
    corroboration_text = infer_group_corroboration_text(event_group)
    relationship_text = infer_group_relationship_text(event_group)
    signal_support_text = infer_group_signal_support_text(event_group)
    link_tempo_text = infer_group_link_tempo_text(event_group)
    headline_role_text = infer_group_headline_role_text(event_group)
    return (
        f"{base_reason} Grouped context: {event_group['member_count']}-event {event_group['dominant_field']} cluster"
        f" with {len(event_group['evidence_chain'])} causal link(s){span_text};"
        f"{corroboration_text}"
        f"{relationship_text}"
        f"{signal_support_text}"
        f"{link_tempo_text}"
        f"{headline_role_text}"
        f"{shared_actor_text}{shared_region_text}"
        f" Evidence chain: {event_group['chain_summary']} "
        f"Causal cluster: {event_group['causal_summary']}"
    )
