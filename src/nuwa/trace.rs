//! Stable JSONL trace types for Nuwa forward simulations.

use std::collections::BTreeMap;
use std::fs::File;
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::hongmeng::agent::AgentAction;
use crate::hongmeng::referee::FieldChange;
use crate::worldline::types::{ActorId, FieldKey};
use crate::TianJiError;

use super::outcome::SimulationOutcome;

pub const SIM_TRACE_SCHEMA_VERSION: &str = "tianji.sim-trace.v1";
pub const REPLAY_BUNDLE_SCHEMA_VERSION: &str = "tianji.replay-bundle.v1";
pub const REPLAY_BUNDLE_MANIFEST_FILE: &str = "manifest.json";
pub const REPLAY_BUNDLE_TRACE_FILE: &str = "trace.jsonl";
pub const REPLAY_BUNDLE_OUTCOME_FILE: &str = "outcome.json";

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SimulationTrace {
    pub metadata: SimulationTraceMetadata,
    pub frames: Vec<SimulationTraceFrame>,
    pub completed: SimulationTraceCompleted,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SimulationTraceMetadata {
    pub schema_version: String,
    pub mode: String,
    pub target_field: Option<FieldKey>,
    pub horizon_ticks: u64,
    pub frame_count: usize,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SimulationTraceFrame {
    pub tick: u64,
    pub field_values: BTreeMap<FieldKey, f64>,
    pub field_changes: Vec<FieldChange>,
    pub agent_actions: Vec<TraceAgentAction>,
    pub event_sequence_len: usize,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TraceAgentAction {
    pub actor_id: ActorId,
    pub action_type: String,
    pub target: Option<ActorId>,
    pub confidence: f64,
    pub rationale: String,
    pub assessment: String,
    pub category: String,
    pub drivers: Vec<String>,
}

impl TraceAgentAction {
    pub fn from_agent_action(actor_id: ActorId, action: &AgentAction) -> Self {
        Self {
            actor_id,
            action_type: action.action_type.clone(),
            target: action.target.clone(),
            confidence: action.confidence,
            rationale: action.rationale.clone(),
            assessment: action.assessment.clone(),
            category: action.category.clone(),
            drivers: action.drivers.clone(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SimulationTraceCompleted {
    pub outcome: SimulationOutcome,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ReplayBundleManifest {
    pub schema_version: String,
    pub created_at: String,
    pub simulation_id: String,
    pub mode: String,
    pub target_field: Option<FieldKey>,
    pub horizon_ticks: u64,
    pub frame_count: usize,
    pub trace_file: String,
    pub outcome_file: String,
    pub trace_bytes: u64,
    pub outcome_bytes: u64,
}

#[derive(Clone, Debug)]
pub enum SimulationTraceRecord {
    Metadata(SimulationTraceMetadata),
    Frame(SimulationTraceFrame),
    Completed(SimulationTraceCompleted),
}

impl Serialize for SimulationTraceRecord {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let (record_type, payload) = match self {
            Self::Metadata(record) => (
                "metadata",
                serde_json::to_value(record).map_err(serde::ser::Error::custom)?,
            ),
            Self::Frame(record) => (
                "frame",
                serde_json::to_value(record).map_err(serde::ser::Error::custom)?,
            ),
            Self::Completed(record) => (
                "completed",
                serde_json::to_value(record).map_err(serde::ser::Error::custom)?,
            ),
        };
        let mut object = match payload {
            serde_json::Value::Object(object) => object,
            _ => serde_json::Map::new(),
        };
        object.insert(
            "record_type".to_string(),
            serde_json::Value::String(record_type.to_string()),
        );
        serde_json::Value::Object(object).serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for SimulationTraceRecord {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let mut value = serde_json::Value::deserialize(deserializer)?;
        let object = value
            .as_object_mut()
            .ok_or_else(|| serde::de::Error::custom("trace record must be a JSON object"))?;
        let record_type = object
            .remove("record_type")
            .and_then(|value| value.as_str().map(str::to_string))
            .ok_or_else(|| serde::de::Error::custom("trace record missing record_type"))?;

        match record_type.as_str() {
            "metadata" => serde_json::from_value(value)
                .map(Self::Metadata)
                .map_err(serde::de::Error::custom),
            "frame" => serde_json::from_value(value)
                .map(Self::Frame)
                .map_err(serde::de::Error::custom),
            "completed" => serde_json::from_value(value)
                .map(Self::Completed)
                .map_err(serde::de::Error::custom),
            other => Err(serde::de::Error::custom(format!(
                "unknown trace record_type: {other}"
            ))),
        }
    }
}

pub fn write_trace_jsonl(
    path: impl AsRef<Path>,
    trace: &SimulationTrace,
) -> Result<(), TianJiError> {
    if let Some(parent) = path.as_ref().parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)?;
        }
    }

    let file = File::create(path)?;
    let mut writer = BufWriter::new(file);
    write_record(
        &mut writer,
        &SimulationTraceRecord::Metadata(trace.metadata.clone()),
    )?;
    for frame in &trace.frames {
        write_record(&mut writer, &SimulationTraceRecord::Frame(frame.clone()))?;
    }
    write_record(
        &mut writer,
        &SimulationTraceRecord::Completed(trace.completed.clone()),
    )?;
    writer.flush()?;
    Ok(())
}

pub fn read_trace_jsonl(path: impl AsRef<Path>) -> Result<SimulationTrace, TianJiError> {
    let file = File::open(path)?;
    let reader = BufReader::new(file);
    let mut metadata = None;
    let mut frames = Vec::new();
    let mut completed = None;

    for line in reader.lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        match serde_json::from_str::<SimulationTraceRecord>(&line)? {
            SimulationTraceRecord::Metadata(record) => metadata = Some(record),
            SimulationTraceRecord::Frame(record) => frames.push(record),
            SimulationTraceRecord::Completed(record) => completed = Some(record),
        }
    }

    let metadata = metadata.ok_or_else(|| {
        TianJiError::DataIntegrity("trace JSONL missing metadata record".to_string())
    })?;
    let completed = completed.ok_or_else(|| {
        TianJiError::DataIntegrity("trace JSONL missing completed record".to_string())
    })?;

    Ok(SimulationTrace {
        metadata,
        frames,
        completed,
    })
}

pub fn write_replay_bundle_dir(
    dir: impl AsRef<Path>,
    trace: &SimulationTrace,
) -> Result<ReplayBundleManifest, TianJiError> {
    let dir = dir.as_ref();
    std::fs::create_dir_all(dir)?;

    let trace_path = dir.join(REPLAY_BUNDLE_TRACE_FILE);
    let outcome_path = dir.join(REPLAY_BUNDLE_OUTCOME_FILE);
    let manifest_path = dir.join(REPLAY_BUNDLE_MANIFEST_FILE);

    write_trace_jsonl(&trace_path, trace)?;

    let outcome_file = File::create(&outcome_path)?;
    let mut outcome_writer = BufWriter::new(outcome_file);
    serde_json::to_writer_pretty(&mut outcome_writer, &trace.completed.outcome)?;
    outcome_writer.write_all(b"\n")?;
    outcome_writer.flush()?;

    let trace_bytes = std::fs::metadata(&trace_path)?.len();
    let outcome_bytes = std::fs::metadata(&outcome_path)?.len();
    let manifest = ReplayBundleManifest {
        schema_version: REPLAY_BUNDLE_SCHEMA_VERSION.to_string(),
        created_at: chrono::Utc::now().to_rfc3339(),
        simulation_id: local_simulation_id(&trace.metadata),
        mode: trace.metadata.mode.clone(),
        target_field: trace.metadata.target_field.clone(),
        horizon_ticks: trace.metadata.horizon_ticks,
        frame_count: trace.frames.len(),
        trace_file: REPLAY_BUNDLE_TRACE_FILE.to_string(),
        outcome_file: REPLAY_BUNDLE_OUTCOME_FILE.to_string(),
        trace_bytes,
        outcome_bytes,
    };

    let manifest_file = File::create(manifest_path)?;
    let mut manifest_writer = BufWriter::new(manifest_file);
    serde_json::to_writer_pretty(&mut manifest_writer, &manifest)?;
    manifest_writer.write_all(b"\n")?;
    manifest_writer.flush()?;

    Ok(manifest)
}

fn local_simulation_id(metadata: &SimulationTraceMetadata) -> String {
    let field = metadata
        .target_field
        .as_ref()
        .map(|field| format!("{}-{}", field.region, field.domain))
        .unwrap_or_else(|| "all-fields".to_string());
    format!(
        "local-{}-{}-h{}-f{}",
        sanitize_id_part(&metadata.mode),
        sanitize_id_part(&field),
        metadata.horizon_ticks,
        metadata.frame_count
    )
}

fn sanitize_id_part(value: &str) -> String {
    value
        .chars()
        .map(|character| match character {
            'a'..='z' | 'A'..='Z' | '0'..='9' => character.to_ascii_lowercase(),
            _ => '-',
        })
        .collect::<String>()
        .trim_matches('-')
        .to_string()
}

fn write_record<W: Write>(
    writer: &mut W,
    record: &SimulationTraceRecord,
) -> Result<(), TianJiError> {
    serde_json::to_writer(&mut *writer, record)?;
    writer.write_all(b"\n")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::nuwa::outcome::{ConvergenceReason, WorldlineBranch};
    use crate::nuwa::sandbox::SimulationMode;
    use crate::worldline::types::Worldline;
    use std::collections::BTreeSet;

    fn sample_outcome() -> SimulationOutcome {
        let fields = BTreeMap::from([(
            FieldKey {
                region: "global".to_string(),
                domain: "conflict".to_string(),
            },
            4.0,
        )]);
        let worldline = Worldline {
            id: 1,
            snapshot_hash: Worldline::compute_snapshot_hash(&fields),
            fields,
            events: vec![],
            causal_graph: petgraph::graph::DiGraph::new(),
            active_actors: BTreeSet::new(),
            divergence: 0.5,
            parent: None,
            diverge_tick: 0,
            created_at: chrono::Utc::now(),
        };

        SimulationOutcome {
            mode: SimulationMode::Forward {
                target_field: FieldKey {
                    region: "global".to_string(),
                    domain: "conflict".to_string(),
                },
                horizon_ticks: 1,
            },
            branches: vec![WorldlineBranch {
                worldline,
                probability: 1.0,
                event_sequence: vec![],
                final_divergence: 0.5,
            }],
            intervention_paths: vec![],
            tick_count: 1,
            convergence_reason: ConvergenceReason::MaxTicksReached(1),
        }
    }

    fn sample_trace() -> SimulationTrace {
        let outcome = sample_outcome();
        let target_field = FieldKey {
            region: "global".to_string(),
            domain: "conflict".to_string(),
        };
        SimulationTrace {
            metadata: SimulationTraceMetadata {
                schema_version: SIM_TRACE_SCHEMA_VERSION.to_string(),
                mode: "forward".to_string(),
                target_field: Some(target_field.clone()),
                horizon_ticks: 1,
                frame_count: 1,
            },
            frames: vec![SimulationTraceFrame {
                tick: 1,
                field_values: BTreeMap::from([(target_field, 4.0)]),
                field_changes: vec![FieldChange {
                    region: "global".to_string(),
                    domain: "conflict".to_string(),
                    delta: 0.5,
                }],
                agent_actions: vec![TraceAgentAction {
                    actor_id: "stub".to_string(),
                    action_type: "observe".to_string(),
                    target: None,
                    confidence: 0.5,
                    rationale: "stub action (no LLM)".to_string(),
                    assessment: "deterministic stub fallback; no LLM assessment available"
                        .to_string(),
                    category: "stub_fallback".to_string(),
                    drivers: vec!["no_llm_provider".to_string()],
                }],
                event_sequence_len: 1,
            }],
            completed: SimulationTraceCompleted { outcome },
        }
    }

    #[test]
    fn trace_record_serializes_metadata_frame_completed() {
        let trace = sample_trace();
        let metadata = serde_json::to_value(SimulationTraceRecord::Metadata(trace.metadata))
            .expect("metadata json");
        assert_eq!(metadata["record_type"], "metadata");
        assert_eq!(metadata["schema_version"], SIM_TRACE_SCHEMA_VERSION);

        let frame = serde_json::to_value(SimulationTraceRecord::Frame(trace.frames[0].clone()))
            .expect("frame json");
        assert_eq!(frame["record_type"], "frame");
        assert_eq!(frame["tick"], 1);
        assert_eq!(
            frame["agent_actions"][0]["assessment"],
            "deterministic stub fallback; no LLM assessment available"
        );
        assert_eq!(frame["event_sequence_len"], 1);

        let completed = serde_json::to_value(SimulationTraceRecord::Completed(trace.completed))
            .expect("completed json");
        assert_eq!(completed["record_type"], "completed");
        assert_eq!(completed["outcome"]["tick_count"], 1);
    }

    #[test]
    fn trace_jsonl_roundtrips_metadata_and_frame_count() {
        let trace = sample_trace();
        let path = std::env::temp_dir().join(format!(
            "tianji_trace_roundtrip_{}_{}.jsonl",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("time")
                .as_nanos()
        ));

        write_trace_jsonl(&path, &trace).expect("write trace");
        let loaded = read_trace_jsonl(&path).expect("read trace");
        let _ = std::fs::remove_file(&path);

        assert_eq!(loaded.metadata.schema_version, SIM_TRACE_SCHEMA_VERSION);
        assert_eq!(loaded.metadata.frame_count, trace.metadata.frame_count);
        assert_eq!(loaded.frames.len(), trace.frames.len());
    }

    #[test]
    fn bundle_writer_creates_manifest_trace_and_outcome() {
        let trace = sample_trace();
        let dir = std::env::temp_dir().join(format!(
            "tianji_bundle_roundtrip_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("time")
                .as_nanos()
        ));

        let manifest = write_replay_bundle_dir(&dir, &trace).expect("write bundle");
        let manifest_path = dir.join(REPLAY_BUNDLE_MANIFEST_FILE);
        let trace_path = dir.join(&manifest.trace_file);
        let outcome_path = dir.join(&manifest.outcome_file);

        assert_eq!(manifest.schema_version, REPLAY_BUNDLE_SCHEMA_VERSION);
        assert_eq!(manifest.trace_file, REPLAY_BUNDLE_TRACE_FILE);
        assert_eq!(manifest.outcome_file, REPLAY_BUNDLE_OUTCOME_FILE);
        assert_eq!(manifest.frame_count, trace.frames.len());
        assert_eq!(
            manifest.trace_bytes,
            std::fs::metadata(&trace_path).unwrap().len()
        );
        assert_eq!(
            manifest.outcome_bytes,
            std::fs::metadata(&outcome_path).unwrap().len()
        );
        assert!(manifest_path.exists());

        let loaded_trace = read_trace_jsonl(&trace_path).expect("read bundled trace");
        assert_eq!(loaded_trace.frames.len(), trace.frames.len());
        let outcome: SimulationOutcome =
            serde_json::from_reader(File::open(&outcome_path).expect("open bundled outcome"))
                .expect("bundled outcome json");
        assert_eq!(outcome.tick_count, trace.completed.outcome.tick_count);

        let manifest_json: ReplayBundleManifest =
            serde_json::from_reader(File::open(&manifest_path).expect("open bundled manifest"))
                .expect("bundled manifest json");
        assert_eq!(manifest_json, manifest);

        let _ = std::fs::remove_dir_all(&dir);
    }
}
