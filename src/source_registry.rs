use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::{run_fixture_path, TianJiError};

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

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct SourcesReport {
    pub schema_version: String,
    pub config: String,
    pub total: usize,
    pub enabled: usize,
    pub disabled: usize,
    pub tiers: BTreeMap<String, usize>,
    pub sources: Vec<SourceDefinition>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub runs: Option<Vec<SourceRunReport>>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct SourceRunReport {
    pub source_id: String,
    pub kind: SourceKind,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
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
) -> Result<SourcesReport, TianJiError> {
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
    let runs = if run_fixtures {
        Some(run_enabled_fixtures(&manifest.sources)?)
    } else {
        None
    };

    Ok(SourcesReport {
        schema_version: SOURCES_REPORT_SCHEMA_VERSION.to_string(),
        config: config_path.to_string(),
        total,
        enabled,
        disabled,
        tiers,
        sources: manifest.sources,
        runs,
    })
}

pub fn run_enabled_fixtures(
    sources: &[SourceDefinition],
) -> Result<Vec<SourceRunReport>, TianJiError> {
    let mut reports = Vec::new();
    let mut first_error = None;

    for source in sources
        .iter()
        .filter(|source| source.enabled && source.kind == SourceKind::Fixture)
    {
        let path = source.path.clone();
        let report = match path.as_deref() {
            Some(path) => match run_fixture_path(path, None) {
                Ok(result) => SourceRunReport {
                    source_id: source.id.clone(),
                    kind: source.kind.clone(),
                    status: "ok".to_string(),
                    path: Some(path.to_string()),
                    raw_item_count: Some(result.artifact.input_summary.raw_item_count),
                    normalized_event_count: Some(
                        result.artifact.input_summary.normalized_event_count,
                    ),
                    scored_event_count: Some(result.artifact.scored_events.len()),
                    intervention_candidate_count: Some(
                        result.artifact.intervention_candidates.len(),
                    ),
                    dominant_field: Some(result.artifact.scenario_summary.dominant_field),
                    risk_level: Some(result.artifact.scenario_summary.risk_level),
                    error: None,
                },
                Err(error) => {
                    if first_error.is_none() {
                        first_error = Some(error.to_string());
                    }
                    SourceRunReport {
                        source_id: source.id.clone(),
                        kind: source.kind.clone(),
                        status: "error".to_string(),
                        path: Some(path.to_string()),
                        raw_item_count: None,
                        normalized_event_count: None,
                        scored_event_count: None,
                        intervention_candidate_count: None,
                        dominant_field: None,
                        risk_level: None,
                        error: first_error.clone(),
                    }
                }
            },
            None => SourceRunReport {
                source_id: source.id.clone(),
                kind: source.kind.clone(),
                status: "error".to_string(),
                path,
                raw_item_count: None,
                normalized_event_count: None,
                scored_event_count: None,
                intervention_candidate_count: None,
                dominant_field: None,
                risk_level: None,
                error: Some("Fixture source is missing path.".to_string()),
            },
        };
        reports.push(report);
    }

    if let Some(error) = first_error {
        return Err(TianJiError::Input(format!(
            "One or more fixture sources failed to run: {error}"
        )));
    }

    Ok(reports)
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
        let report = build_sources_report("examples/sources.example.yaml", manifest, false)
            .expect("source report");

        assert_eq!(report.schema_version, SOURCES_REPORT_SCHEMA_VERSION);
        assert_eq!(report.total, 3);
        assert_eq!(report.enabled, 1);
        assert_eq!(report.disabled, 2);
        assert_eq!(report.tiers["primary"], 1);
        assert!(report.runs.is_none());
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
        let report = build_sources_report("examples/sources.example.yaml", manifest, false)
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

        let report = build_sources_report("examples/sources.example.yaml", manifest, true)
            .expect("source report");
        let runs = report.runs.expect("runs present");

        assert_eq!(runs.len(), 2);
        assert_eq!(runs[0].source_id, "sample_technology");
        assert_eq!(runs[0].scored_event_count, Some(3));
        assert_eq!(runs[1].source_id, "economy_fixture");
        assert_eq!(runs[1].raw_item_count, Some(2));
    }
}
