use serde::{Deserialize, Serialize};

/// Configuration for a Hongmeng simulation run.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct HongmengConfig {
    /// Maximum number of simulation rounds before forced convergence.
    #[serde(default = "default_max_rounds")]
    pub max_rounds: u64,

    /// Epsilon threshold for field-stabilized convergence detection.
    /// When all field deltas have absolute value < epsilon, convergence is triggered.
    #[serde(default = "default_convergence_epsilon")]
    pub convergence_epsilon: f64,

    /// Total token budget for LLM calls (reserved for future LLM integration).
    #[serde(default = "default_token_budget")]
    pub token_budget: usize,

    /// Number of ticks between automatic checkpoints.
    #[serde(default = "default_checkpoint_interval")]
    pub checkpoint_interval: u64,
}

fn default_max_rounds() -> u64 {
    10
}
fn default_convergence_epsilon() -> f64 {
    0.01
}
fn default_token_budget() -> usize {
    100_000
}
fn default_checkpoint_interval() -> u64 {
    5
}

impl Default for HongmengConfig {
    fn default() -> Self {
        Self {
            max_rounds: default_max_rounds(),
            convergence_epsilon: default_convergence_epsilon(),
            token_budget: default_token_budget(),
            checkpoint_interval: default_checkpoint_interval(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hongmeng_config_defaults() {
        let config = HongmengConfig::default();

        assert_eq!(config.max_rounds, 10);
        assert!((config.convergence_epsilon - 0.01).abs() < f64::EPSILON);
        assert_eq!(config.token_budget, 100_000);
        assert_eq!(config.checkpoint_interval, 5);
    }

    #[test]
    fn hongmeng_config_custom_values() {
        let config = HongmengConfig {
            max_rounds: 20,
            convergence_epsilon: 0.05,
            token_budget: 50_000,
            checkpoint_interval: 10,
        };

        assert_eq!(config.max_rounds, 20);
        assert!((config.convergence_epsilon - 0.05).abs() < f64::EPSILON);
        assert_eq!(config.token_budget, 50_000);
        assert_eq!(config.checkpoint_interval, 10);
    }

    #[test]
    fn hongmeng_config_serialization_roundtrip() {
        let config = HongmengConfig::default();
        let json = serde_json::to_string(&config).unwrap();
        let deserialized: HongmengConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, config);
    }

    #[test]
    fn hongmeng_config_deserialize_with_defaults() {
        // Minimal JSON — all fields should use defaults
        let json = "{}";
        let config: HongmengConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config, HongmengConfig::default());
    }
}
