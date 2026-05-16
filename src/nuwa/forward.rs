use std::collections::BTreeMap;

use crate::hongmeng::agent::{Agent, AgentDecisionContext};
use crate::hongmeng::referee::{generate_delta, FieldChange};
use crate::hongmeng::HongmengConfig;
use crate::llm::ProviderRegistry;
use crate::worldline::types::{FieldKey, Worldline};

use super::outcome::{ConvergenceReason, SimulationOutcome, WorldlineBranch};
use super::sandbox::SimulationMode;

pub async fn run_forward(
    base_worldline: &Worldline,
    agents: &[Agent],
    mode: &SimulationMode,
    config: &HongmengConfig,
    provider: Option<&ProviderRegistry>,
) -> SimulationOutcome {
    let (target_field, horizon_ticks) = match mode {
        SimulationMode::Forward {
            target_field,
            horizon_ticks,
        } => (target_field.clone(), *horizon_ticks),
        _ => {
            return SimulationOutcome {
                mode: mode.clone(),
                branches: vec![],
                intervention_paths: vec![],
                tick_count: 0,
                convergence_reason: ConvergenceReason::MaxTicksReached(0),
            }
        }
    };

    let mut worldline = base_worldline.clone();
    worldline.parent = Some(base_worldline.id);
    worldline.diverge_tick = 0;

    let mut working_agents: Vec<Agent> = agents.to_vec();
    let mut delta_history: Vec<FieldChange> = Vec::new();
    let mut event_sequence: Vec<String> = Vec::new();
    let mut tick: u64 = 0;
    let mut convergence_reason = ConvergenceReason::MaxTicksReached(horizon_ticks);
    let mut _prev_fields = worldline.fields.clone();

    loop {
        tick += 1;

        let mut action_types = Vec::new();
        let mut agent_ids = Vec::new();

        for agent in &mut working_agents {
            let action = if let Some(clients) = resolve_forward_clients(provider) {
                let context = AgentDecisionContext {
                    visible_board: &[],
                    stick: &[],
                    fields: &worldline.fields,
                    recent_actions: &agent.action_history,
                };
                agent
                    .pick_llm_action_with_fallback(tick, &clients, context)
                    .await
                    .unwrap_or_else(|_| agent.pick_stub_action(tick))
            } else {
                agent.pick_stub_action(tick)
            };
            action_types.push(action.action_type.clone());
            agent_ids.push(agent.actor_id.clone());
            agent.action_history.push(action);
        }

        let delta = generate_delta(tick, &agent_ids, &action_types);

        for change in &delta.field_changes {
            let key = change.to_field_key();
            let current = worldline.fields.get(&key).copied().unwrap_or(0.0);
            worldline.fields.insert(key.clone(), current + change.delta);
            delta_history.push(change.clone());

            if change.delta.abs() > 0.01 {
                event_sequence.push(format!(
                    "tick {tick}: {} {} by {:.2}",
                    key.domain,
                    if change.delta > 0.0 {
                        "increased"
                    } else {
                        "decreased"
                    },
                    change.delta.abs()
                ));
            }
        }

        worldline.snapshot_hash = Worldline::compute_snapshot_hash(&worldline.fields);
        worldline.divergence = compute_divergence_from(&base_worldline.fields, &worldline.fields);

        if let Some(target_value) = worldline.fields.get(&target_field) {
            if *target_value >= 10.0 {
                convergence_reason = ConvergenceReason::FieldTargetReached;
                break;
            }
        }

        if tick > 1 {
            let all_stable = delta
                .field_changes
                .iter()
                .all(|c| c.delta.abs() < config.convergence_epsilon);
            if all_stable && !delta.field_changes.is_empty() {
                convergence_reason = ConvergenceReason::FieldStabilized(config.convergence_epsilon);
                break;
            }
        }

        if tick >= horizon_ticks {
            break;
        }

        _prev_fields = worldline.fields.clone();
    }

    let base_probability = 1.0 / (1.0 + worldline.divergence);
    let mut branches = Vec::new();

    branches.push(WorldlineBranch {
        worldline: worldline.clone(),
        probability: base_probability,
        event_sequence: event_sequence.clone(),
        final_divergence: worldline.divergence,
    });

    for offset in 1..3u64 {
        let mut alt_worldline = base_worldline.clone();
        alt_worldline.parent = Some(base_worldline.id);
        alt_worldline.diverge_tick = offset;

        let mut alt_event_sequence = Vec::new();
        for t in 1..=tick.min(horizon_ticks) {
            let tick_offset = t + offset;
            let mut alt_action_types = Vec::new();
            let mut alt_agent_ids = Vec::new();
            for agent in agents {
                let action = agent.pick_stub_action(tick_offset);
                alt_action_types.push(action.action_type.clone());
                alt_agent_ids.push(agent.actor_id.clone());
            }
            let alt_delta = generate_delta(t, &alt_agent_ids, &alt_action_types);
            for change in &alt_delta.field_changes {
                let key = change.to_field_key();
                let current = alt_worldline.fields.get(&key).copied().unwrap_or(0.0);
                alt_worldline
                    .fields
                    .insert(key.clone(), current + change.delta);
                if change.delta.abs() > 0.01 {
                    alt_event_sequence.push(format!(
                        "tick {t}: {} {} by {:.2}",
                        key.domain,
                        if change.delta > 0.0 {
                            "increased"
                        } else {
                            "decreased"
                        },
                        change.delta.abs()
                    ));
                }
            }
        }

        alt_worldline.snapshot_hash = Worldline::compute_snapshot_hash(&alt_worldline.fields);
        alt_worldline.divergence =
            compute_divergence_from(&base_worldline.fields, &alt_worldline.fields);

        let branch_prob = base_probability / (1.0 + offset as f64);

        branches.push(WorldlineBranch {
            worldline: alt_worldline,
            probability: branch_prob,
            event_sequence: alt_event_sequence,
            final_divergence: worldline.divergence + offset as f64 * 0.5,
        });
    }

    branches.sort_by(|a, b| {
        b.probability
            .partial_cmp(&a.probability)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    branches.truncate(3);

    SimulationOutcome {
        mode: mode.clone(),
        branches,
        intervention_paths: vec![],
        tick_count: tick,
        convergence_reason,
    }
}

fn resolve_forward_clients(
    provider: Option<&ProviderRegistry>,
) -> Option<Vec<&crate::llm::client::LlmClient>> {
    let registry = provider?;
    let provider_name = registry
        .agent_model_map()
        .get("forward_default")
        .or_else(|| registry.providers().keys().next())?;
    registry.fallback_chain(provider_name).ok()
}

fn compute_divergence_from(
    base_fields: &BTreeMap<FieldKey, f64>,
    current_fields: &BTreeMap<FieldKey, f64>,
) -> f64 {
    let mut total = 0.0;
    let mut count = 0usize;
    for (key, base_val) in base_fields {
        let current_val = current_fields.get(key).copied().unwrap_or(0.0);
        let diff = (current_val - base_val).abs();
        total += diff * diff;
        count += 1;
    }
    for (key, current_val) in current_fields {
        if !base_fields.contains_key(key) {
            total += current_val * current_val;
            count += 1;
        }
    }
    if count == 0 {
        0.0
    } else {
        total.sqrt()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::profile::types::{ActorProfile, ActorTier, Capabilities};
    use std::collections::BTreeSet;

    fn sample_worldline() -> Worldline {
        let mut fields = BTreeMap::new();
        fields.insert(
            FieldKey {
                region: "global".to_string(),
                domain: "conflict".to_string(),
            },
            3.5,
        );
        let hash = Worldline::compute_snapshot_hash(&fields);
        Worldline {
            id: 1,
            fields,
            events: vec![],
            causal_graph: petgraph::graph::DiGraph::new(),
            active_actors: BTreeSet::new(),
            divergence: 0.0,
            parent: None,
            diverge_tick: 0,
            snapshot_hash: hash,
            created_at: chrono::Utc::now(),
        }
    }

    fn sample_agent(id: &str) -> Agent {
        let profile = ActorProfile {
            id: id.to_string(),
            name: id.to_string(),
            tier: ActorTier::Nation,
            interests: vec![],
            red_lines: vec![],
            capabilities: Capabilities::default(),
            behavior_patterns: vec!["observe".to_string()],
            historical_analogues: vec![],
        };
        Agent::from_profile(profile)
    }

    #[tokio::test]
    async fn forward_simulation_runs_to_convergence() {
        let worldline = sample_worldline();
        let agents = vec![sample_agent("usa")];
        let mode = SimulationMode::Forward {
            target_field: FieldKey {
                region: "global".to_string(),
                domain: "conflict".to_string(),
            },
            horizon_ticks: 5,
        };
        let config = HongmengConfig::default();

        let outcome = run_forward(&worldline, &agents, &mode, &config, None).await;

        assert!(outcome.tick_count > 0);
        assert!(outcome.tick_count <= 5);
        assert!(!outcome.branches.is_empty());
    }

    #[tokio::test]
    async fn forward_produces_branches_with_decreasing_probability() {
        let worldline = sample_worldline();
        let agents = vec![sample_agent("usa")];
        let mode = SimulationMode::Forward {
            target_field: FieldKey {
                region: "global".to_string(),
                domain: "conflict".to_string(),
            },
            horizon_ticks: 3,
        };
        let config = HongmengConfig::default();

        let outcome = run_forward(&worldline, &agents, &mode, &config, None).await;

        assert!(outcome.branches.len() >= 1);
        assert!(outcome.branches.len() <= 3);
        for i in 1..outcome.branches.len() {
            assert!(outcome.branches[i].probability <= outcome.branches[i - 1].probability);
        }
    }

    #[tokio::test]
    async fn forward_with_wrong_mode_returns_empty() {
        let worldline = sample_worldline();
        let agents = vec![sample_agent("usa")];
        let mode = SimulationMode::Backward {
            goal_description: "test".to_string(),
            goal_field_constraints: BTreeMap::new(),
            max_interventions: 3,
        };
        let config = HongmengConfig::default();

        let outcome = run_forward(&worldline, &agents, &mode, &config, None).await;

        assert!(outcome.branches.is_empty());
        assert_eq!(outcome.tick_count, 0);
    }

    #[test]
    fn compute_divergence_identical_fields_is_zero() {
        let fields = BTreeMap::from([(
            FieldKey {
                region: "global".to_string(),
                domain: "conflict".to_string(),
            },
            3.5,
        )]);
        assert!((compute_divergence_from(&fields, &fields)).abs() < f64::EPSILON);
    }

    #[test]
    fn compute_divergence_different_fields_is_positive() {
        let base = BTreeMap::from([(
            FieldKey {
                region: "global".to_string(),
                domain: "conflict".to_string(),
            },
            3.5,
        )]);
        let current = BTreeMap::from([(
            FieldKey {
                region: "global".to_string(),
                domain: "conflict".to_string(),
            },
            5.0,
        )]);
        assert!(compute_divergence_from(&base, &current) > 0.0);
    }
}
