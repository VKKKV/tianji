pub mod fetch;
pub mod models;
pub mod normalize;

use std::fs;
use std::path::Path;

use fetch::{assign_canonical_hashes, fixture_source_name, parse_feed};
use models::{InputSummary, RunArtifact, ScenarioSummary};
use normalize::normalize_items;
use serde_json::json;

pub const RUN_ARTIFACT_SCHEMA_VERSION: &str = "tianji.run-artifact.v1";

#[derive(Debug)]
pub enum TianJiError {
    Usage(String),
    Input(String),
    Io(std::io::Error),
    Json(serde_json::Error),
}

impl std::fmt::Display for TianJiError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Usage(message) => write!(formatter, "{message}"),
            Self::Input(message) => write!(formatter, "{message}"),
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
    let mut raw_items = parse_feed(&feed_text, &source)?;
    assign_canonical_hashes(&mut raw_items);
    let normalized_events = normalize_items(&raw_items);

    Ok(RunArtifact {
        schema_version: RUN_ARTIFACT_SCHEMA_VERSION.to_string(),
        mode: "fixture".to_string(),
        generated_at: "1970-01-01T00:00:00+00:00".to_string(),
        input_summary: InputSummary {
            fetch_policy: "always".to_string(),
            normalized_event_count: normalized_events.len(),
            raw_item_count: raw_items.len(),
            source_fetch_details: Vec::new(),
            sources: vec![source],
        },
        scenario_summary: ScenarioSummary {
            dominant_field: "uncategorized".to_string(),
            event_groups: Vec::new(),
            headline: "Rust Milestone 1A scaffold: feed parsing and normalization are implemented; scoring, grouping, and backtracking parity are not implemented yet."
                .to_string(),
            risk_level: "unknown".to_string(),
            top_actors: Vec::new(),
            top_regions: Vec::new(),
        },
        scored_events: normalized_events
            .into_iter()
            .map(|event| json!(event))
            .collect(),
        intervention_candidates: Vec::new(),
    })
}

pub fn artifact_json(artifact: &RunArtifact) -> Result<String, TianJiError> {
    Ok(serde_json::to_string_pretty(artifact)?)
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
        assert_eq!(artifact.input_summary.normalized_event_count, 3);
        assert_eq!(artifact.scored_events.len(), 3);
        assert_eq!(artifact.intervention_candidates.len(), 0);
        assert!(artifact
            .scenario_summary
            .headline
            .contains("not implemented yet"));
    }

    #[test]
    fn rss_fixture_hashes_match_python_oracle() {
        let feed_text = fs::read_to_string(SAMPLE_FIXTURE).expect("sample fixture");
        let mut items = parse_feed(&feed_text, "fixture:sample_feed.xml").expect("parsed feed");
        assign_canonical_hashes(&mut items);

        assert_eq!(items.len(), 3);
        assert_eq!(
            items[0].entry_identity_hash,
            "48e9c7c7ba1368ae400e24d2c52d7ff0a548cc30d7d802a1c3818a1d6408c11c"
        );
        assert_eq!(
            items[0].content_hash,
            "85a987c6722144b1fa896dc6e165254f553565e39362bcb566c1d91255c64c15"
        );
        assert_eq!(
            items[1].entry_identity_hash,
            "3ce9adc06380207df43d8e4390c177e557d5a890a3f6429d58794c41985200f3"
        );
        assert_eq!(
            items[1].content_hash,
            "d0f2b281b31ec2b2558bb89634be64aa059ec2d657dc39643eae3fa267e3046f"
        );
        assert_eq!(
            items[2].entry_identity_hash,
            "9a71b7afc33187e1a7ba58a4cd579e49a8a538f5d5e92982c79be971a815233f"
        );
        assert_eq!(
            items[2].content_hash,
            "29e6f6b81feda388a44508499c02f328f15b7d5c996e586277e26e6e2ae19151"
        );
    }

    #[test]
    fn rss_fixture_normalization_matches_python_oracle() {
        let artifact = run_fixture_path(SAMPLE_FIXTURE).expect("fixture artifact");
        let events = artifact.scored_events;

        assert_eq!(events[0]["event_id"], "82f82016429ecd76");
        assert_eq!(events[0]["keywords"][0], "iran");
        assert_eq!(events[0]["keywords"][11], "tehran");
        assert_eq!(events[0]["actors"], json!(["iran"]));
        assert_eq!(events[0]["regions"], json!(["middle-east"]));
        assert_eq!(events[0]["field_scores"]["conflict"], 3.5);
        assert_eq!(events[0]["field_scores"]["diplomacy"], 5.5);
        assert_eq!(events[0]["field_scores"]["technology"], 0.0);
        assert_eq!(events[0]["field_scores"]["economy"], 2.0);

        assert_eq!(events[1]["event_id"], "1e007871b783bb48");
        assert_eq!(events[1]["actors"], json!(["usa", "china"]));
        assert_eq!(events[1]["regions"], json!(["east-asia", "united-states"]));
        assert_eq!(events[1]["field_scores"]["technology"], 6.5);
        assert_eq!(events[1]["field_scores"]["economy"], 2.0);

        assert_eq!(events[2]["event_id"], "a617fdd9a05f9f2c");
        assert_eq!(events[2]["actors"], json!(["nato", "russia"]));
        assert_eq!(events[2]["regions"], json!(["ukraine", "russia", "europe"]));
        assert_eq!(events[2]["field_scores"]["conflict"], 3.0);
        assert_eq!(events[2]["field_scores"]["technology"], 2.0);
    }

    #[test]
    fn atom_fixture_parses_and_normalizes_one_titled_entry() {
        let atom_feed = r#"<?xml version="1.0" encoding="utf-8"?>
<feed xmlns="http://www.w3.org/2005/Atom">
  <title>TianJi Atom Feed</title>
  <entry>
    <title>EU opens new negotiation channel after cyber dispute</title>
    <link href="https://example.com/eu-negotiation" />
    <updated>2026-03-22T10:00:00Z</updated>
    <content>European Union officials opened a new negotiation channel after a cyber dispute with Beijing.</content>
  </entry>
  <entry>
    <title> </title>
    <link href="https://example.com/ignored" />
    <updated>2026-03-22T11:00:00Z</updated>
    <summary>This entry should be ignored because it has no usable title.</summary>
  </entry>
</feed>"#;
        let mut items = parse_feed(atom_feed, "fixture:sample_atom.xml").expect("atom feed");
        assign_canonical_hashes(&mut items);
        let events = normalize_items(&items);

        assert_eq!(items.len(), 1);
        assert_eq!(events.len(), 1);
        assert_eq!(
            events[0].title,
            "EU opens new negotiation channel after cyber dispute"
        );
        assert_eq!(
            events[0].actors,
            vec!["eu".to_string(), "china".to_string()]
        );
        assert_eq!(events[0].regions, vec!["europe".to_string()]);
        assert_eq!(events[0].field_scores["diplomacy"], 3.0);
        assert_eq!(events[0].field_scores["technology"], 2.5);
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
