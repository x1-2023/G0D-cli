use crate::config;
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
    pub messages: Vec<Value>,
}

#[derive(Debug, Clone)]
pub struct SessionSummary {
    pub id: String,
    pub updated_at: u64,
    pub messages: usize,
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
            messages: Vec::new(),
        })
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
