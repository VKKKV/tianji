use serde::{Deserialize, Serialize};

use crate::TianJiError;

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
/// Corporation-tier actors additionally set `market_share` and `supply_chain`.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Capabilities {
    #[serde(default)]
    pub military: f64,
    #[serde(default)]
    pub economic: f64,
    #[serde(default)]
    pub technological: f64,
    #[serde(default)]
    pub diplomatic: f64,
    #[serde(default)]
    pub cyber: f64,
    #[serde(default)]
    pub market_share: f64,
    #[serde(default)]
    pub supply_chain: f64,
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

impl ActorProfile {
    /// Validate semantic profile constraints that serde cannot express.
    pub fn validate(&self) -> Result<(), TianJiError> {
        let context = profile_context(self);

        if self.id.trim().is_empty() {
            return Err(validation_error(&context, "id", "must not be empty"));
        }
        if self.name.trim().is_empty() {
            return Err(validation_error(&context, "name", "must not be empty"));
        }
        if self.interests.is_empty() {
            return Err(validation_error(
                &context,
                "interests",
                "must contain at least one entry",
            ));
        }

        for (index, interest) in self.interests.iter().enumerate() {
            if interest.goal.trim().is_empty() {
                return Err(validation_error(
                    &context,
                    &format!("interests[{index}].goal"),
                    "must not be empty",
                ));
            }
            validate_unit_interval(
                &context,
                &format!("interests[{index}].salience"),
                interest.salience,
            )?;
        }

        validate_unit_interval(
            &context,
            "capabilities.military",
            self.capabilities.military,
        )?;
        validate_unit_interval(
            &context,
            "capabilities.economic",
            self.capabilities.economic,
        )?;
        validate_unit_interval(
            &context,
            "capabilities.technological",
            self.capabilities.technological,
        )?;
        validate_unit_interval(
            &context,
            "capabilities.diplomatic",
            self.capabilities.diplomatic,
        )?;
        validate_unit_interval(&context, "capabilities.cyber", self.capabilities.cyber)?;

        Ok(())
    }
}

fn profile_context(profile: &ActorProfile) -> String {
    if profile.id.trim().is_empty() {
        "profile <empty id>".to_string()
    } else {
        format!("profile '{}'", profile.id)
    }
}

fn validate_unit_interval(context: &str, field: &str, value: f64) -> Result<(), TianJiError> {
    if !value.is_finite() {
        return Err(validation_error(
            context,
            field,
            &format!("must be finite, got {value}"),
        ));
    }
    if !(0.0..=1.0).contains(&value) {
        return Err(validation_error(
            context,
            field,
            &format!("must be in [0.0, 1.0], got {value}"),
        ));
    }
    Ok(())
}

fn validation_error(context: &str, field: &str, reason: &str) -> TianJiError {
    TianJiError::Usage(format!("Invalid actor profile {context}: {field} {reason}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid_profile() -> ActorProfile {
        ActorProfile {
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
                ..Default::default()
            },
            behavior_patterns: vec!["proportional counter-sanctions".to_string()],
            historical_analogues: vec!["2016 SCS arbitration response".to_string()],
        }
    }

    fn validation_message(profile: ActorProfile) -> String {
        match profile.validate().unwrap_err() {
            TianJiError::Usage(message) => message,
            other => panic!("expected Usage validation error, got: {other}"),
        }
    }

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
        let profile = valid_profile();

        let yaml = serde_yaml::to_string(&profile).unwrap();
        let deserialized: ActorProfile = serde_yaml::from_str(&yaml).unwrap();
        assert_eq!(deserialized, profile);
    }

    #[test]
    fn valid_actor_profile_passes_validation() {
        valid_profile().validate().unwrap();
    }

    #[test]
    fn validation_rejects_empty_id() {
        let mut profile = valid_profile();
        profile.id = "  ".to_string();

        let message = validation_message(profile);
        assert!(message.contains("id"));
        assert!(message.contains("must not be empty"));
    }

    #[test]
    fn validation_rejects_empty_name() {
        let mut profile = valid_profile();
        profile.name = "  ".to_string();

        let message = validation_message(profile);
        assert!(message.contains("china"));
        assert!(message.contains("name"));
    }

    #[test]
    fn validation_rejects_empty_interests() {
        let mut profile = valid_profile();
        profile.interests = vec![];

        let message = validation_message(profile);
        assert!(message.contains("china"));
        assert!(message.contains("interests"));
    }

    #[test]
    fn validation_rejects_empty_interest_goal() {
        let mut profile = valid_profile();
        profile.interests[0].goal = "  ".to_string();

        let message = validation_message(profile);
        assert!(message.contains("interests[0].goal"));
        assert!(message.contains("must not be empty"));
    }

    #[test]
    fn validation_rejects_salience_below_zero() {
        let mut profile = valid_profile();
        profile.interests[0].salience = -0.1;

        let message = validation_message(profile);
        assert!(message.contains("interests[0].salience"));
        assert!(message.contains("-0.1"));
        assert!(message.contains("[0.0, 1.0]"));
    }

    #[test]
    fn validation_rejects_salience_above_one() {
        let mut profile = valid_profile();
        profile.interests[0].salience = 1.1;

        let message = validation_message(profile);
        assert!(message.contains("interests[0].salience"));
        assert!(message.contains("1.1"));
        assert!(message.contains("[0.0, 1.0]"));
    }

    #[test]
    fn validation_rejects_non_finite_salience() {
        let mut profile = valid_profile();
        profile.interests[0].salience = f64::NAN;

        let message = validation_message(profile);
        assert!(message.contains("interests[0].salience"));
        assert!(message.contains("must be finite"));
    }

    #[test]
    fn validation_rejects_capability_below_zero() {
        let mut profile = valid_profile();
        profile.capabilities.cyber = -0.01;

        let message = validation_message(profile);
        assert!(message.contains("capabilities.cyber"));
        assert!(message.contains("-0.01"));
        assert!(message.contains("[0.0, 1.0]"));
    }

    #[test]
    fn validation_rejects_capability_above_one() {
        let mut profile = valid_profile();
        profile.capabilities.military = 1.01;

        let message = validation_message(profile);
        assert!(message.contains("capabilities.military"));
        assert!(message.contains("1.01"));
        assert!(message.contains("[0.0, 1.0]"));
    }

    #[test]
    fn validation_rejects_non_finite_capability() {
        let mut profile = valid_profile();
        profile.capabilities.economic = f64::INFINITY;

        let message = validation_message(profile);
        assert!(message.contains("capabilities.economic"));
        assert!(message.contains("inf"));
        assert!(message.contains("must be finite"));
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
