//! OpenAI-compatible HTTP helpers for first-party and third-party gateways.

use crate::config::{Config, ProviderEntry};
use anyhow::{Context, Result};
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use reqwest::{Client, RequestBuilder, StatusCode};
use serde_json::{json, Value};
use std::time::Duration;

pub fn http_client() -> Result<Client> {
    Client::builder()
        .connect_timeout(Duration::from_secs(15))
        .timeout(Duration::from_secs(180))
        .user_agent(format!("g0d/{}", env!("CARGO_PKG_VERSION")))
        .build()
        .context("Could not create HTTP client")
}

/// Resolve the active provider endpoint, honoring a process-level override.
pub fn active_endpoint(config: &Config) -> String {
    config
        .endpoint_override
        .as_deref()
        .unwrap_or(config.active_provider().endpoint.as_str())
        .trim()
        .trim_end_matches('/')
        .to_string()
}

pub fn chat_completions_url(config: &Config) -> String {
    let provider = config.active_provider();
    let base = active_endpoint(config);
    let path = provider.chat_path.trim();
    let path = if path.is_empty() {
        "/chat/completions"
    } else if path.starts_with('/') {
        path
    } else {
        // allow "chat/completions"
        return format!("{base}/{path}");
    };
    format!("{base}{path}")
}

pub fn models_url(config: &Config) -> String {
    let provider = config.active_provider();
    let base = active_endpoint(config);
    let path = provider
        .models_path
        .as_deref()
        .unwrap_or("/models")
        .trim();
    if path.is_empty() {
        return format!("{base}/models");
    }
    if path.starts_with('/') {
        format!("{base}{path}")
    } else {
        format!("{base}/{path}")
    }
}

pub fn apply_provider_auth(
    mut builder: RequestBuilder,
    provider: &ProviderEntry,
    key: &str,
) -> Result<RequestBuilder> {
    let style = provider
        .auth_style
        .as_deref()
        .unwrap_or("bearer")
        .to_ascii_lowercase();
    builder = match style.as_str() {
        "none" | "local" => builder,
        "x-api-key" | "x_api_key" => builder.header("x-api-key", key),
        "api-key" | "api_key" => builder.header("api-key", key),
        "raw" | "authorization" => builder.header("Authorization", key),
        // default OpenAI-compatible
        _ => builder.bearer_auth(key),
    };

    if !provider.extra_headers.is_empty() {
        let mut headers = HeaderMap::new();
        for (name, value) in &provider.extra_headers {
            let header_name = HeaderName::from_bytes(name.as_bytes())
                .with_context(|| format!("Invalid header name: {name}"))?;
            let header_value = HeaderValue::from_str(value)
                .with_context(|| format!("Invalid header value for {name}"))?;
            headers.insert(header_name, header_value);
        }
        builder = builder.headers(headers);
    }

    // OpenRouter (and some relays) prefer these optional identity headers.
    if provider.endpoint.contains("openrouter.ai") {
        builder = builder
            .header("HTTP-Referer", "https://github.com/x1-2023/G0D-cli")
            .header("X-Title", "g0d");
    }

    Ok(builder)
}

pub async fn post_chat_completions(
    config: &Config,
    key: &str,
    body: &Value,
) -> Result<reqwest::Response> {
    let client = http_client()?;
    let url = chat_completions_url(config);
    let provider = config.active_provider();
    let builder = client.post(&url).json(body);
    let builder = apply_provider_auth(builder, provider, key)?;
    builder
        .send()
        .await
        .with_context(|| format!("API request failed ({url})"))
}

/// Human-readable API failures for third-party gateways.
pub fn format_http_error(status: StatusCode, body: &str, url: &str) -> String {
    let snippet: String = body.chars().take(1200).collect();
    let hint = match status.as_u16() {
        401 | 403 => " Check API key (/provider key <id> <key>) and auth_style (bearer|x-api-key|raw).",
        404 => " Check endpoint base URL (usually ends with /v1) and chat_path (/chat/completions).",
        429 => " Rate limited — wait or switch model/provider.",
        500..=599 => " Upstream gateway error — retry or check provider status.",
        _ => "",
    };
    format!("HTTP {status} from {url}.{hint}\n{snippet}")
}

/// Minimal round-trip: list models if possible, else tiny chat completion.
pub async fn test_provider(config: &Config, key: &str) -> Result<String> {
    let client = http_client()?;
    let provider = config.active_provider();
    let models = models_url(config);
    let get = apply_provider_auth(client.get(&models), provider, key)?;
    if let Ok(response) = get.send().await {
        if response.status().is_success() {
            let payload: Value = response.json().await.unwrap_or(json!({}));
            let count = extract_model_ids(&payload).len();
            return Ok(format!(
                "OK · {} · GET {models} · {count} model id(s) visible · model={}",
                provider.id,
                config.default_model
            ));
        }
    }

    // Fallback: tiny non-streaming completion (works on most OpenAI-compatible relays).
    let url = chat_completions_url(config);
    let body = json!({
        "model": config.default_model,
        "messages": [{"role": "user", "content": "ping"}],
        "max_tokens": 8,
        "temperature": 0,
        "stream": false
    });
    let response = post_chat_completions(config, key, &body).await?;
    let status = response.status();
    let text = response.text().await.unwrap_or_default();
    if !status.is_success() {
        anyhow::bail!(format_http_error(status, &text, &url));
    }
    let payload: Value = serde_json::from_str(&text).unwrap_or(json!({}));
    let reply = payload
        .pointer("/choices/0/message/content")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim();
    Ok(format!(
        "OK · {} · POST {url} · model={} · reply_chars={}",
        provider.id,
        config.default_model,
        reply.chars().count()
    ))
}

pub async fn list_models(config: &Config, key: &str) -> Result<Vec<String>> {
    let client = http_client()?;
    let provider = config.active_provider();
    let url = models_url(config);
    let response = apply_provider_auth(client.get(&url), provider, key)?
        .send()
        .await
        .with_context(|| format!("Models request failed ({url})"))?;
    let status = response.status();
    let text = response.text().await.unwrap_or_default();
    if !status.is_success() {
        anyhow::bail!(format_http_error(status, &text, &url));
    }
    let payload: Value =
        serde_json::from_str(&text).with_context(|| "Models response was not JSON")?;
    let mut ids = extract_model_ids(&payload);
    ids.sort();
    ids.dedup();
    if ids.is_empty() {
        anyhow::bail!(
            "No model ids found in {url}. Body starts with: {}",
            text.chars().take(200).collect::<String>()
        );
    }
    Ok(ids)
}

fn extract_model_ids(payload: &Value) -> Vec<String> {
    let mut ids = Vec::new();
    if let Some(arr) = payload.get("data").and_then(Value::as_array) {
        for item in arr {
            if let Some(id) = item.get("id").and_then(Value::as_str) {
                ids.push(id.to_string());
            }
        }
    } else if let Some(arr) = payload.get("models").and_then(Value::as_array) {
        for item in arr {
            if let Some(id) = item.as_str() {
                ids.push(id.to_string());
            } else if let Some(id) = item.get("id").and_then(Value::as_str) {
                ids.push(id.to_string());
            }
        }
    } else if let Some(arr) = payload.as_array() {
        for item in arr {
            if let Some(id) = item.get("id").and_then(Value::as_str) {
                ids.push(id.to_string());
            }
        }
    }
    ids
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::ProviderEntry;
    use std::collections::BTreeMap;

    #[test]
    fn builds_chat_url_with_custom_path() {
        let mut config = Config::default();
        config.default_provider = "custom".into();
        config.providers.push(ProviderEntry {
            id: "custom".into(),
            endpoint: "https://relay.example/v1".into(),
            api_key: None,
            key_env: None,
            enabled: true,
            is_local: false,
            chat_path: "/v1/chat/completions".into(),
            models_path: Some("/v1/models".into()),
            auth_style: None,
            extra_headers: BTreeMap::new(),
            default_model: None,
        });
        // chat_path already includes /v1 — endpoint should not double-join wrongly
        config.providers.last_mut().unwrap().chat_path = "/chat/completions".into();
        assert_eq!(
            chat_completions_url(&config),
            "https://relay.example/v1/chat/completions"
        );
    }

    #[test]
    fn extracts_openai_model_list() {
        let payload = json!({"data":[{"id":"glm-5"},{"id":"gpt-4.1"}]});
        let ids = extract_model_ids(&payload);
        assert_eq!(ids, vec!["glm-5".to_string(), "gpt-4.1".to_string()]);
    }
}
