# Phase 2.3: Actor Profile System

> Part of plan.md §6.4 Phase 2 Hongmeng
> Target: 3-tier profile loading from YAML, static/dynamic/cross-scenario profiles
> Status: implemented

## Goal

Load actor profiles from `profiles/` YAML files. Three tiers: nation, organization,
corporation. Profiles feed into Hongmeng Agent spawn and Nuwa simulation.

## Profile Directory Structure

```
profiles/
├── nations/
│   ├── china.yaml
│   ├── usa.yaml
│   └── russia.yaml
├── organizations/
│   ├── nato.yaml
│   └── eu.yaml
└── corporations/
    └── huawei.yaml
```

## Profile YAML Format

```yaml
id: china
name: China
tier: nation        # nation | organization | corporation

interests:
  - goal: "maintain territorial integrity in South China Sea"
    salience: 0.95
red_lines:
  - "foreign military presence in Taiwan Strait → full retaliatory posture"
capabilities:
  military: 0.85
  economic: 0.80
  technological: 0.70
  diplomatic: 0.75
  cyber: 0.82
behavior_patterns:
  - "responds to sanctions with proportional counter-sanctions"
  - "prefers economic leverage before military signaling"
historical_analogues:
  - "2016 South China Sea arbitration response"
  - "2017 THAAD deployment → economic retaliation against Lotte"
```

## Rust Types

### src/profile.rs

```rust
pub enum ActorTier {
    Nation,
    Organization,
    Corporation,
}

pub struct Interest {
    pub goal: String,
    pub salience: f64,        // 0.0 - 1.0
}

pub struct Capabilities {
    pub military: f64,
    pub economic: f64,
    pub technological: f64,
    pub diplomatic: f64,
    pub cyber: f64,
}

pub struct ActorProfile {
    pub id: String,
    pub name: String,
    pub tier: ActorTier,
    pub interests: Vec<Interest>,
    pub red_lines: Vec<String>,
    pub capabilities: Capabilities,
    pub behavior_patterns: Vec<String>,
    pub historical_analogues: Vec<String>,
}

pub struct ProfileRegistry {
    pub profiles: BTreeMap<String, ActorProfile>,   // id → profile
}

impl ProfileRegistry {
    pub fn load_from_dir(path: &Path) -> Result<Self, TianJiError>
    pub fn get(&self, id: &str) -> Option<&ActorProfile>
    pub fn of_tier(&self, tier: ActorTier) -> Vec<&ActorProfile>
}
```

## Dynamic Profile (stub)

```rust
pub struct DynamicProfile {
    pub actor_id: String,
    pub temporal_patterns: Vec<String>,     // LLM-extracted patterns
    pub updated_at: chrono::DateTime<chrono::Utc>,
}
```

## Cross-Scenario Memory (stub)

```rust
pub struct CrossScenarioMemory {
    pub actor_id: String,
    pub reputation_score: f64,
    pub relationship_graph: BTreeMap<String, f64>,  // actor_id → score
    pub learned_strategies: Vec<String>,
}
```

## Files

```
src/
├── profile.rs            # mod, re-exports
├── profile/
│   ├── types.rs          # ActorProfile, ActorTier, Interest, Capabilities
│   ├── registry.rs       # ProfileRegistry, YAML loading
│   ├── dynamic.rs        # DynamicProfile (stub)
│   └── memory.rs         # CrossScenarioMemory (stub)
profiles/                  # example profile YAML files
├── nations/
│   └── china.yaml        # example
├── organizations/
│   └── nato.yaml         # example
└── corporations/
    └── huawei.yaml       # example
```

## Tests

- Unit: parse china.yaml → ActorProfile with correct fields
- Unit: parse nato.yaml → organization tier, no military capability
- Unit: parse huawei.yaml → corporation tier, market_share field
- Unit: ProfileRegistry::load_from_dir loads all profiles
- Unit: ProfileRegistry::of_tier filters correctly
- Unit: missing file → error
- Unit: invalid YAML → error

## Verification

- `cargo build` zero error
- `cargo test` all pass
- `cargo clippy -- -D warnings` clean
