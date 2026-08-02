use std::fmt;

#[derive(Debug, thiserror::Error)]
pub enum ProviderError {
    #[error("provider {provider}: HTTP error {status}: {body}")]
    Http { provider: String, status: u16, body: String },

    #[error("provider {provider}: auth error: {detail}")]
    Auth { provider: String, detail: String },

    #[error("provider {provider}: rate limited")]
    RateLimited { provider: String, retry_after: Option<u64> },

    #[error("provider {provider}: timeout after {elapsed:?}")]
    Timeout { provider: String, elapsed: std::time::Duration },

    #[error("provider {provider}: model {model} not found")]
    ModelNotFound { provider: String, model: String },

    #[error("provider {provider}: unsupported capability: {capability}")]
    UnsupportedCapability { provider: String, capability: String },

    #[error("provider {provider}: streaming error: {detail}")]
    Streaming { provider: String, detail: String },

    #[error("provider {provider}: connection error: {detail}")]
    Connection { provider: String, detail: String },

    #[error("provider {provider}: serialization error: {detail}")]
    Serialization { provider: String, detail: String },

    #[error("provider {provider}: response too large ({size} bytes)")]
    ResponseTooLarge { provider: String, size: usize },

    #[error("provider {provider}: context window exceeded ({requested} > {limit})")]
    ContextExceeded { provider: String, requested: usize, limit: usize },

    #[error("provider {provider}: empty response")]
    EmptyResponse { provider: String },

    #[error("provider {provider}: cancelled")]
    Cancelled { provider: String },

    #[error("provider {provider}: unknown error: {detail}")]
    Unknown { provider: String, detail: String },
}

impl ProviderError {
    pub fn provider(&self) -> &str {
        match self {
            ProviderError::Http { provider, .. } => provider,
            ProviderError::Auth { provider, .. } => provider,
            ProviderError::RateLimited { provider, .. } => provider,
            ProviderError::Timeout { provider, .. } => provider,
            ProviderError::ModelNotFound { provider, .. } => provider,
            ProviderError::UnsupportedCapability { provider, .. } => provider,
            ProviderError::Streaming { provider, .. } => provider,
            ProviderError::Connection { provider, .. } => provider,
            ProviderError::Serialization { provider, .. } => provider,
            ProviderError::ResponseTooLarge { provider, .. } => provider,
            ProviderError::ContextExceeded { provider, .. } => provider,
            ProviderError::EmptyResponse { provider, .. } => provider,
            ProviderError::Cancelled { provider, .. } => provider,
            ProviderError::Unknown { provider, .. } => provider,
        }
    }

    pub fn is_retryable(&self) -> bool {
        matches!(
            self,
            ProviderError::RateLimited { .. }
                | ProviderError::Timeout { .. }
                | ProviderError::Connection { .. }
                | ProviderError::Http { status: 429 | 502 | 503 | 504, .. }
        )
    }

    pub fn is_auth_error(&self) -> bool {
        matches!(self, ProviderError::Auth { .. } | ProviderError::Http { status: 401 | 403, .. })
    }
}
