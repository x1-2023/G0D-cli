use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub default_provider: String,
    pub default_model: String,
    pub providers: Vec<ProviderEntry>,
    pub lang: String,
    pub max_context_messages: usize,
    pub approval_mode: ApprovalMode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ApprovalMode {
    On,
    Off,
}

impl ApprovalMode {
    pub fn parse(value: &str) -> Result<Self> {
        match value.to_ascii_lowercase().as_str() {
            "on" | "ask" => Ok(Self::On),
            "off" | "auto" => Ok(Self::Off),
            _ => anyhow::bail!("Approval mode must be on or off"),
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::On => "on",
            Self::Off => "off",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderEntry {
    pub id: String,
    pub endpoint: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_key: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub key_env: Option<String>,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub is_local: bool,
}

fn default_true() -> bool {
    true
}

impl Default for Config {
    fn default() -> Self {
        Self {
            default_provider: "openrouter".into(),
            default_model: "anthropic/claude-sonnet-4".into(),
            providers: builtin_providers(),
            lang: "auto".into(),
            max_context_messages: 20,
            approval_mode: ApprovalMode::On,
        }
    }
}

impl Config {
    pub fn load() -> Result<Self> {
        let path = config_path();
        if !path.exists() {
            return Ok(Self::default());
        }

        let content = std::fs::read_to_string(&path)
            .with_context(|| format!("Could not read config: {}", path.display()))?;
        let mut config: Self = toml::from_str(&content)
            .with_context(|| format!("Invalid TOML in config: {}", path.display()))?;
        config.ensure_builtins();
        config.validate()?;
        Ok(config)
    }

    pub fn save(&self) -> Result<()> {
        self.validate()?;
        let path = config_path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).with_context(|| {
                format!("Could not create config directory: {}", parent.display())
            })?;
        }
        let content = toml::to_string_pretty(self).context("Could not serialize config")?;
        std::fs::write(&path, content)
            .with_context(|| format!("Could not write config: {}", path.display()))
    }

    pub fn validate(&self) -> Result<()> {
        if self.providers.is_empty() {
            anyhow::bail!("At least one provider is required");
        }
        if !self
            .providers
            .iter()
            .any(|provider| provider.id == self.default_provider && provider.enabled)
        {
            anyhow::bail!(
                "Default provider '{}' is missing or disabled",
                self.default_provider
            );
        }
        if self.default_model.trim().is_empty() {
            anyhow::bail!("Default model cannot be empty");
        }
        if !(2..=100).contains(&self.max_context_messages) {
            anyhow::bail!("max_context_messages must be between 2 and 100");
        }
        Ok(())
    }

    fn ensure_builtins(&mut self) {
        for builtin in builtin_providers() {
            if !self
                .providers
                .iter()
                .any(|provider| provider.id == builtin.id)
            {
                self.providers.push(builtin);
            }
        }
    }

    pub fn active_provider(&self) -> &ProviderEntry {
        self.providers
            .iter()
            .find(|provider| provider.id == self.default_provider)
            .expect("validated config always has an active provider")
    }

    pub fn get_api_key(&self) -> Result<String> {
        let provider = self.active_provider();
        if provider.is_local {
            return Ok(provider.api_key.clone().unwrap_or_else(|| "local".into()));
        }
        if let Some(key) = provider.api_key.as_deref().filter(|key| !key.is_empty()) {
            return Ok(key.to_string());
        }
        if let Some(env_name) = &provider.key_env {
            if let Ok(key) = std::env::var(env_name) {
                if !key.is_empty() {
                    return Ok(key);
                }
            }
        }
        anyhow::bail!(
            "No API key for '{}'. Prefer env var {}, or use /provider key {} <key>",
            provider.id,
            provider.key_env.as_deref().unwrap_or("API_KEY"),
            provider.id
        )
    }

    pub fn set_provider_key(&mut self, id: &str, key: &str) -> Result<()> {
        let provider = self
            .providers
            .iter_mut()
            .find(|provider| provider.id == id)
            .with_context(|| format!("Unknown provider: {id}"))?;
        provider.api_key = Some(key.to_string());
        Ok(())
    }

    pub fn add_provider(&mut self, id: &str, endpoint: &str, key_env: Option<&str>) -> Result<()> {
        validate_provider_id(id)?;
        validate_endpoint(endpoint)?;
        self.providers.retain(|provider| provider.id != id);
        let is_local = endpoint.contains("localhost")
            || endpoint.contains("127.0.0.1")
            || endpoint.contains("0.0.0.0");
        self.providers.push(ProviderEntry {
            id: id.into(),
            endpoint: endpoint.trim_end_matches('/').into(),
            api_key: None,
            key_env: key_env.map(str::to_string),
            enabled: true,
            is_local,
        });
        Ok(())
    }

    pub fn remove_provider(&mut self, id: &str) -> Result<()> {
        if id == self.default_provider {
            anyhow::bail!("Cannot remove the active provider; select another provider first");
        }
        let before = self.providers.len();
        self.providers.retain(|provider| provider.id != id);
        if self.providers.len() == before {
            anyhow::bail!("Unknown provider: {id}");
        }
        Ok(())
    }

    pub fn set_default_provider(&mut self, id: &str) -> Result<()> {
        if !self
            .providers
            .iter()
            .any(|provider| provider.id == id && provider.enabled)
        {
            anyhow::bail!("Unknown or disabled provider: {id}");
        }
        self.default_provider = id.into();
        Ok(())
    }
}

fn builtin_providers() -> Vec<ProviderEntry> {
    [
        (
            "openrouter",
            "https://openrouter.ai/api/v1",
            Some("OPENROUTER_API_KEY"),
            false,
        ),
        (
            "venice",
            "https://api.venice.ai/api/v1",
            Some("VENICE_API_KEY"),
            false,
        ),
        ("grok", "https://api.x.ai/v1", Some("GROK_API_KEY"), false),
        ("ollama", "http://127.0.0.1:11434/v1", None, true),
        ("lmstudio", "http://127.0.0.1:1234/v1", None, true),
    ]
    .into_iter()
    .map(|(id, endpoint, key_env, is_local)| ProviderEntry {
        id: id.into(),
        endpoint: endpoint.into(),
        api_key: None,
        key_env: key_env.map(str::to_string),
        enabled: true,
        is_local,
    })
    .collect()
}

fn validate_provider_id(id: &str) -> Result<()> {
    if id.is_empty()
        || !id
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '-' | '_'))
    {
        anyhow::bail!("Provider id may contain only letters, numbers, '-' and '_'");
    }
    Ok(())
}

fn validate_endpoint(endpoint: &str) -> Result<()> {
    let url = reqwest::Url::parse(endpoint).context("Provider endpoint is not a valid URL")?;
    if !matches!(url.scheme(), "http" | "https") {
        anyhow::bail!("Provider endpoint must start with http:// or https://");
    }
    if url.host_str().is_none() {
        anyhow::bail!("Provider endpoint must include a host");
    }
    Ok(())
}

pub fn config_path() -> PathBuf {
    config_root().join("config.toml")
}

pub fn history_path() -> PathBuf {
    if let Some(root) = std::env::var_os("G0D_CONFIG_DIR") {
        return PathBuf::from(root).join("history.txt");
    }
    dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("g0d")
        .join("history.txt")
}

pub fn sessions_dir() -> PathBuf {
    config_root().join("sessions")
}

fn config_root() -> PathBuf {
    std::env::var_os("G0D_CONFIG_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            dirs::config_dir()
                .unwrap_or_else(|| PathBuf::from("."))
                .join("g0d")
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_valid() {
        Config::default().validate().unwrap();
    }

    #[test]
    fn rejects_unknown_default_provider() {
        let mut config = Config::default();
        assert!(config.set_default_provider("missing").is_err());
    }

    #[test]
    fn protects_active_provider_from_removal() {
        let mut config = Config::default();
        assert!(config.remove_provider("openrouter").is_err());
    }

    #[test]
    fn rejects_unsafe_provider_values() {
        let mut config = Config::default();
        assert!(config
            .add_provider("bad id", "https://example.com/v1", None)
            .is_err());
        assert!(config
            .add_provider("safe", "file:///tmp/api", None)
            .is_err());
        assert!(config.add_provider("safe", "http://", None).is_err());
    }
}
