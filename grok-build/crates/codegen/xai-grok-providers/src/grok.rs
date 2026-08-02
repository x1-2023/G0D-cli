use async_trait::async_trait;
use crate::capabilities::ProviderCapabilities;
use crate::config::ProviderConfig;
use crate::error::ProviderError;
use crate::model_catalog::ModelInfo;
use crate::model_health::ProviderHealth;
use crate::provider::ModelProvider;
use crate::request::ModelRequest;
use crate::response::{ModelResponse, ModelStream};

pub struct GrokProvider {
    config: ProviderConfig,
    models: Vec<ModelInfo>,
}

impl GrokProvider {
    pub fn new(config: ProviderConfig) -> Self {
        let models = Self::default_models();
        Self { config, models }
    }

    fn default_models() -> Vec<ModelInfo> {
        let info = ModelInfo {
            provider: "grok".into(),
            model_id: "grok-code-fast".into(),
            display_name: Some("Grok Code Fast".into()),
            context_window: Some(256_000),
            max_output_tokens: Some(32_768),
            capabilities: ProviderCapabilities {
                streaming: true,
                tool_calling: true,
                parallel_tool_calls: true,
                vision: true,
                reasoning: true,
                json_schema: true,
                system_prompt: true,
                token_usage: true,
                context_window: true,
                max_output_tokens: true,
                ..Default::default()
            },
            pricing: None,
            aliases: vec!["grok-code-fast".into()],
            categories: vec!["coding".into(), "reasoning".into()],
            deprecated: false,
            replacement: None,
        };
        vec![info]
    }
}

#[async_trait]
impl ModelProvider for GrokProvider {
    fn id(&self) -> &str { &self.config.id }
    fn display_name(&self) -> &str { &self.config.display_name }
    fn capabilities(&self) -> ProviderCapabilities { ProviderCapabilities::all() }
    fn is_local(&self) -> bool { false }
    fn config(&self) -> &ProviderConfig { &self.config }

    async fn health(&self) -> ProviderHealth {
        ProviderHealth::unknown("grok")
    }

    async fn list_models(&self) -> Result<Vec<ModelInfo>, ProviderError> {
        Ok(self.models.clone())
    }

    async fn complete(&self, _request: ModelRequest) -> Result<ModelResponse, ProviderError> {
        Err(ProviderError::Unknown {
            provider: "grok".into(),
            detail: "Grok provider requires integration with xai-grok-sampler".into(),
        })
    }

    async fn stream(&self, _request: ModelRequest) -> Result<ModelStream, ProviderError> {
        Err(ProviderError::Unknown {
            provider: "grok".into(),
            detail: "Grok provider requires integration with xai-grok-sampler".into(),
        })
    }
}
