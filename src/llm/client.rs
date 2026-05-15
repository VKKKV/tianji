use super::config::{ProviderConfig, ProviderType};
use super::error::LlmError;
use serde_json::json;

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
    #[allow(dead_code)] // Reserved for concurrent request limiting
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
        let model = model.unwrap_or(&self.model);
        match &self.provider_type {
            ProviderType::OpenAI => self.chat_openai_compatible(messages, model).await,
            ProviderType::Ollama => self.chat_ollama(messages, model).await,
        }
    }

    async fn chat_openai_compatible(
        &self,
        messages: Vec<ChatMessage>,
        model: &str,
    ) -> Result<String, LlmError> {
        let base_url = self
            .base_url
            .as_deref()
            .unwrap_or("https://api.openai.com/v1");
        let api_key = self
            .api_key
            .as_deref()
            .ok_or_else(|| LlmError::Config("no API key configured for OpenAI provider".into()))?;

        let url = format!("{}/chat/completions", base_url.trim_end_matches('/'));

        let msgs: Vec<serde_json::Value> = messages
            .into_iter()
            .map(|m| json!({"role": m.role, "content": m.content}))
            .collect();

        let body = json!({
            "model": model,
            "messages": msgs,
        });

        let body_str = serde_json::to_string(&body)
            .map_err(|e| LlmError::ChatFailed(format!("serialize error: {e}")))?;

        let client = reqwest::Client::new();
        let response = client
            .post(&url)
            .header("Authorization", format!("Bearer {}", api_key))
            .header("Content-Type", "application/json")
            .body(body_str)
            .send()
            .await
            .map_err(|e| LlmError::ChatFailed(format!("request failed: {e}")))?;

        let status = response.status();
        let text = response
            .text()
            .await
            .map_err(|e| LlmError::ChatFailed(format!("failed to read response: {e}")))?;

        if !status.is_success() {
            return Err(LlmError::ChatFailed(format!(
                "API error {}: {}",
                status.as_u16(),
                text
            )));
        }

        let json: serde_json::Value = serde_json::from_str(&text)
            .map_err(|e| LlmError::ChatFailed(format!("failed to parse response: {e}")))?;

        Ok(json["choices"][0]["message"]["content"]
            .as_str()
            .unwrap_or("")
            .to_string())
    }

    async fn chat_ollama(
        &self,
        messages: Vec<ChatMessage>,
        model: &str,
    ) -> Result<String, LlmError> {
        let base_url = self.base_url.as_deref().unwrap_or("http://localhost:11434");

        let prompt = messages
            .into_iter()
            .map(|m| match m.role.as_str() {
                "system" => format!("[System]: {}", m.content),
                "assistant" => format!("[Assistant]: {}", m.content),
                _ => m.content,
            })
            .collect::<Vec<_>>()
            .join("\n");

        let url = format!("{}/api/generate", base_url.trim_end_matches('/'));
        let body = json!({
            "model": model,
            "prompt": prompt,
            "stream": false,
        });

        let body_str = serde_json::to_string(&body)
            .map_err(|e| LlmError::ChatFailed(format!("serialize error: {e}")))?;

        let client = reqwest::Client::new();
        let response = client
            .post(&url)
            .body(body_str)
            .send()
            .await
            .map_err(|e| LlmError::ChatFailed(format!("Ollama request failed: {e}")))?;

        let status = response.status();
        let text = response
            .text()
            .await
            .map_err(|e| LlmError::ChatFailed(format!("failed to read Ollama response: {e}")))?;

        if !status.is_success() {
            return Err(LlmError::ChatFailed(format!(
                "Ollama error {}: {}",
                status.as_u16(),
                text
            )));
        }

        let json: serde_json::Value = serde_json::from_str(&text)
            .map_err(|e| LlmError::ChatFailed(format!("failed to parse Ollama response: {e}")))?;

        Ok(json["response"].as_str().unwrap_or("").to_string())
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
    async fn chat_openai_invalid_key_returns_error() {
        let mut config = openai_config();
        config.api_key = Some("invalid-key".to_string());
        let client = LlmClient::new("openai_remote", &config).expect("create client");
        let messages = vec![ChatMessage {
            role: "user".to_string(),
            content: "Hello".to_string(),
        }];
        let result = client.chat(messages, None).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn chat_ollama_unreachable_returns_error() {
        let mut config = ollama_config();
        config.base_url = Some("http://127.0.0.1:19999".to_string());
        let client = LlmClient::new("ollama_local", &config).expect("create client");
        let messages = vec![ChatMessage {
            role: "user".to_string(),
            content: "Hello".to_string(),
        }];
        let result = client.chat(messages, None).await;
        assert!(result.is_err());
    }
}
