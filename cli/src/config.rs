use std::path::PathBuf;
use serde::{Serialize, Deserialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub default_provider: Option<String>,
    pub default_model: Option<String>,
    #[serde(default)]
    pub providers: Vec<ProviderEntry>,
    #[serde(default)]
    pub lang: Option<String>,
    #[serde(default)]
    pub ui_lang: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderEntry {
    pub id: String,
    pub endpoint: String,
    pub api_key: Option<String>,
    pub key_env: Option<String>,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub is_local: bool,
}

fn default_true() -> bool { true }

impl Config {
    pub fn load() -> Self {
        let path = config_path();
        if let Ok(content) = std::fs::read_to_string(&path) {
            if let Ok(mut cfg) = toml::from_str::<Config>(&content) {
                cfg.ensure_builtins();
                return cfg;
            }
        }
        let mut cfg = Config {
            default_provider: Some("openrouter".into()),
            default_model: Some("anthropic/claude-sonnet-4".into()),
            providers: vec![],
        };
        cfg.ensure_builtins();
        cfg.save();
        cfg
    }

    fn ensure_builtins(&mut self) {
        let builtins = [
            ("openrouter", "https://openrouter.ai/api/v1", "OPENROUTER_API_KEY", false),
            ("venice", "https://api.venice.ai/api/v1", "VENICE_API_KEY", false),
            ("grok", "https://api.x.ai/v1", "GROK_API_KEY", false),
        ];
        for (id, endpoint, key_env, is_local) in builtins {
            if !self.providers.iter().any(|p| p.id == id) {
                self.providers.push(ProviderEntry {
                    id: id.into(), endpoint: endpoint.into(),
                    api_key: None, key_env: Some(key_env.into()),
                    enabled: true, is_local,
                });
            }
        }
    }

    pub fn save(&self) {
        let path = config_path();
        if let Some(parent) = path.parent() { let _ = std::fs::create_dir_all(parent); }
        let content = toml::to_string_pretty(self).unwrap_or_default();
        std::fs::write(&path, content).ok();
    }

    pub fn find_provider(&self, id: &str) -> Option<&ProviderEntry> {
        self.providers.iter().find(|p| p.id == id)
    }

    pub fn active_provider_id(&self) -> String {
        self.default_provider.as_deref().unwrap_or("openrouter").to_string()
    }

    pub fn active_provider(&self) -> &ProviderEntry {
        let id = self.active_provider_id();
        self.providers.iter().find(|p| p.id == id)
            .unwrap_or_else(|| self.providers.first().unwrap())
    }

    pub fn get_api_key(&self) -> anyhow::Result<String> {
        let provider = self.active_provider();

        if let Some(ref k) = provider.api_key { if !k.is_empty() { return Ok(k.clone()); } }
        if let Some(ref env) = provider.key_env {
            if let Ok(k) = std::env::var(env) { if !k.is_empty() { return Ok(k.clone()); } }
        }

        anyhow::bail!(
            "No API key for '{}'. Set via /provider key {} <key> or env var {}",
            provider.id, provider.id, provider.key_env.as_deref().unwrap_or("API_KEY")
        )
    }

    pub fn get_endpoint(&self) -> String {
        self.active_provider().endpoint.clone()
    }

    pub fn default_model(&self) -> String {
        self.default_model.as_deref().unwrap_or("anthropic/claude-sonnet-4").to_string()
    }

    pub fn set_model(&mut self, model: &str) {
        self.default_model = Some(model.to_string());
    }

    pub fn set_provider_key(&mut self, id: &str, key: &str) {
        if let Some(p) = self.providers.iter_mut().find(|p| p.id == id) {
            p.api_key = Some(key.to_string());
        }
    }

    pub fn add_provider(&mut self, id: String, endpoint: String, key_env: Option<String>) {
        self.providers.retain(|p| p.id != id);
        let is_local = endpoint.contains("localhost") || endpoint.contains("127.0.0.1") || endpoint.contains("0.0.0.0");
        self.providers.push(ProviderEntry {
            id, endpoint, api_key: None, key_env,
            enabled: true, is_local,
        });
    }

    pub fn remove_provider(&mut self, id: &str) {
        self.providers.retain(|p| p.id != id);
    }

    pub fn set_default_provider(&mut self, id: &str) {
        self.default_provider = Some(id.to_string());
    }

    pub fn get_lang(&self) -> &str {
        self.lang.as_deref().unwrap_or("auto")
    }

    pub fn get_ui_lang(&self) -> &str {
        self.ui_lang.as_deref().unwrap_or("en")
    }

    pub fn set_lang(&mut self, lang: &str) {
        self.lang = Some(lang.to_string());
    }

    pub fn set_ui_lang(&mut self, lang: &str) {
        self.ui_lang = Some(lang.to_string());
    }
}

pub fn config_path() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("g0d")
        .join("config.toml")
}
