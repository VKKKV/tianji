use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Dynamic profile for an actor — temporal patterns extracted by LLM analysis.
/// Stub for Phase 2.3; will be populated by Hongmeng agents in later phases.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DynamicProfile {
    pub actor_id: String,
    #[serde(default)]
    pub temporal_patterns: Vec<String>,
    pub updated_at: DateTime<Utc>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dynamic_profile_serialization_roundtrip() {
        let profile = DynamicProfile {
            actor_id: "china".to_string(),
            temporal_patterns: vec!["escalates after sanctions".to_string()],
            updated_at: DateTime::UNIX_EPOCH,
        };

        let json = serde_json::to_string(&profile).unwrap();
        let deserialized: DynamicProfile = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, profile);
    }

    #[test]
    fn dynamic_profile_default_temporal_patterns() {
        let yaml = r#"
actor_id: usa
updated_at: 1970-01-01T00:00:00Z
"#;
        let profile: DynamicProfile = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(profile.actor_id, "usa");
        assert!(profile.temporal_patterns.is_empty());
    }
}
