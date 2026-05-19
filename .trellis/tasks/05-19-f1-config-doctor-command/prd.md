# PRD — Phase F1: Config Doctor Command

## Goal

Implement `tianji doctor` for operator readiness checks.

## Acceptance Criteria

- CLI exposes `doctor` with `--config`, `--sqlite-path`, and `--json` flags.
- Missing config returns success with a warning because TianJi can run deterministic mode without config.
- Malformed config returns an error.
- Valid config reports providers and agent mappings.
- Missing `api_key_env` env vars are reported as warnings or failures in the report without printing secret values.
- Invalid fallback/provider references are reported.
- Optional sqlite path check reports parent path readiness.
- Tests cover valid, missing, malformed, and missing-env cases.
- Verification passes:
  - `cargo fmt`
  - `cargo test --quiet doctor`
  - `cargo test --quiet`
  - `cargo clippy -- -D warnings`
  - `git diff --check`
