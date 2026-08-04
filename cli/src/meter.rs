//! Context window metering, token estimates, and session compaction.

use crate::config::Config;
use serde_json::Value;

/// Rough token estimate for mixed English / Vietnamese text.
/// ASCII-heavy text ≈ chars/4; non-ASCII heavy (VI/CJK) ≈ chars×2/3.
pub fn estimate_tokens_text(text: &str) -> usize {
    let chars = text.chars().count();
    if chars == 0 {
        return 0;
    }
    let non_ascii = text.chars().filter(|c| !c.is_ascii()).count();
    if non_ascii.saturating_mul(2) > chars {
        ((chars * 2) / 3).max(1)
    } else {
        chars.div_ceil(4)
    }
}

pub fn estimate_message_tokens(message: &Value) -> usize {
    let role = message
        .get("role")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    let mut total = estimate_tokens_text(role) + 4;
    if let Some(content) = message.get("content").and_then(Value::as_str) {
        total += estimate_tokens_text(content);
    } else if let Some(content) = message.get("content") {
        total += estimate_tokens_text(&content.to_string());
    }
    if let Some(tools) = message.get("tool_calls") {
        total += estimate_tokens_text(&tools.to_string());
    }
    total
}

pub fn estimate_messages_tokens(messages: &[Value]) -> usize {
    messages.iter().map(estimate_message_tokens).sum()
}

#[derive(Debug, Clone)]
pub struct ContextMeter {
    pub messages: usize,
    pub estimated_tokens: usize,
    pub budget: usize,
    pub message_cap: usize,
    pub lifetime_prompt: u64,
    pub lifetime_completion: u64,
    pub last_prompt: u64,
    pub last_completion: u64,
    pub compact_count: u32,
}

impl ContextMeter {
    pub fn usage_ratio(&self) -> f32 {
        if self.budget == 0 {
            return 0.0;
        }
        (self.estimated_tokens as f32 / self.budget as f32).min(9.99)
    }

    pub fn usage_pct(&self) -> u32 {
        ((self.usage_ratio() * 100.0).round() as u32).min(999)
    }

    pub fn needs_compact(&self, auto_compact: bool, keep_recent: usize) -> bool {
        if !auto_compact || self.messages <= keep_recent {
            return false;
        }
        self.messages >= self.message_cap
            || self.estimated_tokens >= self.budget.saturating_mul(85) / 100
    }

    pub fn bar(&self, width: usize) -> String {
        render_bar(self.usage_ratio().min(1.0), width)
    }

    pub fn short_label(&self) -> String {
        format!(
            "{}m · ~{} · {}%",
            self.messages,
            format_token_count(self.estimated_tokens),
            self.usage_pct().min(100)
        )
    }
}

pub fn meter_for(
    messages: &[Value],
    config: &Config,
    lifetime_prompt: u64,
    lifetime_completion: u64,
    last_prompt: u64,
    last_completion: u64,
    compact_count: u32,
) -> ContextMeter {
    ContextMeter {
        messages: messages.len(),
        estimated_tokens: estimate_messages_tokens(messages),
        budget: config.context_token_budget.max(1_000),
        message_cap: config.max_context_messages.max(2),
        lifetime_prompt,
        lifetime_completion,
        last_prompt,
        last_completion,
        compact_count,
    }
}

pub fn render_bar(ratio: f32, width: usize) -> String {
    let width = width.clamp(8, 40);
    let filled = ((ratio.clamp(0.0, 1.0) * width as f32).round() as usize).min(width);
    let empty = width - filled;
    format!("[{}{}]", "█".repeat(filled), "░".repeat(empty))
}

pub fn format_token_count(tokens: usize) -> String {
    if tokens >= 1000 {
        format!("{:.1}k", tokens as f64 / 1000.0)
    } else {
        format!("{tokens}")
    }
}

/// Human-readable duration for compact/agent footers (e.g. `14.0s`, `2m23s`).
pub fn format_duration(elapsed: std::time::Duration) -> String {
    let total_ms = elapsed.as_millis();
    if total_ms < 1000 {
        format!("{:.1}s", elapsed.as_secs_f64().max(0.1))
    } else if total_ms < 60_000 {
        format!("{:.1}s", elapsed.as_secs_f64())
    } else {
        let secs = elapsed.as_secs();
        let minutes = secs / 60;
        let rem = secs % 60;
        format!("{minutes}m{rem:02}s")
    }
}

/// Collapse older messages into a single prior-context block, keeping the most recent turns.
/// Returns a human-readable note when compaction happened (Grok-style before→after).
pub fn compact_messages(
    messages: &mut Vec<Value>,
    keep_recent: usize,
    force: bool,
) -> Option<String> {
    let keep_recent = keep_recent.max(2);
    if messages.len() <= keep_recent {
        return if force {
            Some("Nothing to compact — session is already short.".into())
        } else {
            None
        };
    }

    let started = std::time::Instant::now();
    let before = estimate_messages_tokens(messages);
    let split_at = messages.len() - keep_recent;
    let old: Vec<Value> = messages.drain(..split_at).collect();
    let removed = old.len();
    let summary = summarize_turns(&old);
    let old_tokens = estimate_messages_tokens(&old);
    let summary_msg = format!(
        "[compacted prior context — {removed} messages, ~{} tokens estimated]\n\
         Retain only durable facts from this summary. Prefer re-reading files over inventing details.\n\n\
         {summary}",
        format_token_count(old_tokens)
    );
    messages.insert(
        0,
        serde_json::json!({
            "role": "user",
            "content": summary_msg
        }),
    );
    messages.insert(
        1,
        serde_json::json!({
            "role": "assistant",
            "content": "Understood. Continuing from the compacted prior context; I will re-check files before editing."
        }),
    );
    let after = estimate_messages_tokens(messages);
    let elapsed = format_duration(started.elapsed());
    Some(format!(
        "Context compacted: {} → {} tokens ({elapsed})",
        format_token_count(before),
        format_token_count(after),
    ))
}

fn summarize_turns(messages: &[Value]) -> String {
    let mut lines = Vec::new();
    let mut kept = 0usize;
    for message in messages {
        let role = message
            .get("role")
            .and_then(Value::as_str)
            .unwrap_or("unknown");
        if role == "system" || role == "tool" {
            continue;
        }
        let content = message
            .get("content")
            .and_then(Value::as_str)
            .unwrap_or("")
            .trim();
        if content.is_empty() {
            continue;
        }
        // Prefer user objectives and final assistant conclusions.
        if role != "user" && role != "assistant" {
            continue;
        }
        let one_line = content
            .lines()
            .map(str::trim)
            .find(|line| !line.is_empty())
            .unwrap_or("");
        if one_line.is_empty() {
            continue;
        }
        let mut clipped: String = one_line.chars().take(220).collect();
        if one_line.chars().count() > 220 {
            clipped.push('…');
        }
        lines.push(format!("- {role}: {clipped}"));
        kept += 1;
        if kept >= 24 {
            lines.push("- … additional turns omitted".into());
            break;
        }
    }
    if lines.is_empty() {
        "(no durable turn summaries extracted)".into()
    } else {
        lines.join("\n")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn estimates_ascii_roughly_quarter_chars() {
        let text = "a".repeat(400);
        let tokens = estimate_tokens_text(&text);
        assert!((90..=120).contains(&tokens), "got {tokens}");
    }

    #[test]
    fn compact_keeps_recent_and_adds_summary() {
        let mut messages = vec![
            json!({"role":"user","content":"task one"}),
            json!({"role":"assistant","content":"done one"}),
            json!({"role":"user","content":"task two"}),
            json!({"role":"assistant","content":"done two"}),
            json!({"role":"user","content":"task three"}),
            json!({"role":"assistant","content":"done three"}),
        ];
        let note = compact_messages(&mut messages, 2, true).unwrap();
        assert!(
            note.starts_with("Context compacted:"),
            "unexpected note: {note}"
        );
        assert!(note.contains('→') || note.contains("->") || note.contains("→"), "got: {note}");
        assert!(note.contains("tokens"));
        assert_eq!(messages.len(), 4); // summary pair + 2 recent
        let first = messages[0].get("content").and_then(Value::as_str).unwrap();
        assert!(first.contains("compacted prior context"));
        assert!(first.contains("task one"));
    }

    #[test]
    fn format_duration_human() {
        let short = format_duration(std::time::Duration::from_millis(250));
        assert!(short.ends_with('s') && short.starts_with('0'), "got {short}");
        assert_eq!(format_duration(std::time::Duration::from_secs(14)), "14.0s");
        assert_eq!(format_duration(std::time::Duration::from_secs(143)), "2m23s");
    }

    #[test]
    fn bar_renders_fixed_width() {
        let bar = render_bar(0.5, 10);
        assert_eq!(bar.chars().count(), 12); // [ + 10 + ]
        assert!(bar.contains('█'));
        assert!(bar.contains('░'));
    }
}
