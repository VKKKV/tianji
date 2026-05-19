# PRD — Phase D5: LLM Concurrency Limiting

> Priority: D5 | Spec: `.trellis/spec/backend/phase-d5-llm-concurrency-limiting.md`

## Goal

Implement `ProviderConfig.max_concurrency` as a real per-provider runtime limit for LLM requests.

## Background

`ProviderConfig` already parses `max_concurrency`, and `LlmClient` stores it, but the field is currently unused. TianJi simulations can call LLM providers from Hongmeng/Nuwa loops. D5 prevents uncontrolled parallel HTTP calls while keeping deterministic no-provider paths fast.

## Requirements

1. Use `tokio::sync::Semaphore` inside `LlmClient`.
   - `LlmClient` remains cloneable.
   - Clones share the same limit.
   - `max_concurrency == 0` is normalized to 1 or rejected clearly; prefer normalize to 1.

2. Enforce the limit in `LlmClient::chat`.
   - Acquire before provider-specific HTTP call.
   - Hold until response parsing completes.
   - Release on all success/error paths.
   - Do not change public API.

3. Preserve provider behavior.
   - OpenAI request semantics unchanged.
   - Ollama `/api/chat` semantics unchanged.
   - Fallback chain behavior unchanged.

4. Tests.
   - Add deterministic local HTTP tests proving concurrency bound.
   - At minimum:
     - max 1 serializes concurrent calls
     - max 2 permits two but not more
   - No external network required.

## Allowed Files

- `src/llm/client.rs`
- `src/llm/config.rs` if needed
- `src/llm/registry.rs` if needed
- minimal related test files if necessary

## Verification

Run:

```bash
cargo fmt
cargo test --quiet llm
cargo test --quiet
cargo clippy -- -D warnings
git diff --check
```

## Completion Output

```text
DEV_DONE_D5 <summary>
```

or

```text
NEED_INPUT_D5 <reason>
```
