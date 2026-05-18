# Phase C3 — forward.rs Deduplication

> Status: spec | Risk: medium | Files: 2 | ~200 lines removed

## Current State

`src/nuwa/forward.rs` (571 lines) has two functions with ~80% shared loop:

- `run_forward` (line 12, 200 lines): headless forward simulation, returns `SimulationOutcome`
- `run_interactive_forward` (line 381, 190 lines): TUI-connected forward sim, sends `SimUpdate` via channel

Shared logic:
1. Agent action picking loop: `pick_llm_action_with_fallback` / `pick_stub_action`
2. Delta application: `generate_delta` → field updates
3. Divergence computation: `compute_divergence_from`
4. Convergence checks: field target reached / field stabilized / max ticks

Differences:
- `run_forward`: accumulates `delta_history`, `event_sequence`; builds `WorldlineBranch` at end; returns `SimulationOutcome`
- `run_interactive_forward`: builds `SimField`/`SimAgent`/`BranchSummary` per tick; sends via `mpsc::Sender`; handles `PruneRequest` on interval

## Design — `tick_simulation` core function

Extract the pure simulation tick logic:

```rust
struct TickInput<'a> {
    tick: u64,
    worldline: &mut Worldline,
    agents: &mut [Agent],
    provider: Option<&'a ProviderRegistry>,
    config: &'a HongmengConfig,
}

struct TickOutput {
    agent_ids: Vec<ActorId>,
    action_types: Vec<String>,
    field_changes: Vec<FieldChange>,
    // run_forward uses these for branch construction
    // run_interactive_forward builds SimField/SimAgent from these
}

fn tick_simulation(input: TickInput<'_>) -> TickOutput { ... }
```

Then:

```rust
// run_forward uses tick_simulation in its loop
loop {
    let output = tick_simulation(TickInput { tick, ... });
    // accumulate delta_history, event_sequence
    // convergence checks
    // break
}
// build SimulationOutcome from accumulated data

// run_interactive_forward uses the same tick_simulation
loop {
    let output = tick_simulation(TickInput { tick, ... });
    // build SimField/SimAgent from output
    // send via channel
    // prune check
    // convergence checks
    // break
}
```

### What moves into tick_simulation

Lines shared by both functions:
1. Agent loop: `pick_llm_action_with_fallback` / `pick_stub_action` (lines 52-70 in run_forward, 415-433 in interactive)
2. `generate_delta(tick, &agent_ids, &action_types)` (line 72 / 435)
3. Field update loop: apply delta to worldline (lines 74-92 / 438-443)
4. Hash/divergence update (lines 94-95 / 445-446)

### What stays in each caller

- `run_forward`: convergence checks (lines 97-117), branch building (lines 122-129)
- `run_interactive_forward`: field/agent/branch formatting for TUI (lines 449-516), channel send (lines 531-546), convergence checks (lines 549-567)

## Files Changed

- `src/nuwa/forward.rs` — extract `tick_simulation`, refactor both callers
- No new files needed (tick_simulation stays in forward.rs as private)

## Verification

```bash
cargo build && cargo test && cargo clippy -- -D warnings
# All 310 tests pass
# cargo run -- tui --simulate global.conflict:5 --db runs/tianji.sqlite3 — identical behavior
# cargo run -- predict --field global.conflict --horizon 3 — identical output
```
