use serde::{Deserialize, Serialize};

use crate::profile::types::ActorProfile;
use crate::worldline::types::ActorId;

use super::board::BoardMessage;

/// Status of an agent in the simulation lifecycle.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AgentStatus {
    Idle,
    Thinking,
    Done,
    Error(String),
}

/// An action taken by an agent during a simulation tick.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AgentAction {
    pub tick: u64,
    pub action_type: String,
    pub target: Option<ActorId>,
    pub board_message: Option<BoardMessage>,
    pub confidence: f64,
    pub rationale: String,
}

/// An agent in the Hongmeng simulation — wraps an ActorProfile with runtime state.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Agent {
    pub actor_id: ActorId,
    pub profile: ActorProfile,
    pub status: AgentStatus,
    pub action_history: Vec<AgentAction>,
    pub private_state: serde_json::Value,
}

impl Agent {
    /// Construct an Agent from an ActorProfile.
    /// The agent starts in Idle status with empty history and null private state.
    pub fn from_profile(profile: ActorProfile) -> Self {
        let actor_id = profile.id.clone();
        Self {
            actor_id,
            profile,
            status: AgentStatus::Idle,
            action_history: Vec::new(),
            private_state: serde_json::Value::Null,
        }
    }

    /// Pick a stub action from the profile's behavior_patterns.
    /// For Phase 2.4, this is a random selection — no real LLM call.
    pub fn pick_stub_action(&self, tick: u64) -> AgentAction {
        let action_type = if self.profile.behavior_patterns.is_empty() {
            "observe".to_string()
        } else {
            // Deterministic selection based on tick to avoid randomness in tests
            let idx = tick as usize % self.profile.behavior_patterns.len();
            self.profile.behavior_patterns[idx].clone()
        };

        AgentAction {
            tick,
            action_type,
            target: None,
            board_message: None,
            confidence: 0.5,
            rationale: "stub action (no LLM)".to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::profile::types::{ActorTier, Capabilities, Interest};

    fn sample_profile(id: &str, patterns: Vec<&str>) -> ActorProfile {
        ActorProfile {
            id: id.to_string(),
            name: id.to_string(),
            tier: ActorTier::Nation,
            interests: vec![Interest {
                goal: "test goal".to_string(),
                salience: 0.5,
            }],
            red_lines: vec![],
            capabilities: Capabilities::default(),
            behavior_patterns: patterns.iter().map(|s| s.to_string()).collect(),
            historical_analogues: vec![],
        }
    }

    #[test]
    fn agent_construction_from_actor_profile() {
        let profile = sample_profile("china", vec!["proportional counter-sanctions"]);
        let agent = Agent::from_profile(profile.clone());

        assert_eq!(agent.actor_id, "china");
        assert_eq!(agent.profile, profile);
        assert_eq!(agent.status, AgentStatus::Idle);
        assert!(agent.action_history.is_empty());
        assert!(agent.private_state.is_null());
    }

    #[test]
    fn agent_pick_stub_action_with_patterns() {
        let profile = sample_profile("usa", vec!["diplomatic_protest", "military_exercise"]);
        let agent = Agent::from_profile(profile);

        let action = agent.pick_stub_action(0);
        assert_eq!(action.tick, 0);
        assert_eq!(action.action_type, "diplomatic_protest");
        assert!(action.target.is_none());
        assert!((action.confidence - 0.5).abs() < f64::EPSILON);

        let action = agent.pick_stub_action(1);
        assert_eq!(action.action_type, "military_exercise");
    }

    #[test]
    fn agent_pick_stub_action_without_patterns() {
        let profile = sample_profile("test", vec![]);
        let agent = Agent::from_profile(profile);

        let action = agent.pick_stub_action(0);
        assert_eq!(action.action_type, "observe");
    }

    #[test]
    fn agent_status_transitions() {
        let profile = sample_profile("iran", vec![]);
        let mut agent = Agent::from_profile(profile);

        assert_eq!(agent.status, AgentStatus::Idle);

        agent.status = AgentStatus::Thinking;
        assert_eq!(agent.status, AgentStatus::Thinking);

        agent.status = AgentStatus::Done;
        assert_eq!(agent.status, AgentStatus::Done);

        agent.status = AgentStatus::Error("timeout".to_string());
        match &agent.status {
            AgentStatus::Error(msg) => assert_eq!(msg, "timeout"),
            _ => panic!("expected Error status"),
        }
    }

    #[test]
    fn agent_action_history_tracking() {
        let profile = sample_profile("nato", vec!["observe"]);
        let mut agent = Agent::from_profile(profile);

        let action = agent.pick_stub_action(1);
        agent.action_history.push(action.clone());

        assert_eq!(agent.action_history.len(), 1);
        assert_eq!(agent.action_history[0].tick, 1);
    }
}
