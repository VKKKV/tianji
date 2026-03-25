from __future__ import annotations

from datetime import datetime
from typing import cast


def filter_scored_event_details(
    scored_events: list[dict[str, object]],
    *,
    dominant_field: str | None,
    min_impact_score: float | None,
    max_impact_score: float | None,
    min_field_attraction: float | None,
    max_field_attraction: float | None,
    min_divergence_score: float | None,
    max_divergence_score: float | None,
    limit_scored_events: int | None,
) -> list[dict[str, object]]:
    filtered = list(scored_events)
    if dominant_field is not None:
        filtered = [
            event for event in filtered if event.get("dominant_field") == dominant_field
        ]
    if min_impact_score is not None:
        filtered = [
            event
            for event in filtered
            if is_numeric_run_metric_at_or_above(
                event.get("impact_score"), min_impact_score
            )
        ]
    if max_impact_score is not None:
        filtered = [
            event
            for event in filtered
            if is_numeric_run_metric_at_or_below(
                event.get("impact_score"), max_impact_score
            )
        ]
    if min_field_attraction is not None:
        filtered = [
            event
            for event in filtered
            if is_numeric_run_metric_at_or_above(
                event.get("field_attraction"), min_field_attraction
            )
        ]
    if max_field_attraction is not None:
        filtered = [
            event
            for event in filtered
            if is_numeric_run_metric_at_or_below(
                event.get("field_attraction"), max_field_attraction
            )
        ]
    if min_divergence_score is not None:
        filtered = [
            event
            for event in filtered
            if is_numeric_run_metric_at_or_above(
                event.get("divergence_score"), min_divergence_score
            )
        ]
    if max_divergence_score is not None:
        filtered = [
            event
            for event in filtered
            if is_numeric_run_metric_at_or_below(
                event.get("divergence_score"), max_divergence_score
            )
        ]
    if limit_scored_events is not None:
        return filtered[:limit_scored_events]
    return filtered


def filter_intervention_candidate_details(
    intervention_candidates: list[dict[str, object]],
    *,
    visible_scored_event_ids: set[str],
    only_matching_interventions: bool,
) -> list[dict[str, object]]:
    if not only_matching_interventions:
        return intervention_candidates
    return [
        candidate
        for candidate in intervention_candidates
        if candidate.get("event_id") in visible_scored_event_ids
    ]


def filter_event_group_details(
    event_groups: list[dict[str, object]],
    *,
    dominant_field: str | None,
    limit_event_groups: int | None,
) -> list[dict[str, object]]:
    filtered = list(event_groups)
    if dominant_field is not None:
        filtered = [
            group for group in filtered if group.get("dominant_field") == dominant_field
        ]
    if limit_event_groups is not None:
        return filtered[:limit_event_groups]
    return filtered


def filter_run_list_items(
    items: list[dict[str, object]],
    *,
    mode: str | None,
    dominant_field: str | None,
    risk_level: str | None,
    since: str | None,
    until: str | None,
    min_top_impact_score: float | None = None,
    max_top_impact_score: float | None = None,
    min_top_field_attraction: float | None = None,
    max_top_field_attraction: float | None = None,
    min_top_divergence_score: float | None = None,
    max_top_divergence_score: float | None = None,
    top_group_dominant_field: str | None = None,
    min_event_group_count: int | None = None,
    max_event_group_count: int | None = None,
) -> list[dict[str, object]]:
    filtered = items
    if mode is not None:
        filtered = [item for item in filtered if item.get("mode") == mode]
    if dominant_field is not None:
        filtered = [
            item for item in filtered if item.get("dominant_field") == dominant_field
        ]
    if risk_level is not None:
        filtered = [item for item in filtered if item.get("risk_level") == risk_level]
    since_value = parse_history_timestamp(since)
    if since_value is not None:
        filtered = [
            item
            for item in filtered
            if is_history_timestamp_on_or_after(item.get("generated_at"), since_value)
        ]
    until_value = parse_history_timestamp(until)
    if until_value is not None:
        filtered = [
            item
            for item in filtered
            if is_history_timestamp_on_or_before(item.get("generated_at"), until_value)
        ]
    if min_top_impact_score is not None:
        filtered = [
            item
            for item in filtered
            if is_numeric_run_metric_at_or_above(
                item.get("top_impact_score"), min_top_impact_score
            )
        ]
    if max_top_impact_score is not None:
        filtered = [
            item
            for item in filtered
            if is_numeric_run_metric_at_or_below(
                item.get("top_impact_score"), max_top_impact_score
            )
        ]
    if min_top_field_attraction is not None:
        filtered = [
            item
            for item in filtered
            if is_numeric_run_metric_at_or_above(
                item.get("top_field_attraction"), min_top_field_attraction
            )
        ]
    if max_top_field_attraction is not None:
        filtered = [
            item
            for item in filtered
            if is_numeric_run_metric_at_or_below(
                item.get("top_field_attraction"), max_top_field_attraction
            )
        ]
    if min_top_divergence_score is not None:
        filtered = [
            item
            for item in filtered
            if is_numeric_run_metric_at_or_above(
                item.get("top_divergence_score"), min_top_divergence_score
            )
        ]
    if max_top_divergence_score is not None:
        filtered = [
            item
            for item in filtered
            if is_numeric_run_metric_at_or_below(
                item.get("top_divergence_score"), max_top_divergence_score
            )
        ]
    if top_group_dominant_field is not None:
        filtered = [
            item
            for item in filtered
            if item.get("top_event_group_dominant_field") == top_group_dominant_field
        ]
    if min_event_group_count is not None:
        filtered = [
            item
            for item in filtered
            if is_numeric_run_metric_at_or_above(
                item.get("event_group_count"), float(min_event_group_count)
            )
        ]
    if max_event_group_count is not None:
        filtered = [
            item
            for item in filtered
            if is_numeric_run_metric_at_or_below(
                item.get("event_group_count"), float(max_event_group_count)
            )
        ]
    return filtered


def is_numeric_run_metric_at_or_above(value: object, threshold: float) -> bool:
    if not isinstance(value, int | float):
        return False
    return float(value) >= threshold


def is_numeric_run_metric_at_or_below(value: object, threshold: float) -> bool:
    if not isinstance(value, int | float):
        return False
    return float(value) <= threshold


def parse_history_timestamp(value: str | None) -> datetime | None:
    if value is None:
        return None
    return datetime.fromisoformat(value.replace("Z", "+00:00"))


def is_history_timestamp_on_or_after(value: object, threshold: datetime) -> bool:
    if not isinstance(value, str):
        return False
    parsed = parse_history_timestamp(value)
    return parsed is not None and parsed >= threshold


def is_history_timestamp_on_or_before(value: object, threshold: datetime) -> bool:
    if not isinstance(value, str):
        return False
    parsed = parse_history_timestamp(value)
    return parsed is not None and parsed <= threshold
