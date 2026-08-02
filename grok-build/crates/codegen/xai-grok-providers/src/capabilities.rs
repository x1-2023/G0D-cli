use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ProviderCapabilities {
    pub streaming: bool,
    pub tool_calling: bool,
    pub parallel_tool_calls: bool,
    pub vision: bool,
    pub audio_input: bool,
    pub reasoning: bool,
    pub reasoning_summary: bool,
    pub json_mode: bool,
    pub json_schema: bool,
    pub model_discovery: bool,
    pub token_usage: bool,
    pub pricing: bool,
    pub context_window: bool,
    pub max_output_tokens: bool,
    pub system_prompt: bool,
    pub developer_prompt: bool,
    pub prompt_caching: bool,
}

impl ProviderCapabilities {
    pub fn all() -> Self {
        Self {
            streaming: true,
            tool_calling: true,
            parallel_tool_calls: true,
            vision: true,
            audio_input: true,
            reasoning: true,
            reasoning_summary: true,
            json_mode: true,
            json_schema: true,
            model_discovery: true,
            token_usage: true,
            pricing: true,
            context_window: true,
            max_output_tokens: true,
            system_prompt: true,
            developer_prompt: true,
            prompt_caching: true,
        }
    }
}
