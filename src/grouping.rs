use std::collections::BTreeMap;
use std::sync::LazyLock;

use crate::models::{EventChainLink, EventGroupSummary, ScoredEvent};
use crate::utils::{days_since_epoch, round2};

const MIN_SHARED_KEYWORDS: usize = 2;
const MAX_GROUP_TIME_DELTA_SECS: i64 = 24 * 3600;
static ISO_TIME_RE: LazyLock<regex::Regex> = LazyLock::new(|| {
    regex::Regex::new(r"^(\d{4})-(\d{2})-(\d{2})T(\d{2}):(\d{2}):(\d{2})")
        .expect("valid ISO time regex")
});
static RFC2822_TIME_RE: LazyLock<regex::Regex> = LazyLock::new(|| {
    regex::Regex::new(r"^(?:\w{3}, )?(\d{1,2}) (\w{3}) (\d{4}) (\d{2}):(\d{2}):(\d{2})")
        .expect("valid RFC2822 time regex")
});

pub fn group_events(scored_events: &[ScoredEvent]) -> Vec<EventGroupSummary> {
    let mut ordered_events: Vec<&ScoredEvent> = scored_events.iter().collect();
    ordered_events.sort_by(|a, b| {
        b.divergence_score
            .partial_cmp(&a.divergence_score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.event_id.cmp(&b.event_id))
    });

    let mut groups: Vec<Vec<&ScoredEvent>> = Vec::new();
    let mut parent_event_ids_by_group: Vec<BTreeMap<String, Option<String>>> = Vec::new();

    for event in &ordered_events {
        if let Some((best_group_index, parent_event_id)) = select_best_group_match(event, &groups) {
            groups[best_group_index].push(event);
            parent_event_ids_by_group[best_group_index]
                .insert(event.event_id.clone(), Some(parent_event_id));
        } else {
            let mut parent_map = BTreeMap::new();
            parent_map.insert(event.event_id.clone(), None);
            groups.push(vec![event]);
            parent_event_ids_by_group.push(parent_map);
        }
    }

    let summaries: Vec<EventGroupSummary> = groups
        .iter()
        .enumerate()
        .filter(|(_, group)| group.len() > 1)
        .map(|(index, group)| summarize_group(group, &parent_event_ids_by_group[index]))
        .collect();

    let mut sorted_summaries = summaries;
    sorted_summaries.sort_by(|a, b| {
        let score_a = (-a.group_score * 100.0).round() as i64;
        let score_b = (-b.group_score * 100.0).round() as i64;
        score_a
            .cmp(&score_b)
            .then_with(|| a.headline_event_id.cmp(&b.headline_event_id))
    });
    sorted_summaries
}

fn select_best_group_match(
    event: &ScoredEvent,
    groups: &[Vec<&ScoredEvent>],
) -> Option<(usize, String)> {
    let mut best_index: Option<usize> = None;
    let mut best_parent_event_id: Option<String> = None;
    let mut best_score: Option<(usize, f64)> = None;

    for (index, group) in groups.iter().enumerate() {
        if let Some((signal_count, time_delta, parent_id)) = best_group_link(event, group) {
            let score = (signal_count, -time_delta);
            if best_score.as_ref().is_none_or(|current| score > *current) {
                best_score = Some(score);
                best_index = Some(index);
                best_parent_event_id = Some(parent_id);
            }
        }
    }

    match (best_index, best_parent_event_id) {
        (Some(idx), Some(parent)) => Some((idx, parent)),
        _ => None,
    }
}

fn best_group_link(event: &ScoredEvent, group: &[&ScoredEvent]) -> Option<(usize, f64, String)> {
    let mut best: Option<(usize, f64, String)> = None;

    for member in group {
        if let Some((signal_count, time_delta)) = link_score_between_events(event, member) {
            let score = (signal_count, -time_delta, member.event_id.clone());
            if best.as_ref().is_none_or(|current| {
                score.0 > current.0 || (score.0 == current.0 && score.1 > current.1)
            }) {
                best = Some(score);
            }
        }
    }

    best.map(|(signal_count, neg_time_delta, event_id)| (signal_count, -neg_time_delta, event_id))
}

fn link_score_between_events(left: &ScoredEvent, right: &ScoredEvent) -> Option<(usize, f64)> {
    if left.dominant_field != right.dominant_field {
        return None;
    }
    if !is_within_group_time_window(left, right) {
        return None;
    }

    let shared_actors = intersection(&left.actors, &right.actors);
    let shared_regions = intersection(&left.regions, &right.regions);
    if shared_actors.is_empty() && shared_regions.is_empty() {
        return None;
    }

    let shared_keywords = intersection(&left.keywords, &right.keywords);
    if shared_keywords.len() < MIN_SHARED_KEYWORDS {
        return None;
    }

    let time_delta_hours = compute_time_delta_hours(&left.published_at, &right.published_at);
    let time_delta = time_delta_hours.unwrap_or(10_000.0);

    Some((
        shared_keywords.len() + shared_actors.len() + shared_regions.len(),
        time_delta,
    ))
}

fn summarize_group(
    group: &[&ScoredEvent],
    parent_event_ids: &BTreeMap<String, Option<String>>,
) -> EventGroupSummary {
    let mut ordered_group: Vec<&ScoredEvent> = group.to_vec();
    ordered_group.sort_by(|a, b| {
        b.divergence_score
            .partial_cmp(&a.divergence_score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.event_id.cmp(&b.event_id))
    });

    let causal_ordered_group = sort_group_for_causal_chain(group, parent_event_ids);
    let anchor = ordered_group[0];

    let shared_keywords = shared_values(ordered_group.iter().map(|e| &e.keywords));
    let shared_actors = shared_values(ordered_group.iter().map(|e| &e.actors));
    let shared_regions = shared_values(ordered_group.iter().map(|e| &e.regions));

    let evidence_chain = build_evidence_chain(&causal_ordered_group, parent_event_ids);

    let group_score = round2(ordered_group.iter().map(|e| e.divergence_score).sum());

    let causal_span_hours = compute_group_causal_span_hours(&causal_ordered_group);

    let chain_summary = build_chain_summary(
        anchor,
        ordered_group.len(),
        &shared_keywords,
        &shared_actors,
        &shared_regions,
        &evidence_chain,
    );

    let causal_summary = build_causal_summary(&causal_ordered_group, &evidence_chain);

    EventGroupSummary {
        group_id: format!("group:{}", anchor.event_id),
        headline_event_id: anchor.event_id.clone(),
        headline_title: anchor.title.clone(),
        member_event_ids: ordered_group.iter().map(|e| e.event_id.clone()).collect(),
        member_count: ordered_group.len(),
        dominant_field: anchor.dominant_field.clone(),
        shared_keywords,
        shared_actors,
        shared_regions,
        group_score,
        causal_ordered_event_ids: causal_ordered_group
            .iter()
            .map(|e| e.event_id.clone())
            .collect(),
        causal_span_hours,
        evidence_chain,
        chain_summary,
        causal_summary,
    }
}

fn build_evidence_chain(
    ordered_group: &[&ScoredEvent],
    parent_event_ids: &BTreeMap<String, Option<String>>,
) -> Vec<EventChainLink> {
    let events_by_id: BTreeMap<&str, &ScoredEvent> = ordered_group
        .iter()
        .map(|e| (e.event_id.as_str(), *e))
        .collect();

    let mut chain = Vec::new();
    for current_event in ordered_group {
        if let Some(Some(parent_event_id)) = parent_event_ids.get(&current_event.event_id) {
            if let Some(&previous_event) = events_by_id.get(parent_event_id.as_str()) {
                chain.push(EventChainLink {
                    from_event_id: previous_event.event_id.clone(),
                    to_event_id: current_event.event_id.clone(),
                    shared_keywords: intersection(
                        &previous_event.keywords,
                        &current_event.keywords,
                    ),
                    shared_actors: intersection(&previous_event.actors, &current_event.actors),
                    shared_regions: intersection(&previous_event.regions, &current_event.regions),
                    relationship: infer_group_relationship(previous_event),
                    shared_signal_count: compute_shared_signal_count(previous_event, current_event),
                    time_delta_hours: compute_time_delta_hours(
                        &previous_event.published_at,
                        &current_event.published_at,
                    ),
                });
            }
        }
    }
    chain
}

fn build_chain_summary(
    anchor: &ScoredEvent,
    member_count: usize,
    shared_keywords: &[String],
    shared_actors: &[String],
    shared_regions: &[String],
    evidence_chain: &[EventChainLink],
) -> String {
    let mut evidence_parts: Vec<String> = Vec::new();
    if !shared_actors.is_empty() {
        evidence_parts.push(format!("actors {}", shared_actors.join(", ")));
    }
    if !shared_regions.is_empty() {
        evidence_parts.push(format!("regions {}", shared_regions.join(", ")));
    }
    if !shared_keywords.is_empty() {
        evidence_parts.push(format!("keywords {}", shared_keywords.join(", ")));
    }

    let evidence_text = if evidence_parts.is_empty() {
        "repeated field-aligned evidence".to_string()
    } else {
        evidence_parts.join(", ")
    };

    let chain_link_count = evidence_chain.len();
    let chain_text = if chain_link_count > 0 {
        let plural = if chain_link_count != 1 { "s" } else { "" };
        format!(" through {chain_link_count} corroborating link{plural}")
    } else {
        String::new()
    };

    format!(
        "{member_count} related {} events reinforce '{}' via {evidence_text}{chain_text}.",
        anchor.dominant_field, anchor.title
    )
}

fn build_causal_summary(
    causal_ordered_group: &[&ScoredEvent],
    evidence_chain: &[EventChainLink],
) -> String {
    if causal_ordered_group.is_empty() {
        return "No causal cluster available.".to_string();
    }
    let first_event = causal_ordered_group[0];
    let last_event = causal_ordered_group[causal_ordered_group.len() - 1];
    let span_hours = compute_group_causal_span_hours(causal_ordered_group);
    let relationship = if !evidence_chain.is_empty() {
        evidence_chain[0].relationship.clone()
    } else {
        infer_group_relationship(first_event)
    };
    let span_text = match span_hours {
        Some(h) => format!(" over {h}h"),
        None => String::new(),
    };

    if first_event.event_id == last_event.event_id {
        return format!(
            "Single-event {relationship} cluster anchored on '{}'.",
            first_event.title
        );
    }
    if span_hours.is_none() {
        return format!(
            "{relationship} cluster linking '{}' to '{}' across {} events.",
            first_event.title,
            last_event.title,
            causal_ordered_group.len()
        );
    }
    format!(
        "{relationship} cluster from '{}' to '{}' across {} events{span_text}.",
        first_event.title,
        last_event.title,
        causal_ordered_group.len()
    )
}

fn sort_group_for_causal_chain<'a>(
    group: &[&'a ScoredEvent],
    parent_event_ids: &BTreeMap<String, Option<String>>,
) -> Vec<&'a ScoredEvent> {
    let mut ordered_events: Vec<&ScoredEvent> = group.to_vec();
    ordered_events.sort_by(|a, b| {
        b.divergence_score
            .partial_cmp(&a.divergence_score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.event_id.cmp(&b.event_id))
    });

    // Find root: event with no parent
    let root = ordered_events
        .iter()
        .find(|event| {
            parent_event_ids
                .get(&event.event_id)
                .is_none_or(|p| p.is_none())
        })
        .copied()
        .unwrap_or(ordered_events[0]);

    // Build children_by_parent_id
    let mut children_by_parent_id: BTreeMap<String, Vec<&ScoredEvent>> = BTreeMap::new();
    for event in &ordered_events {
        if let Some(Some(parent_id)) = parent_event_ids.get(&event.event_id) {
            children_by_parent_id
                .entry(parent_id.clone())
                .or_default()
                .push(event);
        }
    }

    // Sort children by (time, -divergence, event_id)
    for children in children_by_parent_id.values_mut() {
        children.sort_by(|a, b| {
            let time_a = parse_event_time_rfc(&a.published_at);
            let time_b = parse_event_time_rfc(&b.published_at);
            match (time_a, time_b) {
                (Some(ta), Some(tb)) => ta.cmp(&tb),
                (Some(_), None) => std::cmp::Ordering::Less,
                (None, Some(_)) => std::cmp::Ordering::Greater,
                (None, None) => std::cmp::Ordering::Equal,
            }
            .then_with(|| {
                b.divergence_score
                    .partial_cmp(&a.divergence_score)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .then_with(|| a.event_id.cmp(&b.event_id))
        });
    }

    let mut ordered_chain: Vec<&ScoredEvent> = Vec::new();
    visit_causal_chain(root, &children_by_parent_id, &mut ordered_chain);
    ordered_chain
}

fn visit_causal_chain<'a>(
    event: &'a ScoredEvent,
    children_by_parent_id: &BTreeMap<String, Vec<&'a ScoredEvent>>,
    chain: &mut Vec<&'a ScoredEvent>,
) {
    chain.push(event);
    if let Some(children) = children_by_parent_id.get(&event.event_id) {
        for child in children {
            visit_causal_chain(child, children_by_parent_id, chain);
        }
    }
}

fn infer_group_relationship(event: &ScoredEvent) -> String {
    match event.dominant_field.as_str() {
        "conflict" => "escalation".to_string(),
        "diplomacy" => "negotiation".to_string(),
        "economy" => "pressure".to_string(),
        "technology" => "capability-race".to_string(),
        _ => "reinforcing".to_string(),
    }
}

fn compute_shared_signal_count(left: &ScoredEvent, right: &ScoredEvent) -> usize {
    intersection(&left.keywords, &right.keywords).len()
        + intersection(&left.actors, &right.actors).len()
        + intersection(&left.regions, &right.regions).len()
}

fn shared_values<'a>(value_lists: impl Iterator<Item = &'a Vec<String>>) -> Vec<String> {
    let mut iter = value_lists;
    let shared = match iter.next() {
        Some(first) => first
            .iter()
            .cloned()
            .collect::<std::collections::HashSet<String>>(),
        None => return Vec::new(),
    };

    let mut result = shared;
    for values in iter {
        let set: std::collections::HashSet<String> = values.iter().cloned().collect();
        result = result.intersection(&set).cloned().collect();
    }
    let mut sorted: Vec<String> = result.into_iter().collect();
    sorted.sort();
    sorted
}

fn intersection(left: &[String], right: &[String]) -> Vec<String> {
    let right_set: std::collections::HashSet<&str> = right.iter().map(|s| s.as_str()).collect();
    let mut result: Vec<String> = left
        .iter()
        .filter(|s| right_set.contains(s.as_str()))
        .cloned()
        .collect();
    result.sort();
    result.dedup();
    result
}

fn compute_time_delta_hours(
    left_published_at: &Option<String>,
    right_published_at: &Option<String>,
) -> Option<f64> {
    let left_time = parse_event_time_rfc(left_published_at)?;
    let right_time = parse_event_time_rfc(right_published_at)?;
    let delta = if right_time >= left_time {
        right_time - left_time
    } else {
        left_time - right_time
    };
    Some(round2(delta as f64 / 3600.0))
}

fn compute_group_causal_span_hours(group: &[&ScoredEvent]) -> Option<f64> {
    if group.len() < 2 {
        return Some(0.0);
    }
    let times: Vec<i64> = group
        .iter()
        .filter_map(|e| parse_event_time_rfc(&e.published_at))
        .collect();
    if times.len() < 2 {
        return None;
    }
    let (Some(min_time), Some(max_time)) = (times.iter().min(), times.iter().max()) else {
        return None;
    };
    Some(round2((*max_time - *min_time) as f64 / 3600.0))
}

fn is_within_group_time_window(event: &ScoredEvent, anchor: &ScoredEvent) -> bool {
    let event_time = parse_event_time_rfc(&event.published_at);
    let anchor_time = parse_event_time_rfc(&anchor.published_at);
    match (event_time, anchor_time) {
        (Some(et), Some(at)) => (et - at).abs() <= MAX_GROUP_TIME_DELTA_SECS,
        _ => true,
    }
}

/// Parse event time from various formats and return unix timestamp in seconds.
fn parse_event_time_rfc(value: &Option<String>) -> Option<i64> {
    let value = value.as_ref()?;
    // Try ISO format first
    if let Ok(ts) = parse_iso_time(value) {
        return Some(ts);
    }
    // Try RFC 2822 format (e.g. "Sun, 22 Mar 2026 07:00:00 GMT")
    parse_rfc2822_time(value)
}

fn parse_iso_time(value: &str) -> Result<i64, ()> {
    // Handle "2026-03-22T07:00:00Z" and similar
    let value = value.replace('Z', "+00:00");
    // Simple parser for common ISO formats
    // We only need to handle the formats in our fixtures
    if let Some(caps) = ISO_TIME_RE.captures(&value) {
        let year: i64 = caps[1].parse().map_err(|_| ())?;
        let month: i64 = caps[2].parse().map_err(|_| ())?;
        let day: i64 = caps[3].parse().map_err(|_| ())?;
        let hour: i64 = caps[4].parse().map_err(|_| ())?;
        let minute: i64 = caps[5].parse().map_err(|_| ())?;
        let second: i64 = caps[6].parse().map_err(|_| ())?;
        // Approximate unix timestamp (sufficient for hour-level time deltas)
        let days = days_since_epoch(year, month, day);
        Ok(days * 86400 + hour * 3600 + minute * 60 + second)
    } else {
        Err(())
    }
}

fn parse_rfc2822_time(value: &str) -> Option<i64> {
    // Parse "Sun, 22 Mar 2026 07:00:00 GMT" format
    let caps = RFC2822_TIME_RE.captures(value)?;
    let day: i64 = caps[1].parse().ok()?;
    let month_str = &caps[2];
    let year: i64 = caps[3].parse().ok()?;
    let hour: i64 = caps[4].parse().ok()?;
    let minute: i64 = caps[5].parse().ok()?;
    let second: i64 = caps[6].parse().ok()?;

    let month = month_from_str(month_str)?;
    let days = days_since_epoch(year, month, day);
    Some(days * 86400 + hour * 3600 + minute * 60 + second)
}

fn month_from_str(s: &str) -> Option<i64> {
    match s {
        "Jan" => Some(1),
        "Feb" => Some(2),
        "Mar" => Some(3),
        "Apr" => Some(4),
        "May" => Some(5),
        "Jun" => Some(6),
        "Jul" => Some(7),
        "Aug" => Some(8),
        "Sep" => Some(9),
        "Oct" => Some(10),
        "Nov" => Some(11),
        "Dec" => Some(12),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_groups_for_single_events() {
        // Events with different dominant fields should not group
        let events = vec![
            ScoredEvent {
                event_id: "e1".to_string(),
                title: "Test 1".to_string(),
                source: "test".to_string(),
                link: "http://test/1".to_string(),
                published_at: None,
                actors: vec!["usa".to_string()],
                regions: vec!["east-asia".to_string()],
                keywords: vec!["chip".to_string(), "ai".to_string(), "cyber".to_string()],
                dominant_field: "technology".to_string(),
                impact_score: 10.0,
                field_attraction: 5.0,
                divergence_score: 10.0,
                rationale: vec![],
            },
            ScoredEvent {
                event_id: "e2".to_string(),
                title: "Test 2".to_string(),
                source: "test".to_string(),
                link: "http://test/2".to_string(),
                published_at: None,
                actors: vec!["iran".to_string()],
                regions: vec!["middle-east".to_string()],
                keywords: vec![
                    "missile".to_string(),
                    "talks".to_string(),
                    "negotiation".to_string(),
                ],
                dominant_field: "diplomacy".to_string(),
                impact_score: 8.0,
                field_attraction: 4.0,
                divergence_score: 8.0,
                rationale: vec![],
            },
        ];
        let groups = group_events(&events);
        assert!(groups.is_empty());
    }

    #[test]
    fn parse_rfc2822_time_works() {
        let ts = parse_rfc2822_time("Sun, 22 Mar 2026 07:00:00 GMT");
        assert!(ts.is_some());
        let ts2 = parse_rfc2822_time("Sun, 22 Mar 2026 08:00:00 GMT");
        assert!(ts2.is_some());
        // Difference should be 3600 seconds (1 hour)
        assert_eq!((ts2.unwrap() - ts.unwrap()).abs(), 3600);
    }
}
