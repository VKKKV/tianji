use serde::{Deserialize, Serialize};

use crate::worldline::types::ActorId;

/// Visibility scope for a board message.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Ord, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MessageVisibility {
    /// All agents can see this message.
    Public,
    /// Only the target agent can see this message.
    Directed(ActorId),
}

/// A message posted to the shared board.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct BoardMessage {
    pub tick: u64,
    pub sender: ActorId,
    pub content: String,
    pub visibility: MessageVisibility,
}

/// Strongly typed private stick value.
///
/// The legacy `value` JSON field remains on `StickEntry` for backward-compatible
/// serialization while consumers migrate to this type.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", content = "value", rename_all = "snake_case")]
pub enum StickValue {
    Text(String),
    Number(f64),
    Flag(bool),
    List(Vec<String>),
}

impl StickValue {
    pub fn to_json_value(&self) -> serde_json::Value {
        match self {
            Self::Text(value) => serde_json::Value::String(value.clone()),
            Self::Number(value) => serde_json::json!(value),
            Self::Flag(value) => serde_json::Value::Bool(*value),
            Self::List(values) => serde_json::json!(values),
        }
    }
}

/// A private stick entry — per-actor scratch space not visible to other agents.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct StickEntry {
    pub tick: u64,
    pub key: String,
    pub value: serde_json::Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub typed_value: Option<StickValue>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn board_message_public_visibility() {
        let msg = BoardMessage {
            tick: 1,
            sender: "usa".to_string(),
            content: "Proposing sanctions".to_string(),
            visibility: MessageVisibility::Public,
        };

        assert!(matches!(msg.visibility, MessageVisibility::Public));
        assert_eq!(msg.sender, "usa");
    }

    #[test]
    fn board_message_directed_visibility() {
        let msg = BoardMessage {
            tick: 2,
            sender: "china".to_string(),
            content: "Private counter-offer".to_string(),
            visibility: MessageVisibility::Directed("usa".to_string()),
        };

        match &msg.visibility {
            MessageVisibility::Directed(target) => assert_eq!(target, "usa"),
            _ => panic!("expected Directed visibility"),
        }
    }

    #[test]
    fn stick_entry_construction() {
        let entry = StickEntry {
            tick: 3,
            key: "threat_level".to_string(),
            value: serde_json::json!({"level": "high", "confidence": 0.8}),
            typed_value: None,
        };

        assert_eq!(entry.tick, 3);
        assert_eq!(entry.key, "threat_level");
    }

    #[test]
    fn board_message_serialization_roundtrip() {
        let msg = BoardMessage {
            tick: 1,
            sender: "iran".to_string(),
            content: "Resuming talks".to_string(),
            visibility: MessageVisibility::Directed("usa".to_string()),
        };

        let json = serde_json::to_string(&msg).unwrap();
        let deserialized: BoardMessage = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, msg);
    }

    #[test]
    fn stick_entry_serialization_roundtrip() {
        let entry = StickEntry {
            tick: 5,
            key: "negotiation_stance".to_string(),
            value: serde_json::json!("cautious"),
            typed_value: Some(StickValue::Text("cautious".to_string())),
        };

        let json = serde_json::to_string(&entry).unwrap();
        let deserialized: StickEntry = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, entry);
    }

    #[test]
    fn stick_value_converts_to_legacy_json_value() {
        assert_eq!(
            StickValue::Text("watch".to_string()).to_json_value(),
            serde_json::json!("watch")
        );
        assert_eq!(
            StickValue::Number(0.7).to_json_value(),
            serde_json::json!(0.7)
        );
        assert_eq!(
            StickValue::Flag(true).to_json_value(),
            serde_json::json!(true)
        );
        assert_eq!(
            StickValue::List(vec!["a".to_string(), "b".to_string()]).to_json_value(),
            serde_json::json!(["a", "b"])
        );
    }
}
