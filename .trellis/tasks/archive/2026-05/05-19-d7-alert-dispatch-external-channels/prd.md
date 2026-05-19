# PRD — Phase D7: Alert Dispatch to External Channels

> Priority: D7 | Spec: `.trellis/spec/backend/phase-d7-alert-dispatch-external-channels.md`

## Goal

Add a safe alert dispatcher for Telegram, Discord, and generic webhooks, driven by existing `AlertTier`.

## Requirements

1. New module:
   - `src/alert_dispatch.rs`
   - exported from `src/lib.rs`

2. Config and policy:
   - configure channels
   - map `Flash` / `Priority` / `Routine` to channel names
   - support dry-run mode

3. Channels:
   - Telegram sendMessage payload
   - Discord webhook content payload
   - Generic webhook structured JSON payload

4. Safety:
   - redact bot tokens and webhook URLs in human/debug outputs
   - no network calls in dry-run
   - errors must not leak secrets
   - chunk Telegram at 4096 chars and Discord at 2000 chars

5. Tests:
   - dry-run plans deliveries
   - tier policy filters channels
   - Telegram/Discord chunking deterministic
   - mock HTTP captures payloads for all channel types
   - redaction test

## Allowed Files

- `src/alert_dispatch.rs`
- `src/lib.rs`
- minimal docs/tests if needed

## Verification

Run:

```bash
cargo fmt
cargo test --quiet alert_dispatch
cargo test --quiet
cargo clippy -- -D warnings
git diff --check
```

## Completion Output

```text
DEV_DONE_D7 <summary>
```

or

```text
NEED_INPUT_D7 <reason>
```
