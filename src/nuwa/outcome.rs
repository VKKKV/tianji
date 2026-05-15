//! Nuwa-specific outcome types for simulation results.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::worldline::types::{ActorId, FieldKey, Worldline};

/// Nuwa-specific convergence reason.
///
/// Separate from `hongmeng::ConvergenceReason` because Nuwa has
/// additional convergence triggers (goal-met, intervention limits).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConvergenceReason {
    /// Maximum ticks reached in forward simulation.
    MaxTicksReached(u64),
    /// Target field reached desired value in forward simulation.
    FieldTargetReached,
    /// All field deltas fell below epsilon in forward simulation.
    FieldStabilized(f64),
    /// Goal constraints met in backward search.
    GoalMet,
    /// Maximum interventions exhausted in backward search.
    MaxInterventionsReached(usize),
    /// No viable paths remain after pruning.
    NoViablePaths,
}

/// The outcome of a Nuwa simulation run.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SimulationOutcome {
    pub mode: super::sandbox::SimulationMode,
    pub branches: Vec<WorldlineBranch>,
    pub intervention_paths: Vec<InterventionPath>,
    pub tick_count: u64,
    pub convergence_reason: ConvergenceReason,
}

/// A branch of a forward simulation, representing one possible future.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WorldlineBranch {
    pub worldline: Worldline,
    pub probability: f64,
    pub event_sequence: Vec<String>,
    pub final_divergence: f64,
}

/// A path of interventions found by backward search.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct InterventionPath {
    pub interventions: Vec<InterventionStep>,
    pub path_score: f64,
    pub final_fields: BTreeMap<FieldKey, f64>,
    pub goal_met: bool,
}

/// A single intervention step in a backward search path.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct InterventionStep {
    pub actor: ActorId,
    pub action: String,
    pub target_field: FieldKey,
    pub expected_impact: f64,
    pub confidence: f64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn convergence_reason_variants_serialize() {
        let reasons = vec![
            ConvergenceReason::MaxTicksReached(42),
            ConvergenceReason::FieldTargetReached,
            ConvergenceReason::FieldStabilized(0.01),
            ConvergenceReason::GoalMet,
            ConvergenceReason::MaxInterventionsReached(5),
            ConvergenceReason::NoViablePaths,
        ];

        for reason in &reasons {
            let json = serde_json::to_string(reason).expect("serialize");
            let de: ConvergenceReason = serde_json::from_str(&json).expect("deserialize");
            assert_eq!(&de, reason);
        }
    }

    #[test]
    fn intervention_step_construction() {
        let step = InterventionStep {
            actor: "usa".to_string(),
            action: "diplomatic_protest".to_string(),
            target_field: FieldKey {
                region: "global".to_string(),
                domain: "diplomacy".to_string(),
            },
            expected_impact: 0.5,
            confidence: 0.7,
        };

        assert_eq!(step.actor, "usa");
        assert_eq!(step.action, "diplomatic_protest");
        assert!((step.expected_impact - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn intervention_path_goal_met_flag() {
        let path = InterventionPath {
            interventions: vec![],
            path_score: 0.8,
            final_fields: BTreeMap::new(),
            goal_met: true,
        };

        assert!(path.goal_met);
        assert!(path.interventions.is_empty());
    }

    #[test]
    fn worldline_branch_construction() {
        let mut fields = BTreeMap::new();
        fields.insert(
            FieldKey {
                region: "global".to_string(),
                domain: "conflict".to_string(),
            },
            5.0,
        );

        let hash = Worldline::compute_snapshot_hash(&fields);
        let worldline = Worldline {
            id: 2,
            fields,
            events: vec![],
            causal_graph: petgraph::graph::DiGraph::new(),
            active_actors: std::collections::BTreeSet::new(),
            divergence: 2.0,
            parent: Some(1),
            diverge_tick: 5,
            snapshot_hash: hash,
            created_at: chrono::Utc::now(),
        };

        let branch = WorldlineBranch {
            worldline,
            probability: 0.6,
            event_sequence: vec!["conflict escalated".to_string()],
            final_divergence: 2.0,
        };

        assert!((branch.probability - 0.6).abs() < f64::EPSILON);
        assert_eq!(branch.event_sequence.len(), 1);
    }
}
