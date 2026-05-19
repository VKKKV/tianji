# PRD — Phase E2: Structured Agent Output / Auditability

> Priority: E2 | Spec: `.trellis/spec/backend/phase-e2-structured-agent-output-auditability.md`

## Goal

Enrich Hongmeng `AgentAction` with structured audit metadata while preserving existing action/rationale compatibility.

## Requirements

1. `AgentAction` adds:
   - `assessment: String`
   - `category: String`
   - `drivers: Vec<String>`

2. Compatibility:
   - Old serialized actions without these fields must deserialize successfully.
   - Existing consumers of `action_type`, `confidence`, and `rationale` must keep working.

3. LLM parsing:
   - `LlmActionEnvelope` accepts the new fields.
   - `parse_llm_action` trims strings, clamps confidence, filters blank drivers, and defaults missing values.
   - System prompt asks for the new strict JSON shape.

4. Deterministic fallback:
   - `pick_stub_action` fills structured fields with stable deterministic values.

5. Tests:
   - old action JSON compatibility
   - structured LLM response parsing
   - blank/default handling
   - stub action audit metadata

## Allowed Files

- `src/hongmeng/agent.rs`
- `src/nuwa/forward.rs` only if needed for exposed audit data
- focused tests near affected modules

## Verification

Run:

```bash
cargo fmt
cargo test --quiet agent
cargo test --quiet nuwa
cargo test --quiet
cargo clippy -- -D warnings
git diff --check
```
