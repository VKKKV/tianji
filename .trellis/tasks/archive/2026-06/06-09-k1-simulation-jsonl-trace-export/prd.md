# K1 simulation JSONL trace export

## Purpose

Start Phase K by making Nuwa forward simulations replay/export friendly. Add a stable JSONL trace export path without changing default `predict` stdout behavior.

## Scope

In scope:
- Add canonical trace types for forward simulation frames.
- Add `run_forward_with_trace(...)` that returns the existing `SimulationOutcome` plus a `SimulationTrace`.
- Keep existing `run_forward(...)` as compatibility wrapper returning only `SimulationOutcome`.
- Add JSONL writer/reader helpers for trace export/import.
- Extend `tianji predict` with `--trace-jsonl <PATH>`.
- When `--trace-jsonl` is provided, write trace JSONL and still print the existing final outcome JSON to stdout.
- Add tests for trace serialization, JSONL roundtrip, CLI parse, and predict trace file creation.
- Update README and plan for K1.

Out of scope:
- Replay bundle directory packaging.
- TUI trace loader/navigation changes.
- Structured audit rendering improvements.
- Changing default predict JSON schema.

## Trace contract

Use stable schema names:
- `tianji.sim-trace.v1`
- JSONL record types: `metadata`, `frame`, `completed`

Minimum frame fields:
- `tick`
- `field_values`
- `field_changes`
- `agent_actions` with actor id, action type, confidence, rationale, assessment/category/drivers when available
- `event_sequence_len` or compact event summary

JSONL layout:
1. One metadata line with schema/version, mode, target/horizon, frame_count.
2. One frame line per tick.
3. One completed line with final `SimulationOutcome` or compact completion summary.

## Acceptance criteria

1. Existing `predict` without `--trace-jsonl` preserves stdout schema and tests.
2. `predict --trace-jsonl <PATH>` writes valid JSONL and stdout remains valid final outcome JSON.
3. JSONL reader round-trips exported trace metadata/frame count.
4. Trace has one frame per forward tick and monotonic tick values.
5. Agent audit fields from `AgentAction` are present in frame records.
6. README documents `--trace-jsonl`.
7. plan.md records K1 complete and updates real metrics.

## Verification commands

```bash
cargo test trace
cargo test predict
cargo test nuwa
cargo test
cargo fmt --check
cargo clippy -- -D warnings
git diff --check
```
