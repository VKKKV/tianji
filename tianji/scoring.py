from __future__ import annotations

from collections import Counter

from .models import NormalizedEvent, ScoredEvent


REGION_WEIGHTS = {
    "ukraine": 2.5,
    "russia": 2.0,
    "middle-east": 2.5,
    "east-asia": 2.0,
    "united-states": 1.0,
    "europe": 1.0,
}

ACTOR_WEIGHTS = {
    "nato": 1.5,
    "eu": 1.0,
    "un": 1.0,
    "usa": 1.5,
    "china": 1.5,
    "russia": 1.5,
    "iran": 1.2,
}

IMPACT_WEIGHT = 0.65
FIELD_ATTRACTION_WEIGHT = 1.35
FA_MARGIN_WEIGHT = 0.15
FA_MAX_MARGIN_BONUS = 1.0
FA_COHERENCE_WEIGHT = 0.75
IM_DOMINANT_FIELD_WEIGHT = 0.25
IM_NONZERO_FIELD_WEIGHT = 0.2


def score_events(events: list[NormalizedEvent]) -> list[ScoredEvent]:
    scored = [score_event(event) for event in events]
    return sorted(scored, key=lambda item: item.divergence_score, reverse=True)


def score_event(event: NormalizedEvent) -> ScoredEvent:
    dominant_field, dominant_field_strength = select_dominant_field(event)
    fa_score = compute_fa(event, dominant_field_strength)
    im_score = compute_im(event, dominant_field_strength, event.field_scores)
    divergence_score = compute_divergence_score(im_score, fa_score)
    rationale = build_rationale(
        event=event,
        dominant_field=dominant_field,
        im_score=im_score,
        fa_score=fa_score,
    )
    return ScoredEvent(
        event_id=event.event_id,
        title=event.title,
        source=event.source,
        link=event.link,
        published_at=event.published_at,
        actors=event.actors,
        regions=event.regions,
        keywords=event.keywords,
        dominant_field=dominant_field,
        impact_score=im_score,
        field_attraction=fa_score,
        divergence_score=divergence_score,
        rationale=rationale,
    )


def compute_im(
    event: NormalizedEvent,
    dominant_field_strength: float,
    field_scores: dict[str, float],
) -> float:
    actor_weight = sum(ACTOR_WEIGHTS.get(actor, 0.6) for actor in event.actors)
    region_weight = sum(REGION_WEIGHTS.get(region, 0.5) for region in event.regions)
    keyword_density = min(len(event.keywords) * 0.25, 3.0)
    nonzero_field_count = sum(1 for score in field_scores.values() if score > 0)
    evidence_bonus = (dominant_field_strength * IM_DOMINANT_FIELD_WEIGHT) + (
        nonzero_field_count * IM_NONZERO_FIELD_WEIGHT
    )
    return round(
        3.0 + actor_weight + region_weight + keyword_density + evidence_bonus, 2
    )


def select_dominant_field(event: NormalizedEvent) -> tuple[str, float]:
    dominant_field, field_strength = max(
        event.field_scores.items(),
        key=lambda item: item[1],
        default=("uncategorized", 0.0),
    )
    return dominant_field, round(field_strength, 2)


def compute_fa(event: NormalizedEvent, dominant_field_strength: float) -> float:
    ordered_scores = sorted(event.field_scores.values(), reverse=True)
    second_best_strength = ordered_scores[1] if len(ordered_scores) > 1 else 0.0
    total_strength = sum(score for score in event.field_scores.values() if score > 0)

    margin_bonus = min(
        max(dominant_field_strength - second_best_strength, 0.0) * FA_MARGIN_WEIGHT,
        FA_MAX_MARGIN_BONUS,
    )
    coherence_bonus = 0.0
    if total_strength > 0:
        coherence_bonus = (
            dominant_field_strength / total_strength
        ) * FA_COHERENCE_WEIGHT

    return round(dominant_field_strength + margin_bonus + coherence_bonus, 2)


def compute_divergence_score(im_score: float, fa_score: float) -> float:
    return round((im_score * IMPACT_WEIGHT) + (fa_score * FIELD_ATTRACTION_WEIGHT), 2)


def build_rationale(
    *,
    event: NormalizedEvent,
    dominant_field: str,
    im_score: float,
    fa_score: float,
) -> list[str]:
    rationale = [f"Im={im_score}", f"Fa={fa_score}"]
    if event.actors:
        rationale.append(f"actors={', '.join(event.actors)}")
    if event.regions:
        rationale.append(f"regions={', '.join(event.regions)}")
    if fa_score > 0:
        rationale.append(f"dominant_field={dominant_field}:{fa_score}")
    else:
        rationale.append("dominant_field=uncategorized:0")
    return rationale


def summarize_scenario(scored_events: list[ScoredEvent]) -> dict[str, object]:
    if not scored_events:
        return {
            "headline": "No high-signal events were available for inference.",
            "dominant_field": "uncategorized",
            "risk_level": "low",
            "top_regions": [],
            "top_actors": [],
        }

    top_events = scored_events[:3]
    field_counts = Counter(event.dominant_field for event in scored_events)
    region_counts = Counter(region for event in top_events for region in event.regions)
    actor_counts = Counter(actor for event in top_events for actor in event.actors)
    average_score = sum(event.divergence_score for event in top_events) / len(
        top_events
    )
    risk_level = (
        "high" if average_score >= 9 else "medium" if average_score >= 6 else "low"
    )
    dominant_field = field_counts.most_common(1)[0][0]
    headline = (
        f"The strongest current branch is {dominant_field}, driven by "
        f"{top_events[0].title.lower()} and {len(top_events) - 1} additional high-signal events."
    )
    return {
        "headline": headline,
        "dominant_field": dominant_field,
        "risk_level": risk_level,
        "top_regions": [name for name, _ in region_counts.most_common(3)],
        "top_actors": [name for name, _ in actor_counts.most_common(3)],
    }
