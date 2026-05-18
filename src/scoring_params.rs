use serde::{Deserialize, Serialize};

/// Scoring parameters for the Fuxi (伏羲) scoring engine.
///
/// All weights, thresholds, and caps are exposed for per-environment tuning
/// via YAML configuration. The `Default` implementation matches the hard-coded
/// constants from the original TianJi scoring model.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ScoreParams {
    // ── Region / Actor weights (lookup tables) ──
    #[serde(rename = "region_weights")]
    pub region_weights: Vec<(String, f64)>,
    #[serde(rename = "actor_weights")]
    pub actor_weights: Vec<(String, f64)>,

    // ── Divergence score ──
    pub impact_weight: f64,
    pub field_attraction_weight: f64,

    // ── Field Attraction ──
    pub fa_margin_weight: f64,
    pub fa_max_margin_bonus: f64,
    pub fa_coherence_weight: f64,
    pub fa_near_tie_margin_threshold: f64,
    pub fa_near_tie_weight: f64,
    pub fa_max_near_tie_penalty: f64,
    pub fa_diffuse_third_field_threshold: f64,
    pub fa_diffuse_third_field_weight: f64,
    pub fa_max_diffuse_third_field_penalty: f64,

    // ── Impact magnitude ──
    pub im_dominant_field_weight: f64,
    pub im_nonzero_field_weight: f64,
    pub im_nonzero_field_min_score: f64,
    pub im_title_salience_actor_multiplier: f64,
    pub im_title_salience_region_multiplier: f64,
    pub im_title_salience_actor_max_per_match: f64,
    pub im_title_salience_region_max_per_match: f64,
    pub im_title_salience_max_bonus: f64,
    pub im_field_impact_baseline_average_weight: f64,
    pub im_field_impact_scale_weight: f64,
    pub im_field_impact_max_bonus: f64,
    pub im_text_signal_keyword_weight: f64,
    pub im_text_signal_title_weight: f64,
    pub im_text_signal_summary_weight: f64,
    pub im_text_signal_max_keyword_hits: usize,
    pub im_text_signal_max_title_hits: usize,
    pub im_text_signal_max_summary_hits: usize,
    pub im_text_signal_max_bonus: f64,
}

impl Default for ScoreParams {
    fn default() -> Self {
        Self {
            region_weights: vec![
                ("ukraine".into(), 2.5),
                ("russia".into(), 2.0),
                ("middle-east".into(), 2.5),
                ("east-asia".into(), 2.0),
                ("united-states".into(), 1.0),
                ("europe".into(), 1.0),
            ],
            actor_weights: vec![
                ("nato".into(), 1.5),
                ("eu".into(), 1.0),
                ("un".into(), 1.0),
                ("usa".into(), 1.5),
                ("china".into(), 1.5),
                ("russia".into(), 1.5),
                ("iran".into(), 1.2),
            ],
            impact_weight: 0.65,
            field_attraction_weight: 1.35,
            fa_margin_weight: 0.15,
            fa_max_margin_bonus: 1.0,
            fa_coherence_weight: 0.75,
            fa_near_tie_margin_threshold: 1.0,
            fa_near_tie_weight: 0.35,
            fa_max_near_tie_penalty: 0.3,
            fa_diffuse_third_field_threshold: 2.5,
            fa_diffuse_third_field_weight: 0.1,
            fa_max_diffuse_third_field_penalty: 0.2,
            im_dominant_field_weight: 0.25,
            im_nonzero_field_weight: 0.2,
            im_nonzero_field_min_score: 1.0,
            im_title_salience_actor_multiplier: 0.2,
            im_title_salience_region_multiplier: 0.2,
            im_title_salience_actor_max_per_match: 0.35,
            im_title_salience_region_max_per_match: 0.4,
            im_title_salience_max_bonus: 0.8,
            im_field_impact_baseline_average_weight: 1.5,
            im_field_impact_scale_weight: 0.06,
            im_field_impact_max_bonus: 0.5,
            im_text_signal_keyword_weight: 0.12,
            im_text_signal_title_weight: 0.2,
            im_text_signal_summary_weight: 0.1,
            im_text_signal_max_keyword_hits: 4,
            im_text_signal_max_title_hits: 2,
            im_text_signal_max_summary_hits: 2,
            im_text_signal_max_bonus: 1.0,
        }
    }
}

impl ScoreParams {
    /// Load from a YAML file. Partial overrides are supported — missing keys fall
    /// back to `Default`.
    pub fn load_yaml(path: &str) -> Result<Self, crate::TianJiError> {
        let raw = std::fs::read_to_string(path).map_err(crate::TianJiError::Io)?;
        serde_yaml::from_str::<Self>(&raw)
            .map_err(|e| crate::TianJiError::Yaml(e, path.to_string()))
    }

    /// Convenience lookup: region weight for a given key.
    pub fn region_weight(&self, key: &str, default: f64) -> f64 {
        self.region_weights
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| *v)
            .unwrap_or(default)
    }

    /// Convenience lookup: actor weight for a given key.
    pub fn actor_weight(&self, key: &str, default: f64) -> f64 {
        self.actor_weights
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| *v)
            .unwrap_or(default)
    }
}
