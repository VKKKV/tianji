use super::config::{ProviderConfig, ProviderType};
use super::error::LlmError;
use serde_json::json;
use std::time::Duration;

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
    client: reqwest::Client,
}

impl LlmClient {
    pub fn new(name: &str, config: &ProviderConfig) -> Result<Self, LlmError> {
        let api_key = config.resolve_api_key_for_provider(name)?;
        let client = reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(60))
            .timeout(Duration::from_secs(120))
            .build()
            .map_err(|e| LlmError::Config(format!("failed to build HTTP client: {e}")))?;
        Ok(Self {
            provider_name: name.to_string(),
            provider_type: config.provider_type.clone(),
            model: config.model.clone(),
            base_url: config.base_url.clone(),
            api_key,
            max_concurrency: config.max_concurrency,
            client,
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

        let response = self
            .client
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

        let choices = json["choices"]
            .as_array()
            .ok_or_else(|| LlmError::ChatFailed("no choices in response".into()))?;
        let content = choices
            .first()
            .and_then(|c| c["message"]["content"].as_str())
            .unwrap_or("");
        Ok(content.to_string())
    }

    async fn chat_ollama(
        &self,
        messages: Vec<ChatMessage>,
        model: &str,
    ) -> Result<String, LlmError> {
        let base_url = self.base_url.as_deref().unwrap_or("http://localhost:11434");

        let msgs: Vec<serde_json::Value> = messages
            .into_iter()
            .map(|m| json!({"role": m.role, "content": m.content}))
            .collect();

        let url = format!("{}/api/chat", base_url.trim_end_matches('/'));
        let body = json!({
            "model": model,
            "messages": msgs,
            "stream": false,
        });

        let body_str = serde_json::to_string(&body)
            .map_err(|e| LlmError::ChatFailed(format!("serialize error: {e}")))?;

        let response = self
            .client
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

        Ok(json["message"]["content"]
            .as_str()
            .unwrap_or("")
            .to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::{TcpListener, TcpStream};
    use std::sync::mpsc::{self, Receiver};
    use std::thread;

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

    fn read_http_request(mut stream: &TcpStream) -> (String, serde_json::Value) {
        let mut bytes = Vec::new();
        let mut buffer = [0_u8; 1024];

        loop {
            let count = stream.read(&mut buffer).expect("read request bytes");
            assert!(count > 0, "client closed before headers");
            bytes.extend_from_slice(&buffer[..count]);
            if bytes.windows(4).any(|window| window == b"\r\n\r\n") {
                break;
            }
        }

        let header_end = bytes
            .windows(4)
            .position(|window| window == b"\r\n\r\n")
            .expect("headers terminator")
            + 4;
        let headers = String::from_utf8(bytes[..header_end].to_vec()).expect("headers utf8");
        let content_length = headers
            .lines()
            .find_map(|line| {
                let (name, value) = line.split_once(':')?;
                name.eq_ignore_ascii_case("content-length").then(|| {
                    value
                        .trim()
                        .parse::<usize>()
                        .expect("content length number")
                })
            })
            .expect("content-length header");

        while bytes.len() - header_end < content_length {
            let count = stream.read(&mut buffer).expect("read request body");
            assert!(count > 0, "client closed before body");
            bytes.extend_from_slice(&buffer[..count]);
        }

        let body = &bytes[header_end..header_end + content_length];
        let json = serde_json::from_slice(body).expect("request body JSON");
        (headers, json)
    }

    fn spawn_ollama_server(
        response: &'static str,
    ) -> (String, Receiver<(String, serde_json::Value)>) {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind test server");
        let addr = listener.local_addr().expect("test server addr");
        let (sender, receiver) = mpsc::channel();

        thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept request");
            let request = read_http_request(&stream);
            sender.send(request).expect("send captured request");
            stream
                .write_all(response.as_bytes())
                .expect("write response");
        });

        (format!("http://{addr}"), receiver)
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

    #[tokio::test]
    async fn chat_ollama_posts_structured_messages_to_api_chat() {
        let (base_url, request_receiver) = spawn_ollama_server(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 58\r\n\r\n{\"message\":{\"role\":\"assistant\",\"content\":\"structured ok\"}}",
        );
        let mut config = ollama_config();
        config.base_url = Some(base_url);
        let client = LlmClient::new("ollama_local", &config).expect("create client");
        let messages = vec![
            ChatMessage {
                role: "system".to_string(),
                content: "Return JSON only".to_string(),
            },
            ChatMessage {
                role: "user".to_string(),
                content: "Pick an action".to_string(),
            },
            ChatMessage {
                role: "assistant".to_string(),
                content: "Previous answer".to_string(),
            },
        ];

        let result = client.chat(messages, None).await.expect("ollama chat");
        let (headers, body) = request_receiver.recv().expect("captured request");

        assert_eq!(result, "structured ok");
        assert!(headers.starts_with("POST /api/chat HTTP/1.1"));
        assert_eq!(body["model"], "qwen3:14b");
        assert_eq!(body["stream"], false);
        assert!(body.get("prompt").is_none(), "should not flatten prompt");
        assert_eq!(body["messages"][0]["role"], "system");
        assert_eq!(body["messages"][0]["content"], "Return JSON only");
        assert_eq!(body["messages"][1]["role"], "user");
        assert_eq!(body["messages"][1]["content"], "Pick an action");
        assert_eq!(body["messages"][2]["role"], "assistant");
        assert_eq!(body["messages"][2]["content"], "Previous answer");
    }

    #[tokio::test]
    async fn chat_ollama_non_success_includes_status_and_body() {
        let (base_url, request_receiver) = spawn_ollama_server(
            "HTTP/1.1 500 Internal Server Error\r\nContent-Type: text/plain\r\nContent-Length: 13\r\n\r\nollama failed",
        );
        let mut config = ollama_config();
        config.base_url = Some(base_url);
        let client = LlmClient::new("ollama_local", &config).expect("create client");
        let messages = vec![ChatMessage {
            role: "user".to_string(),
            content: "Hello".to_string(),
        }];

        let result = client.chat(messages, None).await;
        let (headers, _) = request_receiver.recv().expect("captured request");

        assert!(headers.starts_with("POST /api/chat HTTP/1.1"));
        let error = result.expect_err("non-success should fail");
        let message = error.to_string();
        assert!(message.contains("500"), "missing status: {message}");
        assert!(
            message.contains("ollama failed"),
            "missing response body: {message}"
        );
    }
}
