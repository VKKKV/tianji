use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// Cross-scenario memory for an actor — reputation, relationships, and learned strategies.
/// Stub for Phase 2.3; will be populated by Nuwa simulation in later phases.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct CrossScenarioMemory {
    pub actor_id: String,
    #[serde(default)]
    pub reputation_score: f64,
    #[serde(default)]
    pub relationship_graph: BTreeMap<String, f64>,
    #[serde(default)]
    pub learned_strategies: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cross_scenario_memory_serialization_roundtrip() {
        let mut relationships = BTreeMap::new();
        relationships.insert("usa".to_string(), -0.3);
        relationships.insert("russia".to_string(), 0.6);

        let memory = CrossScenarioMemory {
            actor_id: "china".to_string(),
            reputation_score: 0.45,
            relationship_graph: relationships,
            learned_strategies: vec!["sanctions → counter-sanctions".to_string()],
        };

        let json = serde_json::to_string(&memory).unwrap();
        let deserialized: CrossScenarioMemory = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, memory);
    }

    #[test]
    fn cross_scenario_memory_defaults() {
        let yaml = r#"
actor_id: test
"#;
        let memory: CrossScenarioMemory = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(memory.actor_id, "test");
        assert!((memory.reputation_score).abs() < f64::EPSILON);
        assert!(memory.relationship_graph.is_empty());
        assert!(memory.learned_strategies.is_empty());
    }
}
