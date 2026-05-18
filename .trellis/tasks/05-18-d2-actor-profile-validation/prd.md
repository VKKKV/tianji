# PRD — Phase D2: ActorProfile YAML Validation

> Priority: D2 | Spec: `.trellis/spec/backend/phase-d2-actor-profile-validation.md`

## Goal

Add strict validation for actor profiles loaded from YAML so malformed strategic
inputs are rejected before Hongmeng/Nuwa use them.

## Why Now

The core product and Phase A/B/C hardening are complete. D1 storage integration
coverage is complete. The next low-risk production-hardening item in `plan.md` is
D2: ActorProfile YAML validation.

Current code already has:

- `ActorProfile`, `Interest`, `Capabilities` in `src/profile/types.rs`
- `ProfileRegistry::load_from_dir` in `src/profile/registry.rs`
- existing profile parser/registry tests

The missing piece is explicit semantic validation of bounded numeric fields and
required string/list fields.

## Requirements

Implement validation for `ActorProfile` loaded from YAML.

Validation rules:

1. Required string fields:
   - `id.trim()` is non-empty
   - `name.trim()` is non-empty
   - each `Interest.goal.trim()` is non-empty
2. Interests:
   - `interests` is non-empty
   - each `Interest.salience` is finite
   - each `Interest.salience` is in `[0.0, 1.0]`
3. Capabilities:
   - `military`, `economic`, `technological`, `diplomatic`, `cyber`
   - each is finite
   - each is in `[0.0, 1.0]`
4. Registry behavior:
   - validate after YAML deserialization
   - validate before inserting into the `BTreeMap`
   - invalid profile returns `Err`
   - invalid profile is not skipped or clamped
5. Error messages:
   - include path or profile id
   - include invalid field name
   - include reason/range where useful

## Non-goals

- Do not change YAML format.
- Do not normalize/clamp invalid values.
- Do not introduce new dependencies.
- Do not modify daemon, DB, TUI, LLM, Hongmeng orchestration, or Nuwa simulation.
- Do not commit; user will manually inspect and commit.

## Acceptance Criteria

- Existing valid profiles still load.
- Invalid profile YAML fails during `ProfileRegistry::load_from_dir`.
- Direct struct validation rejects NaN/infinity numeric values.
- Error output is actionable enough to locate the bad field.
- `cargo fmt` passes.
- `cargo test --quiet` passes.
- `cargo clippy -- -D warnings` passes.

## Suggested Implementation Shape

Preferred:

```rust
impl ActorProfile {
    pub fn validate(&self) -> Result<(), TianJiError> { ... }
}
```

Alternative if importing `TianJiError` into `types.rs` is awkward:

```rust
impl ActorProfile {
    pub fn validate(&self) -> Result<(), String> { ... }
}
```

Then convert the string into `TianJiError::Usage(...)` in `registry.rs`, adding
path context there.

Helper idea:

```rust
fn validate_unit_interval(value: f64, field: &str, profile_id: &str) -> Result<(), String> {
    if !value.is_finite() { ... }
    if !(0.0..=1.0).contains(&value) { ... }
    Ok(())
}
```

## Tests Required

Add focused tests for:

- valid existing sample profiles still load
- empty id rejected
- empty name rejected
- empty interests rejected
- empty interest goal rejected
- salience < 0 rejected
- salience > 1 rejected
- salience NaN/infinity rejected via direct struct validation
- capability < 0 rejected
- capability > 1 rejected
- capability NaN/infinity rejected via direct struct validation
- registry load error includes field/path context

## Verification Commands

```bash
cargo fmt
cargo test --quiet
cargo clippy -- -D warnings
```

## OpenCode Prompt

```bash
cd /home/kita/code/tianji

opencode run "Implement Phase D2: ActorProfile YAML validation.

Active task: .trellis/tasks/05-18-d2-actor-profile-validation

You are already the trellis-implement sub-agent. Implement directly; do not spawn sub-agents.

Read:
- .trellis/tasks/05-18-d2-actor-profile-validation/prd.md
- .trellis/spec/backend/phase-d2-actor-profile-validation.md
- .trellis/spec/backend/phase-2.3-actor-profiles.md
- src/profile/types.rs
- src/profile/registry.rs

Requirements:
1. Add validation for ActorProfile loaded from YAML.
2. Validate:
   - id is non-empty after trim
   - name is non-empty after trim
   - interests is non-empty
   - each Interest.goal is non-empty after trim
   - each Interest.salience is finite and in [0.0, 1.0]
   - every Capabilities field is finite and in [0.0, 1.0]
3. Call validation in ProfileRegistry::load_from_dir immediately after YAML deserialization and before inserting into the registry.
4. Error behavior:
   - invalid profile returns Err(TianJiError::Usage or existing appropriate variant)
   - error message must include profile path or profile id and invalid field name
   - do not silently skip invalid files
5. Preserve existing valid profile loading behavior and deterministic BTreeMap ordering.
6. Add tests:
   - valid existing profiles still load
   - salience < 0 rejected
   - salience > 1 rejected
   - non-finite salience rejected through direct struct validation
   - capability < 0 rejected
   - capability > 1 rejected
   - non-finite capability rejected through direct struct validation
   - empty id/name/goal/interests rejected
   - invalid registry load error includes useful context
7. Do not modify unrelated modules.
8. Run:
   - cargo fmt
   - cargo test --quiet
   - cargo clippy -- -D warnings

Stop after implementation and verification. Do not commit." \
  --model kita/gpt-5.5
```
