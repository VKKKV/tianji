# Phase D2 — ActorProfile YAML Validation

## Goal

Reject malformed actor profiles at load time so invalid strategic inputs do not
enter Hongmeng orchestration or Nuwa simulation.

This is a validation hardening task only. It must not change the profile YAML
format, profile semantics, simulation behavior, or introduce new dependencies.

## Scope

Files expected to change:

- `src/profile/types.rs`
- `src/profile/registry.rs`

Likely test locations:

- existing tests in `src/profile/types.rs`
- existing tests in `src/profile/registry.rs`

No database, daemon, TUI, LLM, or Nuwa changes.

## Design

Add a validation method on `ActorProfile`:

```rust
impl ActorProfile {
    pub fn validate(&self) -> Result<(), TianJiError> { ... }
}
```

If importing `TianJiError` into `profile/types.rs` creates an undesirable module
cycle or style issue, use a small profile-local validation helper returning
`Result<(), String>` and convert to `TianJiError` in `registry.rs`. Prefer the
simplest design that preserves clean module boundaries and passes clippy.

## Validation Rules

### Required strings

- `ActorProfile.id.trim()` must not be empty.
- `ActorProfile.name.trim()` must not be empty.
- Each `Interest.goal.trim()` must not be empty.

Do not trim/normalize stored values in this task. Validation rejects bad input;
it does not rewrite it.

### Interests

- `ActorProfile.interests` must not be empty.
- Each `Interest.salience` must be finite.
- Each `Interest.salience` must be in `[0.0, 1.0]` inclusive.

### Capabilities

Validate every capability field:

- `military`
- `economic`
- `technological`
- `diplomatic`
- `cyber`

Each value must be:

- finite
- in `[0.0, 1.0]` inclusive

### Error messages

Validation errors must include enough context for a human to fix the YAML.
At minimum include:

- profile id when available, or the profile path from registry loading
- invalid field name, e.g. `interests[0].salience`, `capabilities.cyber`
- invalid value or reason where useful, e.g. `must be in [0.0, 1.0]`

Use an existing error variant. `TianJiError::Usage` is acceptable for invalid
user-supplied YAML. If a profile-specific error variant already exists by the
time this task is implemented, use that instead.

## Registry Contract

`ProfileRegistry::load_from_dir` must validate each parsed YAML file immediately
after deserialization and before insertion into the `BTreeMap`.

Invalid profile behavior:

- return `Err`
- do not skip invalid profiles
- do not clamp invalid values
- do not log-and-continue
- include file path context in the error, if available

Duplicate ids retain the existing behavior unless OpenCode discovers an explicit
existing test/spec saying otherwise. This task is validation only.

## Non-goals

- No schema migration.
- No dynamic profile validation.
- No cross-scenario memory validation.
- No changing YAML keys or examples.
- No normalizing/clamping invalid values.
- No warning-only mode.
- No new dependency.

## Tests

Add tests for both direct validation and registry loading when practical.
Required cases:

- Existing valid sample profiles still load.
- Empty `id` is rejected.
- Empty `name` is rejected.
- Empty `interests` is rejected.
- Empty `Interest.goal` is rejected.
- `Interest.salience < 0.0` is rejected.
- `Interest.salience > 1.0` is rejected.
- Non-finite salience is rejected via direct struct validation (`f64::NAN` and/or `f64::INFINITY`).
- At least one capability `< 0.0` is rejected.
- At least one capability `> 1.0` is rejected.
- Non-finite capability is rejected via direct struct validation.
- Registry load error includes useful context: path or profile id plus field name.

Prefer small focused tests over a large table that hides which field failed.

## Verification

OpenCode must run:

```bash
cargo fmt
cargo test --quiet
cargo clippy -- -D warnings
```

Expected baseline before this task: 321 tests passing. The exact test count may
increase after adding validation tests.

## OpenCode Stop Condition

Stop after implementation and verification. Do not commit; the user will inspect
and decide whether to commit.
