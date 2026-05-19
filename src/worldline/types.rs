use std::collections::BTreeMap;
use std::collections::BTreeSet;

use chrono::Utc;
use petgraph::graph::{DiGraph, NodeIndex};
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
    #[serde(default, with = "causal_graph_serde")]
    pub causal_graph: DiGraph<EventId, CausalRelation>,
    pub active_actors: BTreeSet<ActorId>,
    pub divergence: f64,
    pub parent: Option<WorldlineId>,
    pub diverge_tick: u64,
    pub snapshot_hash: Blake3Hash,
    pub created_at: chrono::DateTime<Utc>,
}

mod causal_graph_serde {
    use super::*;
    use petgraph::visit::EdgeRef;

    #[derive(Serialize, Deserialize)]
    struct StableGraph {
        nodes: Vec<StableNode>,
        edges: Vec<StableEdge>,
    }

    #[derive(Serialize, Deserialize)]
    struct StableNode {
        index: usize,
        event_id: EventId,
    }

    #[derive(Serialize, Deserialize)]
    struct StableEdge {
        source: usize,
        target: usize,
        relation: CausalRelation,
    }

    pub fn serialize<S>(
        graph: &DiGraph<EventId, CausalRelation>,
        serializer: S,
    ) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut nodes: Vec<(NodeIndex, EventId)> = graph
            .node_indices()
            .map(|index| (index, graph[index].clone()))
            .collect();
        nodes.sort_by(|(_, left), (_, right)| left.cmp(right));

        let index_by_node: BTreeMap<NodeIndex, usize> = nodes
            .iter()
            .enumerate()
            .map(|(stable_index, (node_index, _))| (*node_index, stable_index))
            .collect();

        let nodes: Vec<StableNode> = nodes
            .into_iter()
            .enumerate()
            .map(|(index, (_, event_id))| StableNode { index, event_id })
            .collect();

        let mut edges: Vec<StableEdge> = graph
            .edge_references()
            .map(|edge| StableEdge {
                source: index_by_node[&edge.source()],
                target: index_by_node[&edge.target()],
                relation: edge.weight().clone(),
            })
            .collect();
        edges.sort_by(|left, right| {
            left.source
                .cmp(&right.source)
                .then(left.target.cmp(&right.target))
                .then_with(|| format!("{:?}", left.relation).cmp(&format!("{:?}", right.relation)))
        });

        StableGraph { nodes, edges }.serialize(serializer)
    }

    pub fn deserialize<'de, D>(
        deserializer: D,
    ) -> Result<DiGraph<EventId, CausalRelation>, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let stable = StableGraph::deserialize(deserializer)?;
        let mut graph = DiGraph::new();
        let mut node_indices = Vec::with_capacity(stable.nodes.len());

        for (expected_index, node) in stable.nodes.into_iter().enumerate() {
            if node.index != expected_index {
                return Err(serde::de::Error::custom(format!(
                    "causal_graph node index {} is not contiguous at expected index {}",
                    node.index, expected_index
                )));
            }
            node_indices.push(graph.add_node(node.event_id));
        }

        for edge in stable.edges {
            let source = node_indices.get(edge.source).copied().ok_or_else(|| {
                serde::de::Error::custom(format!(
                    "causal_graph edge source index {} does not reference a node",
                    edge.source
                ))
            })?;
            let target = node_indices.get(edge.target).copied().ok_or_else(|| {
                serde::de::Error::custom(format!(
                    "causal_graph edge target index {} does not reference a node",
                    edge.target
                ))
            })?;
            graph.add_edge(source, target, edge.relation);
        }

        Ok(graph)
    }
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
    use petgraph::visit::EdgeRef;

    fn sample_worldline(causal_graph: DiGraph<EventId, CausalRelation>) -> Worldline {
        let mut fields = BTreeMap::new();
        fields.insert(
            FieldKey {
                region: "east-asia".to_string(),
                domain: "conflict".to_string(),
            },
            3.5,
        );

        Worldline {
            id: 1,
            fields: fields.clone(),
            events: vec!["evt-1".to_string(), "evt-2".to_string()],
            causal_graph,
            active_actors: BTreeSet::from(["usa".to_string()]),
            divergence: 0.0,
            parent: None,
            diverge_tick: 0,
            snapshot_hash: Worldline::compute_snapshot_hash(&fields),
            created_at: chrono::DateTime::parse_from_rfc3339("2026-05-19T00:00:00Z")
                .expect("valid timestamp")
                .with_timezone(&Utc),
        }
    }

    fn sample_relation(relation_type: CausalRelationType, confidence: f64) -> CausalRelation {
        CausalRelation {
            relation_type,
            confidence,
        }
    }

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

    #[test]
    fn causal_graph_empty_serializes_and_deserializes() {
        let worldline = sample_worldline(DiGraph::new());

        let value = serde_json::to_value(&worldline).expect("serialize worldline");
        assert_eq!(
            value["causal_graph"],
            serde_json::json!({ "nodes": [], "edges": [] })
        );

        let roundtrip: Worldline = serde_json::from_value(value).expect("deserialize worldline");
        assert_eq!(roundtrip.causal_graph.node_count(), 0);
        assert_eq!(roundtrip.causal_graph.edge_count(), 0);
    }

    #[test]
    fn causal_graph_non_empty_roundtrips_nodes_and_edges() {
        let mut graph = DiGraph::new();
        let source = graph.add_node("evt-1".to_string());
        let target = graph.add_node("evt-2".to_string());
        let relation = sample_relation(CausalRelationType::Causes, 0.82);
        graph.add_edge(source, target, relation.clone());

        let worldline = sample_worldline(graph);
        let json = serde_json::to_string(&worldline).expect("serialize worldline");
        let roundtrip: Worldline = serde_json::from_str(&json).expect("deserialize worldline");

        let mut nodes: Vec<EventId> = roundtrip.causal_graph.node_weights().cloned().collect();
        nodes.sort();
        assert_eq!(nodes, vec!["evt-1".to_string(), "evt-2".to_string()]);

        let edges: Vec<_> = roundtrip.causal_graph.edge_references().collect();
        assert_eq!(edges.len(), 1);
        assert_eq!(edges[0].weight(), &relation);
        assert_eq!(&roundtrip.causal_graph[edges[0].source()], "evt-1");
        assert_eq!(&roundtrip.causal_graph[edges[0].target()], "evt-2");
    }

    #[test]
    fn causal_graph_serialized_order_is_deterministic() {
        let mut graph = DiGraph::new();
        let evt_b = graph.add_node("evt-b".to_string());
        let evt_a = graph.add_node("evt-a".to_string());
        let evt_c = graph.add_node("evt-c".to_string());
        graph.add_edge(
            evt_c,
            evt_a,
            sample_relation(CausalRelationType::Precedes, 0.7),
        );
        graph.add_edge(
            evt_b,
            evt_c,
            sample_relation(CausalRelationType::Correlates, 0.5),
        );
        graph.add_edge(
            evt_a,
            evt_b,
            sample_relation(CausalRelationType::Causes, 0.9),
        );

        let value = serde_json::to_value(sample_worldline(graph)).expect("serialize worldline");

        assert_eq!(
            value["causal_graph"],
            serde_json::json!({
                "nodes": [
                    { "index": 0, "event_id": "evt-a" },
                    { "index": 1, "event_id": "evt-b" },
                    { "index": 2, "event_id": "evt-c" }
                ],
                "edges": [
                    {
                        "source": 0,
                        "target": 1,
                        "relation": { "relation_type": "Causes", "confidence": 0.9 }
                    },
                    {
                        "source": 1,
                        "target": 2,
                        "relation": { "relation_type": "Correlates", "confidence": 0.5 }
                    },
                    {
                        "source": 2,
                        "target": 0,
                        "relation": { "relation_type": "Precedes", "confidence": 0.7 }
                    }
                ]
            })
        );
    }

    #[test]
    fn legacy_json_missing_causal_graph_deserializes_to_empty_graph() {
        let mut value =
            serde_json::to_value(sample_worldline(DiGraph::new())).expect("serialize worldline");
        value
            .as_object_mut()
            .expect("worldline object")
            .remove("causal_graph");

        let worldline: Worldline =
            serde_json::from_value(value).expect("deserialize legacy worldline");

        assert_eq!(worldline.causal_graph.node_count(), 0);
        assert_eq!(worldline.causal_graph.edge_count(), 0);
    }

    #[test]
    fn causal_graph_invalid_edge_index_fails_deserialization() {
        let mut value =
            serde_json::to_value(sample_worldline(DiGraph::new())).expect("serialize worldline");
        value["causal_graph"] = serde_json::json!({
            "nodes": [
                { "index": 0, "event_id": "evt-1" }
            ],
            "edges": [
                {
                    "source": 0,
                    "target": 1,
                    "relation": { "relation_type": "Causes", "confidence": 0.82 }
                }
            ]
        });

        let error = serde_json::from_value::<Worldline>(value).expect_err("invalid edge index");

        assert!(error
            .to_string()
            .contains("causal_graph edge target index 1 does not reference a node"));
    }
}
