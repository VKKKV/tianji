# Phase D6 — Worldline causal_graph Serialization

## Goal

Replace the current `#[serde(default, skip_serializing)]` treatment of `Worldline.causal_graph` with an explicit, stable serde contract for `petgraph::graph::DiGraph<EventId, CausalRelation>`.

D6 must preserve existing snapshot fields while making causal graph nodes and edges round-trip through JSON/YAML artifacts.

## Current Problem

`Worldline` currently declares:

```rust
#[serde(default, skip_serializing)]
pub causal_graph: DiGraph<EventId, CausalRelation>,
```

This silently drops graph data during serialization. TianJi cannot audit or replay causal chains if stored snapshots omit graph structure.

## Scope

In scope:

- `src/worldline/types.rs`
- Nuwa/Hongmeng tests only if constructors or expectations need updates
- Storage/API only if serialization contracts break tests

Out of scope:

- Graph inference algorithms
- UI graph rendering
- Database schema changes unless existing stored JSON cannot round-trip without them
- Migrating historical external artifacts

## Required Contract

Serialized `Worldline` must include `causal_graph` as an object:

```json
{
  "nodes": [
    { "index": 0, "event_id": "evt-1" }
  ],
  "edges": [
    {
      "source": 0,
      "target": 1,
      "relation": {
        "relation_type": "Causes",
        "confidence": 0.82
      }
    }
  ]
}
```

Rules:

- Node order must be deterministic.
- Edge order must be deterministic.
- Node indices in serialized form must be contiguous from `0..nodes.len()`.
- Edges reference serialized node indices, not petgraph internal indices directly.
- Deserialization must rebuild a `DiGraph<EventId, CausalRelation>` with equivalent node weights and edge weights.
- Missing `causal_graph` in old artifacts must deserialize as an empty graph.
- Invalid edge references must fail deserialization clearly.

## Requirements

1. Implement explicit serde helpers.
   - Prefer a private module in `src/worldline/types.rs`, e.g. `mod causal_graph_serde`.
   - Use `#[serde(default, with = "causal_graph_serde")]` or equivalent.
   - Preserve `Worldline` public field type as `DiGraph<EventId, CausalRelation>`.

2. Preserve compatibility.
   - Old JSON without `causal_graph` must still deserialize.
   - Empty graph serializes as `{ "nodes": [], "edges": [] }` or another documented stable empty representation. Prefer object with empty arrays.

3. Add tests.
   - Empty graph serializes/deserializes.
   - Non-empty graph round-trips node IDs and relation weights.
   - Deterministic serialized order is stable.
   - Missing `causal_graph` JSON deserializes to empty graph.
   - Invalid edge index fails.

## Acceptance Criteria

- `Worldline` JSON includes causal graph data.
- Existing worldline tests pass.
- New graph serde tests pass.
- `cargo fmt` passes.
- `cargo test --quiet worldline` passes.
- `cargo test --quiet` passes.
- `cargo clippy -- -D warnings` passes.
- `git diff --check` passes.
