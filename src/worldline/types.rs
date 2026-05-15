use std::collections::BTreeMap;
use std::collections::BTreeSet;

use chrono::Utc;
use petgraph::graph::DiGraph;
use serde::Deserialize;
use serde::Serialize;

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct FieldKey {
    pub region: String,
    pub domain: String,
}

impl Serialize for FieldKey {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&format!("{}:{}", self.region, self.domain))
    }
}

impl<'de> Deserialize<'de> for FieldKey {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        let (region, domain) = s
            .rsplit_once(':')
            .ok_or_else(|| serde::de::Error::custom(format!("invalid FieldKey format: {s}")))?;
        Ok(FieldKey {
            region: region.to_string(),
            domain: domain.to_string(),
        })
    }
}

pub type WorldlineId = u64;
pub type EventId = String;
pub type ActorId = String;
pub type Blake3Hash = String;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CausalRelation {
    pub relation_type: CausalRelationType,
    pub confidence: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum CausalRelationType {
    Causes,
    Correlates,
    Precedes,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Worldline {
    pub id: WorldlineId,
    pub fields: BTreeMap<FieldKey, f64>,
    pub events: Vec<EventId>,
    #[serde(skip)]
    pub causal_graph: DiGraph<EventId, CausalRelation>,
    pub active_actors: BTreeSet<ActorId>,
    pub divergence: f64,
    pub parent: Option<WorldlineId>,
    pub diverge_tick: u64,
    pub snapshot_hash: Blake3Hash,
    pub created_at: chrono::DateTime<Utc>,
}

impl Worldline {
    pub fn compute_snapshot_hash(fields: &BTreeMap<FieldKey, f64>) -> Blake3Hash {
        let mut hasher = blake3::Hasher::new();
        for (key, value) in fields {
            hasher.update(key.region.as_bytes());
            hasher.update(key.domain.as_bytes());
            hasher.update(&value.to_le_bytes());
        }
        hasher.finalize().to_hex().to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn worldline_construction_with_fields_and_events() {
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

        let hash = Worldline::compute_snapshot_hash(&fields);
        let worldline = Worldline {
            id: 1,
            fields: fields.clone(),
            events: vec!["evt-1".to_string(), "evt-2".to_string()],
            causal_graph: DiGraph::new(),
            active_actors: BTreeSet::from(["usa".to_string()]),
            divergence: 0.0,
            parent: None,
            diverge_tick: 0,
            snapshot_hash: hash,
            created_at: Utc::now(),
        };

        assert_eq!(worldline.id, 1);
        assert_eq!(worldline.fields.len(), 2);
        assert_eq!(worldline.events.len(), 2);
        assert!(worldline.fields.contains_key(&FieldKey {
            region: "east-asia".to_string(),
            domain: "conflict".to_string(),
        }));
    }

    #[test]
    fn field_key_equality_and_ordering() {
        let key_a = FieldKey {
            region: "east-asia".to_string(),
            domain: "conflict".to_string(),
        };
        let key_b = FieldKey {
            region: "east-asia".to_string(),
            domain: "diplomacy".to_string(),
        };
        let key_c = FieldKey {
            region: "global".to_string(),
            domain: "conflict".to_string(),
        };

        assert_eq!(key_a, key_a);
        assert_ne!(key_a, key_b);
        assert!(key_a < key_b);
        assert!(key_b < key_c);

        let mut map = BTreeMap::new();
        map.insert(key_a.clone(), 1.0);
        map.insert(key_b.clone(), 2.0);
        map.insert(key_c.clone(), 3.0);
        let keys: Vec<&FieldKey> = map.keys().collect();
        assert_eq!(keys[0], &key_a);
        assert_eq!(keys[1], &key_b);
        assert_eq!(keys[2], &key_c);
    }

    #[test]
    fn blake3_hash_of_fields_produces_deterministic_hex() {
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

        let hash1 = Worldline::compute_snapshot_hash(&fields);
        let hash2 = Worldline::compute_snapshot_hash(&fields);
        assert_eq!(hash1, hash2);
        assert_eq!(hash1.len(), 64);
        assert!(hash1.chars().all(|c| c.is_ascii_hexdigit()));

        let mut different_fields = BTreeMap::new();
        different_fields.insert(
            FieldKey {
                region: "europe".to_string(),
                domain: "diplomacy".to_string(),
            },
            5.0,
        );
        let hash3 = Worldline::compute_snapshot_hash(&different_fields);
        assert_ne!(hash1, hash3);
    }
}
