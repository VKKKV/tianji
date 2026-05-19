# Phase E2 — Structured Agent Output / Auditability

## Goal

Make Hongmeng agent decisions more auditable by enriching `AgentAction` with structured analysis zones while preserving backward-compatible JSON behavior.

## Scope

Add structured fields to agent actions:

- `assessment`: concise natural-language assessment of the situation.
- `category`: machine-readable decision category.
- `drivers`: ordered list of causal/reasoning drivers.

Keep existing fields:

- `action_type`
- `target`
- `board_message`
- `confidence`
- `rationale`

## Contract

`AgentAction` JSON should accept old records that lack the new fields.

Default values:

```json
{
  "assessment": "",
  "category": "uncategorized",
  "drivers": []
}
```

LLM action output should accept both old and new JSON envelopes. New recommended LLM shape:

```json
{
  "action_type": "diplomatic_signal",
  "target": "china",
  "board_message": "We seek talks.",
  "confidence": 0.82,
  "rationale": "de-escalation",
  "assessment": "Escalation risk is rising but still controllable.",
  "category": "diplomacy",
  "drivers": ["public pressure", "alliance signaling"]
}
```

## Requirements

1. Add fields to `AgentAction` with serde defaults.
2. Update stub actions to produce deterministic structured metadata.
3. Update LLM system prompt to request structured output.
4. Parse structured fields from LLM JSON, with sane defaults and trimming.
5. Preserve existing tests and old JSON compatibility.
6. Add tests for:
   - old action JSON deserializes
   - LLM structured fields parse
   - blank/missing structured fields default safely
   - stub action includes deterministic audit fields

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
