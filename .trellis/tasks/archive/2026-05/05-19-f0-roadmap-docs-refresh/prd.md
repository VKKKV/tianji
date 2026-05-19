# PRD — Phase F0: Roadmap and Docs Refresh

## Goal

Refresh TianJi docs after completing Phase E and set the next Phase F direction.

## Acceptance Criteria

- `plan.md` no longer presents completed ShadowBroker borrowings as pending work.
- `plan.md` defines concrete Phase F targets.
- `README.md` reflects the current implemented Rust/LLM/API/TUI feature set and current test counts.
- No credentials, tokens, private hostnames, or personal paths are introduced.
- Verification passes:
  - `git diff --check`
  - `cargo test --quiet`
  - `cargo clippy -- -D warnings`
