# PRD — Phase D6: Worldline causal_graph Serialization

> Priority: D6 | Spec: `.trellis/spec/backend/phase-d6-worldline-causal-graph-serialization.md`

## Goal

Persist `Worldline.causal_graph` explicitly instead of silently dropping it during serde.

## Background

The current `Worldline` struct uses `#[serde(default, skip_serializing)]` for `causal_graph`, which discards graph data in serialized snapshots. TianJi needs persisted causal structure for replay, audit, and future Nuwa analysis.

## Requirements

1. Keep public type:
   - `pub causal_graph: petgraph::graph::DiGraph<EventId, CausalRelation>`

2. Add explicit serde contract:
   - object with `nodes` and `edges`
   - contiguous serialized node indices
   - deterministic node/edge order
   - edge relation includes `CausalRelation`

3. Backward compatibility:
   - missing `causal_graph` deserializes to empty graph
   - invalid edge references error clearly

4. Tests:
   - empty graph serde
   - non-empty graph round-trip
   - deterministic output order
   - legacy missing field compatibility
   - invalid edge index rejection

## Allowed Files

- `src/worldline/types.rs`
- minimal related tests if needed

## Verification

Run:

```bash
cargo fmt
cargo test --quiet worldline
cargo test --quiet
cargo clippy -- -D warnings
git diff --check
```

## Completion Output

```text
DEV_DONE_D6 <summary>
```

or

```text
NEED_INPUT_D6 <reason>
```
