use std::path::PathBuf;
use serde::{Serialize, Deserialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub openrouter_key: Option<String>,
    #[serde(default)]
    pub default_provider: Option<String>,
    #[serde(default)]
    pub default_model: Option<String>,
    #[serde(default)]
    pub providers: Vec<ProviderEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderEntry {
    pub id: String,
    pub endpoint: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub is_local: bool,
    pub key_env: Option<String>,
}

fn default_true() -> bool { true }

impl Config {
    pub fn load() -> Self {
        let path = config_path();
        if let Ok(content) = std::fs::read_to_string(&path) {
            if let Ok(cfg) = toml::from_str::<Config>(&content) {
                return cfg;
            }
        }
        Config {
            openrouter_key: None,
            default_provider: None,
            default_model: None,
            providers: vec![],
        }
    }

    pub fn save(&self) {
        let path = config_path();
        if let Some(parent) = path.parent() { let _ = std::fs::create_dir_all(parent); }
        let content = toml::to_string_pretty(self).unwrap_or_default();
        std::fs::write(&path, content).ok();
    }

    pub fn get_key(&self) -> anyhow::Result<String> {
        if let Some(ref k) = self.openrouter_key { if !k.is_empty() { return Ok(k.clone()); } }
        if let Ok(k) = std::env::var("OPENROUTER_API_KEY") { if !k.is_empty() { return Ok(k); } }
        anyhow::bail!("No API key. /key sk-or-v1-... or set OPENROUTER_API_KEY")
    }

    pub fn set_key(&mut self, key: &str) {
        self.openrouter_key = Some(key.to_string());
    }

    pub fn active_provider(&self) -> String {
        self.default_provider.as_deref().unwrap_or("openrouter").to_string()
    }

    pub fn default_model(&self) -> String {
        self.default_model.as_deref().unwrap_or("anthropic/claude-sonnet-4").to_string()
    }

    pub fn set_model(&mut self, model: &str) {
        self.default_model = Some(model.to_string());
    }

    pub fn add_provider(&mut self, id: String, endpoint: String, key_env: Option<String>) {
        self.providers.retain(|p| p.id != id);
        let is_local = endpoint.contains("localhost") || endpoint.contains("127.0.0.1") || endpoint.contains("0.0.0.0");
        self.providers.push(ProviderEntry { id, endpoint, enabled: true, is_local, key_env });
    }

    pub fn remove_provider(&mut self, id: &str) {
        self.providers.retain(|p| p.id != id);
    }

    pub fn set_default_provider(&mut self, id: &str) {
        self.default_provider = Some(id.to_string());
    }
}

pub fn config_path() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("g0d")
        .join("config.toml")
}
