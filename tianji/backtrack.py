from __future__ import annotations

from .models import InterventionCandidate, ScoredEvent


def backtrack_candidates(
    scored_events: list[ScoredEvent], limit: int = 5
) -> list[InterventionCandidate]:
    candidates: list[InterventionCandidate] = []
    for index, event in enumerate(scored_events[:limit], start=1):
        target = (
            event.actors[0]
            if event.actors
            else event.regions[0]
            if event.regions
            else event.source
        )
        intervention_type = infer_intervention_type(event)
        expected_effect = infer_expected_effect(event)
        reason = build_reason(event)
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


def build_reason(event: ScoredEvent) -> str:
    actor_text = ", ".join(event.actors) if event.actors else event.source
    region_text = ", ".join(event.regions) if event.regions else "global"
    return (
        f"Backtracked from event '{event.title}' because its divergence score is {event.divergence_score} "
        f"with field={event.dominant_field}, actors={actor_text}, regions={region_text}."
    )
