from __future__ import annotations

from collections import Counter
import re

from .models import NormalizedEvent, ScoredEvent
from .normalize import ACTOR_PATTERNS, FIELD_KEYWORDS, REGION_PATTERNS, match_patterns


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
FA_NEAR_TIE_MARGIN_THRESHOLD = 1.0
FA_NEAR_TIE_WEIGHT = 0.35
FA_MAX_NEAR_TIE_PENALTY = 0.3
FA_DIFFUSE_THIRD_FIELD_THRESHOLD = 2.5
FA_DIFFUSE_THIRD_FIELD_WEIGHT = 0.08
FA_MAX_DIFFUSE_THIRD_FIELD_PENALTY = 0.2
IM_DOMINANT_FIELD_WEIGHT = 0.25
IM_NONZERO_FIELD_WEIGHT = 0.2
IM_NONZERO_FIELD_MIN_SCORE = 1.0
IM_TITLE_SALIENCE_ACTOR_MULTIPLIER = 0.2
IM_TITLE_SALIENCE_REGION_MULTIPLIER = 0.2
IM_TITLE_SALIENCE_ACTOR_MAX_PER_MATCH = 0.35
IM_TITLE_SALIENCE_REGION_MAX_PER_MATCH = 0.4
IM_TITLE_SALIENCE_MAX_BONUS = 0.8
IM_FIELD_IMPACT_BASELINE_AVERAGE_WEIGHT = 1.5
IM_FIELD_IMPACT_SCALE_WEIGHT = 0.06
IM_FIELD_IMPACT_MAX_BONUS = 0.5
IM_TEXT_SIGNAL_KEYWORD_WEIGHT = 0.12
IM_TEXT_SIGNAL_TITLE_WEIGHT = 0.2
IM_TEXT_SIGNAL_SUMMARY_WEIGHT = 0.1
IM_TEXT_SIGNAL_MAX_KEYWORD_HITS = 4
IM_TEXT_SIGNAL_MAX_TITLE_HITS = 2
IM_TEXT_SIGNAL_MAX_SUMMARY_HITS = 2
IM_TEXT_SIGNAL_MAX_BONUS = 1.0
TEXT_SIGNAL_PATTERN_CACHE: dict[str, re.Pattern[str]] = {}


def score_events(events: list[NormalizedEvent]) -> list[ScoredEvent]:
    scored = [score_event(event) for event in events]
    return sorted(scored, key=lambda item: item.divergence_score, reverse=True)


def score_event(event: NormalizedEvent) -> ScoredEvent:
    dominant_field, dominant_field_strength = select_dominant_field(event)
    title_salience_bonus = compute_title_salience_bonus(event)
    field_impact_scaling_bonus = compute_field_impact_scaling_bonus(
        dominant_field=dominant_field,
        dominant_field_strength=dominant_field_strength,
    )
    text_signal_intensity = compute_text_signal_intensity(event, dominant_field)
    fa_score = compute_fa(event, dominant_field_strength)
    im_score = compute_im(
        event,
        dominant_field_strength,
        event.field_scores,
        title_salience_bonus,
        field_impact_scaling_bonus,
        text_signal_intensity,
    )
    divergence_score = compute_divergence_score(im_score, fa_score)
    rationale = build_rationale(
        event=event,
        dominant_field=dominant_field,
        im_score=im_score,
        fa_score=fa_score,
        title_salience_bonus=title_salience_bonus,
        field_impact_scaling_bonus=field_impact_scaling_bonus,
        text_signal_intensity=text_signal_intensity,
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
    title_salience_bonus: float,
    field_impact_scaling_bonus: float,
    text_signal_intensity: float,
) -> float:
    actor_weight = sum(ACTOR_WEIGHTS.get(actor, 0.6) for actor in event.actors)
    region_weight = sum(REGION_WEIGHTS.get(region, 0.5) for region in event.regions)
    keyword_density = min(len(event.keywords) * 0.25, 3.0)
    nonzero_field_count = sum(
        1 for score in field_scores.values() if score >= IM_NONZERO_FIELD_MIN_SCORE
    )
    if nonzero_field_count == 0 and dominant_field_strength > 0:
        nonzero_field_count = 1
    evidence_bonus = (dominant_field_strength * IM_DOMINANT_FIELD_WEIGHT) + (
        nonzero_field_count * IM_NONZERO_FIELD_WEIGHT
    )
    return round(
        3.0
        + actor_weight
        + region_weight
        + title_salience_bonus
        + keyword_density
        + evidence_bonus
        + field_impact_scaling_bonus
        + text_signal_intensity,
        2,
    )


def compute_title_salience_bonus(event: NormalizedEvent) -> float:
    title_actors = set(match_patterns(event.title, ACTOR_PATTERNS))
    title_regions = set(match_patterns(event.title, REGION_PATTERNS))
    actor_bonus = sum(
        min(
            ACTOR_WEIGHTS.get(actor, 0.6) * IM_TITLE_SALIENCE_ACTOR_MULTIPLIER,
            IM_TITLE_SALIENCE_ACTOR_MAX_PER_MATCH,
        )
        for actor in event.actors
        if actor in title_actors
    )
    region_bonus = sum(
        min(
            REGION_WEIGHTS.get(region, 0.5) * IM_TITLE_SALIENCE_REGION_MULTIPLIER,
            IM_TITLE_SALIENCE_REGION_MAX_PER_MATCH,
        )
        for region in event.regions
        if region in title_regions
    )
    return round(min(actor_bonus + region_bonus, IM_TITLE_SALIENCE_MAX_BONUS), 2)


def compute_field_impact_scaling_bonus(
    *, dominant_field: str, dominant_field_strength: float
) -> float:
    dominant_keywords = FIELD_KEYWORDS.get(dominant_field)
    if not dominant_keywords or dominant_field_strength <= 0:
        return 0.0
    average_keyword_weight = sum(dominant_keywords.values()) / len(dominant_keywords)
    return round(
        min(
            max(
                average_keyword_weight - IM_FIELD_IMPACT_BASELINE_AVERAGE_WEIGHT,
                0.0,
            )
            * dominant_field_strength
            * IM_FIELD_IMPACT_SCALE_WEIGHT,
            IM_FIELD_IMPACT_MAX_BONUS,
        ),
        2,
    )


def compute_text_signal_intensity(event: NormalizedEvent, dominant_field: str) -> float:
    dominant_keywords = FIELD_KEYWORDS.get(dominant_field, {})
    if not dominant_keywords:
        return 0.0

    keyword_hits = min(
        sum(1 for keyword in event.keywords if keyword in dominant_keywords),
        IM_TEXT_SIGNAL_MAX_KEYWORD_HITS,
    )
    title_hits = count_text_signal_surface_hits(
        text=event.title,
        dominant_keywords=dominant_keywords,
        max_hits=IM_TEXT_SIGNAL_MAX_TITLE_HITS,
    )
    summary_hits = count_text_signal_surface_hits(
        text=event.summary,
        dominant_keywords=dominant_keywords,
        max_hits=IM_TEXT_SIGNAL_MAX_SUMMARY_HITS,
    )

    return round(
        min(
            (keyword_hits * IM_TEXT_SIGNAL_KEYWORD_WEIGHT)
            + (title_hits * IM_TEXT_SIGNAL_TITLE_WEIGHT)
            + (summary_hits * IM_TEXT_SIGNAL_SUMMARY_WEIGHT),
            IM_TEXT_SIGNAL_MAX_BONUS,
        ),
        2,
    )


def count_text_signal_surface_hits(
    *, text: str, dominant_keywords: dict[str, float], max_hits: int
) -> int:
    lowered_text = text.lower()
    return min(
        sum(
            1
            for keyword in dominant_keywords
            if get_text_signal_pattern(keyword).search(lowered_text)
        ),
        max_hits,
    )


def get_text_signal_pattern(keyword: str) -> re.Pattern[str]:
    pattern = TEXT_SIGNAL_PATTERN_CACHE.get(keyword)
    if pattern is None:
        pattern = re.compile(rf"\b{re.escape(keyword)}\b")
        TEXT_SIGNAL_PATTERN_CACHE[keyword] = pattern
    return pattern


def select_dominant_field(event: NormalizedEvent) -> tuple[str, float]:
    total_strength = sum(score for score in event.field_scores.values() if score > 0)
    if total_strength <= 0:
        return "uncategorized", 0.0
    max_strength = max(event.field_scores.values(), default=0.0)
    tied_fields = sorted(
        field_name
        for field_name, field_strength in event.field_scores.items()
        if field_strength == max_strength and field_strength > 0
    )
    if not tied_fields:
        return "uncategorized", 0.0
    return tied_fields[0], round(max_strength, 2)


def compute_fa(event: NormalizedEvent, dominant_field_strength: float) -> float:
    ordered_scores = sorted(event.field_scores.values(), reverse=True)
    second_best_strength = (
        round(ordered_scores[1], 2) if len(ordered_scores) > 1 else 0.0
    )
    third_best_strength = (
        round(ordered_scores[2], 2) if len(ordered_scores) > 2 else 0.0
    )
    total_strength = sum(score for score in event.field_scores.values() if score > 0)
    if total_strength <= 0:
        return 0.0
    dominant_margin = max(dominant_field_strength - second_best_strength, 0.0)

    margin_bonus = min(
        dominant_margin * FA_MARGIN_WEIGHT,
        FA_MAX_MARGIN_BONUS,
    )
    coherence_bonus = 0.0
    if total_strength > 0:
        coherence_bonus = (
            dominant_field_strength / total_strength
        ) * FA_COHERENCE_WEIGHT
    near_tie_penalty = min(
        max(FA_NEAR_TIE_MARGIN_THRESHOLD - dominant_margin, 0.0) * FA_NEAR_TIE_WEIGHT,
        FA_MAX_NEAR_TIE_PENALTY,
    )
    diffuse_third_field_penalty = 0.0
    if dominant_margin >= FA_NEAR_TIE_MARGIN_THRESHOLD:
        diffuse_third_field_penalty = min(
            max(third_best_strength - FA_DIFFUSE_THIRD_FIELD_THRESHOLD, 0.0)
            * FA_DIFFUSE_THIRD_FIELD_WEIGHT,
            FA_MAX_DIFFUSE_THIRD_FIELD_PENALTY,
        )

    return round(
        dominant_field_strength
        + margin_bonus
        + coherence_bonus
        - near_tie_penalty
        - diffuse_third_field_penalty,
        2,
    )


def compute_divergence_score(im_score: float, fa_score: float) -> float:
    return round((im_score * IMPACT_WEIGHT) + (fa_score * FIELD_ATTRACTION_WEIGHT), 2)


def build_rationale(
    *,
    event: NormalizedEvent,
    dominant_field: str,
    im_score: float,
    fa_score: float,
    title_salience_bonus: float,
    field_impact_scaling_bonus: float,
    text_signal_intensity: float,
) -> list[str]:
    rationale = [f"Im={im_score}", f"Fa={fa_score}"]
    if title_salience_bonus > 0:
        rationale.append(f"im_title_salience={title_salience_bonus}")
    if field_impact_scaling_bonus > 0:
        rationale.append(f"im_field_impact_scaling={field_impact_scaling_bonus}")
    dominant_field_keywords = FIELD_KEYWORDS.get(dominant_field, {})
    if dominant_field_keywords:
        rationale.append(f"im_text_signal_intensity={text_signal_intensity}")
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
    max_field_count = max(field_counts.values(), default=0)
    tied_fields = [
        field_name
        for field_name, field_count in field_counts.items()
        if field_count == max_field_count
    ]
    best_field_divergence = {
        field_name: max(
            event.divergence_score
            for event in scored_events
            if event.dominant_field == field_name
        )
        for field_name in tied_fields
    }
    dominant_field = sorted(
        tied_fields,
        key=lambda field_name: (-best_field_divergence[field_name], field_name),
    )[0]
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
