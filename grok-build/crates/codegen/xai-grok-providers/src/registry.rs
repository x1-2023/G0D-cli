use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use crate::provider::ModelProvider;
use crate::model_health::ProviderHealth;

#[derive(Clone)]
pub struct ProviderRegistry {
    providers: Arc<RwLock<HashMap<String, Arc<dyn ModelProvider>>>>,
}

impl ProviderRegistry {
    pub fn new() -> Self {
        Self { providers: Arc::new(RwLock::new(HashMap::new())) }
    }

    pub async fn register(&self, provider: Arc<dyn ModelProvider>) {
        let mut guard = self.providers.write().await;
        guard.insert(provider.id().to_string(), provider);
    }

    pub async fn get(&self, id: &str) -> Option<Arc<dyn ModelProvider>> {
        let guard = self.providers.read().await;
        guard.get(id).cloned()
    }

    pub async fn list(&self) -> Vec<Arc<dyn ModelProvider>> {
        let guard = self.providers.read().await;
        guard.values().cloned().collect()
    }

    pub async fn list_enabled(&self) -> Vec<Arc<dyn ModelProvider>> {
        let guard = self.providers.read().await;
        guard.values().filter(|p| p.config().enabled).cloned().collect()
    }

    pub async fn list_local(&self) -> Vec<Arc<dyn ModelProvider>> {
        let guard = self.providers.read().await;
        guard.values().filter(|p| p.is_local()).cloned().collect()
    }

    pub async fn list_remote(&self) -> Vec<Arc<dyn ModelProvider>> {
        let guard = self.providers.read().await;
        guard.values().filter(|p| !p.is_local() && p.config().enabled).cloned().collect()
    }

    pub async fn remove(&self, id: &str) {
        let mut guard = self.providers.write().await;
        guard.remove(id);
    }

    pub async fn resolve_provider_for_model(&self, provider_id: &str, _model_id: &str) -> Option<Arc<dyn ModelProvider>> {
        self.get(provider_id).await.filter(|_| true)
    }

    pub async fn all_health(&self) -> HashMap<String, ProviderHealth> {
        let providers = self.list().await;
        let mut map = HashMap::new();
        for p in &providers {
            map.insert(p.id().to_string(), p.health().await);
        }
        map
    }
}

impl Default for ProviderRegistry {
    fn default() -> Self { Self::new() }
}
