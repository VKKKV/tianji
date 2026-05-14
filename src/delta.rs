use std::collections::{BTreeMap, BTreeSet};

use crate::utils::round2;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Numeric metric definition for cross-run percentage changes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NumericMetricDef {
    pub key: &'static str,
    pub label: &'static str,
    pub threshold_pct: f64,
    pub risk_sensitive: bool,
}

/// Count metric definition for cross-run absolute changes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CountMetricDef {
    pub key: &'static str,
    pub label: &'static str,
    pub threshold_abs: i64,
    pub risk_sensitive: bool,
}

/// TianJi default numeric metrics for the first delta-engine slice.
pub const TIANJI_NUMERIC_METRICS: &[NumericMetricDef] = &[
    NumericMetricDef {
        key: "top_impact_score",
        label: "Top Impact Score",
        threshold_pct: 20.0,
        risk_sensitive: true,
    },
    NumericMetricDef {
        key: "top_divergence_score",
        label: "Top Divergence Score",
        threshold_pct: 15.0,
        risk_sensitive: true,
    },
    NumericMetricDef {
        key: "top_field_attraction",
        label: "Top Field Attraction",
        threshold_pct: 25.0,
        risk_sensitive: false,
    },
    NumericMetricDef {
        key: "avg_impact_score",
        label: "Avg Impact Score",
        threshold_pct: 30.0,
        risk_sensitive: true,
    },
    NumericMetricDef {
        key: "avg_divergence_score",
        label: "Avg Divergence Score",
        threshold_pct: 20.0,
        risk_sensitive: true,
    },
];

/// TianJi default count metrics for the first delta-engine slice.
pub const TIANJI_COUNT_METRICS: &[CountMetricDef] = &[
    CountMetricDef {
        key: "scored_event_count",
        label: "Scored Events",
        threshold_abs: 3,
        risk_sensitive: true,
    },
    CountMetricDef {
        key: "intervention_candidate_count",
        label: "Intervention Candidates",
        threshold_abs: 2,
        risk_sensitive: true,
    },
    CountMetricDef {
        key: "event_group_count",
        label: "Event Groups",
        threshold_abs: 2,
        risk_sensitive: true,
    },
    CountMetricDef {
        key: "unique_actor_count",
        label: "Unique Actors",
        threshold_abs: 3,
        risk_sensitive: true,
    },
    CountMetricDef {
        key: "unique_region_count",
        label: "Unique Regions",
        threshold_abs: 2,
        risk_sensitive: false,
    },
];

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct MetricSnapshot {
    pub numerics: BTreeMap<String, f64>,
    pub counts: BTreeMap<String, i64>,
}

impl MetricSnapshot {
    pub fn from_run_payload(run: &Value) -> Self {
        let scored_events = array_at(run, "scored_events");
        let interventions = array_at(run, "intervention_candidates");
        let event_groups = run
            .get("scenario_summary")
            .and_then(|s| s.get("event_groups"))
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();

        let mut numerics = BTreeMap::new();
        if let Some(top) = scored_events.first() {
            insert_f64(&mut numerics, "top_impact_score", top, "impact_score");
            insert_f64(
                &mut numerics,
                "top_divergence_score",
                top,
                "divergence_score",
            );
            insert_f64(
                &mut numerics,
                "top_field_attraction",
                top,
                "field_attraction",
            );
        }

        let impact_values = numeric_values(&scored_events, "impact_score");
        if !impact_values.is_empty() {
            numerics.insert("avg_impact_score".to_string(), average(&impact_values));
        }
        let divergence_values = numeric_values(&scored_events, "divergence_score");
        if !divergence_values.is_empty() {
            numerics.insert(
                "avg_divergence_score".to_string(),
                average(&divergence_values),
            );
        }

        let mut actors = BTreeSet::new();
        let mut regions = BTreeSet::new();
        for event in &scored_events {
            collect_string_array(event, "actors", &mut actors);
            collect_string_array(event, "regions", &mut regions);
        }

        let mut counts = BTreeMap::new();
        counts.insert("scored_event_count".to_string(), scored_events.len() as i64);
        counts.insert(
            "intervention_candidate_count".to_string(),
            interventions.len() as i64,
        );
        counts.insert("event_group_count".to_string(), event_groups.len() as i64);
        counts.insert("unique_actor_count".to_string(), actors.len() as i64);
        counts.insert("unique_region_count".to_string(), regions.len() as i64);

        Self { numerics, counts }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeltaDirection {
    Escalated,
    Deescalated,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    Critical,
    High,
    Moderate,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct NumericDelta {
    pub key: String,
    pub label: String,
    pub from: f64,
    pub to: f64,
    pub pct_change: f64,
    pub direction: DeltaDirection,
    pub severity: Severity,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CountDelta {
    pub key: String,
    pub label: String,
    pub from: i64,
    pub to: i64,
    pub change: i64,
    pub pct_change: f64,
    pub direction: DeltaDirection,
    pub severity: Severity,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct NewSignal {
    pub key: String,
    pub label: String,
    pub reason: String,
    pub severity: Severity,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RiskDirection {
    RiskOff,
    RiskOn,
    Mixed,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SignalBreakdown {
    pub new_count: usize,
    pub escalated_count: usize,
    pub deescalated_count: usize,
    pub unchanged_count: usize,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeltaSummary {
    pub total_changes: usize,
    pub critical_changes: usize,
    pub direction: RiskDirection,
    pub signal_breakdown: SignalBreakdown,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DeltaReport {
    pub timestamp: String,
    pub previous_timestamp: Option<String>,
    pub numeric_deltas: Vec<NumericDelta>,
    pub count_deltas: Vec<CountDelta>,
    pub new_signals: Vec<NewSignal>,
    pub summary: DeltaSummary,
}

pub fn compute_delta(current_run: &Value, previous_run: Option<&Value>) -> Option<DeltaReport> {
    let previous_run = previous_run?;
    Some(compute_delta_with_metrics(
        current_run,
        previous_run,
        TIANJI_NUMERIC_METRICS,
        TIANJI_COUNT_METRICS,
    ))
}

pub fn compute_delta_with_metrics(
    current_run: &Value,
    previous_run: &Value,
    numeric_metrics: &[NumericMetricDef],
    count_metrics: &[CountMetricDef],
) -> DeltaReport {
    let current = MetricSnapshot::from_run_payload(current_run);
    let previous = MetricSnapshot::from_run_payload(previous_run);

    let mut numeric_deltas = Vec::new();
    let mut risk_up = 0usize;
    let mut risk_down = 0usize;
    let mut unchanged_count = 0usize;

    for metric in numeric_metrics {
        let Some(cur) = current.numerics.get(metric.key).copied() else {
            unchanged_count += 1;
            continue;
        };
        let Some(prev) = previous.numerics.get(metric.key).copied() else {
            unchanged_count += 1;
            continue;
        };
        let pct_change = percentage_change(prev, cur);
        if pct_change.abs() > metric.threshold_pct {
            let direction = direction_for_f64(pct_change);
            adjust_risk_counts(
                metric.risk_sensitive,
                &direction,
                &mut risk_up,
                &mut risk_down,
            );
            numeric_deltas.push(NumericDelta {
                key: metric.key.to_string(),
                label: metric.label.to_string(),
                from: round2(prev),
                to: round2(cur),
                pct_change: round2(pct_change),
                direction,
                severity: severity_for_numeric(pct_change.abs(), metric.threshold_pct),
            });
        } else {
            unchanged_count += 1;
        }
    }

    let mut count_deltas = Vec::new();
    for metric in count_metrics {
        let Some(cur) = current.counts.get(metric.key).copied() else {
            unchanged_count += 1;
            continue;
        };
        let Some(prev) = previous.counts.get(metric.key).copied() else {
            unchanged_count += 1;
            continue;
        };
        let change = cur - prev;
        if change.abs() >= metric.threshold_abs {
            let direction = direction_for_i64(change);
            adjust_risk_counts(
                metric.risk_sensitive,
                &direction,
                &mut risk_up,
                &mut risk_down,
            );
            count_deltas.push(CountDelta {
                key: metric.key.to_string(),
                label: metric.label.to_string(),
                from: prev,
                to: cur,
                change,
                pct_change: round2(percentage_change(prev as f64, cur as f64)),
                direction,
                severity: severity_for_count(change.abs(), metric.threshold_abs),
            });
        } else {
            unchanged_count += 1;
        }
    }

    let new_signals = detect_new_signals(current_run, previous_run);
    let critical_changes = numeric_deltas
        .iter()
        .filter(|d| d.severity == Severity::Critical)
        .count()
        + count_deltas
            .iter()
            .filter(|d| d.severity == Severity::Critical)
            .count()
        + new_signals
            .iter()
            .filter(|s| s.severity == Severity::Critical)
            .count();
    let escalated_count = numeric_deltas
        .iter()
        .filter(|d| d.direction == DeltaDirection::Escalated)
        .count()
        + count_deltas
            .iter()
            .filter(|d| d.direction == DeltaDirection::Escalated)
            .count();
    let deescalated_count = numeric_deltas
        .iter()
        .filter(|d| d.direction == DeltaDirection::Deescalated)
        .count()
        + count_deltas
            .iter()
            .filter(|d| d.direction == DeltaDirection::Deescalated)
            .count();
    let total_changes = numeric_deltas.len() + count_deltas.len() + new_signals.len();

    DeltaReport {
        timestamp: generated_at(current_run),
        previous_timestamp: Some(generated_at(previous_run)),
        numeric_deltas,
        count_deltas,
        new_signals: new_signals.clone(),
        summary: DeltaSummary {
            total_changes,
            critical_changes,
            direction: infer_risk_direction(risk_up, risk_down),
            signal_breakdown: SignalBreakdown {
                new_count: new_signals.len(),
                escalated_count,
                deescalated_count,
                unchanged_count,
            },
        },
    }
}

pub fn severity_for_numeric(pct_change_abs: f64, threshold: f64) -> Severity {
    let ratio = pct_change_abs / threshold;
    if ratio > 3.0 {
        Severity::Critical
    } else if ratio > 2.0 {
        Severity::High
    } else {
        Severity::Moderate
    }
}

pub fn severity_for_count(change_abs: i64, threshold: i64) -> Severity {
    let ratio = change_abs as f64 / threshold as f64;
    if ratio > 5.0 {
        Severity::Critical
    } else if ratio > 2.0 {
        Severity::High
    } else {
        Severity::Moderate
    }
}

fn detect_new_signals(current_run: &Value, previous_run: &Value) -> Vec<NewSignal> {
    let previous_event_ids = string_set_from_items(previous_run, "scored_events", "event_id");
    let mut signals = Vec::new();
    for event in array_at(current_run, "scored_events") {
        let Some(event_id) = event.get("event_id").and_then(|v| v.as_str()) else {
            continue;
        };
        if !previous_event_ids.contains(event_id) {
            signals.push(NewSignal {
                key: format!("event:{event_id}"),
                label: "New Scored Event".to_string(),
                reason: event
                    .get("title")
                    .and_then(|v| v.as_str())
                    .unwrap_or(event_id)
                    .to_string(),
                severity: Severity::Moderate,
            });
        }
    }

    let previous_intervention_keys = intervention_keys(previous_run);
    for key in intervention_keys(current_run) {
        if !previous_intervention_keys.contains(&key) {
            signals.push(NewSignal {
                key: format!("intervention:{key}"),
                label: "New Intervention Candidate".to_string(),
                reason: key,
                severity: Severity::Moderate,
            });
        }
    }

    let current_field = dominant_field(current_run);
    let previous_field = dominant_field(previous_run);
    if current_field != previous_field {
        signals.push(NewSignal {
            key: format!("dominant_field:{previous_field}->{current_field}"),
            label: "Dominant Field Changed".to_string(),
            reason: format!("dominant_field changed from {previous_field} to {current_field}"),
            severity: Severity::High,
        });
    }

    signals
}

fn array_at(run: &Value, key: &str) -> Vec<Value> {
    run.get(key)
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default()
}

fn insert_f64(
    target: &mut BTreeMap<String, f64>,
    target_key: &str,
    source: &Value,
    source_key: &str,
) {
    if let Some(value) = source.get(source_key).and_then(|v| v.as_f64()) {
        target.insert(target_key.to_string(), value);
    }
}

fn numeric_values(items: &[Value], key: &str) -> Vec<f64> {
    items
        .iter()
        .filter_map(|item| item.get(key).and_then(|v| v.as_f64()))
        .collect()
}

fn average(values: &[f64]) -> f64 {
    values.iter().sum::<f64>() / values.len() as f64
}

fn collect_string_array(value: &Value, key: &str, target: &mut BTreeSet<String>) {
    if let Some(values) = value.get(key).and_then(|v| v.as_array()) {
        for value in values {
            if let Some(text) = value.as_str() {
                target.insert(text.to_string());
            }
        }
    }
}

fn string_set_from_items(run: &Value, array_key: &str, item_key: &str) -> BTreeSet<String> {
    array_at(run, array_key)
        .iter()
        .filter_map(|item| {
            item.get(item_key)
                .and_then(|v| v.as_str())
                .map(String::from)
        })
        .collect()
}

fn intervention_keys(run: &Value) -> BTreeSet<String> {
    array_at(run, "intervention_candidates")
        .iter()
        .filter_map(|item| {
            let event_id = item.get("event_id")?.as_str()?;
            let intervention_type = item.get("intervention_type")?.as_str()?;
            Some(format!("{event_id}:{intervention_type}"))
        })
        .collect()
}

fn dominant_field(run: &Value) -> String {
    run.get("scenario_summary")
        .and_then(|s| s.get("dominant_field"))
        .and_then(|v| v.as_str())
        .unwrap_or("uncategorized")
        .to_string()
}

fn generated_at(run: &Value) -> String {
    run.get("generated_at")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string()
}

fn percentage_change(previous: f64, current: f64) -> f64 {
    if previous == 0.0 {
        if current == 0.0 {
            0.0
        } else {
            100.0
        }
    } else {
        ((current - previous) / previous.abs()) * 100.0
    }
}

fn direction_for_f64(change: f64) -> DeltaDirection {
    if change > 0.0 {
        DeltaDirection::Escalated
    } else {
        DeltaDirection::Deescalated
    }
}

fn direction_for_i64(change: i64) -> DeltaDirection {
    if change > 0 {
        DeltaDirection::Escalated
    } else {
        DeltaDirection::Deescalated
    }
}

fn adjust_risk_counts(
    risk_sensitive: bool,
    direction: &DeltaDirection,
    risk_up: &mut usize,
    risk_down: &mut usize,
) {
    if !risk_sensitive {
        return;
    }
    match direction {
        DeltaDirection::Escalated => *risk_up += 1,
        DeltaDirection::Deescalated => *risk_down += 1,
    }
}

fn infer_risk_direction(risk_up: usize, risk_down: usize) -> RiskDirection {
    if risk_up > risk_down + 1 {
        RiskDirection::RiskOff
    } else if risk_down > risk_up + 1 {
        RiskDirection::RiskOn
    } else {
        RiskDirection::Mixed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run_payload(impact: f64, divergence: f64, event_count: usize, field: &str) -> Value {
        let events: Vec<Value> = (0..event_count)
            .map(|idx| {
                serde_json::json!({
                    "event_id": format!("event-{idx}"),
                    "title": format!("Event {idx}"),
                    "actors": [format!("actor-{idx}")],
                    "regions": ["east-asia"],
                    "dominant_field": field,
                    "impact_score": impact + idx as f64,
                    "field_attraction": 4.0,
                    "divergence_score": divergence + idx as f64,
                })
            })
            .collect();
        serde_json::json!({
            "generated_at": "1970-01-01T00:00:00+00:00",
            "scenario_summary": {
                "dominant_field": field,
                "event_groups": [],
            },
            "scored_events": events,
            "intervention_candidates": [],
        })
    }

    #[test]
    fn metric_snapshot_extracts_top_average_and_counts() {
        let payload = run_payload(10.0, 5.0, 4, "conflict");
        let snapshot = MetricSnapshot::from_run_payload(&payload);
        assert_eq!(snapshot.numerics["top_impact_score"], 10.0);
        assert_eq!(snapshot.numerics["avg_impact_score"], 11.5);
        assert_eq!(snapshot.counts["scored_event_count"], 4);
        assert_eq!(snapshot.counts["unique_actor_count"], 4);
    }

    #[test]
    fn compute_delta_detects_escalation_and_new_events() {
        let previous = run_payload(10.0, 10.0, 1, "diplomacy");
        let current = run_payload(20.0, 20.0, 4, "conflict");
        let delta = compute_delta(&current, Some(&previous)).expect("delta");
        assert!(delta
            .numeric_deltas
            .iter()
            .any(|d| d.key == "top_divergence_score"));
        assert!(delta
            .count_deltas
            .iter()
            .any(|d| d.key == "scored_event_count"));
        assert!(delta
            .new_signals
            .iter()
            .any(|s| s.key == "dominant_field:diplomacy->conflict"));
        assert!(delta.summary.total_changes >= 3);
    }

    #[test]
    fn compute_delta_without_previous_returns_none() {
        let current = run_payload(10.0, 10.0, 1, "conflict");
        assert!(compute_delta(&current, None).is_none());
    }
}
