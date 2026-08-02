use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelResponse {
    pub id: String,
    pub model: String,
    pub provider: String,
    pub content: String,
    pub finish_reason: Option<String>,
    pub tool_calls: Vec<ToolCall>,
    pub usage: Option<UsageInfo>,
    pub model_metadata: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub arguments: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsageInfo {
    pub prompt_tokens: Option<u64>,
    pub completion_tokens: Option<u64>,
    pub total_tokens: Option<u64>,
    pub reasoning_tokens: Option<u64>,
    pub cached_tokens: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelStreamEvent {
    pub event: StreamEvent,
    pub finish_reason: Option<String>,
    pub usage: Option<UsageInfo>,
    pub model_metadata: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum StreamEvent {
    ContentDelta { text: String },
    ReasoningDelta { text: String },
    ToolCallDelta { id: String, name: Option<String>, arguments_delta: Option<String> },
    ToolCallComplete { id: String, name: String, arguments: serde_json::Value },
    ResponseStarted,
    ReasoningCompleted,
    Completed,
}

pub type ModelStream = Box<dyn futures::Stream<Item = Result<ModelStreamEvent, crate::error::ProviderError>> + Send + Unpin>;
