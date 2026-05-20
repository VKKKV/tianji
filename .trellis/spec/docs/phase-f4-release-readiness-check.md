# Phase F4 — Release Readiness Check

## Purpose

Keep release readiness checks reproducible, local-first, and credential-free.

## Checklist rules

1. Release build
   - Use Cargo release mode: `cargo build --release`.
   - Binary path is `target/release/tianji`.
   - Size target is `< 25 MB` measured in bytes.
   - Report the exact byte count, not a rounded-only estimate.

2. Smoke tests
   - Fixture smoke runs must use checked-in fixtures and no network calls.
   - Store transient outputs under `/tmp`, not in the repo.
   - Validate JSON output with a tool (`python3`, `jq`, or equivalent), not by eyeballing.

3. Completions
   - Generate shell completions through the shipped CLI command.
   - At minimum verify `fish`; if possible verify `bash`, `zsh`, and `fish`.
   - Do not install completions into the user shell during readiness checks.

4. Safety
   - Do not start live LLM services for F4.
   - Do not call external alert endpoints.
   - Do not include secrets or user-specific machine paths in the checklist.

5. Checklist format
   - Keep `RELEASE_CHECKLIST.md` concise and terminal-readable.
   - Include:
     - date
     - release readiness scope
     - commands run
     - measured binary size and pass/fail against 25 MB
     - smoke output schema/version summary
     - notes on local-first/no-secrets/no-external-services

## Required local gate

```bash
cargo build --release
stat -c '%s %n' target/release/tianji
cargo run --quiet -- completions fish >/tmp/tianji.fish
cargo run --quiet -- run --fixture tests/fixtures/sample_feed.xml >/tmp/tianji-run.json
cargo test --quiet
cargo clippy -- -D warnings
git diff --check
```
