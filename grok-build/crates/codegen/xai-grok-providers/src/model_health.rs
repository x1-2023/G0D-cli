use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum HealthState {
    Unknown,
    Healthy,
    Degraded,
    RateLimited,
    Unauthorized,
    Unavailable,
    Deprecated,
}

impl HealthState {
    pub fn is_usable(&self) -> bool {
        matches!(self, HealthState::Healthy | HealthState::Degraded | HealthState::Unknown)
    }

    pub fn label(&self) -> &str {
        match self {
            HealthState::Unknown => "unknown",
            HealthState::Healthy => "healthy",
            HealthState::Degraded => "degraded",
            HealthState::RateLimited => "rate-limited",
            HealthState::Unauthorized => "unauthorized",
            HealthState::Unavailable => "unavailable",
            HealthState::Deprecated => "deprecated",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderHealth {
    pub provider: String,
    pub state: HealthState,
    pub last_success: Option<DateTime<Utc>>,
    pub last_failure: Option<DateTime<Utc>>,
    pub recent_latency_ms: Option<f64>,
    pub recent_failure_rate: f64,
    pub detail: Option<String>,
}

impl ProviderHealth {
    pub fn unknown(provider: impl Into<String>) -> Self {
        Self {
            provider: provider.into(),
            state: HealthState::Unknown,
            last_success: None,
            last_failure: None,
            recent_latency_ms: None,
            recent_failure_rate: 0.0,
            detail: None,
        }
    }

    pub fn healthy(provider: impl Into<String>) -> Self {
        Self {
            provider: provider.into(),
            state: HealthState::Healthy,
            last_success: Some(Utc::now()),
            last_failure: None,
            recent_latency_ms: None,
            recent_failure_rate: 0.0,
            detail: None,
        }
    }
}
