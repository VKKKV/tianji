# Phase 3.0: Real LLM chat() Implementation

> Connects Phase 2.1 LLM provider abstraction to real API calls
> Target: replace stub chat() with async-openai and ollama-rs calls
> Status: spec

## Goal

Implement `LlmClient::chat()` to make real LLM API calls. Two backends:
- OpenAI-compatible: via `async-openai` crate
- Ollama local: via `ollama-rs` crate

## Implementation

### src/llm/client.rs — chat() rewrite

```rust
pub async fn chat(
    &self,
    messages: Vec<ChatMessage>,
    model: Option<&str>,
) -> Result<String, LlmError> {
    let model = model.unwrap_or(&self.model);
    match &self.provider_type {
        ProviderType::OpenAI => self.chat_openai(messages, model).await,
        ProviderType::Ollama => self.chat_ollama(messages, model).await,
    }
}

async fn chat_openai(&self, messages: Vec<ChatMessage>, model: &str) -> Result<String, LlmError> {
    let base_url = self.base_url.as_deref().unwrap_or("https://api.openai.com/v1");
    let api_key = self.api_key.as_deref().unwrap_or("");

    let client = async_openai::Client::with_config(
        async_openai::config::OpenAIConfig::new()
            .with_api_base(base_url)
            .with_api_key(api_key),
    );

    let request = async_openai::types::CreateChatCompletionRequestArgs::default()
        .model(model)
        .messages(messages.into_iter().map(|m| {
            async_openai::types::ChatCompletionRequestMessage::User(
                async_openai::types::ChatCompletionRequestUserMessageArgs::default()
                    .content(m.content)
                    .build()?
            )
        }).collect::<Vec<_>>())  // simplified — handle system/user/assistant roles
        .build()?;

    let response = client.chat().create(request).await?;
    Ok(response.choices.first()
        .and_then(|c| c.message.content.clone())
        .unwrap_or_default())
}

async fn chat_ollama(&self, messages: Vec<ChatMessage>, model: &str) -> Result<String, LlmError> {
    let base_url = self.base_url.as_deref().unwrap_or("http://localhost:11434");
    let client = ollama_rs::Ollama::new(base_url.to_string(), 30);  // 30s timeout

    let prompt = messages.into_iter()
        .map(|m| format!("{}: {}", m.role, m.content))
        .collect::<Vec<_>>()
        .join("\n");

    let response = client
        .generate(ollama_rs::generation::completion::request::GenerationRequest::new(
            model.to_string(),
            prompt,
        ))
        .await?;

    Ok(response.response)
}
```

### Role handling

Convert `ChatMessage { role, content }` properly:
- `role == "system"` → `ChatCompletionRequestSystemMessage`
- `role == "user"` → `ChatCompletionRequestUserMessage`
- `role == "assistant"` → `ChatCompletionRequestAssistantMessage`

### Error conversion

Map:
- `async_openai::error::OpenAIError` → `LlmError::ApiError(String)`
- `ollama_rs::error::OllamaError` → `LlmError::ApiError(String)`
- `serde_json::Error` (from message building) → `LlmError::SerializationError(String)`

Add to `src/llm/error.rs`:
```rust
pub enum LlmError {
    ConfigError(String),
    ApiError(String),
    SerializationError(String),
    ProviderNotFound(String),
}
```

### Timeout / concurrency

- OpenAI: use async-openai's built-in timeout (via reqwest)
- Ollama: 30s hardcoded timeout
- Concurrency: `max_concurrency` in config, but enforcement deferred (tokio::Semaphore in ProviderRegistry later)

## Files Changed

- `src/llm/client.rs` — replace stub with real impl
- `src/llm/error.rs` — add error variants

## Tests

- Unit: chat_stub test renamed to chat_openai_stub (no API key → error, not panic)
- Unit: invalid base_url → ApiError
- No integration tests with real APIs (needs network + keys)

## Verification

- `cargo build` zero error
- `cargo test` all pass (248+)
- `cargo clippy -- -D warnings` clean
- Existing stub-dependent tests updated (they expected "stub" return)
