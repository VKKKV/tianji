# Phase 2.2: Worldline Data Model + Baseline

> Part of plan.md §6.3 Phase 2 Hongmeng
> Target: Worldline struct, FieldKey, baseline definition, Blake3 snapshot
> Status: implemented

## Goal

Define the Worldline data model — the core state representation for the
Hongmeng/Nuwa simulation engine. Add baseline locking and Blake3 snapshot hashing.
No agent logic yet.

## New Dependencies

```toml
blake3 = "1"
petgraph = "0.7"
```

## Data Types

### src/worldline.rs

```rust
pub struct Worldline {
    pub id: WorldlineId,
    pub fields: BTreeMap<FieldKey, f64>,
    pub events: Vec<EventId>,
    pub causal_graph: petgraph::DiGraph<EventId, CausalRelation>,
    pub active_actors: BTreeSet<ActorId>,
    pub divergence: f64,
    pub parent: Option<WorldlineId>,
    pub diverge_tick: u64,
    pub snapshot_hash: Blake3Hash,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

pub struct FieldKey {
    pub region: String,       // "east-asia" | "europe" | "middle-east" | "global" | ...
    pub domain: String,       // "conflict" | "economy" | "diplomacy" | "technology" | ...
}

pub type WorldlineId = u64;
pub type EventId = String;
pub type ActorId = String;
pub type Blake3Hash = String;    // hex string

pub struct CausalRelation {
    pub relation_type: CausalRelationType,
    pub confidence: f64,
}

pub enum CausalRelationType {
    Causes,
    Correlates,
    Precedes,
}
```

### Baseline

```rust
pub struct Baseline {
    pub worldline_id: WorldlineId,
    pub snapshot_hash: Blake3Hash,
    pub fields: BTreeMap<FieldKey, f64>,    // frozen snapshot
    pub locked_at: chrono::DateTime<chrono::Utc>,
    pub locked_by: Option<String>,           // "operator" | "auto" | run_id
}

impl Baseline {
    pub fn from_worldline(worldline: &Worldline, locked_by: Option<String>) -> Self
    pub fn compute_divergence(&self, current: &Worldline) -> f64
}
```

### Divergence

```rust
pub fn compute_divergence(baseline: &BTreeMap<FieldKey, f64>, current: &BTreeMap<FieldKey, f64>) -> f64 {
    // Euclidean distance across all shared fields
    let mut sum_sq = 0.0;
    for (key, baseline_value) in baseline {
        let current_value = current.get(key).copied().unwrap_or(0.0);
        let diff = current_value - baseline_value;
        sum_sq += diff * diff;
    }
    // Also include fields in current but not in baseline
    for (key, current_value) in current {
        if !baseline.contains_key(key) {
            sum_sq += current_value * current_value;
        }
    }
    sum_sq.sqrt()
}
```

### Field Dependency Graph (stub)

```rust
pub struct FieldDependencyGraph {
    graph: petgraph::DiGraph<FieldKey, CausalRelation>,
}

impl FieldDependencyGraph {
    pub fn default_graph() -> Self  // predefined core dependency edges
    pub fn topological_order(&self) -> Vec<FieldKey>
}
```

## Storage (optional, deferred)

Don't add SQLite tables for worldlines yet — keep them in-memory for now.
Persistence will come in Phase 2.4 with checkpoints.

## Files

```
src/
├── worldline.rs          # mod, re-exports
├── worldline/
│   ├── types.rs          # Worldline, FieldKey, CausalRelation, Blake3Hash
│   ├── baseline.rs       # Baseline, compute_divergence
│   └── dependency.rs     # FieldDependencyGraph, topological sort
```

## Tests

- Unit: Worldline construction with fields and events
- Unit: Baseline snapshot from worldline
- Unit: compute_divergence — identical worldlines → 0.0
- Unit: compute_divergence — one field changed → non-zero
- Unit: compute_divergence — field only in current, not baseline
- Unit: FieldKey equality and ordering (BTreeMap key)
- Unit: Blake3 hash of fields produces deterministic hex
- Unit: FieldDependencyGraph topological sort

## Verification

- `cargo build` zero error
- `cargo test` all pass
- `cargo clippy -- -D warnings` clean
