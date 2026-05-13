use std::collections::BTreeMap;

use serde::Serialize;
use serde_json::Value;

#[derive(Debug, Serialize)]
pub struct RunArtifact {
    pub schema_version: String,
    pub mode: String,
    pub generated_at: String,
    pub input_summary: InputSummary,
    pub scenario_summary: ScenarioSummary,
    pub scored_events: Vec<Value>,
    pub intervention_candidates: Vec<Value>,
}

#[derive(Debug, Serialize)]
pub struct InputSummary {
    pub fetch_policy: String,
    pub normalized_event_count: usize,
    pub raw_item_count: usize,
    pub source_fetch_details: Vec<Value>,
    pub sources: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct ScenarioSummary {
    pub dominant_field: String,
    pub event_groups: Vec<Value>,
    pub headline: String,
    pub risk_level: String,
    pub top_actors: Vec<String>,
    pub top_regions: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RawItem {
    pub source: String,
    pub title: String,
    pub summary: String,
    pub link: String,
    pub published_at: Option<String>,
    pub entry_identity_hash: String,
    pub content_hash: String,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct NormalizedEvent {
    pub event_id: String,
    pub source: String,
    pub title: String,
    pub summary: String,
    pub link: String,
    pub published_at: Option<String>,
    pub keywords: Vec<String>,
    pub actors: Vec<String>,
    pub regions: Vec<String>,
    pub field_scores: BTreeMap<String, f64>,
    pub entry_identity_hash: String,
    pub content_hash: String,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ScoredEvent {
    pub event_id: String,
    pub title: String,
    pub source: String,
    pub link: String,
    pub published_at: Option<String>,
    pub actors: Vec<String>,
    pub regions: Vec<String>,
    pub keywords: Vec<String>,
    pub dominant_field: String,
    pub impact_score: f64,
    pub field_attraction: f64,
    pub divergence_score: f64,
    pub rationale: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct InterventionCandidate {
    pub priority: usize,
    pub event_id: String,
    pub target: String,
    pub intervention_type: String,
    pub reason: String,
    pub expected_effect: String,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct EventChainLink {
    pub from_event_id: String,
    pub to_event_id: String,
    pub shared_keywords: Vec<String>,
    pub shared_actors: Vec<String>,
    pub shared_regions: Vec<String>,
    pub relationship: String,
    pub shared_signal_count: usize,
    pub time_delta_hours: Option<f64>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct EventGroupSummary {
    pub group_id: String,
    pub headline_event_id: String,
    pub headline_title: String,
    pub member_event_ids: Vec<String>,
    pub member_count: usize,
    pub dominant_field: String,
    pub shared_keywords: Vec<String>,
    pub shared_actors: Vec<String>,
    pub shared_regions: Vec<String>,
    pub group_score: f64,
    pub causal_ordered_event_ids: Vec<String>,
    pub causal_span_hours: Option<f64>,
    pub evidence_chain: Vec<EventChainLink>,
    pub chain_summary: String,
    pub causal_summary: String,
}
