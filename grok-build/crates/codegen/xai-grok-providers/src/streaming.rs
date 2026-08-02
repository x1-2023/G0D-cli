use crate::error::ProviderError;
use crate::model_health::HealthState;

pub struct StreamingConfig {
    pub timeout_seconds: u64,
    pub max_idle_seconds: u64,
    pub buffer_size: usize,
}

impl Default for StreamingConfig {
    fn default() -> Self {
        Self { timeout_seconds: 300, max_idle_seconds: 30, buffer_size: 8192 }
    }
}

pub fn classify_streaming_error(err: &ProviderError, _provider: &str) -> Option<HealthState> {
    match err {
        ProviderError::RateLimited { .. } => Some(HealthState::RateLimited),
        ProviderError::Auth { .. } => Some(HealthState::Unauthorized),
        ProviderError::Connection { .. } | ProviderError::Timeout { .. } => Some(HealthState::Degraded),
        _ => None,
    }
}

pub fn normalize_tool_calls(
    raw_tool_calls: Vec<crate::response::ToolCall>,
    _provider: &str,
) -> Vec<crate::response::ToolCall> {
    raw_tool_calls
}

pub fn normalize_usage(
    raw: Option<crate::response::UsageInfo>,
) -> Option<crate::response::UsageInfo> {
    raw
}
