use serde::{Deserialize, Serialize};

/// Tier of an actor: nation, organization, or corporation.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Ord, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ActorTier {
    Nation,
    Organization,
    Corporation,
}

/// An interest of an actor with a salience weight (0.0–1.0).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Interest {
    pub goal: String,
    pub salience: f64,
}

/// Capability scores for an actor across five domains (0.0–1.0).
/// Organization-tier actors typically have `military: 0.0`.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Capabilities {
    pub military: f64,
    pub economic: f64,
    pub technological: f64,
    pub diplomatic: f64,
    pub cyber: f64,
}

/// A static actor profile loaded from YAML.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ActorProfile {
    pub id: String,
    pub name: String,
    pub tier: ActorTier,
    #[serde(default)]
    pub interests: Vec<Interest>,
    #[serde(default)]
    pub red_lines: Vec<String>,
    #[serde(default)]
    pub capabilities: Capabilities,
    #[serde(default)]
    pub behavior_patterns: Vec<String>,
    #[serde(default)]
    pub historical_analogues: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn actor_tier_deserialize_lowercases() {
        let tier: ActorTier = serde_yaml::from_str("nation").unwrap();
        assert_eq!(tier, ActorTier::Nation);

        let tier: ActorTier = serde_yaml::from_str("organization").unwrap();
        assert_eq!(tier, ActorTier::Organization);

        let tier: ActorTier = serde_yaml::from_str("corporation").unwrap();
        assert_eq!(tier, ActorTier::Corporation);
    }

    #[test]
    fn actor_profile_roundtrip() {
        let profile = ActorProfile {
            id: "china".to_string(),
            name: "China".to_string(),
            tier: ActorTier::Nation,
            interests: vec![Interest {
                goal: "maintain territorial integrity".to_string(),
                salience: 0.95,
            }],
            red_lines: vec!["foreign military presence".to_string()],
            capabilities: Capabilities {
                military: 0.85,
                economic: 0.80,
                technological: 0.70,
                diplomatic: 0.75,
                cyber: 0.82,
            },
            behavior_patterns: vec!["proportional counter-sanctions".to_string()],
            historical_analogues: vec!["2016 SCS arbitration response".to_string()],
        };

        let yaml = serde_yaml::to_string(&profile).unwrap();
        let deserialized: ActorProfile = serde_yaml::from_str(&yaml).unwrap();
        assert_eq!(deserialized, profile);
    }

    #[test]
    fn actor_profile_from_yaml_string() {
        let yaml = r#"
id: china
name: China
tier: nation
interests:
  - goal: "maintain territorial integrity in South China Sea"
    salience: 0.95
red_lines:
  - "foreign military presence in Taiwan Strait → full retaliatory posture"
capabilities:
  military: 0.85
  economic: 0.80
  technological: 0.70
  diplomatic: 0.75
  cyber: 0.82
behavior_patterns:
  - "responds to sanctions with proportional counter-sanctions"
  - "prefers economic leverage before military signaling"
historical_analogues:
  - "2016 South China Sea arbitration response"
  - "2017 THAAD deployment → economic retaliation against Lotte"
"#;
        let profile: ActorProfile = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(profile.id, "china");
        assert_eq!(profile.name, "China");
        assert_eq!(profile.tier, ActorTier::Nation);
        assert_eq!(profile.interests.len(), 1);
        assert!((profile.interests[0].salience - 0.95).abs() < f64::EPSILON);
        assert_eq!(profile.red_lines.len(), 1);
        assert!((profile.capabilities.military - 0.85).abs() < f64::EPSILON);
        assert!((profile.capabilities.economic - 0.80).abs() < f64::EPSILON);
        assert_eq!(profile.behavior_patterns.len(), 2);
        assert_eq!(profile.historical_analogues.len(), 2);
    }

    #[test]
    fn organization_tier_military_zero() {
        let yaml = r#"
id: nato
name: NATO
tier: organization
interests:
  - goal: "collective defense of member states"
    salience: 0.90
red_lines:
  - "armed attack against any member state"
capabilities:
  military: 0.0
  economic: 0.60
  technological: 0.70
  diplomatic: 0.85
  cyber: 0.65
behavior_patterns:
  - "issues diplomatic statements before considering military action"
historical_analogues:
  - "1999 Kosovo intervention"
"#;
        let profile: ActorProfile = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(profile.tier, ActorTier::Organization);
        assert!((profile.capabilities.military).abs() < f64::EPSILON);
    }

    #[test]
    fn corporation_tier_profile() {
        let yaml = r#"
id: huawei
name: Huawei
tier: corporation
interests:
  - goal: "maintain global telecom market share"
    salience: 0.85
red_lines:
  - "forced technology transfer requirements"
capabilities:
  military: 0.0
  economic: 0.70
  technological: 0.90
  diplomatic: 0.30
  cyber: 0.75
behavior_patterns:
  - "leverages government relationships for market access"
historical_analogues:
  - "2019 US entity list response"
"#;
        let profile: ActorProfile = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(profile.tier, ActorTier::Corporation);
        assert_eq!(profile.id, "huawei");
        assert!((profile.capabilities.technological - 0.90).abs() < f64::EPSILON);
    }

    #[test]
    fn invalid_tier_returns_error() {
        let yaml = r#"
id: test
name: Test
tier: invalid_tier
"#;
        let result: Result<ActorProfile, _> = serde_yaml::from_str(yaml);
        assert!(result.is_err());
    }
}
