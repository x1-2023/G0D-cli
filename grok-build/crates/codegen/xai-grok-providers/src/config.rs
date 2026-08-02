use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderConfig {
    pub id: String,
    pub display_name: String,
    pub provider_type: ProviderType,
    pub enabled: bool,
    pub local: bool,
    pub base_url: Option<String>,
    pub api_key_env: Option<String>,
    pub api_key: Option<String>,
    pub request_timeout_seconds: u64,
    pub model_cache_ttl_seconds: u64,
    pub extra_headers: Vec<(String, String)>,
    pub http_referer: Option<String>,
    pub x_title: Option<String>,
    pub models: Vec<String>,
    pub default_model: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum ProviderType {
    Grok,
    OpenRouter,
    Venice,
    OpenAICompatible,
}

impl Default for ProviderConfig {
    fn default() -> Self {
        Self {
            id: String::new(),
            display_name: String::new(),
            provider_type: ProviderType::OpenAICompatible,
            enabled: false,
            local: false,
            base_url: None,
            api_key_env: None,
            api_key: None,
            request_timeout_seconds: 180,
            model_cache_ttl_seconds: 3600,
            extra_headers: Vec::new(),
            http_referer: None,
            x_title: None,
            models: Vec::new(),
            default_model: None,
        }
    }
}

impl ProviderConfig {
    pub fn resolve_api_key(&self) -> Option<String> {
        if let Some(ref key) = self.api_key {
            if !key.is_empty() { return Some(key.clone()); }
        }
        if let Some(ref env_var) = self.api_key_env {
            if let Ok(val) = std::env::var(env_var) {
                if !val.is_empty() { return Some(val); }
            }
        }
        None
    }

    pub fn grok_default() -> Self {
        Self {
            id: "grok".into(),
            display_name: "Grok".into(),
            provider_type: ProviderType::Grok,
            enabled: true,
            local: false,
            ..Default::default()
        }
    }

    pub fn openrouter_default() -> Self {
        Self {
            id: "openrouter".into(),
            display_name: "OpenRouter".into(),
            provider_type: ProviderType::OpenRouter,
            enabled: false,
            local: false,
            base_url: Some("https://openrouter.ai/api/v1".into()),
            api_key_env: Some("OPENROUTER_API_KEY".into()),
            request_timeout_seconds: 180,
            model_cache_ttl_seconds: 3600,
            http_referer: Some("https://github.com/x1-2023/G0D-cli".into()),
            x_title: Some("G0D-cli".into()),
            ..Default::default()
        }
    }

    pub fn venice_default() -> Self {
        Self {
            id: "venice".into(),
            display_name: "Venice".into(),
            provider_type: ProviderType::Venice,
            enabled: false,
            local: false,
            api_key_env: Some("VENICE_API_KEY".into()),
            request_timeout_seconds: 180,
            ..Default::default()
        }
    }

    pub fn local_default(id: impl Into<String>, base_url: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            display_name: "Local".into(),
            provider_type: ProviderType::OpenAICompatible,
            enabled: false,
            local: true,
            base_url: Some(base_url.into()),
            api_key_env: Some("LOCAL_LLM_API_KEY".into()),
            request_timeout_seconds: 300,
            ..Default::default()
        }
    }
}
