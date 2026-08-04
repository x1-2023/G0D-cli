use crate::{
    config::{ApprovalMode, Config},
    context, status,
    terminal::TerminalState,
};
use anyhow::{Context, Result};
use serde_json::{json, Value};
use std::collections::VecDeque;
use std::io::{IsTerminal, Write};
use std::path::{Component, Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;
use tokio::io::AsyncWriteExt;
use tokio::process::Command;
use tokio::time::timeout;

const MAX_STEPS: usize = 20;
const MAX_TOOL_OUTPUT: usize = 40_000;
const MAX_READ_LINES: usize = 500;

pub async fn run(
    config: &Config,
    key: &str,
    query: &str,
    term: &TerminalState,
    session: &mut Vec<Value>,
) -> Result<()> {
    if session.is_empty() && is_vague_continuation(query) {
        let answer = "Session này chưa có nhiệm vụ trước đó để tiếp tục. Hãy chạy /resume latest \
                      để nạp session gần nhất có nội dung, hoặc mô tả rõ công việc cần làm.";
        println!("{answer}");
        session.push(json!({"role": "user", "content": query}));
        session.push(json!({"role": "assistant", "content": answer}));
        return Ok(());
    }
    let workspace = Workspace::discover()?;
    let project = context::read_context();
    let language = match config.lang.as_str() {
        "vi" => "Reply in Vietnamese.",
        "en" => "Reply in English.",
        _ => "Reply in the user's language.",
    };
    let shell_guidance = if cfg!(windows) {
        "This host is Windows. run_command defaults to Windows PowerShell 5.1. Never use cmd operators \
         such as && or || with the powershell shell. Select shell=cmd for cmd syntax, or use PowerShell \
         constructs such as Get-Command and $LASTEXITCODE. Prefer one efficient environment probe; after \
         two failed discovery attempts, stop probing and explain what is missing."
    } else {
        "This host is Unix. run_command uses sh syntax."
    };
    let system = format!(
        "You are g0d, an autonomous coding agent. {language}\n\
         Work only inside the workspace. Inspect real files before proposing or making edits. \
         Use list_files, search_files, read_file, and the read-only Git tools to gather evidence. \
         Use replace_in_file/create_file for focused edits, apply_patch for unified diffs, and \
         run_command for builds, tests, and other shell/Git operations. Approval mode is {}: when \
         on, commands and writes require explicit user approval. {shell_guidance} \
         After editing, re-read relevant files and report exactly what changed. Never claim a tool \
         succeeded unless its observation says it did. Stop if blocked; do not loop indefinitely.\n\n\
         Workspace root: {}\n{}",
        config.approval_mode.label(),
        workspace.root.display(),
        context::context_summary(&project)
    );

    let mut messages = vec![json!({"role": "system", "content": system})];
    messages.extend(session.iter().cloned());
    messages.push(json!({"role": "user", "content": query}));
    let client = reqwest::Client::builder()
        .connect_timeout(std::time::Duration::from_secs(15))
        .timeout(std::time::Duration::from_secs(180))
        .user_agent(format!("g0d/{}", env!("CARGO_PKG_VERSION")))
        .build()
        .context("Could not create HTTP client")?;
    let url = format!(
        "{}/chat/completions",
        config.active_provider().endpoint.trim_end_matches('/')
    );

    let mut trace = Vec::new();
    for step in 0..MAX_STEPS {
        let indicator = status::StatusIndicator::start(if step == 0 {
            "Inspecting task"
        } else {
            "Continuing"
        });
        let response = client
            .post(&url)
            .bearer_auth(key)
            .json(&json!({
                "model": &config.default_model,
                "messages": messages,
                "tools": tool_definitions(),
                "tool_choice": "auto",
                "stream": false,
                "temperature": 0.2,
                "max_tokens": 8192
            }))
            .send()
            .await
            .context("Agent API request failed")?;
        indicator.stop();

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            let detail: String = body.chars().take(1500).collect();
            anyhow::bail!("Agent API HTTP {status}: {detail}");
        }

        let payload: Value = response
            .json()
            .await
            .context("Provider returned invalid JSON")?;
        let message = payload
            .pointer("/choices/0/message")
            .cloned()
            .context("Provider response did not contain an assistant message")?;
        let calls = message
            .get("tool_calls")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();

        if calls.is_empty() {
            let answer = message
                .get("content")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .trim();
            if answer.is_empty() {
                anyhow::bail!("Agent stopped without a response");
            }
            println!("{answer}");
            session.push(json!({"role": "user", "content": query}));
            session.push(json!({"role": "assistant", "content": answer}));
            trim_session(session, config.max_context_messages);
            return Ok(());
        }

        messages.push(message);
        for call in calls {
            let id = call
                .get("id")
                .and_then(Value::as_str)
                .unwrap_or("tool-call");
            let function = call.get("function").cloned().unwrap_or_default();
            let name = function
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or("unknown");
            let arguments = function
                .get("arguments")
                .and_then(Value::as_str)
                .unwrap_or("{}");
            println!("{}", term.dim(&format!("→ {name}")));
            let observation =
                match execute_tool(&workspace, name, arguments, term, config.approval_mode).await {
                    Ok(output) => truncate(output, MAX_TOOL_OUTPUT),
                    Err(error) => format!("ERROR: {error:#}"),
                };
            println!(
                "{}",
                term.dim(&format!("  {}", one_line_summary(&observation)))
            );
            trace.push(format!("{name}: {}", one_line_summary(&observation)));
            messages.push(json!({
                "role": "tool",
                "tool_call_id": id,
                "content": observation
            }));
        }
    }

    let checkpoint_instruction = "The bounded tool budget is exhausted. Do not call tools. \
        Produce a concise checkpoint for the next session containing: original objective, evidence \
        found, files changed, commands/tests and results, unresolved blockers, and the exact next \
        action. Clearly state that the task is incomplete.";
    messages.push(json!({"role": "user", "content": checkpoint_instruction}));
    let checkpoint = match client
        .post(&url)
        .bearer_auth(key)
        .json(&json!({
            "model": &config.default_model,
            "messages": messages,
            "tool_choice": "none",
            "stream": false,
            "temperature": 0.1,
            "max_tokens": 1400
        }))
        .send()
        .await
    {
        Ok(response) if response.status().is_success() => {
            response.json::<Value>().await.ok().and_then(|payload| {
                payload
                    .pointer("/choices/0/message/content")
                    .and_then(Value::as_str)
                    .map(str::trim)
                    .filter(|content| !content.is_empty())
                    .map(str::to_string)
            })
        }
        _ => None,
    }
    .unwrap_or_else(|| {
        let recent = trace
            .iter()
            .rev()
            .take(12)
            .rev()
            .cloned()
            .collect::<Vec<_>>()
            .join("\n");
        format!(
            "Task incomplete: paused at the {MAX_STEPS}-step safety limit. \
             Recent tool observations:\n{recent}\nResume with a concrete next action."
        )
    });
    println!("{checkpoint}");
    println!(
        "{}",
        term.yellow("Checkpoint saved. Send 'tiếp tục' in this session to resume.")
    );
    session.push(json!({"role": "user", "content": query}));
    session.push(json!({"role": "assistant", "content": checkpoint}));
    trim_session(session, config.max_context_messages);
    Ok(())
}

fn is_vague_continuation(query: &str) -> bool {
    matches!(
        query.trim().to_lowercase().as_str(),
        "continue"
            | "continue work"
            | "continue the work"
            | "resume"
            | "tiếp tục"
            | "tiếp tục công việc"
            | "làm tiếp"
    )
}

fn tool_definitions() -> Value {
    json!([
        {
            "type": "function",
            "function": {
                "name": "list_files",
                "description": "List files and directories inside the workspace. Use this before guessing paths.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "path": {"type": "string", "description": "Workspace-relative directory; defaults to ."},
                        "recursive": {"type": "boolean", "description": "Recursively list up to 500 entries"}
                    }
                }
            }
        },
        {
            "type": "function",
            "function": {
                "name": "read_file",
                "description": "Read a UTF-8 text file with line numbers. Reads at most 500 lines per call.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "path": {"type": "string"},
                        "start_line": {"type": "integer", "minimum": 1},
                        "end_line": {"type": "integer", "minimum": 1}
                    },
                    "required": ["path"]
                }
            }
        },
        {
            "type": "function",
            "function": {
                "name": "search_files",
                "description": "Search UTF-8 workspace files for literal text and return file:line matches.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "query": {"type": "string"},
                        "path": {"type": "string", "description": "Workspace-relative directory; defaults to ."}
                    },
                    "required": ["query"]
                }
            }
        },
        {
            "type": "function",
            "function": {
                "name": "replace_in_file",
                "description": "Replace exactly one occurrence in an existing UTF-8 file. Fails if old_text is missing or appears more than once. Requires approval.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "path": {"type": "string"},
                        "old_text": {"type": "string"},
                        "new_text": {"type": "string"}
                    },
                    "required": ["path", "old_text", "new_text"]
                }
            }
        },
        {
            "type": "function",
            "function": {
                "name": "create_file",
                "description": "Create a new UTF-8 file. Never overwrites an existing file. Requires approval.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "path": {"type": "string"},
                        "content": {"type": "string"}
                    },
                    "required": ["path", "content"]
                }
            }
        },
        {
            "type": "function",
            "function": {
                "name": "run_command",
                "description": "Run a command inside the workspace for builds, tests, or Git operations. On Windows select powershell (default, Windows PowerShell 5.1 syntax) or cmd. Never mix shell syntaxes. Requires approval when approval mode is on.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "command": {"type": "string"},
                        "shell": {"type": "string", "enum": ["powershell", "cmd", "sh"], "description": "Shell syntax to use. Defaults to powershell on Windows and sh elsewhere."},
                        "cwd": {"type": "string", "description": "Workspace-relative directory; defaults to ."},
                        "timeout_seconds": {"type": "integer", "minimum": 1, "maximum": 120}
                    },
                    "required": ["command"]
                }
            }
        },
        {
            "type": "function",
            "function": {
                "name": "git_status",
                "description": "Show Git status for the workspace without modifying it.",
                "parameters": {"type": "object", "properties": {}}
            }
        },
        {
            "type": "function",
            "function": {
                "name": "git_diff",
                "description": "Show unstaged or staged Git diff without modifying it.",
                "parameters": {
                    "type": "object",
                    "properties": {"staged": {"type": "boolean"}}
                }
            }
        },
        {
            "type": "function",
            "function": {
                "name": "git_log",
                "description": "Show recent Git commits without modifying the repository.",
                "parameters": {
                    "type": "object",
                    "properties": {"limit": {"type": "integer", "minimum": 1, "maximum": 50}}
                }
            }
        },
        {
            "type": "function",
            "function": {
                "name": "apply_patch",
                "description": "Validate and apply a unified Git patch inside the workspace. Blocks .git, .env, absolute paths, and parent traversal. Requires approval when approval mode is on.",
                "parameters": {
                    "type": "object",
                    "properties": {"patch": {"type": "string"}},
                    "required": ["patch"]
                }
            }
        }
    ])
}

async fn execute_tool(
    workspace: &Workspace,
    name: &str,
    raw_arguments: &str,
    term: &TerminalState,
    approval_mode: ApprovalMode,
) -> Result<String> {
    let arguments: Value =
        serde_json::from_str(raw_arguments).context("Tool arguments were not valid JSON")?;
    match name {
        "list_files" => workspace.list_files(
            string_arg(&arguments, "path").unwrap_or("."),
            arguments
                .get("recursive")
                .and_then(Value::as_bool)
                .unwrap_or(false),
        ),
        "read_file" => workspace.read_file(
            required_string(&arguments, "path")?,
            arguments
                .get("start_line")
                .and_then(Value::as_u64)
                .unwrap_or(1) as usize,
            arguments
                .get("end_line")
                .and_then(Value::as_u64)
                .map(|value| value as usize),
        ),
        "search_files" => workspace.search_files(
            required_string(&arguments, "query")?,
            string_arg(&arguments, "path").unwrap_or("."),
        ),
        "replace_in_file" => workspace.replace_in_file(
            required_string(&arguments, "path")?,
            required_string(&arguments, "old_text")?,
            required_string(&arguments, "new_text")?,
            term,
            approval_mode,
        ),
        "create_file" => workspace.create_file(
            required_string(&arguments, "path")?,
            required_string(&arguments, "content")?,
            term,
            approval_mode,
        ),
        "run_command" => {
            workspace
                .run_command(
                    required_string(&arguments, "command")?,
                    string_arg(&arguments, "cwd").unwrap_or("."),
                    string_arg(&arguments, "shell").unwrap_or(default_shell()),
                    arguments
                        .get("timeout_seconds")
                        .and_then(Value::as_u64)
                        .unwrap_or(120),
                    term,
                    approval_mode,
                )
                .await
        }
        "git_status" => workspace.git_read(&["status", "--short", "--branch"]).await,
        "git_diff" => {
            if arguments
                .get("staged")
                .and_then(Value::as_bool)
                .unwrap_or(false)
            {
                workspace
                    .git_read(&["diff", "--cached", "--no-ext-diff"])
                    .await
            } else {
                workspace.git_read(&["diff", "--no-ext-diff"]).await
            }
        }
        "git_log" => {
            let limit = arguments
                .get("limit")
                .and_then(Value::as_u64)
                .unwrap_or(10)
                .clamp(1, 50);
            workspace
                .git_read(&["log", &format!("-{limit}"), "--oneline", "--decorate"])
                .await
        }
        "apply_patch" => {
            workspace
                .apply_patch(required_string(&arguments, "patch")?, term, approval_mode)
                .await
        }
        _ => anyhow::bail!("Unknown tool: {name}"),
    }
}

struct Workspace {
    root: PathBuf,
}

impl Workspace {
    fn discover() -> Result<Self> {
        Ok(Self {
            root: std::env::current_dir()?
                .canonicalize()
                .context("Could not resolve workspace root")?,
        })
    }

    fn list_files(&self, path: &str, recursive: bool) -> Result<String> {
        let start = self.existing_path(path)?;
        if !start.is_dir() {
            anyhow::bail!("Not a directory: {path}");
        }
        let mut queue = VecDeque::from([start]);
        let mut output = Vec::new();
        while let Some(directory) = queue.pop_front() {
            let mut entries: Vec<_> = std::fs::read_dir(&directory)?.flatten().collect();
            entries.sort_by_key(|entry| entry.file_name());
            for entry in entries {
                let entry_path = entry.path();
                if is_ignored(&entry_path) {
                    continue;
                }
                let relative = entry_path.strip_prefix(&self.root).unwrap_or(&entry_path);
                let is_dir = entry.file_type().map(|kind| kind.is_dir()).unwrap_or(false);
                output.push(format!(
                    "{}{}",
                    relative.display(),
                    if is_dir { "/" } else { "" }
                ));
                if recursive && is_dir {
                    queue.push_back(entry_path);
                }
                if output.len() >= 500 {
                    output.push("... truncated at 500 entries".into());
                    return Ok(output.join("\n"));
                }
            }
            if !recursive {
                break;
            }
        }
        Ok(if output.is_empty() {
            "(empty)".into()
        } else {
            output.join("\n")
        })
    }

    fn read_file(&self, path: &str, start_line: usize, end_line: Option<usize>) -> Result<String> {
        let resolved = self.existing_path(path)?;
        if !resolved.is_file() {
            anyhow::bail!("Not a file: {path}");
        }
        let content = std::fs::read_to_string(&resolved).context("File is not valid UTF-8 text")?;
        let start = start_line.max(1);
        let end = end_line.unwrap_or(start.saturating_add(MAX_READ_LINES - 1));
        if end < start {
            anyhow::bail!("end_line must be greater than or equal to start_line");
        }
        let mut lines = Vec::new();
        for (index, line) in content
            .lines()
            .enumerate()
            .skip(start - 1)
            .take((end - start + 1).min(MAX_READ_LINES))
        {
            lines.push(format!("{:>6} | {}", index + 1, line));
        }
        Ok(if lines.is_empty() {
            "(no lines in requested range)".into()
        } else {
            lines.join("\n")
        })
    }

    fn search_files(&self, query: &str, path: &str) -> Result<String> {
        if query.is_empty() {
            anyhow::bail!("Search query cannot be empty");
        }
        let start = self.existing_path(path)?;
        let mut queue = VecDeque::from([start]);
        let needle = query.to_lowercase();
        let mut matches = Vec::new();
        while let Some(candidate) = queue.pop_front() {
            if is_ignored(&candidate) {
                continue;
            }
            if candidate.is_dir() {
                for entry in std::fs::read_dir(candidate)?.flatten() {
                    queue.push_back(entry.path());
                }
                continue;
            }
            if std::fs::metadata(&candidate)
                .map(|meta| meta.len() > 1_000_000)
                .unwrap_or(true)
            {
                continue;
            }
            let Ok(content) = std::fs::read_to_string(&candidate) else {
                continue;
            };
            for (index, line) in content.lines().enumerate() {
                if line.to_lowercase().contains(&needle) {
                    let relative = candidate.strip_prefix(&self.root).unwrap_or(&candidate);
                    matches.push(format!(
                        "{}:{}: {}",
                        relative.display(),
                        index + 1,
                        line.trim()
                    ));
                    if matches.len() >= 100 {
                        matches.push("... truncated at 100 matches".into());
                        return Ok(matches.join("\n"));
                    }
                }
            }
        }
        Ok(if matches.is_empty() {
            "No matches".into()
        } else {
            matches.join("\n")
        })
    }

    fn replace_in_file(
        &self,
        path: &str,
        old_text: &str,
        new_text: &str,
        term: &TerminalState,
        approval_mode: ApprovalMode,
    ) -> Result<String> {
        if old_text.is_empty() {
            anyhow::bail!("old_text cannot be empty");
        }
        let resolved = self.existing_path(path)?;
        self.ensure_writable_target(&resolved)?;
        let content = std::fs::read_to_string(&resolved).context("File is not valid UTF-8 text")?;
        let occurrences = content.matches(old_text).count();
        if occurrences != 1 {
            anyhow::bail!("Expected old_text exactly once, found {occurrences}");
        }
        let preview = format!(
            "Replace {} bytes with {} bytes in {}",
            old_text.len(),
            new_text.len(),
            path
        );
        if !approve(&preview, term, approval_mode)? {
            return Ok("DENIED: user did not approve the edit".into());
        }
        std::fs::write(&resolved, content.replacen(old_text, new_text, 1))?;
        Ok(format!("Updated {path}"))
    }

    fn create_file(
        &self,
        path: &str,
        content: &str,
        term: &TerminalState,
        approval_mode: ApprovalMode,
    ) -> Result<String> {
        let resolved = self.new_path(path)?;
        if resolved.exists() {
            anyhow::bail!("File already exists: {path}");
        }
        self.ensure_writable_target(&resolved)?;
        let preview = format!("Create {} ({} bytes)", path, content.len());
        if !approve(&preview, term, approval_mode)? {
            return Ok("DENIED: user did not approve file creation".into());
        }
        if let Some(parent) = resolved.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&resolved, content)?;
        Ok(format!("Created {path}"))
    }

    async fn run_command(
        &self,
        command: &str,
        cwd: &str,
        shell: &str,
        timeout_seconds: u64,
        term: &TerminalState,
        approval_mode: ApprovalMode,
    ) -> Result<String> {
        if command.trim().is_empty() {
            anyhow::bail!("Command cannot be empty");
        }
        let directory = self.existing_path(cwd)?;
        if !directory.is_dir() {
            anyhow::bail!("Command cwd is not a directory: {cwd}");
        }
        if !approve(
            &format!("Run {shell} command in {cwd}: {command}"),
            term,
            approval_mode,
        )? {
            return Ok("DENIED: user did not approve command execution".into());
        }

        let mut process = shell_command(command, shell)?;
        process.current_dir(directory).kill_on_drop(true);
        let seconds = timeout_seconds.clamp(1, 120);
        let output = timeout(Duration::from_secs(seconds), process.output())
            .await
            .with_context(|| format!("Command timed out after {seconds}s"))??;
        Ok(format_process_output(
            output.status.code(),
            &output.stdout,
            &output.stderr,
        ))
    }

    async fn git_read(&self, args: &[&str]) -> Result<String> {
        let output = timeout(
            Duration::from_secs(30),
            Command::new("git")
                .args(args)
                .current_dir(&self.root)
                .kill_on_drop(true)
                .output(),
        )
        .await
        .context("Git command timed out after 30s")?
        .context("Could not start git; ensure it is installed and on PATH")?;
        Ok(format_process_output(
            output.status.code(),
            &output.stdout,
            &output.stderr,
        ))
    }

    async fn apply_patch(
        &self,
        patch: &str,
        term: &TerminalState,
        approval_mode: ApprovalMode,
    ) -> Result<String> {
        validate_patch(patch)?;
        let check = run_git_with_stdin(&self.root, &["apply", "--check", "-"], patch).await?;
        if !check.status.success() {
            anyhow::bail!(
                "Patch validation failed:\n{}",
                String::from_utf8_lossy(&check.stderr)
            );
        }
        if !approve("Apply unified patch", term, approval_mode)? {
            return Ok("DENIED: user did not approve patch application".into());
        }
        let output =
            run_git_with_stdin(&self.root, &["apply", "--whitespace=nowarn", "-"], patch).await?;
        if !output.status.success() {
            anyhow::bail!(
                "Patch application failed:\n{}",
                String::from_utf8_lossy(&output.stderr)
            );
        }
        Ok("Applied unified patch".into())
    }

    fn existing_path(&self, path: &str) -> Result<PathBuf> {
        let joined = self.join_relative(path)?;
        let resolved = joined
            .canonicalize()
            .with_context(|| format!("Path does not exist: {path}"))?;
        self.ensure_inside(&resolved)?;
        Ok(resolved)
    }

    fn new_path(&self, path: &str) -> Result<PathBuf> {
        let joined = self.join_relative(path)?;
        let parent = joined.parent().context("Path has no parent")?;
        let existing_parent = nearest_existing_parent(parent)?;
        let canonical_parent = existing_parent.canonicalize()?;
        self.ensure_inside(&canonical_parent)?;
        Ok(joined)
    }

    fn join_relative(&self, path: &str) -> Result<PathBuf> {
        let path = Path::new(path);
        if path.is_absolute() {
            anyhow::bail!("Absolute paths are not allowed");
        }
        if path
            .components()
            .any(|component| matches!(component, Component::ParentDir))
        {
            anyhow::bail!("Parent directory traversal is not allowed");
        }
        Ok(self.root.join(path))
    }

    fn ensure_inside(&self, path: &Path) -> Result<()> {
        if !path.starts_with(&self.root) {
            anyhow::bail!("Path escapes the workspace");
        }
        Ok(())
    }

    fn ensure_writable_target(&self, path: &Path) -> Result<()> {
        self.ensure_inside(path)?;
        let relative = path.strip_prefix(&self.root).unwrap_or(path);
        if relative
            .components()
            .any(|component| component.as_os_str() == ".git")
        {
            anyhow::bail!("Writing inside .git is blocked");
        }
        if path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name == ".env" || name.starts_with(".env."))
        {
            anyhow::bail!("Writing .env files is blocked; manage secrets manually");
        }
        Ok(())
    }
}

fn approve(description: &str, term: &TerminalState, approval_mode: ApprovalMode) -> Result<bool> {
    if approval_mode == ApprovalMode::Off {
        return Ok(true);
    }
    if !term.is_tty || !std::io::stdin().is_terminal() {
        return Ok(false);
    }
    print!(
        "{} [y/N]: ",
        term.yellow(&format!("Approve: {description}"))
    );
    std::io::stdout().flush()?;
    let mut answer = String::new();
    std::io::stdin().read_line(&mut answer)?;
    Ok(matches!(
        answer.trim().to_ascii_lowercase().as_str(),
        "y" | "yes" | "c" | "co" | "có"
    ))
}

fn default_shell() -> &'static str {
    if cfg!(windows) {
        "powershell"
    } else {
        "sh"
    }
}

fn shell_command(command: &str, shell: &str) -> Result<Command> {
    #[cfg(windows)]
    {
        match shell {
            "powershell" => {
                let mut process = Command::new("powershell.exe");
                process.args([
                    "-NoLogo",
                    "-NoProfile",
                    "-NonInteractive",
                    "-Command",
                    command,
                ]);
                Ok(process)
            }
            "cmd" => {
                let mut process = Command::new("cmd.exe");
                process.args(["/d", "/s", "/c", command]);
                Ok(process)
            }
            _ => anyhow::bail!("Unsupported Windows shell: {shell}"),
        }
    }
    #[cfg(not(windows))]
    {
        if shell != "sh" {
            anyhow::bail!("Unsupported shell on this host: {shell}");
        }
        let mut process = Command::new("sh");
        process.args(["-lc", command]);
        Ok(process)
    }
}

fn format_process_output(code: Option<i32>, stdout: &[u8], stderr: &[u8]) -> String {
    let mut parts = vec![format!(
        "exit_code: {}",
        code.map_or_else(|| "terminated".into(), |value| value.to_string())
    )];
    if !stdout.is_empty() {
        parts.push(format!(
            "stdout:\n{}",
            String::from_utf8_lossy(stdout).trim_end()
        ));
    }
    if !stderr.is_empty() {
        parts.push(format!(
            "stderr:\n{}",
            String::from_utf8_lossy(stderr).trim_end()
        ));
    }
    parts.join("\n")
}

async fn run_git_with_stdin(
    root: &Path,
    args: &[&str],
    input: &str,
) -> Result<std::process::Output> {
    let mut child = Command::new("git")
        .args(args)
        .current_dir(root)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .context("Could not start git; ensure it is installed and on PATH")?;
    child
        .stdin
        .take()
        .context("Could not open git stdin")?
        .write_all(input.as_bytes())
        .await?;
    timeout(Duration::from_secs(30), child.wait_with_output())
        .await
        .context("Git patch command timed out after 30s")?
        .context("Git patch command failed")
}

fn validate_patch(patch: &str) -> Result<()> {
    if patch.trim().is_empty() {
        anyhow::bail!("Patch cannot be empty");
    }
    let mut paths = Vec::new();
    for line in patch.lines() {
        if let Some(rest) = line.strip_prefix("diff --git ") {
            let mut fields = rest.split_whitespace();
            for _ in 0..2 {
                if let Some(path) = fields.next() {
                    paths.push(path);
                }
            }
        } else if let Some(path) = line
            .strip_prefix("--- ")
            .or_else(|| line.strip_prefix("+++ "))
        {
            paths.push(path.split('\t').next().unwrap_or(path).trim());
        }
    }
    if paths.is_empty() {
        anyhow::bail!("Patch has no file headers");
    }
    for raw in paths {
        if raw == "/dev/null" {
            continue;
        }
        if raw.starts_with('"') {
            anyhow::bail!("Quoted patch paths are not supported");
        }
        let stripped = raw
            .strip_prefix("a/")
            .or_else(|| raw.strip_prefix("b/"))
            .unwrap_or(raw);
        let path = Path::new(stripped);
        if path.is_absolute()
            || path
                .components()
                .any(|part| matches!(part, Component::ParentDir))
        {
            anyhow::bail!("Unsafe patch path: {raw}");
        }
        if path.components().any(|part| part.as_os_str() == ".git") {
            anyhow::bail!("Patch cannot write inside .git");
        }
        if path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name == ".env" || name.starts_with(".env."))
        {
            anyhow::bail!("Patch cannot write .env files");
        }
    }
    Ok(())
}

fn nearest_existing_parent(path: &Path) -> Result<PathBuf> {
    let mut current = path.to_path_buf();
    while !current.exists() {
        current = current
            .parent()
            .context("Could not resolve parent directory")?
            .to_path_buf();
    }
    Ok(current)
}

fn is_ignored(path: &Path) -> bool {
    path.components().any(|component| {
        matches!(
            component.as_os_str().to_str(),
            Some(".git" | "node_modules" | "target" | ".verify-target")
        )
    })
}

fn required_string<'a>(value: &'a Value, name: &str) -> Result<&'a str> {
    string_arg(value, name).with_context(|| format!("Missing string argument: {name}"))
}

fn string_arg<'a>(value: &'a Value, name: &str) -> Option<&'a str> {
    value.get(name).and_then(Value::as_str)
}

fn truncate(value: String, limit: usize) -> String {
    if value.chars().count() <= limit {
        return value;
    }
    value.chars().take(limit).collect::<String>() + "\n... tool output truncated"
}

fn one_line_summary(value: &str) -> String {
    let summary = value.lines().next().unwrap_or("(empty)");
    let mut result: String = summary.chars().take(140).collect();
    if summary.chars().count() > 140 {
        result.push_str("...");
    }
    result
}

fn trim_session(session: &mut Vec<Value>, limit: usize) {
    if session.len() > limit {
        session.drain(..session.len() - limit);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_parent_traversal_and_absolute_paths() {
        let workspace = Workspace {
            root: std::env::current_dir().unwrap().canonicalize().unwrap(),
        };
        assert!(workspace.join_relative("../secret").is_err());
        assert!(workspace.join_relative(r"C:\Windows\system.ini").is_err());
    }

    #[test]
    fn tool_registry_contains_only_curated_tools() {
        let tools = tool_definitions();
        assert_eq!(tools.as_array().unwrap().len(), 10);
    }

    #[test]
    fn patch_validation_blocks_sensitive_and_traversal_paths() {
        assert!(validate_patch("--- a/.env\n+++ b/.env\n").is_err());
        assert!(validate_patch("--- a/../outside.txt\n+++ b/../outside.txt\n").is_err());
        assert!(validate_patch("--- a/.git/config\n+++ b/.git/config\n").is_err());
        assert!(validate_patch("--- a/src/main.rs\n+++ b/src/main.rs\n").is_ok());
    }

    #[test]
    fn validates_shell_selection() {
        assert!(shell_command("echo ok", default_shell()).is_ok());
        assert!(shell_command("echo no", "unsupported").is_err());
    }

    #[test]
    fn detects_vague_continuation_requests() {
        assert!(is_vague_continuation("tiếp tục công việc"));
        assert!(is_vague_continuation(" continue "));
        assert!(!is_vague_continuation("continue fixing src/main.rs"));
    }
}
