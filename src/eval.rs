use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{artifact_json, run_fixture_path, TianJiError, RUN_ARTIFACT_SCHEMA_VERSION};

pub const EVAL_REPORT_SCHEMA_VERSION: &str = "tianji.eval-report.v1";

#[derive(Clone, Debug, Deserialize)]
pub struct EvalCorpusManifest {
    pub cases: Vec<EvalCase>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct EvalCase {
    pub id: String,
    pub description: String,
    pub fixture: String,
    pub expected: EvalExpected,
    pub tolerance: EvalTolerance,
    pub golden: String,
}

#[derive(Clone, Debug, Deserialize)]
pub struct EvalExpected {
    pub schema_version: String,
    pub mode: String,
    pub raw_item_count: usize,
    pub normalized_event_count: usize,
    pub scored_event_count: usize,
    pub intervention_count: usize,
    pub dominant_field: String,
    pub risk_level: String,
    pub top_event_id: Option<String>,
}

#[derive(Clone, Copy, Debug, Deserialize)]
pub struct EvalTolerance {
    pub score_abs: f64,
}

#[derive(Clone, Debug, Serialize)]
pub struct EvalReport {
    pub schema_version: String,
    pub manifest: String,
    pub case_count: usize,
    pub passed: usize,
    pub failed: usize,
    pub max_score_delta: f64,
    pub updated_golden_paths: Vec<String>,
    pub cases: Vec<EvalCaseReport>,
}

#[derive(Clone, Debug, Serialize)]
pub struct EvalCaseReport {
    pub id: String,
    pub description: String,
    pub status: EvalStatus,
    pub fixture: String,
    pub check_count: usize,
    pub failed_check_count: usize,
    pub checks: Vec<EvalCheck>,
    pub max_score_delta: f64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EvalStatus {
    Pass,
    Fail,
}

#[derive(Clone, Debug, Serialize)]
pub struct EvalCheck {
    pub name: String,
    pub status: EvalStatus,
    pub expected: Value,
    pub actual: Value,
    pub delta: Option<f64>,
    pub tolerance: Option<f64>,
}

#[derive(Clone, Debug, Serialize)]
struct EvalGoldenSnapshot {
    schema_version: String,
    case_id: String,
    fixture: String,
    artifact_schema_version: Value,
    mode: Value,
    raw_item_count: Value,
    normalized_event_count: Value,
    scored_event_count: usize,
    intervention_count: usize,
    scenario_summary: EvalGoldenScenarioSummary,
    top_scored_event: Option<EvalGoldenTopEvent>,
    top_intervention: Option<EvalGoldenTopIntervention>,
}

#[derive(Clone, Debug, Serialize)]
struct EvalGoldenScenarioSummary {
    dominant_field: Value,
    risk_level: Value,
}

#[derive(Clone, Debug, Serialize)]
struct EvalGoldenTopEvent {
    event_id: Value,
    dominant_field: Value,
    impact_score: Value,
    field_attraction: Value,
    divergence_score: Value,
}

#[derive(Clone, Debug, Serialize)]
struct EvalGoldenTopIntervention {
    event_id: Value,
    intervention_type: Value,
    priority: Value,
    target: Value,
}

pub fn load_manifest(path: impl AsRef<Path>) -> Result<EvalCorpusManifest, TianJiError> {
    let path = path.as_ref();
    let text = std::fs::read_to_string(path)?;
    serde_yaml::from_str(&text)
        .map_err(|error| TianJiError::Yaml(error, path.display().to_string()))
}

pub fn run_eval_manifest(
    path: impl AsRef<Path>,
    update_golden: bool,
) -> Result<EvalReport, TianJiError> {
    let path = path.as_ref();
    let manifest = load_manifest(path)?;
    let base_dir = path.parent().unwrap_or_else(|| Path::new("."));
    evaluate_manifest_with_update(
        &manifest,
        path.display().to_string(),
        base_dir,
        update_golden,
    )
}

pub fn evaluate_manifest(
    manifest: &EvalCorpusManifest,
    manifest_path: String,
    base_dir: &Path,
) -> Result<EvalReport, TianJiError> {
    evaluate_manifest_with_update(manifest, manifest_path, base_dir, false)
}

pub fn evaluate_manifest_with_update(
    manifest: &EvalCorpusManifest,
    manifest_path: String,
    base_dir: &Path,
    update_golden: bool,
) -> Result<EvalReport, TianJiError> {
    let mut updated_golden_paths = Vec::new();
    let mut cases = Vec::with_capacity(manifest.cases.len());
    for case in &manifest.cases {
        cases.push(evaluate_case(
            case,
            base_dir,
            update_golden,
            &mut updated_golden_paths,
        )?);
    }
    let passed = cases
        .iter()
        .filter(|case| case.status == EvalStatus::Pass)
        .count();
    let failed = cases.len().saturating_sub(passed);
    let max_score_delta = cases
        .iter()
        .map(|case| case.max_score_delta)
        .fold(0.0_f64, f64::max);
    Ok(EvalReport {
        schema_version: EVAL_REPORT_SCHEMA_VERSION.to_string(),
        manifest: manifest_path,
        case_count: cases.len(),
        passed,
        failed,
        max_score_delta,
        updated_golden_paths,
        cases,
    })
}

fn evaluate_case(
    case: &EvalCase,
    base_dir: &Path,
    update_golden: bool,
    updated_golden_paths: &mut Vec<String>,
) -> Result<EvalCaseReport, TianJiError> {
    let mut checks = Vec::new();
    let mut max_score_delta = 0.0_f64;
    let fixture_path = resolve_manifest_path(base_dir, &case.fixture);
    let golden_path = resolve_manifest_path(base_dir, &case.golden);

    match run_fixture_path(&fixture_path, None).and_then(|result| {
        let text = artifact_json(&result.artifact)?;
        serde_json::from_str::<Value>(&text).map_err(TianJiError::Json)
    }) {
        Ok(actual) => {
            push_check(
                &mut checks,
                "schema_version",
                Value::String(case.expected.schema_version.clone()),
                actual["schema_version"].clone(),
            );
            push_check(
                &mut checks,
                "mode",
                Value::String(case.expected.mode.clone()),
                actual["mode"].clone(),
            );
            push_check(
                &mut checks,
                "raw_item_count",
                Value::from(case.expected.raw_item_count),
                actual["input_summary"]["raw_item_count"].clone(),
            );
            push_check(
                &mut checks,
                "normalized_event_count",
                Value::from(case.expected.normalized_event_count),
                actual["input_summary"]["normalized_event_count"].clone(),
            );
            push_check(
                &mut checks,
                "scored_event_count",
                Value::from(case.expected.scored_event_count),
                Value::from(array_len(&actual["scored_events"])),
            );
            push_check(
                &mut checks,
                "intervention_count",
                Value::from(case.expected.intervention_count),
                Value::from(array_len(&actual["intervention_candidates"])),
            );
            push_check(
                &mut checks,
                "dominant_field",
                Value::String(case.expected.dominant_field.clone()),
                actual["scenario_summary"]["dominant_field"].clone(),
            );
            push_check(
                &mut checks,
                "risk_level",
                Value::String(case.expected.risk_level.clone()),
                actual["scenario_summary"]["risk_level"].clone(),
            );
            if let Some(top_event_id) = &case.expected.top_event_id {
                push_check(
                    &mut checks,
                    "top_event_id",
                    Value::String(top_event_id.clone()),
                    top_event(&actual)["event_id"].clone(),
                );
            }

            if update_golden {
                write_golden_snapshot(case, &actual, &golden_path)?;
                updated_golden_paths.push(case.golden.clone());
            }

            match std::fs::read_to_string(&golden_path)
                .map_err(TianJiError::Io)
                .and_then(|text| serde_json::from_str::<Value>(&text).map_err(TianJiError::Json))
            {
                Ok(golden) => {
                    push_check(
                        &mut checks,
                        "golden.top_event.dominant_field",
                        golden["top_scored_event"]["dominant_field"].clone(),
                        top_event(&actual)["dominant_field"].clone(),
                    );
                    push_check(
                        &mut checks,
                        "golden.top_event.event_id",
                        golden["top_scored_event"]["event_id"].clone(),
                        top_event(&actual)["event_id"].clone(),
                    );
                    for score_name in ["impact_score", "field_attraction", "divergence_score"] {
                        let expected = golden["top_scored_event"][score_name].as_f64();
                        let actual_score = top_event(&actual)[score_name].as_f64();
                        if let (Some(expected), Some(actual_score)) = (expected, actual_score) {
                            let delta = (actual_score - expected).abs();
                            max_score_delta = max_score_delta.max(delta);
                            push_check_status_with_delta(
                                &mut checks,
                                format!("golden.top_event.{score_name}"),
                                if delta <= case.tolerance.score_abs {
                                    EvalStatus::Pass
                                } else {
                                    EvalStatus::Fail
                                },
                                Value::from(expected),
                                Value::from(actual_score),
                                Some(delta),
                                Some(case.tolerance.score_abs),
                            );
                        } else {
                            push_check_status(
                                &mut checks,
                                format!("golden.top_event.{score_name}"),
                                EvalStatus::Fail,
                                golden["top_scored_event"][score_name].clone(),
                                top_event(&actual)[score_name].clone(),
                            );
                        }
                    }
                    if golden
                        .get("top_intervention")
                        .is_some_and(|value| !value.is_null())
                    {
                        push_check(
                            &mut checks,
                            "golden.top_intervention.event_id",
                            golden["top_intervention"]["event_id"].clone(),
                            top_intervention(&actual)["event_id"].clone(),
                        );
                        push_check(
                            &mut checks,
                            "golden.top_intervention.intervention_type",
                            golden["top_intervention"]["intervention_type"].clone(),
                            top_intervention(&actual)["intervention_type"].clone(),
                        );
                        push_check(
                            &mut checks,
                            "golden.top_intervention.priority",
                            golden["top_intervention"]["priority"].clone(),
                            top_intervention(&actual)["priority"].clone(),
                        );
                    }
                }
                Err(error) => push_check_status(
                    &mut checks,
                    "golden.load".to_string(),
                    EvalStatus::Fail,
                    Value::String(case.golden.clone()),
                    Value::String(error.to_string()),
                ),
            }
        }
        Err(error) => push_check_status(
            &mut checks,
            "fixture.run".to_string(),
            EvalStatus::Fail,
            Value::String(RUN_ARTIFACT_SCHEMA_VERSION.to_string()),
            Value::String(error.to_string()),
        ),
    }

    let status = if checks.iter().all(|check| check.status == EvalStatus::Pass) {
        EvalStatus::Pass
    } else {
        EvalStatus::Fail
    };
    let failed_check_count = checks
        .iter()
        .filter(|check| check.status == EvalStatus::Fail)
        .count();
    Ok(EvalCaseReport {
        id: case.id.clone(),
        description: case.description.clone(),
        status,
        fixture: case.fixture.clone(),
        check_count: checks.len(),
        failed_check_count,
        checks,
        max_score_delta,
    })
}

fn resolve_manifest_path(base_dir: &Path, raw_path: &str) -> PathBuf {
    let path = Path::new(raw_path);
    if path.is_absolute() || path.exists() {
        path.to_path_buf()
    } else {
        base_dir.join(path)
    }
}

fn array_len(value: &Value) -> usize {
    value.as_array().map(Vec::len).unwrap_or_default()
}

fn top_event(value: &Value) -> &Value {
    value["scored_events"]
        .as_array()
        .and_then(|items| items.first())
        .unwrap_or(&Value::Null)
}

fn top_intervention(value: &Value) -> &Value {
    value["intervention_candidates"]
        .as_array()
        .and_then(|items| items.first())
        .unwrap_or(&Value::Null)
}

fn write_golden_snapshot(
    case: &EvalCase,
    actual: &Value,
    golden_path: &Path,
) -> Result<(), TianJiError> {
    let top_event = top_event(actual);
    let top_intervention = top_intervention(actual);
    let snapshot = EvalGoldenSnapshot {
        schema_version: "tianji.eval-golden.v1".to_string(),
        case_id: case.id.clone(),
        fixture: case.fixture.clone(),
        artifact_schema_version: actual["schema_version"].clone(),
        mode: actual["mode"].clone(),
        raw_item_count: actual["input_summary"]["raw_item_count"].clone(),
        normalized_event_count: actual["input_summary"]["normalized_event_count"].clone(),
        scored_event_count: array_len(&actual["scored_events"]),
        intervention_count: array_len(&actual["intervention_candidates"]),
        scenario_summary: EvalGoldenScenarioSummary {
            dominant_field: actual["scenario_summary"]["dominant_field"].clone(),
            risk_level: actual["scenario_summary"]["risk_level"].clone(),
        },
        top_scored_event: top_event.as_object().map(|_| EvalGoldenTopEvent {
            event_id: top_event["event_id"].clone(),
            dominant_field: top_event["dominant_field"].clone(),
            impact_score: top_event["impact_score"].clone(),
            field_attraction: top_event["field_attraction"].clone(),
            divergence_score: top_event["divergence_score"].clone(),
        }),
        top_intervention: top_intervention
            .as_object()
            .map(|_| EvalGoldenTopIntervention {
                event_id: top_intervention["event_id"].clone(),
                intervention_type: top_intervention["intervention_type"].clone(),
                priority: top_intervention["priority"].clone(),
                target: top_intervention["target"].clone(),
            }),
    };
    if let Some(parent) = golden_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let text = serde_json::to_string_pretty(&snapshot)?;
    std::fs::write(golden_path, format!("{text}\n"))?;
    Ok(())
}

fn push_check(checks: &mut Vec<EvalCheck>, name: &str, expected: Value, actual: Value) {
    let status = if expected == actual {
        EvalStatus::Pass
    } else {
        EvalStatus::Fail
    };
    push_check_status(checks, name.to_string(), status, expected, actual);
}

fn push_check_status(
    checks: &mut Vec<EvalCheck>,
    name: String,
    status: EvalStatus,
    expected: Value,
    actual: Value,
) {
    checks.push(EvalCheck {
        name,
        status,
        expected,
        actual,
        delta: None,
        tolerance: None,
    });
}

fn push_check_status_with_delta(
    checks: &mut Vec<EvalCheck>,
    name: String,
    status: EvalStatus,
    expected: Value,
    actual: Value,
    delta: Option<f64>,
    tolerance: Option<f64>,
) {
    checks.push(EvalCheck {
        name,
        status,
        expected,
        actual,
        delta,
        tolerance,
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    const CORPUS: &str = "tests/fixtures/eval/corpus.yaml";

    #[test]
    fn eval_manifest_loads_checked_in_corpus() {
        let manifest = load_manifest(CORPUS).expect("load eval corpus");

        assert_eq!(manifest.cases.len(), 2);
        assert_eq!(manifest.cases[0].id, "sample_feed_technology_high");
        assert_eq!(manifest.cases[0].expected.dominant_field, "technology");
    }

    #[test]
    fn eval_checked_in_corpus_passes() {
        let report = run_eval_manifest(CORPUS, false).expect("eval report");

        assert_eq!(report.schema_version, EVAL_REPORT_SCHEMA_VERSION);
        assert_eq!(report.case_count, 2);
        assert_eq!(report.passed, 2);
        assert_eq!(report.failed, 0);
        assert!(report.updated_golden_paths.is_empty());
        assert!(report
            .cases
            .iter()
            .all(|case| case.status == EvalStatus::Pass));
        assert!(report.cases.iter().all(|case| case.check_count > 0));
        assert!(report.cases.iter().all(|case| case.failed_check_count == 0));
    }

    #[test]
    fn eval_reports_intentional_expectation_mismatch() {
        let mut manifest = load_manifest(CORPUS).expect("load eval corpus");
        manifest.cases.truncate(1);
        manifest.cases[0].expected.risk_level = "low".to_string();

        let report =
            evaluate_manifest(&manifest, CORPUS.to_string(), Path::new(".")).expect("eval report");

        assert_eq!(report.passed, 0);
        assert_eq!(report.failed, 1);
        assert_eq!(report.cases[0].status, EvalStatus::Fail);
        assert!(report.cases[0]
            .checks
            .iter()
            .any(|check| check.name == "risk_level" && check.status == EvalStatus::Fail));
    }

    #[test]
    fn eval_reports_numeric_drift_with_delta_and_tolerance() {
        let mut manifest = load_manifest(CORPUS).expect("load eval corpus");
        manifest.cases.truncate(1);
        manifest.cases[0].tolerance.score_abs = 0.0;
        let golden_path = resolve_manifest_path(Path::new("."), &manifest.cases[0].golden);
        let original = std::fs::read_to_string(&golden_path).expect("golden");
        let mut golden: Value = serde_json::from_str(&original).expect("golden json");
        golden["top_scored_event"]["impact_score"] = Value::from(999.0);
        let temp_golden = std::env::temp_dir().join(format!(
            "tianji_eval_drift_{}_{}.json",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("time")
                .as_nanos()
        ));
        std::fs::write(
            &temp_golden,
            serde_json::to_string_pretty(&golden).expect("json"),
        )
        .expect("write temp golden");
        manifest.cases[0].golden = temp_golden.to_string_lossy().to_string();

        let report =
            evaluate_manifest(&manifest, CORPUS.to_string(), Path::new(".")).expect("eval report");

        assert_eq!(report.failed, 1);
        let check = report.cases[0]
            .checks
            .iter()
            .find(|check| check.name == "golden.top_event.impact_score")
            .expect("impact score check");
        assert_eq!(check.status, EvalStatus::Fail);
        assert!(check.delta.expect("delta") > 0.0);
        assert_eq!(check.tolerance, Some(0.0));
        let _ = std::fs::remove_file(temp_golden);
    }

    #[test]
    fn eval_update_golden_writes_only_manifest_golden_path() {
        let mut manifest = load_manifest(CORPUS).expect("load eval corpus");
        let temp_golden = std::env::temp_dir().join(format!(
            "tianji_eval_update_{}_{}.json",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("time")
                .as_nanos()
        ));
        manifest.cases.truncate(1);
        manifest.cases[0].golden = temp_golden.to_string_lossy().to_string();

        let report =
            evaluate_manifest_with_update(&manifest, CORPUS.to_string(), Path::new("."), true)
                .expect("eval report");

        assert_eq!(report.failed, 0);
        assert_eq!(
            report.updated_golden_paths,
            vec![manifest.cases[0].golden.clone()]
        );
        let written = std::fs::read_to_string(&temp_golden).expect("written golden");
        assert!(written.contains("sample_feed_technology_high"));
        let _ = std::fs::remove_file(temp_golden);
    }
}
