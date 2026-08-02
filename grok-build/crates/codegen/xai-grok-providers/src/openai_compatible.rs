use async_trait::async_trait;
use reqwest::Client;
use std::time::Duration;
use crate::capabilities::ProviderCapabilities;
use crate::config::ProviderConfig;
use crate::error::ProviderError;
use crate::model_catalog::ModelInfo;
use crate::model_health::{HealthState, ProviderHealth};
use crate::provider::ModelProvider;
use crate::request::ModelRequest;
use crate::response::{ModelResponse, ModelStream};

pub struct OpenAICompatibleProvider {
    config: ProviderConfig,
    client: Client,
    models_cache: tokio::sync::RwLock<Option<(Vec<ModelInfo>, std::time::Instant)>>,
}

impl OpenAICompatibleProvider {
    pub fn new(config: ProviderConfig) -> Result<Self, ProviderError> {
        let client = Client::builder()
            .timeout(Duration::from_secs(config.request_timeout_seconds))
            .danger_accept_invalid_certs(false)
            .build()
            .map_err(|e| ProviderError::Connection { provider: config.id.clone(), detail: e.to_string() })?;
        Ok(Self { config, client, models_cache: tokio::sync::RwLock::new(None) })
    }

    fn api_key(&self) -> Option<String> { self.config.resolve_api_key() }

    fn auth_header(&self) -> Vec<(String, String)> {
        let mut headers = vec![("Content-Type".into(), "application/json".into())];
        if let Some(key) = self.api_key() {
            headers.push(("Authorization".into(), format!("Bearer {}", key)));
        }
        for (k, v) in &self.config.extra_headers {
            headers.push((k.clone(), v.clone()));
        }
        headers
    }
}

#[async_trait]
impl ModelProvider for OpenAICompatibleProvider {
    fn id(&self) -> &str { &self.config.id }
    fn display_name(&self) -> &str { &self.config.display_name }
    fn capabilities(&self) -> ProviderCapabilities { ProviderCapabilities::all() }
    fn is_local(&self) -> bool { self.config.local }
    fn config(&self) -> &ProviderConfig { &self.config }

    async fn health(&self) -> ProviderHealth {
        let base = self.config.base_url.as_deref().unwrap_or("http://127.0.0.1:11434/v1");
        let url = format!("{}/models", base.trim_end_matches('/'));
        match self.client.get(&url).timeout(Duration::from_secs(10)).send().await {
            Ok(resp) if resp.status().is_success() => ProviderHealth::healthy(&self.config.id),
            Ok(resp) => ProviderHealth {
                provider: self.config.id.clone(),
                state: HealthState::Degraded,
                detail: Some(format!("HTTP {}", resp.status())),
                ..ProviderHealth::unknown(&self.config.id)
            },
            Err(e) => ProviderHealth {
                provider: self.config.id.clone(),
                state: HealthState::Unavailable,
                detail: Some(e.to_string()),
                ..ProviderHealth::unknown(&self.config.id)
            },
        }
    }

    async fn list_models(&self) -> Result<Vec<ModelInfo>, ProviderError> {
        let base = self.config.base_url.as_deref().unwrap_or("http://127.0.0.1:11434/v1");
        let url = format!("{}/models", base.trim_end_matches('/'));
        let resp = self.client.get(&url)
            .headers(self.auth_header().iter().map(|(k, v)| {
                (reqwest::header::HeaderName::from_bytes(k.as_bytes()).unwrap(),
                 reqwest::header::HeaderValue::from_str(v).unwrap())
            }).collect::<reqwest::header::HeaderMap>())
            .send().await
            .map_err(|e| ProviderError::Connection { provider: self.config.id.clone(), detail: e.to_string() })?;

        let body: serde_json::Value = resp.json().await.map_err(|e| ProviderError::Serialization {
            provider: self.config.id.clone(), detail: e.to_string(),
        })?;

        let models: Vec<ModelInfo> = body["data"].as_array().unwrap_or(&vec![]).iter().map(|m| ModelInfo {
            provider: self.config.id.clone(),
            model_id: m["id"].as_str().unwrap_or("unknown").to_string(),
            display_name: m["id"].as_str().map(|s| s.to_string()),
            context_window: None,
            max_output_tokens: None,
            capabilities: ProviderCapabilities { streaming: true, model_discovery: true, ..Default::default() },
            pricing: None,
            aliases: vec![],
            categories: vec![],
            deprecated: false,
            replacement: None,
        }).collect();

        let mut cache = self.models_cache.write().await;
        *cache = Some((models.clone(), std::time::Instant::now()));
        Ok(models)
    }

    async fn complete(&self, _request: ModelRequest) -> Result<ModelResponse, ProviderError> {
        Err(ProviderError::Unknown { provider: self.config.id.clone(), detail: "OpenAI-compatible complete not yet implemented".into() })
    }

    async fn stream(&self, _request: ModelRequest) -> Result<ModelStream, ProviderError> {
        Err(ProviderError::Unknown { provider: self.config.id.clone(), detail: "OpenAI-compatible stream not yet implemented".into() })
    }
}
