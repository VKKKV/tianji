# Phase F2 — API Contract Fixtures

## Goal

Add stable, mocked/dry-run contract tests for Phase D/E user-facing surfaces so future refactors cannot silently change JSON envelopes, signed command semantics, alert payload shapes, or TUI replay metadata.

## Scope

1. API contracts
   - `/api/v1/meta` envelope shape and core fields.
   - `/api/v1/agent/command` accepted/rejected command envelopes.
   - HMAC tests must use deterministic test secrets and never log raw credentials.

2. Alert dispatch contracts
   - Dry-run delivery reports for Telegram/Discord/webhook.
   - Payload builder/body shape for generic webhook.
   - Redaction must remain part of the contract.

3. TUI replay contracts
   - Stable formatting/metadata for replay cursor/frame position.
   - Existing keybinding semantics preserved.

4. Fixtures
   - Prefer checked-in static fixture JSON/YAML where useful.
   - Keep external services mocked or dry-run only.
   - Avoid environment-dependent live network/model calls.

## Verification

- `cargo fmt`
- `cargo test --quiet contract`
- `cargo test --quiet api`
- `cargo test --quiet alert_dispatch`
- `cargo test --quiet tui`
- `cargo test --quiet`
- `cargo clippy -- -D warnings`
- `git diff --check`
