# Phase F0 — Roadmap and Docs Refresh

## Goal

Align user-facing documentation and the authoritative roadmap with the actual Phase D/E implementation state, then define the next Phase F target set.

## Requirements

1. `plan.md`
   - Keep Phase A-E marked complete.
   - Remove or mark completed cross-project borrowing entries that have already landed in D/E.
   - Add a concrete Phase F target list focused on product polish and user-facing readiness.
   - Update verification counts from the latest final test run.

2. `README.md`
   - Replace stale 2026-05-15 / 111-test / no-LLM language.
   - Mention current implemented capabilities: LLM provider config, Hongmeng/Nuwa simulation, daemon API, TUI, alert dispatch, HMAC command channel, replay.
   - Update CLI/API summaries where outdated.
   - Do not add secrets, personal paths, or environment-specific credentials.

3. Keep the refresh documentation-only.

## Verification

- `git diff --check`
- `cargo test --quiet`
- `cargo clippy -- -D warnings`
