# Phase D7 — Alert Dispatch to External Channels

## Goal

Add a safe, testable alert dispatch module that maps TianJi `AlertTier` values to external delivery channels: Telegram, Discord, and generic webhooks.

D7 must provide production-shaped request payloads while keeping tests offline and preventing secret leakage in logs/errors/debug output.

## Scope

In scope:

- New `src/alert_dispatch.rs`
- Export module from `src/lib.rs`
- Minimal daemon/API integration only if straightforward
- Config structs and dry-run dispatch behavior
- Chunking for platform limits
- Offline tests using local mock HTTP server or reqwest client injection if feasible

Out of scope:

- Real Telegram/Discord credentials
- Durable retry queue
- UI configuration editor
- User-facing command to send live alerts, unless trivial
- Secret storage outside existing config patterns

## Alert Tiers

Use existing `AlertTier`:

- `Flash`
- `Priority`
- `Routine`

Dispatch policy must be configurable per tier. A tier may dispatch to zero or more channels.

## Channel Types

Support:

1. Telegram bot API
   - Config: bot token, chat id, optional thread/topic id
   - Endpoint shape: `/bot<TOKEN>/sendMessage`
   - Payload includes `chat_id`, `text`, optional `message_thread_id`

2. Discord webhook
   - Config: webhook URL
   - Payload includes `content`

3. Generic webhook
   - Config: URL, optional static headers if easy/safe
   - Payload includes structured JSON with tier, title, summary, body

## Safety Requirements

- No secret values in `Debug` output, error messages, or dry-run payload summaries.
- Redact Telegram bot token and webhook URLs when formatting configs/results for humans.
- Dry-run mode must not make network calls.
- Dispatch errors should include channel kind/name and HTTP status/body snippet, but not secret URL/token.
- Chunk long messages by platform limit.
  - Telegram safe text limit: 4096 chars
  - Discord safe content limit: 2000 chars
  - Generic webhook can send full structured body unless configured otherwise

## API Shape

Suggested public API:

```rust
pub struct AlertDispatchConfig { ... }
pub struct AlertMessage { tier: AlertTier, title: String, summary: String, body: String }
pub struct AlertDispatcher { ... }
impl AlertDispatcher {
    pub fn new(config: AlertDispatchConfig) -> Self;
    pub async fn dispatch(&self, message: &AlertMessage) -> Result<DispatchReport, TianJiError>;
}
```

Exact names may vary, but tests and module should clearly expose:

- config parse/construct
- tier-to-channel policy
- dry-run report
- real HTTP dispatch via local mock server

## Acceptance Criteria

- `src/alert_dispatch.rs` exists and is exported.
- `AlertTier` can be mapped to Telegram/Discord/webhook channels.
- Dry-run mode returns planned deliveries without network.
- Telegram/Discord messages are chunked deterministically.
- Secret-bearing configs/results redact tokens and webhook URLs in debug/human output.
- Local tests verify request payload shapes without external services.
- `cargo fmt` passes.
- `cargo test --quiet alert_dispatch` passes.
- `cargo test --quiet` passes.
- `cargo clippy -- -D warnings` passes.
- `git diff --check` passes.
