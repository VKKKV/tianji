use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::error::LlmError;

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub enum ProviderType {
    #[serde(rename = "openai")]
    OpenAI,
    #[serde(rename = "ollama")]
    Ollama,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct ProviderConfig {
    #[serde(rename = "type")]
    pub provider_type: ProviderType,
    pub model: String,
    #[serde(default)]
    pub base_url: Option<String>,
    #[serde(default)]
    pub api_key_env: Option<String>,
    #[serde(default)]
    pub api_key: Option<String>,
    #[serde(default = "default_max_concurrency")]
    pub max_concurrency: usize,
    #[serde(default)]
    pub fallback: Option<String>,
}

fn default_max_concurrency() -> usize {
    1
}

impl ProviderConfig {
    pub fn resolve_api_key(&self) -> Result<Option<String>, LlmError> {
        if let Some(ref key) = self.api_key {
            return Ok(Some(key.clone()));
        }
        if let Some(ref env_var) = self.api_key_env {
            match env::var(env_var) {
                Ok(value) if !value.is_empty() => return Ok(Some(value)),
                Ok(_) => {
                    return Err(LlmError::MissingApiKey {
                        provider: String::new(),
                        env_var: env_var.clone(),
                    })
                }
                Err(_) => {
                    return Err(LlmError::MissingApiKey {
                        provider: String::new(),
                        env_var: env_var.clone(),
                    })
                }
            }
        }
        Ok(None)
    }

    pub fn resolve_api_key_for_provider(&self, name: &str) -> Result<Option<String>, LlmError> {
        if let Some(ref key) = self.api_key {
            return Ok(Some(key.clone()));
        }
        if let Some(ref env_var) = self.api_key_env {
            match env::var(env_var) {
                Ok(value) if !value.is_empty() => return Ok(Some(value)),
                Ok(_) => {
                    return Err(LlmError::MissingApiKey {
                        provider: name.to_string(),
                        env_var: env_var.clone(),
                    })
                }
                Err(_) => {
                    return Err(LlmError::MissingApiKey {
                        provider: name.to_string(),
                        env_var: env_var.clone(),
                    })
                }
            }
        }
        Ok(None)
    }
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq)]
pub struct TianJiConfig {
    #[serde(default)]
    pub providers: BTreeMap<String, ProviderConfig>,
    #[serde(default)]
    pub agent_model_map: BTreeMap<String, String>,
}

impl TianJiConfig {
    pub fn default_path() -> PathBuf {
        let home = env::var("HOME").unwrap_or_else(|_| ".".to_string());
        PathBuf::from(home).join(".tianji").join("config.yaml")
    }

    pub fn load() -> Result<Self, LlmError> {
        let path = Self::default_path();
        Self::load_from(&path)
    }

    pub fn load_from(path: impl AsRef<Path>) -> Result<Self, LlmError> {
        let path = path.as_ref();
        if !path.exists() {
            return Ok(Self::default());
        }
        let content = fs::read_to_string(path)
            .map_err(|e| LlmError::Config(format!("failed to read config file: {e}")))?;
        let config: TianJiConfig = serde_yaml::from_str(&content)
            .map_err(|e| LlmError::Config(format!("failed to parse config YAML: {e}")))?;
        Ok(config)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn temp_config_path(label: &str) -> PathBuf {
        PathBuf::from(format!(
            "/tmp/tianji_test_config_{label}_{}.yaml",
            std::process::id()
        ))
    }

    fn cleanup_config(path: &PathBuf) {
        let _ = fs::remove_file(path);
    }

    #[test]
    fn parse_valid_config_yaml() {
        let yaml = r#"
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
"#;
        let config: TianJiConfig = serde_yaml::from_str(yaml).expect("parse config");
        assert_eq!(config.providers.len(), 2);
        assert_eq!(config.agent_model_map.len(), 3);

        let ollama = &config.providers["ollama_local"];
        assert_eq!(ollama.provider_type, ProviderType::Ollama);
        assert_eq!(ollama.model, "qwen3:14b");
        assert_eq!(ollama.base_url.as_deref(), Some("http://localhost:11434"));
        assert_eq!(ollama.max_concurrency, 3);
        assert!(ollama.api_key_env.is_none());
        assert!(ollama.fallback.is_none());

        let openai = &config.providers["openai_remote"];
        assert_eq!(openai.provider_type, ProviderType::OpenAI);
        assert_eq!(openai.model, "gpt-4o");
        assert_eq!(openai.api_key_env.as_deref(), Some("OPENAI_API_KEY"));
        assert_eq!(openai.fallback.as_deref(), Some("ollama_local"));
        assert_eq!(openai.max_concurrency, 1);

        assert_eq!(config.agent_model_map["forward_default"], "ollama_local");
        assert_eq!(config.agent_model_map["backward_coarse"], "openai_remote");
    }

    #[test]
    fn parse_config_with_missing_optional_fields_returns_defaults() {
        let yaml = r#"
providers:
  minimal:
    type: ollama
    model: tiny
"#;
        let config: TianJiConfig = serde_yaml::from_str(yaml).expect("parse config");
        assert_eq!(config.providers.len(), 1);
        assert!(config.agent_model_map.is_empty());

        let minimal = &config.providers["minimal"];
        assert_eq!(minimal.provider_type, ProviderType::Ollama);
        assert_eq!(minimal.model, "tiny");
        assert!(minimal.base_url.is_none());
        assert!(minimal.api_key_env.is_none());
        assert!(minimal.api_key.is_none());
        assert_eq!(minimal.max_concurrency, 1);
        assert!(minimal.fallback.is_none());
    }

    #[test]
    fn empty_config_returns_default() {
        let config = TianJiConfig::default();
        assert!(config.providers.is_empty());
        assert!(config.agent_model_map.is_empty());
    }

    #[test]
    fn load_from_missing_file_returns_default() {
        let path = PathBuf::from("/tmp/tianji_nonexistent_config_99999.yaml");
        let config = TianJiConfig::load_from(&path).expect("load from missing file");
        assert!(config.providers.is_empty());
        assert!(config.agent_model_map.is_empty());
    }

    #[test]
    fn load_from_valid_file() {
        let path = temp_config_path("valid");
        let yaml = r#"
providers:
  test_provider:
    type: openai
    model: gpt-4o
    api_key_env: TEST_LLM_API_KEY
agent_model_map:
  default: test_provider
"#;
        let mut file = fs::File::create(&path).expect("create temp config");
        write!(file, "{yaml}").expect("write temp config");

        let config = TianJiConfig::load_from(&path).expect("load config");
        assert_eq!(config.providers.len(), 1);
        assert_eq!(config.agent_model_map["default"], "test_provider");

        cleanup_config(&path);
    }

    #[test]
    fn resolve_api_key_from_inline() {
        let provider = ProviderConfig {
            provider_type: ProviderType::OpenAI,
            model: "gpt-4o".to_string(),
            base_url: None,
            api_key_env: None,
            api_key: Some("sk-test-key".to_string()),
            max_concurrency: 1,
            fallback: None,
        };
        let key = provider.resolve_api_key().expect("resolve inline key");
        assert_eq!(key.as_deref(), Some("sk-test-key"));
    }

    #[test]
    fn resolve_api_key_from_env_var() {
        env::set_var("TIANJI_TEST_LLM_KEY", "test-api-key-value");
        let provider = ProviderConfig {
            provider_type: ProviderType::OpenAI,
            model: "gpt-4o".to_string(),
            base_url: None,
            api_key_env: Some("TIANJI_TEST_LLM_KEY".to_string()),
            api_key: None,
            max_concurrency: 1,
            fallback: None,
        };
        let key = provider.resolve_api_key().expect("resolve env key");
        assert_eq!(key.as_deref(), Some("test-api-key-value"));
        env::remove_var("TIANJI_TEST_LLM_KEY");
    }

    #[test]
    fn resolve_api_key_missing_env_var_returns_error() {
        env::remove_var("TIANJI_NONEXISTENT_KEY_FOR_TEST");
        let provider = ProviderConfig {
            provider_type: ProviderType::OpenAI,
            model: "gpt-4o".to_string(),
            base_url: None,
            api_key_env: Some("TIANJI_NONEXISTENT_KEY_FOR_TEST".to_string()),
            api_key: None,
            max_concurrency: 1,
            fallback: None,
        };
        let result = provider.resolve_api_key();
        assert!(result.is_err());
        let err = result.unwrap_err();
        match err {
            LlmError::MissingApiKey {
                provider: _,
                env_var,
            } => {
                assert_eq!(env_var, "TIANJI_NONEXISTENT_KEY_FOR_TEST");
            }
            other => panic!("expected MissingApiKey, got {other:?}"),
        }
    }

    #[test]
    fn resolve_api_key_no_key_configured_returns_none() {
        let provider = ProviderConfig {
            provider_type: ProviderType::Ollama,
            model: "tiny".to_string(),
            base_url: None,
            api_key_env: None,
            api_key: None,
            max_concurrency: 1,
            fallback: None,
        };
        let key = provider.resolve_api_key().expect("resolve no key");
        assert!(key.is_none());
    }

    #[test]
    fn default_path_uses_home_dot_tianji() {
        let path = TianJiConfig::default_path();
        assert!(path.to_string_lossy().contains(".tianji"));
        assert!(path.to_string_lossy().contains("config.yaml"));
    }
}
