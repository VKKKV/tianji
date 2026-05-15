use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::worldline::types::{ActorId, Worldline};
use crate::TianJiError;

use super::agent::{Agent, AgentAction, AgentStatus};
use super::board::{BoardMessage, MessageVisibility, StickEntry};
use super::checkpoint::HongmengCheckpoint;
use super::config::HongmengConfig;
use super::convergence::{check_convergence, ConvergenceReason};
use super::referee::{generate_delta, WorldStateDelta};

/// Current status of a simulation.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SimulationStatus {
    Idle,
    Running,
    Paused { reason: String },
    Converged { reason: ConvergenceReason },
    Failed { error: String },
}

/// The outcome of a completed simulation run.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SimulationOutcome {
    pub simulation_id: String,
    pub total_ticks: u64,
    pub convergence_reason: ConvergenceReason,
    pub final_worldline: Worldline,
    pub agent_histories: BTreeMap<ActorId, Vec<AgentAction>>,
    pub delta_history: Vec<WorldStateDelta>,
}

/// The central Hongmeng orchestration struct — holds all simulation state.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Hongmeng {
    pub agents: BTreeMap<ActorId, Agent>,
    pub board: Vec<BoardMessage>,
    pub sticks: BTreeMap<ActorId, Vec<StickEntry>>,
    pub referee_history: Vec<WorldStateDelta>,
    pub worldline: Worldline,
    pub config: HongmengConfig,
    pub tick: u64,
    pub status: SimulationStatus,
}

impl Hongmeng {
    /// Create a new Hongmeng instance with the given worldline, agents, and config.
    pub fn new(worldline: Worldline, agents: Vec<Agent>, config: HongmengConfig) -> Self {
        let mut agent_map = BTreeMap::new();
        for agent in agents {
            agent_map.insert(agent.actor_id.clone(), agent);
        }

        Self {
            agents: agent_map,
            board: Vec::new(),
            sticks: BTreeMap::new(),
            referee_history: Vec::new(),
            worldline,
            config,
            tick: 0,
            status: SimulationStatus::Idle,
        }
    }

    // -----------------------------------------------------------------------
    // Board / Stick routing
    // -----------------------------------------------------------------------

    /// Broadcast a public message to all agents on the board.
    pub fn broadcast_to_board(&mut self, message: BoardMessage) {
        self.board.push(message);
    }

    /// Send a directed message from one agent to another.
    pub fn send_directed(&mut self, sender: ActorId, target: ActorId, content: String) {
        self.board.push(BoardMessage {
            tick: self.tick,
            sender,
            content,
            visibility: MessageVisibility::Directed(target),
        });
    }

    /// Get all board messages visible to a given viewer.
    /// Public messages are visible to all; Directed messages only to the target.
    pub fn get_visible_board(&self, viewer: &ActorId) -> Vec<&BoardMessage> {
        self.board
            .iter()
            .filter(|msg| match &msg.visibility {
                MessageVisibility::Public => true,
                MessageVisibility::Directed(target) => target == viewer,
            })
            .collect()
    }

    /// Read an agent's stick (private scratch space).
    pub fn get_stick(&self, actor_id: &ActorId) -> &[StickEntry] {
        self.sticks
            .get(actor_id)
            .map(|s| s.as_slice())
            .unwrap_or(&[])
    }

    /// Write a key-value pair to an agent's stick.
    pub fn set_stick(&mut self, actor_id: &ActorId, key: String, value: serde_json::Value) {
        let stick = self.sticks.entry(actor_id.clone()).or_default();
        stick.push(StickEntry {
            tick: self.tick,
            key,
            value,
        });
    }

    // -----------------------------------------------------------------------
    // Simulation loop
    // -----------------------------------------------------------------------

    /// Run the simulation to convergence.
    ///
    /// In Phase 2.4 this is a stub: agents pick random actions from their
    /// `behavior_patterns` instead of calling an LLM.
    pub fn run_simulation(&mut self) -> Result<SimulationOutcome, TianJiError> {
        self.status = SimulationStatus::Running;

        let simulation_id = format!("sim-{}", chrono::Utc::now().timestamp());
        let mut prev_fields = self.worldline.fields.clone();

        loop {
            self.tick += 1;

            // 1. Collect agent actions (stub — no LLM)
            let mut action_types = Vec::new();
            let mut agent_ids = Vec::new();

            for (id, agent) in &mut self.agents {
                agent.status = AgentStatus::Thinking;
                let action = agent.pick_stub_action(self.tick);
                action_types.push(action.action_type.clone());
                agent_ids.push(id.clone());

                // Post action as a board message if it has content
                if let Some(ref msg) = action.board_message {
                    self.board.push(msg.clone());
                }

                agent.action_history.push(action);
                agent.status = AgentStatus::Done;
            }

            // 2. Referee generates WorldStateDelta
            let delta = generate_delta(self.tick, &agent_ids, &action_types);
            self.referee_history.push(delta.clone());

            // 3. Apply field changes to worldline
            for change in &delta.field_changes {
                let key = change.to_field_key();
                let current = self.worldline.fields.get(&key).copied().unwrap_or(0.0);
                self.worldline.fields.insert(key, current + change.delta);
            }

            // 4. Check convergence
            if let Some(reason) = check_convergence(self, &prev_fields, &self.config) {
                self.status = SimulationStatus::Converged {
                    reason: reason.clone(),
                };

                let agent_histories: BTreeMap<ActorId, Vec<AgentAction>> = self
                    .agents
                    .iter()
                    .map(|(id, agent)| (id.clone(), agent.action_history.clone()))
                    .collect();

                return Ok(SimulationOutcome {
                    simulation_id,
                    total_ticks: self.tick,
                    convergence_reason: reason,
                    final_worldline: self.worldline.clone(),
                    agent_histories,
                    delta_history: self.referee_history.clone(),
                });
            }

            // 5. Checkpoint if interval reached
            if self.tick.is_multiple_of(self.config.checkpoint_interval) {
                let agent_states: BTreeMap<ActorId, AgentStatus> = self
                    .agents
                    .iter()
                    .map(|(id, agent)| (id.clone(), agent.status.clone()))
                    .collect();

                let checkpoint = HongmengCheckpoint {
                    simulation_id: simulation_id.clone(),
                    tick: self.tick,
                    worldline_snapshot: self.worldline.clone(),
                    agent_states,
                    board_snapshot: self.board.clone(),
                    created_at: chrono::Utc::now(),
                };

                // For stub purposes, save to in-memory connection if available
                // In production, the caller would provide a connection
                let _ = checkpoint; // Suppress unused warning — real save needs a conn
            }

            prev_fields = self.worldline.fields.clone();

            // Safety: if max_rounds is 0 (unusual but possible), prevent infinite loop
            if self.tick > self.config.max_rounds + 100 {
                self.status = SimulationStatus::Failed {
                    error: "Simulation exceeded safety limit".to_string(),
                };
                return Err(TianJiError::Usage(
                    "Simulation exceeded safety limit (max_rounds + 100)".to_string(),
                ));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hongmeng::board::{BoardMessage, MessageVisibility};
    use crate::profile::types::{ActorProfile, ActorTier, Capabilities};
    use crate::worldline::types::FieldKey;
    use std::collections::BTreeSet;

    fn sample_profile(id: &str, patterns: Vec<&str>) -> ActorProfile {
        ActorProfile {
            id: id.to_string(),
            name: id.to_string(),
            tier: ActorTier::Nation,
            interests: vec![],
            red_lines: vec![],
            capabilities: Capabilities::default(),
            behavior_patterns: patterns.iter().map(|s| s.to_string()).collect(),
            historical_analogues: vec![],
        }
    }

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

    // -----------------------------------------------------------------------
    // Board visibility tests
    // -----------------------------------------------------------------------

    #[test]
    fn board_broadcast_all_agents_see_public() {
        let worldline = sample_worldline();
        let config = HongmengConfig::default();
        let agents = vec![
            Agent::from_profile(sample_profile("usa", vec!["observe"])),
            Agent::from_profile(sample_profile("china", vec!["observe"])),
        ];

        let mut hongmeng = Hongmeng::new(worldline, agents, config);

        hongmeng.broadcast_to_board(BoardMessage {
            tick: 1,
            sender: "usa".to_string(),
            content: "Public announcement".to_string(),
            visibility: MessageVisibility::Public,
        });

        let usa_visible = hongmeng.get_visible_board(&"usa".to_string());
        let china_visible = hongmeng.get_visible_board(&"china".to_string());

        assert_eq!(usa_visible.len(), 1);
        assert_eq!(china_visible.len(), 1);
        assert_eq!(usa_visible[0].content, "Public announcement");
        assert_eq!(china_visible[0].content, "Public announcement");
    }

    #[test]
    fn board_directed_message_only_target_sees() {
        let worldline = sample_worldline();
        let config = HongmengConfig::default();
        let agents = vec![
            Agent::from_profile(sample_profile("usa", vec!["observe"])),
            Agent::from_profile(sample_profile("china", vec!["observe"])),
            Agent::from_profile(sample_profile("iran", vec!["observe"])),
        ];

        let mut hongmeng = Hongmeng::new(worldline, agents, config);

        hongmeng.send_directed(
            "usa".to_string(),
            "china".to_string(),
            "Secret message".to_string(),
        );

        let usa_visible = hongmeng.get_visible_board(&"usa".to_string());
        let china_visible = hongmeng.get_visible_board(&"china".to_string());
        let iran_visible = hongmeng.get_visible_board(&"iran".to_string());

        // Sender (usa) does NOT see their own directed message — only the target does
        assert!(usa_visible.is_empty());
        assert_eq!(china_visible.len(), 1);
        assert_eq!(china_visible[0].content, "Secret message");
        assert!(iran_visible.is_empty());
    }

    #[test]
    fn board_mixed_visibility() {
        let worldline = sample_worldline();
        let config = HongmengConfig::default();
        let agents = vec![
            Agent::from_profile(sample_profile("usa", vec!["observe"])),
            Agent::from_profile(sample_profile("china", vec!["observe"])),
        ];

        let mut hongmeng = Hongmeng::new(worldline, agents, config);

        hongmeng.broadcast_to_board(BoardMessage {
            tick: 1,
            sender: "usa".to_string(),
            content: "Public".to_string(),
            visibility: MessageVisibility::Public,
        });
        hongmeng.send_directed(
            "usa".to_string(),
            "china".to_string(),
            "Private".to_string(),
        );

        let china_visible = hongmeng.get_visible_board(&"china".to_string());
        assert_eq!(china_visible.len(), 2); // public + directed to china

        let usa_visible = hongmeng.get_visible_board(&"usa".to_string());
        assert_eq!(usa_visible.len(), 1); // only public
    }

    // -----------------------------------------------------------------------
    // Stick read/write isolation tests
    // -----------------------------------------------------------------------

    #[test]
    fn stick_read_write_isolation() {
        let worldline = sample_worldline();
        let config = HongmengConfig::default();
        let agents = vec![
            Agent::from_profile(sample_profile("usa", vec!["observe"])),
            Agent::from_profile(sample_profile("china", vec!["observe"])),
        ];

        let mut hongmeng = Hongmeng::new(worldline, agents, config);

        hongmeng.set_stick(
            &"usa".to_string(),
            "threat_assessment".to_string(),
            serde_json::json!({"level": "elevated"}),
        );
        hongmeng.set_stick(
            &"china".to_string(),
            "threat_assessment".to_string(),
            serde_json::json!({"level": "low"}),
        );

        let usa_stick = hongmeng.get_stick(&"usa".to_string());
        assert_eq!(usa_stick.len(), 1);
        assert_eq!(usa_stick[0].key, "threat_assessment");

        let china_stick = hongmeng.get_stick(&"china".to_string());
        assert_eq!(china_stick.len(), 1);
        assert_eq!(china_stick[0].key, "threat_assessment");

        // Verify values are different (isolation)
        assert_ne!(usa_stick[0].value["level"], china_stick[0].value["level"]);

        // Agent without stick entries returns empty
        let iran_stick = hongmeng.get_stick(&"iran".to_string());
        assert!(iran_stick.is_empty());
    }

    #[test]
    fn stick_multiple_entries() {
        let worldline = sample_worldline();
        let config = HongmengConfig::default();
        let agents = vec![Agent::from_profile(sample_profile("usa", vec!["observe"]))];

        let mut hongmeng = Hongmeng::new(worldline, agents, config);

        hongmeng.tick = 1;
        hongmeng.set_stick(
            &"usa".to_string(),
            "key1".to_string(),
            serde_json::json!("value1"),
        );
        hongmeng.tick = 2;
        hongmeng.set_stick(
            &"usa".to_string(),
            "key2".to_string(),
            serde_json::json!("value2"),
        );

        let stick = hongmeng.get_stick(&"usa".to_string());
        assert_eq!(stick.len(), 2);
        assert_eq!(stick[0].tick, 1);
        assert_eq!(stick[1].tick, 2);
    }

    // -----------------------------------------------------------------------
    // SimulationStatus transition tests
    // -----------------------------------------------------------------------

    #[test]
    fn simulation_status_transitions() {
        let worldline = sample_worldline();
        let config = HongmengConfig {
            max_rounds: 3,
            ..Default::default()
        };
        let agents = vec![Agent::from_profile(sample_profile(
            "usa",
            vec!["observe", "diplomatic_protest"],
        ))];

        let mut hongmeng = Hongmeng::new(worldline, agents, config);

        assert_eq!(hongmeng.status, SimulationStatus::Idle);

        let outcome = hongmeng.run_simulation().expect("simulation completes");

        // After simulation, should be converged
        match &hongmeng.status {
            SimulationStatus::Converged { reason } => {
                assert!(matches!(reason, ConvergenceReason::MaxRounds(_)));
            }
            other => panic!("expected Converged, got {other:?}"),
        }

        assert!(outcome.total_ticks <= 3);
    }

    #[test]
    fn simulation_produces_delta_history() {
        let worldline = sample_worldline();
        let config = HongmengConfig {
            max_rounds: 2,
            ..Default::default()
        };
        let agents = vec![Agent::from_profile(sample_profile("usa", vec!["observe"]))];

        let mut hongmeng = Hongmeng::new(worldline, agents, config);
        let outcome = hongmeng.run_simulation().expect("simulation");

        assert!(!outcome.delta_history.is_empty());
        assert_eq!(outcome.delta_history.len() as u64, outcome.total_ticks);
    }

    #[test]
    fn simulation_agent_histories_populated() {
        let worldline = sample_worldline();
        let config = HongmengConfig {
            max_rounds: 2,
            ..Default::default()
        };
        let agents = vec![
            Agent::from_profile(sample_profile("usa", vec!["observe"])),
            Agent::from_profile(sample_profile("china", vec!["diplomatic_protest"])),
        ];

        let mut hongmeng = Hongmeng::new(worldline, agents, config);
        let outcome = hongmeng.run_simulation().expect("simulation");

        assert_eq!(outcome.agent_histories.len(), 2);
        assert!(outcome.agent_histories["usa"].len() > 0);
        assert!(outcome.agent_histories["china"].len() > 0);
    }

    #[test]
    fn simulation_applies_field_changes_to_worldline() {
        let worldline = sample_worldline();
        let initial_conflict = worldline
            .fields
            .get(&FieldKey {
                region: "global".to_string(),
                domain: "conflict".to_string(),
            })
            .copied()
            .unwrap_or(0.0);

        let config = HongmengConfig {
            max_rounds: 2,
            ..Default::default()
        };
        let agents = vec![Agent::from_profile(sample_profile(
            "usa",
            vec!["military_exercise"],
        ))];

        let mut hongmeng = Hongmeng::new(worldline, agents, config);
        let outcome = hongmeng.run_simulation().expect("simulation");

        let final_conflict = outcome
            .final_worldline
            .fields
            .get(&FieldKey {
                region: "global".to_string(),
                domain: "conflict".to_string(),
            })
            .copied()
            .unwrap_or(0.0);

        // military_exercise generates positive conflict delta
        assert!(final_conflict > initial_conflict);
    }
}
