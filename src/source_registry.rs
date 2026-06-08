use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::storage::{LatestSourceHealth, SourceHealthCheckInput};
use crate::{run_feed_text, run_fixture_path, TianJiError};

pub const SOURCES_REPORT_SCHEMA_VERSION: &str = "tianji.sources-report.v1";

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct SourceManifest {
    #[serde(default)]
    pub sources: Vec<SourceDefinition>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct SourceDefinition {
    pub id: String,
    pub name: String,
    pub enabled: bool,
    pub tier: String,
    pub kind: SourceKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SourceKind {
    Fixture,
    Rss,
    Atom,
    Unknown(String),
}

impl Serialize for SourceKind {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(match self {
            Self::Fixture => "fixture",
            Self::Rss => "rss",
            Self::Atom => "atom",
            Self::Unknown(kind) => kind,
        })
    }
}

impl<'de> Deserialize<'de> for SourceKind {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let kind = String::deserialize(deserializer)?;
        Ok(match kind.trim() {
            "fixture" => Self::Fixture,
            "rss" => Self::Rss,
            "atom" => Self::Atom,
            _ => Self::Unknown(kind),
        })
    }
}

impl SourceKind {
    pub fn as_str(&self) -> &str {
        match self {
            Self::Fixture => "fixture",
            Self::Rss => "rss",
            Self::Atom => "atom",
            Self::Unknown(kind) => kind,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct SourcesReport {
    pub schema_version: String,
    pub config: String,
    pub total: usize,
    pub enabled: usize,
    pub disabled: usize,
    pub ready: usize,
    pub skipped: usize,
    pub errors: usize,
    pub tiers: BTreeMap<String, usize>,
    pub sources: Vec<SourceStatusReport>,
    pub runs: Vec<SourceRunReport>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct SourceStatusReport {
    pub id: String,
    pub name: String,
    pub enabled: bool,
    pub tier: String,
    pub kind: SourceKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    pub tags: Vec<String>,
    pub status: String,
    pub runnable: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_success: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_error_message: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct SourceRunReport {
    pub source_id: String,
    pub kind: SourceKind,
    pub status: String,
    pub checked_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub raw_item_count: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub normalized_event_count: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scored_event_count: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub intervention_candidate_count: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dominant_field: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub risk_level: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

pub fn load_source_manifest(path: impl AsRef<Path>) -> Result<SourceManifest, TianJiError> {
    let path = path.as_ref();
    let raw = fs::read_to_string(path)?;
    let manifest: SourceManifest = serde_yaml::from_str(&raw)
        .map_err(|error| TianJiError::Yaml(error, path.display().to_string()))?;
    validate_source_manifest(&manifest)?;
    Ok(manifest)
}

pub fn validate_source_manifest(manifest: &SourceManifest) -> Result<(), TianJiError> {
    let mut seen_ids = BTreeSet::new();
    for source in &manifest.sources {
        let id = source.id.trim();
        if id.is_empty() {
            return Err(TianJiError::Input(
                "Source id must be non-empty.".to_string(),
            ));
        }
        if !seen_ids.insert(id.to_string()) {
            return Err(TianJiError::Input(format!(
                "Duplicate source id in registry: {id}"
            )));
        }
        if source.name.trim().is_empty() {
            return Err(TianJiError::Input(format!(
                "Source {id} name must be non-empty."
            )));
        }
        if source.tier.trim().is_empty() {
            return Err(TianJiError::Input(format!(
                "Source {id} tier must be non-empty."
            )));
        }
        match &source.kind {
            SourceKind::Fixture => {
                require_non_empty(source.path.as_deref(), id, "path", "fixture")?;
            }
            SourceKind::Rss => {
                require_non_empty(source.url.as_deref(), id, "url", "rss")?;
            }
            SourceKind::Atom => {
                require_non_empty(source.url.as_deref(), id, "url", "atom")?;
            }
            SourceKind::Unknown(kind) => {
                return Err(TianJiError::Input(format!(
                    "Source {id} has unknown kind: {kind}."
                )));
            }
        }
    }
    Ok(())
}

pub fn build_sources_report(
    config_path: &str,
    manifest: SourceManifest,
    run_fixtures: bool,
    fetch_live: bool,
) -> Result<SourcesReport, TianJiError> {
    build_sources_report_with_fetcher(
        config_path,
        manifest,
        run_fixtures,
        fetch_live,
        BTreeMap::new(),
        fetch_feed_url,
    )
}

pub fn build_sources_report_with_health(
    config_path: &str,
    manifest: SourceManifest,
    run_fixtures: bool,
    fetch_live: bool,
    latest_health: BTreeMap<String, LatestSourceHealth>,
) -> Result<SourcesReport, TianJiError> {
    build_sources_report_with_fetcher(
        config_path,
        manifest,
        run_fixtures,
        fetch_live,
        latest_health,
        fetch_feed_url,
    )
}

pub fn build_sources_report_with_fetcher<F>(
    config_path: &str,
    manifest: SourceManifest,
    run_fixtures: bool,
    fetch_live: bool,
    latest_health: BTreeMap<String, LatestSourceHealth>,
    mut fetcher: F,
) -> Result<SourcesReport, TianJiError>
where
    F: FnMut(&str) -> Result<String, TianJiError>,
{
    let mut tiers = BTreeMap::new();
    for source in &manifest.sources {
        *tiers.entry(source.tier.clone()).or_insert(0) += 1;
    }

    let total = manifest.sources.len();
    let enabled = manifest
        .sources
        .iter()
        .filter(|source| source.enabled)
        .count();
    let disabled = total.saturating_sub(enabled);
    let checked_at = current_checked_at();
    let runs = build_source_runs(
        &manifest.sources,
        run_fixtures,
        fetch_live,
        &checked_at,
        &mut fetcher,
    );
    let sources = build_source_statuses(
        &manifest.sources,
        &runs,
        run_fixtures,
        fetch_live,
        &latest_health,
    );
    let ready = sources
        .iter()
        .filter(|source| source.status == "ready")
        .count();
    let skipped = sources
        .iter()
        .filter(|source| source.status == "skipped")
        .count();
    let errors = sources
        .iter()
        .filter(|source| source.status == "error")
        .count();

    Ok(SourcesReport {
        schema_version: SOURCES_REPORT_SCHEMA_VERSION.to_string(),
        config: config_path.to_string(),
        total,
        enabled,
        disabled,
        ready,
        skipped,
        errors,
        tiers,
        sources,
        runs,
    })
}

pub fn run_enabled_fixtures(
    sources: &[SourceDefinition],
) -> Result<Vec<SourceRunReport>, TianJiError> {
    let checked_at = current_checked_at();
    Ok(sources
        .iter()
        .filter(|source| source.enabled && source.kind == SourceKind::Fixture)
        .map(|source| run_fixture_source(source, &checked_at))
        .collect())
}

pub fn source_health_inputs_from_runs(runs: &[SourceRunReport]) -> Vec<SourceHealthCheckInput> {
    runs.iter()
        .map(|run| SourceHealthCheckInput {
            source_id: run.source_id.clone(),
            kind: run.kind.as_str().to_string(),
            status: run.status.clone(),
            checked_at: run.checked_at.clone(),
            raw_item_count: run.raw_item_count.and_then(usize_to_i64),
            normalized_event_count: run.normalized_event_count.and_then(usize_to_i64),
            scored_event_count: run.scored_event_count.and_then(usize_to_i64),
            intervention_candidate_count: run.intervention_candidate_count.and_then(usize_to_i64),
            dominant_field: run.dominant_field.clone(),
            risk_level: run.risk_level.clone(),
            error: run.error.clone(),
            run_id: None,
        })
        .collect()
}

fn usize_to_i64(value: usize) -> Option<i64> {
    i64::try_from(value).ok()
}

fn current_checked_at() -> String {
    chrono::Utc::now().to_rfc3339()
}

fn build_source_statuses(
    sources: &[SourceDefinition],
    runs: &[SourceRunReport],
    run_fixtures: bool,
    fetch_live: bool,
    latest_health: &BTreeMap<String, LatestSourceHealth>,
) -> Vec<SourceStatusReport> {
    sources
        .iter()
        .map(|source| {
            let run = runs.iter().find(|run| run.source_id == source.id);
            let runnable = source.enabled
                && matches!(
                    source.kind,
                    SourceKind::Fixture | SourceKind::Rss | SourceKind::Atom
                );
            let status = if !source.enabled {
                "skipped"
            } else if let Some(run) = run {
                match run.status.as_str() {
                    "ok" => "ready",
                    "error" => "error",
                    _ => "skipped",
                }
            } else if !run_fixtures && !fetch_live {
                "ready"
            } else {
                "skipped"
            };
            let persisted = latest_health.get(&source.id);
            SourceStatusReport {
                id: source.id.clone(),
                name: source.name.clone(),
                enabled: source.enabled,
                tier: source.tier.clone(),
                kind: source.kind.clone(),
                path: source.path.clone(),
                url: source.url.clone(),
                tags: source.tags.clone(),
                status: status.to_string(),
                runnable,
                last_success: run
                    .filter(|run| run.status == "ok")
                    .map(|run| run.checked_at.clone())
                    .or_else(|| persisted.and_then(|health| health.last_success.clone())),
                last_error: run
                    .filter(|run| run.status == "error")
                    .map(|run| run.checked_at.clone())
                    .or_else(|| persisted.and_then(|health| health.last_error.clone())),
                last_error_message: run
                    .filter(|run| run.status == "error")
                    .and_then(|run| run.error.clone())
                    .or_else(|| persisted.and_then(|health| health.last_error_message.clone())),
            }
        })
        .collect()
}

fn build_source_runs<F>(
    sources: &[SourceDefinition],
    run_fixtures: bool,
    fetch_live: bool,
    checked_at: &str,
    fetcher: &mut F,
) -> Vec<SourceRunReport>
where
    F: FnMut(&str) -> Result<String, TianJiError>,
{
    if !run_fixtures && !fetch_live {
        return Vec::new();
    }

    sources
        .iter()
        .map(|source| {
            if !source.enabled {
                return skipped_run(source, checked_at, "Source is disabled.");
            }
            match (&source.kind, run_fixtures, fetch_live) {
                (SourceKind::Fixture, true, _) => run_fixture_source(source, checked_at),
                (SourceKind::Fixture, false, true) => {
                    skipped_run(source, checked_at, "Fixture source is not a live source.")
                }
                (SourceKind::Rss | SourceKind::Atom, _, true) => {
                    run_live_source(source, checked_at, fetcher)
                }
                (SourceKind::Rss | SourceKind::Atom, true, false) => {
                    skipped_run(source, checked_at, "Live source requires --fetch-live.")
                }
                _ => skipped_run(source, checked_at, "Source is not selected for this mode."),
            }
        })
        .collect()
}

fn run_fixture_source(source: &SourceDefinition, checked_at: &str) -> SourceRunReport {
    match source.path.as_deref() {
        Some(path) => match run_fixture_path(path, None) {
            Ok(result) => successful_run(source, checked_at, Some(path.to_string()), None, result),
            Err(error) => errored_run(
                source,
                checked_at,
                Some(path.to_string()),
                None,
                error.to_string(),
            ),
        },
        None => errored_run(
            source,
            checked_at,
            None,
            None,
            "Fixture source is missing path.".to_string(),
        ),
    }
}

fn run_live_source<F>(
    source: &SourceDefinition,
    checked_at: &str,
    fetcher: &mut F,
) -> SourceRunReport
where
    F: FnMut(&str) -> Result<String, TianJiError>,
{
    match source.url.as_deref() {
        Some(url) => {
            match fetcher(url).and_then(|feed_text| run_feed_text(&feed_text, url, None)) {
                Ok(result) => {
                    successful_run(source, checked_at, None, Some(url.to_string()), result)
                }
                Err(error) => errored_run(
                    source,
                    checked_at,
                    None,
                    Some(url.to_string()),
                    error.to_string(),
                ),
            }
        }
        None => errored_run(
            source,
            checked_at,
            None,
            None,
            "Live source is missing url.".to_string(),
        ),
    }
}

fn successful_run(
    source: &SourceDefinition,
    checked_at: &str,
    path: Option<String>,
    url: Option<String>,
    result: crate::RunResult,
) -> SourceRunReport {
    SourceRunReport {
        source_id: source.id.clone(),
        kind: source.kind.clone(),
        status: "ok".to_string(),
        checked_at: checked_at.to_string(),
        path,
        url,
        raw_item_count: Some(result.artifact.input_summary.raw_item_count),
        normalized_event_count: Some(result.artifact.input_summary.normalized_event_count),
        scored_event_count: Some(result.artifact.scored_events.len()),
        intervention_candidate_count: Some(result.artifact.intervention_candidates.len()),
        dominant_field: Some(result.artifact.scenario_summary.dominant_field),
        risk_level: Some(result.artifact.scenario_summary.risk_level),
        error: None,
    }
}

fn skipped_run(source: &SourceDefinition, checked_at: &str, reason: &str) -> SourceRunReport {
    SourceRunReport {
        source_id: source.id.clone(),
        kind: source.kind.clone(),
        status: "skipped".to_string(),
        checked_at: checked_at.to_string(),
        path: source.path.clone(),
        url: source.url.clone(),
        raw_item_count: None,
        normalized_event_count: None,
        scored_event_count: None,
        intervention_candidate_count: None,
        dominant_field: None,
        risk_level: None,
        error: Some(reason.to_string()),
    }
}

fn errored_run(
    source: &SourceDefinition,
    checked_at: &str,
    path: Option<String>,
    url: Option<String>,
    error: String,
) -> SourceRunReport {
    SourceRunReport {
        source_id: source.id.clone(),
        kind: source.kind.clone(),
        status: "error".to_string(),
        checked_at: checked_at.to_string(),
        path,
        url,
        raw_item_count: None,
        normalized_event_count: None,
        scored_event_count: None,
        intervention_candidate_count: None,
        dominant_field: None,
        risk_level: None,
        error: Some(error),
    }
}

pub fn fetch_feed_url(source_url: &str) -> Result<String, TianJiError> {
    if !(source_url.starts_with("http://") || source_url.starts_with("https://")) {
        return Err(TianJiError::Usage(
            "Live source URL must be HTTP or HTTPS.".to_string(),
        ));
    }
    let response = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|error| TianJiError::Input(format!("Failed to build feed client: {error}")))?
        .get(source_url)
        .send()
        .map_err(|error| TianJiError::Input(format!("Failed to fetch feed: {error}")))?;
    let status = response.status();
    if !status.is_success() {
        return Err(TianJiError::Input(format!(
            "Failed to fetch feed: HTTP {status}"
        )));
    }
    response
        .text()
        .map_err(|error| TianJiError::Input(format!("Failed to read feed: {error}")))
}

fn require_non_empty(
    value: Option<&str>,
    source_id: &str,
    field: &str,
    kind: &str,
) -> Result<(), TianJiError> {
    if value.is_none_or(|value| value.trim().is_empty()) {
        return Err(TianJiError::Input(format!(
            "Source {source_id} with kind {kind} requires non-empty {field}."
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid_manifest() -> SourceManifest {
        SourceManifest {
            sources: vec![
                SourceDefinition {
                    id: "sample_technology".to_string(),
                    name: "Sample technology fixture".to_string(),
                    enabled: true,
                    tier: "primary".to_string(),
                    kind: SourceKind::Fixture,
                    path: Some("tests/fixtures/sample_feed.xml".to_string()),
                    url: None,
                    tags: vec!["technology".to_string(), "demo".to_string()],
                },
                SourceDefinition {
                    id: "economy_fixture".to_string(),
                    name: "Economy fixture".to_string(),
                    enabled: false,
                    tier: "secondary".to_string(),
                    kind: SourceKind::Fixture,
                    path: Some("tests/fixtures/economy_feed.xml".to_string()),
                    url: None,
                    tags: vec!["economy".to_string()],
                },
                SourceDefinition {
                    id: "disabled_dummy_remote".to_string(),
                    name: "Disabled dummy remote feed".to_string(),
                    enabled: false,
                    tier: "watchlist".to_string(),
                    kind: SourceKind::Rss,
                    path: None,
                    url: Some("https://example.invalid/feed.xml".to_string()),
                    tags: vec!["dummy".to_string()],
                },
            ],
        }
    }

    #[test]
    fn source_registry_valid_load_accepts_example_shape() {
        let manifest = valid_manifest();

        validate_source_manifest(&manifest).expect("valid manifest");
        let report = build_sources_report("examples/sources.example.yaml", manifest, false, false)
            .expect("source report");

        assert_eq!(report.schema_version, SOURCES_REPORT_SCHEMA_VERSION);
        assert_eq!(report.total, 3);
        assert_eq!(report.enabled, 1);
        assert_eq!(report.disabled, 2);
        assert_eq!(report.ready, 1);
        assert_eq!(report.skipped, 2);
        assert_eq!(report.errors, 0);
        assert_eq!(report.tiers["primary"], 1);
        assert!(report.runs.is_empty());
        assert_eq!(report.sources[0].status, "ready");
        assert_eq!(report.sources[1].status, "skipped");
    }

    #[test]
    fn source_registry_rejects_duplicate_ids() {
        let mut manifest = valid_manifest();
        manifest.sources[1].id = manifest.sources[0].id.clone();

        let error = validate_source_manifest(&manifest).expect_err("duplicate rejected");

        assert!(error.to_string().contains("Duplicate source id"));
    }

    #[test]
    fn source_registry_rejects_missing_fixture_path() {
        let mut manifest = valid_manifest();
        manifest.sources[0].path = None;

        let error = validate_source_manifest(&manifest).expect_err("missing path rejected");

        assert!(error.to_string().contains("requires non-empty path"));
    }

    #[test]
    fn source_registry_rejects_empty_identity_fields() {
        let mut manifest = valid_manifest();
        manifest.sources[0].id = " ".to_string();
        assert!(validate_source_manifest(&manifest)
            .expect_err("empty id rejected")
            .to_string()
            .contains("id must be non-empty"));

        let mut manifest = valid_manifest();
        manifest.sources[0].name = " ".to_string();
        assert!(validate_source_manifest(&manifest)
            .expect_err("empty name rejected")
            .to_string()
            .contains("name must be non-empty"));

        let mut manifest = valid_manifest();
        manifest.sources[0].tier = " ".to_string();
        assert!(validate_source_manifest(&manifest)
            .expect_err("empty tier rejected")
            .to_string()
            .contains("tier must be non-empty"));
    }

    #[test]
    fn source_registry_rejects_unknown_kind() {
        let manifest: SourceManifest = serde_yaml::from_str(
            r#"
sources:
  - id: bad_kind
    name: Bad kind
    enabled: true
    tier: primary
    kind: jsonfeed
    url: https://example.invalid/feed.json
"#,
        )
        .expect("unknown kind deserializes for validation");

        let error = validate_source_manifest(&manifest).expect_err("unknown kind rejected");

        assert!(error.to_string().contains("unknown kind"));
    }

    #[test]
    fn source_registry_rejects_missing_rss_or_atom_url() {
        for kind in [SourceKind::Rss, SourceKind::Atom] {
            let mut manifest = valid_manifest();
            manifest.sources[0].kind = kind;
            manifest.sources[0].path = None;
            manifest.sources[0].url = None;

            let error = validate_source_manifest(&manifest).expect_err("missing url rejected");

            assert!(error.to_string().contains("requires non-empty url"));
        }
    }

    #[test]
    fn source_registry_loads_checked_in_example() {
        let manifest = load_source_manifest("examples/sources.example.yaml").expect("example load");
        let report = build_sources_report("examples/sources.example.yaml", manifest, false, false)
            .expect("example report");

        assert_eq!(report.total, 3);
        assert_eq!(report.enabled, 2);
        assert_eq!(report.disabled, 1);
        assert!(report
            .sources
            .iter()
            .any(|source| source.id == "disabled_dummy_remote" && !source.enabled));
    }

    #[test]
    fn source_registry_excludes_disabled_sources_from_fixture_runs() {
        let manifest = valid_manifest();

        let runs = run_enabled_fixtures(&manifest.sources).expect("fixture runs");

        assert_eq!(runs.len(), 1);
        assert_eq!(runs[0].source_id, "sample_technology");
        assert_eq!(runs[0].status, "ok");
        assert_eq!(runs[0].raw_item_count, Some(3));
    }

    #[test]
    fn source_registry_run_fixtures_report_counts_artifacts() {
        let mut manifest = valid_manifest();
        manifest.sources[1].enabled = true;

        let report = build_sources_report("examples/sources.example.yaml", manifest, true, false)
            .expect("source report");
        let runs = report.runs;

        assert_eq!(runs.len(), 3);
        assert_eq!(runs[0].source_id, "sample_technology");
        assert_eq!(runs[0].scored_event_count, Some(3));
        assert_eq!(runs[1].source_id, "economy_fixture");
        assert_eq!(runs[1].raw_item_count, Some(2));
        assert_eq!(runs[2].source_id, "disabled_dummy_remote");
        assert_eq!(runs[2].status, "skipped");
    }

    #[test]
    fn source_registry_default_listing_does_not_fetch_live_sources() {
        let mut manifest = valid_manifest();
        manifest.sources[2].enabled = true;
        let mut fetch_count = 0;

        let report = build_sources_report_with_fetcher(
            "examples/sources.example.yaml",
            manifest,
            false,
            false,
            BTreeMap::new(),
            |_| {
                fetch_count += 1;
                Err(TianJiError::Input("should not fetch".to_string()))
            },
        )
        .expect("source report");

        assert_eq!(fetch_count, 0);
        assert!(report.runs.is_empty());
        assert_eq!(report.ready, 2);
        assert_eq!(report.skipped, 1);
        assert_eq!(report.errors, 0);
    }

    #[test]
    fn source_registry_fetch_live_uses_injected_feed_text() {
        let mut manifest = valid_manifest();
        manifest.sources[0].enabled = false;
        manifest.sources[2].enabled = true;
        let fixture = std::fs::read_to_string("tests/fixtures/sample_feed.xml").expect("fixture");
        let mut fetched_urls = Vec::new();

        let report = build_sources_report_with_fetcher(
            "tests/live.yaml",
            manifest,
            false,
            true,
            BTreeMap::new(),
            |url| {
                fetched_urls.push(url.to_string());
                Ok(fixture.clone())
            },
        )
        .expect("source report");

        assert_eq!(fetched_urls, vec!["https://example.invalid/feed.xml"]);
        assert_eq!(report.runs.len(), 3);
        let live_run = report
            .runs
            .iter()
            .find(|run| run.source_id == "disabled_dummy_remote")
            .expect("live run");
        assert_eq!(live_run.status, "ok");
        assert_eq!(live_run.raw_item_count, Some(3));
        assert_eq!(live_run.dominant_field.as_deref(), Some("technology"));
        let fixture_run = report
            .runs
            .iter()
            .find(|run| run.source_id == "sample_technology")
            .expect("fixture skipped");
        assert_eq!(fixture_run.status, "skipped");
        assert_eq!(report.errors, 0);
    }

    #[test]
    fn source_registry_fetch_live_never_fetches_disabled_sources() {
        let manifest = valid_manifest();
        let mut fetch_count = 0;

        let report = build_sources_report_with_fetcher(
            "tests/live.yaml",
            manifest,
            false,
            true,
            BTreeMap::new(),
            |_| {
                fetch_count += 1;
                Err(TianJiError::Input("disabled fetch attempted".to_string()))
            },
        )
        .expect("source report");

        assert_eq!(fetch_count, 0);
        let disabled = report
            .runs
            .iter()
            .find(|run| run.source_id == "disabled_dummy_remote")
            .expect("disabled run report");
        assert_eq!(disabled.status, "skipped");
    }

    #[test]
    fn source_registry_listing_enriches_persisted_source_health() {
        let mut latest_health = BTreeMap::new();
        latest_health.insert(
            "sample_technology".to_string(),
            LatestSourceHealth {
                source_id: "sample_technology".to_string(),
                latest_status: "error".to_string(),
                latest_checked_at: "2026-06-09T00:00:00+00:00".to_string(),
                last_success: Some("2026-06-08T00:00:00+00:00".to_string()),
                last_error: Some("2026-06-09T00:00:00+00:00".to_string()),
                last_error_message: Some("temporary parse failure".to_string()),
            },
        );

        let report = build_sources_report_with_health(
            "examples/sources.example.yaml",
            valid_manifest(),
            false,
            false,
            latest_health,
        )
        .expect("source report");

        let source = report
            .sources
            .iter()
            .find(|source| source.id == "sample_technology")
            .expect("sample status");
        assert_eq!(
            source.last_success.as_deref(),
            Some("2026-06-08T00:00:00+00:00")
        );
        assert_eq!(
            source.last_error.as_deref(),
            Some("2026-06-09T00:00:00+00:00")
        );
        assert_eq!(
            source.last_error_message.as_deref(),
            Some("temporary parse failure")
        );
    }
}
