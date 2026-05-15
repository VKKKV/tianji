//! Pruning decision types for simulation branch management.
//!
//! Pruning is currently a stub — actual TUI integration comes later.
//! For forward mode: keep all branches, return top 3 by probability.
//! For backward mode: alpha-beta pruning by path_score.

use serde::{Deserialize, Serialize};

/// Decision returned by a pruning hook during simulation.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PruningDecision {
    /// Continue simulation without pruning.
    Continue,
    /// Prune the specified branch indices.
    Prune(Vec<usize>),
    /// Pause simulation and present options to the user.
    Pause {
        reason: String,
        options: Vec<String>,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pruning_decision_continue() {
        let decision = PruningDecision::Continue;
        assert_eq!(decision, PruningDecision::Continue);
    }

    #[test]
    fn pruning_decision_prune_indices() {
        let decision = PruningDecision::Prune(vec![1, 3]);
        match decision {
            PruningDecision::Prune(indices) => {
                assert_eq!(indices, vec![1, 3]);
            }
            _ => panic!("expected Prune variant"),
        }
    }

    #[test]
    fn pruning_decision_pause_with_options() {
        let decision = PruningDecision::Pause {
            reason: "branch diverged".to_string(),
            options: vec!["continue".to_string(), "restart".to_string()],
        };
        match decision {
            PruningDecision::Pause { reason, options } => {
                assert_eq!(reason, "branch diverged");
                assert_eq!(options.len(), 2);
            }
            _ => panic!("expected Pause variant"),
        }
    }

    #[test]
    fn pruning_decision_serialization_roundtrip() {
        let decisions = vec![
            PruningDecision::Continue,
            PruningDecision::Prune(vec![0, 2]),
            PruningDecision::Pause {
                reason: "test".to_string(),
                options: vec!["a".to_string(), "b".to_string()],
            },
        ];

        for decision in &decisions {
            let json = serde_json::to_string(decision).expect("serialize");
            let de: PruningDecision = serde_json::from_str(&json).expect("deserialize");
            assert_eq!(&de, decision);
        }
    }
}
