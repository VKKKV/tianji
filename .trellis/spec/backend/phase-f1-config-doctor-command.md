# Phase F1 — Config Doctor Command

## Goal

Add a safe operator-facing `tianji doctor` command that validates local configuration readiness without leaking secrets.

## Scope

1. CLI
   - Add `tianji doctor [--config <PATH>] [--sqlite-path <PATH>] [--json]`.
   - Default config path should match `TianJiConfig::default_path()`.

2. Checks
   - Config file presence and parse status.
   - Provider count and per-provider shape.
   - `api_key_env` presence should check whether the env var is set/non-empty.
   - Inline `api_key` should be reported as present but never printed.
   - `max_concurrency` must be at least 1.
   - Fallback provider names must refer to configured providers.
   - Agent model map entries must refer to configured providers.
   - Optional sqlite path parent directory should exist and be writable or creatable according to existing behavior.

3. Output
   - Human output by default.
   - JSON output with `--json` for automation.
   - Never include API keys, webhook URLs with secrets, or raw credential values.

4. Tests
   - Missing config should be warning/not fatal.
   - Malformed YAML should fail.
   - Missing env vars should be reported without leaking values.
   - Valid minimal config should pass.

## Verification

- `cargo fmt`
- `cargo test --quiet doctor`
- `cargo test --quiet`
- `cargo clippy -- -D warnings`
- `git diff --check`
