# Phase 2.5: Nuwa Simulation Sandbox

> Part of plan.md §6.2 Phase 3 Nuwa
> Target: worldline fork/COW, forward simulation with LLM, backward search, pruning
> Status: spec

## Goal

Build the simulation sandbox that runs multi-round agent simulations on forked
worldlines. Forward (predict) and backward (backtrack) modes. Manual pruning
integration point for TUI.

## Core Types

### src/nuwa.rs

```rust
pub struct NuwaSandbox {
    pub id: String,                    // sandbox session id
    pub base_worldline: Worldline,     // original (read-only reference)
    pub forked_worldline: Worldline,   // working copy
    pub hongmeng: Hongmeng,
    pub provider: ProviderRegistry,
    pub mode: SimulationMode,
    pub outcome: Option<SimulationOutcome>,
}

pub enum SimulationMode {
    Forward {
        target_field: FieldKey,
        horizon_ticks: u64,
    },
    Backward {
        goal_description: String,
        goal_field_constraints: BTreeMap<FieldKey, (f64, f64)>,  // (min, max)
        max_interventions: usize,
    },
}

pub struct SimulationOutcome {
    pub mode: SimulationMode,
    pub branches: Vec<WorldlineBranch>,
    pub tick_count: u64,
    pub convergence_reason: ConvergenceReason,
}

pub struct WorldlineBranch {
    pub worldline: Worldline,
    pub probability: f64,              // 0.0 - 1.0
    pub event_sequence: Vec<String>,   // narrative of what happened
    pub final_divergence: f64,
}
```

### Forward Simulation

```rust
impl NuwaSandbox {
    pub async fn run_forward(
        base_worldline: Worldline,
        agents: Vec<Agent>,
        target_field: FieldKey,
        horizon_ticks: u64,
        provider: &ProviderRegistry,
    ) -> Result<SimulationOutcome, TianJiError> {
        // 1. Fork worldline
        // 2. Initialize Hongmeng with agents
        // 3. For each tick up to horizon_ticks:
        //    a. Referee generates WorldStateDelta
        //    b. Each agent receives visible board + stick + delta
        //    c. Agent calls LLM to decide action (use provider)
        //    d. Apply actions → update worldline fields
        //    e. Collision detection + convergence check
        //    f. If converged → break
        // 4. Return branches with probability estimates
    }
}
```

### Backward Search

```rust
impl NuwaSandbox {
    pub fn run_backward(
        base_worldline: Worldline,
        agents: Vec<Agent>,
        goal_field_constraints: BTreeMap<FieldKey, (f64, f64)>,
        max_interventions: usize,
    ) -> Result<SimulationOutcome, TianJiError> {
        // 1. Parse goal → field constraints
        // 2. Constraint pre-pruning: filter actions that violate red_lines
        // 3. For each intervention up to max_interventions:
        //    a. LLM coarse-filter: 3-5 most likely action directions
        //    b. Constraint fine-prune: alpha-beta on field impact
        //    c. Apply best action → update worldline
        //    d. Check if goal constraints met
        // 4. Return intervention paths
    }
}
```

### Intervention Path (backward output)

```rust
pub struct InterventionPath {
    pub interventions: Vec<InterventionStep>,
    pub path_score: f64,
    pub final_fields: BTreeMap<FieldKey, f64>,
    pub goal_met: bool,
}

pub struct InterventionStep {
    pub actor: ActorId,
    pub action: String,
    pub target_field: FieldKey,
    pub expected_impact: f64,
    pub confidence: f64,
}
```

## Pruning Protocol (stub)

```rust
pub enum PruningDecision {
    Continue,
    Prune(Vec<usize>),     // indices of branches to remove
    Pause { reason: String, options: Vec<String> },
}
```

Pruning is a stub — actual TUI integration comes later. For now:
- Forward: keep all branches, return top 3 by probability
- Backward: alpha-beta pruning by path_score

## Agent LLM Integration

In `run_forward`, each agent calls the LLM via ProviderRegistry:

```rust
let client = provider.resolve_with_fallback(&agent.profile.id);
let prompt = format!(
    "You are {}. Board: {:?}. Your stick: {:?}. World delta: {:?}. Choose next action.",
    agent.profile.name, visible_board, stick, delta
);
let response = client.chat(vec![ChatMessage { role: "user", content: prompt }], None).await?;
// Parse response → AgentAction
```

For now, keep the stub behavior (random action from behavior_patterns)
since the LLM client's `chat()` is also a stub. This is the integration
point that becomes real when `chat()` is implemented.

## Files

```
src/
├── nuwa.rs                # mod, re-exports
├── nuwa/
│   ├── sandbox.rs         # NuwaSandbox, fork/COW
│   ├── forward.rs         # run_forward simulation loop
│   ├── backward.rs        # run_backward constraint search
│   ├── pruning.rs         # PruningDecision (stub)
│   └── outcome.rs         # SimulationOutcome, WorldlineBranch, InterventionPath
```

No new dependencies.

## Tests

- Unit: worldline fork — forked has new id, parent set, diverge_tick
- Unit: worldline fork — modifying forked doesn't affect base
- Unit: forward simulation runs to convergence (stub agents)
- Unit: forward produces branches with decreasing probability
- Unit: backward search with simple goal constraints
- Unit: backward path_score calculation
- Unit: PruningDecision enum variants
- Integration: NuwaSandbox::new creates valid sandbox

## Stub behavior

- Agent LLM calls use random action selection (not real API calls)
- This keeps tests deterministic and offline
- Real LLM integration is the connection point between Phase 2.1 and 2.5

## Verification

- `cargo build` zero error
- `cargo test` all pass (222+ new tests)
- `cargo clippy -- -D warnings` clean
