use std::collections::BTreeMap;

use serde::{Deserialize, Deserializer, Serialize};
use serde_json::json;

use crate::llm::client::{ChatMessage, LlmClient};
use crate::llm::error::LlmError;
use crate::models::AgentPrivateState;
use crate::profile::types::ActorProfile;
use crate::worldline::types::{ActorId, FieldKey};

use super::board::{BoardMessage, MessageVisibility, StickEntry};

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
    #[serde(default)]
    pub assessment: String,
    #[serde(default = "default_action_category")]
    pub category: String,
    #[serde(default)]
    pub drivers: Vec<String>,
}

/// Visible simulation context passed to an LLM-backed agent decision.
#[derive(Clone, Debug)]
pub struct AgentDecisionContext<'a> {
    pub visible_board: &'a [&'a BoardMessage],
    pub stick: &'a [StickEntry],
    pub fields: &'a BTreeMap<FieldKey, f64>,
    pub recent_actions: &'a [AgentAction],
}

#[derive(Debug, Deserialize)]
struct LlmActionEnvelope {
    action_type: Option<String>,
    target: Option<String>,
    board_message: Option<String>,
    confidence: Option<f64>,
    rationale: Option<String>,
    assessment: Option<String>,
    category: Option<String>,
    drivers: Option<Vec<String>>,
}

fn default_action_category() -> String {
    "uncategorized".to_string()
}

/// An agent in the Hongmeng simulation — wraps an ActorProfile with runtime state.
#[derive(Clone, Debug, Serialize)]
pub struct Agent {
    pub actor_id: ActorId,
    pub profile: ActorProfile,
    pub status: AgentStatus,
    pub action_history: Vec<AgentAction>,
    pub private_state_typed: AgentPrivateState,
}

impl<'de> Deserialize<'de> for Agent {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct AgentCompat {
            actor_id: ActorId,
            profile: ActorProfile,
            status: AgentStatus,
            action_history: Vec<AgentAction>,
            #[serde(default)]
            private_state_typed: Option<AgentPrivateState>,
            #[serde(default)]
            private_state: Option<serde_json::Value>,
        }

        let compat = AgentCompat::deserialize(deserializer)?;
        let private_state_typed = compat
            .private_state_typed
            .or_else(|| {
                compat
                    .private_state
                    .map(agent_private_state_from_json_lossy)
            })
            .unwrap_or_default();

        Ok(Self {
            actor_id: compat.actor_id,
            profile: compat.profile,
            status: compat.status,
            action_history: compat.action_history,
            private_state_typed,
        })
    }
}

fn agent_private_state_from_json_lossy(value: serde_json::Value) -> AgentPrivateState {
    let Some(object) = value.as_object() else {
        return AgentPrivateState::default();
    };

    let objectives = object
        .get("objectives")
        .and_then(|value| value.as_array())
        .filter(|values| values.iter().all(|value| value.as_str().is_some()))
        .map(|values| {
            values
                .iter()
                .filter_map(|value| value.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default();

    let memory = object
        .get("memory")
        .and_then(|value| value.as_object())
        .map(|values| {
            values
                .iter()
                .filter_map(|(key, value)| {
                    value.as_str().map(|text| (key.clone(), text.to_string()))
                })
                .collect()
        })
        .unwrap_or_default();

    let numeric_state = object
        .get("numeric_state")
        .and_then(|value| value.as_object())
        .map(|values| {
            values
                .iter()
                .filter_map(|(key, value)| value.as_f64().map(|number| (key.clone(), number)))
                .collect()
        })
        .unwrap_or_default();

    AgentPrivateState {
        objectives,
        memory,
        numeric_state,
    }
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
            private_state_typed: AgentPrivateState::default(),
        }
    }

    /// Construct an Agent with typed private state mirrored into legacy JSON.
    pub fn from_profile_with_private_state(
        profile: ActorProfile,
        private_state_typed: AgentPrivateState,
    ) -> Self {
        let mut agent = Self::from_profile(profile);
        agent.set_private_state_typed(private_state_typed);
        agent
    }

    /// Set typed private state and keep the legacy JSON field in sync.
    pub fn set_private_state_typed(&mut self, private_state_typed: AgentPrivateState) {
        self.private_state_typed = private_state_typed;
    }

    /// Pick a stub action from the profile's behavior_patterns.
    /// This is the deterministic fallback when no LLM provider is available.
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
            assessment: "deterministic stub fallback; no LLM assessment available".to_string(),
            category: "stub_fallback".to_string(),
            drivers: vec![
                "no_llm_provider".to_string(),
                "deterministic_profile_pattern".to_string(),
            ],
        }
    }

    /// Ask an LLM client to pick the next action. The model must return a JSON object.
    /// Callers should fall back to `pick_stub_action` on any error to preserve deterministic CI.
    pub async fn pick_llm_action(
        &self,
        tick: u64,
        client: &LlmClient,
        context: AgentDecisionContext<'_>,
    ) -> Result<AgentAction, LlmError> {
        let clients = [client];
        self.pick_llm_action_with_fallback(tick, &clients, context)
            .await
    }

    /// Ask LLM clients in fallback order. Only returns an error after every client fails.
    pub async fn pick_llm_action_with_fallback(
        &self,
        tick: u64,
        clients: &[&LlmClient],
        context: AgentDecisionContext<'_>,
    ) -> Result<AgentAction, LlmError> {
        let system = "You are a geopolitical simulation agent. Return ONLY strict JSON: {\"action_type\": string, \"target\": string|null, \"board_message\": string|null, \"confidence\": number between 0 and 1, \"rationale\": string, \"assessment\": string, \"category\": string, \"drivers\": string[]}. Keep action_type and category concise and machine-readable; drivers must be ordered, non-empty causal factors.";
        let user = self.build_llm_prompt(tick, context)?;
        let mut last_error = None;

        for client in clients {
            let response = client
                .chat(
                    vec![
                        ChatMessage {
                            role: "system".to_string(),
                            content: system.to_string(),
                        },
                        ChatMessage {
                            role: "user".to_string(),
                            content: user.clone(),
                        },
                    ],
                    None,
                )
                .await;

            match response {
                Ok(response) => return self.parse_llm_action(tick, &response),
                Err(error) => last_error = Some(error),
            }
        }

        Err(last_error.unwrap_or_else(|| LlmError::NoAvailableProvider("agent".to_string())))
    }

    fn build_llm_prompt(
        &self,
        tick: u64,
        context: AgentDecisionContext<'_>,
    ) -> Result<String, LlmError> {
        let visible_board: Vec<serde_json::Value> = context
            .visible_board
            .iter()
            .map(|message| {
                json!({
                    "tick": message.tick,
                    "sender": message.sender,
                    "content": message.content,
                    "visibility": message.visibility,
                })
            })
            .collect();

        let stick: Vec<serde_json::Value> = context
            .stick
            .iter()
            .map(|entry| {
                let value = entry.typed_value.to_json_value();
                json!({"tick": entry.tick, "key": entry.key, "value": value})
            })
            .collect();

        let private_state = serde_json::to_value(&self.private_state_typed).map_err(|error| {
            LlmError::ChatFailed(format!("private state serialization failed: {error}"))
        })?;

        let fields: Vec<serde_json::Value> = context
            .fields
            .iter()
            .map(|(key, value)| {
                json!({"field": {"region": key.region, "domain": key.domain}, "value": value})
            })
            .collect();

        let recent_actions: Vec<serde_json::Value> = context
            .recent_actions
            .iter()
            .rev()
            .take(5)
            .map(|action| {
                json!({
                    "tick": action.tick,
                    "action_type": action.action_type,
                    "confidence": action.confidence,
                    "rationale": action.rationale,
                })
            })
            .collect();

        let payload = json!({
            "tick": tick,
            "actor": {
                "id": self.profile.id,
                "name": self.profile.name,
                "tier": self.profile.tier,
                "interests": self.profile.interests,
                "red_lines": self.profile.red_lines,
                "capabilities": self.profile.capabilities,
                "behavior_patterns": self.profile.behavior_patterns,
                "historical_analogues": self.profile.historical_analogues,
            },
            "worldline_fields": fields,
            "visible_board": visible_board,
            "private_stick": stick,
            "private_state": private_state,
            "recent_actions": recent_actions,
        });

        serde_json::to_string_pretty(&payload)
            .map_err(|error| LlmError::ChatFailed(format!("prompt serialization failed: {error}")))
    }

    fn parse_llm_action(&self, tick: u64, response: &str) -> Result<AgentAction, LlmError> {
        let json_text = extract_json_object(response).unwrap_or(response);
        let parsed = serde_json::from_str::<LlmActionEnvelope>(json_text);

        match parsed {
            Ok(envelope) => {
                let confidence = envelope.confidence.unwrap_or(0.6).clamp(0.0, 1.0);
                let board_message = envelope.board_message.and_then(|content| {
                    let trimmed = content.trim();
                    if trimmed.is_empty() {
                        None
                    } else {
                        Some(BoardMessage {
                            tick,
                            sender: self.actor_id.clone(),
                            content: trimmed.to_string(),
                            visibility: MessageVisibility::Public,
                        })
                    }
                });

                Ok(AgentAction {
                    tick,
                    action_type: trim_or_default(envelope.action_type, "observe"),
                    target: envelope.target.and_then(trim_optional),
                    board_message,
                    confidence,
                    rationale: trim_or_default(envelope.rationale, "LLM action"),
                    assessment: trim_or_default(envelope.assessment, ""),
                    category: trim_or_default(envelope.category, "uncategorized"),
                    drivers: envelope
                        .drivers
                        .unwrap_or_default()
                        .into_iter()
                        .filter_map(trim_optional)
                        .collect(),
                })
            }
            Err(e) => Err(LlmError::ChatFailed(format!(
                "failed to parse LLM response as JSON: {e}"
            ))),
        }
    }
}

fn trim_or_default(value: Option<String>, default: &str) -> String {
    value
        .and_then(trim_optional)
        .unwrap_or_else(|| default.to_string())
}

fn trim_optional(value: String) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn extract_json_object(text: &str) -> Option<&str> {
    let start = text.find('{')?;
    let end = text.rfind('}')?;
    if start <= end {
        Some(&text[start..=end])
    } else {
        None
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
        assert_eq!(agent.private_state_typed, AgentPrivateState::default());
    }

    #[test]
    fn typed_private_state_mirrors_to_legacy_json() {
        let profile = sample_profile("china", vec!["observe"]);
        let mut typed = AgentPrivateState {
            objectives: vec!["de-escalate".to_string()],
            ..Default::default()
        };
        typed
            .memory
            .insert("last_signal".to_string(), "cautious".to_string());

        let agent = Agent::from_profile_with_private_state(profile, typed.clone());

        assert_eq!(agent.private_state_typed, typed);
    }

    #[test]
    fn agent_deserializes_legacy_private_state_json() {
        let profile = sample_profile("china", vec!["observe"]);
        let json = serde_json::json!({
            "actor_id": "china",
            "profile": profile,
            "status": "idle",
            "action_history": [],
            "private_state": {
                "objectives": ["de-escalate"],
                "memory": {"last_signal": "cautious"},
                "numeric_state": {"risk": 0.7}
            }
        });

        let agent: Agent = serde_json::from_value(json).expect("legacy agent JSON");

        assert_eq!(agent.private_state_typed.objectives, vec!["de-escalate"]);
        assert_eq!(
            agent.private_state_typed.memory.get("last_signal"),
            Some(&"cautious".to_string())
        );
        assert_eq!(
            agent.private_state_typed.numeric_state.get("risk"),
            Some(&0.7)
        );
    }

    #[test]
    fn agent_deserializes_malformed_legacy_private_state_lossily() {
        let profile = sample_profile("china", vec!["observe"]);
        let json = serde_json::json!({
            "actor_id": "china",
            "profile": profile,
            "status": "idle",
            "action_history": [],
            "private_state": {
                "objectives": ["keep-talks-open", 42],
                "memory": {"last_signal": "cautious", "bad": true},
                "numeric_state": {"risk": 0.7, "bad": "high"}
            }
        });

        let agent: Agent = serde_json::from_value(json).expect("lossy legacy agent JSON");

        assert!(agent.private_state_typed.objectives.is_empty());
        assert_eq!(agent.private_state_typed.memory.len(), 1);
        assert_eq!(
            agent.private_state_typed.memory.get("last_signal"),
            Some(&"cautious".to_string())
        );
        assert_eq!(agent.private_state_typed.numeric_state.len(), 1);
        assert_eq!(
            agent.private_state_typed.numeric_state.get("risk"),
            Some(&0.7)
        );
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
    fn agent_pick_stub_action_includes_audit_metadata() {
        let profile = sample_profile("usa", vec!["diplomatic_protest"]);
        let agent = Agent::from_profile(profile);

        let action = agent.pick_stub_action(0);

        assert_eq!(
            action.assessment,
            "deterministic stub fallback; no LLM assessment available"
        );
        assert_eq!(action.category, "stub_fallback");
        assert_eq!(
            action.drivers,
            vec!["no_llm_provider", "deterministic_profile_pattern"]
        );
    }

    #[test]
    fn agent_pick_stub_action_without_patterns() {
        let profile = sample_profile("test", vec![]);
        let agent = Agent::from_profile(profile);

        let action = agent.pick_stub_action(0);
        assert_eq!(action.action_type, "observe");
    }

    #[test]
    fn parse_llm_action_json_creates_public_board_message() {
        let profile = sample_profile("usa", vec!["observe"]);
        let agent = Agent::from_profile(profile);
        let action = agent.parse_llm_action(
            7,
            r#"{"action_type":"diplomatic_signal","target":"china","board_message":"We seek talks.","confidence":0.82,"rationale":"de-escalation"}"#,
        ).expect("valid JSON should parse");

        assert_eq!(action.tick, 7);
        assert_eq!(action.action_type, "diplomatic_signal");
        assert_eq!(action.target.as_deref(), Some("china"));
        assert!((action.confidence - 0.82).abs() < f64::EPSILON);
        assert_eq!(action.rationale, "de-escalation");
        let message = action.board_message.expect("board message");
        assert_eq!(message.sender, "usa");
        assert_eq!(message.content, "We seek talks.");
    }

    #[test]
    fn agent_action_deserializes_old_json_with_audit_defaults() {
        let action: AgentAction = serde_json::from_str(
            r#"{"tick":3,"action_type":"observe","target":null,"board_message":null,"confidence":0.4,"rationale":"legacy"}"#,
        )
        .expect("old action JSON should deserialize");

        assert_eq!(action.assessment, "");
        assert_eq!(action.category, "uncategorized");
        assert!(action.drivers.is_empty());
    }

    #[test]
    fn parse_llm_action_structured_fields() {
        let profile = sample_profile("usa", vec!["observe"]);
        let agent = Agent::from_profile(profile);
        let action = agent
            .parse_llm_action(
                7,
                r#"{
                    "action_type":" diplomatic_signal ",
                    "target":" china ",
                    "board_message":" We seek talks. ",
                    "confidence":0.82,
                    "rationale":" de-escalation ",
                    "assessment":" Escalation risk is rising but still controllable. ",
                    "category":" diplomacy ",
                    "drivers":[" public pressure ", "", " alliance signaling ", "   "]
                }"#,
            )
            .expect("structured JSON should parse");

        assert_eq!(action.action_type, "diplomatic_signal");
        assert_eq!(action.target.as_deref(), Some("china"));
        assert_eq!(action.rationale, "de-escalation");
        assert_eq!(
            action.assessment,
            "Escalation risk is rising but still controllable."
        );
        assert_eq!(action.category, "diplomacy");
        assert_eq!(
            action.drivers,
            vec!["public pressure", "alliance signaling"]
        );
        assert_eq!(
            action.board_message.expect("board message").content,
            "We seek talks."
        );
    }

    #[test]
    fn parse_llm_action_blank_structured_fields_default() {
        let profile = sample_profile("usa", vec!["observe"]);
        let agent = Agent::from_profile(profile);
        let action = agent
            .parse_llm_action(
                1,
                r#"{"assessment":"   ","category":"","drivers":[" ","valid driver"],"confidence":0.5}"#,
            )
            .expect("blank structured fields should default");

        assert_eq!(action.assessment, "");
        assert_eq!(action.category, "uncategorized");
        assert_eq!(action.drivers, vec!["valid driver"]);
    }

    #[test]
    fn parse_llm_action_clamps_confidence_and_defaults_action() {
        let profile = sample_profile("usa", vec!["observe"]);
        let agent = Agent::from_profile(profile);
        let action = agent
            .parse_llm_action(1, r#"{"confidence":2.5,"rationale":"x"}"#)
            .expect("JSON with missing action_type should still parse");

        assert_eq!(action.action_type, "observe");
        assert!((action.confidence - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn parse_llm_action_non_json_records_observation() {
        let profile = sample_profile("usa", vec!["observe"]);
        let agent = Agent::from_profile(profile);
        let result = agent.parse_llm_action(1, "watch and wait");

        assert!(result.is_err(), "non-JSON response should return Err");
        let err = result.unwrap_err();
        assert!(
            matches!(&err, LlmError::ChatFailed(_)),
            "should be ChatFailed, got {err:?}"
        );
    }

    #[test]
    fn extract_json_from_markdownish_response() {
        let text = "```json\n{\"action_type\":\"observe\"}\n```";
        assert_eq!(
            extract_json_object(text),
            Some("{\"action_type\":\"observe\"}")
        );
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
