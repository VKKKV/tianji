# PRD — Phase C3: forward.rs Deduplication

> Priority: C (medium risk) | Spec: .trellis/spec/backend/phase-c3-forward-dedup.md
> Removes ~200 lines of duplicated simulation loop logic

## Goal

Extract `tick_simulation` core from `run_forward` and `run_interactive_forward`,
which share ~80% of their simulation loop.

## Steps

### Step 1 — Define TickInput / TickOutput

File: `src/nuwa/forward.rs` (private, near top)

```rust
struct TickInput<'a> {
    tick: u64,
    worldline: &mut Worldline,
    agents: &mut [Agent],
    provider: Option<&'a ProviderRegistry>,
}

struct TickOutput {
    agent_ids: Vec<ActorId>,
    action_types: Vec<String>,
    field_changes: Vec<FieldChange>,
}
```

### Step 2 — Extract tick_simulation function

Move these shared steps into `fn tick_simulation(input: TickInput<'_>) -> TickOutput`:

1. Agent loop: for each agent, `pick_llm_action_with_fallback` or `pick_stub_action`,
   push to `action_types` and `agent_ids`, push action to `agent.action_history`.
2. `generate_delta(tick, &agent_ids, &action_types)`.
3. Apply delta to worldline fields (for each FieldChange, update field value).
4. Update `worldline.snapshot_hash` and `worldline.divergence`.

Return `TickOutput { agent_ids, action_types, field_changes }`.

### Step 3 — Refactor run_forward

Replace inline loop body with `tick_simulation()` call.
Keep convergence checks, delta_history accumulation, event_sequence building,
and branch construction in run_forward.

### Step 4 — Refactor run_interactive_forward

Replace inline loop body with `tick_simulation()` call.
Keep TUI formatting (SimField/SimAgent/BranchSummary), channel send,
prune handling, and convergence checks.

### Step 5 — Verify convergence checks identical

Both functions share convergence logic but with slight differences:
- `run_forward`: uses `config.convergence_epsilon` explicitly
- `run_interactive_forward`: uses same epsilon but doesn't pass `config` explicitly?

Check: both callers pass `&HongmengConfig`. Make sure epsilon check is consistent.

## Key Files

| Action | File | Lines changed |
|--------|------|---------------|
| Add TickInput/Output | src/nuwa/forward.rs | +30 |
| Extract tick_simulation | src/nuwa/forward.rs | +50 (from existing) |
| Shrink run_forward | src/nuwa/forward.rs | -60 |
| Shrink run_interactive_forward | src/nuwa/forward.rs | -70 |
| Net | | ~-50 |

## Commands

```bash
cargo build && cargo test && cargo clippy -- -D warnings && cargo fmt
# smoke predict: cargo run -- predict --field global.conflict --horizon 3
# smoke tui sim: cargo run -- tui --simulate global.conflict:5 --db runs/tianji.sqlite3
# output must be identical to pre-refactor
```
