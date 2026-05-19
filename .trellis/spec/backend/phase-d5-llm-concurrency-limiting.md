# Phase D5 — LLM Concurrency Limiting

## Goal

Enforce per-provider `max_concurrency` for all live LLM requests using `tokio::sync::Semaphore`, without serializing deterministic/no-provider simulation paths.

TianJi config already exposes `ProviderConfig.max_concurrency`, and `LlmClient` stores it but currently does not use it. D5 turns this field into an actual runtime limit for `LlmClient::chat` calls.

## Scope

In scope:

- `src/llm/client.rs`
- `src/llm/config.rs` only if normalization/validation is needed
- `src/llm/registry.rs` only if clone/share semantics need adjustment
- `src/hongmeng/*` and `src/nuwa/*` only if tests reveal call-site changes are required
- Deterministic tests proving concurrency is bounded

Out of scope:

- Provider fallback redesign
- Retrying failed calls
- Queue cancellation policies
- Global cross-provider limit
- Real network integration tests against OpenAI/Ollama

## Requirements

1. Add per-client semaphore enforcement.
   - `LlmClient` should own/share a `tokio::sync::Semaphore` sized by `max_concurrency`.
   - `LlmClient` remains `Clone`.
   - Clones of the same client must share the same semaphore, not each create independent limits.
   - A zero config value must not create an unusable semaphore. Normalize `max_concurrency == 0` to 1 or return a clear config error; prefer normalizing to 1 to preserve permissive config loading.

2. Limit all live provider calls.
   - `LlmClient::chat` must acquire a permit before dispatching OpenAI/Ollama HTTP requests.
   - Permit must be held for the entire request/response parsing path.
   - Permit must be released on success and error.
   - Deterministic stub paths with `provider: None` must not touch the semaphore or serialize.

3. Preserve behavior.
   - Public `LlmClient::chat` signature remains unchanged.
   - Existing OpenAI/Ollama request body and response semantics remain unchanged from D4.
   - Existing fallback order remains unchanged.

4. Add deterministic tests.
   - Do not require external network services.
   - Use local mock HTTP servers or test-only hooks.
   - Prove `max_concurrency = 1` serializes overlapping LLM calls.
   - Prove `max_concurrency = 2` allows at most two concurrent requests.
   - Keep tests reliable and fast.

## Acceptance Criteria

- `max_concurrency` is no longer dead/reserved-only.
- Concurrent `chat` calls on cloned/shared clients obey the configured bound.
- Existing provider config parsing tests still pass.
- Existing D4 Ollama `/api/chat` tests still pass.
- `cargo fmt` passes.
- `cargo test --quiet llm` passes.
- `cargo test --quiet` passes.
- `cargo clippy -- -D warnings` passes.
- `git diff --check` passes.

## Implementation Notes

Suggested design:

- Store `Arc<tokio::sync::Semaphore>` in `LlmClient`.
- Add `pub fn max_concurrency(&self) -> usize` if useful for tests.
- In `new`, use `config.max_concurrency.max(1)`.
- In `chat`, acquire an owned permit before matching provider type:
  ```rust
  let _permit = self.semaphore.clone().acquire_owned().await ...?;
  ```
- Map semaphore close errors to `LlmError::ChatFailed` or `LlmError::Config`; the semaphore should never close in normal use.

Testing approach:

- Local TCP mock server can track active request count with atomics and delay responses until overlap is observed.
- Prefer simple Tokio tasks over sleeps where possible, but bounded short sleeps are acceptable in tests if deterministic.
