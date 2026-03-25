from __future__ import annotations

from typing import cast

from .storage_views import get_run_summary


def compare_runs(
    *,
    sqlite_path: str,
    left_run_id: int,
    right_run_id: int,
    dominant_field: str | None = None,
    min_impact_score: float | None = None,
    max_impact_score: float | None = None,
    min_field_attraction: float | None = None,
    max_field_attraction: float | None = None,
    min_divergence_score: float | None = None,
    max_divergence_score: float | None = None,
    limit_scored_events: int | None = None,
    only_matching_interventions: bool = False,
    group_dominant_field: str | None = None,
    limit_event_groups: int | None = None,
) -> dict[str, object] | None:
    left = get_run_summary(
        sqlite_path=sqlite_path,
        run_id=left_run_id,
        dominant_field=dominant_field,
        min_impact_score=min_impact_score,
        max_impact_score=max_impact_score,
        min_field_attraction=min_field_attraction,
        max_field_attraction=max_field_attraction,
        min_divergence_score=min_divergence_score,
        max_divergence_score=max_divergence_score,
        limit_scored_events=limit_scored_events,
        only_matching_interventions=only_matching_interventions,
        group_dominant_field=group_dominant_field,
        limit_event_groups=limit_event_groups,
    )
    right = get_run_summary(
        sqlite_path=sqlite_path,
        run_id=right_run_id,
        dominant_field=dominant_field,
        min_impact_score=min_impact_score,
        max_impact_score=max_impact_score,
        min_field_attraction=min_field_attraction,
        max_field_attraction=max_field_attraction,
        min_divergence_score=min_divergence_score,
        max_divergence_score=max_divergence_score,
        limit_scored_events=limit_scored_events,
        only_matching_interventions=only_matching_interventions,
        group_dominant_field=group_dominant_field,
        limit_event_groups=limit_event_groups,
    )
    if left is None or right is None:
        return None

    left_summary = build_compare_side(left)
    right_summary = build_compare_side(right)
    return {
        "left_run_id": left_run_id,
        "right_run_id": right_run_id,
        "left": left_summary,
        "right": right_summary,
        "diff": build_compare_diff(left_summary, right_summary),
    }


def build_compare_side(run_payload: dict[str, object]) -> dict[str, object]:
    input_summary = cast(dict[str, object], run_payload["input_summary"])
    scenario_summary = cast(dict[str, object], run_payload["scenario_summary"])
    event_groups = cast(
        list[dict[str, object]], scenario_summary.get("event_groups", [])
    )
    scored_events = cast(list[dict[str, object]], run_payload["scored_events"])
    intervention_candidates = cast(
        list[dict[str, object]], run_payload["intervention_candidates"]
    )
    top_event_group = event_groups[0] if event_groups else None
    top_scored_event = scored_events[0] if scored_events else None
    top_intervention = intervention_candidates[0] if intervention_candidates else None
    event_group_headline_event_ids = [
        cast(str, group["headline_event_id"]) for group in event_groups
    ]
    return {
        "run_id": run_payload["run_id"],
        "schema_version": run_payload["schema_version"],
        "mode": run_payload["mode"],
        "raw_item_count": input_summary.get("raw_item_count", 0),
        "normalized_event_count": input_summary.get("normalized_event_count", 0),
        "dominant_field": scenario_summary.get("dominant_field", "uncategorized"),
        "risk_level": scenario_summary.get("risk_level", "low"),
        "headline": scenario_summary.get("headline", ""),
        "event_group_count": len(event_groups),
        "event_group_headline_event_ids": event_group_headline_event_ids,
        "top_event_group": top_event_group,
        "top_scored_event": top_scored_event,
        "top_intervention": top_intervention,
        "intervention_event_ids": [
            cast(str, candidate["event_id"]) for candidate in intervention_candidates
        ],
    }


def build_compare_diff(
    left: dict[str, object],
    right: dict[str, object],
) -> dict[str, object]:
    left_intervention_ids = cast(list[str], left["intervention_event_ids"])
    right_intervention_ids = cast(list[str], right["intervention_event_ids"])
    left_top_scored_event = cast(dict[str, object] | None, left["top_scored_event"])
    right_top_scored_event = cast(dict[str, object] | None, right["top_scored_event"])
    left_top_event_group = cast(dict[str, object] | None, left["top_event_group"])
    right_top_event_group = cast(dict[str, object] | None, right["top_event_group"])
    left_top_intervention = cast(dict[str, object] | None, left["top_intervention"])
    right_top_intervention = cast(dict[str, object] | None, right["top_intervention"])
    left_top_event_group_headline_event_id = (
        cast(str, left_top_event_group["headline_event_id"])
        if left_top_event_group is not None
        else None
    )
    right_top_event_group_headline_event_id = (
        cast(str, right_top_event_group["headline_event_id"])
        if right_top_event_group is not None
        else None
    )
    left_top_scored_event_id = (
        cast(str, left_top_scored_event["event_id"])
        if left_top_scored_event is not None
        else None
    )
    right_top_scored_event_id = (
        cast(str, right_top_scored_event["event_id"])
        if right_top_scored_event is not None
        else None
    )
    left_top_impact_score = get_top_score_metric(left_top_scored_event, "impact_score")
    right_top_impact_score = get_top_score_metric(
        right_top_scored_event, "impact_score"
    )
    left_top_field_attraction = get_top_score_metric(
        left_top_scored_event, "field_attraction"
    )
    right_top_field_attraction = get_top_score_metric(
        right_top_scored_event, "field_attraction"
    )
    left_top_divergence_score = get_top_score_metric(
        left_top_scored_event, "divergence_score"
    )
    right_top_divergence_score = get_top_score_metric(
        right_top_scored_event, "divergence_score"
    )
    left_top_intervention_event_id = (
        cast(str, left_top_intervention["event_id"])
        if left_top_intervention is not None
        else None
    )
    right_top_intervention_event_id = (
        cast(str, right_top_intervention["event_id"])
        if right_top_intervention is not None
        else None
    )
    left_event_group_headline_ids = cast(
        list[str], left["event_group_headline_event_ids"]
    )
    right_event_group_headline_ids = cast(
        list[str], right["event_group_headline_event_ids"]
    )
    return {
        "raw_item_count_delta": cast(int, right["raw_item_count"])
        - cast(int, left["raw_item_count"]),
        "normalized_event_count_delta": cast(int, right["normalized_event_count"])
        - cast(int, left["normalized_event_count"]),
        "event_group_count_delta": cast(int, right["event_group_count"])
        - cast(int, left["event_group_count"]),
        "dominant_field_changed": left["dominant_field"] != right["dominant_field"],
        "risk_level_changed": left["risk_level"] != right["risk_level"],
        "top_event_group_changed": left_top_event_group_headline_event_id
        != right_top_event_group_headline_event_id,
        "left_top_event_group_headline_event_id": left_top_event_group_headline_event_id,
        "right_top_event_group_headline_event_id": right_top_event_group_headline_event_id,
        "top_scored_event_changed": left_top_scored_event_id
        != right_top_scored_event_id,
        "top_scored_event_comparable": left_top_scored_event_id is not None
        and right_top_scored_event_id is not None
        and left_top_scored_event_id == right_top_scored_event_id,
        "top_intervention_changed": left_top_intervention_event_id
        != right_top_intervention_event_id,
        "left_top_scored_event_id": left_top_scored_event_id,
        "right_top_scored_event_id": right_top_scored_event_id,
        "left_top_impact_score": left_top_impact_score,
        "right_top_impact_score": right_top_impact_score,
        "top_impact_score_delta": build_score_delta(
            left_top_impact_score,
            right_top_impact_score,
        ),
        "left_top_field_attraction": left_top_field_attraction,
        "right_top_field_attraction": right_top_field_attraction,
        "top_field_attraction_delta": build_score_delta(
            left_top_field_attraction,
            right_top_field_attraction,
        ),
        "left_top_divergence_score": left_top_divergence_score,
        "right_top_divergence_score": right_top_divergence_score,
        "top_divergence_score_delta": build_score_delta(
            left_top_divergence_score,
            right_top_divergence_score,
        ),
        "left_top_intervention_event_id": left_top_intervention_event_id,
        "right_top_intervention_event_id": right_top_intervention_event_id,
        "left_only_event_group_headline_event_ids": [
            event_id
            for event_id in left_event_group_headline_ids
            if event_id not in right_event_group_headline_ids
        ],
        "right_only_event_group_headline_event_ids": [
            event_id
            for event_id in right_event_group_headline_ids
            if event_id not in left_event_group_headline_ids
        ],
        "top_event_group_evidence_diff": build_top_event_group_evidence_diff(
            left_top_event_group,
            right_top_event_group,
        ),
        "left_only_intervention_event_ids": [
            event_id
            for event_id in left_intervention_ids
            if event_id not in right_intervention_ids
        ],
        "right_only_intervention_event_ids": [
            event_id
            for event_id in right_intervention_ids
            if event_id not in left_intervention_ids
        ],
    }


def get_top_score_metric(
    top_scored_event: dict[str, object] | None,
    metric_name: str,
) -> float | None:
    if top_scored_event is None:
        return None
    value = top_scored_event.get(metric_name)
    if not isinstance(value, int | float):
        return None
    return float(value)


def build_score_delta(
    left_value: float | None, right_value: float | None
) -> float | None:
    if left_value is None or right_value is None:
        return None
    return round(right_value - left_value, 2)


def build_top_event_group_evidence_diff(
    left_top_event_group: dict[str, object] | None,
    right_top_event_group: dict[str, object] | None,
) -> dict[str, object]:
    left_member_event_ids = sorted(
        cast(list[str], left_top_event_group.get("member_event_ids", []))
        if left_top_event_group is not None
        else []
    )
    right_member_event_ids = sorted(
        cast(list[str], right_top_event_group.get("member_event_ids", []))
        if right_top_event_group is not None
        else []
    )
    left_shared_keywords = sorted(
        cast(list[str], left_top_event_group.get("shared_keywords", []))
        if left_top_event_group is not None
        else []
    )
    right_shared_keywords = sorted(
        cast(list[str], right_top_event_group.get("shared_keywords", []))
        if right_top_event_group is not None
        else []
    )
    left_shared_actors = sorted(
        cast(list[str], left_top_event_group.get("shared_actors", []))
        if left_top_event_group is not None
        else []
    )
    right_shared_actors = sorted(
        cast(list[str], right_top_event_group.get("shared_actors", []))
        if right_top_event_group is not None
        else []
    )
    left_shared_regions = sorted(
        cast(list[str], left_top_event_group.get("shared_regions", []))
        if left_top_event_group is not None
        else []
    )
    right_shared_regions = sorted(
        cast(list[str], right_top_event_group.get("shared_regions", []))
        if right_top_event_group is not None
        else []
    )
    left_chain_summary = (
        cast(str, left_top_event_group.get("chain_summary"))
        if left_top_event_group is not None
        and left_top_event_group.get("chain_summary")
        else None
    )
    right_chain_summary = (
        cast(str, right_top_event_group.get("chain_summary"))
        if right_top_event_group is not None
        and right_top_event_group.get("chain_summary")
        else None
    )
    left_evidence_links = sorted(
        [
            format_evidence_chain_link(link)
            for link in cast(
                list[dict[str, object]],
                left_top_event_group.get("evidence_chain", []),
            )
        ]
        if left_top_event_group is not None
        else []
    )
    right_evidence_links = sorted(
        [
            format_evidence_chain_link(link)
            for link in cast(
                list[dict[str, object]],
                right_top_event_group.get("evidence_chain", []),
            )
        ]
        if right_top_event_group is not None
        else []
    )
    left_headline_event_id = (
        cast(str, left_top_event_group.get("headline_event_id"))
        if left_top_event_group is not None
        and left_top_event_group.get("headline_event_id")
        else None
    )
    right_headline_event_id = (
        cast(str, right_top_event_group.get("headline_event_id"))
        if right_top_event_group is not None
        and right_top_event_group.get("headline_event_id")
        else None
    )

    return {
        "comparable": left_headline_event_id is not None
        and right_headline_event_id is not None
        and left_headline_event_id == right_headline_event_id,
        "same_headline_event_id": left_headline_event_id == right_headline_event_id,
        "member_count_delta": len(right_member_event_ids) - len(left_member_event_ids),
        "left_member_event_ids": left_member_event_ids,
        "right_member_event_ids": right_member_event_ids,
        "left_only_member_event_ids": [
            event_id
            for event_id in left_member_event_ids
            if event_id not in right_member_event_ids
        ],
        "right_only_member_event_ids": [
            event_id
            for event_id in right_member_event_ids
            if event_id not in left_member_event_ids
        ],
        "shared_keywords_added": [
            keyword
            for keyword in right_shared_keywords
            if keyword not in left_shared_keywords
        ],
        "shared_keywords_removed": [
            keyword
            for keyword in left_shared_keywords
            if keyword not in right_shared_keywords
        ],
        "shared_actors_added": [
            actor for actor in right_shared_actors if actor not in left_shared_actors
        ],
        "shared_actors_removed": [
            actor for actor in left_shared_actors if actor not in right_shared_actors
        ],
        "shared_regions_added": [
            region
            for region in right_shared_regions
            if region not in left_shared_regions
        ],
        "shared_regions_removed": [
            region
            for region in left_shared_regions
            if region not in right_shared_regions
        ],
        "evidence_chain_link_count_delta": len(right_evidence_links)
        - len(left_evidence_links),
        "left_evidence_chain_links": left_evidence_links,
        "right_evidence_chain_links": right_evidence_links,
        "evidence_chain_links_added": [
            link for link in right_evidence_links if link not in left_evidence_links
        ],
        "evidence_chain_links_removed": [
            link for link in left_evidence_links if link not in right_evidence_links
        ],
        "chain_summary_changed": left_chain_summary != right_chain_summary,
        "left_chain_summary": left_chain_summary,
        "right_chain_summary": right_chain_summary,
    }


def format_evidence_chain_link(link: dict[str, object]) -> str:
    shared_keywords = cast(list[str], link.get("shared_keywords", []))
    shared_actors = cast(list[str], link.get("shared_actors", []))
    shared_regions = cast(list[str], link.get("shared_regions", []))
    time_delta_hours = link.get("time_delta_hours")
    time_delta_text = "unknown"
    if isinstance(time_delta_hours, int | float):
        time_delta_text = str(float(time_delta_hours)).rstrip("0").rstrip(".")
    return (
        f"{link.get('from_event_id', '?')}->{link.get('to_event_id', '?')}"
        f"|keywords={','.join(sorted(shared_keywords))}"
        f"|actors={','.join(sorted(shared_actors))}"
        f"|regions={','.join(sorted(shared_regions))}"
        f"|delta_h={time_delta_text}"
    )
