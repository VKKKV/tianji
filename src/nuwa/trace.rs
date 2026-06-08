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
}
