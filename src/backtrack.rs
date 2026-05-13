use crate::models::{EventGroupSummary, InterventionCandidate, ScoredEvent};
use std::collections::BTreeMap;

const FIELD_INTERVENTION_TYPES: &[(&str, &str)] = &[
    ("conflict", "de-escalation"),
    ("diplomacy", "negotiation"),
    ("economy", "economic-pressure"),
    ("technology", "capability-control"),
];

const STRONG_GROUP_INTERVENTION_TYPES: &[(&str, &str)] = &[
    ("conflict", "escalation-override"),
    ("diplomacy", "treaty-invalidation"),
    ("economy", "market-freeze"),
    ("technology", "capability-freeze"),
];

const WEAK_GROUP_INTERVENTION_TYPES: &[(&str, &str)] = &[
    ("conflict", "escalation-containment"),
    ("diplomacy", "channel-stabilization"),
    ("economy", "market-stabilization"),
    ("technology", "capability-containment"),
];

const MIN_STRONG_GROUP_SHARED_SIGNAL_COUNT: usize = 5;
const FAST_GROUP_CAUSAL_SPAN_HOURS: f64 = 2.0;

pub fn backtrack_candidates(
    scored_events: &[ScoredEvent],
    limit: usize,
    event_groups: Option<&[EventGroupSummary]>,
) -> Vec<InterventionCandidate> {
    let event_group_by_headline_id: BTreeMap<&str, &EventGroupSummary> = event_groups
        .map(|groups| {
            groups
                .iter()
                .map(|g| (g.headline_event_id.as_str(), g))
                .collect()
        })
        .unwrap_or_default();

    let selected_events = select_backtrack_events(scored_events, limit, event_groups);
    let mut candidates = Vec::new();

    for (index, event) in selected_events.iter().enumerate() {
        let event_group = event_group_by_headline_id
            .get(event.event_id.as_str())
            .copied();
        let target = select_intervention_target(event, event_group);
        let intervention_type = infer_intervention_type(event, event_group);
        let expected_effect = infer_expected_effect(event, event_group);
        let reason = build_reason(event, event_group);

        candidates.push(InterventionCandidate {
            priority: index + 1,
            event_id: event.event_id.clone(),
            target,
            intervention_type,
            reason,
            expected_effect,
        });
    }

    candidates
}

fn select_backtrack_events<'a>(
    scored_events: &'a [ScoredEvent],
    limit: usize,
    event_groups: Option<&[EventGroupSummary]>,
) -> Vec<&'a ScoredEvent> {
    let event_groups = match event_groups {
        Some(groups) if !groups.is_empty() => groups,
        _ => return scored_events.iter().take(limit).collect(),
    };

    let events_by_id: BTreeMap<&str, &ScoredEvent> = scored_events
        .iter()
        .map(|e| (e.event_id.as_str(), e))
        .collect();

    let mut selected: Vec<&ScoredEvent> = Vec::new();
    let mut seen_event_ids: std::collections::HashSet<&str> = std::collections::HashSet::new();

    for group in event_groups {
        let headline_event_id = &group.headline_event_id;
        if let Some(&headline_event) = events_by_id.get(headline_event_id.as_str()) {
            selected.push(headline_event);
            for member_id in &group.member_event_ids {
                seen_event_ids.insert(member_id.as_str());
            }
            if selected.len() >= limit {
                return selected.into_iter().take(limit).collect();
            }
        }
    }

    for event in scored_events {
        if seen_event_ids.contains(event.event_id.as_str()) {
            continue;
        }
        selected.push(event);
        if selected.len() >= limit {
            break;
        }
    }

    selected.into_iter().take(limit).collect()
}

fn select_intervention_target(
    event: &ScoredEvent,
    event_group: Option<&EventGroupSummary>,
) -> String {
    if let Some(group) = event_group {
        let headline_role_text = infer_group_headline_role_text(group);
        if headline_role_text == " headline role=chain endpoint;"
            || headline_role_text == " headline role=chain pivot;"
        {
            if !event.actors.is_empty() {
                return event.actors[0].clone();
            }
            if !event.regions.is_empty() {
                return event.regions[0].clone();
            }
        }
        if !group.shared_actors.is_empty() {
            return group.shared_actors[0].clone();
        }
        if !group.shared_regions.is_empty() {
            return group.shared_regions[0].clone();
        }
    }
    if !event.actors.is_empty() {
        return event.actors[0].clone();
    }
    if !event.regions.is_empty() {
        return event.regions[0].clone();
    }
    event.source.clone()
}

fn infer_intervention_type(event: &ScoredEvent, event_group: Option<&EventGroupSummary>) -> String {
    if let Some(group) = event_group {
        if let Some(group_type) = infer_group_intervention_type(group) {
            return group_type;
        }
    }
    infer_field_intervention_type(&event.dominant_field)
}

fn infer_group_intervention_type(event_group: &EventGroupSummary) -> Option<String> {
    let member_count = event_group.member_count;
    let link_count = event_group.evidence_chain.len();
    if member_count >= 3
        && link_count >= 2
        && event_group
            .evidence_chain
            .iter()
            .all(|link| link.shared_signal_count >= MIN_STRONG_GROUP_SHARED_SIGNAL_COUNT)
    {
        return Some(
            STRONG_GROUP_INTERVENTION_TYPES
                .iter()
                .find(|(field, _)| *field == event_group.dominant_field)
                .map(|(_, v)| v.to_string())
                .unwrap_or_else(|| "pattern-disruption".to_string()),
        );
    }
    if member_count >= 2 && link_count >= 1 {
        return Some(
            WEAK_GROUP_INTERVENTION_TYPES
                .iter()
                .find(|(field, _)| *field == event_group.dominant_field)
                .map(|(_, v)| v.to_string())
                .unwrap_or_else(|| "pattern-monitoring".to_string()),
        );
    }
    None
}

fn infer_field_intervention_type(dominant_field: &str) -> String {
    FIELD_INTERVENTION_TYPES
        .iter()
        .find(|(field, _)| *field == dominant_field)
        .map(|(_, v)| v.to_string())
        .unwrap_or_else(|| "information-gathering".to_string())
}

fn infer_expected_effect(event: &ScoredEvent, event_group: Option<&EventGroupSummary>) -> String {
    if let Some(group) = event_group {
        return infer_group_expected_effect(event, group);
    }
    match event.dominant_field.as_str() {
        "conflict" => {
            "Reduce near-term escalation incentives around the triggering event.".to_string()
        }
        "diplomacy" => "Shift the branch toward a negotiated or paused outcome.".to_string(),
        "economy" => {
            "Change economic signaling before it compounds into a wider crisis.".to_string()
        }
        "technology" => {
            "Constrain a fast-moving capability race before spillover grows.".to_string()
        }
        _ => "Collect better evidence before attempting stronger intervention.".to_string(),
    }
}

fn infer_group_expected_effect(event: &ScoredEvent, event_group: &EventGroupSummary) -> String {
    let member_count = event_group.member_count;
    let link_count = event_group.evidence_chain.len();
    let chain_type = if link_count >= 2 {
        "reinforcing chain"
    } else {
        "linked cluster"
    };
    let relationship_phrase = infer_group_effect_relationship_phrase(event_group);
    let role_phrase = infer_group_effect_role_phrase(event_group);
    let urgency_prefix = infer_group_effect_urgency_prefix(event_group, link_count);

    let (conflict_action, diplomacy_action, economy_action, generic_action) =
        if urgency_prefix.is_empty() {
            ("Disrupt", "Stabilize", "Interrupt", "Break")
        } else {
            ("disrupt", "stabilize", "interrupt", "break")
        };

    match event.dominant_field.as_str() {
        "conflict" => format!(
            "{urgency_prefix}{conflict_action} the {chain_type}{relationship_phrase}{role_phrase} before escalation compounds across {member_count} related events."
        ),
        "diplomacy" => format!(
            "{urgency_prefix}{diplomacy_action} the {chain_type}{relationship_phrase}{role_phrase} so {member_count} related diplomatic moves do not harden into a wider standoff."
        ),
        "economy" => format!(
            "{urgency_prefix}{economy_action} the {chain_type}{relationship_phrase}{role_phrase} before {member_count} linked economic signals compound into a broader shock."
        ),
        "technology" => format!(
            "{urgency_prefix}{conflict_action} the {chain_type}{relationship_phrase}{role_phrase} before {member_count} related capability moves harden into a broader race."
        ),
        _ => format!(
            "{urgency_prefix}{generic_action} the {chain_type}{relationship_phrase}{role_phrase} and collect better evidence before {member_count} related events reinforce the branch further."
        ),
    }
}

fn infer_group_effect_relationship_phrase(event_group: &EventGroupSummary) -> String {
    let dominant = infer_group_dominant_relationship(event_group);
    if dominant == "reinforcing" {
        String::new()
    } else {
        format!(" in the {dominant} pattern")
    }
}

fn infer_group_effect_role_phrase(event_group: &EventGroupSummary) -> String {
    let role_text = infer_group_headline_role_text(event_group);
    match role_text.as_str() {
        " headline role=chain origin;" => " at the chain origin".to_string(),
        " headline role=chain endpoint;" => " at the chain endpoint".to_string(),
        " headline role=chain pivot;" => " at the chain pivot".to_string(),
        _ => String::new(),
    }
}

fn infer_group_effect_urgency_prefix(event_group: &EventGroupSummary, link_count: usize) -> String {
    match event_group.causal_span_hours {
        None => String::new(),
        Some(span) if span <= FAST_GROUP_CAUSAL_SPAN_HOURS => {
            if link_count >= 2 {
                "Urgently ".to_string()
            } else {
                "Quickly ".to_string()
            }
        }
        _ => String::new(),
    }
}

fn infer_group_corroboration_text(event_group: &EventGroupSummary) -> String {
    if event_group
        .evidence_chain
        .iter()
        .all(|link| link.shared_signal_count >= MIN_STRONG_GROUP_SHARED_SIGNAL_COUNT)
    {
        " high corroboration across causal links;".to_string()
    } else {
        " moderate corroboration across causal links;".to_string()
    }
}

fn infer_group_dominant_relationship(event_group: &EventGroupSummary) -> String {
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    for link in &event_group.evidence_chain {
        *counts.entry(link.relationship.clone()).or_insert(0) += 1;
    }
    counts
        .iter()
        .min_by_key(|(name, count)| (std::cmp::Reverse(*count), *name))
        .map(|(name, _)| name.clone())
        .unwrap_or_else(|| "reinforcing".to_string())
}

fn infer_group_relationship_text(event_group: &EventGroupSummary) -> String {
    let dominant = infer_group_dominant_relationship(event_group);
    format!(" dominant relationship={dominant};")
}

fn infer_group_signal_support_text(event_group: &EventGroupSummary) -> String {
    let signal_counts: Vec<usize> = event_group
        .evidence_chain
        .iter()
        .map(|link| link.shared_signal_count)
        .collect();
    if signal_counts.is_empty() {
        return String::new();
    }
    let min_count = *signal_counts.iter().min().unwrap();
    let max_count = *signal_counts.iter().max().unwrap();
    if min_count == max_count {
        format!(" signal support={min_count};")
    } else {
        format!(" signal support range={min_count}-{max_count};")
    }
}

fn infer_group_link_tempo_text(event_group: &EventGroupSummary) -> String {
    let link_deltas: Vec<f64> = event_group
        .evidence_chain
        .iter()
        .filter_map(|link| link.time_delta_hours)
        .collect();
    if link_deltas.is_empty() {
        return String::new();
    }
    let min_delta = link_deltas.iter().cloned().fold(f64::INFINITY, f64::min);
    let max_delta = link_deltas.iter().cloned().fold(0.0_f64, f64::max);
    if (min_delta - max_delta).abs() < f64::EPSILON {
        format!(" link tempo={min_delta}h;")
    } else {
        format!(" link tempo range={min_delta}-{max_delta}h;")
    }
}

fn infer_group_headline_role_text(event_group: &EventGroupSummary) -> String {
    let causal_ids = &event_group.causal_ordered_event_ids;
    let headline_id = &event_group.headline_event_id;
    if causal_ids.is_empty() || causal_ids.len() == 1 {
        return " headline role=standalone;".to_string();
    }
    if headline_id == &causal_ids[0] {
        return " headline role=chain origin;".to_string();
    }
    if headline_id == &causal_ids[causal_ids.len() - 1] {
        return " headline role=chain endpoint;".to_string();
    }
    " headline role=chain pivot;".to_string()
}

fn build_reason(event: &ScoredEvent, event_group: Option<&EventGroupSummary>) -> String {
    let actor_text = if !event.actors.is_empty() {
        event.actors.join(", ")
    } else {
        event.source.clone()
    };
    let region_text = if !event.regions.is_empty() {
        event.regions.join(", ")
    } else {
        "global".to_string()
    };
    let base_reason = format!(
        "Backtracked from event '{}' because its divergence score is {} with field={}, actors={}, regions={}.",
        event.title, event.divergence_score, event.dominant_field, actor_text, region_text
    );

    match event_group {
        None => base_reason,
        Some(group) => {
            let shared_actor_text = if !group.shared_actors.is_empty() {
                format!(" shared actors={};", group.shared_actors.join(", "))
            } else {
                String::new()
            };
            let shared_region_text = if !group.shared_regions.is_empty() {
                format!(" shared regions={};", group.shared_regions.join(", "))
            } else {
                String::new()
            };
            let span_text = match group.causal_span_hours {
                Some(h) => format!(" over {h}h"),
                None => String::new(),
            };
            let corroboration_text = infer_group_corroboration_text(group);
            let relationship_text = infer_group_relationship_text(group);
            let signal_support_text = infer_group_signal_support_text(group);
            let link_tempo_text = infer_group_link_tempo_text(group);
            let headline_role_text = infer_group_headline_role_text(group);

            format!(
                "{base_reason} Grouped context: {}-event {} cluster with {} causal link(s){span_text};{corroboration_text}{relationship_text}{signal_support_text}{link_tempo_text}{headline_role_text}{shared_actor_text}{shared_region_text} Evidence chain: {} Causal cluster: {}",
                group.member_count,
                group.dominant_field,
                group.evidence_chain.len(),
                group.chain_summary,
                group.causal_summary
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backtrack_without_groups_uses_field_intervention_type() {
        let events = vec![ScoredEvent {
            event_id: "e1".to_string(),
            title: "Test conflict event".to_string(),
            source: "test".to_string(),
            link: "http://test".to_string(),
            published_at: None,
            actors: vec!["nato".to_string()],
            regions: vec!["ukraine".to_string()],
            keywords: vec!["troop".to_string(), "strike".to_string()],
            dominant_field: "conflict".to_string(),
            impact_score: 10.0,
            field_attraction: 5.0,
            divergence_score: 10.0,
            rationale: vec![],
        }];
        let candidates = backtrack_candidates(&events, 5, None);
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].intervention_type, "de-escalation");
        assert_eq!(candidates[0].target, "nato");
    }

    #[test]
    fn backtrack_technology_event_uses_capability_control() {
        let events = vec![ScoredEvent {
            event_id: "e1".to_string(),
            title: "Tech event".to_string(),
            source: "test".to_string(),
            link: "http://test".to_string(),
            published_at: None,
            actors: vec!["usa".to_string(), "china".to_string()],
            regions: vec!["east-asia".to_string()],
            keywords: vec!["chip".to_string(), "ai".to_string()],
            dominant_field: "technology".to_string(),
            impact_score: 15.0,
            field_attraction: 7.0,
            divergence_score: 20.0,
            rationale: vec![],
        }];
        let candidates = backtrack_candidates(&events, 5, None);
        assert_eq!(candidates[0].intervention_type, "capability-control");
        assert_eq!(candidates[0].target, "usa");
    }
}
