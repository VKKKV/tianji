# Phase F4 — Release Readiness Check

## Goal

Verify TianJi is release-ready as a local-first single-binary Rust tool and record the result in a concise checked-in release checklist.

## Scope

1. Release build
   - Run `cargo build --release`.
   - Confirm release binary exists at `target/release/tianji`.
   - Measure byte size with `stat -c '%s %n' target/release/tianji`.
   - Pass target: single binary < 25 MB.

2. Shell completions
   - Verify completion generation for at least `fish`.
   - Prefer checking all supported shells if cheap: `bash`, `zsh`, `fish`.
   - Write outputs to `/tmp`, not the repo.

3. Fixture smoke run
   - Run a local fixture smoke command without network or LLM.
   - Example: `cargo run --quiet -- run --fixture tests/fixtures/sample_feed.xml >/tmp/tianji-run.json`.
   - Confirm output is valid JSON and has the expected artifact schema/version fields.

4. Regression gate
   - Run `cargo test --quiet`.
   - Run `cargo clippy -- -D warnings`.
   - Run `git diff --check`.

5. Checklist artifact
   - Add a concise repo checklist file, preferably `RELEASE_CHECKLIST.md`.
   - Include exact commands run, pass/fail status, measured binary size, and local-first/security notes.
   - Do not include real secrets, personal paths, external service credentials, or live webhook/model calls.

## Non-goals

- No live LLM endpoint test unless explicitly requested.
- No external alert dispatch.
- No publishing/release tag creation.
- No git push.

## Verification

- `cargo build --release`
- `stat -c '%s %n' target/release/tianji`
- `cargo run --quiet -- completions fish >/tmp/tianji.fish`
- `cargo run --quiet -- run --fixture tests/fixtures/sample_feed.xml >/tmp/tianji-run.json`
- JSON/schema smoke check on `/tmp/tianji-run.json`
- `cargo test --quiet`
- `cargo clippy -- -D warnings`
- `git diff --check`
