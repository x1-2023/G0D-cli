use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GodmodeCliFlags {
    pub provider: Option<String>,
    pub model: Option<String>,
    pub mode: CliMode,
    pub godmode_profile: Option<String>,
    pub ultra_tier: Option<String>,
    pub judge_model: Option<String>,
    pub judge_count: Option<usize>,
    pub autotune: Option<bool>,
    pub parseltongue: Option<String>,
    pub local_only: bool,
    pub no_log: bool,
    pub privacy_preview: bool,
    pub image: Vec<String>,
    pub max_candidates: Option<usize>,
    pub max_cost_usd: Option<f64>,
    pub max_input_tokens: Option<u64>,
    pub max_output_tokens: Option<u64>,
    pub list_models: bool,
    pub test_providers: bool,
    pub export_race: Option<String>,
    pub headless: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum CliMode {
    Single,
    Godmode,
    Ultra,
}

impl Default for CliMode {
    fn default() -> Self { Self::Single }
}

impl Default for GodmodeCliFlags {
    fn default() -> Self {
        Self {
            provider: None, model: None, mode: CliMode::Single,
            godmode_profile: None, ultra_tier: None,
            judge_model: None, judge_count: None,
            autotune: None, parseltongue: None,
            local_only: false, no_log: false, privacy_preview: false,
            image: vec![], max_candidates: None,
            max_cost_usd: None, max_input_tokens: None, max_output_tokens: None,
            list_models: false, test_providers: false,
            export_race: None, headless: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlashCommand {
    pub command: String,
    pub args: Vec<String>,
}

impl SlashCommand {
    pub fn parse(input: &str) -> Option<Self> {
        let input = input.trim();
        if !input.starts_with('/') { return None; }
        let mut parts: Vec<String> = input.split_whitespace().map(|s| s.to_string()).collect();
        let command = parts.remove(0);
        Some(Self { command, args: parts })
    }
}

pub const SLASH_COMMANDS: &[(&str, &str)] = &[
    ("/godmode", "on|off|classic|profile|candidates|models|judge|compare|export"),
    ("/ultra", "fast|standard|smart|power|ultra|custom"),
    ("/providers", "|test|health"),
    ("/models", "|refresh|search"),
    ("/model", "provider:model/id"),
    ("/autotune", "on|off|status"),
    ("/parseltongue", "off|light|standard|heavy|preview"),
    ("/privacy", "|no-log|local-only|preview"),
    ("/race", "|compare|rejudge|winner|merge|export|cancel"),
    ("/key", "sk-or-v1-..."),
    ("/status", ""),
    ("/help", ""),
];
