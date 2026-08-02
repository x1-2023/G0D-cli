use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum PrivacyMode {
    Standard,
    NoLog,
    LocalOnly,
    PrivacyPreview,
}

impl PrivacyMode {
    pub fn label(&self) -> &str {
        match self {
            PrivacyMode::Standard => "Standard",
            PrivacyMode::NoLog => "No-Log",
            PrivacyMode::LocalOnly => "Local-Only",
            PrivacyMode::PrivacyPreview => "Privacy Preview",
        }
    }
}

impl Default for PrivacyMode {
    fn default() -> Self { Self::Standard }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrivacyConfig {
    pub mode: PrivacyMode,
    pub remote_provider_warning: bool,
    pub telemetry: bool,
    pub dataset_contribution: bool,
    pub preview_before_remote: bool,
}

impl Default for PrivacyConfig {
    fn default() -> Self {
        Self {
            mode: PrivacyMode::Standard,
            remote_provider_warning: true,
            telemetry: false,
            dataset_contribution: false,
            preview_before_remote: true,
        }
    }
}

impl PrivacyConfig {
    pub fn is_local_only(&self) -> bool {
        self.mode == PrivacyMode::LocalOnly
    }

    pub fn is_no_log(&self) -> bool {
        matches!(self.mode, PrivacyMode::NoLog | PrivacyMode::LocalOnly)
    }

    pub fn requires_preview(&self) -> bool {
        self.mode == PrivacyMode::PrivacyPreview || self.preview_before_remote
    }

    pub fn allow_remote(&self, provider_is_local: bool) -> bool {
        if self.is_local_only() { return provider_is_local; }
        true
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrivacyPreview {
    pub providers: Vec<ProviderPreview>,
    pub total_estimated_tokens: u64,
    pub total_estimated_cost_usd: f64,
    pub files_sent: Vec<String>,
    pub attachments_count: usize,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderPreview {
    pub provider_id: String,
    pub models: Vec<String>,
    pub estimated_tokens: u64,
    pub estimated_cost_usd: f64,
    pub is_local: bool,
}

impl PrivacyPreview {
    pub fn new() -> Self {
        Self {
            providers: vec![],
            total_estimated_tokens: 0,
            total_estimated_cost_usd: 0.0,
            files_sent: vec![],
            attachments_count: 0,
            warnings: vec![],
        }
    }

    pub fn has_remote_providers(&self) -> bool {
        self.providers.iter().any(|p| !p.is_local)
    }

    pub fn summary(&self) -> String {
        if self.providers.is_empty() {
            return "No providers configured.".into();
        }
        let remote: Vec<_> = self.providers.iter().filter(|p| !p.is_local).collect();
        let local: Vec<_> = self.providers.iter().filter(|p| p.is_local).collect();
        format!(
            "Privacy Preview: {} remote provider(s), {} local provider(s), ~{} tokens, ~${:.4}",
            remote.len(), local.len(), self.total_estimated_tokens, self.total_estimated_cost_usd,
        )
    }
}
