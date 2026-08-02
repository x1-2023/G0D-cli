use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct Config {
    pub openrouter_key: Option<String>,
}

impl Config {
    pub fn get_key(&self) -> anyhow::Result<String> {
        if let Some(ref k) = self.openrouter_key { if !k.is_empty() { return Ok(k.clone()); } }
        if let Ok(k) = std::env::var("OPENROUTER_API_KEY") { if !k.is_empty() { return Ok(k); } }
        anyhow::bail!("No API key. Set via: god3 --key sk-or-v1-... or OPENROUTER_API_KEY env var\nGet one: https://openrouter.ai/keys")
    }
}

pub fn load() -> Config {
    let path = config_path();
    if let Ok(content) = fs::read_to_string(&path) {
        if let Ok(cfg) = toml::from_str::<ConfigToml>(&content) {
            return Config { openrouter_key: cfg.key };
        }
    }
    Config { openrouter_key: None }
}

pub fn set_key(key: &str) {
    let path = config_path();
    if let Some(parent) = path.parent() { let _ = fs::create_dir_all(parent); }
    let cfg = ConfigToml { key: Some(key.to_string()) };
    let content = toml::to_string_pretty(&cfg).unwrap_or_default();
    fs::write(&path, content).ok();
    println!("\x1b[32m✓ Key saved to {}\x1b[0m", path.display());
}

pub fn config_path() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("god3")
        .join("config.toml")
}

#[derive(serde::Serialize, serde::Deserialize)]
struct ConfigToml {
    key: Option<String>,
}
