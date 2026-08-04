use crate::config::{self, Config};
use crate::meter::{self, ContextMeter};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub id: String,
    pub workspace: String,
    pub created_at: u64,
    pub updated_at: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    pub messages: Vec<Value>,
    #[serde(default)]
    pub total_prompt_tokens: u64,
    #[serde(default)]
    pub total_completion_tokens: u64,
    #[serde(default)]
    pub last_prompt_tokens: u64,
    #[serde(default)]
    pub last_completion_tokens: u64,
    #[serde(default)]
    pub compact_count: u32,
}

#[derive(Debug, Clone)]
pub struct SessionSummary {
    pub id: String,
    pub updated_at: u64,
    pub messages: usize,
    pub title: Option<String>,
}

impl Session {
    pub fn new() -> Result<Self> {
        Self::new_in(&std::env::current_dir()?)
    }

    fn new_in(workspace: &Path) -> Result<Self> {
        let now = now_millis()?;
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .context("System clock is before UNIX epoch")?
            .as_nanos();
        Ok(Self {
            id: format!("{nonce:x}-{}", std::process::id()),
            workspace: canonical_workspace(workspace)?,
            created_at: now,
            updated_at: now,
            title: None,
            messages: Vec::new(),
            total_prompt_tokens: 0,
            total_completion_tokens: 0,
            last_prompt_tokens: 0,
            last_completion_tokens: 0,
            compact_count: 0,
        })
    }

    pub fn meter(&self, config: &Config) -> ContextMeter {
        meter::meter_for(
            &self.messages,
            config,
            self.total_prompt_tokens,
            self.total_completion_tokens,
            self.last_prompt_tokens,
            self.last_completion_tokens,
            self.compact_count,
        )
    }

    pub fn record_usage(&mut self, prompt: u64, completion: u64) {
        self.last_prompt_tokens = prompt;
        self.last_completion_tokens = completion;
        self.total_prompt_tokens = self.total_prompt_tokens.saturating_add(prompt);
        self.total_completion_tokens = self.total_completion_tokens.saturating_add(completion);
    }

    /// Compact older turns when thresholds trip (or when forced).
    pub fn compact(&mut self, config: &Config, force: bool) -> Option<String> {
        let snapshot = self.meter(config);
        if !force
            && !snapshot.needs_compact(config.auto_compact, config.keep_recent_messages)
        {
            return None;
        }
        let note = meter::compact_messages(
            &mut self.messages,
            config.keep_recent_messages,
            force,
        )?;
        if note.starts_with("Nothing to compact") {
            return Some(note);
        }
        self.compact_count = self.compact_count.saturating_add(1);
        Some(note)
    }

    /// Capture a short title from the first user turn when none is set yet.
    pub fn ensure_title(&mut self, query: &str) {
        if self.title.is_some() {
            return;
        }
        let compact = query.split_whitespace().collect::<Vec<_>>().join(" ");
        if compact.is_empty() {
            return;
        }
        let mut title: String = compact.chars().take(72).collect();
        if compact.chars().count() > 72 {
            title.push('…');
        }
        self.title = Some(title);
    }

    pub fn save(&mut self) -> Result<PathBuf> {
        self.save_to(&config::sessions_dir())
    }

    fn save_to(&mut self, directory: &Path) -> Result<PathBuf> {
        validate_id(&self.id)?;
        self.updated_at = now_millis()?;
        std::fs::create_dir_all(directory).with_context(|| {
            format!(
                "Could not create session directory: {}",
                directory.display()
            )
        })?;
        let path = directory.join(format!("{}.json", self.id));
        let content = serde_json::to_vec_pretty(self).context("Could not serialize session")?;
        std::fs::write(&path, content)
            .with_context(|| format!("Could not save session: {}", path.display()))?;
        Ok(path)
    }

    pub fn load(id: &str) -> Result<Self> {
        Self::load_from(&config::sessions_dir(), id, &std::env::current_dir()?)
    }

    fn load_from(directory: &Path, id: &str, workspace: &Path) -> Result<Self> {
        let expected_workspace = canonical_workspace(workspace)?;
        let resolved_id = if id == "latest" {
            list_from(directory, &expected_workspace)?
                .into_iter()
                .filter(|summary| summary.messages > 0)
                .max_by_key(|summary| summary.updated_at)
                .map(|summary| summary.id)
                .context("No non-empty saved session exists for this workspace")?
        } else {
            validate_id(id)?;
            id.to_string()
        };
        let path = directory.join(format!("{resolved_id}.json"));
        let content = std::fs::read_to_string(&path)
            .with_context(|| format!("Could not read session: {}", path.display()))?;
        let session: Self = serde_json::from_str(&content)
            .with_context(|| format!("Invalid session file: {}", path.display()))?;
        if session.workspace != expected_workspace {
            anyhow::bail!(
                "Session belongs to another workspace: {}",
                session.workspace
            );
        }
        Ok(session)
    }

    pub fn list() -> Result<Vec<SessionSummary>> {
        let workspace = canonical_workspace(&std::env::current_dir()?)?;
        list_from(&config::sessions_dir(), &workspace)
    }

    /// Export the session as a readable Markdown transcript.
    pub fn export_markdown(&self, path: &Path) -> Result<PathBuf> {
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent).with_context(|| {
                    format!("Could not create export directory: {}", parent.display())
                })?;
            }
        }
        let mut body = String::new();
        body.push_str("# g0d session export\n\n");
        body.push_str(&format!("- **id**: `{}`\n", self.id));
        body.push_str(&format!("- **workspace**: `{}`\n", self.workspace));
        if let Some(title) = &self.title {
            body.push_str(&format!("- **title**: {title}\n"));
        }
        body.push_str(&format!("- **created_at**: {}\n", self.created_at));
        body.push_str(&format!("- **updated_at**: {}\n", self.updated_at));
        body.push_str(&format!("- **messages**: {}\n\n", self.messages.len()));
        body.push_str("---\n\n");
        for (index, message) in self.messages.iter().enumerate() {
            let role = message
                .get("role")
                .and_then(Value::as_str)
                .unwrap_or("unknown");
            let content = message
                .get("content")
                .and_then(Value::as_str)
                .unwrap_or("")
                .trim();
            body.push_str(&format!("## {}. {role}\n\n", index + 1));
            if content.is_empty() {
                body.push_str("_(empty)_\n\n");
            } else {
                body.push_str(content);
                body.push_str("\n\n");
            }
        }
        std::fs::write(path, body)
            .with_context(|| format!("Could not write export: {}", path.display()))?;
        Ok(path.to_path_buf())
    }

    pub fn default_export_path(&self) -> PathBuf {
        let slug = self
            .title
            .as_deref()
            .unwrap_or("session")
            .chars()
            .map(|c| {
                if c.is_ascii_alphanumeric() {
                    c.to_ascii_lowercase()
                } else {
                    '-'
                }
            })
            .collect::<String>();
        let slug = slug.trim_matches('-');
        let slug = if slug.is_empty() { "session" } else { slug };
        let short_id: String = self.id.chars().take(8).collect();
        PathBuf::from(format!("g0d-{slug}-{short_id}.md"))
    }
}

fn list_from(directory: &Path, workspace: &str) -> Result<Vec<SessionSummary>> {
    if !directory.exists() {
        return Ok(Vec::new());
    }
    let mut sessions = Vec::new();
    for entry in std::fs::read_dir(directory)? {
        let Ok(entry) = entry else { continue };
        if entry.path().extension().and_then(|value| value.to_str()) != Some("json") {
            continue;
        }
        let Ok(content) = std::fs::read_to_string(entry.path()) else {
            continue;
        };
        let Ok(session) = serde_json::from_str::<Session>(&content) else {
            continue;
        };
        if session.workspace == workspace {
            sessions.push(SessionSummary {
                id: session.id,
                updated_at: session.updated_at,
                messages: session.messages.len(),
                title: session.title,
            });
        }
    }
    sessions.sort_by_key(|session| std::cmp::Reverse(session.updated_at));
    Ok(sessions)
}

fn canonical_workspace(path: &Path) -> Result<String> {
    Ok(path
        .canonicalize()
        .context("Could not resolve workspace path")?
        .to_string_lossy()
        .to_string())
}

fn validate_id(id: &str) -> Result<()> {
    if id.is_empty()
        || !id.chars().all(|character| {
            character.is_ascii_alphanumeric() || character == '-' || character == '_'
        })
    {
        anyhow::bail!("Invalid session id");
    }
    Ok(())
}

fn now_millis() -> Result<u64> {
    Ok(SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .context("System clock is before UNIX epoch")?
        .as_millis() as u64)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!("g0d-{name}-{}", std::process::id()))
    }

    #[test]
    fn saves_and_resumes_workspace_session() {
        let workspace = temp_dir("workspace");
        let store = temp_dir("session-store");
        std::fs::create_dir_all(&workspace).unwrap();
        let mut session = Session::new_in(&workspace).unwrap();
        session
            .messages
            .push(serde_json::json!({"role": "user", "content": "hello"}));
        session.save_to(&store).unwrap();
        let loaded = Session::load_from(&store, "latest", &workspace).unwrap();
        assert_eq!(loaded.id, session.id);
        assert_eq!(loaded.messages.len(), 1);
    }

    #[test]
    fn blocks_session_id_traversal() {
        assert!(validate_id("../other").is_err());
    }

    #[test]
    fn default_export_path_is_safe() {
        let session = Session {
            id: "abcd1234-session".into(),
            workspace: "E:/tmp".into(),
            created_at: 1,
            updated_at: 1,
            title: Some("Fix auth bug!".into()),
            messages: vec![],
            total_prompt_tokens: 0,
            total_completion_tokens: 0,
            last_prompt_tokens: 0,
            last_completion_tokens: 0,
            compact_count: 0,
        };
        let path = session.default_export_path();
        assert_eq!(
            path.file_name().and_then(|n| n.to_str()),
            Some("g0d-fix-auth-bug-abcd1234.md")
        );
    }

    #[test]
    fn exports_markdown_transcript() {
        let dir = temp_dir("export");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("out.md");
        let session = Session {
            id: "export1".into(),
            workspace: "E:/tmp".into(),
            created_at: 1,
            updated_at: 2,
            title: Some("demo".into()),
            messages: vec![
                serde_json::json!({"role": "user", "content": "hello"}),
                serde_json::json!({"role": "assistant", "content": "hi there"}),
            ],
            total_prompt_tokens: 0,
            total_completion_tokens: 0,
            last_prompt_tokens: 0,
            last_completion_tokens: 0,
            compact_count: 0,
        };
        session.export_markdown(&path).unwrap();
        let body = std::fs::read_to_string(&path).unwrap();
        assert!(body.contains("# g0d session export"));
        assert!(body.contains("hello"));
        assert!(body.contains("hi there"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn latest_ignores_newer_empty_sessions() {
        let workspace = temp_dir("latest-workspace");
        let store = temp_dir("latest-store");
        std::fs::create_dir_all(&workspace).unwrap();
        std::fs::create_dir_all(&store).unwrap();

        let mut meaningful = Session::new_in(&workspace).unwrap();
        meaningful.id = "meaningful".into();
        meaningful.updated_at = 10;
        meaningful
            .messages
            .push(serde_json::json!({"role": "assistant", "content": "checkpoint"}));
        std::fs::write(
            store.join("meaningful.json"),
            serde_json::to_vec(&meaningful).unwrap(),
        )
        .unwrap();

        let mut empty = Session::new_in(&workspace).unwrap();
        empty.id = "empty".into();
        empty.updated_at = 20;
        std::fs::write(
            store.join("empty.json"),
            serde_json::to_vec(&empty).unwrap(),
        )
        .unwrap();

        let loaded = Session::load_from(&store, "latest", &workspace).unwrap();
        assert_eq!(loaded.id, "meaningful");
    }
}
