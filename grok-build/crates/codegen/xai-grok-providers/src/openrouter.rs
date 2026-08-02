use async_trait::async_trait;
use futures::stream::{self, StreamExt};
use reqwest::Client;
use std::sync::Arc;
use std::time::Duration;
use crate::capabilities::ProviderCapabilities;
use crate::config::ProviderConfig;
use crate::error::ProviderError;
use crate::model_catalog::ModelInfo;
use crate::model_health::{HealthState, ProviderHealth};
use crate::provider::ModelProvider;
use crate::request::ModelRequest;
use crate::response::{ModelResponse, ModelStream, ModelStreamEvent, StreamEvent, UsageInfo};

pub struct OpenRouterProvider {
    config: ProviderConfig,
    client: Client,
    models_cache: tokio::sync::RwLock<Option<(Vec<ModelInfo>, std::time::Instant)>>,
}

impl OpenRouterProvider {
    pub fn new(config: ProviderConfig) -> Result<Self, ProviderError> {
        let client = Client::builder()
            .timeout(Duration::from_secs(config.request_timeout_seconds))
            .build()
            .map_err(|e| ProviderError::Connection { provider: config.id.clone(), detail: e.to_string() })?;

        Ok(Self {
            config,
            client,
            models_cache: tokio::sync::RwLock::new(None),
        })
    }

    fn api_key(&self) -> Result<String, ProviderError> {
        self.config.resolve_api_key().ok_or_else(|| ProviderError::Auth {
            provider: self.config.id.clone(),
            detail: format!("API key not set. Set {} or pass api_key", self.config.api_key_env.as_deref().unwrap_or("API key")),
        })
    }

    fn build_headers(&self) -> Result<Vec<(String, String)>, ProviderError> {
        let mut headers = vec![
            ("Authorization".into(), format!("Bearer {}", self.api_key()?)),
            ("Content-Type".into(), "application/json".into()),
        ];
        if let Some(ref referer) = self.config.http_referer {
            headers.push(("HTTP-Referer".into(), referer.clone()));
        }
        if let Some(ref title) = self.config.x_title {
            headers.push(("X-Title".into(), title.clone()));
        }
        for (k, v) in &self.config.extra_headers {
            headers.push((k.clone(), v.clone()));
        }
        Ok(headers)
    }

    fn to_openai_body(&self, req: &ModelRequest) -> serde_json::Value {
        let messages: Vec<serde_json::Value> = req.messages.iter().map(|m| {
            let role = match m.role {
                crate::request::MessageRole::System => "system",
                crate::request::MessageRole::User => "user",
                crate::request::MessageRole::Assistant => "assistant",
                crate::request::MessageRole::Tool => "tool",
                crate::request::MessageRole::Developer => "developer",
            };
            let mut msg = serde_json::json!({ "role": role });

            match &m.content {
                crate::request::MessageContent::Text(text) => {
                    msg["content"] = serde_json::Value::String(text.clone());
                }
                crate::request::MessageContent::MultiPart(parts) => {
                    let content: Vec<serde_json::Value> = parts.iter().map(|p| match p {
                        crate::request::ContentPart::Text { text } => {
                            serde_json::json!({ "type": "text", "text": text })
                        }
                        crate::request::ContentPart::ImageUrl { image_url } => {
                            serde_json::json!({ "type": "image_url", "image_url": { "url": image_url.url } })
                        }
                        _ => serde_json::json!({}),
                    }).collect();
                    msg["content"] = serde_json::Value::Array(content);
                }
            }

            if let Some(ref name) = m.name { msg["name"] = serde_json::Value::String(name.clone()); }
            if let Some(ref tool_id) = m.tool_call_id { msg["tool_call_id"] = serde_json::Value::String(tool_id.clone()); }
            msg
        }).collect();

        let mut body = serde_json::json!({
            "model": req.model,
            "messages": messages,
        });

        if let Some(temp) = req.temperature { body["temperature"] = serde_json::Value::Number(serde_json::Number::from_f64(temp as f64).unwrap()); }
        if let Some(top_p) = req.top_p { body["top_p"] = serde_json::Value::Number(serde_json::Number::from_f64(top_p as f64).unwrap()); }
        if let Some(top_k) = req.top_k { body["top_k"] = serde_json::Value::Number(top_k.into()); }
        if let Some(max_tokens) = req.max_tokens { body["max_tokens"] = serde_json::Value::Number(max_tokens.into()); }
        if let Some(fp) = req.frequency_penalty { body["frequency_penalty"] = serde_json::Value::Number(serde_json::Number::from_f64(fp as f64).unwrap()); }
        if let Some(pp) = req.presence_penalty { body["presence_penalty"] = serde_json::Value::Number(serde_json::Number::from_f64(pp as f64).unwrap()); }
        if let Some(rp) = req.repetition_penalty { body["repetition_penalty"] = serde_json::Value::Number(serde_json::Number::from_f64(rp as f64).unwrap()); }

        if !req.tools.is_empty() {
            let tools: Vec<serde_json::Value> = req.tools.iter().map(|t| {
                serde_json::json!({
                    "type": "function",
                    "function": {
                        "name": t.name,
                        "description": t.description,
                        "parameters": t.parameters,
                    }
                })
            }).collect();
            body["tools"] = serde_json::Value::Array(tools);
        }

        match &req.tool_choice {
            Some(crate::request::ToolChoice::Auto) => { body["tool_choice"] = serde_json::Value::String("auto".into()); }
            Some(crate::request::ToolChoice::None) => { body["tool_choice"] = serde_json::Value::String("none".into()); }
            Some(crate::request::ToolChoice::Required) => { body["tool_choice"] = serde_json::Value::String("required".into()); }
            Some(crate::request::ToolChoice::Specific { name }) => {
                body["tool_choice"] = serde_json::json!({ "type": "function", "function": { "name": name } });
            }
            None => {}
        }

        if let Some(ref schema) = req.json_schema {
            body["response_format"] = serde_json::json!({
                "type": "json_schema",
                "json_schema": schema,
            });
        }

        body
    }

    fn parse_stream_event(line: &str) -> Option<Result<ModelStreamEvent, ProviderError>> {
        let data = line.strip_prefix("data: ")?.trim().to_string();
        if data == "[DONE]" { return None; }
        let parsed: serde_json::Value = serde_json::from_str(&data).ok()?;
        let choices = parsed.get("choices")?.as_array()?;
        let choice = choices.first()?;
        let delta = choice.get("delta")?;

        if let Some(content) = delta.get("content").and_then(|c| c.as_str()) {
            return Some(Ok(ModelStreamEvent {
                event: StreamEvent::ContentDelta { text: content.to_string() },
                finish_reason: None,
                usage: None,
                model_metadata: None,
            }));
        }

        if let Some(tool_calls) = delta.get("tool_calls").and_then(|t| t.as_array()) {
            for tc in tool_calls {
                let id = tc.get("id").and_then(|i| i.as_str()).unwrap_or("");
                let name = tc.get("function").and_then(|f| f.get("name")).and_then(|n| n.as_str());
                let args = tc.get("function").and_then(|f| f.get("arguments")).and_then(|a| a.as_str());

                if let Some(args) = args {
                    return Some(Ok(ModelStreamEvent {
                        event: StreamEvent::ToolCallDelta {
                            id: id.to_string(),
                            name: name.map(|n| n.to_string()),
                            arguments_delta: Some(args.to_string()),
                        },
                        finish_reason: None,
                        usage: None,
                        model_metadata: None,
                    }));
                }
            }
        }

        let finish_reason = choice.get("finish_reason").and_then(|f| f.as_str()).map(|s| s.to_string());
        if finish_reason.is_some() {
            let usage = parsed.get("usage").map(|u| UsageInfo {
                prompt_tokens: u.get("prompt_tokens").and_then(|v| v.as_u64()),
                completion_tokens: u.get("completion_tokens").and_then(|v| v.as_u64()),
                total_tokens: u.get("total_tokens").and_then(|v| v.as_u64()),
                reasoning_tokens: None,
                cached_tokens: None,
            });

            return Some(Ok(ModelStreamEvent {
                event: StreamEvent::Completed,
                finish_reason,
                usage,
                model_metadata: None,
            }));
        }

        None
    }
}

#[async_trait]
impl ModelProvider for OpenRouterProvider {
    fn id(&self) -> &str { &self.config.id }
    fn display_name(&self) -> &str { &self.config.display_name }
    fn capabilities(&self) -> ProviderCapabilities { ProviderCapabilities::all() }
    fn is_local(&self) -> bool { false }
    fn config(&self) -> &ProviderConfig { &self.config }

    async fn health(&self) -> ProviderHealth {
        let url = format!("{}/models", self.config.base_url.as_deref().unwrap_or("https://openrouter.ai/api/v1"));
        match self.api_key() {
            Ok(key) => {
                let result = self.client.get(&url)
                    .header("Authorization", format!("Bearer {}", key))
                    .timeout(Duration::from_secs(15))
                    .send()
                    .await;
                match result {
                    Ok(resp) if resp.status().is_success() => ProviderHealth::healthy(&self.config.id),
                    Ok(resp) if resp.status() == 401 || resp.status() == 403 => ProviderHealth {
                        provider: self.config.id.clone(),
                        state: HealthState::Unauthorized,
                        detail: Some(format!("HTTP {}", resp.status())),
                        ..ProviderHealth::unknown(&self.config.id)
                    },
                    Ok(resp) => ProviderHealth {
                        provider: self.config.id.clone(),
                        state: HealthState::Degraded,
                        detail: Some(format!("HTTP {}", resp.status())),
                        ..ProviderHealth::unknown(&self.config.id)
                    },
                    Err(e) => ProviderHealth {
                        provider: self.config.id.clone(),
                        state: HealthState::Unavailable,
                        detail: Some(e.to_string()),
                        ..ProviderHealth::unknown(&self.config.id)
                    },
                }
            }
            Err(_) => ProviderHealth {
                provider: self.config.id.clone(),
                state: HealthState::Unauthorized,
                detail: Some("API key not configured".into()),
                ..ProviderHealth::unknown(&self.config.id)
            },
        }
    }

    async fn list_models(&self) -> Result<Vec<ModelInfo>, ProviderError> {
        {
            let cache = self.models_cache.read().await;
            if let Some((ref models, timestamp)) = *cache {
                if timestamp.elapsed() < Duration::from_secs(self.config.model_cache_ttl_seconds) {
                    return Ok(models.clone());
                }
            }
        }

        let url = format!("{}/models", self.config.base_url.as_deref().unwrap_or("https://openrouter.ai/api/v1"));
        let resp = self.client.get(&url)
            .headers(self.build_headers()?.iter().map(|(k, v)| {
                (reqwest::header::HeaderName::from_bytes(k.as_bytes()).unwrap(),
                 reqwest::header::HeaderValue::from_str(v).unwrap())
            }).collect::<reqwest::header::HeaderMap>())
            .send()
            .await
            .map_err(|e| ProviderError::Connection { provider: self.config.id.clone(), detail: e.to_string() })?;

        if !resp.status().is_success() {
            return Err(ProviderError::Http {
                provider: self.config.id.clone(),
                status: resp.status().as_u16(),
                body: resp.text().await.unwrap_or_default(),
            });
        }

        let body: serde_json::Value = resp.json().await.map_err(|e| ProviderError::Serialization {
            provider: self.config.id.clone(), detail: e.to_string(),
        })?;

        let models: Vec<ModelInfo> = body["data"].as_array().unwrap_or(&vec![])
            .iter()
            .map(|m| {
                let id = m["id"].as_str().unwrap_or("").to_string();
                ModelInfo {
                    provider: self.config.id.clone(),
                    model_id: id.clone(),
                    display_name: m["name"].as_str().map(|s| s.to_string()),
                    context_window: m["context_length"].as_u64().map(|c| c as usize),
                    max_output_tokens: m["top_provider"]["max_completion_tokens"].as_u64().map(|c| c as usize),
                    capabilities: ProviderCapabilities {
                        streaming: true,
                        tool_calling: m.get("supported_parameters").and_then(|p| p.as_array()).map(|a| a.iter().any(|s| s.as_str() == Some("tools"))).unwrap_or(false),
                        parallel_tool_calls: m.get("supported_parameters").and_then(|p| p.as_array()).map(|a| a.iter().any(|s| s.as_str() == Some("parallel_tool_calls"))).unwrap_or(false),
                        vision: m.get("architecture").and_then(|a| a.get("modality")).and_then(|mo| mo.as_str()).map(|s| s.contains("image")).unwrap_or(false),
                        ..Default::default()
                    },
                    pricing: m.get("pricing").map(|p| crate::model_catalog::ModelPricing {
                        prompt_price_per_1m_tokens: p.get("prompt").and_then(|v| v.as_str()).and_then(|s| s.parse().ok()),
                        completion_price_per_1m_tokens: p.get("completion").and_then(|v| v.as_str()).and_then(|s| s.parse().ok()),
                        image_price_per_image: p.get("image").and_then(|v| v.as_str()).and_then(|s| s.parse().ok()),
                        currency: "USD".into(),
                    }),
                    aliases: vec![],
                    categories: vec![],
                    deprecated: false,
                    replacement: None,
                }
            })
            .collect();

        let mut cache = self.models_cache.write().await;
        *cache = Some((models.clone(), std::time::Instant::now()));
        Ok(models)
    }

    async fn complete(&self, request: ModelRequest) -> Result<ModelResponse, ProviderError> {
        let url = format!("{}/chat/completions", self.config.base_url.as_deref().unwrap_or("https://openrouter.ai/api/v1"));
        let body = self.to_openai_body(&request);

        let resp = self.client.post(&url)
            .headers(self.build_headers()?.iter().map(|(k, v)| {
                (reqwest::header::HeaderName::from_bytes(k.as_bytes()).unwrap(),
                 reqwest::header::HeaderValue::from_str(v).unwrap())
            }).collect::<reqwest::header::HeaderMap>())
            .json(&body)
            .send()
            .await
            .map_err(|e| ProviderError::Connection { provider: self.config.id.clone(), detail: e.to_string() })?;

        let status = resp.status();
        if !status.is_success() {
            return Err(ProviderError::Http {
                provider: self.config.id.clone(),
                status: status.as_u16(),
                body: resp.text().await.unwrap_or_default(),
            });
        }

        let json: serde_json::Value = resp.json().await.map_err(|e| ProviderError::Serialization {
            provider: self.config.id.clone(), detail: e.to_string(),
        })?;

        let choice = json["choices"].as_array().and_then(|c| c.first())
            .ok_or_else(|| ProviderError::EmptyResponse { provider: self.config.id.clone() })?;

        let content = choice["message"]["content"].as_str().unwrap_or("").to_string();

        let tool_calls: Vec<crate::response::ToolCall> = choice["message"]["tool_calls"]
            .as_array()
            .map(|arr| arr.iter().filter_map(|tc| {
                Some(crate::response::ToolCall {
                    id: tc["id"].as_str()?.to_string(),
                    name: tc["function"]["name"].as_str()?.to_string(),
                    arguments: tc["function"]["arguments"].clone(),
                })
            }).collect())
            .unwrap_or_default();

        let usage = json.get("usage").map(|u| UsageInfo {
            prompt_tokens: u.get("prompt_tokens").and_then(|v| v.as_u64()),
            completion_tokens: u.get("completion_tokens").and_then(|v| v.as_u64()),
            total_tokens: u.get("total_tokens").and_then(|v| v.as_u64()),
            reasoning_tokens: None,
            cached_tokens: None,
        });

        Ok(ModelResponse {
            id: json["id"].as_str().unwrap_or("").to_string(),
            model: json["model"].as_str().unwrap_or(&request.model).to_string(),
            provider: self.config.id.clone(),
            content,
            finish_reason: choice["finish_reason"].as_str().map(|s| s.to_string()),
            tool_calls,
            usage,
            model_metadata: None,
        })
    }

    async fn stream(&self, request: ModelRequest) -> Result<ModelStream, ProviderError> {
        let url = format!("{}/chat/completions", self.config.base_url.as_deref().unwrap_or("https://openrouter.ai/api/v1"));
        let mut body = self.to_openai_body(&request);
        body["stream"] = serde_json::Value::Bool(true);

        let resp = self.client.post(&url)
            .headers(self.build_headers()?.iter().map(|(k, v)| {
                (reqwest::header::HeaderName::from_bytes(k.as_bytes()).unwrap(),
                 reqwest::header::HeaderValue::from_str(v).unwrap())
            }).collect::<reqwest::header::HeaderMap>())
            .json(&body)
            .send()
            .await
            .map_err(|e| ProviderError::Connection { provider: self.config.id.clone(), detail: e.to_string() })?;

        if !resp.status().is_success() {
            return Err(ProviderError::Http {
                provider: self.config.id.clone(),
                status: resp.status().as_u16(),
                body: resp.text().await.unwrap_or_default(),
            });
        }

        let stream = resp.bytes_stream()
            .map(move |chunk| -> Result<ModelStreamEvent, ProviderError> {
                let bytes = chunk.map_err(|e| ProviderError::Streaming {
                    provider: "openrouter".into(), detail: e.to_string(),
                })?;
                let text = String::from_utf8_lossy(&bytes);
                for line in text.lines() {
                    if let Some(event) = Self::parse_stream_event(line) {
                        return event;
                    }
                }
                Ok(ModelStreamEvent {
                    event: StreamEvent::ContentDelta { text: String::new() },
                    finish_reason: None, usage: None, model_metadata: None,
                })
            })
            .filter(|r| futures::future::ready(!matches!(r, Ok(ModelStreamEvent { event: StreamEvent::ContentDelta { text }, .. }) if text.is_empty())));

        Ok(Box::new(stream))
    }
}
