use thiserror::Error;

#[derive(Debug, Error)]
pub enum LlmError {
    #[error("provider not found: {0}")]
    ProviderNotFound(String),

    #[error("missing API key for provider {provider}: env var {env_var} not set")]
    MissingApiKey { provider: String, env_var: String },

    #[error("config error: {0}")]
    Config(String),

    #[error("chat request failed: {0}")]
    ChatFailed(String),

    #[error("no available provider in fallback chain starting from {0}")]
    NoAvailableProvider(String),
}
