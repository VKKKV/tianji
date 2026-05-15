# Phase 2.4: Hongmeng Orchestration Layer

> Part of plan.md §6.1 Phase 2 Hongmeng
> Target: Agent lifecycle, Board/Stick routing, Referee, Checkpoint
> Status: spec

## Goal

Build the coordination layer that manages LLM-driven agents in a multi-round
simulation. Agent lifecycle, message routing, convergence, and crash recovery.

## Core Types

### src/hongmeng.rs

```rust
pub struct Hongmeng {
    pub agents: BTreeMap<ActorId, Agent>,
    pub board: Vec<BoardMessage>,
    pub sticks: BTreeMap<ActorId, Vec<StickEntry>>,
    pub referee_history: Vec<WorldStateDelta>,
    pub worldline: Worldline,
    pub config: HongmengConfig,
    pub tick: u64,
    pub status: SimulationStatus,
}

pub enum SimulationStatus {
    Idle,
    Running,
    Paused { reason: String },
    Converged { reason: ConvergenceReason },
    Failed { error: String },
}

pub enum ConvergenceReason {
    MaxRounds(u64),
    AgentConsensus,
    FieldStabilized(f64),
    TokenBudgetExhausted,
}
```

### Agent

```rust
pub struct Agent {
    pub actor_id: ActorId,
    pub profile: ActorProfile,
    pub status: AgentStatus,
    pub action_history: Vec<AgentAction>,
    pub private_state: serde_json::Value,    // Stick data
}

pub enum AgentStatus {
    Idle,
    Thinking,
    Done,
    Error(String),
}

pub struct AgentAction {
    pub tick: u64,
    pub action_type: String,       // "diplomatic_protest", "military_exercise", etc.
    pub target: Option<String>,    // target actor
    pub board_message: Option<BoardMessage>,
    pub confidence: f64,
    pub rationale: String,
}
```

### Board/Stick

```rust
pub struct BoardMessage {
    pub tick: u64,
    pub sender: ActorId,
    pub content: String,
    pub visibility: MessageVisibility,
}

pub enum MessageVisibility {
    Public,             // all agents see
    Directed(ActorId),   // only target sees
}

pub struct StickEntry {
    pub tick: u64,
    pub key: String,
    pub value: serde_json::Value,
}
```

### Referee

```rust
pub struct WorldStateDelta {
    pub tick: u64,
    pub summary: String,                       // "Iran increased military readiness to Level 3"
    pub field_changes: BTreeMap<FieldKey, f64>,  // delta per field
    pub affected_actors: Vec<ActorId>,
}
```

### Checkpoint

```rust
pub struct HongmengCheckpoint {
    pub simulation_id: String,
    pub tick: u64,
    pub worldline_snapshot: Worldline,
    pub agent_states: BTreeMap<ActorId, AgentStatus>,
    pub board_snapshot: Vec<BoardMessage>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

impl HongmengCheckpoint {
    pub fn save(&self, conn: &rusqlite::Connection) -> Result<(), TianJiError>
    pub fn load(conn: &rusqlite::Connection, simulation_id: &str) -> Result<Option<Self>, TianJiError>
}
```

### Convergence

```rust
pub fn check_convergence(
    hongmeng: &Hongmeng,
    prev_fields: &BTreeMap<FieldKey, f64>,
    config: &HongmengConfig,
) -> Option<ConvergenceReason> {
    // max_rounds reached
    // all agents predicted same action 2 consecutive rounds
    // field change < epsilon
}
```

## Message Routing

```rust
impl Hongmeng {
    pub fn broadcast_to_board(&mut self, message: BoardMessage)
    pub fn send_directed(&mut self, sender: ActorId, target: ActorId, content: String)
    pub fn get_visible_board(&self, viewer: &ActorId) -> Vec<&BoardMessage>
    // Returns: Public messages + Directed messages where viewer is target
    pub fn get_stick(&self, actor_id: &ActorId) -> &[StickEntry]
    pub fn set_stick(&mut self, actor_id: &ActorId, key: String, value: serde_json::Value)
}
```

## Configuration

```rust
pub struct HongmengConfig {
    pub max_rounds: u64,            // default: 10
    pub convergence_epsilon: f64,   // default: 0.01
    pub token_budget: usize,        // default: 100_000
    pub checkpoint_interval: u64,  // ticks between checkpoints
}
```

## Simulation Loop (skeleton)

```rust
impl Hongmeng {
    pub async fn run_simulation(
        &mut self,
        worldline: Worldline,
        agents: Vec<Agent>,
        provider: &ProviderRegistry,
    ) -> Result<SimulationOutcome, TianJiError> {
        // 1. Initialize
        // 2. For each tick:
        //    a. Referee generates WorldStateDelta
        //    b. Each agent receives visible board + own stick + delta
        //    c. Agents call LLM to decide action (stub: random choice for now)
        //    d. Collision detection
        //    e. Check convergence
        //    f. Checkpoint if needed
        // 3. Return outcome
    }
}
```

## Files

```
src/
├── hongmeng.rs              # mod, re-exports
├── hongmeng/
│   ├── agent.rs             # Agent, AgentStatus, AgentAction
│   ├── board.rs             # BoardMessage, StickEntry, visibility routing
│   ├── referee.rs           # WorldStateDelta generation
│   ├── convergence.rs       # check_convergence, ConvergenceReason
│   ├── checkpoint.rs        # HongmengCheckpoint, SQLite save/load
│   ├── config.rs            # HongmengConfig
│   └── simulation.rs        # run_simulation skeleton
```

No new Cargo dependencies needed (all already added).

## Tests

- Unit: Agent construction from ActorProfile
- Unit: Board broadcast → all agents see Public
- Unit: Directed message → only target sees
- Unit: Stick read/write isolation
- Unit: Convergence — max_rounds triggers
- Unit: Convergence — field_stabilized triggers when delta < epsilon
- Unit: Checkpoint save/load roundtrip via in-memory SQLite
- Unit: SimulationStatus transitions

## Stub behavior

- `run_simulation` does NOT call real LLMs yet — agents return a dummy action
- Agent thinking is simulated (random action from behavior_patterns)
- This keeps the phase testable without network/API keys

## Verification

- `cargo build` zero error
- `cargo test` all pass
- `cargo clippy -- -D warnings` clean
