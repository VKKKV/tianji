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
