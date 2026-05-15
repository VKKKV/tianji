use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::hongmeng::agent::Agent;
use crate::hongmeng::HongmengConfig;
use crate::llm::ProviderRegistry;
use crate::worldline::types::{FieldKey, Worldline, WorldlineId};

use super::outcome::SimulationOutcome;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SimulationMode {
    Forward {
        target_field: FieldKey,
        horizon_ticks: u64,
    },
    Backward {
        goal_description: String,
        goal_field_constraints: BTreeMap<FieldKey, (f64, f64)>,
        max_interventions: usize,
    },
}

pub struct NuwaSandbox {
    pub id: String,
    pub base_worldline: Worldline,
    pub forked_worldline: Worldline,
    pub agents: Vec<Agent>,
    pub provider: ProviderRegistry,
    pub mode: SimulationMode,
    pub config: HongmengConfig,
    pub outcome: Option<SimulationOutcome>,
}

impl NuwaSandbox {
    pub fn new(
        base_worldline: Worldline,
        agents: Vec<Agent>,
        provider: ProviderRegistry,
        mode: SimulationMode,
        config: HongmengConfig,
    ) -> Self {
        let id = format!("nuwa-{}", chrono::Utc::now().timestamp());
        let forked_worldline = fork_worldline(&base_worldline);
        Self {
            id,
            base_worldline,
            forked_worldline,
            agents,
            provider,
            mode,
            config,
            outcome: None,
        }
    }
}

fn fork_worldline(base: &Worldline) -> Worldline {
    static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
    let new_id: WorldlineId = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let mut forked = base.clone();
    forked.id = new_id;
    forked.parent = Some(base.id);
    forked.diverge_tick = 0;
    forked.snapshot_hash = Worldline::compute_snapshot_hash(&forked.fields);
    forked.created_at = chrono::Utc::now();
    forked
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::llm::config::{ProviderConfig, ProviderType, TianJiConfig};
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
            id: 100,
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

    fn sample_provider() -> ProviderRegistry {
        let mut providers = BTreeMap::new();
        providers.insert(
            "ollama_local".to_string(),
            ProviderConfig {
                provider_type: ProviderType::Ollama,
                model: "qwen3:14b".to_string(),
                base_url: Some("http://localhost:11434".to_string()),
                api_key_env: None,
                api_key: None,
                max_concurrency: 3,
                fallback: None,
            },
        );
        let config = TianJiConfig {
            providers,
            agent_model_map: BTreeMap::new(),
        };
        ProviderRegistry::from_config(config).expect("registry")
    }

    fn sample_agent(id: &str) -> Agent {
        let profile = ActorProfile {
            id: id.to_string(),
            name: id.to_string(),
            tier: ActorTier::Nation,
            interests: vec![],
            red_lines: vec![],
            capabilities: Capabilities::default(),
            behavior_patterns: vec!["observe".to_string(), "diplomatic_protest".to_string()],
            historical_analogues: vec![],
        };
        Agent::from_profile(profile)
    }

    #[test]
    fn fork_has_new_id_and_parent_set() {
        let base = sample_worldline();
        let forked = fork_worldline(&base);

        assert_ne!(forked.id, base.id);
        assert_eq!(forked.parent, Some(base.id));
        assert_eq!(forked.diverge_tick, 0);
    }

    #[test]
    fn fork_modifying_does_not_affect_base() {
        let base = sample_worldline();
        let mut forked = fork_worldline(&base);

        let key = FieldKey {
            region: "global".to_string(),
            domain: "conflict".to_string(),
        };
        forked.fields.insert(key.clone(), 99.0);

        assert_eq!(base.fields.get(&key).copied(), Some(3.5));
        assert_eq!(forked.fields.get(&key).copied(), Some(99.0));
    }

    #[test]
    fn sandbox_new_creates_valid_instance() {
        let worldline = sample_worldline();
        let provider = sample_provider();
        let agents = vec![sample_agent("usa")];
        let mode = SimulationMode::Forward {
            target_field: FieldKey {
                region: "global".to_string(),
                domain: "conflict".to_string(),
            },
            horizon_ticks: 10,
        };
        let config = HongmengConfig::default();

        let sandbox = NuwaSandbox::new(worldline, agents, provider, mode, config);

        assert!(sandbox.id.starts_with("nuwa-"));
        assert_eq!(sandbox.base_worldline.id, 100);
        assert_ne!(sandbox.forked_worldline.id, 100);
        assert!(sandbox.outcome.is_none());
        assert_eq!(sandbox.agents.len(), 1);
    }

    #[test]
    fn simulation_mode_forward_serialization() {
        let mode = SimulationMode::Forward {
            target_field: FieldKey {
                region: "global".to_string(),
                domain: "conflict".to_string(),
            },
            horizon_ticks: 20,
        };
        let json = serde_json::to_string(&mode).expect("serialize");
        let de: SimulationMode = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(de, mode);
    }

    #[test]
    fn simulation_mode_backward_serialization() {
        let mut constraints = BTreeMap::new();
        constraints.insert(
            FieldKey {
                region: "global".to_string(),
                domain: "conflict".to_string(),
            },
            (0.0, 2.0),
        );
        let mode = SimulationMode::Backward {
            goal_description: "reduce conflict".to_string(),
            goal_field_constraints: constraints,
            max_interventions: 5,
        };
        let json = serde_json::to_string(&mode).expect("serialize");
        let de: SimulationMode = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(de, mode);
    }
}
