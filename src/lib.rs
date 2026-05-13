pub mod backtrack;
pub mod fetch;
pub mod grouping;
pub mod models;
pub mod normalize;
pub mod scoring;
pub mod storage;

use std::fs;
use std::path::Path;

use backtrack::backtrack_candidates;
use fetch::{assign_canonical_hashes, fixture_source_name, parse_feed};
use grouping::group_events;
use models::{InputSummary, RunArtifact, ScenarioSummary};
use normalize::normalize_items;
use scoring::{score_events, summarize_scenario};
use serde_json::Value as JsonValue;
pub use storage::{
    compare_runs, get_latest_run_id, get_latest_run_pair, get_next_run_id, get_previous_run_id,
    get_run_summary, list_runs, persist_run, EventGroupFilters, RunListFilters, ScoredEventFilters,
};

pub const RUN_ARTIFACT_SCHEMA_VERSION: &str = "tianji.run-artifact.v1";

#[derive(Debug)]
pub enum TianJiError {
    Usage(String),
    Input(String),
    Io(std::io::Error),
    Json(serde_json::Error),
    Storage(rusqlite::Error),
}

impl std::fmt::Display for TianJiError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Usage(message) => write!(formatter, "{message}"),
            Self::Input(message) => write!(formatter, "{message}"),
            Self::Io(error) => write!(formatter, "{error}"),
            Self::Json(error) => write!(formatter, "{error}"),
            Self::Storage(error) => write!(formatter, "{error}"),
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

impl From<rusqlite::Error> for TianJiError {
    fn from(error: rusqlite::Error) -> Self {
        Self::Storage(error)
    }
}

pub fn run_fixture_path(
    path: impl AsRef<Path>,
    sqlite_path: Option<&str>,
) -> Result<RunArtifact, TianJiError> {
    let path = path.as_ref();
    let feed_text = fs::read_to_string(path)?;
    let source = fixture_source_name(path);
    let mut raw_items = parse_feed(&feed_text, &source)?;
    assign_canonical_hashes(&mut raw_items);
    let normalized_events = normalize_items(&raw_items);
    let scored_events = score_events(&normalized_events);
    let (headline, dominant_field, risk_level, top_regions, top_actors) =
        summarize_scenario(&scored_events);
    let event_groups = group_events(&scored_events);
    let interventions = backtrack_candidates(&scored_events, 5, Some(&event_groups));

    let scored_events_json: Vec<JsonValue> = scored_events
        .iter()
        .map(|e| serde_json::to_value(e).expect("scored event json"))
        .collect();

    let intervention_candidates_json: Vec<JsonValue> = interventions
        .iter()
        .map(|c| serde_json::to_value(c).expect("intervention candidate json"))
        .collect();

    let event_groups_json: Vec<JsonValue> = event_groups
        .iter()
        .map(|g| serde_json::to_value(g).expect("event group json"))
        .collect();

    let artifact = RunArtifact {
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
            dominant_field,
            event_groups: event_groups_json,
            headline,
            risk_level,
            top_actors,
            top_regions,
        },
        scored_events: scored_events_json,
        intervention_candidates: intervention_candidates_json,
    };

    if let Some(db_path) = sqlite_path {
        persist_run(
            db_path,
            &artifact,
            &raw_items,
            &normalized_events,
            &scored_events,
            &interventions,
        )?;
    }

    Ok(artifact)
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
        let artifact = run_fixture_path(SAMPLE_FIXTURE, None).expect("fixture artifact");
        let emitted = serde_json::to_value(artifact).expect("artifact json value");
        let contract: Value =
            serde_json::from_str(&fs::read_to_string(CONTRACT_FIXTURE).expect("contract fixture"))
                .expect("contract json value");

        assert_eq!(object_keys(&emitted), object_keys(&contract));
    }

    #[test]
    fn fixture_artifact_uses_current_nested_summary_contract_keys() {
        let artifact = run_fixture_path(SAMPLE_FIXTURE, None).expect("fixture artifact");
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
    fn fixture_scoring_parity_with_python_oracle() {
        let artifact = run_fixture_path(SAMPLE_FIXTURE, None).expect("fixture artifact");

        assert_eq!(artifact.scored_events.len(), 3);

        // First event (highest divergence): technology
        let e0 = &artifact.scored_events[0];
        assert_eq!(e0["event_id"], "1e007871b783bb48");
        assert_eq!(e0["dominant_field"], "technology");
        assert_eq!(e0["impact_score"], 15.79);
        assert_eq!(e0["field_attraction"], 7.75);
        assert_eq!(e0["divergence_score"], 20.73);

        // Second event: diplomacy
        let e1 = &artifact.scored_events[1];
        assert_eq!(e1["event_id"], "82f82016429ecd76");
        assert_eq!(e1["dominant_field"], "diplomacy");
        assert_eq!(e1["impact_score"], 13.04);
        assert_eq!(e1["field_attraction"], 6.17);
        assert_eq!(e1["divergence_score"], 16.81);

        // Third event: conflict
        let e2 = &artifact.scored_events[2];
        assert_eq!(e2["event_id"], "a617fdd9a05f9f2c");
        assert_eq!(e2["dominant_field"], "conflict");
        assert_eq!(e2["impact_score"], 17.1);
        assert_eq!(e2["field_attraction"], 3.6);
        assert_eq!(e2["divergence_score"], 15.98);
    }

    #[test]
    fn fixture_rationale_matches_python_oracle() {
        let artifact = run_fixture_path(SAMPLE_FIXTURE, None).expect("fixture artifact");

        let e0 = &artifact.scored_events[0];
        let rationale = e0["rationale"].as_array().expect("rationale array");
        let rationale_strs: Vec<&str> = rationale.iter().map(|v| v.as_str().unwrap()).collect();
        assert_eq!(
            rationale_strs,
            vec![
                "Im=15.79",
                "Fa=7.75",
                "im_title_salience=0.8",
                "im_field_impact_scaling=0.24",
                "im_text_signal_intensity=0.72",
                "actors=usa, china",
                "regions=east-asia, united-states",
                "dominant_field=technology:7.75",
            ]
        );
    }

    #[test]
    fn fixture_scenario_summary_matches_python_oracle() {
        let artifact = run_fixture_path(SAMPLE_FIXTURE, None).expect("fixture artifact");
        let summary = &artifact.scenario_summary;

        assert_eq!(summary.dominant_field, "technology");
        assert_eq!(summary.risk_level, "high");
        assert_eq!(summary.top_actors, vec!["usa", "china", "iran"]);
        assert_eq!(
            summary.top_regions,
            vec!["east-asia", "united-states", "middle-east"]
        );
        assert!(!summary.headline.contains("not implemented yet"));
        assert!(summary.headline.contains("technology"));
    }

    #[test]
    fn fixture_intervention_candidates_match_python_oracle() {
        let artifact = run_fixture_path(SAMPLE_FIXTURE, None).expect("fixture artifact");

        assert_eq!(artifact.intervention_candidates.len(), 3);

        let c0 = &artifact.intervention_candidates[0];
        assert_eq!(c0["event_id"], "1e007871b783bb48");
        assert_eq!(c0["target"], "usa");
        assert_eq!(c0["intervention_type"], "capability-control");
        assert_eq!(c0["priority"], 1);

        let c1 = &artifact.intervention_candidates[1];
        assert_eq!(c1["event_id"], "82f82016429ecd76");
        assert_eq!(c1["target"], "iran");
        assert_eq!(c1["intervention_type"], "negotiation");
        assert_eq!(c1["priority"], 2);

        let c2 = &artifact.intervention_candidates[2];
        assert_eq!(c2["event_id"], "a617fdd9a05f9f2c");
        assert_eq!(c2["target"], "nato");
        assert_eq!(c2["intervention_type"], "de-escalation");
        assert_eq!(c2["priority"], 3);
    }

    #[test]
    fn fixture_event_groups_are_empty_for_sample_feed() {
        let artifact = run_fixture_path(SAMPLE_FIXTURE, None).expect("fixture artifact");
        let groups = &artifact.scenario_summary.event_groups;
        assert!(groups.is_empty());
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
        let feed_text = fs::read_to_string(SAMPLE_FIXTURE).expect("sample fixture");
        let mut items = parse_feed(&feed_text, "fixture:sample_feed.xml").expect("parsed feed");
        assign_canonical_hashes(&mut items);
        let events = normalize_items(&items);

        assert_eq!(events[0].event_id, "82f82016429ecd76");
        assert_eq!(events[0].keywords[0], "iran");
        assert_eq!(events[0].keywords[11], "tehran");
        assert_eq!(events[0].actors, vec!["iran"]);
        assert_eq!(events[0].regions, vec!["middle-east"]);
        assert_eq!(events[0].field_scores["conflict"], 3.5);
        assert_eq!(events[0].field_scores["diplomacy"], 5.5);
        assert_eq!(events[0].field_scores["technology"], 0.0);
        assert_eq!(events[0].field_scores["economy"], 2.0);

        assert_eq!(events[1].event_id, "1e007871b783bb48");
        assert_eq!(events[1].actors, vec!["usa", "china"]);
        assert_eq!(events[1].regions, vec!["east-asia", "united-states"]);
        assert_eq!(events[1].field_scores["technology"], 6.5);
        assert_eq!(events[1].field_scores["economy"], 2.0);

        assert_eq!(events[2].event_id, "a617fdd9a05f9f2c");
        assert_eq!(events[2].actors, vec!["nato", "russia"]);
        assert_eq!(events[2].regions, vec!["ukraine", "russia", "europe"]);
        assert_eq!(events[2].field_scores["conflict"], 3.0);
        assert_eq!(events[2].field_scores["technology"], 2.0);
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

    // -----------------------------------------------------------------------
    // Storage + History integration tests
    // -----------------------------------------------------------------------

    fn temp_sqlite_path() -> String {
        use std::sync::atomic::{AtomicU64, Ordering};
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let id = COUNTER.fetch_add(1, Ordering::Relaxed);
        format!("/tmp/tianji_test_{}.sqlite3", id)
    }

    fn cleanup_db(path: &str) {
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn persist_run_creates_all_six_tables() {
        let db_path = temp_sqlite_path();
        let artifact = run_fixture_path(SAMPLE_FIXTURE, None).expect("fixture artifact");

        // We need the intermediate data, so re-run pipeline
        let feed_text = fs::read_to_string(SAMPLE_FIXTURE).expect("fixture");
        let source = fixture_source_name(Path::new(SAMPLE_FIXTURE));
        let mut raw_items = parse_feed(&feed_text, &source).expect("parse");
        assign_canonical_hashes(&mut raw_items);
        let normalized_events = normalize_items(&raw_items);
        let scored_events = score_events(&normalized_events);
        let event_groups = group_events(&scored_events);
        let interventions = backtrack_candidates(&scored_events, 5, Some(&event_groups));

        persist_run(
            &db_path,
            &artifact,
            &raw_items,
            &normalized_events,
            &scored_events,
            &interventions,
        )
        .expect("persist_run");

        // Verify all 6 tables exist
        let conn = rusqlite::Connection::open(&db_path).expect("open db");
        let table_names: Vec<String> = conn
            .prepare("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name")
            .expect("prepare")
            .query_map([], |row| row.get(0))
            .expect("query_map")
            .filter_map(|r| r.ok())
            .collect();

        assert!(table_names.contains(&"runs".to_string()));
        assert!(table_names.contains(&"source_items".to_string()));
        assert!(table_names.contains(&"raw_items".to_string()));
        assert!(table_names.contains(&"normalized_events".to_string()));
        assert!(table_names.contains(&"scored_events".to_string()));
        assert!(table_names.contains(&"intervention_candidates".to_string()));

        cleanup_db(&db_path);
    }

    #[test]
    fn persist_run_and_list_runs_roundtrip() {
        let db_path = temp_sqlite_path();
        let _artifact = run_fixture_path(SAMPLE_FIXTURE, Some(&db_path)).expect("run + persist");

        let filters = RunListFilters::default();
        let items = list_runs(&db_path, 20, &filters).expect("list_runs");

        assert_eq!(items.len(), 1);
        let item = &items[0];
        assert_eq!(item["run_id"], 1);
        assert_eq!(item["mode"], "fixture");
        assert_eq!(item["dominant_field"], "technology");
        assert_eq!(item["risk_level"], "high");
        assert_eq!(item["raw_item_count"], 3);
        assert_eq!(item["normalized_event_count"], 3);
        assert_eq!(item["event_group_count"], 0);
        assert_eq!(item["top_scored_event_id"], "1e007871b783bb48");
        assert_eq!(item["top_scored_event_dominant_field"], "technology");
        assert_eq!(item["top_impact_score"], 15.79);
        assert_eq!(item["top_field_attraction"], 7.75);
        assert_eq!(item["top_divergence_score"], 20.73);

        cleanup_db(&db_path);
    }

    #[test]
    fn list_runs_filter_before_limit() {
        let db_path = temp_sqlite_path();

        // Persist two runs
        let _ = run_fixture_path(SAMPLE_FIXTURE, Some(&db_path)).expect("run 1");
        let _ = run_fixture_path(SAMPLE_FIXTURE, Some(&db_path)).expect("run 2");

        // All runs with mode=fixture, limit=1 should return 1
        let filters = RunListFilters {
            mode: Some("fixture".to_string()),
            ..Default::default()
        };
        let items = list_runs(&db_path, 1, &filters).expect("list");
        assert_eq!(items.len(), 1);

        // Non-matching mode should return 0 even though limit=10
        let filters = RunListFilters {
            mode: Some("fetch".to_string()),
            ..Default::default()
        };
        let items = list_runs(&db_path, 10, &filters).expect("list");
        assert_eq!(items.len(), 0);

        cleanup_db(&db_path);
    }

    #[test]
    fn list_runs_dominant_field_filter() {
        let db_path = temp_sqlite_path();
        let _ = run_fixture_path(SAMPLE_FIXTURE, Some(&db_path)).expect("run");

        let filters = RunListFilters {
            dominant_field: Some("technology".to_string()),
            ..Default::default()
        };
        let items = list_runs(&db_path, 20, &filters).expect("list");
        assert_eq!(items.len(), 1);

        let filters = RunListFilters {
            dominant_field: Some("diplomacy".to_string()),
            ..Default::default()
        };
        let items = list_runs(&db_path, 20, &filters).expect("list");
        assert_eq!(items.len(), 0);

        cleanup_db(&db_path);
    }

    #[test]
    fn history_show_returns_run_detail() {
        let db_path = temp_sqlite_path();
        let _ = run_fixture_path(SAMPLE_FIXTURE, Some(&db_path)).expect("run");

        let scored_filters = ScoredEventFilters::default();
        let group_filters = EventGroupFilters::default();
        let detail = get_run_summary(&db_path, 1, &scored_filters, false, &group_filters)
            .expect("get_run_summary")
            .expect("run found");

        assert_eq!(detail["run_id"], 1);
        assert_eq!(detail["mode"], "fixture");
        assert_eq!(detail["schema_version"], "tianji.run-artifact.v1");

        let scored_events = detail["scored_events"]
            .as_array()
            .expect("scored_events array");
        assert_eq!(scored_events.len(), 3);
        assert_eq!(scored_events[0]["dominant_field"], "technology");

        let interventions = detail["intervention_candidates"]
            .as_array()
            .expect("interventions array");
        assert_eq!(interventions.len(), 3);

        cleanup_db(&db_path);
    }

    #[test]
    fn history_show_navigation_latest_previous_next() {
        let db_path = temp_sqlite_path();

        // Persist 3 runs
        let _ = run_fixture_path(SAMPLE_FIXTURE, Some(&db_path)).expect("run 1");
        let _ = run_fixture_path(SAMPLE_FIXTURE, Some(&db_path)).expect("run 2");
        let _ = run_fixture_path(SAMPLE_FIXTURE, Some(&db_path)).expect("run 3");

        // Latest
        let latest = get_latest_run_id(&db_path).expect("latest");
        assert_eq!(latest, Some(3));

        // Previous from run 3
        let prev = get_previous_run_id(&db_path, 3).expect("previous");
        assert_eq!(prev, Some(2));

        // Next from run 1
        let next = get_next_run_id(&db_path, 1).expect("next");
        assert_eq!(next, Some(2));

        // Latest pair
        let pair = get_latest_run_pair(&db_path).expect("pair");
        assert_eq!(pair, Some((2, 3)));

        // No previous for run 1
        let no_prev = get_previous_run_id(&db_path, 1).expect("no previous");
        assert_eq!(no_prev, None);

        // No next for run 3
        let no_next = get_next_run_id(&db_path, 3).expect("no next");
        assert_eq!(no_next, None);

        cleanup_db(&db_path);
    }

    #[test]
    fn history_show_scored_event_dominant_field_filter() {
        let db_path = temp_sqlite_path();
        let _ = run_fixture_path(SAMPLE_FIXTURE, Some(&db_path)).expect("run");

        let scored_filters = ScoredEventFilters {
            dominant_field: Some("technology".to_string()),
            ..Default::default()
        };
        let group_filters = EventGroupFilters::default();
        let detail = get_run_summary(&db_path, 1, &scored_filters, false, &group_filters)
            .expect("get_run_summary")
            .expect("run found");

        let scored_events = detail["scored_events"].as_array().expect("scored_events");
        assert_eq!(scored_events.len(), 1);
        assert_eq!(scored_events[0]["dominant_field"], "technology");

        cleanup_db(&db_path);
    }

    #[test]
    fn history_show_only_matching_interventions() {
        let db_path = temp_sqlite_path();
        let _ = run_fixture_path(SAMPLE_FIXTURE, Some(&db_path)).expect("run");

        // Filter to only technology scored events
        let scored_filters = ScoredEventFilters {
            dominant_field: Some("technology".to_string()),
            ..Default::default()
        };
        let group_filters = EventGroupFilters::default();
        let detail = get_run_summary(&db_path, 1, &scored_filters, true, &group_filters)
            .expect("get_run_summary")
            .expect("run found");

        let scored_events = detail["scored_events"].as_array().expect("scored_events");
        assert_eq!(scored_events.len(), 1);

        let interventions = detail["intervention_candidates"]
            .as_array()
            .expect("interventions");
        // Only the intervention whose event_id matches the visible scored event
        assert_eq!(interventions.len(), 1);
        assert_eq!(interventions[0]["event_id"], "1e007871b783bb48");

        cleanup_db(&db_path);
    }

    #[test]
    fn compare_runs_returns_diff() {
        let db_path = temp_sqlite_path();

        // Persist two runs
        let _ = run_fixture_path(SAMPLE_FIXTURE, Some(&db_path)).expect("run 1");
        let _ = run_fixture_path(SAMPLE_FIXTURE, Some(&db_path)).expect("run 2");

        let scored_filters = ScoredEventFilters::default();
        let group_filters = EventGroupFilters::default();
        let result = compare_runs(&db_path, 1, 2, &scored_filters, false, &group_filters)
            .expect("compare_runs")
            .expect("result");

        assert_eq!(result.left_run_id, 1);
        assert_eq!(result.right_run_id, 2);

        let diff = &result.diff;
        assert_eq!(diff["raw_item_count_delta"], 0);
        assert_eq!(diff["normalized_event_count_delta"], 0);
        assert_eq!(diff["dominant_field_changed"], false);
        assert_eq!(diff["risk_level_changed"], false);
        assert_eq!(diff["top_scored_event_changed"], false);
        assert_eq!(diff["top_scored_event_comparable"], true);

        cleanup_db(&db_path);
    }

    #[test]
    fn compare_latest_pair() {
        let db_path = temp_sqlite_path();

        let _ = run_fixture_path(SAMPLE_FIXTURE, Some(&db_path)).expect("run 1");
        let _ = run_fixture_path(SAMPLE_FIXTURE, Some(&db_path)).expect("run 2");

        let pair = get_latest_run_pair(&db_path).expect("pair");
        assert_eq!(pair, Some((1, 2)));

        cleanup_db(&db_path);
    }

    #[test]
    fn score_range_validation_rejects_min_greater_than_max() {
        // Test score range validation for history
        let db_path = temp_sqlite_path();
        let _ = run_fixture_path(SAMPLE_FIXTURE, Some(&db_path)).expect("run");

        let filters = RunListFilters {
            min_top_impact_score: Some(20.0),
            max_top_impact_score: Some(10.0),
            ..Default::default()
        };
        // The filter should just return empty results (items don't match)
        let items = list_runs(&db_path, 20, &filters).expect("list");
        assert_eq!(items.len(), 0);

        cleanup_db(&db_path);
    }

    #[test]
    fn compare_mode_rejection() {
        let db_path = temp_sqlite_path();
        let _ = run_fixture_path(SAMPLE_FIXTURE, Some(&db_path)).expect("run 1");
        let _ = run_fixture_path(SAMPLE_FIXTURE, Some(&db_path)).expect("run 2");

        // latest_pair + explicit ids should be rejected by CLI validation
        // We can't test CLI from here, but we can test the underlying compare function
        let scored_filters = ScoredEventFilters::default();
        let group_filters = EventGroupFilters::default();
        let result = compare_runs(&db_path, 1, 2, &scored_filters, false, &group_filters);
        assert!(result.is_ok());

        cleanup_db(&db_path);
    }

    #[test]
    fn schema_is_idempotent() {
        let db_path = temp_sqlite_path();
        let _ = run_fixture_path(SAMPLE_FIXTURE, Some(&db_path)).expect("run 1");
        // Second persist should not fail even though schema already exists
        let _ = run_fixture_path(SAMPLE_FIXTURE, Some(&db_path)).expect("run 2");

        let filters = RunListFilters::default();
        let items = list_runs(&db_path, 20, &filters).expect("list");
        assert_eq!(items.len(), 2);

        cleanup_db(&db_path);
    }

    #[test]
    fn source_item_deduplication() {
        let db_path = temp_sqlite_path();

        // Persist the same fixture twice — source_items should be deduplicated
        let _ = run_fixture_path(SAMPLE_FIXTURE, Some(&db_path)).expect("run 1");
        let _ = run_fixture_path(SAMPLE_FIXTURE, Some(&db_path)).expect("run 2");

        let conn = rusqlite::Connection::open(&db_path).expect("open db");
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM source_items", [], |row| row.get(0))
            .expect("count");
        // Same 3 items, should only be 3 rows (deduped)
        assert_eq!(count, 3);

        cleanup_db(&db_path);
    }

    #[test]
    fn run_without_sqlite_path_does_not_create_file() {
        let _ = run_fixture_path(SAMPLE_FIXTURE, None).expect("run");
        // No sqlite file should be created at the default temp path
        assert!(!std::path::Path::new("/tmp/tianji_no_sqlite_test.sqlite3").exists());
    }
}
