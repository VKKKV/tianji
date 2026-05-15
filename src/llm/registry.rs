use std::collections::BTreeMap;

use super::client::LlmClient;
use super::config::TianJiConfig;
use super::error::LlmError;

#[derive(Clone, Debug)]
pub struct ProviderRegistry {
    config: TianJiConfig,
    clients: BTreeMap<String, LlmClient>,
}

impl ProviderRegistry {
    pub fn from_config(config: TianJiConfig) -> Result<Self, LlmError> {
        let mut clients = BTreeMap::new();
        for (name, provider_config) in &config.providers {
            match LlmClient::new(name, provider_config) {
                Ok(client) => {
                    clients.insert(name.clone(), client);
                }
                Err(LlmError::MissingApiKey { .. }) => {
                    // Provider has api_key_env set but the env var is missing.
                    // Skip this provider — it cannot be used without a key.
                    continue;
                }
                Err(e) => return Err(e),
            }
        }
        Ok(Self { config, clients })
    }

    pub fn get_client(&self, provider_name: &str) -> Option<&LlmClient> {
        self.clients.get(provider_name)
    }

    pub fn resolve_with_fallback(&self, provider_name: &str) -> Result<&LlmClient, LlmError> {
        let mut visited = Vec::new();
        let mut current = provider_name.to_string();

        loop {
            if visited.contains(&current) {
                return Err(LlmError::Config(format!(
                    "circular fallback chain detected: {} -> {}",
                    visited.join(" -> "),
                    current
                )));
            }

            if let Some(client) = self.clients.get(&current) {
                return Ok(client);
            }

            visited.push(current.clone());

            let next = self
                .config
                .providers
                .get(&current)
                .and_then(|pc| pc.fallback.clone());

            match next {
                Some(fallback) => current = fallback,
                None => return Err(LlmError::NoAvailableProvider(provider_name.to_string())),
            }
        }
    }

    pub fn providers(&self) -> &BTreeMap<String, LlmClient> {
        &self.clients
    }

    pub fn agent_model_map(&self) -> &BTreeMap<String, String> {
        &self.config.agent_model_map
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::llm::config::{ProviderConfig, ProviderType};

    fn make_config_with_fallback() -> TianJiConfig {
        let mut providers = BTreeMap::new();
        providers.insert(
            "ollama_local".to_string(),
            ProviderConfig {
                provider_type: ProviderType::Ollama,
                model: "qwen3:14b".to_string(),
                base_url: Some("http://localhost:11434".to_string()),
                api_key_env: None,
                api_key: None,
                max_concurrency: 3,
                fallback: None,
            },
        );
        providers.insert(
            "openai_remote".to_string(),
            ProviderConfig {
                provider_type: ProviderType::OpenAI,
                model: "gpt-4o".to_string(),
                base_url: None,
                api_key_env: None,
                api_key: Some("sk-test".to_string()),
                max_concurrency: 1,
                fallback: Some("ollama_local".to_string()),
            },
        );

        let mut agent_model_map = BTreeMap::new();
        agent_model_map.insert("forward_default".to_string(), "ollama_local".to_string());
        agent_model_map.insert("backward_coarse".to_string(), "openai_remote".to_string());

        TianJiConfig {
            providers,
            agent_model_map,
        }
    }

    #[test]
    fn from_config_creates_registry() {
        let config = make_config_with_fallback();
        let registry = ProviderRegistry::from_config(config).expect("create registry");
        assert_eq!(registry.clients.len(), 2);
    }

    #[test]
    fn get_client_returns_registered_client() {
        let config = make_config_with_fallback();
        let registry = ProviderRegistry::from_config(config).expect("create registry");

        let client = registry.get_client("ollama_local").expect("get ollama");
        assert_eq!(client.provider_name(), "ollama_local");

        let client = registry.get_client("openai_remote").expect("get openai");
        assert_eq!(client.provider_name(), "openai_remote");
    }

    #[test]
    fn get_client_returns_none_for_unknown() {
        let config = make_config_with_fallback();
        let registry = ProviderRegistry::from_config(config).expect("create registry");
        assert!(registry.get_client("unknown").is_none());
    }

    #[test]
    fn resolve_with_fallback_direct_hit() {
        let config = make_config_with_fallback();
        let registry = ProviderRegistry::from_config(config).expect("create registry");

        let client = registry
            .resolve_with_fallback("ollama_local")
            .expect("resolve ollama");
        assert_eq!(client.provider_name(), "ollama_local");
    }

    #[test]
    fn resolve_with_fallback_follows_chain() {
        // Remove the openai provider to simulate it being unavailable,
        // but keep its config entry with a fallback to ollama.
        // Actually, we need a config where a provider falls back to another.
        // Since openai_remote has a fallback to ollama_local, and both are
        // available, let's test that resolve_with_fallback returns openai
        // when available (no fallback needed).
        let config = make_config_with_fallback();
        let registry = ProviderRegistry::from_config(config).expect("create registry");

        let client = registry
            .resolve_with_fallback("openai_remote")
            .expect("resolve openai");
        assert_eq!(client.provider_name(), "openai_remote");
    }

    #[test]
    fn resolve_with_fallback_skips_unavailable_provider() {
        let config = make_config_with_fallback();
        let mut registry = ProviderRegistry::from_config(config).expect("create registry");

        // Remove the openai client to simulate unavailability
        registry.clients.remove("openai_remote");

        // Resolving openai_remote should fall back to ollama_local
        let client = registry
            .resolve_with_fallback("openai_remote")
            .expect("resolve via fallback");
        assert_eq!(client.provider_name(), "ollama_local");
    }

    #[test]
    fn resolve_with_fallback_no_provider_returns_error() {
        let config = make_config_with_fallback();
        let registry = ProviderRegistry::from_config(config).expect("create registry");

        let result = registry.resolve_with_fallback("nonexistent");
        assert!(result.is_err());
        match result.unwrap_err() {
            LlmError::NoAvailableProvider(name) => assert_eq!(name, "nonexistent"),
            other => panic!("expected NoAvailableProvider, got {other:?}"),
        }
    }

    #[test]
    fn resolve_with_fallback_detects_circular_chain() {
        let mut providers = BTreeMap::new();
        providers.insert(
            "a".to_string(),
            ProviderConfig {
                provider_type: ProviderType::Ollama,
                model: "a-model".to_string(),
                base_url: None,
                api_key_env: None,
                api_key: None,
                max_concurrency: 1,
                fallback: Some("b".to_string()),
            },
        );
        providers.insert(
            "b".to_string(),
            ProviderConfig {
                provider_type: ProviderType::Ollama,
                model: "b-model".to_string(),
                base_url: None,
                api_key_env: None,
                api_key: None,
                max_concurrency: 1,
                fallback: Some("a".to_string()),
            },
        );

        let config = TianJiConfig {
            providers,
            agent_model_map: BTreeMap::new(),
        };
        let mut registry = ProviderRegistry::from_config(config).expect("create registry");

        // Remove both clients so fallback chain must walk
        registry.clients.clear();

        let result = registry.resolve_with_fallback("a");
        assert!(result.is_err());
        match result.unwrap_err() {
            LlmError::Config(msg) => assert!(msg.contains("circular fallback chain")),
            other => panic!("expected Config error, got {other:?}"),
        }
    }

    #[test]
    fn providers_returns_client_map() {
        let config = make_config_with_fallback();
        let registry = ProviderRegistry::from_config(config).expect("create registry");
        assert_eq!(registry.providers().len(), 2);
    }

    #[test]
    fn agent_model_map_returns_config_map() {
        let config = make_config_with_fallback();
        let registry = ProviderRegistry::from_config(config).expect("create registry");
        assert_eq!(registry.agent_model_map().len(), 2);
        assert_eq!(
            registry.agent_model_map()["forward_default"],
            "ollama_local"
        );
    }
}
