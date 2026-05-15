use serde::{Deserialize, Serialize};

use crate::worldline::types::FieldKey;

use super::config::HongmengConfig;
use super::simulation::Hongmeng;

/// Reason the simulation converged (or was stopped).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConvergenceReason {
    /// Maximum rounds reached.
    MaxRounds(u64),
    /// All agents chose the same action for 2 consecutive rounds.
    AgentConsensus,
    /// All field deltas fell below the configured epsilon.
    FieldStabilized(f64),
    /// Token budget exhausted (reserved for future LLM integration).
    TokenBudgetExhausted,
}

/// Check whether the simulation has converged.
///
/// Three convergence triggers:
/// 1. **MaxRounds** — `hongmeng.tick >= config.max_rounds`
/// 2. **AgentConsensus** — all agents chose the same `action_type` in the
///    last two consecutive rounds
/// 3. **FieldStabilized** — all field deltas in the latest referee delta have
///    absolute value < `config.convergence_epsilon`
pub fn check_convergence(
    hongmeng: &Hongmeng,
    prev_fields: &std::collections::BTreeMap<FieldKey, f64>,
    config: &HongmengConfig,
) -> Option<ConvergenceReason> {
    // 1. Max rounds
    if hongmeng.tick >= config.max_rounds {
        return Some(ConvergenceReason::MaxRounds(hongmeng.tick));
    }

    // 2. Agent consensus — all agents same action for last 2 rounds
    if hongmeng.tick >= 2 && !hongmeng.agents.is_empty() {
        let agent_ids: Vec<_> = hongmeng.agents.keys().cloned().collect();

        let mut all_same_last = true;
        let mut all_same_prev = true;

        if let Some(first_id) = agent_ids.first() {
            let first_agent = &hongmeng.agents[first_id];
            let history_len = first_agent.action_history.len();

            if history_len >= 2 {
                let last_action = &first_agent.action_history[history_len - 1].action_type;
                let prev_action = &first_agent.action_history[history_len - 2].action_type;

                for id in &agent_ids {
                    let agent = &hongmeng.agents[id];
                    let h_len = agent.action_history.len();
                    if h_len < 2 {
                        all_same_last = false;
                        all_same_prev = false;
                        break;
                    }
                    if &agent.action_history[h_len - 1].action_type != last_action {
                        all_same_last = false;
                    }
                    if &agent.action_history[h_len - 2].action_type != prev_action {
                        all_same_prev = false;
                    }
                }

                if all_same_last && all_same_prev && last_action == prev_action {
                    return Some(ConvergenceReason::AgentConsensus);
                }
            }
        }
    }

    // 3. Field stabilized — all field deltas < epsilon
    if let Some(latest_delta) = hongmeng.referee_history.last() {
        if !latest_delta.field_changes.is_empty() {
            let all_below_epsilon = latest_delta
                .field_changes
                .iter()
                .all(|c| c.delta.abs() < config.convergence_epsilon);

            if all_below_epsilon {
                // Also compare with previous fields to ensure meaningful stability
                let _ = prev_fields; // Used for future comparison; currently just check deltas
                return Some(ConvergenceReason::FieldStabilized(
                    config.convergence_epsilon,
                ));
            }
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hongmeng::agent::Agent;
    use crate::hongmeng::referee::{FieldChange, WorldStateDelta};
    use crate::profile::types::{ActorProfile, ActorTier, Capabilities};
    use crate::worldline::types::Worldline;
    use std::collections::BTreeMap;
    use std::collections::BTreeSet;

    fn make_test_profile(id: &str) -> ActorProfile {
        ActorProfile {
            id: id.to_string(),
            name: id.to_string(),
            tier: ActorTier::Nation,
            interests: vec![],
            red_lines: vec![],
            capabilities: Capabilities::default(),
            behavior_patterns: vec!["observe".to_string()],
            historical_analogues: vec![],
        }
    }

    fn make_hongmeng_with_tick(tick: u64) -> Hongmeng {
        let profile = make_test_profile("usa");
        let agent = Agent::from_profile(profile);

        let mut agents = BTreeMap::new();
        agents.insert("usa".to_string(), agent);

        let mut fields = BTreeMap::new();
        fields.insert(
            FieldKey {
                region: "global".to_string(),
                domain: "conflict".to_string(),
            },
            3.5,
        );

        let hash = Worldline::compute_snapshot_hash(&fields);
        let worldline = Worldline {
            id: 1,
            fields: fields.clone(),
            events: vec![],
            causal_graph: petgraph::graph::DiGraph::new(),
            active_actors: BTreeSet::new(),
            divergence: 0.0,
            parent: None,
            diverge_tick: 0,
            snapshot_hash: hash,
            created_at: chrono::Utc::now(),
        };

        Hongmeng {
            agents,
            board: vec![],
            sticks: BTreeMap::new(),
            referee_history: vec![],
            worldline,
            config: HongmengConfig::default(),
            tick,
            status: crate::hongmeng::simulation::SimulationStatus::Idle,
        }
    }

    #[test]
    fn convergence_max_rounds_triggered() {
        let mut hongmeng = make_hongmeng_with_tick(10);
        hongmeng.config.max_rounds = 10;

        let prev_fields = BTreeMap::new();
        let result = check_convergence(&hongmeng, &prev_fields, &hongmeng.config);
        assert_eq!(result, Some(ConvergenceReason::MaxRounds(10)));
    }

    #[test]
    fn convergence_max_rounds_not_yet() {
        let hongmeng = make_hongmeng_with_tick(5);
        let config = HongmengConfig {
            max_rounds: 10,
            ..Default::default()
        };

        let prev_fields = BTreeMap::new();
        let result = check_convergence(&hongmeng, &prev_fields, &config);
        assert!(result.is_none());
    }

    #[test]
    fn convergence_field_stabilized_when_delta_below_epsilon() {
        let mut hongmeng = make_hongmeng_with_tick(3);

        hongmeng.referee_history.push(WorldStateDelta {
            tick: 3,
            summary: "Tiny change".to_string(),
            field_changes: vec![FieldChange {
                region: "global".to_string(),
                domain: "conflict".to_string(),
                delta: 0.001,
            }],
            affected_actors: vec!["usa".to_string()],
        });

        let config = HongmengConfig {
            max_rounds: 100,
            convergence_epsilon: 0.01,
            ..Default::default()
        };

        let prev_fields = BTreeMap::new();
        let result = check_convergence(&hongmeng, &prev_fields, &config);
        assert_eq!(result, Some(ConvergenceReason::FieldStabilized(0.01)));
    }

    #[test]
    fn convergence_field_not_stabilized_when_delta_above_epsilon() {
        let mut hongmeng = make_hongmeng_with_tick(3);

        hongmeng.referee_history.push(WorldStateDelta {
            tick: 3,
            summary: "Significant change".to_string(),
            field_changes: vec![FieldChange {
                region: "global".to_string(),
                domain: "conflict".to_string(),
                delta: 0.5,
            }],
            affected_actors: vec!["usa".to_string()],
        });

        let config = HongmengConfig::default();
        let prev_fields = BTreeMap::new();
        let result = check_convergence(&hongmeng, &prev_fields, &config);
        assert!(result.is_none());
    }

    #[test]
    fn convergence_no_delta_history_means_no_field_stabilized() {
        let hongmeng = make_hongmeng_with_tick(1);
        let config = HongmengConfig {
            max_rounds: 100,
            ..Default::default()
        };

        let prev_fields = BTreeMap::new();
        let result = check_convergence(&hongmeng, &prev_fields, &config);
        assert!(result.is_none());
    }
}
