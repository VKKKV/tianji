use std::collections::BTreeMap;

use chrono::Utc;
use serde::Deserialize;
use serde::Serialize;

use super::types::Blake3Hash;
use super::types::FieldKey;
use super::types::Worldline;
use super::types::WorldlineId;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Baseline {
    pub worldline_id: WorldlineId,
    pub snapshot_hash: Blake3Hash,
    pub fields: BTreeMap<FieldKey, f64>,
    pub locked_at: chrono::DateTime<Utc>,
    pub locked_by: Option<String>,
}

impl Baseline {
    pub fn from_worldline(worldline: &Worldline, locked_by: Option<String>) -> Self {
        Self {
            worldline_id: worldline.id,
            snapshot_hash: worldline.snapshot_hash.clone(),
            fields: worldline.fields.clone(),
            locked_at: Utc::now(),
            locked_by,
        }
    }

    pub fn compute_divergence(&self, current: &Worldline) -> f64 {
        compute_divergence(&self.fields, &current.fields)
    }
}

pub fn compute_divergence(
    baseline: &BTreeMap<FieldKey, f64>,
    current: &BTreeMap<FieldKey, f64>,
) -> f64 {
    let mut sum_sq = 0.0;
    for (key, baseline_value) in baseline {
        let current_value = current.get(key).copied().unwrap_or(0.0);
        let diff = current_value - baseline_value;
        sum_sq += diff * diff;
    }
    for (key, current_value) in current {
        if !baseline.contains_key(key) {
            sum_sq += current_value * current_value;
        }
    }
    sum_sq.sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;
    use petgraph::graph::DiGraph;
    use std::collections::BTreeSet;

    fn sample_worldline(id: WorldlineId, fields: BTreeMap<FieldKey, f64>) -> Worldline {
        let hash = Worldline::compute_snapshot_hash(&fields);
        Worldline {
            id,
            fields,
            events: vec![],
            causal_graph: DiGraph::new(),
            active_actors: BTreeSet::new(),
            divergence: 0.0,
            parent: None,
            diverge_tick: 0,
            snapshot_hash: hash,
            created_at: Utc::now(),
        }
    }

    #[test]
    fn baseline_snapshot_from_worldline() {
        let mut fields = BTreeMap::new();
        fields.insert(
            FieldKey {
                region: "east-asia".to_string(),
                domain: "conflict".to_string(),
            },
            3.5,
        );
        let worldline = sample_worldline(1, fields.clone());
        let baseline = Baseline::from_worldline(&worldline, Some("auto".to_string()));

        assert_eq!(baseline.worldline_id, 1);
        assert_eq!(baseline.snapshot_hash, worldline.snapshot_hash);
        assert_eq!(baseline.fields, fields);
        assert_eq!(baseline.locked_by, Some("auto".to_string()));
    }

    #[test]
    fn compute_divergence_identical_worldlines_zero() {
        let mut fields = BTreeMap::new();
        fields.insert(
            FieldKey {
                region: "east-asia".to_string(),
                domain: "conflict".to_string(),
            },
            3.5,
        );
        fields.insert(
            FieldKey {
                region: "global".to_string(),
                domain: "economy".to_string(),
            },
            2.0,
        );
        let worldline = sample_worldline(1, fields.clone());
        let baseline = Baseline::from_worldline(&worldline, None);
        let divergence = baseline.compute_divergence(&worldline);
        assert!((divergence - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn compute_divergence_one_field_changed_nonzero() {
        let mut baseline_fields = BTreeMap::new();
        baseline_fields.insert(
            FieldKey {
                region: "east-asia".to_string(),
                domain: "conflict".to_string(),
            },
            3.5,
        );

        let mut current_fields = BTreeMap::new();
        current_fields.insert(
            FieldKey {
                region: "east-asia".to_string(),
                domain: "conflict".to_string(),
            },
            5.5,
        );

        let baseline_worldline = sample_worldline(1, baseline_fields.clone());
        let baseline = Baseline::from_worldline(&baseline_worldline, None);
        let current_worldline = sample_worldline(2, current_fields);
        let divergence = baseline.compute_divergence(&current_worldline);

        let expected = (2.0_f64 * 2.0).sqrt();
        assert!((divergence - expected).abs() < 1e-10);
    }

    #[test]
    fn compute_divergence_field_only_in_current_not_baseline() {
        let baseline_fields = BTreeMap::new();
        let mut current_fields = BTreeMap::new();
        current_fields.insert(
            FieldKey {
                region: "europe".to_string(),
                domain: "diplomacy".to_string(),
            },
            4.0,
        );

        let baseline_worldline = sample_worldline(1, baseline_fields);
        let baseline = Baseline::from_worldline(&baseline_worldline, None);
        let current_worldline = sample_worldline(2, current_fields);
        let divergence = baseline.compute_divergence(&current_worldline);

        let expected = (4.0_f64 * 4.0).sqrt();
        assert!((divergence - expected).abs() < 1e-10);
    }
}
