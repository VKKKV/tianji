# Phase C1 — serde_json::Value → Strong Types

> Status: spec | Risk: medium-high | Files: ~8 | Est: 3 phases

## Current State

`serde_json::Value` appears in ~30 locations across 8 files. By role:

| Role | File | Lines | Migrate? |
|------|------|-------|----------|
| Agent runtime state | hongmeng/agent.rs:59 | `private_state: Value` | Yes — typed bag |
| Board stick entries | hongmeng/board.rs:29 | `value: Value` | Yes — TypedStickValue |
| Simulation stick set | hongmeng/simulation.rs:111 | `set_stick(..., value: Value)` | Yes — follow board |
| LLM prompt construction | hongmeng/agent.rs:152-210 | `Vec<Value>` for json!() | No — serializer helper |
| LLM API messages | llm/client.rs:83,120,182 | `Vec<Value>`, `json!()` | No — API contract |
| TUI formatting | tui/state.rs:419-432 | `compact_json_value`, `string_field` | Partial — reader only |
| Delta/storage | delta.rs, storage.rs, delta_memory.rs | RunArtifact fields | Yes — core data |
| API response | api.rs:64-126 | `JsonValue` envelope builders | No — API contract |

## Design

### Phase 1 — Add typed structs alongside Value (backward compat)

1. `AgentPrivateState`: typed struct replacing `Value::Null`
   - Fields: `objectives: Vec<String>`, `memory: BTreeMap<String, String>`, `numeric_state: BTreeMap<String, f64>`
   - Serialize as nested JSON object
   - Add `agent.private_state_typed: Option<AgentPrivateState>` field
   - `private_state: Value` stays, populated from typed via `serde_json::to_value()`

2. `StickValue`: enum replacing `Value` in board entries
   ```rust
   enum StickValue {
       Text(String),
       Number(f64),
       Flag(bool),
       List(Vec<String>),
   }
   ```
   - Add `StickEntry.typed_value: Option<StickValue>`
   - `value: Value` stays for backward compat

3. Delta/metric types: already partially typed via `MetricSnapshot`, `DeltaSummary`
   — no change needed here yet.

### Phase 2 — Migrate consumers

- `hongmeng/simulation.rs::set_stick` accepts `StickValue`
- `hongmeng/agent.rs::build_llm_prompt` reads from typed fields when present
- `tui/state.rs::compact_json_value` gains `StickValue` formatting
- Delta computation reads from typed fields

### Phase 3 — Remove Value fields

- Delete `private_state: Value`, keep only typed
- Delete `value: Value` on StickEntry, keep only typed
- Clean up `#[serde(skip)]` markers
- Bump schema version

### Compatibility Contract

Removing the legacy fields must not make existing serialized Hongmeng state
unreadable. Keep custom deserializers at the boundary until there is a formal
schema migration.

#### Agent payloads

Accepted inputs:

- `private_state_typed` with the new typed object.
- Legacy `private_state` JSON object.
- Missing private state, which defaults to `AgentPrivateState::default()`.

`private_state` objects map into `AgentPrivateState` as:

- `objectives`: string arrays only; missing or malformed values become `[]`.
- `memory`: string-to-string object entries only; non-string values are skipped.
- `numeric_state`: string-to-number object entries only; non-number values are skipped.

#### StickEntry payloads

Accepted inputs:

- `typed_value` with the new enum representation.
- Legacy `value` JSON scalar/array.
- Missing value, which defaults to `StickValue::Text(String::new())`.

Legacy JSON maps into `StickValue` as:

- String -> `Text`.
- Number -> `Number` when representable as `f64`, otherwise `Text` with JSON text.
- Bool -> `Flag`.
- Array -> `List`, stringifying non-string elements.
- Object/null -> `Text` with JSON text.

Wrong:

```rust
#[derive(Deserialize)]
pub struct StickEntry {
    pub typed_value: StickValue,
}
```

Correct:

```rust
impl<'de> Deserialize<'de> for StickEntry {
    // Accept both typed_value and legacy value at the serialization boundary.
}
```

Required tests:

- `Agent` deserializes a legacy `private_state` JSON payload.
- `StickEntry` deserializes a legacy `value` JSON payload.
- Prompt/API output continues to serialize typed state as JSON only at boundaries.

## Verification

```bash
cargo build && cargo test && cargo clippy -- -D warnings
# Phase 1: all 310 tests pass, no behavioral change
# Phase 2: delta output identical to pre-migration
# Phase 3: no Value fields remain in delta/agent/board structs
```
