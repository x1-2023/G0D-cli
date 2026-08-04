use crate::{
    config::{ApprovalMode, Config},
    context,
    output::{AgentEvent, EventSink},
    session::Session,
    status,
    terminal::TerminalState,
};
use anyhow::{Context, Result};
use serde_json::{json, Value};
use std::collections::VecDeque;
use std::io::IsTerminal;
use std::path::{Component, Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;
use tokio::io::AsyncWriteExt;
use tokio::process::Command;
use tokio::time::timeout;

const MAX_TOOL_OUTPUT: usize = 40_000;
const MAX_READ_LINES: usize = 500;
const MAX_GLOB_MATCHES: usize = 200;
const MAX_IDENTICAL_ERRORS: usize = 3;

/// Optional per-run overrides for the agent loop.
#[derive(Debug, Clone, Copy, Default)]
pub struct RunOptions {
    pub max_steps: Option<usize>,
    pub approval: Option<ApprovalMode>,
}

#[derive(Default)]
struct TokenUsage {
    prompt: u64,
    completion: u64,
}

impl TokenUsage {
    fn absorb(&mut self, payload: &Value) {
        let usage = payload.get("usage");
        if let Some(prompt) = usage
            .and_then(|value| value.get("prompt_tokens"))
            .and_then(Value::as_u64)
        {
            self.prompt = self.prompt.saturating_add(prompt);
        }
        if let Some(completion) = usage
            .and_then(|value| value.get("completion_tokens"))
            .and_then(Value::as_u64)
        {
            self.completion = self.completion.saturating_add(completion);
        }
    }

    fn total(&self) -> u64 {
        self.prompt.saturating_add(self.completion)
    }

    fn is_empty(&self) -> bool {
        self.prompt == 0 && self.completion == 0
    }
}

pub async fn run(
    config: &Config,
    key: &str,
    query: &str,
    term: &TerminalState,
    session: &mut Session,
    options: RunOptions,
    sink: &mut dyn EventSink,
) -> Result<()> {
    if session.messages.is_empty() && is_vague_continuation(query) {
        let answer = "Session này chưa có nhiệm vụ trước đó để tiếp tục. Hãy chạy /resume latest \
                      để nạp session gần nhất có nội dung, hoặc mô tả rõ công việc cần làm.";
        sink.emit(AgentEvent::Assistant(answer.into()));
        session.ensure_title(query);
        session
            .messages
            .push(json!({"role": "user", "content": query}));
        session
            .messages
            .push(json!({"role": "assistant", "content": answer}));
        let _ = session.save();
        return Ok(());
    }

    session.ensure_title(query);
    if let Some(note) = session.compact(config, false) {
        sink.emit(AgentEvent::Notice(note));
        let _ = session.save();
    }
    let workspace = Workspace::discover()?;
    let project = context::read_context();
    let max_steps = options
        .max_steps
        .unwrap_or(config.max_agent_steps)
        .clamp(1, 50);
    let approval_mode = options.approval.unwrap_or(config.approval_mode);
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
    let instructions_note = if project.instructions.is_some() {
        " Honor project instructions below when they do not conflict with safety rules."
    } else {
        ""
    };
    let system = format!(
        "You are g0d, an autonomous coding agent. {language}\n\
         Work only inside the workspace. Inspect real files before proposing or making edits. \
         Use list_files, glob_files, search_files, read_file, and the read-only Git tools to gather \
         evidence. Use replace_in_file for surgical edits, write_file/create_file for new or full-file \
         rewrites, delete_file/rename_file for removals and renames, apply_patch for unified diffs, \
         git_add/git_commit for staging commits, and run_command for builds, tests, and other shell \
         operations. Approval mode is {}: when on, commands and writes require explicit user approval. \
         {shell_guidance}{instructions_note} After editing, re-read relevant files and report exactly \
         what changed. Never claim a tool succeeded unless its observation says it did. Stop if blocked; \
         do not loop the same failing tool call.\n\n\
         Workspace root: {}\n{}",
        approval_mode.label(),
        workspace.root.display(),
        context::context_summary(&project)
    );

    let mut messages = vec![json!({"role": "system", "content": system})];
    messages.extend(session.messages.iter().cloned());
    messages.push(json!({"role": "user", "content": query}));
    let client = crate::api::http_client()?;
    let url = crate::api::chat_completions_url(config);

    let mut usage = TokenUsage::default();
    let mut trace = Vec::new();
    let mut last_error_sig: Option<String> = None;
    let mut identical_error_streak = 0usize;
    let mut stuck_on_errors = false;
    for step in 0..max_steps {
        let status_label = if step == 0 {
            format!("Thinking · step 1/{max_steps}")
        } else {
            format!("Thinking · step {}/{max_steps}", step + 1)
        };
        sink.emit(AgentEvent::Status(status_label.clone()));
        let indicator = status::StatusIndicator::start(&status_label);
        let body = json!({
            "model": &config.default_model,
            "messages": messages,
            "tools": tool_definitions(),
            "tool_choice": "auto",
            "stream": false,
            "temperature": 0.2,
            "max_tokens": 8192
        });
        let builder = client.post(&url).json(&body);
        let builder = crate::api::apply_provider_auth(builder, config.active_provider(), key)?;
        let response = builder
            .send()
            .await
            .with_context(|| format!("Agent API request failed ({url})"))?;
        indicator.stop();
        sink.emit(AgentEvent::ClearStatus);

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            anyhow::bail!(crate::api::format_http_error(status, &body, &url));
        }

        let payload: Value = response
            .json()
            .await
            .context("Provider returned invalid JSON")?;
        usage.absorb(&payload);
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
            sink.emit(AgentEvent::Assistant(answer.into()));
            finish_session(session, query, answer, config.max_context_messages, &usage)?;
            print_usage(config, &usage, session, sink);
            return Ok(());
        }

        if let Some(content) = message.get("content").and_then(Value::as_str) {
            let content = content.trim();
            if !content.is_empty() {
                sink.emit(AgentEvent::Assistant(content.into()));
            }
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
            let preview = tool_arg_preview(name, arguments);
            sink.emit(AgentEvent::Tool {
                name: name.into(),
                preview: preview.clone(),
            });
            let observation = match execute_tool(
                &workspace,
                name,
                arguments,
                term,
                approval_mode,
                sink,
            )
            .await
            {
                Ok(output) => truncate(output, MAX_TOOL_OUTPUT),
                Err(error) => format!("ERROR: {error:#}"),
            };
            let summary = one_line_summary(&observation);
            sink.emit(AgentEvent::ToolResult(summary.clone()));
            trace.push(format!("{name}: {summary}"));
            if observation.starts_with("ERROR:") {
                let sig = format!("{name}|{summary}");
                if last_error_sig.as_deref() == Some(sig.as_str()) {
                    identical_error_streak += 1;
                } else {
                    last_error_sig = Some(sig);
                    identical_error_streak = 1;
                }
                if identical_error_streak >= MAX_IDENTICAL_ERRORS {
                    stuck_on_errors = true;
                }
            } else {
                last_error_sig = None;
                identical_error_streak = 0;
            }
            messages.push(json!({
                "role": "tool",
                "tool_call_id": id,
                "content": observation
            }));
        }
        // Persist a crash-safe progress snapshot without mutating the live session yet.
        persist_progress_snapshot(session, query, &trace);
        if stuck_on_errors {
            sink.emit(AgentEvent::Warn(format!(
                "Stopped after {MAX_IDENTICAL_ERRORS} identical tool errors."
            )));
            break;
        }
    }

    let checkpoint_instruction = if stuck_on_errors {
        "The agent repeated the same tool error and was stopped. Do not call tools. Produce a concise \
         checkpoint: original objective, what failed and why, evidence found, files changed, and the \
         exact next action a human or later session should take. State that the task is incomplete."
    } else {
        "The bounded tool budget is exhausted. Do not call tools. \
        Produce a concise checkpoint for the next session containing: original objective, evidence \
        found, files changed, commands/tests and results, unresolved blockers, and the exact next \
        action. Clearly state that the task is incomplete."
    };
    messages.push(json!({"role": "user", "content": checkpoint_instruction}));
    let checkpoint_body = json!({
        "model": &config.default_model,
        "messages": messages,
        "tool_choice": "none",
        "stream": false,
        "temperature": 0.1,
        "max_tokens": 1400
    });
    let checkpoint = match (async {
        let builder = client.post(&url).json(&checkpoint_body);
        let builder = crate::api::apply_provider_auth(builder, config.active_provider(), key)?;
        builder.send().await.map_err(anyhow::Error::from)
    })
    .await
    {
        Ok(response) if response.status().is_success() => {
            if let Ok(payload) = response.json::<Value>().await {
                usage.absorb(&payload);
                payload
                    .pointer("/choices/0/message/content")
                    .and_then(Value::as_str)
                    .map(str::trim)
                    .filter(|content| !content.is_empty())
                    .map(str::to_string)
            } else {
                None
            }
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
            "Task incomplete: paused at the {max_steps}-step safety limit. \
             Recent tool observations:\n{recent}\nResume with a concrete next action."
        )
    });
    sink.emit(AgentEvent::Assistant(checkpoint.clone()));
    sink.emit(AgentEvent::Warn(
        "Checkpoint saved. Send 'tiếp tục' in this session to resume.".into(),
    ));
    finish_session(session, query, &checkpoint, config.max_context_messages, &usage)?;
    print_usage(config, &usage, session, sink);
    Ok(())
}

fn finish_session(
    session: &mut Session,
    query: &str,
    answer: &str,
    max_messages: usize,
    usage: &TokenUsage,
) -> Result<()> {
    session
        .messages
        .push(json!({"role": "user", "content": query}));
    session
        .messages
        .push(json!({"role": "assistant", "content": answer}));
    trim_session(&mut session.messages, max_messages);
    session.record_usage(usage.prompt, usage.completion);
    session.save()?;
    Ok(())
}

fn persist_progress_snapshot(session: &mut Session, query: &str, trace: &[String]) {
    let recent = trace
        .iter()
        .rev()
        .take(20)
        .rev()
        .cloned()
        .collect::<Vec<_>>()
        .join("\n");
    let progress = format!(
        "Work in progress after {} tool observation(s):\n{recent}\n\
         Resume with a concrete next action if this turn was interrupted.",
        trace.len()
    );
    let mut snapshot = session.messages.clone();
    snapshot.push(json!({"role": "user", "content": query}));
    snapshot.push(json!({"role": "assistant", "content": progress}));
    let original = std::mem::replace(&mut session.messages, snapshot);
    let _ = session.save();
    session.messages = original;
}

fn print_usage(config: &Config, usage: &TokenUsage, session: &Session, sink: &mut dyn EventSink) {
    let meter = session.meter(config);
    let line = if usage.is_empty() {
        format!(
            "context · {} {} · est ~{} / {} ({}%)",
            meter.bar(12),
            meter.messages,
            crate::meter::format_token_count(meter.estimated_tokens),
            crate::meter::format_token_count(meter.budget),
            meter.usage_pct().min(100)
        )
    } else {
        format!(
            "tokens · turn p{} / c{} (Σ{}) · {} · lifetime Σ{} · compact×{}",
            usage.prompt,
            usage.completion,
            usage.total(),
            meter.short_label(),
            session
                .total_prompt_tokens
                .saturating_add(session.total_completion_tokens),
            session.compact_count
        )
    };
    sink.emit(AgentEvent::Usage(line));
}

fn tool_arg_preview(name: &str, arguments: &str) -> String {
    let Ok(value) = serde_json::from_str::<Value>(arguments) else {
        return String::new();
    };
    let preview = match name {
        "list_files" | "glob_files" => string_arg(&value, "path")
            .or_else(|| string_arg(&value, "pattern"))
            .unwrap_or("."),
        "read_file" | "create_file" | "write_file" | "delete_file" | "replace_in_file" => {
            string_arg(&value, "path").unwrap_or("?")
        }
        "rename_file" => {
            let from = string_arg(&value, "from").unwrap_or("?");
            let to = string_arg(&value, "to").unwrap_or("?");
            return format!("({from} → {to})");
        }
        "search_files" => string_arg(&value, "query").unwrap_or("?"),
        "run_command" => string_arg(&value, "command").unwrap_or("?"),
        "apply_patch" => "unified patch",
        "git_add" => {
            let paths = value
                .get("paths")
                .and_then(Value::as_array)
                .map(|items| {
                    items
                        .iter()
                        .filter_map(Value::as_str)
                        .take(3)
                        .collect::<Vec<_>>()
                        .join(", ")
                })
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| "?".into());
            let mut short: String = paths.chars().take(80).collect();
            if paths.chars().count() > 80 {
                short.push('…');
            }
            return format!("({short})");
        }
        "git_commit" => string_arg(&value, "message").unwrap_or("?"),
        "git_diff" => {
            if value
                .get("staged")
                .and_then(Value::as_bool)
                .unwrap_or(false)
            {
                "staged"
            } else {
                "unstaged"
            }
        }
        "git_log" => "recent",
        "git_status" => "workspace",
        _ => return String::new(),
    };
    let mut short: String = preview.chars().take(80).collect();
    if preview.chars().count() > 80 {
        short.push('…');
    }
    format!("({short})")
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
                "name": "glob_files",
                "description": "Find workspace files by glob pattern. Supports * (segment), ** (recursive), and ?. Skips .git, node_modules, and target.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "pattern": {"type": "string", "description": "Glob relative to the workspace root, e.g. src/**/*.rs"},
                        "path": {"type": "string", "description": "Workspace-relative start directory; defaults to ."}
                    },
                    "required": ["pattern"]
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
                "description": "Replace text in an existing UTF-8 file. By default requires exactly one match; set replace_all=true to replace every occurrence. Requires approval.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "path": {"type": "string"},
                        "old_text": {"type": "string"},
                        "new_text": {"type": "string"},
                        "replace_all": {"type": "boolean", "description": "Replace all occurrences instead of exactly one"}
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
                "name": "write_file",
                "description": "Create or overwrite a UTF-8 file with full content. Prefer replace_in_file for small edits. Requires approval.",
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
                "name": "delete_file",
                "description": "Delete a file inside the workspace. Blocks .git and .env targets. Requires approval.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "path": {"type": "string"}
                    },
                    "required": ["path"]
                }
            }
        },
        {
            "type": "function",
            "function": {
                "name": "rename_file",
                "description": "Rename or move a file inside the workspace. Destination must not already exist. Blocks .git and .env targets. Requires approval.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "from": {"type": "string"},
                        "to": {"type": "string"}
                    },
                    "required": ["from", "to"]
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
                "name": "git_add",
                "description": "Stage workspace-relative paths with git add. Does not commit. Requires approval when approval mode is on.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "paths": {
                            "type": "array",
                            "items": {"type": "string"},
                            "description": "One or more workspace-relative paths to stage"
                        }
                    },
                    "required": ["paths"]
                }
            }
        },
        {
            "type": "function",
            "function": {
                "name": "git_commit",
                "description": "Create a git commit from already staged changes using a plain message. Does not push, amend, or skip hooks. Requires approval when approval mode is on.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "message": {"type": "string", "description": "Commit message"}
                    },
                    "required": ["message"]
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
    sink: &mut dyn EventSink,
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
        "glob_files" => workspace.glob_files(
            required_string(&arguments, "pattern")?,
            string_arg(&arguments, "path").unwrap_or("."),
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
            arguments
                .get("replace_all")
                .and_then(Value::as_bool)
                .unwrap_or(false),
            term,
            approval_mode,
            sink,
        ),
        "create_file" => workspace.create_file(
            required_string(&arguments, "path")?,
            required_string(&arguments, "content")?,
            term,
            approval_mode,
            sink,
        ),
        "write_file" => workspace.write_file(
            required_string(&arguments, "path")?,
            required_string(&arguments, "content")?,
            term,
            approval_mode,
            sink,
        ),
        "delete_file" => workspace.delete_file(
            required_string(&arguments, "path")?,
            term,
            approval_mode,
            sink,
        ),
        "rename_file" => workspace.rename_file(
            required_string(&arguments, "from")?,
            required_string(&arguments, "to")?,
            term,
            approval_mode,
            sink,
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
                    sink,
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
        "git_add" => {
            let paths = arguments
                .get("paths")
                .and_then(Value::as_array)
                .context("Missing paths array for git_add")?
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect::<Vec<_>>();
            workspace
                .git_add(&paths, term, approval_mode, sink)
                .await
        }
        "git_commit" => {
            workspace
                .git_commit(
                    required_string(&arguments, "message")?,
                    term,
                    approval_mode,
                    sink,
                )
                .await
        }
        "apply_patch" => {
            workspace
                .apply_patch(
                    required_string(&arguments, "patch")?,
                    term,
                    approval_mode,
                    sink,
                )
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

    fn glob_files(&self, pattern: &str, path: &str) -> Result<String> {
        if pattern.trim().is_empty() {
            anyhow::bail!("Glob pattern cannot be empty");
        }
        let start = self.existing_path(path)?;
        if !start.is_dir() {
            anyhow::bail!("Not a directory: {path}");
        }
        let relative_start = start
            .strip_prefix(&self.root)
            .unwrap_or(Path::new(""))
            .to_path_buf();
        let full_pattern = if relative_start.as_os_str().is_empty() {
            pattern.replace('\\', "/")
        } else {
            format!(
                "{}/{}",
                relative_start.to_string_lossy().replace('\\', "/"),
                pattern.trim_start_matches(['/', '\\']).replace('\\', "/")
            )
        };
        let mut matches = Vec::new();
        let mut queue = VecDeque::from([start]);
        while let Some(directory) = queue.pop_front() {
            let mut entries: Vec<_> = std::fs::read_dir(&directory)?.flatten().collect();
            entries.sort_by_key(|entry| entry.file_name());
            for entry in entries {
                let entry_path = entry.path();
                if is_ignored(&entry_path) {
                    continue;
                }
                let is_dir = entry.file_type().map(|kind| kind.is_dir()).unwrap_or(false);
                if is_dir {
                    queue.push_back(entry_path);
                    continue;
                }
                let relative = entry_path
                    .strip_prefix(&self.root)
                    .unwrap_or(&entry_path)
                    .to_string_lossy()
                    .replace('\\', "/");
                if glob_match(&full_pattern, &relative) {
                    matches.push(relative);
                    if matches.len() >= MAX_GLOB_MATCHES {
                        matches.push(format!("... truncated at {MAX_GLOB_MATCHES} matches"));
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
        replace_all: bool,
        term: &TerminalState,
        approval_mode: ApprovalMode,
        sink: &mut dyn EventSink,
    ) -> Result<String> {
        if old_text.is_empty() {
            anyhow::bail!("old_text cannot be empty");
        }
        let resolved = self.existing_path(path)?;
        self.ensure_writable_target(&resolved)?;
        let content = std::fs::read_to_string(&resolved).context("File is not valid UTF-8 text")?;
        let occurrences = content.matches(old_text).count();
        if occurrences == 0 {
            anyhow::bail!("old_text was not found");
        }
        if !replace_all && occurrences != 1 {
            anyhow::bail!("Expected old_text exactly once, found {occurrences}. Set replace_all=true to replace all.");
        }
        let preview = format!(
            "Replace {occurrences} occurrence(s) in {path} ({} -> {} bytes)",
            old_text.len(),
            new_text.len()
        );
        if !approve(&preview, term, approval_mode, sink)? {
            return Ok("DENIED: user did not approve the edit".into());
        }
        let updated = if replace_all {
            content.replace(old_text, new_text)
        } else {
            content.replacen(old_text, new_text, 1)
        };
        std::fs::write(&resolved, updated)?;
        Ok(format!("Updated {path} ({occurrences} replacement(s))"))
    }

    fn create_file(
        &self,
        path: &str,
        content: &str,
        term: &TerminalState,
        approval_mode: ApprovalMode,
        sink: &mut dyn EventSink,
    ) -> Result<String> {
        let resolved = self.new_path(path)?;
        if resolved.exists() {
            anyhow::bail!("File already exists: {path}");
        }
        self.ensure_writable_target(&resolved)?;
        let preview = format!("Create {} ({} bytes)", path, content.len());
        if !approve(&preview, term, approval_mode, sink)? {
            return Ok("DENIED: user did not approve file creation".into());
        }
        if let Some(parent) = resolved.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&resolved, content)?;
        Ok(format!("Created {path}"))
    }

    fn write_file(
        &self,
        path: &str,
        content: &str,
        term: &TerminalState,
        approval_mode: ApprovalMode,
        sink: &mut dyn EventSink,
    ) -> Result<String> {
        let joined = self.join_relative(path)?;
        let resolved = if joined.exists() {
            self.existing_path(path)?
        } else {
            self.new_path(path)?
        };
        self.ensure_writable_target(&resolved)?;
        let action = if resolved.exists() {
            "Overwrite"
        } else {
            "Create"
        };
        let preview = format!("{action} {} ({} bytes)", path, content.len());
        if !approve(&preview, term, approval_mode, sink)? {
            return Ok("DENIED: user did not approve file write".into());
        }
        if let Some(parent) = resolved.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&resolved, content)?;
        Ok(format!("{action}d {path}"))
    }

    fn delete_file(
        &self,
        path: &str,
        term: &TerminalState,
        approval_mode: ApprovalMode,
        sink: &mut dyn EventSink,
    ) -> Result<String> {
        let resolved = self.existing_path(path)?;
        if !resolved.is_file() {
            anyhow::bail!("Not a file: {path}");
        }
        self.ensure_writable_target(&resolved)?;
        let preview = format!("Delete {path}");
        if !approve(&preview, term, approval_mode, sink)? {
            return Ok("DENIED: user did not approve deletion".into());
        }
        std::fs::remove_file(&resolved)
            .with_context(|| format!("Could not delete {path}"))?;
        Ok(format!("Deleted {path}"))
    }

    fn rename_file(
        &self,
        from: &str,
        to: &str,
        term: &TerminalState,
        approval_mode: ApprovalMode,
        sink: &mut dyn EventSink,
    ) -> Result<String> {
        let source = self.existing_path(from)?;
        if !source.is_file() {
            anyhow::bail!("Not a file: {from}");
        }
        self.ensure_writable_target(&source)?;
        let destination = self.new_path(to)?;
        if destination.exists() {
            anyhow::bail!("Destination already exists: {to}");
        }
        self.ensure_writable_target(&destination)?;
        let preview = format!("Rename {from} → {to}");
        if !approve(&preview, term, approval_mode, sink)? {
            return Ok("DENIED: user did not approve rename".into());
        }
        if let Some(parent) = destination.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::rename(&source, &destination)
            .with_context(|| format!("Could not rename {from} to {to}"))?;
        Ok(format!("Renamed {from} → {to}"))
    }

    async fn git_add(
        &self,
        paths: &[String],
        term: &TerminalState,
        approval_mode: ApprovalMode,
        sink: &mut dyn EventSink,
    ) -> Result<String> {
        if paths.is_empty() {
            anyhow::bail!("git_add requires at least one path");
        }
        let mut relative = Vec::new();
        for path in paths {
            let staged = self.stageable_path(path)?;
            let display = staged
                .strip_prefix(&self.root)
                .unwrap_or(&staged)
                .to_string_lossy()
                .replace('\\', "/");
            relative.push(display);
        }
        let preview = format!("git add {}", relative.join(" "));
        if !approve(&preview, term, approval_mode, sink)? {
            return Ok("DENIED: user did not approve git add".into());
        }
        let mut args = vec!["add".to_string(), "--".to_string()];
        args.extend(relative.iter().cloned());
        let arg_refs: Vec<&str> = args.iter().map(String::as_str).collect();
        self.git_read(&arg_refs).await
    }

    async fn git_commit(
        &self,
        message: &str,
        term: &TerminalState,
        approval_mode: ApprovalMode,
        sink: &mut dyn EventSink,
    ) -> Result<String> {
        let message = message.trim();
        if message.is_empty() {
            anyhow::bail!("Commit message cannot be empty");
        }
        if message.starts_with('-') {
            anyhow::bail!("Commit message cannot start with '-'");
        }
        let preview: String = format!(
            "git commit -m {}",
            message.chars().take(80).collect::<String>()
        );
        if !approve(&preview, term, approval_mode, sink)? {
            return Ok("DENIED: user did not approve git commit".into());
        }
        let output = run_git_with_stdin(
            &self.root,
            &["commit", "-F", "-"],
            &format!("{message}\n"),
        )
        .await?;
        Ok(format_process_output(
            output.status.code(),
            &output.stdout,
            &output.stderr,
        ))
    }

    fn stageable_path(&self, path: &str) -> Result<PathBuf> {
        let joined = self.join_relative(path)?;
        if joined.exists() {
            let resolved = joined
                .canonicalize()
                .with_context(|| format!("Path does not exist: {path}"))?;
            self.ensure_inside(&resolved)?;
            self.ensure_writable_target(&resolved)?;
            return Ok(joined);
        }
        // Allow staging deleted paths whose parent remains inside the workspace.
        let parent = joined
            .parent()
            .map(nearest_existing_parent)
            .transpose()?
            .context("Path has no parent")?;
        let canonical_parent = parent.canonicalize()?;
        self.ensure_inside(&canonical_parent)?;
        if path
            .split(['/', '\\'])
            .any(|part| part == ".git")
            || Path::new(path)
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name == ".env" || name.starts_with(".env."))
        {
            anyhow::bail!("Staging .git or .env paths is blocked");
        }
        Ok(joined)
    }

    async fn run_command(
        &self,
        command: &str,
        cwd: &str,
        shell: &str,
        timeout_seconds: u64,
        term: &TerminalState,
        approval_mode: ApprovalMode,
        sink: &mut dyn EventSink,
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
            sink,
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
        sink: &mut dyn EventSink,
    ) -> Result<String> {
        validate_patch(patch)?;
        let check = run_git_with_stdin(&self.root, &["apply", "--check", "-"], patch).await?;
        if !check.status.success() {
            anyhow::bail!(
                "Patch validation failed:\n{}",
                String::from_utf8_lossy(&check.stderr)
            );
        }
        if !approve("Apply unified patch", term, approval_mode, sink)? {
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

fn approve(
    description: &str,
    term: &TerminalState,
    approval_mode: ApprovalMode,
    sink: &mut dyn EventSink,
) -> Result<bool> {
    if approval_mode == ApprovalMode::Off {
        return Ok(true);
    }
    let (tx, rx) = std::sync::mpsc::channel();
    sink.emit(AgentEvent::NeedApproval {
        description: description.to_string(),
        response: tx,
    });
    match rx.recv() {
        Ok(value) => Ok(value),
        Err(_) => {
            // Fallback if the sink dropped the channel without answering.
            if term.is_tty && std::io::stdin().is_terminal() {
                crate::output::console_approve(description, term)
            } else {
                Ok(false)
            }
        }
    }
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

/// Minimal glob matcher: `*` (one segment), `**` (any depth), `?` (one char).
fn glob_match(pattern: &str, path: &str) -> bool {
    let pattern = pattern.replace('\\', "/");
    let path = path.replace('\\', "/");
    glob_match_parts(
        &pattern.split('/').filter(|part| !part.is_empty()).collect::<Vec<_>>(),
        &path.split('/').filter(|part| !part.is_empty()).collect::<Vec<_>>(),
    )
}

fn glob_match_parts(pattern: &[&str], path: &[&str]) -> bool {
    let mut p = 0usize;
    let mut s = 0usize;
    while p < pattern.len() {
        if pattern[p] == "**" {
            if p + 1 == pattern.len() {
                return true;
            }
            while s <= path.len() {
                if glob_match_parts(&pattern[p + 1..], &path[s..]) {
                    return true;
                }
                s += 1;
            }
            return false;
        }
        if s >= path.len() {
            return false;
        }
        if !segment_match(pattern[p], path[s]) {
            return false;
        }
        p += 1;
        s += 1;
    }
    s == path.len()
}

fn segment_match(pattern: &str, segment: &str) -> bool {
    let pattern: Vec<char> = pattern.chars().collect();
    let segment: Vec<char> = segment.chars().collect();
    let mut dp = vec![false; segment.len() + 1];
    dp[0] = true;
    for (i, pattern_char) in pattern.iter().enumerate() {
        let mut next = vec![false; segment.len() + 1];
        if *pattern_char == '*' {
            next[0] = dp[0];
            for j in 1..=segment.len() {
                next[j] = next[j - 1] || dp[j];
            }
        } else {
            for j in 1..=segment.len() {
                let matches = *pattern_char == '?' || *pattern_char == segment[j - 1];
                next[j] = matches && dp[j - 1];
            }
        }
        let _ = i;
        dp = next;
    }
    dp[segment.len()]
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
        assert_eq!(tools.as_array().unwrap().len(), 16);
        let names: Vec<_> = tools
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|tool| tool.pointer("/function/name").and_then(Value::as_str))
            .collect();
        for expected in [
            "glob_files",
            "write_file",
            "delete_file",
            "rename_file",
            "replace_in_file",
            "git_add",
            "git_commit",
            "run_command",
        ] {
            assert!(names.contains(&expected), "missing tool {expected}");
        }
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

    #[test]
    fn glob_matches_nested_rust_sources() {
        assert!(glob_match("src/**/*.rs", "src/main.rs"));
        assert!(glob_match("src/**/*.rs", "src/agent/tools.rs"));
        assert!(!glob_match("src/**/*.rs", "src/main.toml"));
        assert!(glob_match("*.md", "README.md"));
        assert!(!glob_match("*.md", "docs/README.md"));
    }
}
