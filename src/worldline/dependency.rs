use petgraph::graph::DiGraph;

use super::types::CausalRelation;
use super::types::CausalRelationType;
use super::types::FieldKey;

#[derive(Clone, Debug)]
pub struct FieldDependencyGraph {
    graph: DiGraph<FieldKey, CausalRelation>,
}

impl FieldDependencyGraph {
    pub fn default_graph() -> Self {
        let mut graph: DiGraph<FieldKey, CausalRelation> = DiGraph::new();

        let conflict = graph.add_node(FieldKey {
            region: "global".to_string(),
            domain: "conflict".to_string(),
        });
        let diplomacy = graph.add_node(FieldKey {
            region: "global".to_string(),
            domain: "diplomacy".to_string(),
        });
        let economy = graph.add_node(FieldKey {
            region: "global".to_string(),
            domain: "economy".to_string(),
        });
        let technology = graph.add_node(FieldKey {
            region: "global".to_string(),
            domain: "technology".to_string(),
        });

        graph.add_edge(
            conflict,
            diplomacy,
            CausalRelation {
                relation_type: CausalRelationType::Causes,
                confidence: 0.7,
            },
        );
        graph.add_edge(
            conflict,
            economy,
            CausalRelation {
                relation_type: CausalRelationType::Correlates,
                confidence: 0.5,
            },
        );
        graph.add_edge(
            economy,
            diplomacy,
            CausalRelation {
                relation_type: CausalRelationType::Precedes,
                confidence: 0.6,
            },
        );
        graph.add_edge(
            technology,
            economy,
            CausalRelation {
                relation_type: CausalRelationType::Causes,
                confidence: 0.6,
            },
        );
        graph.add_edge(
            technology,
            conflict,
            CausalRelation {
                relation_type: CausalRelationType::Correlates,
                confidence: 0.4,
            },
        );

        Self { graph }
    }

    pub fn topological_order(&self) -> Vec<FieldKey> {
        match petgraph::algo::toposort(&self.graph, None) {
            Ok(indices) => indices.iter().map(|idx| self.graph[*idx].clone()).collect(),
            Err(_) => Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn field_dependency_graph_topological_sort() {
        let graph = FieldDependencyGraph::default_graph();
        let order = graph.topological_order();

        assert!(!order.is_empty());

        let conflict_pos = order
            .iter()
            .position(|k| k.domain == "conflict")
            .expect("conflict in order");
        let diplomacy_pos = order
            .iter()
            .position(|k| k.domain == "diplomacy")
            .expect("diplomacy in order");
        let technology_pos = order
            .iter()
            .position(|k| k.domain == "technology")
            .expect("technology in order");
        let economy_pos = order
            .iter()
            .position(|k| k.domain == "economy")
            .expect("economy in order");

        assert!(conflict_pos < diplomacy_pos);
        assert!(technology_pos < economy_pos);
    }
}
