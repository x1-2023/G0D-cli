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

pub struct VeniceProvider {
    config: ProviderConfig,
    client: Client,
    models_cache: tokio::sync::RwLock<Option<(Vec<ModelInfo>, std::time::Instant)>>,
}

impl VeniceProvider {
    pub fn new(config: ProviderConfig) -> Result<Self, ProviderError> {
        let client = Client::builder()
            .timeout(Duration::from_secs(config.request_timeout_seconds))
            .build()
            .map_err(|e| ProviderError::Connection { provider: config.id.clone(), detail: e.to_string() })?;
        Ok(Self { config, client, models_cache: tokio::sync::RwLock::new(None) })
    }

    fn api_key(&self) -> Result<String, ProviderError> {
        self.config.resolve_api_key().ok_or_else(|| ProviderError::Auth {
            provider: self.config.id.clone(),
            detail: "VENICE_API_KEY not set".into(),
        })
    }
}

#[async_trait]
impl ModelProvider for VeniceProvider {
    fn id(&self) -> &str { &self.config.id }
    fn display_name(&self) -> &str { &self.config.display_name }
    fn capabilities(&self) -> ProviderCapabilities { ProviderCapabilities::all() }
    fn is_local(&self) -> bool { false }
    fn config(&self) -> &ProviderConfig { &self.config }

    async fn health(&self) -> ProviderHealth {
        match self.api_key() {
            Ok(_) => ProviderHealth::unknown(&self.config.id),
            Err(_) => ProviderHealth {
                provider: self.config.id.clone(),
                state: HealthState::Unauthorized,
                detail: Some("Venice API key not configured".into()),
                ..ProviderHealth::unknown(&self.config.id)
            },
        }
    }

    async fn list_models(&self) -> Result<Vec<ModelInfo>, ProviderError> {
        Ok(vec![])
    }

    async fn complete(&self, _request: ModelRequest) -> Result<ModelResponse, ProviderError> {
        Err(ProviderError::Unknown { provider: self.config.id.clone(), detail: "Venice complete not yet implemented".into() })
    }

    async fn stream(&self, _request: ModelRequest) -> Result<ModelStream, ProviderError> {
        Err(ProviderError::Unknown { provider: self.config.id.clone(), detail: "Venice stream not yet implemented".into() })
    }
}
