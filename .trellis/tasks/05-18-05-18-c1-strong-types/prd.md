# PRD — Phase C1: serde_json::Value → Strong Types

> Priority: C (medium-high risk) | Spec: .trellis/spec/backend/phase-c1-strong-types.md
> Tests: no net new tests expected; existing 310 must pass at each phase

## Goal

Replace `serde_json::Value` in core data structures with Rust strong types,
while keeping `Value` for LLM prompt construction and API responses.

## Three-Phase Approach

### Phase 1 — Add typed fields alongside Value (backward compat)

Files: `src/hongmeng/agent.rs`, `src/hongmeng/board.rs`, `src/models.rs`

1. Add `AgentPrivateState` struct to `src/models.rs`:
   - `objectives: Vec<String>`
   - `memory: BTreeMap<String, String>`
   - `numeric_state: BTreeMap<String, f64>`
   - Derive Serialize/Deserialize/Debug/Clone/Default

2. Add `agent.private_state_typed: Option<AgentPrivateState>` to `Agent`
   - Default: None
   - When Some, `private_state: Value` is populated from it

3. Add `StickValue` enum:
   ```rust
   enum StickValue { Text(String), Number(f64), Flag(bool), List(Vec<String>) }
   ```

4. Add `StickEntry.typed_value: Option<StickValue>`

Verification: `cargo build && cargo test` — all 310 pass

### Phase 2 — Migrate consumers

Files: `src/hongmeng/simulation.rs`, `src/hongmeng/agent.rs`, `src/tui/state.rs`

5. `set_stick()` accepts `StickValue`, writes both `.value` and `.typed_value`
6. `build_llm_prompt()` reads `private_state_typed` when Some
7. `compact_json_value()` handles `StickValue` variants
8. Delta computation uses typed fields where available

Verification: delta output identical to pre-migration on sample_feed.xml

### Phase 3 — Remove Value fields

Files: `src/hongmeng/agent.rs`, `src/hongmeng/board.rs`

9. Delete `private_state: Value` from Agent
10. Delete `value: Value` from StickEntry
11. Clean up `#[serde(skip)]` markers
12. Bump schema version constant

Verification: `cargo build && cargo test && cargo clippy -- -D warnings`

## Key Files

| Action | File | Lines |
|--------|------|-------|
| Add AgentPrivateState | src/models.rs | +30 |
| Add StickValue | src/hongmeng/board.rs | +20 |
| Add typed fields | src/hongmeng/agent.rs | ~10 |
| Migrate set_stick | src/hongmeng/simulation.rs | ~5 |
| Migrate prompt builder | src/hongmeng/agent.rs | ~15 |
| TUI format for StickValue | src/tui/state.rs | ~15 |
| Phase 3 cleanup | agent.rs, board.rs | -10 |

## Commands

```bash
# After each phase:
cargo build
cargo test               # 310 pass
cargo clippy -- -D warnings
cargo fmt
```
