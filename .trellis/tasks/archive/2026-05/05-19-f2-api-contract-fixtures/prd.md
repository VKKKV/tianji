# PRD — Phase F2: API Contract Fixtures

## Goal

Lock the Phase D/E API, alert dispatch, and TUI replay contracts with tests/fixtures.

## Acceptance Criteria

- `/api/v1/meta` response envelope is covered by a stable contract test.
- `/api/v1/agent/command` covers at least one accepted signed command and one rejection path.
- Alert dispatch dry-run or mocked HTTP payload shape is covered without external network.
- Secret redaction remains covered by contract assertions.
- TUI replay formatting/metadata has a stable test.
- Fixture files, if added, contain no real secrets and use `[REDACTED]` or test-only dummy values.
- Verification passes:
  - `cargo fmt`
  - `cargo test --quiet contract`
  - `cargo test --quiet api`
  - `cargo test --quiet alert_dispatch`
  - `cargo test --quiet tui`
  - `cargo test --quiet`
  - `cargo clippy -- -D warnings`
  - `git diff --check`
