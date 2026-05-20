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
    pub cases: Vec<EvalCaseReport>,
}

#[derive(Clone, Debug, Serialize)]
pub struct EvalCaseReport {
    pub id: String,
    pub status: EvalStatus,
    pub fixture: String,
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
}

pub fn load_manifest(path: impl AsRef<Path>) -> Result<EvalCorpusManifest, TianJiError> {
    let path = path.as_ref();
    let text = std::fs::read_to_string(path)?;
    serde_yaml::from_str(&text)
        .map_err(|error| TianJiError::Yaml(error, path.display().to_string()))
}

pub fn run_eval_manifest(path: impl AsRef<Path>) -> Result<EvalReport, TianJiError> {
    let path = path.as_ref();
    let manifest = load_manifest(path)?;
    let base_dir = path.parent().unwrap_or_else(|| Path::new("."));
    Ok(evaluate_manifest(
        &manifest,
        path.display().to_string(),
        base_dir,
    ))
}

pub fn evaluate_manifest(
    manifest: &EvalCorpusManifest,
    manifest_path: String,
    base_dir: &Path,
) -> EvalReport {
    let cases: Vec<EvalCaseReport> = manifest
        .cases
        .iter()
        .map(|case| evaluate_case(case, base_dir))
        .collect();
    let passed = cases
        .iter()
        .filter(|case| case.status == EvalStatus::Pass)
        .count();
    let failed = cases.len().saturating_sub(passed);
    EvalReport {
        schema_version: EVAL_REPORT_SCHEMA_VERSION.to_string(),
        manifest: manifest_path,
        case_count: cases.len(),
        passed,
        failed,
        cases,
    }
}

fn evaluate_case(case: &EvalCase, base_dir: &Path) -> EvalCaseReport {
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
                    for score_name in ["impact_score", "field_attraction", "divergence_score"] {
                        let expected = golden["top_scored_event"][score_name].as_f64();
                        let actual_score = top_event(&actual)[score_name].as_f64();
                        if let (Some(expected), Some(actual_score)) = (expected, actual_score) {
                            let delta = (actual_score - expected).abs();
                            max_score_delta = max_score_delta.max(delta);
                            push_check_status(
                                &mut checks,
                                format!("golden.top_event.{score_name}"),
                                if delta <= case.tolerance.score_abs {
                                    EvalStatus::Pass
                                } else {
                                    EvalStatus::Fail
                                },
                                Value::from(expected),
                                Value::from(actual_score),
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
    EvalCaseReport {
        id: case.id.clone(),
        status,
        fixture: case.fixture.clone(),
        checks,
        max_score_delta,
    }
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
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    const CORPUS: &str = "tests/fixtures/eval/corpus.yaml";

    #[test]
    fn eval_manifest_loads_checked_in_corpus() {
        let manifest = load_manifest(CORPUS).expect("load eval corpus");

        assert_eq!(manifest.cases.len(), 1);
        assert_eq!(manifest.cases[0].id, "sample_feed_technology_high");
        assert_eq!(manifest.cases[0].expected.dominant_field, "technology");
    }

    #[test]
    fn eval_checked_in_corpus_passes() {
        let report = run_eval_manifest(CORPUS).expect("eval report");

        assert_eq!(report.schema_version, EVAL_REPORT_SCHEMA_VERSION);
        assert_eq!(report.case_count, 1);
        assert_eq!(report.passed, 1);
        assert_eq!(report.failed, 0);
        assert_eq!(report.cases[0].status, EvalStatus::Pass);
    }

    #[test]
    fn eval_reports_intentional_expectation_mismatch() {
        let mut manifest = load_manifest(CORPUS).expect("load eval corpus");
        manifest.cases[0].expected.risk_level = "low".to_string();

        let report = evaluate_manifest(&manifest, CORPUS.to_string(), Path::new("."));

        assert_eq!(report.passed, 0);
        assert_eq!(report.failed, 1);
        assert_eq!(report.cases[0].status, EvalStatus::Fail);
        assert!(report.cases[0]
            .checks
            .iter()
            .any(|check| check.name == "risk_level" && check.status == EvalStatus::Fail));
    }
}
