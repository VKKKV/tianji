pub mod models;

use std::fs;
use std::path::Path;

use models::{InputSummary, RunArtifact, ScenarioSummary};

pub const RUN_ARTIFACT_SCHEMA_VERSION: &str = "tianji.run-artifact.v1";

#[derive(Debug)]
pub enum TianJiError {
    Usage(String),
    Io(std::io::Error),
    Json(serde_json::Error),
}

impl std::fmt::Display for TianJiError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Usage(message) => write!(formatter, "{message}"),
            Self::Io(error) => write!(formatter, "{error}"),
            Self::Json(error) => write!(formatter, "{error}"),
        }
    }
}

impl std::error::Error for TianJiError {}

impl From<std::io::Error> for TianJiError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<serde_json::Error> for TianJiError {
    fn from(error: serde_json::Error) -> Self {
        Self::Json(error)
    }
}

pub fn run_fixture_path(path: impl AsRef<Path>) -> Result<RunArtifact, TianJiError> {
    let path = path.as_ref();
    let feed_text = fs::read_to_string(path)?;
    let source = fixture_source_name(path);
    let raw_item_count = count_rss_items(&feed_text);

    Ok(RunArtifact {
        schema_version: RUN_ARTIFACT_SCHEMA_VERSION.to_string(),
        mode: "fixture".to_string(),
        generated_at: "1970-01-01T00:00:00+00:00".to_string(),
        input_summary: InputSummary {
            fetch_policy: "always".to_string(),
            normalized_event_count: 0,
            raw_item_count,
            source_fetch_details: Vec::new(),
            sources: vec![source],
        },
        scenario_summary: ScenarioSummary {
            dominant_field: "unknown".to_string(),
            event_groups: Vec::new(),
            headline: "Rust Milestone 0 scaffold: scoring parity is not implemented yet."
                .to_string(),
            risk_level: "unknown".to_string(),
            top_actors: Vec::new(),
            top_regions: Vec::new(),
        },
        scored_events: Vec::new(),
        intervention_candidates: Vec::new(),
    })
}

pub fn artifact_json(artifact: &RunArtifact) -> Result<String, TianJiError> {
    Ok(serde_json::to_string_pretty(artifact)?)
}

fn fixture_source_name(path: &Path) -> String {
    let name = path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("fixture");
    format!("fixture:{name}")
}

fn count_rss_items(feed_text: &str) -> usize {
    feed_text.matches("<item>").count()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;
    use std::collections::BTreeSet;

    const SAMPLE_FIXTURE: &str = "tests/fixtures/sample_feed.xml";
    const CONTRACT_FIXTURE: &str = "tests/fixtures/contracts/run_artifact_v1.json";

    #[test]
    fn fixture_artifact_uses_current_top_level_contract_keys() {
        let artifact = run_fixture_path(SAMPLE_FIXTURE).expect("fixture artifact");
        let emitted = serde_json::to_value(artifact).expect("artifact json value");
        let contract: Value =
            serde_json::from_str(&fs::read_to_string(CONTRACT_FIXTURE).expect("contract fixture"))
                .expect("contract json value");

        assert_eq!(object_keys(&emitted), object_keys(&contract));
    }

    #[test]
    fn fixture_artifact_uses_current_nested_summary_contract_keys() {
        let artifact = run_fixture_path(SAMPLE_FIXTURE).expect("fixture artifact");
        let emitted = serde_json::to_value(artifact).expect("artifact json value");
        let contract: Value =
            serde_json::from_str(&fs::read_to_string(CONTRACT_FIXTURE).expect("contract fixture"))
                .expect("contract json value");

        assert_eq!(
            object_keys(&emitted["input_summary"]),
            object_keys(&contract["input_summary"]),
        );
        assert_eq!(
            object_keys(&emitted["scenario_summary"]),
            object_keys(&contract["scenario_summary"]),
        );
    }

    #[test]
    fn fixture_artifact_makes_missing_scoring_parity_explicit() {
        let artifact = run_fixture_path(SAMPLE_FIXTURE).expect("fixture artifact");

        assert_eq!(artifact.input_summary.raw_item_count, 3);
        assert_eq!(artifact.input_summary.normalized_event_count, 0);
        assert_eq!(artifact.scored_events.len(), 0);
        assert_eq!(artifact.intervention_candidates.len(), 0);
        assert!(artifact
            .scenario_summary
            .headline
            .contains("not implemented yet"));
    }

    fn object_keys(value: &Value) -> BTreeSet<String> {
        value
            .as_object()
            .expect("json object")
            .keys()
            .cloned()
            .collect()
    }
}
