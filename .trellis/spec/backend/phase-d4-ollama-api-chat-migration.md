# Phase D4 — Ollama /api/chat Migration

## Goal

Migrate TianJi's Ollama provider from string-flattened `/api/generate` prompts to structured `/api/chat` messages while preserving provider registry compatibility, deterministic tests, and existing Hongmeng agent call sites.

The current LLM client already exposes a generic `chat(messages, model)` API, and OpenAI-compatible providers already send structured `messages`. Ollama is the exception: `chat_ollama` flattens messages into a synthetic prompt and posts to `/api/generate`. D4 aligns Ollama with the chat contract by posting the same role/content structure to `/api/chat`.

## Scope

In scope:

- `src/llm/client.rs`
- Tests for Ollama request construction / response parsing using deterministic local test servers or existing test helpers
- Minimal re-exports or helper methods only if needed
- Hongmeng agent compatibility verification where useful

Out of scope:

- Real Ollama integration tests requiring a running daemon
- Streaming responses
- Tool calling
- Provider registry schema changes
- LLM concurrency limiting (Phase D5)
- Prompt redesign in Hongmeng / Nuwa

## Requirements

1. Ollama provider must use `/api/chat`.
   - URL: `{base_url.trim_end_matches('/')}/api/chat`
   - Request body includes:
     - `model`
     - `messages: [{ role, content }, ...]`
     - `stream: false`
   - Do not flatten messages into a single prompt.

2. Preserve generic client API.
   - `LlmClient::chat(messages, model)` signature remains stable.
   - `ChatMessage { role, content }` remains the shared message type.
   - OpenAI-compatible request behavior is unchanged.

3. Parse Ollama chat responses.
   - Prefer `message.content` from response JSON:
     ```json
     { "message": { "role": "assistant", "content": "..." } }
     ```
   - Empty/missing content may return an empty string only if that matches existing OpenAI behavior; otherwise return a clear `LlmError::ChatFailed`.
   - HTTP non-success responses must include status code and body in the error.

4. Preserve deterministic test behavior.
   - No test may require network access to a real Ollama instance.
   - Existing unreachable-Ollama test may remain, but add deterministic tests that bind a local HTTP listener and inspect the request path/body.
   - Tests should prove roles are preserved, especially system/user messages used by `hongmeng::agent::Agent::pick_llm_action_with_fallback`.

5. Avoid secret leakage.
   - Do not log API keys or provider config secrets.
   - Ollama normally has no API key; keep it that way.

## Acceptance Criteria

- Ollama chat calls POST `/api/chat`, not `/api/generate`.
- Request body preserves the input `ChatMessage` order and roles.
- Response parser returns assistant `message.content`.
- OpenAI-compatible tests still pass.
- `cargo fmt` passes.
- `cargo test --quiet` passes.
- `cargo clippy -- -D warnings` passes.

## Implementation Notes

Suggested implementation:

- In `chat_ollama`, remove prompt flattening.
- Build `messages_json` similarly to `chat_openai_compatible`:
  ```rust
  let msgs: Vec<serde_json::Value> = messages
      .into_iter()
      .map(|m| json!({ "role": m.role, "content": m.content }))
      .collect();
  ```
- POST to `/api/chat` with JSON body:
  ```json
  { "model": model, "messages": msgs, "stream": false }
  ```
- Parse `json["message"]["content"]`.

Testing approach:

- Use a local `TcpListener` in a Tokio test if no existing HTTP mock helper is present.
- Accept a single request, read request bytes, assert path contains `POST /api/chat`, assert body JSON contains the expected `messages` array, then write an HTTP 200 JSON response with assistant content.
- Keep the test small and deterministic.

## Verification Commands

```bash
cargo fmt
cargo test --quiet llm::client
cargo test --quiet
cargo clippy -- -D warnings
git diff --check
```
