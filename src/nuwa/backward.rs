use std::collections::BTreeMap;

use crate::hongmeng::agent::{Agent, AgentDecisionContext};
use crate::hongmeng::referee::generate_delta;
use crate::llm::ProviderRegistry;
use crate::worldline::types::{FieldKey, Worldline};

use super::outcome::{ConvergenceReason, InterventionPath, InterventionStep, SimulationOutcome};
use super::sandbox::SimulationMode;

const W1_GOAL_PROXIMITY: f64 = 0.4;
const W2_PATH_PROBABILITY: f64 = 0.2;
const W3_INTERVENTION_COUNT: f64 = 0.2;
const W4_COLLATERAL: f64 = 0.2;

pub async fn run_backward(
    base_worldline: &Worldline,
    agents: &[Agent],
    mode: &SimulationMode,
    provider: Option<&ProviderRegistry>,
) -> SimulationOutcome {
    let (goal_field_constraints, max_interventions) = match mode {
        SimulationMode::Backward {
            goal_field_constraints,
            max_interventions,
            ..
        } => (goal_field_constraints.clone(), *max_interventions),
        _ => {
            return SimulationOutcome {
                mode: mode.clone(),
                branches: vec![],
                intervention_paths: vec![],
                tick_count: 0,
                convergence_reason: ConvergenceReason::MaxInterventionsReached(0),
            }
        }
    };

    if goal_field_constraints.is_empty() || agents.is_empty() || max_interventions == 0 {
        return SimulationOutcome {
            mode: mode.clone(),
            branches: vec![],
            intervention_paths: vec![],
            tick_count: 0,
            convergence_reason: ConvergenceReason::NoViablePaths,
        };
    }

    let mut paths: Vec<InterventionPath> = Vec::new();

    let mut total_tick: u64 = 0;

    for mut agent in agents.iter().cloned() {
        let mut working_worldline = base_worldline.clone();
        working_worldline.parent = Some(base_worldline.id);
        working_worldline.diverge_tick = 0;

        let mut interventions: Vec<InterventionStep> = Vec::new();
        let mut tick: u64 = 0;
        let mut goal_met = false;
        let mut collateral_sum = 0.0f64;
        let mut _prev_fields = working_worldline.fields.clone();

        for i in 0..max_interventions {
            tick += 1;

            let llm_action = if let Some(clients) = resolve_backward_clients(provider) {
                let context = AgentDecisionContext {
                    visible_board: &[],
                    stick: &[],
                    fields: &working_worldline.fields,
                    recent_actions: &agent.action_history,
                };
                agent
                    .pick_llm_action_with_fallback(tick, &clients, context)
                    .await
                    .ok()
            } else {
                None
            };

            let candidate_actions: Vec<String> = llm_action
                .as_ref()
                .map(|action| vec![action.action_type.clone()])
                .unwrap_or_else(|| coarse_filter_actions(&agent, i));

            let (best_action, target_field, expected_impact) = fine_prune_action(
                &candidate_actions,
                &goal_field_constraints,
                &working_worldline,
            );

            let action = llm_action.unwrap_or_else(|| {
                let mut action = agent.pick_stub_action(tick);
                action.action_type = best_action.clone();
                action
            });
            let delta = generate_delta(
                tick,
                std::slice::from_ref(&agent.actor_id),
                std::slice::from_ref(&best_action),
            );

            for change in &delta.field_changes {
                let key = change.to_field_key();
                let current = working_worldline.fields.get(&key).copied().unwrap_or(0.0);
                working_worldline
                    .fields
                    .insert(key.clone(), current + change.delta);

                let is_goal_field = goal_field_constraints.contains_key(&key);
                if !is_goal_field && change.delta.abs() > 0.01 {
                    collateral_sum += change.delta.abs();
                }
            }

            working_worldline.snapshot_hash =
                Worldline::compute_snapshot_hash(&working_worldline.fields);

            interventions.push(InterventionStep {
                actor: agent.actor_id.clone(),
                action: best_action,
                target_field: target_field.clone(),
                expected_impact,
                confidence: action.confidence,
            });
            agent.action_history.push(action);

            goal_met = check_goal_met(&working_worldline.fields, &goal_field_constraints);

            if goal_met {
                total_tick = total_tick.max(tick);
                break;
            }

            _prev_fields = working_worldline.fields.clone();
            total_tick = total_tick.max(tick);
        }

        let goal_proximity =
            compute_goal_proximity(&working_worldline.fields, &goal_field_constraints);
        let path_probability = 1.0 / (1.0 + working_worldline.divergence);
        let intervention_count = interventions.len() as f64;

        let path_score = W1_GOAL_PROXIMITY * goal_proximity
            + W2_PATH_PROBABILITY * path_probability
            - W3_INTERVENTION_COUNT * intervention_count
            - W4_COLLATERAL * collateral_sum;

        paths.push(InterventionPath {
            interventions,
            path_score,
            final_fields: working_worldline.fields.clone(),
            goal_met,
        });
    }

    paths.sort_by(|a, b| {
        b.path_score
            .partial_cmp(&a.path_score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let convergence_reason = if paths.iter().any(|p| p.goal_met) {
        ConvergenceReason::GoalMet
    } else {
        ConvergenceReason::MaxInterventionsReached(max_interventions)
    };

    SimulationOutcome {
        mode: mode.clone(),
        branches: vec![],
        intervention_paths: paths,
        tick_count: total_tick,
        convergence_reason,
    }
}

fn resolve_backward_clients(
    provider: Option<&ProviderRegistry>,
) -> Option<Vec<&crate::llm::client::LlmClient>> {
    let registry = provider?;
    let provider_name = registry
        .agent_model_map()
        .get("backward_coarse")
        .or_else(|| registry.agent_model_map().get("backward_fine"))
        .or_else(|| registry.providers().keys().next())?;
    registry.fallback_chain(provider_name).ok()
}

fn coarse_filter_actions(agent: &Agent, _intervention_index: usize) -> Vec<String> {
    let patterns = &agent.profile.behavior_patterns;
    if patterns.is_empty() {
        vec!["observe".to_string()]
    } else {
        let count = 3.min(patterns.len());
        patterns.iter().take(count).cloned().collect()
    }
}

fn fine_prune_action(
    candidates: &[String],
    goal_constraints: &BTreeMap<FieldKey, (f64, f64)>,
    worldline: &Worldline,
) -> (String, FieldKey, f64) {
    let goal_key = goal_constraints
        .keys()
        .next()
        .cloned()
        .unwrap_or_else(|| FieldKey {
            region: "global".to_string(),
            domain: "economy".to_string(),
        });

    let mut best_action = candidates
        .first()
        .cloned()
        .unwrap_or_else(|| "observe".to_string());
    let mut best_impact = f64::NEG_INFINITY;

    for action in candidates {
        let impact = estimate_action_impact(action, &goal_key, worldline);
        if impact > best_impact {
            best_impact = impact;
            best_action = action.clone();
        }
    }

    (best_action, goal_key.clone(), best_impact)
}

fn estimate_action_impact(action: &str, target_field: &FieldKey, worldline: &Worldline) -> f64 {
    let current = worldline.fields.get(target_field).copied().unwrap_or(0.0);

    let delta: f64 = match action {
        s if s.contains("military") || s.contains("exercise") => 1.5,
        s if s.contains("sanctions") || s.contains("counter") => 0.8,
        s if s.contains("diplomatic") || s.contains("protest") => 0.5,
        s if s.contains("negotiation") || s.contains("talks") => -0.3,
        _ => 0.1,
    };

    delta.abs() + current.abs() * 0.01
}

fn check_goal_met(
    fields: &BTreeMap<FieldKey, f64>,
    constraints: &BTreeMap<FieldKey, (f64, f64)>,
) -> bool {
    for (key, (min, max)) in constraints {
        let value = fields.get(key).copied().unwrap_or(0.0);
        if value < *min || value > *max {
            return false;
        }
    }
    true
}

fn compute_goal_proximity(
    fields: &BTreeMap<FieldKey, f64>,
    constraints: &BTreeMap<FieldKey, (f64, f64)>,
) -> f64 {
    if constraints.is_empty() {
        return 0.0;
    }

    let mut total_proximity = 0.0;
    let mut count = 0usize;

    for (key, (min, max)) in constraints {
        let value = fields.get(key).copied().unwrap_or(0.0);
        let range = max - min;
        if range.abs() < f64::EPSILON {
            if (value - min).abs() < f64::EPSILON {
                total_proximity += 1.0;
            } else {
                total_proximity += 0.0;
            }
        } else if value >= *min && value <= *max {
            total_proximity += 1.0;
        } else {
            let distance = if value < *min {
                min - value
            } else {
                value - max
            };
            total_proximity += 1.0 / (1.0 + distance / range);
        }
        count += 1;
    }

    total_proximity / count as f64
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
            8.0,
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

    fn sample_agent(id: &str, patterns: Vec<&str>) -> Agent {
        let profile = ActorProfile {
            id: id.to_string(),
            name: id.to_string(),
            tier: ActorTier::Nation,
            interests: vec![],
            red_lines: vec![],
            capabilities: Capabilities::default(),
            behavior_patterns: patterns.iter().map(|s| s.to_string()).collect(),
            historical_analogues: vec![],
        };
        Agent::from_profile(profile)
    }

    #[tokio::test]
    async fn backward_search_with_goal_constraints() {
        let worldline = sample_worldline();
        let agents = vec![sample_agent(
            "usa",
            vec!["negotiation", "diplomatic_protest"],
        )];

        let mut constraints = BTreeMap::new();
        constraints.insert(
            FieldKey {
                region: "global".to_string(),
                domain: "conflict".to_string(),
            },
            (0.0, 100.0),
        );

        let mode = SimulationMode::Backward {
            goal_description: "keep conflict low".to_string(),
            goal_field_constraints: constraints,
            max_interventions: 3,
        };

        let outcome = run_backward(&worldline, &agents, &mode, None).await;

        assert!(!outcome.intervention_paths.is_empty());
        assert!(outcome.intervention_paths.len() <= agents.len());
        assert!(outcome.tick_count > 0);
    }

    #[test]
    fn backward_path_score_calculation() {
        let fields = BTreeMap::from([(
            FieldKey {
                region: "global".to_string(),
                domain: "conflict".to_string(),
            },
            5.0,
        )]);
        let constraints = BTreeMap::from([(
            FieldKey {
                region: "global".to_string(),
                domain: "conflict".to_string(),
            },
            (0.0, 10.0),
        )]);

        let proximity = compute_goal_proximity(&fields, &constraints);
        assert!((proximity - 1.0).abs() < f64::EPSILON);

        let fields_outside = BTreeMap::from([(
            FieldKey {
                region: "global".to_string(),
                domain: "conflict".to_string(),
            },
            15.0,
        )]);
        let proximity_outside = compute_goal_proximity(&fields_outside, &constraints);
        assert!(proximity_outside < 1.0);
        assert!(proximity_outside > 0.0);
    }

    #[tokio::test]
    async fn backward_with_wrong_mode_returns_empty() {
        let worldline = sample_worldline();
        let agents = vec![sample_agent("usa", vec!["observe"])];
        let mode = SimulationMode::Forward {
            target_field: FieldKey {
                region: "global".to_string(),
                domain: "conflict".to_string(),
            },
            horizon_ticks: 10,
        };

        let outcome = run_backward(&worldline, &agents, &mode, None).await;
        assert!(outcome.intervention_paths.is_empty());
        assert_eq!(outcome.tick_count, 0);
    }

    #[tokio::test]
    async fn backward_empty_constraints_returns_no_viable_paths() {
        let worldline = sample_worldline();
        let agents = vec![sample_agent("usa", vec!["observe"])];
        let mode = SimulationMode::Backward {
            goal_description: "empty".to_string(),
            goal_field_constraints: BTreeMap::new(),
            max_interventions: 3,
        };

        let outcome = run_backward(&worldline, &agents, &mode, None).await;
        assert_eq!(outcome.convergence_reason, ConvergenceReason::NoViablePaths);
    }

    #[test]
    fn check_goal_met_when_in_range() {
        let fields = BTreeMap::from([(
            FieldKey {
                region: "global".to_string(),
                domain: "conflict".to_string(),
            },
            5.0,
        )]);
        let constraints = BTreeMap::from([(
            FieldKey {
                region: "global".to_string(),
                domain: "conflict".to_string(),
            },
            (0.0, 10.0),
        )]);
        assert!(check_goal_met(&fields, &constraints));
    }

    #[test]
    fn check_goal_not_met_when_out_of_range() {
        let fields = BTreeMap::from([(
            FieldKey {
                region: "global".to_string(),
                domain: "conflict".to_string(),
            },
            15.0,
        )]);
        let constraints = BTreeMap::from([(
            FieldKey {
                region: "global".to_string(),
                domain: "conflict".to_string(),
            },
            (0.0, 10.0),
        )]);
        assert!(!check_goal_met(&fields, &constraints));
    }

    #[test]
    fn coarse_filter_returns_up_to_three_actions() {
        let agent = sample_agent("usa", vec!["a", "b", "c", "d", "e"]);
        let actions = coarse_filter_actions(&agent, 0);
        assert_eq!(actions.len(), 3);
        assert_eq!(actions[0], "a");
    }

    #[test]
    fn path_score_formula_components() {
        let fields = BTreeMap::from([(
            FieldKey {
                region: "global".to_string(),
                domain: "conflict".to_string(),
            },
            5.0,
        )]);
        let constraints = BTreeMap::from([(
            FieldKey {
                region: "global".to_string(),
                domain: "conflict".to_string(),
            },
            (0.0, 10.0),
        )]);

        let proximity = compute_goal_proximity(&fields, &constraints);
        assert!((proximity - 1.0).abs() < f64::EPSILON);

        let path_probability = 0.8;
        let intervention_count = 2.0f64;
        let collateral = 0.5f64;
        let score = W1_GOAL_PROXIMITY * proximity + W2_PATH_PROBABILITY * path_probability
            - W3_INTERVENTION_COUNT * intervention_count
            - W4_COLLATERAL * collateral;

        let expected = 0.4 * 1.0 + 0.2 * 0.8 - 0.2 * 2.0 - 0.2 * 0.5;
        assert!((score - expected).abs() < f64::EPSILON);
    }
}
