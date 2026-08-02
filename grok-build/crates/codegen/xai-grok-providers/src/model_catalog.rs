use serde::{Deserialize, Serialize};
use crate::capabilities::ProviderCapabilities;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelInfo {
    pub provider: String,
    pub model_id: String,
    pub display_name: Option<String>,
    pub context_window: Option<usize>,
    pub max_output_tokens: Option<usize>,
    pub capabilities: ProviderCapabilities,
    pub pricing: Option<ModelPricing>,
    pub aliases: Vec<String>,
    pub categories: Vec<String>,
    pub deprecated: bool,
    pub replacement: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelPricing {
    pub prompt_price_per_1m_tokens: Option<f64>,
    pub completion_price_per_1m_tokens: Option<f64>,
    pub image_price_per_image: Option<f64>,
    pub currency: String,
}

#[derive(Debug, Clone, Default)]
pub struct ModelCatalog {
    pub models: Vec<ModelInfo>,
}

impl ModelCatalog {
    pub fn new() -> Self {
        Self { models: Vec::new() }
    }

    pub fn add(&mut self, model: ModelInfo) {
        if !self.models.iter().any(|m| m.provider == model.provider && m.model_id == model.model_id) {
            self.models.push(model);
        }
    }

    pub fn merge(&mut self, other: &ModelCatalog) {
        for model in &other.models {
            self.add(model.clone());
        }
    }

    pub fn find(&self, provider: &str, model_id: &str) -> Option<&ModelInfo> {
        self.models.iter().find(|m| m.provider == provider && m.model_id == model_id)
    }

    pub fn find_by_alias(&self, provider: &str, alias: &str) -> Option<&ModelInfo> {
        self.models.iter().find(|m| {
            m.provider == provider && (m.model_id == alias || m.aliases.iter().any(|a| a == alias))
        })
    }

    pub fn resolve(&self, provider: &str, model_id_or_alias: &str) -> Option<&ModelInfo> {
        self.find_by_alias(provider, model_id_or_alias)
            .or_else(|| self.find(provider, model_id_or_alias))
    }

    pub fn provider_models(&self, provider: &str) -> Vec<&ModelInfo> {
        self.models.iter().filter(|m| m.provider == provider).collect()
    }

    pub fn all_providers(&self) -> Vec<&str> {
        let mut providers: Vec<&str> = self.models.iter().map(|m| m.provider.as_str()).collect();
        providers.sort();
        providers.dedup();
        providers
    }

    pub fn filter_by_capability(&self, capability: &str) -> Vec<&ModelInfo> {
        self.models.iter().filter(|m| match capability {
            "tool_calling" => m.capabilities.tool_calling,
            "vision" => m.capabilities.vision,
            "reasoning" => m.capabilities.reasoning,
            "streaming" => m.capabilities.streaming,
            "json_schema" => m.capabilities.json_schema,
            _ => true,
        }).collect()
    }
}
