# PRD — Phase D4: Ollama /api/chat Migration

> Priority: D4 | Spec: `.trellis/spec/backend/phase-d4-ollama-api-chat-migration.md`

## Goal

Migrate the Ollama LLM provider from `/api/generate` prompt flattening to structured `/api/chat` messages.

## Background

`src/llm/client.rs` already exposes `LlmClient::chat(Vec<ChatMessage>, Option<&str>)`. OpenAI-compatible providers serialize those messages directly to `/chat/completions`, but Ollama currently flattens roles into a single text prompt and posts to `/api/generate`.

Hongmeng agents already pass separate system/user messages. D4 should preserve that structure for Ollama too.

## Requirements

1. Update `chat_ollama` in `src/llm/client.rs`.
   - POST to `/api/chat`.
   - Request body:
     - `model`
     - `messages`
     - `stream: false`
   - Preserve role/content/order from `ChatMessage`.
   - Do not synthesize `[System]` / `[Assistant]` prompt text.

2. Parse Ollama chat response.
   - Return assistant content from `message.content`.
   - Keep clear `LlmError::ChatFailed` errors for HTTP failures and malformed JSON.
   - Include HTTP status/body for non-success responses.

3. Preserve compatibility.
   - Do not change public `LlmClient::chat` signature.
   - Do not alter provider config schema.
   - Do not change OpenAI-compatible request behavior.
   - Do not modify Hongmeng prompt contents unless a test requires a minimal compatibility adjustment.

4. Add deterministic tests.
   - No real Ollama daemon required.
   - Local mock listener/server should assert:
     - request path is `/api/chat`
     - JSON body includes model and messages
     - roles are preserved for system/user/assistant messages
     - response `message.content` is returned
   - Add or update a malformed/non-success response test if straightforward.

## Files Allowed

- `src/llm/client.rs`
- nearby tests in the same module
- `.trellis` task/spec files only if needed for context correction

Do not modify unrelated modules.

## Verification

Run:

```bash
cargo fmt
cargo test --quiet llm::client
cargo test --quiet
cargo clippy -- -D warnings
git diff --check
```

## Completion Output

When implementation and verification are complete, reply with:

```text
DEV_DONE_D4 <summary>
```

If blocked, reply with:

```text
NEED_INPUT_D4 <reason>
```
