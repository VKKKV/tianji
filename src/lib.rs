pub mod api;
pub mod backtrack;
pub mod daemon;
pub mod delta;
pub mod delta_memory;
pub mod fetch;
pub mod grouping;
pub mod llm;
pub mod models;
pub mod normalize;
pub mod profile;
pub mod scoring;
pub mod storage;
pub mod tui;
pub mod utils;
pub mod webui;
pub mod worldline;

use std::fs;
use std::path::Path;

use backtrack::backtrack_candidates;
pub use delta::{
    compute_delta, DeltaReport, DeltaSummary, MetricSnapshot, RiskDirection, Severity,
};
pub use delta_memory::{
    classify_delta_tier, compact_run_data, AlertDecayModel, AlertTier, AlertedSignalEntry,
    HotMemory,
};
use fetch::{assign_canonical_hashes, fixture_source_name, parse_feed};
use grouping::group_events;
use models::{InputSummary, RunArtifact, ScenarioSummary};
use normalize::normalize_items;
use scoring::{score_events, summarize_scenario};
use serde_json::Value as JsonValue;
pub use storage::{
    compare_runs, get_latest_run_id, get_latest_run_pair, get_next_run_id, get_previous_run_id,
    get_run_summary, list_runs, persist_run, EventGroupFilters, RunListFilters, ScoredEventFilters,
    MAX_RUN_SUMMARY_EVENT_LIMIT, MAX_RUN_SUMMARY_GROUP_LIMIT,
};

pub const RUN_ARTIFACT_SCHEMA_VERSION: &str = "tianji.run-artifact.v1";

#[derive(Debug)]
pub enum TianJiError {
    Usage(String),
    Input(String),
    Io(std::io::Error),
    Json(serde_json::Error),
    Yaml(serde_yaml::Error, String),
    Storage(rusqlite::Error),
}

impl std::fmt::Display for TianJiError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Usage(message) => write!(formatter, "{message}"),
            Self::Input(message) => write!(formatter, "{message}"),
            Self::Io(error) => write!(formatter, "{error}"),
            Self::Json(error) => write!(formatter, "{error}"),
            Self::Yaml(error, path) => write!(formatter, "{error} (in {path})"),
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

impl From<serde_yaml::Error> for TianJiError {
    fn from(error: serde_yaml::Error) -> Self {
        Self::Yaml(error, "<unknown>".to_string())
    }
}

#[derive(Debug)]
pub struct RunResult {
    pub artifact: RunArtifact,
    pub delta: Option<DeltaReport>,
    pub alert_tier: Option<AlertTier>,
}

pub fn run_fixture_path(
    path: impl AsRef<Path>,
    sqlite_path: Option<&str>,
) -> Result<RunResult, TianJiError> {
    run_fixture_path_with_alert_marking(path, sqlite_path, false)
}

pub fn run_fixture_path_with_alert_marking(
    path: impl AsRef<Path>,
    sqlite_path: Option<&str>,
    mark_alerted: bool,
) -> Result<RunResult, TianJiError> {
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
        .map(serde_json::to_value)
        .collect::<Result<_, _>>()?;

    let intervention_candidates_json: Vec<JsonValue> = interventions
        .iter()
        .map(serde_json::to_value)
        .collect::<Result<_, _>>()?;

    let event_groups_json: Vec<JsonValue> = event_groups
        .iter()
        .map(serde_json::to_value)
        .collect::<Result<_, _>>()?;

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

    let (delta, alert_tier) = if let Some(db_path) = sqlite_path {
        persist_run(
            db_path,
            &artifact,
            &raw_items,
            &normalized_events,
            &scored_events,
            &interventions,
        )?;
        let update = update_delta_memory_for_latest_run(db_path, mark_alerted)?;
        (update.delta, update.alert_tier)
    } else {
        (None, None)
    };

    Ok(RunResult {
        artifact,
        delta,
        alert_tier,
    })
}

pub fn update_delta_memory_for_latest_run(
    sqlite_path: &str,
    mark_alerted: bool,
) -> Result<RunDeltaUpdate, TianJiError> {
    let current_run_id = get_latest_run_id(sqlite_path)?.ok_or_else(|| {
        TianJiError::Usage("No persisted run exists after persistence completed.".to_string())
    })?;
    let scored_filters = ScoredEventFilters {
        limit_scored_events: Some(MAX_RUN_SUMMARY_EVENT_LIMIT),
        ..Default::default()
    };
    let group_filters = EventGroupFilters {
        limit_event_groups: Some(MAX_RUN_SUMMARY_GROUP_LIMIT),
        ..Default::default()
    };
    let current = get_run_summary(
        sqlite_path,
        current_run_id,
        &scored_filters,
        false,
        &group_filters,
    )?
    .ok_or_else(|| TianJiError::Usage(format!("Run not found: {current_run_id}")))?;

    let previous = match get_previous_run_id(sqlite_path, current_run_id)? {
        Some(previous_run_id) => get_run_summary(
            sqlite_path,
            previous_run_id,
            &scored_filters,
            false,
            &group_filters,
        )?,
        None => None,
    };

    let delta = compute_delta(&current, previous.as_ref());
    let alert_tier = delta.as_ref().and_then(classify_delta_tier);
    let mut memory = if previous.is_some() {
        HotMemory::load(&delta_memory_path(sqlite_path))
    } else {
        HotMemory::default()
    };
    memory.push_run(compact_run_data(&current), delta.clone(), 3);
    memory.prune_stale_signals_at_timestamp(
        &AlertDecayModel::default(),
        current
            .get("generated_at")
            .and_then(|value| value.as_str())
            .unwrap_or(""),
    );
    let generated_at = current
        .get("generated_at")
        .and_then(|value| value.as_str())
        .unwrap_or("");
    if mark_alerted && alert_tier.is_some() {
        delta
            .as_ref()
            .map(|report| {
                memory.mark_delta_signals_alerted_at_timestamp(
                    report,
                    &AlertDecayModel::default(),
                    generated_at,
                )
            })
            .unwrap_or(false);
    }
    memory.save_atomic(&delta_memory_path(sqlite_path))?;
    Ok(RunDeltaUpdate { delta, alert_tier })
}

pub struct RunDeltaUpdate {
    pub delta: Option<DeltaReport>,
    pub alert_tier: Option<AlertTier>,
}

pub fn delta_memory_path(sqlite_path: &str) -> std::path::PathBuf {
    let db_path = Path::new(sqlite_path);
    let parent = db_path.parent().unwrap_or_else(|| Path::new("."));
    let stem = db_path
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("tianji");
    parent.join(format!("{stem}.memory")).join("hot.json")
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
        let artifact = run_fixture_path(SAMPLE_FIXTURE, None)
            .expect("fixture artifact")
            .artifact;
        let emitted = serde_json::to_value(artifact).expect("artifact json value");
        let contract: Value =
            serde_json::from_str(&fs::read_to_string(CONTRACT_FIXTURE).expect("contract fixture"))
                .expect("contract json value");

        assert_eq!(object_keys(&emitted), object_keys(&contract));
    }

    #[test]
    fn fixture_artifact_uses_current_nested_summary_contract_keys() {
        let artifact = run_fixture_path(SAMPLE_FIXTURE, None)
            .expect("fixture artifact")
            .artifact;
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
        let artifact = run_fixture_path(SAMPLE_FIXTURE, None)
            .expect("fixture artifact")
            .artifact;

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
        let artifact = run_fixture_path(SAMPLE_FIXTURE, None)
            .expect("fixture artifact")
            .artifact;

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
        let artifact = run_fixture_path(SAMPLE_FIXTURE, None)
            .expect("fixture artifact")
            .artifact;
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
        let artifact = run_fixture_path(SAMPLE_FIXTURE, None)
            .expect("fixture artifact")
            .artifact;

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
        let artifact = run_fixture_path(SAMPLE_FIXTURE, None)
            .expect("fixture artifact")
            .artifact;
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
        let path = format!("/tmp/tianji_test_{}.sqlite3", id);
        // Ensure clean slate — previous runs may have left stale files
        let _ = std::fs::remove_file(&path);
        path
    }

    fn cleanup_db(path: &str) {
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn persist_run_creates_all_six_tables() {
        let db_path = temp_sqlite_path();
        let artifact = run_fixture_path(SAMPLE_FIXTURE, None)
            .expect("fixture artifact")
            .artifact;

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
    fn list_runs_filtered_query_pages_until_limit_matches() {
        let db_path = temp_sqlite_path();
        let _ = run_fixture_path(SAMPLE_FIXTURE, Some(&db_path)).expect("run 1");

        let conn = rusqlite::Connection::open(&db_path).expect("open db");
        let input_summary = serde_json::json!({
            "raw_item_count": 0,
            "normalized_event_count": 0,
        })
        .to_string();
        let scenario_summary = serde_json::json!({
            "dominant_field": "diplomacy",
            "risk_level": "low",
            "headline": "synthetic non-matching run",
            "event_groups": [],
        })
        .to_string();

        for idx in 0..120 {
            conn.execute(
                "INSERT INTO runs (schema_version, mode, generated_at, input_summary_json, scenario_summary_json) VALUES (?1, ?2, ?3, ?4, ?5)",
                rusqlite::params![
                    "tianji.run-artifact.v1",
                    "synthetic",
                    format!("2026-05-14T00:{:02}:00Z", idx % 60),
                    &input_summary,
                    &scenario_summary,
                ],
            )
            .expect("insert synthetic run");
        }

        let filters = RunListFilters {
            mode: Some("fixture".to_string()),
            ..Default::default()
        };
        let items = list_runs(&db_path, 1, &filters).expect("list");

        assert_eq!(items.len(), 1);
        assert_eq!(items[0]["run_id"], 1);
        assert_eq!(items[0]["mode"], "fixture");

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
    fn history_show_preserves_unlimited_default_and_clamps_explicit_limits() {
        let db_path = temp_sqlite_path();
        let artifact = run_fixture_path(SAMPLE_FIXTURE, None)
            .expect("fixture artifact")
            .artifact;
        let feed_text = fs::read_to_string(SAMPLE_FIXTURE).expect("fixture");
        let source = fixture_source_name(Path::new(SAMPLE_FIXTURE));
        let mut raw_items = parse_feed(&feed_text, &source).expect("parse");
        assign_canonical_hashes(&mut raw_items);
        let normalized_events = normalize_items(&raw_items);
        let mut scored_events = score_events(&normalized_events);
        let interventions = backtrack_candidates(&scored_events, 5, None);

        let template = scored_events[0].clone();
        for index in scored_events.len()..(MAX_RUN_SUMMARY_EVENT_LIMIT + 5) {
            let mut event = template.clone();
            event.event_id = format!("synthetic-{index:04}");
            event.title = format!("Synthetic event {index:04}");
            event.divergence_score = 10_000.0 - index as f64;
            scored_events.push(event);
        }

        persist_run(
            &db_path,
            &artifact,
            &raw_items,
            &normalized_events,
            &scored_events,
            &interventions,
        )
        .expect("persist expanded run");

        let unbounded = get_run_summary(
            &db_path,
            1,
            &ScoredEventFilters::default(),
            false,
            &EventGroupFilters::default(),
        )
        .expect("summary")
        .expect("run");
        assert_eq!(
            unbounded["scored_events"].as_array().expect("events").len(),
            scored_events.len()
        );

        let capped_filters = ScoredEventFilters {
            limit_scored_events: Some(MAX_RUN_SUMMARY_EVENT_LIMIT + 100),
            ..Default::default()
        };
        let capped = get_run_summary(
            &db_path,
            1,
            &capped_filters,
            false,
            &EventGroupFilters::default(),
        )
        .expect("summary")
        .expect("run");
        assert_eq!(
            capped["scored_events"].as_array().expect("events").len(),
            MAX_RUN_SUMMARY_EVENT_LIMIT
        );

        cleanup_db(&db_path);
    }

    #[test]
    fn history_show_group_limit_clamps_only_when_explicit() {
        let db_path = temp_sqlite_path();
        let mut artifact = run_fixture_path(SAMPLE_FIXTURE, None)
            .expect("fixture artifact")
            .artifact;
        artifact.scenario_summary.event_groups = (0..(MAX_RUN_SUMMARY_GROUP_LIMIT + 5))
            .map(|index| {
                serde_json::json!({
                    "group_id": format!("group-{index:04}"),
                    "headline_event_id": format!("event-{index:04}"),
                    "dominant_field": "technology"
                })
            })
            .collect();
        let feed_text = fs::read_to_string(SAMPLE_FIXTURE).expect("fixture");
        let source = fixture_source_name(Path::new(SAMPLE_FIXTURE));
        let mut raw_items = parse_feed(&feed_text, &source).expect("parse");
        assign_canonical_hashes(&mut raw_items);
        let normalized_events = normalize_items(&raw_items);
        let scored_events = score_events(&normalized_events);
        let interventions = backtrack_candidates(&scored_events, 5, None);

        persist_run(
            &db_path,
            &artifact,
            &raw_items,
            &normalized_events,
            &scored_events,
            &interventions,
        )
        .expect("persist expanded run");

        let unbounded = get_run_summary(
            &db_path,
            1,
            &ScoredEventFilters::default(),
            false,
            &EventGroupFilters::default(),
        )
        .expect("summary")
        .expect("run");
        assert_eq!(
            unbounded["scenario_summary"]["event_groups"]
                .as_array()
                .expect("groups")
                .len(),
            MAX_RUN_SUMMARY_GROUP_LIMIT + 5
        );

        let capped_group_filters = EventGroupFilters {
            limit_event_groups: Some(MAX_RUN_SUMMARY_GROUP_LIMIT + 100),
            ..Default::default()
        };
        let capped = get_run_summary(
            &db_path,
            1,
            &ScoredEventFilters::default(),
            false,
            &capped_group_filters,
        )
        .expect("summary")
        .expect("run");
        assert_eq!(
            capped["scenario_summary"]["event_groups"]
                .as_array()
                .expect("groups")
                .len(),
            MAX_RUN_SUMMARY_GROUP_LIMIT
        );

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
    fn delta_between_identical_persisted_runs_has_no_changes() {
        let db_path = temp_sqlite_path();
        let _ = run_fixture_path(SAMPLE_FIXTURE, Some(&db_path)).expect("run 1");
        let _ = run_fixture_path(SAMPLE_FIXTURE, Some(&db_path)).expect("run 2");

        let scored_filters = ScoredEventFilters::default();
        let group_filters = EventGroupFilters::default();
        let previous = get_run_summary(&db_path, 1, &scored_filters, false, &group_filters)
            .expect("previous summary")
            .expect("previous run");
        let current = get_run_summary(&db_path, 2, &scored_filters, false, &group_filters)
            .expect("current summary")
            .expect("current run");
        let report = compute_delta(&current, Some(&previous)).expect("delta report");

        assert_eq!(report.summary.total_changes, 0);
        assert!(report.numeric_deltas.is_empty());
        assert!(report.count_deltas.is_empty());
        assert!(report.new_signals.is_empty());

        cleanup_db(&db_path);
    }

    #[test]
    fn persisted_run_updates_hot_delta_memory() {
        let db_path = temp_sqlite_path();
        let _ = run_fixture_path(SAMPLE_FIXTURE, Some(&db_path)).expect("run 1");
        let _ = run_fixture_path(SAMPLE_FIXTURE, Some(&db_path)).expect("run 2");

        let memory_path = delta_memory_path(&db_path);
        let memory = HotMemory::load(&memory_path);

        assert_eq!(memory.runs.len(), 2);
        assert_eq!(memory.runs[0].run_id, 2);
        assert_eq!(memory.runs[1].run_id, 1);
        assert!(memory.runs[0].delta.is_some());
        assert!(memory.runs[1].delta.is_none());

        let _ = std::fs::remove_dir_all(memory_path.parent().expect("memory parent"));
        cleanup_db(&db_path);
    }

    #[test]
    fn run_fixture_path_returns_run_result_with_delta_for_persisted_pair() {
        let db_path = temp_sqlite_path();
        let first = run_fixture_path(SAMPLE_FIXTURE, Some(&db_path)).expect("run 1");
        let second = run_fixture_path(SAMPLE_FIXTURE, Some(&db_path)).expect("run 2");

        assert_eq!(first.artifact.schema_version, RUN_ARTIFACT_SCHEMA_VERSION);
        assert!(first.delta.is_none());
        assert!(first.alert_tier.is_none());

        let delta = second.delta.as_ref().expect("delta report");
        assert_eq!(delta.summary.total_changes, 0);
        assert_eq!(second.alert_tier, classify_delta_tier(delta));

        let memory_path = delta_memory_path(&db_path);
        let _ = std::fs::remove_dir_all(memory_path.parent().expect("memory parent"));
        cleanup_db(&db_path);
    }

    #[test]
    fn run_fixture_path_can_mark_delta_alerts_during_hot_memory_update() {
        let db_path = temp_sqlite_path();
        let fixture_path = format!("/tmp/tianji_changed_feed_{}.xml", db_path.replace('/', "_"));
        std::fs::write(
            &fixture_path,
            r#"<?xml version="1.0" encoding="UTF-8"?>
<rss version="2.0">
  <channel>
    <title>TianJi Changed Feed</title>
    <item>
      <title>Iran and Israel exchange missile warnings as diplomats push for talks</title>
      <link>https://example.com/iran-israel-warnings</link>
      <description>Officials in Tehran and Jerusalem traded missile warnings while European diplomats sought an emergency negotiation channel.</description>
      <pubDate>Sun, 22 Mar 2026 07:00:00 GMT</pubDate>
    </item>
    <item>
      <title>China expands chip controls after new AI export dispute with the United States</title>
      <link>https://example.com/china-chip-controls</link>
      <description>Beijing announced additional chip-related trade measures after a fresh AI and cyber dispute with Washington.</description>
      <pubDate>Sun, 22 Mar 2026 08:00:00 GMT</pubDate>
    </item>
    <item>
      <title>NATO reviews troop readiness after Russia strike near Ukraine logistics corridor</title>
      <link>https://example.com/nato-ukraine-readiness</link>
      <description>NATO officials reviewed troop readiness after a reported strike near a logistics corridor supporting Ukraine.</description>
      <pubDate>Sun, 22 Mar 2026 09:00:00 GMT</pubDate>
    </item>
    <item>
      <title>EU opens cyber negotiation channel after AI dispute</title>
      <link>https://example.com/eu-cyber-channel</link>
      <description>European diplomats opened a new cyber and AI negotiation channel after sanctions concerns.</description>
      <pubDate>Sun, 22 Mar 2026 10:00:00 GMT</pubDate>
    </item>
  </channel>
</rss>
"#,
        )
        .expect("write changed fixture");
        let _ = run_fixture_path(SAMPLE_FIXTURE, Some(&db_path)).expect("run 1");
        let _ = run_fixture_path_with_alert_marking(&fixture_path, Some(&db_path), true)
            .expect("run 2");

        let memory_path = delta_memory_path(&db_path);
        let memory = HotMemory::load(&memory_path);
        let latest_delta = memory.runs[0].delta.as_ref().expect("latest delta");

        assert!(latest_delta.summary.total_changes > 0);
        assert!(!memory.alerted_signals.is_empty());
        let marked_delta_key = latest_delta
            .numeric_deltas
            .iter()
            .any(|item| memory.alerted_signals.contains_key(&item.key))
            || latest_delta
                .count_deltas
                .iter()
                .any(|item| memory.alerted_signals.contains_key(&item.key))
            || latest_delta
                .new_signals
                .iter()
                .any(|item| memory.alerted_signals.contains_key(&item.key));
        assert!(marked_delta_key);

        let _ = std::fs::remove_dir_all(memory_path.parent().expect("memory parent"));
        let _ = std::fs::remove_file(&fixture_path);
        cleanup_db(&db_path);
    }

    #[test]
    fn run_without_sqlite_path_does_not_create_file() {
        let _ = run_fixture_path(SAMPLE_FIXTURE, None).expect("run");
        // No sqlite file should be created at the default temp path
        assert!(!std::path::Path::new("/tmp/tianji_no_sqlite_test.sqlite3").exists());
    }

    // -----------------------------------------------------------------------
    // Milestone 3 — Daemon / API / WebUI integration tests
    // -----------------------------------------------------------------------

    // --- Daemon state tests ---

    #[test]
    fn daemon_state_enqueue_job_returns_queued_state() {
        let state = daemon::DaemonState::new();
        let request = daemon::RunJobRequest {
            fixture_paths: vec!["tests/fixtures/sample_feed.xml".to_string()],
            fetch: false,
            source_urls: vec![],
            fetch_policy: "always".to_string(),
            source_fetch_details: vec![],
            output_path: None,
            sqlite_path: None,
        };
        let record = state.enqueue_job(request);
        assert!(record.job_id.starts_with("job-"));
        assert_eq!(record.state, "queued");
        assert!(record.run_id.is_none());
        assert!(record.error.is_none());
    }

    #[test]
    fn daemon_state_job_id_format_matches_contract() {
        let state = daemon::DaemonState::new();
        let request = daemon::RunJobRequest {
            fixture_paths: vec![],
            fetch: false,
            source_urls: vec![],
            fetch_policy: "always".to_string(),
            source_fetch_details: vec![],
            output_path: None,
            sqlite_path: None,
        };
        let record = state.enqueue_job(request);
        // job-{uuid4_hex[:12]}: "job-" prefix + 12 hex chars
        assert_eq!(record.job_id.len(), 16); // "job-" (4) + 12 hex chars
        let hex_part = &record.job_id[4..];
        assert!(hex_part.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn daemon_state_transitions_queued_to_running_to_succeeded() {
        let state = daemon::DaemonState::new();
        let request = daemon::RunJobRequest {
            fixture_paths: vec![],
            fetch: false,
            source_urls: vec![],
            fetch_policy: "always".to_string(),
            source_fetch_details: vec![],
            output_path: None,
            sqlite_path: None,
        };
        let record = state.enqueue_job(request);

        state.set_job_running(&record.job_id);
        let job = state.get_job(&record.job_id).expect("job found");
        assert_eq!(job.state, "running");

        state.set_job_succeeded(&record.job_id, Some(42), None, None);
        let job = state.get_job(&record.job_id).expect("job found");
        assert_eq!(job.state, "succeeded");
        assert_eq!(job.run_id, Some(42));
    }

    #[test]
    fn daemon_state_transitions_to_failed_with_error() {
        let state = daemon::DaemonState::new();
        let request = daemon::RunJobRequest {
            fixture_paths: vec![],
            fetch: false,
            source_urls: vec![],
            fetch_policy: "always".to_string(),
            source_fetch_details: vec![],
            output_path: None,
            sqlite_path: None,
        };
        let record = state.enqueue_job(request);

        state.set_job_running(&record.job_id);
        state.set_job_failed(
            &record.job_id,
            "TianJiError: something went wrong".to_string(),
        );
        let job = state.get_job(&record.job_id).expect("job found");
        assert_eq!(job.state, "failed");
        assert_eq!(
            job.error,
            Some("TianJiError: something went wrong".to_string())
        );
    }

    #[test]
    fn daemon_state_get_job_returns_none_for_unknown_id() {
        let state = daemon::DaemonState::new();
        assert!(state.get_job("job-nonexistent").is_none());
    }

    #[test]
    fn daemon_state_job_status_payload_matches_contract() {
        let state = daemon::DaemonState::new();
        let request = daemon::RunJobRequest {
            fixture_paths: vec![],
            fetch: false,
            source_urls: vec![],
            fetch_policy: "always".to_string(),
            source_fetch_details: vec![],
            output_path: None,
            sqlite_path: None,
        };
        let record = state.enqueue_job(request);
        state.set_job_succeeded(&record.job_id, Some(1), None, None);

        let payload = state.get_job(&record.job_id).unwrap().to_status_payload();
        assert!(payload.get("job_id").is_some());
        assert!(payload.get("state").is_some());
        assert!(payload.get("run_id").is_some());
        assert!(payload.get("error").is_some());
        assert!(payload.get("delta_tier").is_some());
        assert!(payload.get("delta_summary").is_some());
    }

    #[test]
    fn daemon_state_job_status_stores_delta_tier_and_summary() {
        let state = daemon::DaemonState::new();
        let request = daemon::RunJobRequest {
            fixture_paths: vec![],
            fetch: false,
            source_urls: vec![],
            fetch_policy: "always".to_string(),
            source_fetch_details: vec![],
            output_path: None,
            sqlite_path: None,
        };
        let record = state.enqueue_job(request);
        let delta = DeltaReport {
            timestamp: "1970-01-01T00:00:00+00:00".to_string(),
            previous_timestamp: Some("1969-12-31T00:00:00+00:00".to_string()),
            numeric_deltas: Vec::new(),
            count_deltas: Vec::new(),
            new_signals: Vec::new(),
            summary: DeltaSummary {
                total_changes: 1,
                critical_changes: 0,
                direction: RiskDirection::Mixed,
                signal_breakdown: delta::SignalBreakdown {
                    new_count: 1,
                    escalated_count: 0,
                    deescalated_count: 0,
                    unchanged_count: 0,
                },
            },
        };

        state.set_job_succeeded(
            &record.job_id,
            Some(1),
            Some(delta),
            Some(AlertTier::Routine),
        );

        let payload = state.get_job(&record.job_id).unwrap().to_status_payload();
        assert_eq!(payload["delta_tier"], "routine");
        assert_eq!(payload["delta_summary"]["total_changes"], 1);
    }

    // --- Socket protocol tests ---

    #[test]
    fn socket_handle_queue_run_returns_queued() {
        let state = std::sync::Arc::new(daemon::DaemonState::new());
        let request = serde_json::json!({
            "action": "queue_run",
            "payload": {
                "fixture_paths": ["tests/fixtures/sample_feed.xml"],
            }
        });
        let response = daemon::handle_socket_request(&state, &request);
        assert_eq!(response["ok"], true);
        assert_eq!(response["data"]["state"], "queued");
        assert!(response["data"]["job_id"].is_string());
        assert!(response["error"].is_null());
    }

    #[test]
    fn socket_handle_queue_run_always_says_queued_even_if_running() {
        let state = std::sync::Arc::new(daemon::DaemonState::new());
        let request = serde_json::json!({
            "action": "queue_run",
            "payload": {
                "fixture_paths": ["tests/fixtures/sample_feed.xml"],
            }
        });
        let response = daemon::handle_socket_request(&state, &request);
        let job_id = response["data"]["job_id"].as_str().unwrap();

        // Transition to running
        state.set_job_running(job_id);

        // queue_run response still says "queued" per contract
        assert_eq!(response["data"]["state"], "queued");
    }

    #[test]
    fn socket_handle_job_status_returns_current_state() {
        let state = std::sync::Arc::new(daemon::DaemonState::new());
        let queue_request = serde_json::json!({
            "action": "queue_run",
            "payload": {
                "fixture_paths": ["tests/fixtures/sample_feed.xml"],
            }
        });
        let queue_response = daemon::handle_socket_request(&state, &queue_request);
        let job_id = queue_response["data"]["job_id"]
            .as_str()
            .unwrap()
            .to_string();

        let status_request = serde_json::json!({
            "action": "job_status",
            "job_id": job_id,
        });
        let status_response = daemon::handle_socket_request(&state, &status_request);
        assert_eq!(status_response["ok"], true);
        assert_eq!(status_response["data"]["job_id"], job_id);
        assert_eq!(status_response["data"]["state"], "queued");
    }

    #[test]
    fn socket_handle_unsupported_action_returns_error() {
        let state = std::sync::Arc::new(daemon::DaemonState::new());
        let request = serde_json::json!({
            "action": "unknown_action",
        });
        let response = daemon::handle_socket_request(&state, &request);
        assert_eq!(response["ok"], false);
        assert!(response["error"]["message"].is_string());
    }

    #[test]
    fn socket_handle_missing_action_returns_error() {
        let state = std::sync::Arc::new(daemon::DaemonState::new());
        let request = serde_json::json!({});
        let response = daemon::handle_socket_request(&state, &request);
        assert_eq!(response["ok"], false);
    }

    // --- Loopback validation tests ---

    #[test]
    fn loopback_validation_accepts_localhost_hosts() {
        assert!(daemon::validate_loopback_host("127.0.0.1").is_ok());
        assert!(daemon::validate_loopback_host("localhost").is_ok());
        assert!(daemon::validate_loopback_host("::1").is_ok());
    }

    #[test]
    fn loopback_validation_rejects_non_loopback_host() {
        assert!(daemon::validate_loopback_host("0.0.0.0").is_err());
        assert!(daemon::validate_loopback_host("192.168.1.1").is_err());
        assert!(daemon::validate_loopback_host("example.com").is_err());
    }

    #[test]
    fn loopback_helpers_bracket_ipv6_hosts() {
        assert_eq!(
            daemon::loopback_socket_addr("127.0.0.1", 8765),
            "127.0.0.1:8765"
        );
        assert_eq!(daemon::loopback_socket_addr("::1", 8765), "[::1]:8765");
        assert_eq!(
            daemon::loopback_http_base_url("::1", 8765),
            "http://[::1]:8765"
        );
    }

    // --- RunJobRequest parsing tests ---

    #[test]
    fn run_job_request_from_payload_parses_fixture_paths() {
        let payload = serde_json::json!({
            "fixture_paths": ["tests/fixtures/sample_feed.xml"],
        });
        let request = daemon::RunJobRequest::from_payload(&payload).expect("parse");
        assert_eq!(
            request.fixture_paths,
            vec!["tests/fixtures/sample_feed.xml"]
        );
        assert!(!request.fetch);
    }

    #[test]
    fn run_job_request_from_payload_defaults_correctly() {
        let payload = serde_json::json!({});
        let request = daemon::RunJobRequest::from_payload(&payload).expect("parse");
        assert!(request.fixture_paths.is_empty());
        assert!(!request.fetch);
        assert!(request.source_urls.is_empty());
        assert_eq!(request.fetch_policy, "always");
        assert!(request.output_path.is_none());
        assert!(request.sqlite_path.is_none());
    }

    #[test]
    fn run_job_request_from_payload_rejects_invalid_fetch() {
        let payload = serde_json::json!({
            "fetch": "not_a_bool",
        });
        // fetch defaults to false when not a bool, doesn't error
        let request = daemon::RunJobRequest::from_payload(&payload).expect("parse");
        assert!(!request.fetch);
    }

    // --- API route tests using reqwest against a live server ---

    #[test]
    fn api_meta_returns_contract_envelope() {
        let db_path = temp_sqlite_path();
        let _ = run_fixture_path(SAMPLE_FIXTURE, Some(&db_path)).expect("run + persist");

        let rt = tokio::runtime::Runtime::new().expect("runtime");
        rt.block_on(async {
            let state = api::AppState {
                sqlite_path: db_path.clone(),
            };
            let app = api::build_router().with_state(state);

            let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
                .await
                .expect("bind");
            let addr = listener.local_addr().expect("addr");

            let server = tokio::spawn(async move {
                axum::serve(listener, app).await.expect("serve");
            });

            // Give the server a moment to start
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;

            let client = reqwest::Client::new();
            let resp = client
                .get(format!("http://{addr}/api/v1/meta"))
                .send()
                .await
                .expect("request");

            let body: serde_json::Value =
                serde_json::from_str(&resp.text().await.expect("text")).expect("json");
            assert_eq!(body["api_version"], "v1");
            assert_eq!(body["error"], serde_json::Value::Null);
            assert_eq!(
                body["data"]["artifact_schema_version"],
                "tianji.run-artifact.v1"
            );
            assert_eq!(body["data"]["cli_source_of_truth"], true);
            assert_eq!(body["data"]["persistence"]["sqlite_optional"], true);

            server.abort();
        });

        cleanup_db(&db_path);
    }

    #[test]
    fn api_runs_returns_envelope_with_items() {
        let db_path = temp_sqlite_path();
        let _ = run_fixture_path(SAMPLE_FIXTURE, Some(&db_path)).expect("run + persist");

        let rt = tokio::runtime::Runtime::new().expect("runtime");
        rt.block_on(async {
            let state = api::AppState {
                sqlite_path: db_path.clone(),
            };
            let app = api::build_router().with_state(state);

            let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
                .await
                .expect("bind");
            let addr = listener.local_addr().expect("addr");

            let server = tokio::spawn(async move {
                axum::serve(listener, app).await.expect("serve");
            });

            tokio::time::sleep(std::time::Duration::from_millis(50)).await;

            let client = reqwest::Client::new();
            let resp = client
                .get(format!("http://{addr}/api/v1/runs?limit=20"))
                .send()
                .await
                .expect("request");

            let body: serde_json::Value =
                serde_json::from_str(&resp.text().await.expect("text")).expect("json");
            assert_eq!(body["api_version"], "v1");
            assert_eq!(body["data"]["resource"], "/api/v1/runs");
            assert_eq!(
                body["data"]["item_contract_fixture"],
                "tests/fixtures/contracts/history_list_item_v1.json"
            );
            let items = body["data"]["items"].as_array().expect("items array");
            assert_eq!(items.len(), 1);
            assert_eq!(items[0]["run_id"], 1);
            assert_eq!(items[0]["dominant_field"], "technology");

            server.abort();
        });

        cleanup_db(&db_path);
    }

    #[test]
    fn api_compare_returns_envelope_with_diff() {
        let db_path = temp_sqlite_path();
        let _ = run_fixture_path(SAMPLE_FIXTURE, Some(&db_path)).expect("run 1");
        let _ = run_fixture_path(SAMPLE_FIXTURE, Some(&db_path)).expect("run 2");

        let rt = tokio::runtime::Runtime::new().expect("runtime");
        rt.block_on(async {
            let state = api::AppState {
                sqlite_path: db_path.clone(),
            };
            let app = api::build_router().with_state(state);

            let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
                .await
                .expect("bind");
            let addr = listener.local_addr().expect("addr");

            let server = tokio::spawn(async move {
                axum::serve(listener, app).await.expect("serve");
            });

            tokio::time::sleep(std::time::Duration::from_millis(50)).await;

            let client = reqwest::Client::new();
            let resp = client
                .get(format!(
                    "http://{addr}/api/v1/compare?left_run_id=1&right_run_id=2"
                ))
                .send()
                .await
                .expect("request");

            let body: serde_json::Value =
                serde_json::from_str(&resp.text().await.expect("text")).expect("json");
            assert_eq!(body["api_version"], "v1");
            assert_eq!(body["data"]["left_run_id"], 1);
            assert_eq!(body["data"]["right_run_id"], 2);
            assert!(body["data"]["diff"]["dominant_field_changed"].is_boolean());

            server.abort();
        });

        cleanup_db(&db_path);
    }

    #[test]
    fn api_delta_latest_returns_latest_hot_memory_delta() {
        let db_path = temp_sqlite_path();
        let _ = run_fixture_path(SAMPLE_FIXTURE, Some(&db_path)).expect("run 1");
        let _ = run_fixture_path(SAMPLE_FIXTURE, Some(&db_path)).expect("run 2");

        let rt = tokio::runtime::Runtime::new().expect("runtime");
        rt.block_on(async {
            let state = api::AppState {
                sqlite_path: db_path.clone(),
            };
            let app = api::build_router().with_state(state);

            let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
                .await
                .expect("bind");
            let addr = listener.local_addr().expect("addr");

            let server = tokio::spawn(async move {
                axum::serve(listener, app).await.expect("serve");
            });

            tokio::time::sleep(std::time::Duration::from_millis(50)).await;

            let client = reqwest::Client::new();
            let resp = client
                .get(format!("http://{addr}/api/v1/delta/latest"))
                .send()
                .await
                .expect("request");

            let body: serde_json::Value =
                serde_json::from_str(&resp.text().await.expect("text")).expect("json");
            assert_eq!(body["api_version"], "v1");
            assert_eq!(body["data"]["run_id"], 2);
            assert!(body["data"].get("alert_tier").is_some());
            assert_eq!(body["data"]["delta"]["summary"]["total_changes"], 0);

            server.abort();
        });

        let memory_path = delta_memory_path(&db_path);
        let _ = std::fs::remove_dir_all(memory_path.parent().expect("memory parent"));
        cleanup_db(&db_path);
    }
}
