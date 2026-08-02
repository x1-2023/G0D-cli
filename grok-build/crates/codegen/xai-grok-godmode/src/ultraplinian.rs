use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum UltraplinianTier {
    Fast,
    Standard,
    Smart,
    Power,
    Ultra,
}

impl UltraplinianTier {
    pub fn model_count(&self) -> usize {
        match self {
            UltraplinianTier::Fast => 12,
            UltraplinianTier::Standard => 27,
            UltraplinianTier::Smart => 41,
            UltraplinianTier::Power => 53,
            UltraplinianTier::Ultra => 60,
        }
    }

    pub fn label(&self) -> &str {
        match self {
            UltraplinianTier::Fast => "FAST (12 models)",
            UltraplinianTier::Standard => "STANDARD (27 models)",
            UltraplinianTier::Smart => "SMART (41 models)",
            UltraplinianTier::Power => "POWER (53 models)",
            UltraplinianTier::Ultra => "ULTRA (60 models)",
        }
    }
}

pub fn tier_models(tier: &UltraplinianTier) -> Vec<String> {
    match tier {
        UltraplinianTier::Fast => fast_models(),
        UltraplinianTier::Standard => { let mut m = fast_models(); m.extend(standard_models()); m }
        UltraplinianTier::Smart => { let mut m = fast_models(); m.extend(standard_models()); m.extend(smart_models()); m }
        UltraplinianTier::Power => { let mut m = fast_models(); m.extend(standard_models()); m.extend(smart_models()); m.extend(power_models()); m }
        UltraplinianTier::Ultra => { let mut m = fast_models(); m.extend(standard_models()); m.extend(smart_models()); m.extend(power_models()); m.extend(ultra_models()); m }
    }
}

fn fast_models() -> Vec<String> {
    vec![
        "openrouter:google/gemini-2.5-flash".into(),
        "openrouter:deepseek/deepseek-chat".into(),
        "openrouter:perplexity/sonar".into(),
        "openrouter:meta-llama/llama-3.1-8b-instruct".into(),
        "openrouter:moonshotai/kimi-k2.5".into(),
        "openrouter:mistralai/mistral-small-3.2".into(),
        "openrouter:nvidia/nemotron-nano".into(),
        "openrouter:z-ai/glm-5-turbo".into(),
        "openrouter:qwen/qwen-2.5-7b-instruct".into(),
        "openrouter:google/gemma-3-12b".into(),
        "openrouter:cohere/command-a".into(),
        "openrouter:sao10k/l3.3-euryale-70b".into(),
    ]
}

fn standard_models() -> Vec<String> {
    vec![
        "openrouter:anthropic/claude-sonnet-5".into(),
        "openrouter:openai/gpt-4o".into(),
        "openrouter:google/gemini-2.5-pro".into(),
        "openrouter:meta-llama/llama-4-scout".into(),
        "openrouter:nousresearch/hermes-3-70b".into(),
        "openrouter:mistralai/mixtral-8x22b".into(),
        "openrouter:qwen/qwen-2.5-72b-instruct".into(),
        "openrouter:deepseek/deepseek-r1-distill-llama-70b".into(),
        "openrouter:meta-llama/llama-3.3-70b-instruct".into(),
        "openrouter:google/gemma-2-27b".into(),
        "openrouter:qwen/qwq-32b".into(),
        "openrouter:infinilam/infinimixtral-8x22b".into(),
        "openrouter:alpindale/magnum-v4-72b".into(),
        "openrouter:nousresearch/deephermes-3-24b".into(),
        "openrouter:eva-unit-01/eva-qwen-2.5-72b".into(),
    ]
}

fn smart_models() -> Vec<String> {
    vec![
        "openrouter:openai/gpt-5".into(),
        "openrouter:openai/gpt-5.2".into(),
        "openrouter:anthropic/claude-opus-4-6".into(),
        "openrouter:anthropic/claude-fable-5".into(),
        "openrouter:qwen/qwen-3.5-max".into(),
        "openrouter:z-ai/glm-5".into(),
        "openrouter:z-ai/glm-5.2".into(),
        "openrouter:deepseek/deepseek-r1".into(),
        "openrouter:google/gemini-2.5-pro-experimental".into(),
        "openrouter:x-ai/grok-3".into(),
        "openrouter:meta-llama/llama-4-maverick".into(),
        "openrouter:qwen/qwen-3.7-max".into(),
        "openrouter:openai/o3".into(),
        "openrouter:anthropic/claude-sonnet-4.5".into(),
    ]
}

fn power_models() -> Vec<String> {
    vec![
        "openrouter:x-ai/grok-4.5".into(),
        "openrouter:openai/gpt-5.4".into(),
        "openrouter:openai/gpt-5.5-pro".into(),
        "openrouter:openai/gpt-5.6-luna".into(),
        "openrouter:meta-llama/llama-4-maverick-17b".into(),
        "openrouter:qwen/qwen-235b".into(),
        "openrouter:qwen/qwen-coder".into(),
        "openrouter:minimax/minimax-m2.5".into(),
        "openrouter:mistralai/mistral-large".into(),
        "openrouter:google/gemini-3.1-pro".into(),
        "openrouter:anthropic/claude-opus-4-8".into(),
        "openrouter:openai/gpt-5.5".into(),
    ]
}

fn ultra_models() -> Vec<String> {
    vec![
        "openrouter:anthropic/claude-opus-4-8".into(),
        "openrouter:nousresearch/hermes-4-405b-v3".into(),
        "openrouter:nousresearch/hermes-4-405b-v4".into(),
        "openrouter:openai/gpt-5.6-terra".into(),
        "openrouter:openai/gpt-5.6-sol".into(),
        "openrouter:mistralai/codestral".into(),
        "openrouter:anthropic/claude-opus-4-6-fast".into(),
    ]
}
