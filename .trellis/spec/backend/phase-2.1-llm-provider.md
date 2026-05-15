# Phase 2.1: LLM Provider Abstraction + Config

> Part of plan.md §6.5 Phase 2 Hongmeng/Nuwa
> Target: provider registry, config loading, LLM abstraction layer
> Status: spec

## Goal

Add an LLM provider abstraction that supports OpenAI-compatible APIs and Ollama
local models, with fallback chains. Load provider config from `~/.tianji/config.yaml`.
No agent logic yet — just the ability to call LLMs.

## New Dependencies

```toml
serde_yaml = "0.9"
async-openai = "0.34"
ollama-rs = "0.3"
anyhow = "1"
thiserror = "2"
tracing = "0.1"
tracing-subscriber = "0.3"
chrono = { version = "0.4", features = ["serde"] }
```

## Config File: `~/.tianji/config.yaml`

```yaml
providers:
  ollama_local:
    type: ollama
    model: qwen3:14b
    base_url: http://localhost:11434
    max_concurrency: 3

  openai_remote:
    type: openai
    model: gpt-4o
    api_key_env: OPENAI_API_KEY
    fallback: ollama_local

agent_model_map:
  forward_default: ollama_local
  backward_coarse: openai_remote
  backward_fine: ollama_local
```

## Rust Types

### src/llm.rs

```rust
pub enum ProviderType {
    OpenAI,
    Ollama,
}

pub struct ProviderConfig {
    pub name: String,
    pub provider_type: ProviderType,
    pub model: String,
    pub base_url: Option<String>,
    pub api_key_env: Option<String>,     // reads from env var
    pub api_key: Option<String>,         // inline (discouraged)
    pub max_concurrency: usize,
    pub fallback: Option<String>,        // provider name
}

pub struct TianJiConfig {
    pub providers: BTreeMap<String, ProviderConfig>,
    pub agent_model_map: BTreeMap<String, String>,
}

impl TianJiConfig {
    pub fn load() -> Result<Self, TianJiError>  // from ~/.tianji/config.yaml
    pub fn default_path() -> PathBuf
}
```

### src/llm/client.rs

```rust
pub struct LlmClient {
    // abstracts over async-openai and ollama-rs
}

impl LlmClient {
    pub async fn chat(
        &self,
        messages: Vec<ChatMessage>,
        model: Option<&str>,
    ) -> Result<String, TianJiError>;
}

pub struct ChatMessage {
    pub role: String,    // "system" | "user" | "assistant"
    pub content: String,
}
```

### Provider Registry

```rust
pub struct ProviderRegistry {
    config: TianJiConfig,
    clients: BTreeMap<String, LlmClient>,
}

impl ProviderRegistry {
    pub fn from_config(config: TianJiConfig) -> Self
    pub fn get_client(&self, provider_name: &str) -> Option<&LlmClient>
    pub fn resolve_with_fallback(&self, provider_name: &str) -> (&LlmClient, &str)
}
```

## Files

```
src/
├── llm.rs              # mod declarations, re-exports
├── llm/
│   ├── config.rs       # TianJiConfig, ProviderConfig, YAML loading
│   ├── client.rs       # LlmClient, ChatMessage, chat()
│   ├── registry.rs     # ProviderRegistry, fallback resolution
│   └── error.rs        # LLM-specific error types
├── main.rs             # add: load config if --config flag or default path
└── lib.rs              # add: pub mod llm
```

## No CLI changes yet

Don't add `tianji config` subcommand — that's Phase 2.2. Just load config
from `~/.tianji/config.yaml` if the file exists, silent skip if not.

## Tests

- Unit: parse valid config.yaml
- Unit: parse config with missing optional fields → defaults
- Unit: fallback chain resolution (openai_remote → ollama_local)
- Unit: api_key_env reads from environment
- Unit: empty config (no file) → default

No integration tests with real LLM calls (needs network + API keys).

## Verification

- `cargo build` zero error
- `cargo test` all pass
- `cargo clippy -- -D warnings` clean
- Config loads silently if `~/.tianji/config.yaml` exists
