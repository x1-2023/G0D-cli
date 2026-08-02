use crate::error::ProviderError;
use crate::model_catalog::ModelInfo;
use crate::provider::ModelProvider;
use std::sync::Arc;

pub async fn discover_models(provider: &dyn ModelProvider) -> Result<Vec<ModelInfo>, ProviderError> {
    if provider.capabilities().model_discovery {
        provider.list_models().await
    } else {
        Ok(vec![])
    }
}

pub async fn discover_all(providers: &[Arc<dyn ModelProvider>]) -> Vec<Result<Vec<ModelInfo>, ProviderError>> {
    let mut results = Vec::new();
    for p in providers {
        results.push(discover_models(p.as_ref()).await);
    }
    results
}

pub async fn find_compatible_models(
    providers: &[Arc<dyn ModelProvider>],
    required_capabilities: &[&str],
) -> Vec<ModelInfo> {
    let mut all = Vec::new();
    for p in providers {
        if let Ok(models) = p.list_models().await {
            for m in models {
                let compatible = required_capabilities.iter().all(|cap| match *cap {
                    "tool_calling" => m.capabilities.tool_calling,
                    "vision" => m.capabilities.vision,
                    "reasoning" => m.capabilities.reasoning,
                    "streaming" => m.capabilities.streaming,
                    "json_schema" => m.capabilities.json_schema,
                    _ => true,
                });
                if compatible { all.push(m); }
            }
        }
    }
    all
}
