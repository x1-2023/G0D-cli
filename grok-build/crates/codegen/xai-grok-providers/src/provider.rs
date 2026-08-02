use async_trait::async_trait;
use crate::capabilities::ProviderCapabilities;
use crate::config::ProviderConfig;
use crate::error::ProviderError;
use crate::model_catalog::ModelInfo;
use crate::model_health::ProviderHealth;
use crate::request::ModelRequest;
use crate::response::{ModelResponse, ModelStream};

#[async_trait]
pub trait ModelProvider: Send + Sync {
    fn id(&self) -> &str;
    fn display_name(&self) -> &str;
    fn capabilities(&self) -> ProviderCapabilities;
    fn is_local(&self) -> bool;
    fn config(&self) -> &ProviderConfig;

    async fn health(&self) -> ProviderHealth;
    async fn list_models(&self) -> Result<Vec<ModelInfo>, ProviderError>;
    async fn complete(&self, request: ModelRequest) -> Result<ModelResponse, ProviderError>;
    async fn stream(&self, request: ModelRequest) -> Result<ModelStream, ProviderError>;
}
