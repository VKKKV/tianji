use super::config::{ProviderConfig, ProviderType};
use super::error::LlmError;

#[derive(Clone, Debug)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

#[derive(Clone, Debug)]
pub struct LlmClient {
    provider_name: String,
    provider_type: ProviderType,
    model: String,
    base_url: Option<String>,
    api_key: Option<String>,
    max_concurrency: usize,
}

impl LlmClient {
    pub fn new(name: &str, config: &ProviderConfig) -> Result<Self, LlmError> {
        let api_key = config.resolve_api_key_for_provider(name)?;
        Ok(Self {
            provider_name: name.to_string(),
            provider_type: config.provider_type.clone(),
            model: config.model.clone(),
            base_url: config.base_url.clone(),
            api_key,
            max_concurrency: config.max_concurrency,
        })
    }

    pub fn provider_name(&self) -> &str {
        &self.provider_name
    }

    pub fn model(&self) -> &str {
        &self.model
    }

    pub fn provider_type(&self) -> &ProviderType {
        &self.provider_type
    }

    pub async fn chat(
        &self,
        messages: Vec<ChatMessage>,
        model: Option<&str>,
    ) -> Result<String, LlmError> {
        let _ = (
            &messages,
            model,
            &self.base_url,
            &self.api_key,
            self.max_concurrency,
            &self.provider_type,
        );
        // Stub implementation — real async-openai / ollama-rs integration in Phase 3+
        Ok("stub".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ollama_config() -> ProviderConfig {
        ProviderConfig {
            provider_type: ProviderType::Ollama,
            model: "qwen3:14b".to_string(),
            base_url: Some("http://localhost:11434".to_string()),
            api_key_env: None,
            api_key: None,
            max_concurrency: 3,
            fallback: None,
        }
    }

    fn openai_config() -> ProviderConfig {
        ProviderConfig {
            provider_type: ProviderType::OpenAI,
            model: "gpt-4o".to_string(),
            base_url: None,
            api_key_env: None,
            api_key: Some("sk-test".to_string()),
            max_concurrency: 1,
            fallback: None,
        }
    }

    #[test]
    fn new_creates_client_from_config() {
        let config = ollama_config();
        let client = LlmClient::new("ollama_local", &config).expect("create client");
        assert_eq!(client.provider_name(), "ollama_local");
        assert_eq!(client.model(), "qwen3:14b");
    }

    #[test]
    fn new_with_inline_api_key() {
        let config = openai_config();
        let client = LlmClient::new("openai_remote", &config).expect("create client");
        assert_eq!(client.provider_name(), "openai_remote");
    }

    #[tokio::test]
    async fn chat_stub_returns_stub_response() {
        let config = ollama_config();
        let client = LlmClient::new("ollama_local", &config).expect("create client");
        let messages = vec![ChatMessage {
            role: "user".to_string(),
            content: "Hello".to_string(),
        }];
        let response = client.chat(messages, None).await.expect("chat response");
        assert_eq!(response, "stub");
    }
}
